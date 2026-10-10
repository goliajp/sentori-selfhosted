"""Disposable task/inbox browser scenarios; notifications use the real command API."""
import uuid

def run(*,base,owner,sql,password,client,request):
    owner_id=sql("SELECT id FROM users WHERE email='owner@example.test';")
    for engine in ['chromium','webkit']:
        project=request(owner,base,'/admin/api/projects','POST',{'name':'Tasks browser '+engine},201)['id']
        member=request(owner,base,'/admin/api/users','POST',{'email':'tasks-browser-'+engine+'@example.test','password':password},201)
        request(owner,base,f"/admin/api/users/{member['id']}/projects/{project}",'PUT',{})
        http=client();request(http,base,'/auth/login','POST',{'email':'tasks-browser-'+engine+'@example.test','password':password})
        sql(f"""INSERT INTO tasks(id,project_id,title,kind,priority,stage,assignee_user_id,description)
            SELECT gen_random_uuid(),'{project}','Checkout task '||n,'bug','high',CASE WHEN n%3=0 THEN 'review' WHEN n%3=1 THEN 'inProgress' ELSE 'triage' END,'{owner_id}','Check actual deployment evidence before completing.' FROM generate_series(1,55) n;""")
        target=sql(f"SELECT id FROM tasks WHERE project_id='{project}' ORDER BY id LIMIT 1;")
        request(http,base,f'/admin/api/projects/{project}/tasks/commands','POST',dict(action='comment',commandId=str(uuid.uuid4()),taskId=target,expectedVersion=1,body='Please review the checkout fix',mention=owner_id))
