"""Backup permissions, encrypted content, immutable commands, cancellation and restart leases."""
import base64
import hashlib
import json
import os
import pathlib
import subprocess
import time
import uuid
import urllib.request
import urllib.error
NEEDS_RUNTIME = True


def run(*, base, owner, sql, password, client, request, restart, login, runtime):
    endpoint='/admin/api/instance/operations'
    def call(path=endpoint, method='GET', data=None, status=200, http=None):
        return request(http or owner,runtime()[1],path,method,data,status)
    member=login('member@example.test')
    instance=call('/admin/api/instance')
    key='81'*32
    def body():return dict(commandId=str(uuid.uuid4()),instanceId=instance['id'],generation=instance['generation'],recoveryKey=key)
    call(status=401,http=client());call(status=403,http=member)
    call(method='POST',data=body(),status=403,http=member)
    assert call('/admin/api/system')['capabilities']['instance.backup']['allowed']
    assert not call('/admin/api/system',http=member)['capabilities']['instance.backup']['allowed']
    invalid=body();invalid['recoveryKey']='short';call(method='POST',data=invalid,status=400)
    invalid=body();invalid['generation']+=1;call(method='POST',data=invalid,status=409)
    def wait(id,states={'ready'},seconds=45):
        for _ in range(seconds*10):
            row=next(row for row in call()['items'] if row['id']==id)
            if row['state'] in states:return row
            assert row['state'] not in {'failed','cancelled'},row
            time.sleep(.1)
        raise AssertionError(row)
    first=body();operation=call(method='POST',data=first)['operation'];id=operation['id']
    assert call(method='POST',data=first)['operation']['id']==id
    changed={**first,'recoveryKey':'82'*32};call(method='POST',data=changed,status=409)
    ready=wait(id)
    assert sql(f"SELECT key_blob IS NULL AND lease_owner IS NULL FROM instance_operations WHERE id='{id}';")=='t'
    assert 'key' not in json.dumps(ready).lower()
    call(endpoint+'/'+id+'/download',status=403,http=member)
    archive=owner.open(runtime()[1]+endpoint+'/'+id+'/download').read()
    assert hashlib.sha256(archive).hexdigest()==ready['sha256'] and len(archive)==ready['bytes']
    assert b'owner@example.test' not in archive and key.encode() not in archive
    app,_=runtime();inspect=json.loads(subprocess.check_output(['docker','inspect',app],text=True))[0]
    network=next(iter(inspect['NetworkSettings']['Networks']))
    cli=pathlib.Path(os.environ['SENTORI_TEST_CLI']).resolve(strict=True)
    subprocess.run(['docker','exec','-i',app,'sh','-c','cat > /data/http-backup.sentori'],input=archive,check=True)
    subprocess.run(['docker','exec','-i',app,'sh','-c','cat > /data/http-recovery.key'],input=key.encode(),check=True)
    manifest=json.loads(subprocess.check_output(['docker','run','--rm','--volumes-from',app,'-v',str(cli)+':/cli:ro','--entrypoint','/cli','ubuntu:24.04','verify','--from','/data/http-backup.sentori','--key-file','/data/http-recovery.key'],text=True))
    assert manifest['header']['archive_id']==ready['manifest']['archiveId']
    assert manifest['tables']['instance_operations']['rows']==1
    print('PASS owner-only backup command, immutable idempotency, encrypted HTTP download and independent CLI verification',flush=True)
    # Keep capture waiting on a real table; terminate only this fixture's server
    # while it holds a lease, then verify a later attempt safely replaces it.
    db_url=next(e.split('=',1)[1] for e in inspect['Config']['Env'] if e.startswith('DATABASE_URL='))
    db=db_url.split('@',1)[1].split('/',1)[0]
    def barrier():
        guard=subprocess.Popen(['docker','exec','-i',db,'psql','-v','ON_ERROR_STOP=1','-U','sentori','-d','fresh','-qtA'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True)
        guard.stdin.write("SET idle_in_transaction_session_timeout='25s'; BEGIN; LOCK TABLE backend_checks IN ACCESS EXCLUSIVE MODE; SELECT 'held';\n");guard.stdin.flush()
        assert guard.stdout.readline().strip()=='held'
        return guard
    def release(guard):
        if guard.poll() is None:guard.stdin.write('COMMIT;\n');guard.stdin.flush();guard.stdin.close();guard.wait(timeout=10)
    guard=barrier()
    try:
        cancel_id=call(method='POST',data=body())['operation']['id'];wait(cancel_id,{'running'})
        call(method='POST',data=body(),status=409)
        call(endpoint+'/'+cancel_id+'/cancel','POST',{},http=member,status=403)
        call(endpoint+'/'+cancel_id+'/cancel','POST',{})
    finally:release(guard)
    cancelled=wait(cancel_id,{'cancelled'});assert cancelled['cancelRequested']
    assert sql(f"SELECT key_blob IS NULL FROM instance_operations WHERE id='{cancel_id}';")=='t'
    call(endpoint+'/'+cancel_id+'/download',status=404)
    print('PASS active backup cancellation clears keys and never publishes a download',flush=True)
    guard=barrier()
    try:
        recovery=call(method='POST',data=body())['operation']['id'];wait(recovery,{'running'})
        # Kill this disposable container only. No process name searches.
        subprocess.run(['docker','kill',app],check=True,stdout=subprocess.DEVNULL)
    finally:release(guard)
    sql(f"UPDATE instance_operations SET lease_until=now()-interval '1 minute' WHERE id='{recovery}';")
    restart();owner=login()
    resumed=wait(recovery)
    assert resumed['attempt']==2,resumed
    assert sql(f"SELECT count(*) FROM audit_logs WHERE action='instance.backup.ready' AND target_id='{recovery}';")=='1'
    assert len(call()['items'])==3
    print('PASS server crash preserves durable commands and a new fenced attempt completes exactly once',flush=True)

    def upload(command, content=archive, recovery_key=key, http=None, status=200):
        boundary='sentori-verify-boundary-'+uuid.uuid4().hex
        fields={**command,'recoveryKey':recovery_key}
        payload=b''.join((f'--{boundary}\r\nContent-Disposition: form-data; name="{name}"\r\n\r\n{value}\r\n').encode() for name,value in fields.items())
        payload+=(f'--{boundary}\r\nContent-Disposition: form-data; name="archive"; filename="../../escape.sentori"\r\nContent-Type: application/octet-stream\r\n\r\n').encode()+content+f'\r\n--{boundary}--\r\n'.encode()
        try: response=(http or owner).open(urllib.request.Request(runtime()[1]+endpoint+'/verify',data=payload,headers={'Content-Type':'multipart/form-data; boundary='+boundary}))
        except urllib.error.HTTPError as error:response=error
        result=json.load(response);assert response.code==status,result
        return result
    upload_body={name:value for name,value in body().items() if name!='recoveryKey'}
    upload(upload_body,http=member,status=403)
    verification=upload(upload_body)['operation']['id']
    verified=wait(verification)
    assert verified['kind']=='verify' and verified['sha256']==ready['sha256']
    assert upload(upload_body)['operation']['id']==verification
    upload(upload_body,archive+b'changed',status=409)
    invalid_body={**upload_body,'commandId':str(uuid.uuid4())}
    wrong=upload(invalid_body,recovery_key='82'*32)['operation']['id'];wait(wrong,{'failed'})
    assert sql(f"SELECT key_blob IS NULL FROM instance_operations WHERE id='{wrong}';")=='t'
    call(endpoint+'/'+wrong+'/download',status=404)
    invalid_body['commandId']=str(uuid.uuid4())
    truncated=upload(invalid_body,archive[:-1])['operation']['id'];wait(truncated,{'failed'})
    call(endpoint+'/'+truncated+'/download',status=404)
    assert subprocess.run(['docker','exec',runtime()[0],'test','-e','/escape.sentori'],capture_output=True).returncode!=0
    print('PASS uploaded archive verification, filename isolation, idempotent content binding and failed-key/truncation rejection',flush=True)

    project=call('/admin/api/projects','POST',{'name':'Migration package fixture'},201)['id']
    subprocess.run(['docker','run','--rm','--network',network,'--volumes-from',runtime()[0],'-v',str(cli)+':/cli:ro','-e','DATABASE_URL='+db_url,'--entrypoint','/cli','ubuntu:24.04','--quiet','package-projects','--out','/data/package.sentori','--blob-dir','/data/blobs','--key-file','/data/http-recovery.key','--project',project],check=True)
    package=subprocess.check_output(['docker','exec',runtime()[0],'cat','/data/package.sentori'])
    invalid_body['commandId']=str(uuid.uuid4())
    separate_mode=upload(invalid_body,package)['operation']['id']
    assert wait(separate_mode,{'failed'})['errorCode']=='verification_failed'
    call(endpoint+'/'+separate_mode+'/download',status=404)
    print('PASS valid project packages cannot be presented as full instance backups',flush=True)

    subprocess.run(['docker','exec','-i',runtime()[0],'sh','-c','cat > /data/instance-master.key'],input=b'71'*32,check=True)
    restart('fresh',None,True);owner=login()
    assert call('/admin/api/system')['capabilities']['instance.backup']['allowed']
    from_file=call(method='POST',data=body())['operation']['id'];wait(from_file)
    print('PASS instance master key supplied exclusively through a mounted secret file',flush=True)
    # A managed snapshot includes its own active command and earlier history.
    # Restoring it must erase temporary keys, cancel pending work and mark files
    # that were not recursively archived as unavailable.
    archive_with_history=owner.open(runtime()[1]+endpoint+'/'+recovery+'/download').read()
    subprocess.run(['docker','exec','-i',runtime()[0],'sh','-c','cat > /data/with-history.sentori'],input=archive_with_history,check=True)
    sql('CREATE DATABASE restored_operations;', 'postgres')
    target_url=db_url.rsplit('/',1)[0]+'/restored_operations'
    subprocess.run(['docker','exec','-i',runtime()[0],'sh','-c','cat > /data/target-database.url'],input=target_url.encode(),check=True)
    def operator(*args):
        return json.loads(subprocess.check_output(['docker','run','--rm','--network',network,'--volumes-from',runtime()[0],'-v',str(cli)+':/cli:ro','-e','SENTORI_DATABASE_URL_FILE=/data/target-database.url','-e','SENTORI_MASTER_KEY_FILE=/data/instance-master.key','--entrypoint','/cli','ubuntu:24.04',*args],text=True) or '{}')
    operator('migrate')
    report=operator('restore','--from','/data/with-history.sentori','--key-file','/data/http-recovery.key','--blob-dir','/data/restored-operations')
    assert report['instance_id']!=instance['id']
    assert sql("SELECT count(*) FROM instance_operations WHERE key_blob IS NOT NULL OR state IN ('queued','running','ready');",'restored_operations')=='0'
    assert sql(f"SELECT state FROM instance_operations WHERE id='{id}';",'restored_operations')=='unavailable'
    assert sql(f"SELECT state FROM instance_operations WHERE id='{recovery}';",'restored_operations')=='cancelled'
    print('PASS CLI mounted database/master secrets and restored lifecycle commands are terminal without retained keys',flush=True)
