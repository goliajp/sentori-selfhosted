"""Seed known queued/failed states in the disposable live-browser database."""
import uuid


def run(*, base, owner, sql, password, client, request):
    for engine in ['chromium', 'webkit']:
        project = request(owner, base, '/admin/api/projects', 'POST', {'name': 'Jobs browser ' + engine}, 201)['id']
        release = str(uuid.uuid4())
        sql(f"INSERT INTO releases(id,project_id,name) VALUES('{release}','{project}','ui@1');")
        for state in ['queued', 'failed']:
            job = str(uuid.uuid4())
            sql(f"""INSERT INTO jobs(id,project_id,release_id,generation,kind,state,next_attempt_at,attempt,error_code,finished_at)
              VALUES('{job}','{project}','{release}',1,'resymbolicate','{state}',now()+interval '1 hour',{5 if state=='failed' else 0},{"'processing_failed'" if state=='failed' else 'NULL'},{'now()' if state=='failed' else 'NULL'});
              INSERT INTO outbox_events(id,job_id,event_type) VALUES(gen_random_uuid(),'{job}','job.ready');""")
