"""Push browser fixtures; all outgoing notifications remain held in this disposable DB."""
import json
import subprocess
import uuid

def run(*,base,owner,sql,password,client,request):
    sql("CREATE FUNCTION browser_hold_push() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN NEW.next_attempt_at=now()+interval '1 day'; RETURN NEW; END $$; CREATE TRIGGER browser_hold_push BEFORE INSERT ON push_sends FOR EACH ROW EXECUTE FUNCTION browser_hold_push();")
    secret=subprocess.check_output(['openssl','genpkey','-algorithm','EC','-pkeyopt','ec_paramgen_curve:P-256'],text=True)
    for engine in ['chromium','webkit']:
        project=request(owner,base,'/admin/api/projects','POST',dict(name='Push browser '+engine),201)['id']; path=f'/admin/api/projects/{project}/push'
        def command(action):return request(owner,base,path+'/commands','POST',dict(commandId=str(uuid.uuid4()),action=action))
        for i in range(2):
            credential=command(dict(type='createCredential',provider='webpush',config=dict(subject='mailto:fixture@example.test',vapidPublicKey='fixture-public-key'),secret=secret,label=['Primary browser credential','Next rotation'][i]))
            if i==0:command(dict(type='activateCredential',id=credential['id'],expectedVersion=1,force=True))
        targets=[]
        for i in range(32):
            id=str(uuid.uuid4());targets.append(id)
            provider='webpush' if i%2==0 else 'apns';traits=json.dumps(dict(plan='pro' if i<5 else 'free',locale='ja-JP'));metadata=json.dumps(dict(appVersion='5.0.0',model='Browser fixture — 国际化 / デバイス',auth='hidden-browser-secret'))
            sql(f"INSERT INTO device_tokens(id,project_id,provider,native_token,traits,metadata,user_key) VALUES('{id}','{project}','{provider}','https://push.example.invalid/device-{i:04d}','{traits}','{metadata}',repeat('a',64));")
        for i,state in enumerate(['sent','failed','uncertain','queued','cancelled']):
            result=command(dict(type='testSend',deviceId=targets[0],title=['Accepted notification / 通知 / 通知','Failed notification','Uncertain notification','Queued notification','Cancelled notification'][i],body='A reviewed notification body. 实际投递结果 / 実際の配信結果。'))
            id=result['sendId'];status='failed' if state in ['failed','uncertain','cancelled'] else state
            sql(f"UPDATE push_sends SET status='{status}',provider_outcome='{state}',error={'NULL' if state in ['sent','queued'] else repr('fixture_provider_error')},sent_at={'now()' if state=='sent' else 'NULL'} WHERE id='{id}';")
            if state!='queued':sql(f"INSERT INTO push_delivery_logs(id,send_id,attempt,outcome,provider_status,duration_ms) VALUES(gen_random_uuid(),'{id}',1,'{state}',{201 if state=='sent' else 503},68);")
        sql(f"UPDATE device_tokens SET revoked_at=now(),bad_streak=3 WHERE id='{targets[-1]}';")
    print('PASS browser push fixtures ready; outgoing sends held in disposable database',flush=True)
