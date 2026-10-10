"""Persistent browser fixtures. Network deliveries are held in the disposable database."""
import json
import uuid

def run(*,base,owner,sql,password,client,request):
    sql("CREATE FUNCTION browser_hold_delivery() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN NEW.next_attempt_at=now()+interval '1 day'; RETURN NEW; END $$; CREATE TRIGGER browser_hold_delivery BEFORE INSERT ON notification_deliveries FOR EACH ROW EXECUTE FUNCTION browser_hold_delivery();")
    owner_id=sql("SELECT id FROM users WHERE email='owner@example.test';")
    for engine in ['chromium','webkit']:
        project=request(owner,base,'/admin/api/projects','POST',{'name':'Monitors browser '+engine},201)['id'];path=f'/admin/api/projects/{project}'
        for index,status in enumerate(['operational','down','degraded','unknown']):
            id=str(uuid.uuid4())
            config=dict(name=['Checkout API / 決済サービス / 支付接口','Release readiness','A long international service name to check wrapping in narrow cards','Paused service'][index],url='https://health.example.com/ready',intervalSeconds=300,timeoutSeconds=10,expectedStatus=200,failureThreshold=2,enabled=False)
            request(owner,base,path+'/monitors/commands','POST',dict(action='save',commandId=str(uuid.uuid4()),id=id,expectedVersion=0,configuration=config))
            incident=str(uuid.uuid4())
            sql(f"UPDATE monitors SET status='{status}',enabled={'false' if index==3 else 'true'},next_check_at=now()+interval '1 day',incident_id={'NULL' if index!=1 else repr(incident)+'::uuid'} WHERE id='{id}';")
            if index<3:
                sql(f"INSERT INTO monitor_samples(id,monitor_id,config_version,checked_at,ok,status_code,latency_ms,error_code) SELECT gen_random_uuid(),'{id}',1,now()-n*interval '5 minutes',n%9<>0,CASE WHEN n%9=0 THEN 503 ELSE 200 END,43+n,CASE WHEN n%9=0 THEN 'unexpected_status' ELSE NULL END FROM generate_series(0,59) n;")
            if index==1:
                event=str(uuid.uuid4())
                sql(f"INSERT INTO monitor_events(id,monitor_id,incident_id,kind) VALUES('{event}','{id}','{incident}','monitor.down'); INSERT INTO inbox_items(id,user_id,project_id,monitor_id,monitor_event_id,category,title,body) VALUES(gen_random_uuid(),'{owner_id}','{project}','{id}','{event}','monitor','Release readiness','down');")
        configs={'webhook':dict(endpoint='https://hooks.example.com/private-browser-key',token='browser-signing-secret'),'github':dict(repository='fixture/browser',token='browser-github-secret'),'jira':dict(endpoint='https://fixture.atlassian.net/',email='bot@example.test',token='browser-jira-secret',projectKey='ENG',issueTypeId='10001'),'email':dict(recipient='team@example.test')}
        integrations={}
        for kind,settings in configs.items():
            integration=request(owner,base,path+'/integrations/'+kind,'PUT',dict(commandId=str(uuid.uuid4()),expectedVersion=0,enabled=True,events=[],settings=settings));integrations[kind]=integration
            delivery=request(owner,base,path+'/integrations/'+integration['id']+'/send','POST',dict(commandId=str(uuid.uuid4()),expectedVersion=1,taskId=None,expectedTaskVersion=None))
            state={'webhook':'failed','github':'delivered','jira':'uncertain','email':'queued'}[kind]
            # Stored historical outcomes exercise layout; the runtime cannot send these fixtures.
            sql(f"UPDATE notification_deliveries SET state='{state}',attempt=1,error_code={'NULL' if state in ['delivered','queued'] else repr('fixture_delivery_error')},finished_at={'NULL' if state=='queued' else 'now()'} WHERE id='{delivery['deliveryId']}';")
        sql(f"INSERT INTO tasks(id,project_id,title,description,kind,priority) VALUES(gen_random_uuid(),'{project}','Copy checkout task','Reviewed task description','task','normal');")
    print('PASS browser fixture ready; outbound deliveries held and checks scheduled beyond test window',flush=True)
