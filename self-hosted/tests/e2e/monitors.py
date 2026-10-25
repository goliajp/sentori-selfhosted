"""Real scheduled probes against an isolated Docker HTTP fixture, never the public Internet."""
import concurrent.futures
import json
import pathlib
import subprocess
import tempfile
import time
import uuid

NEEDS_RUNTIME = True

FIXTURE = r'''
import http.server,json,pathlib,threading,time
root=pathlib.Path('/fixture')
lock=threading.Lock(); active=0
class Handler(http.server.BaseHTTPRequestHandler):
 def do_GET(self):
  global active
  with lock:
   active+=1
   with (root/'requests.jsonl').open('a') as f:f.write(json.dumps({'path':self.path,'active':active})+'\n')
  try:
   state=json.loads((root/'state.json').read_text())
   time.sleep(state.get('delay',0))
   code=state.get('code',200);self.send_response(code)
   if code in [301,302,307,308]:self.send_header('Location',state['location'])
   self.end_headers();self.wfile.write(b'x'*state.get('size',2))
  except (BrokenPipeError,ConnectionResetError):pass
  finally:
   with lock:active-=1
 def log_message(self,*args):pass
http.server.ThreadingHTTPServer(('0.0.0.0',80),Handler).serve_forever()
'''

def run(*, base, owner, sql, password, client, request, restart, login, runtime):
    def docker(*args):
        return subprocess.check_output(['docker', *args], text=True).strip()
    def call(path, method='GET', data=None, status=200, http=None):
        return request(http or owner, base, path, method, data, status)
    def wait(test, seconds=20):
        until=time.monotonic()+seconds
        while time.monotonic()<until:
            result=test()
            if result:return result
            time.sleep(.1)
        raise AssertionError('monitor condition timed out')
    project=call('/admin/api/projects','POST',{'name':'Availability'},201)['id']
    foreign=call('/admin/api/projects','POST',{'name':'Other availability'},201)['id']
    member=login('member@example.test')
    member_id=sql("SELECT id FROM users WHERE email='member@example.test';")
    call(f'/admin/api/users/{member_id}/projects/{project}','PUT',{})
    path=f'/admin/api/projects/{project}/monitors'
    call(f'/admin/api/projects/{foreign}/monitors',status=403,http=member)
    assert call('/admin/api/system')['capabilities']['monitors']['supported']
    app,_=runtime(); network='sentori-monitors-'+uuid.uuid4().hex[:12]; fixture=network+'-http'
    config=dict(name='Availability',url='http://health.sentori.example/health',intervalSeconds=60,timeoutSeconds=3,expectedStatus=200,failureThreshold=2,enabled=True)
    def command(id, version=0, configuration=config, action='save', **extra):
        status=extra.pop('status',200);http=extra.pop('http',None)
        body=dict(commandId=str(uuid.uuid4()),id=id,expectedVersion=version,configuration=configuration,action=action,**extra)
        return call(path+'/commands','POST',body,status,http)
    def monitor(id):return next(m for m in call(path)['monitors'] if m['id']==id)
    def samples(id):return int(sql(f"SELECT count(*) FROM monitor_samples WHERE monitor_id='{id}';"))
    def check(id):
        before=samples(id);m=monitor(id)
        command(id,m['version'],None,'check')
        wait(lambda:samples(id)>before)
        return monitor(id)
    with tempfile.TemporaryDirectory(prefix=network) as directory:
        root=pathlib.Path(directory)
        def state(**values):
            temp=root/'state.tmp';temp.write_text(json.dumps(values));temp.replace(root/'state.json')
        (root/'server.py').write_text(FIXTURE);state(code=200)
        # A globally classified IPv4 range exists ONLY inside this disposable internal
        # Docker network. Production uses the exact public-egress path, with no bypass.
        subnet=f'11.{128+int(uuid.uuid4().hex[:2],16)%100}.{int(uuid.uuid4().hex[:2],16)}.0/24'
        docker('network','create','--internal','--subnet',subnet,network)
        try:
            docker('network','connect',network,app)
            docker('run','-d','--name',fixture,'--network',network,'--network-alias','health.sentori.example',
                   '-v',str(root)+':/fixture','python:3.13-alpine','python','/fixture/server.py')
            wait(lambda:subprocess.run(['docker','exec',fixture,'python','-c',"import urllib.request; urllib.request.urlopen('http://127.0.0.1/health').read()"],capture_output=True).returncode==0)
            # Configuration, authorization and command replay are independent of scheduling.
            id=str(uuid.uuid4());payload=dict(commandId=str(uuid.uuid4()),id=id,expectedVersion=0,configuration=dict(config,enabled=False),action='save')
            with concurrent.futures.ThreadPoolExecutor(6) as pool:
                results=list(pool.map(lambda _:call(path+'/commands','POST',payload),range(12)))
            assert all(value==results[0] for value in results)
            assert sql(f"SELECT count(*) FROM monitor_commands WHERE project_id='{project}';")=='1'
            call(path+'/commands','POST',dict(payload,configuration=dict(config,name='collision')),409)
            command(id,1,dict(config,name='denied'),http=member,status=403)
            command(id,0,config,status=409)
            for url in ['http://127.0.0.1','http://2130706433','http://[::ffff:127.0.0.1]','http://169.254.169.254','http://example.com:22','https://u:p@example.com']:
                command(str(uuid.uuid4()),configuration=dict(config,url=url),status=400)
            call(f'/admin/api/projects/{foreign}/monitors/{id}/history',status=404)
            print('PASS monitor authority, validation, concurrent command replay and version conflicts',flush=True)

            command(id,1,config)
            wait(lambda:samples(id)==1)
            assert monitor(id)['status']=='operational',monitor(id)
            state(code=503)
            assert check(id)['status']=='degraded'
            assert not call(path+'/'+id+'/history')['events']
            assert check(id)['status']=='down'
            check(id)
            events=call(path+'/'+id+'/history')['events']
            assert len(events)==1 and events[0]['kind']=='monitor.down'
            notes=call('/admin/api/inbox',http=member)['items']
            assert len(notes)==1 and notes[0]['monitorId']==id and notes[0]['taskId'] is None
            state(code=200)
            assert check(id)['status']=='operational'
            events=call(path+'/'+id+'/history')['events']
            assert len(events)==2 and events[0]['kind']=='monitor.recovered' and events[0]['incidentId']==events[1]['incidentId']
            assert len(call('/admin/api/inbox',http=member)['items'])==2
            print('PASS threshold, one incident, repeated-failure dedup and one recovery notification',flush=True)

            # Every redirect repeats URL/DNS checks; no private destination is contacted.
            state(code=302,location='http://169.254.169.254/latest/meta-data')
            assert check(id)['samples'][-1]['error']=='non_public_address'
            state(code=302,location='http://health.sentori.example/again')
            assert check(id)['samples'][-1]['error']=='too_many_redirects'
            state(code=200,size=65537)
            assert check(id)['samples'][-1]['error']=='response_too_large'
            state(code=200,delay=5)
            assert check(id)['samples'][-1]['error']=='timeout'
            print('PASS redirect revalidation, redirect cap, bounded body and whole-probe timeout',flush=True)

            # Editing while a probe is in flight fences the previous version's result.
            state(code=503,delay=2)
            command(id,2,None,'check')
            wait(lambda:sql(f"SELECT lease_owner IS NOT NULL FROM monitors WHERE id='{id}';")=='t')
            before=samples(id)
            command(id,2,dict(config,enabled=False))
            time.sleep(3)
            assert samples(id)==before and monitor(id)['status']=='unknown'
            assert monitor(id)['samples']==[] and monitor(id)['availability'] is None
            command(id,3,None,'check',status=409)
            print('PASS configuration cancellation and truthful new measurement period',flush=True)

            # Force the lease-recovery path then restart the actual owned server container.
            state(code=200)
            sql(f"UPDATE monitors SET enabled=true,lease_owner=gen_random_uuid(),lease_until=now()-interval '1 second',next_check_at=now() WHERE id='{id}';")
            docker('kill','--signal','KILL',app);docker('start',app)
            base='http://127.0.0.1:'+docker('port',app,'8080/tcp').split(':')[-1]
            def healthy():
                try:return request(client(),base,'/healthz')
                except OSError:return False
            wait(healthy)
            wait(lambda:samples(id)>before)
            assert monitor(id)['status']=='operational',monitor(id)
            assert sql(f"SELECT lease_owner IS NULL FROM monitors WHERE id='{id}';")=='t'

            # A generation mismatch cannot be scheduled, and every send observes preferences.
            count=samples(id)
            sql(f"UPDATE monitors SET generation=generation-1,next_check_at=now() WHERE id='{id}';")
            time.sleep(2);assert samples(id)==count
            sql(f"UPDATE monitors SET generation=(SELECT generation FROM instance_settings) WHERE id='{id}';")
            call('/admin/api/inbox-preferences','PUT',dict(assignments=True,mentions=True,taskUpdates=True,monitors=False),http=member)
            before_notes=call('/admin/api/inbox',http=member)['total'];state(code=503)
            check(id);check(id);check(id)
            assert call('/admin/api/inbox',http=member)['total']==before_notes
            print('PASS crash lease recovery, generation fencing and monitor preferences',flush=True)

            # An atomic inbox failure rolls back both sample and incident state.
            state(code=200);check(id)
            sql("CREATE FUNCTION reject_monitor_inbox() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.category='monitor' THEN RAISE EXCEPTION 'fixture inbox failure'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_monitor_inbox BEFORE INSERT ON inbox_items FOR EACH ROW EXECUTE FUNCTION reject_monitor_inbox();")
            state(code=503);check(id);before=samples(id);before_events=len(call(path+'/'+id+'/history')['events'])
            command(id,3,None,'check');time.sleep(2)
            assert samples(id)==before and len(call(path+'/'+id+'/history')['events'])==before_events
            sql('DROP TRIGGER reject_monitor_inbox ON inbox_items; DROP FUNCTION reject_monitor_inbox();')
            sql(f"UPDATE monitors SET lease_until=now()-interval '1 second' WHERE id='{id}';")
            wait(lambda:samples(id)>before)
            assert monitor(id)['status']=='down'
            command(id,3,None,'delete')
            assert not call(path)['monitors'] and call(path+'/'+id+'/history')['events']
            print('PASS atomic incident/inbox rollback and audited deletion with retained history',flush=True)

            # DNS is re-resolved for a later check; a private answer is rejected before HTTP.
            other_network=next(n for n in json.loads(docker('inspect','--format','{{json .NetworkSettings.Networks}}',app)) if n!=network)
            docker('network','connect','--alias','health.sentori.example',other_network,fixture)
            docker('network','disconnect',network,fixture)
            before=len((root/'requests.jsonl').read_text().splitlines())
            dns_id=str(uuid.uuid4());command(dns_id)
            wait(lambda:samples(dns_id)>0)
            assert monitor(dns_id)['samples'][-1]['error']=='non_public_address'
            assert len((root/'requests.jsonl').read_text().splitlines())==before
            command(dns_id,1,None,'delete')
            docker('network','disconnect',other_network,fixture)
            docker('network','connect','--alias','health.sentori.example',network,fixture)

            state(code=200,delay=1)
            (root/'requests.jsonl').write_text('')
            identifiers=[str(uuid.uuid4()) for _ in range(10)]
            for item in identifiers:command(item)
            wait(lambda:all(samples(item)==1 for item in identifiers))
            active=[json.loads(line)['active'] for line in (root/'requests.jsonl').read_text().splitlines()]
            assert max(active)<=4 and max(active)>1,active
            for item in identifiers:command(item,1,None,'delete')
            print('PASS DNS re-resolution blocks private answers and scheduler bounds concurrency to four',flush=True)

        finally:
            subprocess.run(['docker','rm','-f','-v',fixture],capture_output=True)
            subprocess.run(['docker','network','disconnect','-f',network,app],capture_output=True)
            subprocess.run(['docker','network','rm',network],capture_output=True)
