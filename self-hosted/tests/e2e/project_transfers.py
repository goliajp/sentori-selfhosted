"""Two real runtimes: bound encrypted copy, final reconciliation and SDK cutover."""
import concurrent.futures
import datetime
import hashlib
import gzip
import json
import os
import pathlib
import sys
import tempfile
import subprocess
import time
import urllib.error
import urllib.request
import uuid
NEEDS_RUNTIME = True


def run(*, base, owner, sql, password, client, request, restart, login, runtime):
    def docker(*args): return subprocess.check_output(['docker', *args], text=True).strip()
    app, _ = runtime()
    info = json.loads(docker('inspect', app))[0]
    network = next(iter(info['NetworkSettings']['Networks']))
    database_url = next(value.split('=', 1)[1] for value in info['Config']['Env'] if value.startswith('DATABASE_URL='))
    db = database_url.split('@', 1)[1].split('/', 1)[0]
    binary = next(value['Source'] for value in info['Mounts'] if value['Destination'] == '/sentori')
    target_name = 'sentori-transfer-' + uuid.uuid4().hex[:12]
    volume = target_name + '-data'
    tls_root = pathlib.Path(os.environ['SENTORI_TRANSFER_FIXTURE']) if os.environ.get('SENTORI_TRANSFER_FIXTURE') else None
    tail_network = target_name+'-tail'
    proxy_name = target_name+'-proxy'
    endpoint = '/admin/api/transfers'
    key = '91' * 32
    def source(path=endpoint, method='GET', data=None, status=200, http=None):
        return request(http or owner, runtime()[1], path, method, data, status)
    def wait(fn, seconds=60):
        until = time.monotonic() + seconds
        while time.monotonic() < until:
            value = fn()
            if value: return value
            time.sleep(.1)
        raise AssertionError('transfer timed out: ' + docker('logs', runtime()[0])[-3500:] + docker('logs', target_name)[-3500:])
    def sdk(origin, credential, path, data, status, headers=None):
        req = urllib.request.Request(origin + path, data=data if isinstance(data,bytes) else json.dumps(data).encode(), headers={'Content-Type': 'application/json', 'Authorization': 'Bearer ' + credential, **(headers or {})})
        try: response = urllib.request.urlopen(req, timeout=30)
        except urllib.error.HTTPError as error: response = error
        value = json.load(response)
        assert response.code == status, (response.code, status, value)
        return value
    def event(message):
        return dict(id=str(uuid.uuid4()), kind='error', occurredAt=datetime.datetime.now(datetime.timezone.utc).isoformat(), platform='javascript', release='transfer@1', payload={'app': {'identifier': 'jp.example.transferred'}, 'error': {'type': 'Error', 'message': message}})
    def row(call, identity): return next(item for item in call()['items'] if item['id'] == identity)
    def command(call, identity, action, status=200, **extra):
        transfer = row(call, identity)
        body = dict(commandId=str(uuid.uuid4()), instanceId=transfer['instanceId'], generation=transfer['generation'], version=transfer['version'], action=action, **extra)
        value = call(endpoint+'/'+identity+'/commands', 'POST', body, status)
        if status == 200:
            assert call(endpoint+'/'+identity+'/commands', 'POST', body) == value
            call(endpoint+'/'+identity+'/commands', 'POST', {**body, 'sdkUpdated': not body.get('sdkUpdated', False)}, 409)
        return value
    def done(call, identity, job, state='ready'):
        def check():
            value = next(j for j in row(call, identity)['jobs'] if j['id'] == job)
            if value['state'] == state: return value
            assert value['state'] in {'queued', 'running'}, value
        return wait(check)
    sql('CREATE DATABASE transfer_target;', 'postgres')
    docker('volume', 'create', volume)
    try:
        docker('run', '-d', '--name', target_name, '--network', network, '-p', '127.0.0.1::8080',
               '-v', binary+':/sentori:ro', '-v', volume+':/data', '-e', 'DATABASE_URL='+database_url.rsplit('/',1)[0]+'/transfer_target',
               '-e', 'SENTORI_OWNER_EMAIL=owner@example.test', '-e', 'SENTORI_OWNER_PASSWORD='+password,
               '-e', 'SENTORI_MASTER_KEY='+'72'*32, '-e', 'SENTORI_ATTACHMENT_STORE=fs:/data/blobs', '--entrypoint', '/sentori', 'ubuntu:24.04')
        target_base = 'http://127.0.0.1:'+docker('port', target_name, '8080/tcp').split(':')[-1]
        def healthy():
            try: return request(client(), target_base, '/healthz')
            except (OSError, AssertionError): return None
        wait(healthy)
        target_http = client()
        request(target_http, target_base, '/auth/login', 'POST', {'email':'owner@example.test','password':password})
        def target(path=endpoint, method='GET', data=None, status=200): return request(target_http, target_base, path, method, data, status)
        member = login('member@example.test')
        source(status=401, http=client()); source(status=403, http=member)
        a, b = source('/admin/api/instance'), target('/admin/api/instance')
        project = source('/admin/api/projects', 'POST', {'name':'迁移 / Migration'}, 201)['id']
        other = target('/admin/api/projects', 'POST', {'name':'Untouched target'}, 201)['id']
        token = source('/admin/api/projects/'+project+'/tokens', 'POST', {'name':'Old SDK','scope':'ingest'}, 201)['token']
        original = event('Initial migration event')
        sdk(runtime()[1], token, '/v1/events', original, 202)
        source_app=source('/admin/api/projects/'+project+'/apps')['apps'][0]
        source('/admin/api/projects/'+project+'/apps/'+source_app['id'],'PATCH',dict(expectedVersion=1,name='Transferred app',repositoryUrl='https://github.com/example/transferred'))
        sql("UPDATE events SET received_at=now()+interval '1 day' WHERE id='"+original['id']+"';")
        session=str(uuid.uuid4())
        sdk(runtime()[1],token,'/v1/events:batch',{'sessions':[{'id':session,'status':'ok','startedAt':datetime.datetime.now(datetime.timezone.utc).isoformat(),'platform':'javascript','release':'transfer@1','durationMs':123}]},200)
        transfer = str(uuid.uuid4())
        definition = dict(commandId=str(uuid.uuid4()), id=transfer, direction='source', instanceId=a['id'], generation=a['generation'], sourceInstanceId=a['id'], sourceGeneration=a['generation'], targetInstanceId=b['id'], targetGeneration=b['generation'], projectIds=[project])
        source(method='POST', data=definition, status=403, http=member)
        created = source(method='POST', data=definition)
        assert source(method='POST', data=definition) == created
        source(method='POST', data={**definition,'targetGeneration':b['generation']+1}, status=409)
        source(method='POST', data={**definition,'commandId':str(uuid.uuid4()),'id':str(uuid.uuid4())}, status=409)
        target_definition = {**definition, 'commandId':str(uuid.uuid4()), 'direction':'target', 'instanceId':b['id'], 'generation':b['generation']}
        target(method='POST', data=target_definition)
        def barrier(statement):
            guard = subprocess.Popen(['docker','exec','-i',db,'psql','-v','ON_ERROR_STOP=1','-U','sentori','-d','fresh','-qtA'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True)
            guard.stdin.write("SET idle_in_transaction_session_timeout='90s'; BEGIN; "+statement+"; SELECT 'held';\n");guard.stdin.flush()
            assert guard.stdout.readline().strip()=='held'
            return guard
        def release(guard):
            if guard.poll() is None:
                guard.stdin.write('COMMIT;\n');guard.stdin.flush();guard.stdin.close();guard.wait(timeout=10)
        worker_lock=int('53454e5454524e53',16)
        guard=barrier(f"DO $$ BEGIN PERFORM pg_advisory_xact_lock({worker_lock}); END $$")
        try:
            cancelled=command(source,transfer,'export_copy',recoveryKey=key)['jobId']
            source(endpoint+'/'+transfer+'/jobs/'+cancelled+'/cancel','POST',{})
            done(source,transfer,cancelled,'cancelled')
            assert sql("SELECT key_blob IS NULL FROM project_transfer_jobs WHERE id='"+cancelled+"';")=='t'
        finally:release(guard)
        guard=barrier('LOCK TABLE events IN ACCESS EXCLUSIVE MODE')
        try:
            copy_job = command(source, transfer, 'export_copy', recoveryKey=key)['jobId']
            done(source,transfer,copy_job,'running')
            docker('kill',runtime()[0])
        finally:release(guard)
        sql("UPDATE project_transfer_jobs SET lease_until=now()-interval '1 minute' WHERE id='"+copy_job+"';")
        restart();owner=login()

        exported = done(source, transfer, copy_job)
        assert exported['attempt']==2,exported
        print('PASS queued-job cancellation erases key, worker crash resumes under a new fenced lease',flush=True)
        archive = owner.open(runtime()[1]+endpoint+'/'+transfer+'/jobs/'+copy_job+'/download').read()
        assert hashlib.sha256(archive).hexdigest() == exported['sha256']
        assert token.encode() not in archive and b'owner@example.test' not in archive
        def upload(content, phase='copy', recovery_key=key, body=None, status=200):
            current = row(target, transfer)
            body = body or dict(commandId=str(uuid.uuid4()),instanceId=b['id'],generation=b['generation'],recoveryKey=recovery_key)
            boundary = 'sentori'+uuid.uuid4().hex
            data = b''.join((f'--{boundary}\r\nContent-Disposition: form-data; name="{name}"\r\n\r\n{value}\r\n').encode() for name,value in body.items())
            data += (f'--{boundary}\r\nContent-Disposition: form-data; name="archive"; filename="../../escape.sentori"\r\nContent-Type: application/octet-stream\r\n\r\n').encode()+content+f'\r\n--{boundary}--\r\n'.encode()
            req = urllib.request.Request(target_base+endpoint+'/'+transfer+'/upload?phase='+phase+'&version='+str(current['version']),data=data,headers={'Content-Type':'multipart/form-data; boundary='+boundary})
            try: response=target_http.open(req,timeout=30)
            except urllib.error.HTTPError as error: response=error
            value=json.load(response); assert response.code==status,(response.code,status,value)
            return value
        bad=upload(archive,recovery_key='92'*32)['jobId'];done(target,transfer,bad,'failed')
        assert sql("SELECT count(*) FROM projects WHERE id<>'"+other+"';",'transfer_target')=='0'
        first=upload(archive)['jobId'];done(target,transfer,first)
        mapped=row(target,transfer)['projectMapping'][project]
        target_app=target('/admin/api/projects/'+mapped+'/apps')['apps'][0]
        assert target_app['id']!=source_app['id'] and target_app['name']=='Transferred app' and target_app['repositoryUrl']=='https://github.com/example/transferred'
        assert sql("SELECT has_explicit_identifier FROM apps WHERE id='"+target_app['id']+"';",'transfer_target')=='t'
        assert target_app['releaseCount']==1 and target_app['latestRelease']['name']=='transfer@1'
        assert sql("SELECT app_id FROM events WHERE id='"+original['id']+"';",'transfer_target')==target_app['id']
        assert mapped!=project and sql("SELECT count(*) FROM events WHERE project_id='"+mapped+"';",'transfer_target')=='1'
        assert sql("SELECT count(*) FROM tokens WHERE project_id='"+mapped+"';",'transfer_target')=='0'
        assert sql("SELECT key_blob IS NULL FROM project_transfer_jobs WHERE id='"+first+"';",'transfer_target')=='t'
        new_token=target('/admin/api/projects/'+mapped+'/tokens','POST',{'name':'Renewed SDK','scope':'ingest'},201)['token']
        sdk(target_base,new_token,'/v1/events',event('Staged target'),503)
        command(target,transfer,'activate',status=409,rollbackPlan='Back up target and reconcile all new data before any source unfreeze.')
        assert sql("SELECT count(*) FROM sessions WHERE id='"+session+"';",'transfer_target')=='1'
        sql("DELETE FROM sessions WHERE id='"+session+"'; UPDATE projects SET name='最终迁移项目' WHERE id='"+project+"';")
        print('PASS both owner authorizations, bound encrypted copy, wrong-key atomic failure, isolated mapping and staged ingest fence',flush=True)

        # A batch's second issue INSERT pauses before its project guard. Seal
        # between events, then prove the middleware returns a retryable 503.
        first_race,second_race=event('migration-race-first'),event('migration-race-second')
        lock_id=int(uuid.uuid4().hex[:14],16)
        guard=subprocess.Popen(['docker','exec','-i',db,'psql','-v','ON_ERROR_STOP=1','-U','sentori','-d','fresh','-qtA'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True)
        guard.stdin.write(f"SET idle_in_transaction_session_timeout='90s'; BEGIN; SELECT pg_advisory_xact_lock({lock_id}); SELECT pg_backend_pid();\n");guard.stdin.flush()
        guard.stdout.readline();guard_pid=int(guard.stdout.readline().strip())
        sql(f"CREATE FUNCTION pause_transfer_test() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.message_sample LIKE '%migration-race-second%' THEN PERFORM pg_advisory_xact_lock({lock_id}); END IF; RETURN NEW; END $$; CREATE TRIGGER aa_pause_transfer BEFORE INSERT ON issues FOR EACH ROW EXECUTE FUNCTION pause_transfer_test();")
        with concurrent.futures.ThreadPoolExecutor(1) as executor:
            future=executor.submit(sdk,runtime()[1],token,'/v1/events:batch',{'events':[first_race,second_race]},503)
            try:
                wait(lambda:sql(f"SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE {guard_pid}=ANY(pg_blocking_pids(pid)));")=='t')
                plan='Keep source fenced. Back up the target, stop ingestion, reconcile new target rows and attachments, then obtain operator review before restoring routing.'
                command(source,transfer,'seal',rollbackPlan=plan)
            finally:
                guard.stdin.write('COMMIT;\n');guard.stdin.flush();guard.stdin.close();guard.wait(timeout=10)
            future.result(timeout=30)
        sql('DROP TRIGGER aa_pause_transfer ON issues; DROP FUNCTION pause_transfer_test();')
        command(source,transfer,'cancel',status=409)
        sdk(runtime()[1],token,'/v1/events:batch',{'events':[event('Fenced source')]},503)
        final_job=command(source,transfer,'export_final',recoveryKey=key)['jobId'];done(source,transfer,final_job)
        final_archive=owner.open(runtime()[1]+endpoint+'/'+transfer+'/jobs/'+final_job+'/download').read()
        # An initial archive cannot satisfy a final upload or activate a target.
        mismatch=upload(archive,'final')['jobId'];done(target,transfer,mismatch,'failed')
        final_import=upload(final_archive,'final')['jobId'];done(target,transfer,final_import)
        assert row(target,transfer)['state']=='ready'
        assert sql("SELECT count(*) FROM sessions WHERE id='"+session+"';",'transfer_target')=='0'
        assert sql("SELECT name FROM projects WHERE id='"+mapped+"';",'transfer_target')=='最终迁移项目'
        assert sql("SELECT count(*) FROM events WHERE project_id='"+mapped+"';",'transfer_target')=='2'
        assert sql("SELECT name FROM projects WHERE id='"+other+"';",'transfer_target')=='Untouched target'
        sdk(target_base,new_token,'/v1/events',event('Revoked staging token'),401)
        new_token=target('/admin/api/projects/'+mapped+'/tokens','POST',{'name':'Final renewed SDK','scope':'ingest'},201)['token']
        command(target,transfer,'activate',rollbackPlan=plan)
        command(target,transfer,'complete',status=409,sdkUpdated=True)
        command(target,transfer,'verify',status=409,events={project:original['id']})
        transient=event('Unverified event removed by retention')
        sdk(target_base,new_token,'/v1/events',transient,202)
        assert row(target,transfer)['observedEvents'][project]==transient['id']
        sql("DELETE FROM events WHERE project_id='"+mapped+"' AND id='"+transient['id']+"';",'transfer_target')
        command(target,transfer,'verify',status=409,events={project:transient['id']})
        sdk(target_base,new_token,'/v1/events:batch',{'events':[first_race,second_race]},200)
        assert row(target,transfer)['observedEvents'][project]==second_race['id']
        assert sql("SELECT count(*) FROM events WHERE project_id='"+mapped+"';",'transfer_target')=='3'
        command(target,transfer,'verify',events={project:second_race['id']})
        command(target,transfer,'complete',status=409,sdkUpdated=False)
        command(target,transfer,'complete',sdkUpdated=True)
        command(target,transfer,'cancel',status=409)
        assert row(target,transfer)['state']=='completed'
        proof=sdk(target_base,new_token,'/v1/transfer-proof',{'transferId':transfer},200)
        assert proof['archiveId']==row(source,transfer)['manifest']['archiveId'] and proof['eventVerified']
        for origin in ['http://example.com','https://127.0.0.1','https://example.com/path']:
            current=row(source,transfer)
            source(endpoint+'/'+transfer+'/tail','POST',dict(commandId=str(uuid.uuid4()),instanceId=a['id'],generation=a['generation'],version=current['version'],projectId=project,targetOrigin=origin,targetToken=new_token,expiresInDays=7),400)
        assert sql("SELECT migration_fence IS NOT NULL FROM projects WHERE id='"+project+"';")=='t'
        assert key not in json.dumps(source()) and new_token not in json.dumps(target())
        if tls_root:
            subnet=f'11.{128+int(uuid.uuid4().hex[:2],16)%100}.{int(uuid.uuid4().hex[:2],16)}.0/24'
            docker('network','create','--internal','--subnet',subnet,tail_network)
            docker('network','connect',tail_network,runtime()[0]);docker('network','connect',tail_network,target_name)
            (tls_root/'proxy.py').write_text(PROXY)
            (tls_root/'state.json').write_text('{}')
            docker('run','-d','--name',proxy_name,'--network',tail_network,'--network-alias','transfer.sentori.example','-e','TARGET='+target_name,'-v',str(tls_root)+':/fixture','python:3.13-alpine','python','/fixture/proxy.py')
            wait(lambda:subprocess.run(['docker','exec',proxy_name,'python','-c',"import socket;socket.create_connection(('127.0.0.1',443)).close()"],capture_output=True).returncode==0)
            def configure(credential=new_token,status=200):
                current=row(source,transfer)
                body=dict(commandId=str(uuid.uuid4()),instanceId=a['id'],generation=a['generation'],version=current['version'],projectId=project,targetOrigin='https://transfer.sentori.example',targetToken=credential,expiresInDays=7)
                value=source(endpoint+'/'+transfer+'/tail','POST',body,status)
                if status==200:assert source(endpoint+'/'+transfer+'/tail','POST',body)==value
                return value
            unrelated_token=target('/admin/api/projects/'+other+'/tokens','POST',{'name':'Unrelated SDK','scope':'ingest'},201)['token']
            configure(unrelated_token,409);configure()
            forwarded=event('Old native client tail')
            sdk(runtime()[1],token,'/v1/events',forwarded,202)
            sdk(runtime()[1],token,'/v1/events',forwarded,202)
            assert sql("SELECT count(*) FROM events WHERE id='"+forwarded['id']+"';",'transfer_target')=='1'
            assert sql("SELECT count(*) FROM events WHERE id='"+forwarded['id']+"';")=='0'
            assert row(source,transfer)['routes'][0]['forwarded']==2
            compressed=event('Compressed old-client batch')
            sdk(runtime()[1],token,'/v1/events:batch',gzip.compress(json.dumps({'events':[compressed]}).encode()),200,{'Content-Encoding':'gzip'})
            assert sql("SELECT count(*) FROM events WHERE id='"+compressed['id']+"';",'transfer_target')=='1'
            boundary='sentori-tail-'+uuid.uuid4().hex
            replay=b'{"type":"frame","tree":[]}\n'
            attachment=(f'--{boundary}\r\nContent-Disposition: form-data; name="source"\r\n\r\njs\r\n--{boundary}\r\nContent-Disposition: form-data; name="file"; filename="replay.ndjson"\r\nContent-Type: application/x-ndjson\r\n\r\n').encode()+replay+f'\r\n--{boundary}--\r\n'.encode()
            evidence=sdk(runtime()[1],token,'/v1/events/'+forwarded['id']+'/attachments/replay',attachment,202,{'Content-Type':'multipart/form-data; boundary='+boundary})
            assert target('/admin/api/attachments/'+evidence['refId'])==replay.decode()

            wire=json.dumps(source());assert token not in wire and new_token not in wire
            assert new_token.encode().hex() not in sql("SELECT encode(sealed_token,'hex') FROM project_transfer_routes;")
            (tls_root/'state.json').write_text('{"code":302}')
            sdk(runtime()[1],token,'/v1/events',event('Do not follow redirects'),503)
            (tls_root/'state.json').write_text('{}')
            assert row(source,transfer)['routes'][0]['failed']==1
            sql("UPDATE project_transfer_routes SET expires_at=now()-interval '1 second';")
            sdk(runtime()[1],token,'/v1/events',event('Expired tail'),503)
            print('PASS real TLS target proof, wrong-project token refusal, encrypted renewed token, old-client forwarding/dedup, redirect refusal and finite expiry',flush=True)

        print('PASS mid-batch seal returns 503, final package binding, checksum reconciliation, activation and real target-event evidence before completion',flush=True)
    finally:
        subprocess.run(['docker','rm','-f',proxy_name],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        subprocess.run(['docker','network','disconnect','-f',tail_network,runtime()[0]],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        subprocess.run(['docker','network','disconnect','-f',tail_network,target_name],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        subprocess.run(['docker','network','rm',tail_network],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        subprocess.run(['docker','rm','-f',target_name],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        subprocess.run(['docker','volume','rm',volume],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)


PROXY = r'''
import http.client,http.server,json,os,pathlib,ssl
root=pathlib.Path('/fixture')
class Handler(http.server.BaseHTTPRequestHandler):
 def do_POST(self):
  body=self.rfile.read(int(self.headers.get('Content-Length','0')))
  state=json.loads((root/'state.json').read_text())
  if state.get('code'):
   self.send_response(state['code']);self.send_header('Location','http://127.0.0.1/private');self.end_headers();return
  upstream=http.client.HTTPConnection(os.environ['TARGET'],8080,timeout=20)
  upstream.request('POST',self.path,body,headers={name:value for name,value in self.headers.items() if name.lower() not in ['host','connection']})
  response=upstream.getresponse();data=response.read();self.send_response(response.status)
  self.send_header('Content-Type','application/json');self.send_header('Content-Length',str(len(data)));self.end_headers();self.wfile.write(data);upstream.close()
 def log_message(self,*args):pass
server=http.server.ThreadingHTTPServer(('0.0.0.0',443),Handler)
tls=ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER);tls.load_cert_chain(root/'server.pem',root/'server.key');server.socket=tls.wrap_socket(server.socket,server_side=True);server.serve_forever()
'''

def main():
    with tempfile.TemporaryDirectory(prefix='sentori-transfer-tls-') as directory:
        root=pathlib.Path(directory)
        def openssl(*args):subprocess.run(['openssl',*args],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        openssl('req','-x509','-newkey','rsa:2048','-nodes','-keyout',str(root/'ca.key'),'-out',str(root/'ca.pem'),'-days','1','-subj','/CN=Sentori disposable transfer CA','-addext','basicConstraints=critical,CA:TRUE')
        openssl('req','-newkey','rsa:2048','-nodes','-keyout',str(root/'server.key'),'-out',str(root/'server.csr'),'-subj','/CN=transfer.sentori.example')
        (root/'server.ext').write_text('basicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\nsubjectAltName=DNS:transfer.sentori.example\n')
        openssl('x509','-req','-in',str(root/'server.csr'),'-CA',str(root/'ca.pem'),'-CAkey',str(root/'ca.key'),'-CAcreateserial','-out',str(root/'server.pem'),'-days','1','-extfile',str(root/'server.ext'))
        host=next(p for p in [pathlib.Path('/etc/ssl/certs/ca-certificates.crt'),pathlib.Path('/etc/ssl/cert.pem')] if p.is_file())
        (root/'roots.pem').write_bytes(host.read_bytes()+b'\n'+(root/'ca.pem').read_bytes())
        result=subprocess.run([sys.executable,str(pathlib.Path(__file__).with_name('instance.py')),*sys.argv[1:],'--ca-bundle',str(root/'roots.pem'),'--domain-test',__file__],env={**os.environ,'SENTORI_TRANSFER_FIXTURE':str(root)})
        raise SystemExit(result.returncode)

if __name__=='__main__':main()
