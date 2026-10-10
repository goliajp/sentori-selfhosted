#!/usr/bin/env python3
"""Durable push with an isolated, real TLS provider and disposable database."""
import concurrent.futures
import json
import os
import pathlib
import subprocess
import sys
import time
import uuid
import integrations

NEEDS_RUNTIME = True

def run(*, base, owner, sql, password, client, request, restart, login, runtime):
    root = pathlib.Path(os.environ['SENTORI_INTEGRATION_FIXTURE'])
    def docker(*args): return subprocess.check_output(['docker', *args], text=True).strip()
    def call(path, method='GET', data=None, status=200, http=None): return request(http or owner, base, path, method, data, status)
    def wait(test, seconds=30):
        until = time.monotonic() + seconds
        while time.monotonic() < until:
            value = test()
            if value: return value
            time.sleep(.1)
        raise AssertionError('push condition timed out: ' + docker('logs', app)[-2500:])
    def state(**value):
        p = root/'state.tmp'; p.write_text(json.dumps(value)); p.replace(root/'state.json')
    def posts(): return [json.loads(line) for line in (root/'requests.jsonl').read_text().splitlines()] if (root/'requests.jsonl').exists() else []
    state(); (root/'server.py').write_text(integrations.FIXTURE)
    subprocess.run(['openssl','genpkey','-algorithm','EC','-pkeyopt','ec_paramgen_curve:P-256','-out',str(root/'vapid.pem')], check=True, capture_output=True)
    secret = (root/'vapid.pem').read_text()
    app, _ = runtime(); network = 'sentori-push-' + uuid.uuid4().hex[:12]; fixture = network+'-tls'
    subnet = f'11.{128+int(uuid.uuid4().hex[:2],16)%100}.{int(uuid.uuid4().hex[:2],16)}.0/24'
    project = call('/admin/api/projects','POST',{'name':'Push operations'},201)['id']; path = f'/admin/api/projects/{project}/push'
    other = call('/admin/api/projects','POST',{'name':'Other push'},201)['id']
    def command(action, key=None, status=200, http=None): return call(path+'/commands','POST',dict(commandId=key or str(uuid.uuid4()),action=action),status,http)
    def credentials(): return call(path+'/credentials')['credentials']
    def credential(id): return next(c for c in credentials() if c['id']==id)
    def create(label='Primary', secret=secret): return command(dict(type='createCredential',provider='webpush',config=dict(subject='mailto:fixture@example.test',vapidPublicKey='fixture-public-key'),secret=secret,label=label))['id']
    def activate(id, force=False, status=200, version=None): return command(dict(type='activateCredential',id=id,expectedVersion=version or credential(id)['version'],force=force),status=status)
    def device(name, endpoint=None, traits=None):
        id = str(uuid.uuid4()); endpoint = endpoint or 'https://hooks.sentori.example/'+name
        sql(f"INSERT INTO device_tokens(id,project_id,provider,native_token,traits,metadata) VALUES('{id}','{project}','webpush','{endpoint}','{json.dumps(traits or {})}','{{\"auth\":\"subscription-private\"}}');")
        return id
    def send(id, key=None, status=200): return command(dict(type='testSend',deviceId=id,title='Reviewed notification',body='Provider fixture'),key,status)
    def detail(id): return call(path+'/sends/'+id)
    def done(id, want='sent'): return wait(lambda: (d if (d:=detail(id))['send']['displayState']==want else None))
    docker('network','create','--internal','--subnet',subnet,network)
    try:
        docker('network','connect',network,app)
        docker('run','-d','--name',fixture,'--network',network,'--network-alias','hooks.sentori.example','-v',str(root)+':/fixture','python:3.13-alpine','python','/fixture/server.py')
        wait(lambda: subprocess.run(['docker','exec',fixture,'python','-c',"import socket; socket.create_connection(('127.0.0.1',443)).close()"],capture_output=True).returncode==0)
        docker('exec','-d','-e','SENTORI_BIND=0.0.0.0:8081',app,'sh','-c','echo $$ > /tmp/push-replica.pid; exec /sentori')
        primary=create(); assert not credential(primary)['active'] and credential(primary)['encrypted']
        assert secret.encode().hex() not in sql(f"SELECT encode(secret_blob,'hex') FROM push_credentials WHERE id='{primary}';")
        activate(primary,status=409); activate(primary,True); activate(primary,True,status=409,version=1)
        command(dict(type='createCredential',provider='webpush',config=dict(subject='mailto:fixture@example.test',vapidPublicKey='fixture-public-key'),secret='not a key',label='Malformed'),status=400)
        candidate=create('Candidate'); assert credential(primary)['active'] and not credential(candidate)['active']
        wire=json.dumps(credentials()); assert 'PRIVATE KEY' not in wire and 'not a key' not in wire
        member=login('member@example.test'); command(dict(type='testSend',deviceId=str(uuid.uuid4()),title='x',body=''),status=403,http=member)
        legacy=str(uuid.uuid4()); sql(f"INSERT INTO push_credentials(id,project_id,kind,config,secret_blob,secret_format,active,label) VALUES('{legacy}','{project}','webpush','{{}}',decode('{secret.encode().hex()}','hex'),'legacy',false,'Legacy');")
        wait(lambda:sql(f"SELECT secret_format FROM push_credentials WHERE id='{legacy}';")=='sealed')
        print('PASS encrypted staged credentials, explicit activation, version and project authorization',flush=True)

        target=device('delivered'); key=str(uuid.uuid4())
        with concurrent.futures.ThreadPoolExecutor(4) as pool: results=list(pool.map(lambda _:send(target,key),range(12)))
        assert all(result==results[0] for result in results)
        id=results[0]['sendId']; result=done(id); assert len(posts())==1 and len(result['attempts'])==1
        assert result['send']['deliveredAt'] is None and result['send']['credentialId']==primary
        ingest_token=call(f'/admin/api/projects/{project}/tokens','POST',dict(name='Push device receipt',scope='ingest'),201)['token']
        sdk=client(); sdk.addheaders=[('Authorization','Bearer '+ingest_token)]
        call(path+'/sends',http=sdk,status=401)
        call('/v1/push/deliveries/'+id+'/ack','POST',dict(ackSessionId='fixture-session'),202,sdk)
        receipt=detail(id)['send']['deliveredAt']; assert receipt
        call('/v1/push/deliveries/'+id+'/ack','POST',dict(ackSessionId='duplicate-session'),202,sdk)
        assert detail(id)['send']['deliveredAt']==receipt
        assert posts()[0]['headers'].get('authorization',posts()[0]['headers'].get('Authorization','')).startswith('vapid t=')
        command(dict(type='testSend',deviceId=target,title='Different',body=''),key,status=409)
        call(f'/admin/api/projects/{other}/push/sends/{id}',status=404)
        assert call(path+'/sends?q=notification&provider=webpush')['total']==1
        assert call(path+'/sends?provider=apns')['total']==0
        assert call(path+'/devices?q='+target)['total']==1
        assert 'subscription-private' not in json.dumps(call(path+'/devices'))
        print('PASS real TLS provider send, concurrent dedup, attempt history and accepted versus SDK receipt',flush=True)

        # Insight's unchanged sender: legacy URL/body, real worker and provider,
        # UUID and historical ipt_<uuid> address the same project-scoped row.
        api_token=call(f'/admin/api/projects/{project}/tokens','POST',dict(name='Insight backend',scope='api'),201)['token']
        backend=client(); backend.addheaders=[('Authorization','Bearer '+api_token)]
        old='/v1/push/send'
        payload=dict(to=target,title='Insight notification',body='Camera event',data={'camera_id':'compat-fixture'})
        call(old,'POST',payload,403,sdk)
        call(old,'POST',payload,401,client())
        for handle in [target,'ipt_'+target,'ipt_'+target.replace('-','')]:
            before=len(posts())
            result=call(old,'POST',dict(payload,to=handle),202,backend)
            ticket=result['tickets'][0]
            assert ticket['status']=='queued' and ticket['id']
            assert done(ticket['id'])['send']['payload']=={k:payload[k] for k in ['title','body','data']}
            assert len(posts())==before+1
            receipt=call('/v1/push/deliveries/'+ticket['id'],http=sdk)
            assert receipt['spToken']==target
        for handle in ['ipt_invalid',str(uuid.uuid4())]:
            call(old,'POST',dict(payload,to=handle),400,backend)
        foreign=str(uuid.uuid4())
        sql(f"INSERT INTO device_tokens(id,project_id,provider,native_token) VALUES('{foreign}','{other}','webpush','https://hooks.sentori.example/foreign');")
        call(old,'POST',dict(payload,to=foreign),400,backend)
        call(old,'POST',dict(payload,sendAt='2030-01-01T00:00:00Z'),422,backend)
        registration=call('/v1/push/tokens','POST',dict(kind='webpush',nativeToken='https://hooks.sentori.example/compat-register'),202,sdk)
        assert registration['token_id']==registration['spToken'] and registration['is_new']==registration['isNew']
        assert registration['id']=='ipt_'+registration['spToken'].replace('-','')
        repeated=call('/v1/push/tokens','POST',dict(provider='webpush',nativeToken='https://hooks.sentori.example/compat-register'),200,sdk)
        assert repeated['spToken']==registration['spToken'] and not repeated['isNew']
        assert repeated['createdAt'] and repeated['updatedAt']
        call('/v1/push/tokens/'+registration['id'],'DELETE',status=202,http=sdk)
        call(old,'POST',dict(payload,to=registration['id']),400,backend)
        # Native v1 API stays unchanged, including its response contract.
        modern=call('/v1/push/sends','POST',dict(spTokens=[target],payload={'title':'Modern'}),202,backend)
        assert modern['queued']==1 and not modern['capped'] and 'tickets' not in modern
        wait(lambda:sql(f"SELECT status FROM push_sends WHERE batch_id='{modern['sendId']}';")=='sent')
        print('PASS Insight legacy push: exact wire, TLS delivery, registration, revoked and cross-project refusal',flush=True)


        state(code=410); dead=device('gone'); result=done(send(dead)['sendId'],'failed')
        assert result['send']['provider_outcome']=='permanent_failure' and sql(f"SELECT revoked_at IS NOT NULL FROM device_tokens WHERE id='{dead}';")=='t'
        state(code=503); retry=device('retry'); id=send(retry)['sendId']
        wait(lambda:detail(id)['send']['retry_count']==1)
        assert sql(f"SELECT revoked_at IS NULL FROM device_tokens WHERE id='{retry}';")=='t'
        state(); sql(f"UPDATE push_sends SET next_attempt_at=now() WHERE id='{id}';"); assert len(done(id)['attempts'])==2
        state(code=302); id=send(target)['sendId']; done(id,'failed'); assert len(detail(id)['attempts'])==1
        state(); private=device('private','https://127.0.0.1/private'); before=len(posts()); done(send(private)['sendId'],'failed'); assert len(posts())==before
        print('PASS permanent quarantine, transient retry, redirect refusal and private endpoint protection',flush=True)

        # Hold dispatch while checking the reviewed audience and queue fences.
        sql("CREATE FUNCTION hold_push() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN NEW.next_attempt_at=now()+interval '1 hour'; RETURN NEW; END $$; CREATE TRIGGER hold_push BEFORE INSERT ON push_sends FOR EACH ROW EXECUTE FUNCTION hold_push();")
        a=device('audience-a',traits={'group':'reviewed'}); b=device('audience-b',traits={'group':'other'})
        query={'traits':{'group':'reviewed'}}
        def preview(): return call(path+'/audience/preview','POST',query)
        def audience(proof,key=None,status=200): return call(path+'/audience/send','POST',dict(**query,title='Audience',body='Reviewed',expectedMatched=proof['matched'],previewId=proof['previewId'],idempotencyKey=key or str(uuid.uuid4())),status)
        proof=preview(); assert proof['matched']==1 and 'subscription-private' not in json.dumps(proof)
        sql(f"UPDATE device_tokens SET traits='{{\"group\":\"other\"}}' WHERE id='{a}'; UPDATE device_tokens SET traits='{{\"group\":\"reviewed\"}}' WHERE id='{b}';")
        audience(proof,status=409)
        proof=preview(); key=str(uuid.uuid4()); first=audience(proof,key); assert first['queued']==1
        sql(f"UPDATE device_tokens SET traits='{{}}' WHERE id='{b}';")
        assert audience(proof,key)['alreadySent']
        stale=preview(); sql(f"UPDATE push_audience_previews SET expires_at=now()-interval '1 second' WHERE id='{stale['previewId']}';"); audience(stale,status=409)
        wrong=preview(); call(f'/admin/api/projects/{other}/push/audience/send','POST',dict(**query,title='Wrong project',body='',expectedMatched=wrong['matched'],previewId=wrong['previewId'],idempotencyKey=str(uuid.uuid4())),409)
        print('PASS exact audience IDs, immutable preview, expiry and replay after membership changes',flush=True)

        held=send(target)['sendId']; command(dict(type='revokeDevice',id=target,expectedVersion=int(sql(f"SELECT version FROM device_tokens WHERE id='{target}';"))))
        before=len(posts()); sql(f"UPDATE push_sends SET next_attempt_at=now() WHERE id='{held}';"); done(held,'cancelled'); assert len(posts())==before
        held=send(retry)['sendId']; sql(f"UPDATE push_sends SET generation=generation-1,next_attempt_at=now() WHERE id='{held}';"); done(held,'cancelled')
        held=send(retry)['sendId']; new=create('Replacement'); activate(new,True)
        sql(f"UPDATE push_sends SET next_attempt_at=now() WHERE id='{held}';"); done(held,'failed'); assert len(posts())==before
        sql('DROP TRIGGER hold_push ON push_sends; DROP FUNCTION hold_push();')
        sql("CREATE FUNCTION reject_push_audit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.action='push.test' THEN RAISE EXCEPTION 'fixture audit'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_push_audit BEFORE INSERT ON audit_logs FOR EACH ROW EXECUTE FUNCTION reject_push_audit();")
        counts=sql('SELECT (SELECT count(*) FROM push_sends)||\',\'||(SELECT count(*) FROM push_commands);'); send(retry,status=500)
        assert counts==sql('SELECT (SELECT count(*) FROM push_sends)||\',\'||(SELECT count(*) FROM push_commands);')
        sql('DROP TRIGGER reject_push_audit ON audit_logs; DROP FUNCTION reject_push_audit();')
        print('PASS device revocation, generation and credential fences, atomic queue/audit rollback',flush=True)

        # A failed completion log must not commit a success or create a blind retry.
        sql("CREATE FUNCTION reject_push_log() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.outcome='ok' THEN RAISE EXCEPTION 'fixture completion'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_push_log BEFORE INSERT ON push_delivery_logs FOR EACH ROW EXECUTE FUNCTION reject_push_log();")
        before=len(posts()); blocked=send(retry)['sendId']; wait(lambda:len(posts())>before); time.sleep(1)
        assert detail(blocked)['send']['displayState']=='sending' and not detail(blocked)['attempts']
        sql('DROP TRIGGER reject_push_log ON push_delivery_logs; DROP FUNCTION reject_push_log();')
        sql(f"UPDATE push_sends SET lease_until=now()-interval '1 second' WHERE id='{blocked}';")
        done(blocked,'uncertain'); assert len(posts())==before+1
        print('PASS two workers use exclusive leases; completion and attempt logs commit together',flush=True)

        # A provider may accept the POST before a process dies: never resend automatically.
        state(delay=20); before=len(posts()); id=send(retry)['sendId']; wait(lambda:len(posts())>before)
        docker('kill','--signal','KILL',app)
        sql(f"UPDATE push_sends SET lease_until=now()-interval '1 second' WHERE id='{id}';")
        docker('start',app); base='http://127.0.0.1:'+docker('port',app,'8080/tcp').split(':')[-1]
        def healthy():
            try:return request(client(),base,'/healthz')
            except OSError:return False
        wait(healthy); result=done(id,'uncertain'); time.sleep(1); assert len(posts())==before+1
        action=dict(type='retrySend',id=id,expectedVersion=result['send']['version'],acknowledgeDuplicate=False)
        command(action,status=409); state(); action['acknowledgeDuplicate']=True
        key=str(uuid.uuid4()); command(action,key); command(action,key); result=done(id); assert len(posts())==before+2
        assert [a['attempt'] for a in result['attempts']]==[2,1]
        print('PASS crash after provider receipt remains uncertain; explicit retry deduplicates',flush=True)
    finally:
        subprocess.run(['docker','rm','-f',fixture],capture_output=True)
        subprocess.run(['docker','network','disconnect','-f',network,app],capture_output=True)
        subprocess.run(['docker','network','rm',network],capture_output=True)

if __name__=='__main__':
    integrations.__file__=__file__
    if '--no-mailbox' not in sys.argv: sys.argv.append('--no-mailbox')
    integrations.main()
