"""Sampled denominators, project ownership and browser job leases against PostgreSQL."""
import concurrent.futures
import datetime
import json
import time
import uuid

NEEDS_RUNTIME = True


def run(*, base, owner, sql, password, client, request, restart, login, runtime):
    runner_token = 'test-runner-' + uuid.uuid4().hex + uuid.uuid4().hex
    restart(web_runner_token=runner_token)
    _, base = runtime()
    owner = login()
    def call(path, method='GET', data=None, status=200, http=owner):
        return request(http, base, path, method, data, status)
    project=call('/admin/api/projects','POST',{'name':'Web observations'},201)['id']
    foreign=call('/admin/api/projects','POST',{'name':'Other web observations'},201)['id']
    path=f'/admin/api/projects/{project}'
    def token(scope):
        value=call(path+'/tokens','POST',{'name':'Web '+scope,'scope':scope},201)['token']
        http=client();http.addheaders=[('Authorization','Bearer '+value)];return http
    ingest=token('ingest');api=token('api')
    runner=client();runner.addheaders=[('Authorization','Bearer '+runner_token)]
    app=call(path+'/apps','POST',{'platform':'web','identifier':'admin-panel','name':'Admin','kind':'webapp'},201)['id']
    other_app=call(f'/admin/api/projects/{foreign}/apps','POST',{'platform':'web','identifier':'elsewhere','name':'Elsewhere','kind':'webapp'},201)['id']
    member=login('member@example.test')
    member_id=sql("SELECT id FROM users WHERE email='member@example.test';")
    call(f'/admin/api/users/{member_id}/projects/{project}','PUT',{})
    query='/api/web/performance?app='+app
    assert call(query,http=api)['visits']==0
    assert call(query,http=api)['estimatedErrorRate'] is None
    call('/api/web/performance?app='+other_app,http=api,status=404)
    call(query,http=ingest,status=403)
    call(f'/admin/api/projects/{foreign}/web/performance?app={other_app}',http=member,status=403)
    now=datetime.datetime.now(datetime.timezone.utc).isoformat()
    def visit(i,**change):
        return dict(id=str(uuid.uuid4()),revision=1,startedAt=now,appIdentifier='admin-panel',release='web@1',environment='test',route='/orders/:id',deviceClass='mobile',navigationType='document',sampleRate=0.5,browser='Chrome',
            metrics={'LCP':{'id':'metric-'+str(i),'value':1000+i*100,'unit':'ms'}},missing={'INP':'no-interaction'},errorCount=0,resourceErrorCount=0,
            diagnostics={'resources':[],'longTasks':{'count':0,'totalMs':0,'maxMs':0}},**change)
    rows=[visit(i) for i in range(20)]
    rows[0]['errorCount']=1
    assert call('/v1/web/visits','POST',{'visits':rows},http=ingest)['accepted']==20
    assert call('/v1/web/visits','POST',{'visits':rows},http=ingest)['accepted']==0
    summary=call(query,http=api)
    assert summary['visits']==20 and summary['documentVisits']==20 and summary['errorVisits']==1
    assert abs(summary['estimatedErrorRate']-.05)<.000001
    assert summary['metrics'][0]['p75']==2425 and summary['metrics'][0]['assessment']=='good'
    assert summary['coverage']['INP']['no-interaction']==20
    newer=dict(rows[0],revision=2,metrics={'LCP':{'id':'same-metric','value':6000,'unit':'ms'}})
    assert call('/v1/web/visits','POST',{'visits':[newer]},http=ingest)['accepted']==1
    assert call('/v1/web/visits','POST',{'visits':[rows[0]]},http=ingest)['accepted']==0
    assert call(query,http=api)['visits']==20
    for bad in [dict(rows[0],sampleRate=0),dict(rows[0],route='/private?token=secret'),dict(rows[0],metrics={'INP':{'id':'bad','value':10,'unit':'score'}})]:
        call('/v1/web/visits','POST',{'visits':[bad]},http=ingest,status=400)
    call('/v1/web/visits','POST',{'visits':[dict(rows[0],appIdentifier='elsewhere')]},http=ingest,status=404)
    # A route-readiness record cannot inflate the document denominator or masquerade as LCP.
    spa=dict(visit(30),navigationType='spa',metrics={'routeReady':{'id':'spa','value':55,'unit':'ms'}},missing={})
    call('/v1/web/visits','POST',{'visits':[spa]},http=ingest)
    summary=call(query,http=api);assert summary['visits']==21 and summary['documentVisits']==20
    assert next(m for m in summary['metrics'] if m['metric']=='routeReady')['assessment']=='insufficient-data'
    call('/v1/web/visits','POST',{'visits':[dict(spa,metrics=rows[1]['metrics'])]},http=ingest,status=400)
    assert call(query+'&device=desktop',http=api)['visits']==0

    endpoint='/api/web/checks'
    config=dict(appId=app,name='Public site',url='https://web.example/',policy='public',enabled=False,intervalSeconds=0,lighthouse=False,device='desktop',alerts=True,failureThreshold=2)
    def command(action,**fields):return dict(commandId=str(uuid.uuid4()),action=action,**fields)
    payload=command('create',configuration=config)
    with concurrent.futures.ThreadPoolExecutor(4) as pool:
        created=list(pool.map(lambda _:call(endpoint,'POST',payload,http=api),range(8)))
    assert all(c==created[0] for c in created)
    check=created[0]['id']
    call(endpoint,'POST',dict(payload,configuration=dict(config,name='collision')),http=api,status=409)
    call(endpoint,'POST',command('create',configuration=dict(config,appId=other_app)),http=api,status=404)
    call(endpoint,'POST',command('create',configuration=dict(config,url='http://169.254.169.254/')),http=api,status=400)
    call(endpoint,'POST',command('create',configuration=dict(config,intervalSeconds=1)),http=api,status=400)
    call(path+'/web/checks','POST',command('run',id=check),http=member,status=403)
    call('/internal/web/claim','POST',{},status=401,http=api)
    first=call(endpoint,'POST',command('run',id=check),http=api)['id']
    second=call(endpoint,'POST',command('run',id=check),http=api)['id']
    call(endpoint,'POST',command('run',id=check),http=api,status=429)
    job=call('/internal/web/claim','POST',{},http=runner)['job']
    assert job['id'] in [first,second] and job['projectId']==project and job['appId']==app
    assert job['configuration']['url']==config['url']
    def report(status='failed'):
        return dict(source='synthetic',status=status,ruleVersion=1,checkedAt=now,url=config['url'],profile={'device':'desktop'},versions={'runner':'fixture'},metrics=[],findings=[dict(rule='journey.ready',severity='error',evidence='not ready',advice='check readiness',url=config['url'])] if status=='failed' else [],coverage={'indexing':'not-measured'})
    def complete(job,status='failed',expected=200):
        return call('/internal/web/runs/'+job['id']+'/complete','POST',dict(leaseId=job['leaseId'],result=report(status)),http=runner,status=expected)
    call('/internal/web/runs/'+job['id']+'/complete','POST',dict(leaseId=str(uuid.uuid4()),result=report()),http=runner,status=409)
    job2=call('/internal/web/claim','POST',{},http=runner)['job']
    assert job2['id']!=job['id']
    # Finishing the newer run first must still count both failures once both finish.
    complete(job2)
    listed=call(endpoint+'?app='+app,http=api)['checks'][0];assert listed['health']=='degraded' and listed['incidentId'] is None
    complete(job)
    listed=call(endpoint+'?app='+app,http=api)['checks'][0];assert listed['health']=='failing' and listed['incidentId']
    assert sql("SELECT count(*) FROM notification_events WHERE source_key LIKE 'web:%' AND kind='monitor.down';")=='1'
    call(endpoint,'POST',command('run',id=check),http=api)
    recovery=call('/internal/web/claim','POST',{},http=runner)['job'];complete(recovery,'passed')
    assert call(endpoint+'?app='+app,http=api)['checks'][0]['health']=='healthy'
    assert sql("SELECT count(*) FROM notification_events WHERE source_key LIKE 'web:%' AND kind='monitor.recovered';")=='1'
    call(endpoint,'POST',command('run',id=check),http=api)
    stale=call('/internal/web/claim','POST',{},http=runner)['job']
    update=call(endpoint,'POST',command('update',id=check,expectedVersion=1,configuration=dict(config,name='New configuration')),http=api)
    assert update['version']==2
    complete(stale,expected=409)
    assert call('/api/web/runs?app='+app+'&id='+stale['id'],http=api)['runs'][0]['status']=='cancelled'
    call('/api/web/runs?app='+other_app+'&id='+stale['id'],http=api,status=404)
    call(endpoint,'POST',command('update',id=check,expectedVersion=1,configuration=config),http=api,status=409)
    # Schedules start only after explicitly enabled; their frozen config has the current version.
    call(endpoint,'POST',command('update',id=check,expectedVersion=2,configuration=dict(config,enabled=True,intervalSeconds=900)),http=api)
    until=time.monotonic()+40
    while time.monotonic()<until:
        if sql(f"SELECT count(*) FROM web_check_runs WHERE check_id='{check}' AND trigger='schedule';")!='0':break
        time.sleep(.5)
    assert sql(f"SELECT count(*) FROM web_check_runs WHERE check_id='{check}' AND trigger='schedule';")=='1'
    scheduled=call('/internal/web/claim','POST',{},http=runner)['job'];assert scheduled
    complete(scheduled,'error')
    assert call(endpoint+'?app='+app,http=api)['checks'][0]['health']=='monitor-error'
    assert sql("SELECT count(*) FROM notification_events WHERE source_key LIKE 'web:%' AND kind='monitor.down';")=='1'
    call(endpoint,'POST',command('delete',id=check,expectedVersion=3),http=api)
    assert call(endpoint+'?app='+app,http=api)['checks']==[]
    print('PASS Web RUM denominators, revisions, scopes, command replay, runner leases, persistent failures and recovery')
