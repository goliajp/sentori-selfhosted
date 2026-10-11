"""Encrypted consistent recovery and strict preflight using the actual operator CLI."""
import base64
import concurrent.futures
import hashlib
import json
import os
import pathlib
import subprocess
import time
import urllib.request
import uuid

NEEDS_RUNTIME = True


def run(*, base, owner, sql, password, client, request, restart, login, runtime):
    app, _ = runtime()
    cli = pathlib.Path(os.environ['SENTORI_TEST_CLI']).resolve(strict=True)
    inspect = json.loads(subprocess.check_output(['docker','inspect',app], text=True))[0]
    network = next(iter(inspect['NetworkSettings']['Networks']))
    db_url = next(e.split('=',1)[1] for e in inspect['Config']['Env'] if e.startswith('DATABASE_URL='))
    def docker(*args, **kw): return subprocess.check_output(['docker',*args], **kw)
    def invoke(*args, database='fresh', ok=True, key='71'*32, stdin=None):
        command=['docker','run','--rm','-i','--network',network,'--volumes-from',app,
                 '-v',str(cli)+':/cli:ro','-e','DATABASE_URL='+db_url.rsplit('/',1)[0]+'/'+database,
                 '-e','SENTORI_MASTER_KEY='+key,'--entrypoint','/cli','ubuntu:24.04',*args]
        result=subprocess.run(command,input=stdin,text=True,capture_output=True)
        assert (result.returncode==0)==ok,(args,result.returncode,result.stderr)
        return result.stdout if ok else result.stderr
    def call(path, method='GET', data=None, status=200, http=None):
        return request(http or owner,runtime()[1],path,method,data,status)
    def write(path, content):
        docker('exec',app,'sh','-c','printf %s '+base64.b64encode(content).decode()+' | base64 -d > '+path)
    project=call('/admin/api/projects','POST',{'name':'Recovery fixture'},201)['id']
    foreign=call('/admin/api/projects','POST',{'name':'Private other project'},201)['id']
    token=call(f'/admin/api/projects/{project}/tokens','POST',dict(name='Recovery SDK',scope='ingest'),201)['token']
    sdk=client();sdk.addheaders=[('Authorization','Bearer '+token)]
    event={'kind':'error','occurredAt':'2026-10-10T00:00:00Z','platform':'android','release':'recovery@1','environment':'test',
           'payload':{'app':{'packageName':'jp.example.recovery'},'error':{'type':'TypeError','message':'Recovery source','stack':[{'file':'index.android.bundle','line':1,'column':20,'function':'e','inApp':True}]}}}
    event_id=call('/v1/events','POST',event,202,sdk)['eventId']
    issue=call('/admin/api/events/'+event_id)['issueId']
    replay=b'{"kind":"keyframe","at":0,"width":320,"height":640,"nodes":[]}\n'
    reference=str(uuid.uuid4());hash=hashlib.sha256(replay).hexdigest()
    docker('exec',app,'mkdir','-p','/data/blobs/'+hash[:2]);write('/data/blobs/'+hash[:2]+'/'+hash[2:]+'.bin',replay)
    sql(f"INSERT INTO event_attachments(ref,project_id,event_id,kind,media_type,size_bytes,blob_hash,source,captured_at) VALUES('{reference}','{project}','{event_id}','replay','application/x-ndjson',{len(replay)},'{hash}','javascript',now());")
    release=sql(f"SELECT id FROM releases WHERE project_id='{project}' AND name='recovery@1';")
    app_row=call(f'/admin/api/projects/{project}/apps')['apps'][0]
    call(f'/admin/api/projects/{project}/apps/{app_row["id"]}','PATCH',dict(expectedVersion=1,name='Recovery Android',repositoryUrl='https://github.com/example/recovery'))
    sourcemap=json.dumps({'version':3,'file':'index.android.bundle','sources':['src/checkout.ts'],'sourcesContent':["export function charge() {\n  const token = 'test'\n  return post('/pay', { token })\n}\n"],'names':[],'mappings':'oBAEI'})
    boundary='recovery-fixture-boundary'
    content=(f'--{boundary}\r\nContent-Disposition: form-data; name="kind"\r\n\r\nsourcemap\r\n--{boundary}\r\nContent-Disposition: form-data; name="file"; filename="index.android.bundle.map"\r\nContent-Type: application/json\r\n\r\n{sourcemap}\r\n--{boundary}--\r\n').encode()
    uploaded=json.load(owner.open(urllib.request.Request(runtime()[1]+f'/admin/api/projects/{project}/releases/{release}/artifacts',data=content,headers={'Content-Type':'multipart/form-data; boundary='+boundary})))
    assert uploaded['usable']
    for _ in range(100):
        if sql(f"SELECT state FROM jobs WHERE id='{uploaded['jobId']}';")=='succeeded':break
        time.sleep(.1)
    else:raise AssertionError('symbolication fixture did not finish')
    secret=subprocess.check_output(['openssl','genpkey','-algorithm','EC','-pkeyopt','ec_paramgen_curve:P-256'],text=True)
    call(f'/admin/api/projects/{project}/push/commands','POST',dict(commandId=str(uuid.uuid4()),action=dict(type='createCredential',provider='webpush',config=dict(subject='mailto:backup@example.test',vapidPublicKey='fixture-public-key'),secret=secret,label='Encrypted recovery fixture')))
    device=str(uuid.uuid4());queued=str(uuid.uuid4());leased=str(uuid.uuid4())
    sql(f"INSERT INTO device_tokens(id,project_id,provider,native_token) VALUES('{device}','{project}','webpush','https://hooks.sentori.example/recovery'); INSERT INTO push_sends(id,project_id,token_id,provider,payload,next_attempt_at) VALUES('{queued}','{project}','{device}','webpush','{{}}',now()+interval '1 day'); INSERT INTO push_sends(id,project_id,token_id,provider,payload,next_attempt_at,lease_owner,lease_until) VALUES('{leased}','{project}','{device}','webpush','{{}}',now()+interval '1 day',gen_random_uuid(),now()+interval '1 day');")
    invoke('keygen','--out','/data/recovery.key')
    sql(f"UPDATE issues SET assignee_user_id=(SELECT id FROM users WHERE role='superadmin' LIMIT 1) WHERE id='{issue}';")
    package=json.loads(invoke('package-projects','--out','/data/project.sentori','--blob-dir','/data/blobs','--key-file','/data/recovery.key','--project',project,key=''))
    assert package['header']['mode']=='project_migration' and package['header']['project_ids']==[project]
    assert package['header']['master_key_fingerprint'] is None
    assert package['tables']['projects']['rows']==1 and package['tables']['users']['rows']==1
    assert {c['name'] for c in package['tables']['users']['columns']}=={'id','display_name'}
    assert {c['name'] for c in package['tables']['projects']['columns']}=={'id','name','platform','created_at'}
    assert not ({'tokens','auth_sessions','push_credentials','device_tokens','instance_operations','task_integrations','outbox_events'} & package['tables'].keys())
    assert package['tables']['apps']['rows']==1 and package['tables']['app_releases']['rows']==1
    assert package['tables']['events']['rows']==1 and package['blobs'][hash]==len(replay) and len(package['blobs'])==2
    assert json.loads(invoke('verify','--from','/data/project.sentori','--key-file','/data/recovery.key',key=''))==package
    invoke('package-projects','--out','/data/missing-project.sentori','--blob-dir','/data/blobs','--key-file','/data/recovery.key','--project',str(uuid.uuid4()),ok=False)
    assert subprocess.run(['docker','exec',app,'test','-e','/data/missing-project.sentori']).returncode!=0
    print('PASS encrypted scoped project package excludes credentials and preserves replay/source files without an instance master key',flush=True)
    invoke('dump','--out','/data/wrong-master.sentori','--blob-dir','/data/blobs','--key-file','/data/recovery.key',key='72'*32,ok=False)
    invoke('dump','--out','/data/no-master.sentori','--blob-dir','/data/blobs','--key-file','/data/recovery.key',key='',ok=False)
    blob_path='/data/blobs/'+hash[:2]+'/'+hash[2:]+'.bin'
    docker('exec',app,'mv',blob_path,blob_path+'.held')
    try: invoke('dump','--out','/data/missing-blob.sentori','--blob-dir','/data/blobs','--key-file','/data/recovery.key',ok=False)
    finally: docker('exec',app,'mv',blob_path+'.held',blob_path)
    assert subprocess.run(['docker','exec',app,'test','-e','/data/missing-blob.sentori']).returncode!=0
    # Hold one late table so capture's live consistent snapshot and GC lock can
    # be observed while a different transaction commits a real new event.
    db_container=db_url.split('@',1)[1].split('/',1)[0]
    guard=subprocess.Popen(['docker','exec','-i',db_container,'psql','-v','ON_ERROR_STOP=1','-U','sentori','-d','fresh','-qtA'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True)
    try:
        guard.stdin.write("SET idle_in_transaction_session_timeout='30s'; BEGIN; LOCK TABLE backend_checks IN ACCESS EXCLUSIVE MODE; SELECT 'held';\n");guard.stdin.flush()
        assert guard.stdout.readline().strip()=='held'
        with concurrent.futures.ThreadPoolExecutor(1) as pool:
            pending=pool.submit(invoke,'dump','--out','/data/recovery.sentori','--blob-dir','/data/blobs','--key-file','/data/recovery.key')
            try:
                for _ in range(100):
                    if pending.done(): pending.result(); raise AssertionError('capture completed before snapshot barrier')
                    if sql("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname='fresh' AND (query LIKE '%row_to_json(t)%backend_checks%' OR query LIKE '%pg_catalog.pg_attribute%') AND wait_event_type='Lock');")=='t':break
                    time.sleep(.1)
                else:raise AssertionError('capture did not hold a snapshot')
                assert sql('SELECT pg_try_advisory_lock(6000288202337570626);')=='f'
                added=call('/v1/events','POST',event,202,sdk)['eventId']
            finally:
                if guard.poll() is None:
                    guard.stdin.write('COMMIT;\n');guard.stdin.flush();guard.stdin.close();guard.wait(timeout=10)
            manifest=json.loads(pending.result(timeout=60))
        assert manifest['tables']['events']['rows']==int(sql('SELECT count(*) FROM events;'))-1
        assert sql('SELECT pg_try_advisory_lock(6000288202337570626);')=='t'
    finally:
        if guard.poll() is None:
            guard.stdin.write('ROLLBACK;\n');guard.stdin.flush();guard.stdin.close();guard.wait(timeout=10)
    assert len(manifest['blobs'])==2
    assert manifest['tables']['push_credentials']['rows']==1
    assert json.loads(invoke('verify','--from','/data/recovery.sentori','--key-file','/data/recovery.key'))==manifest
    assert 'PRIVATE KEY' not in docker('exec',app,'cat','/data/recovery.sentori').decode(errors='replace')
    print('PASS encrypted consistent DB and replay/source-map archive with independent verification',flush=True)
    # Unsafe table names cannot reach SQL and import rolls back earlier valid rows.
    invoke('export','--tables','users; DROP TABLE projects;--',ok=False)
    fake=str(uuid.uuid4());row=json.loads(sql(f"SELECT row_to_json(t) FROM projects t WHERE id='{project}';"));row.update(id=fake,name='must rollback');payload=json.dumps({'table':'projects','row':row})+'\n'+json.dumps({'table':'users; DROP TABLE projects;--','row':{}})+'\n'
    invoke('import',stdin=payload,ok=False)
    assert sql(f"SELECT count(*) FROM projects WHERE id='{fake}';")=='0'
    exported=[json.loads(line) for line in invoke('export','--project',project).splitlines()]
    assert all(row['table'] not in ['users','auth_sessions','password_resets','instance_settings'] for row in exported)
    assert [row['row']['id'] for row in exported if row['table']=='projects']==[project]
    print('PASS CLI table allowlists, transactional import and child-table project scope',flush=True)
    sql('CREATE DATABASE recovery;', 'postgres');invoke('migrate',database='recovery')
    invoke('restore','--from','/data/project.sentori','--key-file','/data/recovery.key','--blob-dir','/data/rejected',database='recovery',ok=False)
    assert sql('SELECT count(*) FROM projects;', 'recovery')=='0'
    before=sql('SELECT row_to_json(t) FROM instance_settings t;', 'recovery')
    write('/data/wrong.key',b'00'*32)
    invoke('restore','--from','/data/recovery.sentori','--key-file','/data/wrong.key','--blob-dir','/data/rejected',database='recovery',ok=False)
    invoke('restore','--from','/data/recovery.sentori','--key-file','/data/recovery.key','--blob-dir','/data/rejected',database='recovery',key='72'*32,ok=False)
    docker('exec',app,'sh','-c','head -c 100 /data/recovery.sentori > /data/truncated.sentori')
    invoke('restore','--from','/data/truncated.sentori','--key-file','/data/recovery.key','--blob-dir','/data/rejected',database='recovery',ok=False)
    assert sql('SELECT row_to_json(t) FROM instance_settings t;', 'recovery')==before
    assert subprocess.run(['docker','exec',app,'test','-e','/data/rejected']).returncode!=0
    sql('ALTER TABLE projects ALTER COLUMN name TYPE varchar(200);','recovery')
    invoke('restore','--from','/data/recovery.sentori','--key-file','/data/recovery.key','--blob-dir','/data/rejected',database='recovery',ok=False)
    sql('ALTER TABLE projects ALTER COLUMN name TYPE text;','recovery')
    assert sql('SELECT row_to_json(t) FROM instance_settings t;', 'recovery')==before
    # An insert error must roll back all previous tables and this operation's blobs.
    sql("CREATE FUNCTION reject_restore() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'restore fault'; END $$; CREATE TRIGGER reject_restore BEFORE INSERT ON events FOR EACH ROW EXECUTE FUNCTION reject_restore();",'recovery')
    invoke('restore','--from','/data/recovery.sentori','--key-file','/data/recovery.key','--blob-dir','/data/rejected',database='recovery',ok=False)
    assert sql('SELECT count(*) FROM users;', 'recovery')=='0'
    assert sql('SELECT row_to_json(t) FROM instance_settings t;', 'recovery')==before
    sql('DROP TRIGGER reject_restore ON events; DROP FUNCTION reject_restore();','recovery')
    restored=json.loads(invoke('restore','--from','/data/recovery.sentori','--key-file','/data/recovery.key','--blob-dir','/data/restored',database='recovery'))
    assert restored['instance_id']!=manifest['header']['instance_id'] and restored['source_instance_id']==manifest['header']['instance_id']
    for table,summary in manifest['tables'].items():assert int(sql(f'SELECT count(*) FROM {table};','recovery'))==summary['rows'],table
    invoke('restore','--from','/data/recovery.sentori','--key-file','/data/recovery.key','--blob-dir','/data/second',database='recovery',ok=False)
    print('PASS wrong key, truncation, incompatible instance key, transactional fault rollback and occupied-target refusal',flush=True)
    assert sql(f"SELECT provider_outcome FROM push_sends WHERE id='{queued}';",'recovery')=='cancelled'
    assert sql(f"SELECT provider_outcome FROM push_sends WHERE id='{leased}';",'recovery')=='uncertain'
    restart('recovery','fs:/data/restored');owner=login()
    assert call('/admin/api/instance')['operatingMode']=='restore_validation'
    failure=invoke('restore','--from','/data/recovery.sentori','--key-file','/data/recovery.key','--blob-dir','/data/running-target',database='recovery',ok=False)
    assert 'target server is running' in failure,failure
    assert owner.open(runtime()[1]+'/admin/api/attachments/'+reference).read()==replay
    fresh=call('/v1/events','POST',event,202,sdk)['eventId']
    frame=call('/admin/api/events/'+fresh)['payload']['error']['stack'][0]
    assert frame['file']=='src/checkout.ts' and "return post('/pay'" in frame['contextLine'],frame
    assert call('/admin/api/issues/'+issue)['id']==issue
    recovered_app=call(f'/admin/api/projects/{project}/apps')['apps'][0]
    assert sql(f"SELECT has_explicit_identifier FROM apps WHERE id='{recovered_app['id']}';",'recovery')=='t'
    assert recovered_app['id']==app_row['id'] and recovered_app['name']=='Recovery Android'
    assert recovered_app['repositoryUrl']=='https://github.com/example/recovery' and recovered_app['latestRelease']['id']==release
    print('PASS restored target login, byte-identical replay, retained SDK credential and actual source-map symbolication',flush=True)
    job=call(f'/admin/api/projects/{project}/releases/{release}/resymbolicate','POST',{'idempotencyKey':str(uuid.uuid4())},202)['jobId']
    time.sleep(2)
    assert sql(f"SELECT state FROM jobs WHERE id='{job}';",'recovery')=='queued'
    validation_event=json.loads(json.dumps(event))
    validation_event['payload']['error']['type']='ValidationOnlyError'
    validation_event['payload']['error']['message']='Recovery evidence must not notify'
    validation_id=call('/v1/events','POST',validation_event,202,sdk)['eventId']
    notification_id=sql(f"SELECT id FROM notification_events WHERE source_key LIKE '%:{validation_id}';",'recovery')
    assert notification_id and sql(f"SELECT routed_at IS NULL FROM notification_events WHERE id='{notification_id}';",'recovery')=='t'
    assert sql(f"SELECT count(*) FROM notification_deliveries WHERE event_id='{notification_id}';",'recovery')=='0'
    resume=['resume-recovery','--instance-id',restored['instance_id'],'--generation',str(restored['generation']),'--archive-id',manifest['header']['archive_id']]
    invoke(*resume,'--confirm-old-instance-stopped',database='recovery',ok=False)
    docker('stop',app)
    invoke(*resume,database='recovery',ok=False)
    invoke(*resume,'--confirm-old-instance-stopped',database='recovery')
    assert sql(f"SELECT routed_at IS NOT NULL FROM notification_events WHERE id='{notification_id}';",'recovery')=='t'
    restart('recovery','fs:/data/restored');owner=login()
    assert call('/admin/api/instance')['operatingMode']=='normal'
    for _ in range(100):
        if sql(f"SELECT state FROM jobs WHERE id='{job}';",'recovery')=='succeeded':break
        time.sleep(.1)
    else:raise AssertionError('explicit resume did not restart background jobs')
    assert sql(f"SELECT count(*) FROM notification_deliveries WHERE event_id='{notification_id}';",'recovery')=='0'
    print('PASS restored instance pauses automatic workers, cancels queued external writes, preserves uncertainty and requires explicit offline resume',flush=True)
    restart()
