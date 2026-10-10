"""Task commands, real inbox delivery, scope, rollback and ingest regression."""
import concurrent.futures
import time
import uuid


NEEDS_RUNTIME = True


def run(*, base, owner, sql, password, client, request, restart, login, runtime):
    def call(path, method='GET', data=None, status=200, http=None):
        return request(http or owner, base, path, method, data, status)
    def wait(test):
        until=time.monotonic()+15
        while time.monotonic()<until:
            value=test()
            if value: return value
            time.sleep(.1)
        raise AssertionError('inbox delivery timeout')
    project=call('/admin/api/projects','POST',{'name':'Tasks'},201)['id']
    foreign=call('/admin/api/projects','POST',{'name':'Private tasks'},201)['id']
    account=call('/admin/api/users','POST',{'email':'tasks-member@example.test','password':password},201)
    member=client()
    call('/auth/login','POST',{'email':'tasks-member@example.test','password':password},http=member)
    call(f"/admin/api/users/{account['id']}/projects/{project}",'PUT',{})
    fields=dict(title='Investigate checkout %_',description='Evidence',kind='bug',priority='high',assignee=account['id'],due='2026-10-12',reproduction='',rootCause='')
    path=f'/admin/api/projects/{project}/tasks'
    def cmd(action,**body):
        status=body.pop('status',200);http=body.pop('http',None)
        return call(path+'/commands','POST',dict(commandId=str(uuid.uuid4()),action=action,**body),status,http)
    def item(id):return call(path+'/'+id)['task']
    def inbox(http=member,query=''):return call('/admin/api/inbox'+query,http=http)
    payload=dict(action='create',commandId=str(uuid.uuid4()),issueId=None,fields=fields)
    with concurrent.futures.ThreadPoolExecutor(max_workers=6) as executor:
        tasks=list(executor.map(lambda _:call(path+'/commands','POST',payload),range(12)))
    task=tasks[0];id=task['id']
    assert all(t==task for t in tasks)
    assert sql(f"SELECT count(*) FROM task_events WHERE task_id='{id}';")=='1'
    assert sql(f"SELECT count(*) FROM task_commands WHERE project_id='{project}';")=='1'
    call(path+'/commands','POST',dict(payload,fields=dict(fields,title='Different')),409)
    rows=wait(lambda:inbox()['items']);note=rows[0]
    assert len(rows)==1 and note['category']=='assignment' and note['taskId']==id
    assert not inbox(owner)['items']
    call('/admin/api/inbox/'+note['id'],'PATCH',{'read':True},404)
    call('/admin/api/inbox/'+note['id'],'PATCH',{'read':True},http=member)
    assert not inbox(query='?unread=true')['items']
    call('/admin/api/inbox/'+note['id'],'PATCH',{'archived':True},http=member)
    assert inbox()['total']==0 and inbox(query='?archived=true')['total']==1
    print('PASS concurrent task commands and private idempotent inbox actions',flush=True)

    cmd('transition',taskId=id,expectedVersion=1,stage='done',status=409)
    task=cmd('transition',taskId=id,expectedVersion=1,stage='ready')
    cmd('edit',taskId=id,expectedVersion=1,fields=fields,status=409)
    cmd('edit',taskId=id,expectedVersion=task['version'],fields=dict(fields,due='2026-99-88'),status=400)
    cmd('create',issueId=None,fields=dict(fields,title='  '),status=400)
    cmd('comment',taskId=id,expectedVersion=task['version'],body='  ',mention=None,status=400)
    cmd('comment',taskId=id,expectedVersion=task['version'],body='secret',mention=str(uuid.uuid4()),status=400)
    call(f'/admin/api/projects/{foreign}/tasks/{id}',status=404)
    call(f'/admin/api/projects/{foreign}/tasks',status=403,http=member)
    assert call(path+'?q=%25_')['total']==1
    assert call(path+'?offset=100')['total']==1
    assert call(path+'?mine=true',http=member)['total']==1

    # An audit failure cannot commit a task, event, dedup record or outbox record.
    sql("CREATE FUNCTION reject_task_audit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.action LIKE 'task.%' THEN RAISE EXCEPTION 'fixture audit failure'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_task_audit BEFORE INSERT ON audit_logs FOR EACH ROW EXECUTE FUNCTION reject_task_audit();")
    before=sql("SELECT (SELECT count(*) FROM tasks)||','||(SELECT count(*) FROM task_events)||','||(SELECT count(*) FROM task_commands)||','||(SELECT count(*) FROM outbox_events);")
    cmd('create',issueId=None,fields=fields,status=500)
    cmd('comment',taskId=id,expectedVersion=task['version'],body='rollback',mention=None,status=500)
    after=sql("SELECT (SELECT count(*) FROM tasks)||','||(SELECT count(*) FROM task_events)||','||(SELECT count(*) FROM task_commands)||','||(SELECT count(*) FROM outbox_events);")
    assert before==after and item(id)['version']==task['version']
    sql('DROP TRIGGER reject_task_audit ON audit_logs; DROP FUNCTION reject_task_audit();')
    print('PASS version fences, validation, literal filtering and transaction rollback',flush=True)

    prefs=dict(assignments=True,mentions=False,taskUpdates=False,monitors=True)
    call('/admin/api/inbox-preferences','PUT',prefs,http=member)
    assert call('/admin/api/inbox-preferences',http=member)==prefs
    task=cmd('comment',taskId=id,expectedVersion=task['version'],body='Muted mention',mention=account['id'])
    wait(lambda:sql(f"SELECT count(*) FROM outbox_events o JOIN task_events e ON e.id=o.task_event_id WHERE e.task_id='{id}' AND o.delivered_at IS NULL;")=='0')
    assert all(n['body']!='Muted mention' for n in inbox()['items'])
    call('/admin/api/inbox-preferences','PUT',dict(prefs,mentions=True),http=member)
    task=cmd('comment',taskId=id,expectedVersion=task['version'],body='Please review',mention=account['id'])
    note=wait(lambda:next((n for n in inbox()['items'] if n['body']=='Please review'),None))
    assert note['category']=='mention'
    sql(f"UPDATE outbox_events SET delivered_at=NULL WHERE task_event_id IN(SELECT id FROM task_events WHERE task_id='{id}');")
    wait(lambda:sql(f"SELECT count(*) FROM outbox_events o JOIN task_events e ON e.id=o.task_event_id WHERE e.task_id='{id}' AND o.delivered_at IS NULL;")=='0')
    assert len([n for n in inbox()['items'] if n['body']=='Please review'])==1
    call(f"/admin/api/users/{account['id']}/projects/{project}",'DELETE')
    assert not inbox()['items'] and not inbox(query='?archived=true')['items']
    call('/admin/api/inbox/'+note['id'],'PATCH',{'read':True},404,member)
    call(path+'/'+id,status=403,http=member)
    cmd('create',issueId=None,fields=fields,status=400)
    print('PASS current membership, preferences, mention delivery and consumer replay deduplication',flush=True)

    # Real SDK regression must atomically reopen the linked task and erase old proof.
    token=call(f'/admin/api/projects/{project}/tokens','POST',{'name':'task tests','scope':'ingest'},201)['token']
    import json, urllib.request
    def ingest():
        event=dict(kind='error',platform='javascript',occurredAt='2026-10-10T00:00:00Z',release='task@2',environment='production',payload={'error':{'type':'TaskRegression','message':'again','stack':[]}})
        req=urllib.request.Request(base+'/v1/events',data=json.dumps(event).encode(),headers={'Content-Type':'application/json','Authorization':'Bearer '+token})
        with urllib.request.urlopen(req) as response:return json.load(response)
    ingest()
    issue=sql(f"SELECT id FROM issues WHERE project_id='{project}' AND group_title='TaskRegression';")
    assert issue
    linked=cmd('create',issueId=issue,fields=dict(fields,assignee=None))
    cmd('create',issueId=issue,fields=dict(fields,assignee=None),status=409)
    other_issue=str(uuid.uuid4())
    sql(f"INSERT INTO issues(id,project_id,fingerprint,kind,group_title,first_seen,last_seen) VALUES('{other_issue}','{foreign}','private-task','error','Private',now(),now());")
    cmd('create',issueId=other_issue,fields=dict(fields,assignee=None),status=400)
    for stage in ['ready','inProgress','review','verify','done']:
        linked=cmd('transition',taskId=linked['id'],expectedVersion=linked['version'],stage=stage)
    assert sql(f"SELECT status FROM issues WHERE id='{issue}';")=='resolved'
    assert not linked['verified'] and not linked['fixRelease']
    print('PASS issue linkage scope and explicit manual completion without fabricated verification',flush=True)
    sql(f"UPDATE tasks SET verified=true,fix_release='task@1',automation='{{\"workingCommit\":\"old\"}}' WHERE id='{linked['id']}'; UPDATE issues SET resolved_at='2026-10-09T00:00:00Z' WHERE id='{issue}';")
    ingest()
    reopened=item(linked['id'])
    assert reopened['stage']=='inProgress' and not reopened['verified'] and not reopened['fixRelease'] and 'regressionAt' in reopened['automation']
    assert sql(f"SELECT count(*) FROM task_events WHERE task_id='{linked['id']}' AND action='issue.regressed';")=='1'
    assert sql(f"SELECT count(*) FROM audit_logs WHERE target_id='{linked['id']}' AND action='task.regressed';")=='1'
    ingest()
    assert item(linked['id'])['version']==reopened['version']
    print('PASS actual SDK recurrence reopens task once and invalidates all prior proof',flush=True)

    # Signed generic CI adapter. Every request reaches the actual public webhook.
    import datetime, hashlib, hmac, urllib.error
    integration=f'/admin/api/projects/{project}/task-integration'
    config=dict(repository='acme/checkout',environment='production',enabled=True,rotateSecret=False,expectedVersion=0)
    configured=call(integration,'PUT',config)
    secret=configured['secret'];assert len(secret)==64
    assert call(integration)['configuration']['secretConfigured']
    assert secret not in str(call(integration))
    assert secret.encode().hex() not in sql(f"SELECT encode(secret_blob,'hex') FROM task_integrations WHERE project_id='{project}';")
    call(integration,'PUT',config,409)
    call(integration,'PUT',dict(config,expectedVersion=1),403,member)
    def signal(kind,commit='a'*40,**extra):
        return dict(deliveryId=uuid.uuid4().hex,taskId=linked['id'],repository='acme/checkout',commit=commit,kind=kind,occurredAt=datetime.datetime.now(datetime.timezone.utc).isoformat(),**extra)
    def deliver(payload,status=200,key=None,timestamp=None,signature=None):
        raw=json.dumps(payload).encode();stamp=str(int(time.time()) if timestamp is None else timestamp)
        sig=signature or 'sha256='+hmac.new((key or secret).encode(),stamp.encode()+b'.'+raw,hashlib.sha256).hexdigest()
        req=urllib.request.Request(base+f'/hooks/tasks/{project}',data=raw,headers={'Content-Type':'application/json','X-Sentori-Timestamp':stamp,'X-Sentori-Signature':sig})
        try:
            with urllib.request.urlopen(req,timeout=15) as response:
                result=json.load(response);assert response.code==status,(response.code,result);return result
        except urllib.error.HTTPError as failure:
            result=json.load(failure);assert failure.code==status,(failure.code,result);return result
    work=signal('work.started')
    deliver(work,401,key='wrong')
    deliver(work,401,timestamp=int(time.time())-301)
    deliver(dict(work,repository='acme/foreign'),400)
    deliver(dict(work,commit='short'),400)
    deliver(dict(work,occurredAt=(datetime.datetime.now(datetime.timezone.utc)+datetime.timedelta(seconds=30)).isoformat()),400)
    assert sql(f"SELECT count(*) FROM task_signal_deliveries WHERE project_id='{project}';")=='0'
    with concurrent.futures.ThreadPoolExecutor(max_workers=6) as executor:
        outcomes=list(executor.map(lambda _:deliver(work)['outcome'],range(10)))
    assert outcomes.count('accepted')==1 and outcomes.count('duplicate')==9
    deliver(dict(work,commit='b'*40),409)
    assert item(linked['id'])['stage']=='inProgress'
    deliver(signal('ci.completed',unit='passed',e2e='passed'))
    deliver(signal('pr.opened',number=12))
    assert item(linked['id'])['stage']=='review'
    deliver(signal('pr.merged',number=12))
    assert item(linked['id'])['stage']=='verify' and not item(linked['id'])['verified']
    deliver(signal('release.deployed',release='task@3',environment='staging'),400)
    deliver(signal('release.deployed',commit='b'*40,release='task@3',environment='production'))
    assert item(linked['id'])['stage']=='verify'
    deliver(signal('release.deployed',release='task@3',environment='production'))
    done=item(linked['id']);assert done['stage']=='done' and done['verified'] and done['fixRelease']=='task@3'
    assert sql(f"SELECT status||':'||resolved_in_release FROM issues WHERE id='{issue}';")=='resolved:task@3'
    # A later failure on another commit cannot clobber the verified commit.
    deliver(signal('ci.completed',commit='b'*40,unit='failed',e2e='failed'))
    assert item(linked['id'])['version']==done['version']
    deliver(signal('ci.completed',unit='failed',e2e='passed'))
    assert not item(linked['id'])['verified'] and item(linked['id'])['stage']=='verify'
    assert sql(f"SELECT status FROM issues WHERE id='{issue}';")=='open'
    print('PASS signed webhook scope, replay bounds, concurrent dedup and same-commit verification',flush=True)

    old_pr=signal('pr.merged',number=12)
    deliver(signal('work.started',commit='b'*40))
    deliver(old_pr)
    assert item(linked['id'])['automation']['workingCommit']=='b'*40
    deliver(signal('ci.completed',commit='b'*40,unit='passed',e2e='passed'))
    deliver(signal('pr.merged',commit='b'*40,number=13))
    deliver(signal('release.deployed',commit='b'*40,release='task@4',environment='production'))
    assert item(linked['id'])['verified']
    old=signal('ci.completed',commit='b'*40,unit='passed',e2e='passed')
    # Real recurrence invalidates all signatures' old evidence via a server time barrier.
    sql(f"UPDATE issues SET resolved_at='2026-10-09T00:00:00Z',resolved_in_release=NULL WHERE id='{issue}';")
    ingest()
    current=item(linked['id']);assert current['stage']=='inProgress' and not current['verified']
    assert deliver(old)['outcome']=='ignored'
    assert item(linked['id'])['version']==current['version']
    rotated=call(integration,'PUT',dict(config,expectedVersion=1,rotateSecret=True))
    deliver(signal('work.started'),401)
    secret=rotated['secret']
    deliver(signal('pr.merged',commit='b'*40,number=14))
    assert not item(linked['id'])['verified'],'configuration changes invalidate old CI/deployment proof'
    call(integration,'PUT',dict(config,expectedVersion=2,enabled=False))
    deliver(signal('work.started'),404)
    print('PASS stale authority, regression barrier, key rotation, configuration epochs and disabling',flush=True)

    call(integration,'PUT',dict(config,expectedVersion=3))
    atomic=signal('work.started')
    sql("CREATE FUNCTION reject_automatic_audit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.action='task.automated' THEN RAISE EXCEPTION 'fixture automatic audit failure'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_automatic_audit BEFORE INSERT ON audit_logs FOR EACH ROW EXECUTE FUNCTION reject_automatic_audit();")
    before=item(linked['id'])
    deliver(atomic,500)
    assert item(linked['id'])==before
    assert sql(f"SELECT count(*) FROM task_signal_deliveries WHERE delivery_id='{atomic['deliveryId']}';")=='0'
    sql('DROP TRIGGER reject_automatic_audit ON audit_logs; DROP FUNCTION reject_automatic_audit();')
    assert deliver(atomic)['outcome']=='accepted'
    print('PASS automation, delivery dedup record and audit share one transaction',flush=True)

    call(f"/admin/api/users/{account['id']}/projects/{project}",'PUT',{})
    sql("CREATE FUNCTION delay_inbox_delivery() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.body='resume-inbox' THEN RAISE EXCEPTION 'fixture unavailable inbox'; END IF; RETURN NEW; END $$; CREATE TRIGGER delay_inbox_delivery BEFORE INSERT ON inbox_items FOR EACH ROW EXECUTE FUNCTION delay_inbox_delivery();")
    current=item(id)
    cmd('comment',taskId=id,expectedVersion=current['version'],body='resume-inbox',mention=account['id'])
    assert sql("SELECT count(*) FROM outbox_events o JOIN task_events e ON e.id=o.task_event_id WHERE e.body='resume-inbox' AND o.delivered_at IS NULL;")=='1'
    import subprocess
    subprocess.run(['docker','kill','--signal=KILL',runtime()[0]],check=True,capture_output=True)
    restart();base=runtime()[1];owner=login()
    sql('DROP TRIGGER delay_inbox_delivery ON inbox_items; DROP FUNCTION delay_inbox_delivery();')
    wait(lambda:any(n['body']=='resume-inbox' for n in inbox()['items']))
    assert sql("SELECT count(*) FROM inbox_items WHERE body='resume-inbox';")=='1'
    result=call('/admin/api/inbox/read-all','POST',{},http=member)
    assert result['updated']>=1 and inbox()['unread']==0
    assert call('/admin/api/inbox/read-all','POST',{},http=member)['updated']==0
    print('PASS failed inbox delivery survives real process kill, resumes once and bulk-read is idempotent',flush=True)
