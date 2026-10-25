#!/usr/bin/env python3
"""Real outbound HTTP/SMTP fixture. Run this file with --binary (it prepares isolated TLS roots)."""
import concurrent.futures
import hashlib
import hmac
import json
import os
import pathlib
import subprocess
import sys
import tempfile
import time
import uuid

NEEDS_RUNTIME=True
FIXTURE=r'''
import http.server,json,pathlib,ssl,time
root=pathlib.Path('/fixture')
class Handler(http.server.BaseHTTPRequestHandler):
 def do_POST(self):
  body=self.rfile.read(int(self.headers.get('Content-Length','0')))
  with (root/'requests.jsonl').open('a') as f:f.write(json.dumps({'path':self.path,'headers':dict(self.headers),'body':body.decode()})+'\n')
  state=json.loads((root/'state.json').read_text());time.sleep(state.get('delay',0))
  code=state.get('code',201 if self.path.startswith('/repos/') or self.path=='/rest/api/3/issue' else 200)
  self.send_response(code)
  if code in [301,302,307,308]:self.send_header('Location','http://127.0.0.1/secret')
  response=json.dumps({'number':42,'key':'ENG-42','html_url':'javascript:alert(1)'}).encode()
  self.send_header('Content-Type','application/json');self.send_header('Content-Length',str(len(response)));self.end_headers()
  try:self.wfile.write(response)
  except (BrokenPipeError,ConnectionResetError):pass
 def do_GET(self):self.send_response(json.loads((root/'state.json').read_text()).get('getCode',200));self.send_header('Content-Length','0');self.end_headers()
 def log_message(self,*args):pass
server=http.server.ThreadingHTTPServer(('0.0.0.0',443),Handler)
tls=ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER);tls.load_cert_chain(root/'server.pem',root/'server.key');server.socket=tls.wrap_socket(server.socket,server_side=True)
server.serve_forever()
'''

def run(*,base,owner,sql,password,client,request,restart,login,runtime):
    root=pathlib.Path(os.environ['SENTORI_INTEGRATION_FIXTURE'])
    smtp=os.environ.get('SENTORI_INTEGRATION_NO_SMTP')!='1'
    def docker(*args):return subprocess.check_output(['docker',*args],text=True).strip()
    def call(path,method='GET',data=None,status=200,http=None):return request(http or owner,base,path,method,data,status)
    def wait(test,seconds=25):
        until=time.monotonic()+seconds
        while time.monotonic()<until:
            value=test()
            if value:return value
            time.sleep(.1)
        raise AssertionError('integration condition timed out')
    def state(**value):
        p=root/'state.tmp';p.write_text(json.dumps(value));p.replace(root/'state.json')
    def requests():return [json.loads(line) for line in (root/'requests.jsonl').read_text().splitlines()] if (root/'requests.jsonl').exists() else []
    state();(root/'server.py').write_text(FIXTURE)
    app,_=runtime();network='sentori-outbound-'+uuid.uuid4().hex[:12];fixture=network+'-https'
    project=call('/admin/api/projects','POST',{'name':'Integrations'},201)['id'];path=f'/admin/api/projects/{project}'
    member=login('member@example.test');member_id=sql("SELECT id FROM users WHERE email='member@example.test';")
    call(f'/admin/api/users/{member_id}/projects/{project}','PUT',{})
    subnet=f'11.{128+int(uuid.uuid4().hex[:2],16)%100}.{int(uuid.uuid4().hex[:2],16)}.0/24'
    docker('network','create','--internal','--subnet',subnet,network)
    try:
        docker('network','connect',network,app)
        docker('run','-d','--name',fixture,'--network',network,'--network-alias','hooks.sentori.example','--network-alias','jira.sentori.example','--network-alias','api.github.com',
            '-v',str(root)+':/fixture','python:3.13-alpine','python','/fixture/server.py')
        wait(lambda:subprocess.run(['docker','exec',fixture,'python','-c',"import socket; socket.create_connection(('127.0.0.1',443)).close()"],capture_output=True).returncode==0)
        fixture_ip=json.loads(docker('inspect','--format','{{json .NetworkSettings.Networks}}',fixture))[network]['IPAddress']
        for host in ['api.github.com','jira.sentori.example','hooks.sentori.example']:
            addresses=docker('exec',app,'getent','ahostsv4',host).splitlines()
            assert addresses and all(line.split()[0]==fixture_ip for line in addresses),(host,addresses)
        configs={
            'webhook':dict(endpoint='https://hooks.sentori.example/private-hook-secret',token='signing-secret-123456'),
            'github':dict(repository='team/repo',token='github-test-secret'),
            'jira':dict(endpoint='https://jira.sentori.example/',token='jira-test-secret-123456',email='bot@example.test',projectKey='ENG',issueTypeId='10001'),
            'email':dict(recipient='notifications@example.test'),
        }
        def configure(kind,version=0,settings=None,events=None,enabled=True,status=200,http=None,key=None):
            return call(path+'/integrations/'+kind,'PUT',dict(commandId=key or str(uuid.uuid4()),expectedVersion=version,settings=configs[kind] if settings is None else settings,events=events or [],enabled=enabled),status,http)
        ids={}
        for kind in configs:
            key=str(uuid.uuid4());one=configure(kind,key=key);two=configure(kind,key=key);assert one==two;ids[kind]=one['id']
            configure(kind,settings=dict(configs[kind],token='different-secret'),key=key,status=409)
        result=call(path+'/integrations');wire=json.dumps(result)
        assert result['smtpConfigured']==smtp and result['vaultConfigured'] and len(result['integrations'])==4
        assert all(secret not in wire for secret in ['private-hook-secret','signing-secret','github-test-secret','jira-test-secret'])
        assert all(secret.encode().hex() not in sql("SELECT string_agg(encode(secret_blob,'hex'),'') FROM outbound_integrations;") for secret in ['private-hook-secret','github-test-secret'])
        configure('webhook',1,http=member,status=403)
        configure('webhook',1,dict(endpoint='http://127.0.0.1',token='abcdefghijklmnop'),status=400)
        configure('github',0,status=409)
        configure('webhook',1,dict(endpoint='',token=''))
        versions={kind:2 if kind=='webhook' else 1 for kind in configs}
        print('PASS encrypted settings, redacted reads, credential binding and idempotent configuration',flush=True)

        def send(kind,status=200,task=None,task_version=None,key=None,http=None):
            return call(path+'/integrations/'+ids[kind]+'/send','POST',dict(commandId=key or str(uuid.uuid4()),expectedVersion=versions[kind],taskId=task,expectedTaskVersion=task_version),status,http)
        def delivery(id):return next((d for d in call(path+'/deliveries')['deliveries'] if d['id']==id),None)
        def done(id,want='delivered'):
            try:return wait(lambda:(d if (d:=delivery(id)) and d['state']==want else None))
            except AssertionError:raise AssertionError({'wanted':want,'delivery':delivery(id),'logs':docker('logs',app)[-3000:]})
        for kind in configs:
            key=str(uuid.uuid4())
            with concurrent.futures.ThreadPoolExecutor(4) as executor:
                sent=list(executor.map(lambda _:send(kind,key=key),range(8)))
            assert all(row==sent[0] for row in sent)
            row=done(sent[0]['deliveryId'],'failed' if kind=='email' and not smtp else 'delivered');assert row['attempt']==1
            if kind=='email' and not smtp:assert row['error']=='smtp_unconfigured'
            if kind=='github':assert row['externalUrl']=='https://github.com/team/repo/issues/42'
            if kind=='jira':assert row['externalUrl']=='https://jira.sentori.example/browse/ENG-42'
        posts=requests();assert len(posts)==3,posts
        hook=next(p for p in posts if p['path']=='/private-hook-secret');headers={k.lower():v for k,v in hook['headers'].items()}
        expected='sha256='+hmac.new(configs['webhook']['token'].encode(),(headers['x-sentori-timestamp']+'.'+hook['body']).encode(),hashlib.sha256).hexdigest()
        assert hmac.compare_digest(headers['x-sentori-signature'],expected)
        github=next(p for p in posts if p['path'].startswith('/repos/'))
        assert {k.lower():v for k,v in github['headers'].items()}['authorization']=='Bearer github-test-secret'
        jira=next(p for p in posts if p['path']=='/rest/api/3/issue')
        assert json.loads(jira['body'])['fields']['description']['type']=='doc'
        send('github',http=member,status=403)
        print('PASS real TLS POSTs, provider payloads, signed webhooks, '+('SMTP' if smtp else 'explicit smtp_unconfigured failure without blocking other channels')+' and concurrent-send dedup',flush=True)

        # Known HTTP failures retry webhooks with the same delivery ID and bounded attempts.
        state(code=503);id=send('webhook')['deliveryId'];done(id,'retry_wait');state()
        sql(f"UPDATE notification_deliveries SET next_attempt_at=now() WHERE id='{id}';")
        assert done(id)['attempt']==2
        matching=[p for p in requests() if {k.lower():v for k,v in p['headers'].items()}.get('x-sentori-delivery')==id]
        assert len(matching)==2
        state(code=302);id=send('webhook')['deliveryId'];assert done(id,'failed')['error']=='redirect_refused'
        state(code=503);id=send('github')['deliveryId'];assert done(id,'uncertain')['httpStatus']==503
        retry=dict(commandId=str(uuid.uuid4()),expectedAttempt=1,acknowledgeDuplicate=False)
        call(path+'/deliveries/'+id+'/retry','POST',retry,409)
        state();retry['acknowledgeDuplicate']=True
        call(path+'/deliveries/'+id+'/retry','POST',retry);call(path+'/deliveries/'+id+'/retry','POST',retry)
        assert done(id)['attempt']==2
        print('PASS bounded webhook retry, refused write redirects and explicit uncertain-send confirmation',flush=True)

        # Queue with a DB trigger so configuration and authorization can change before dispatch.
        sql("CREATE FUNCTION hold_delivery() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN NEW.next_attempt_at=now()+interval '1 hour'; RETURN NEW; END $$; CREATE TRIGGER hold_delivery BEFORE INSERT ON notification_deliveries FOR EACH ROW EXECUTE FUNCTION hold_delivery();")
        before=len(requests());id=send('webhook')['deliveryId'];configure('webhook',2,dict(endpoint='',token=''),enabled=False);versions['webhook']=3
        sql(f"UPDATE notification_deliveries SET next_attempt_at=now() WHERE id='{id}';")
        assert done(id,'cancelled')['error']=='integration_changed' and len(requests())==before
        id=send('github')['deliveryId'];sql(f"UPDATE notification_events SET generation=generation-1 WHERE id=(SELECT event_id FROM notification_deliveries WHERE id='{id}'); UPDATE notification_deliveries SET next_attempt_at=now() WHERE id='{id}';")
        assert done(id,'cancelled')['error']=='instance_changed'
        sql('DROP TRIGGER hold_delivery ON notification_deliveries; DROP FUNCTION hold_delivery();')
        print('PASS queued configuration revocation and instance generation fences',flush=True)

        # Task export carries the reviewed version; it cannot silently copy a changed task.
        task=call(path+'/tasks/commands','POST',dict(commandId=str(uuid.uuid4()),action='create',issueId=None,fields=dict(title='Export me',description='Task evidence',kind='task',priority='normal',assignee=None,due=None,reproduction='',rootCause='')))
        send('github',task=task['id'],task_version=0,status=409)
        id=send('github',task=task['id'],task_version=task['version'])['deliveryId'];done(id)
        assert any(json.loads(p['body']).get('title')=='Export me' for p in requests())

        # Audit failure rolls back the queued delivery and idempotency key.
        sql("CREATE FUNCTION reject_integration_audit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.action='integration.send_requested' THEN RAISE EXCEPTION 'fixture audit'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_integration_audit BEFORE INSERT ON audit_logs FOR EACH ROW EXECUTE FUNCTION reject_integration_audit();")
        counts=sql("SELECT (SELECT count(*) FROM notification_events)||','||(SELECT count(*) FROM notification_deliveries)||','||(SELECT count(*) FROM integration_commands);")
        send('jira',status=500)
        assert counts==sql("SELECT (SELECT count(*) FROM notification_events)||','||(SELECT count(*) FROM notification_deliveries)||','||(SELECT count(*) FROM integration_commands);")
        sql('DROP TRIGGER reject_integration_audit ON audit_logs; DROP FUNCTION reject_integration_audit();')
        print('PASS reviewed task export and atomic delivery/audit/idempotency rollback',flush=True)

        # Real task, SDK and monitor transactions publish automatic events to the queue.
        configure('webhook',3,dict(endpoint='',token=''),events=['issue.new','issue.regressed','task.updated','monitor.down','monitor.recovered']);versions['webhook']=4
        state()
        task=call(path+'/tasks/commands','POST',dict(commandId=str(uuid.uuid4()),action='create',issueId=None,fields=dict(title='Automatic task',description='',kind='task',priority='normal',assignee=None,due=None,reproduction='',rootCause='')))
        wait(lambda:any(d['event']=='task.updated' and d['state']=='delivered' for d in call(path+'/deliveries')['deliveries']))
        # Legacy values are encrypted on startup/worker reconciliation, including secret URL paths.
        sql(f"UPDATE projects SET webhook_url='https://hooks.sentori.example/legacy-room-secret' WHERE id='{project}';")
        wait(lambda:sql(f"SELECT webhook_url IS NULL AND webhook_secret_blob IS NOT NULL FROM projects WHERE id='{project}';")=='t')
        assert call(path)['webhookUrl']=='https://hooks.sentori.example/legacy-room-secret'
        assert call(path,http=member)['webhookUrl'] is None
        call('/admin/api/notification-prefs','PUT',dict(projectId=project,onNewIssue=False,onRegression=False),http=member)
        token=call(path+'/tokens','POST',dict(name='delivery test',scope='ingest'),201)['token']
        import urllib.request
        event_id=str(uuid.uuid4())
        event=dict(id=event_id,kind='error',platform='javascript',occurredAt='2026-10-10T00:00:00Z',release='delivery@2',environment='production',payload={'error':{'type':'DurableNotification','message':'new fault','stack':[]}})
        def ingest(event):
            req=urllib.request.Request(base+'/v1/events',data=json.dumps(event).encode(),headers={'Content-Type':'application/json','Authorization':'Bearer '+token})
            with urllib.request.urlopen(req) as response:return json.load(response)
        outcome=ingest(event);ingest(event)
        wait(lambda:sql(f"SELECT count(*) FROM notification_deliveries d JOIN notification_events e ON e.id=d.event_id WHERE e.issue_id='{outcome['issueId']}' AND d.state='delivered';")==('3' if smtp else '2'))
        assert sql(f"SELECT count(*) FROM notification_events WHERE issue_id='{outcome['issueId']}';")=='1'
        assert sql(f"SELECT count(*) FROM notification_deliveries d JOIN notification_events e ON e.id=d.event_id WHERE e.issue_id='{outcome['issueId']}' AND d.user_id='{member_id}';")=='0'
        assert any(p['path']=='/legacy-room-secret' for p in requests())
        # Recurrence produces a separate event exactly once.
        sql(f"UPDATE issues SET status='resolved',resolved_at='2026-10-09T00:00:00Z' WHERE id='{outcome['issueId']}';")
        ingest(dict(event,id=str(uuid.uuid4())))
        wait(lambda:sql(f"SELECT count(*) FROM notification_deliveries d JOIN notification_events e ON e.id=d.event_id WHERE e.issue_id='{outcome['issueId']}' AND e.kind='issue.regressed' AND d.state='delivered';")==('3' if smtp else '2'))
        state(getCode=503)
        monitor_id=str(uuid.uuid4())
        monitor=dict(name='Automatic monitor',url='https://hooks.sentori.example/health',intervalSeconds=60,timeoutSeconds=5,expectedStatus=200,failureThreshold=1,enabled=True)
        call(path+'/monitors/commands','POST',dict(action='save',commandId=str(uuid.uuid4()),id=monitor_id,expectedVersion=0,configuration=monitor))
        wait(lambda:any(d['event']=='monitor.down' and d['state']=='delivered' for d in call(path+'/deliveries')['deliveries']))
        state();call(path+'/monitors/commands','POST',dict(action='check',commandId=str(uuid.uuid4()),id=monitor_id,expectedVersion=1,configuration=None))
        wait(lambda:any(d['event']=='monitor.recovered' and d['state']=='delivered' for d in call(path+'/deliveries')['deliveries']))
        call(path+'/monitors/commands','POST',dict(action='delete',commandId=str(uuid.uuid4()),id=monitor_id,expectedVersion=1,configuration=None))
        print('PASS automatic task, real SDK new/regressed, encrypted legacy webhook, recipient opt-out and monitor recovery delivery',flush=True)

        # Kill the actual owned process after the fixture has received the write.
        state(delay=20);id=send('github')['deliveryId']
        wait(lambda:any({k.lower():v for k,v in p['headers'].items()}.get('x-sentori-delivery')==id for p in requests()))
        docker('kill','--signal','KILL',app)
        sql(f"UPDATE notification_deliveries SET lease_until=now()-interval '1 second' WHERE id='{id}';")
        docker('start',app);base='http://127.0.0.1:'+docker('port',app,'8080/tcp').split(':')[-1]
        def healthy():
            try:return request(client(),base,'/healthz')
            except OSError:return False
        wait(healthy);assert done(id,'uncertain')['error']=='worker_interrupted'
        time.sleep(1)
        assert sum({k.lower():v for k,v in p['headers'].items()}.get('x-sentori-delivery')==id for p in requests())==1
        print('PASS crash-after-send remains uncertain without duplicate external creation',flush=True)
    finally:
        subprocess.run(['docker','rm','-f','-v',fixture],capture_output=True)
        subprocess.run(['docker','network','disconnect','-f',network,app],capture_output=True)
        subprocess.run(['docker','network','rm',network],capture_output=True)

def main():
    no_smtp='--no-mailbox' in sys.argv
    if no_smtp:
        sys.argv.remove('--no-mailbox');os.environ['SENTORI_INTEGRATION_NO_SMTP']='1'
    with tempfile.TemporaryDirectory(prefix='sentori-outbound-tls-') as directory:
        root=pathlib.Path(directory)
        def openssl(*args):subprocess.run(['openssl',*args],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        openssl('req','-x509','-newkey','rsa:2048','-nodes','-keyout',str(root/'ca.key'),'-out',str(root/'ca.pem'),'-days','1','-subj','/CN=Sentori disposable fixture CA','-addext','basicConstraints=critical,CA:TRUE')
        openssl('req','-newkey','rsa:2048','-nodes','-keyout',str(root/'server.key'),'-out',str(root/'server.csr'),'-subj','/CN=hooks.sentori.example')
        (root/'server.ext').write_text('basicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\nsubjectAltName=DNS:hooks.sentori.example,DNS:jira.sentori.example,DNS:api.github.com\n')
        openssl('x509','-req','-in',str(root/'server.csr'),'-CA',str(root/'ca.pem'),'-CAkey',str(root/'ca.key'),'-CAcreateserial','-out',str(root/'server.pem'),'-days','1','-extfile',str(root/'server.ext'))
        host=next(p for p in [pathlib.Path('/etc/ssl/certs/ca-certificates.crt'),pathlib.Path('/etc/ssl/cert.pem')] if p.is_file())
        (root/'roots.pem').write_bytes(host.read_bytes()+b'\n'+(root/'ca.pem').read_bytes())
        result=subprocess.run([sys.executable,str(pathlib.Path(__file__).with_name('instance.py')),*sys.argv[1:],'--ca-bundle',str(root/'roots.pem'),*(['--mailbox'] if not no_smtp else []),'--domain-test',__file__],env={**os.environ,'SENTORI_INTEGRATION_FIXTURE':str(root)})
        raise SystemExit(result.returncode)

if __name__=='__main__':main()
