"""Real telemetry filtering and workbench query contracts in a disposable instance."""
import json
import uuid
from urllib.parse import urlencode


def run(*, base, owner, sql, password, client, request):
    def call(path, method='GET', data=None, status=200, http=owner):
        return request(http, base, path, method, data, status)

    project = call('/admin/api/projects', 'POST', {'name':'Workbench audit'}, 201)['id']
    foreign = call('/admin/api/projects', 'POST', {'name':'Other workbench'}, 201)['id']
    app_a, app_b, other_app, issue, other_issue = [str(uuid.uuid4()) for _ in range(5)]
    sql(f"""INSERT INTO apps(id,project_id,platform,identifier) VALUES
      ('{app_a}','{project}','android','client-a'),('{app_b}','{project}','android','client-b'),
      ('{other_app}','{foreign}','ios','private-client');
      INSERT INTO issues(id,project_id,fingerprint,kind,group_title,event_count,users_count,max_per_user,last_release,first_seen,last_seen)
      VALUES ('{issue}','{project}','shared','error','Shared failure',3,2,2,'2.0',now(),now()),
             ('{other_issue}','{foreign}','private','error','Secret failure',1,1,1,'secret',now(),now());""")
    event_ids=[]
    observations=[(project,issue,app_a,'production','1.0',{'context':{'appType':'insight','tenant':'A','enabled':True,'empty':''},'tags':{'region':'jp'}},'u1'),
                  (project,issue,app_a,'staging','2.0',{'context':{'appType':'insight','tenant':'B'},'tags':{'region':'us'}},'u2'),
                  (project,issue,app_b,'production','2.0',{'context':{'appType':'home','tenant':'B'},'tags':{'region':'jp'}},'u2'),
                  (foreign,other_issue,other_app,'private','secret',{'context':{'secretKey':'secretValue'}},'secret')]
    for p,i,a,env,release,payload,user in observations:
        eid=str(uuid.uuid4());event_ids.append(eid)
        body=json.dumps(payload).replace("'","''")
        sql(f"""INSERT INTO events(id,project_id,issue_id,app_id,kind,platform,occurred_at,received_at,environment,release,payload,user_key)
          VALUES ('{eid}','{p}','{i}','{a}','error','android',now(),now(),'{env}','{release}','{body}','{user}');""")
    def listing(**filters):
        return call('/admin/api/issues?'+urlencode({'project_id':project,'status':'all',**filters}))
    def tags(*items):
        return json.dumps([{'source':source,'key':key,'value':value} for source,key,value in items])

    assert listing()['summary']['events']==3
    assert listing(app_id=app_a)['issues'][0]['eventCount']==2
    assert listing(app_id=app_a)['issues'][0]['usersCount']==2
    assert listing(app_id=app_b)['summary']['events']==1
    assert listing(app_id=other_app)['total']==0
    assert listing(platform='ios')['total']==0
    assert listing(app_id=app_a,environment='production',release='2.0')['total']==0, 'dimensions must match one event'
    assert listing(environment='production',context_key='tenant',context_value='B')['summary']['events']==1
    selected=listing(app_id=app_a,environment='production',tags=tags(('context','appType','insight'),('tags','region','jp')))
    assert selected['total']==1 and selected['summary']=={'events':1,'open':1,'regressed':0,'releases':1}
    assert selected['issues'][0]['lastRelease']=='1.0'
    assert listing(tags=tags(('context','tenant','A'),('tags','region','us')))['total']==0
    assert listing(tags=tags(('context','enabled','true'),('context','empty','')))['total']==1
    assert listing(tags=tags(('context','missing','')))['total']==0, 'missing is not an empty tag'
    for params in [{'platform':'bogus'},{'context_key':'tenant'},{'tags':'{}'}, {'tags':tags(('wrong','x','y'))},{'tags':tags(*[('context','x','y')]*13)}]:
        call('/admin/api/issues?'+urlencode({'project_id':project,**params}),status=400)
    selected_events=call('/admin/api/issues/'+issue+'/events?'+urlencode({'app_id':app_b,'environment':'production'}))
    assert selected_events['total']==1 and selected_events['events'][0]['id']==event_ids[2]
    assert call('/admin/api/issues/'+issue+'/events?'+urlencode({'app_id':app_a,'environment':'production','release':'2.0'}))['total']==0
    assert call(f'/admin/api/projects/{project}/environments?app_id={app_b}')['environments']==['production']
    assert set(call(f'/admin/api/projects/{project}/context-keys?source=tags')['keys'])=={'region'}
    values=call(f'/admin/api/projects/{project}/context-values?'+urlencode({'source':'context','key':'appType','app_id':app_b}))
    assert values['values']==['home']
    assert 'secretKey' not in call(f'/admin/api/projects/{project}/context-keys')['keys']
    # Malformed historical context does not hide the whole facet list.
    sql(f"UPDATE events SET payload='{{\"context\":[]}}' WHERE id='{event_ids[1]}';")
    assert 'tenant' in call(f'/admin/api/projects/{project}/context-keys')['keys']
    member=call('/admin/api/users','POST',{'email':'audit-member@example.test','password':password},201)
    member_http=client();call('/auth/login','POST',{'email':'audit-member@example.test','password':password},http=member_http)
    call(f'/admin/api/users/{member["id"]}/projects/{project}','PUT',{})
    call(f'/admin/api/projects/{foreign}/context-keys',status=403,http=member_http)
    assert listing(app_id=app_a)['total']==1
    # Charts use actual receive-time buckets; launch excludes prewarmed starts.
    launch_issue=str(uuid.uuid4())
    sql(f"""INSERT INTO issues(id,project_id,fingerprint,kind,group_title,event_count,last_release,first_seen,last_seen)
      VALUES ('{launch_issue}','{project}','launch','trace','app.launch',3,'1.0',now(),now());""")
    for duration,prewarmed in [(100,False),(300,False),(9000,True)]:
        payload=json.dumps({'data':{'totalMs':duration,'prewarmed':prewarmed}})
        sql(f"""INSERT INTO events(id,project_id,issue_id,app_id,kind,platform,occurred_at,received_at,environment,release,payload)
          VALUES ('{uuid.uuid4()}','{project}','{launch_issue}','{app_a}','trace','android',now(),now(),'production','1.0','{payload}');""")
    instruments=call(f'/admin/api/projects/{project}/instruments?hours=24')
    assert instruments['windowHours']==24 and sum(row['events'] for row in instruments['activity'])==6
    assert sum(row['errors'] for row in instruments['activity'])==3
    assert sum(row['traces'] for row in instruments['activity'])==3
    assert instruments['launch'][0]['samples']==3 and instruments['launch'][0]['prewarmed']==1
    assert instruments['launch'][0]['p50']==200
    assert len(instruments['traces'])==1 and instruments['traces'][0]['eventCount']==3
    assert call(f'/admin/api/projects/{project}/instruments?release=missing')['launch']==[]
    assert sum(row['events'] for row in call(f'/admin/api/projects/{project}/instruments?q=missing')['activity'])==0
    call(f'/admin/api/projects/{project}/instruments?hours=999',status=400)
    call(f'/admin/api/projects/{foreign}/instruments',status=403,http=member_http)
    # Failed reads must not be painted as a healthy, empty dashboard.
    sql('ALTER TABLE assert_stats RENAME TO audit_hidden_assert_stats;')
    try: call(f'/admin/api/projects/{project}/instruments',status=500)
    finally: sql('ALTER TABLE audit_hidden_assert_stats RENAME TO assert_stats;')
    release_id,foreign_release=[str(uuid.uuid4()) for _ in range(2)]
    sql(f"INSERT INTO releases(id,project_id,name) VALUES ('{release_id}','{project}','deep-link@1'),('{foreign_release}','{foreign}','hidden@1');")
    assert call(f'/admin/api/projects/{project}/releases?id={release_id}')['releases'][0]['name']=='deep-link@1'
    assert call(f'/admin/api/projects/{project}/releases?id={foreign_release}')['total']==0
    assert call(f'/admin/api/projects/{project}/health')['counts24h']['error']==3
    sql('ALTER TABLE events RENAME TO audit_hidden_events;')
    try: call(f'/admin/api/projects/{project}/health',status=500)
    finally: sql('ALTER TABLE audit_hidden_events RENAME TO events;')
    print('PASS workbench event intersections, scoped metrics, facets, actual charts and read failures')
