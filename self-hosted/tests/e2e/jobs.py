"""Durable queue, transaction rollback, authorization and real crash recovery.

Run via instance.py --domain-test self-hosted/tests/e2e/jobs.py.
All database faults and process stops target only this disposable fixture.
"""
import concurrent.futures
import json
import subprocess
import time
import urllib.error
import urllib.request
import uuid

NEEDS_RUNTIME = True


def run(*, base, owner, sql, password, client, request, restart, login, runtime):
    def call(path, method='GET', data=None, status=200, http=None):
        return request(http or owner, runtime()[1], path, method, data, status)

    def wait(test, label, seconds=35):
        until = time.monotonic() + seconds
        while time.monotonic() < until:
            result = test()
            if result:
                return result
            time.sleep(.1)
        raise AssertionError('Timed out: ' + label)

    project, foreign = [call('/admin/api/projects', 'POST', {'name': name}, 201)['id'] for name in ['Durable jobs', 'Foreign jobs']]
    release, other_release, private_release = [str(uuid.uuid4()) for _ in range(3)]
    sql(f"INSERT INTO releases(id,project_id,name) VALUES ('{release}','{project}','durable@1'),('{other_release}','{project}','other@1'),('{private_release}','{foreign}','private@1');")
    path = f'/admin/api/projects/{project}'
    def enqueue(key=None, rel=release, status=202, http=None):
        return call(path + f'/releases/{rel}/resymbolicate', 'POST', {'idempotencyKey': key or uuid.uuid4().hex}, status, http)
    def job(id): return call(path + '/jobs/' + id)
    def activate(id): sql(f"UPDATE jobs SET next_attempt_at=now() WHERE id='{id}';")
    def terminal(id, expected='succeeded'):
        result = wait(lambda: (row if (row := job(id))['state'] in ['succeeded', 'failed', 'cancelled'] else None), 'terminal ' + id)
        assert result['state'] == expected, result
        return result

    # Delay only this fixture's new jobs, so races are released deliberately.
    sql("CREATE FUNCTION delay_jobs() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN NEW.next_attempt_at=now()+interval '1 hour'; RETURN NEW; END $$; CREATE TRIGGER delay_jobs BEFORE INSERT ON jobs FOR EACH ROW EXECUTE FUNCTION delay_jobs();")
    with concurrent.futures.ThreadPoolExecutor(max_workers=8) as executor:
        ids = list(executor.map(lambda _: enqueue('same-request')['jobId'], range(16)))
    first = ids[0]
    assert len(set(ids)) == 1
    assert sql(f"SELECT count(*) FROM audit_logs WHERE target_id='{first}' AND action='job.queued';") == '1'
    assert sql(f"SELECT count(*) FROM outbox_events WHERE job_id='{first}' AND delivered_at IS NULL;") == '1'
    enqueue('same-request', rel=other_release, status=409)
    for key in ['', ' ', 'x' * 129, '\n']:
        call(path + f'/releases/{release}/resymbolicate', 'POST', {'idempotencyKey': key}, 400)
    enqueue(rel=private_release, status=404)
    call(path + '/jobs/' + str(uuid.uuid4()), status=404)
    activate(first)
    assert terminal(first)['scanned'] == 0
    assert sql(f"SELECT count(*) FROM outbox_events WHERE job_id='{first}' AND delivered_at IS NOT NULL;") == '1'
    assert enqueue('same-request')['jobId'] == first
    print('PASS concurrent idempotency, digest collision, initial outbox delivery', flush=True)

    account = call('/admin/api/users', 'POST', {'email': 'jobs-member@example.test', 'password': password}, 201)
    member = client()
    call('/auth/login', 'POST', {'email': 'jobs-member@example.test', 'password': password}, http=member)
    for suffix in ['/jobs', '/jobs/' + first, '/jobs/' + first + '/cancel']:
        call(path + suffix, 'POST' if suffix.endswith('/cancel') else 'GET', {} if suffix.endswith('/cancel') else None, 403, member)
    enqueue(http=member, status=403)
    call(f"/admin/api/users/{account['id']}/projects/{project}", 'PUT', {})
    assert call(path + '/jobs/' + first, http=member)['id'] == first
    call(f'/admin/api/projects/{foreign}/jobs/{first}', status=403, http=member)
    call(f'/admin/api/projects/{foreign}/jobs/{first}', status=404)
    own = enqueue('same-request', http=member)['jobId']
    assert own != first, 'keys are scoped to the authenticated actor'
    call(path + '/jobs/' + own + '/cancel', 'POST', {}, http=member)
    assert terminal(own, 'cancelled')['scanned'] == 0
    call(path + '/jobs/' + own + '/cancel', 'POST', {}, http=member)
    assert sql(f"SELECT count(*) FROM audit_logs WHERE action='job.cancel_requested' AND target_id='{own}';") == '1'
    call(f"/admin/api/users/{account['id']}/projects/{project}", 'DELETE')
    call(path + '/jobs/' + own, status=403, http=member)
    call(path + '/jobs', status=401, http=client())
    print('PASS project authorization, revoked membership and idempotent cancellation', flush=True)

    sourcemap = json.dumps({'version': 3, 'file': 'index.android.bundle', 'sources': ['src/checkout.ts'],
        'sourcesContent': ["export function charge(userId) {\n  const token = mintToken(userId)\n  return post('/pay', { token })\n}\n"], 'names': [], 'mappings': 'oBAEI'})
    def upload(expected=201):
        boundary = 'durable-jobs-boundary'
        content = (f'--{boundary}\r\nContent-Disposition: form-data; name="kind"\r\n\r\nsourcemap\r\n'
                   f'--{boundary}\r\nContent-Disposition: form-data; name="file"; filename="index.android.bundle.map"\r\nContent-Type: application/json\r\n\r\n{sourcemap}\r\n--{boundary}--\r\n').encode()
        req = urllib.request.Request(runtime()[1] + path + f'/releases/{release}/artifacts', data=content,
            headers={'Content-Type': 'multipart/form-data; boundary=' + boundary})
        try: response = owner.open(req, timeout=15)
        except urllib.error.HTTPError as failure: response = failure
        body = json.load(response)
        assert response.code == expected, (response.code, body)
        return body

    before = sql('SELECT count(*) FROM jobs;')
    sql("CREATE FUNCTION fail_job_audit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.action='job.queued' THEN RAISE EXCEPTION 'injected audit failure'; END IF; RETURN NEW; END $$; CREATE TRIGGER fail_job_audit BEFORE INSERT ON audit_logs FOR EACH ROW EXECUTE FUNCTION fail_job_audit();")
    enqueue(status=500)
    upload(500)
    assert sql('SELECT count(*) FROM jobs;') == before
    assert sql(f"SELECT count(*) FROM release_artifacts WHERE release_id='{release}';") == '0'
    assert sql('SELECT count(*) FROM jobs;') == sql('SELECT count(*) FROM outbox_events;') == sql('SELECT count(*) FROM idempotency_requests;')
    sql('DROP TRIGGER fail_job_audit ON audit_logs; DROP FUNCTION fail_job_audit();')
    print('PASS artifact, command, idempotency, audit and outbox roll back together', flush=True)

    issue = str(uuid.uuid4())
    sql(f"INSERT INTO issues(id,project_id,fingerprint,kind,group_title,first_seen,last_seen) VALUES('{issue}','{project}','job-test','error','Stored crash',now(),now());")
    raw_payload = json.dumps({'error': {'type': 'TypeError', 'message': 'before map', 'stack': [{'file': 'index.android.bundle', 'line': 1, 'column': 20, 'function': 'e', 'inApp': True}]}})
    def events(count):
        sql(f"INSERT INTO events(id,project_id,issue_id,kind,platform,occurred_at,release,payload) SELECT gen_random_uuid(),'{project}','{issue}','error','android',now()-interval '1 day','durable@1','{raw_payload}'::jsonb FROM generate_series(1,{count});")
    events(63)
    result = upload()
    assert result['usable'] is True
    actual = result['jobId']
    assert job(actual)['state'] == 'queued'
    activate(actual)
    result = terminal(actual)
    assert (result['scanned'], result['rewritten'], result['frames']) == (63, 63, 63), result
    assert sql(f"SELECT count(*) FROM events WHERE issue_id='{issue}' AND payload#>>'{{error,stack,0,file}}'='src/checkout.ts';") == '63'
    repeat = enqueue()['jobId']; activate(repeat)
    assert terminal(repeat)['rewritten'] == 0
    print('PASS real sourcemap recovery, stable cursor across timestamp ties, repeat safety', flush=True)

    # Missing blobs are transient work failures, while the ingest path must
    # continue accepting events and use the other valid map that is available.
    fake_artifact = str(uuid.uuid4())
    sql(f"INSERT INTO release_artifacts(id,release_id,kind,name,content_hash,size_bytes,usable) VALUES('{fake_artifact}','{release}','sourcemap','missing.map','{'a'*64}',10,true);")
    missing = enqueue()['jobId']; activate(missing)
    wait(lambda: job(missing)['state'] == 'retry_wait', 'missing blob retry')
    assert job(missing)['scanned'] == 0
    token = call(path + '/tokens', 'POST', {'name': 'ingest during blob failure', 'scope': 'ingest'}, 201)['token']
    body = {'kind': 'error', 'occurredAt': '2026-10-10T00:00:00Z', 'platform': 'android', 'release': 'durable@1', 'payload': json.loads(raw_payload)}
    req = urllib.request.Request(runtime()[1] + '/v1/events', data=json.dumps(body).encode(), headers={'Authorization': 'Bearer ' + token, 'Content-Type': 'application/json'})
    with urllib.request.urlopen(req, timeout=15) as response:
        assert response.code == 202
    sql(f"DELETE FROM release_artifacts WHERE id='{fake_artifact}';")
    activate(missing)
    recovered = terminal(missing)
    assert recovered['attempt'] == 2 and recovered['scanned'] == 63, recovered
    print('PASS unavailable blob retries, ingest remains available, snapshot excludes later events', flush=True)

    # Stop the worker before its write transaction by holding only the artifact
    # table read lock. The lease may change during computation; old work must
    # then discard its result instead of writing under the replacement lease.
    def hold_artifacts():
        db = runtime()[0].removesuffix('-app') + '-db'
        process = subprocess.Popen(['docker', 'exec', '-i', db, 'psql', '-v', 'ON_ERROR_STOP=1', '-U', 'sentori', '-d', 'fresh', '-qtA'], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        process.stdin.write("BEGIN; LOCK TABLE release_artifacts IN ACCESS EXCLUSIVE MODE; SELECT 'LOCK_READY';\n")
        process.stdin.flush()
        assert process.stdout.readline().strip() == 'LOCK_READY'
        return process
    fenced = enqueue()['jobId']
    blocker = hold_artifacts()
    try:
        activate(fenced)
        wait(lambda: job(fenced)['state'] == 'running', 'claimed fenced job')
        sql(f"UPDATE jobs SET lease_owner=gen_random_uuid(),lease_until=now()+interval '1 hour' WHERE id='{fenced}';")
    finally:
        blocker.communicate('ROLLBACK;\n', timeout=10)
        assert blocker.returncode == 0
    time.sleep(.5)
    assert job(fenced)['scanned'] == 0 and job(fenced)['state'] == 'running'
    sql(f"UPDATE jobs SET lease_until=now()-interval '1 second' WHERE id='{fenced}';")
    assert terminal(fenced)['attempt'] == 2
    generation_job = enqueue()['jobId']
    blocker = hold_artifacts()
    try:
        activate(generation_job)
        wait(lambda: job(generation_job)['state'] == 'running', 'claimed generation job')
        sql('UPDATE instance_settings SET generation=generation+1;')
    finally:
        blocker.communicate('ROLLBACK;\n', timeout=10)
        assert blocker.returncode == 0
    assert terminal(generation_job, 'failed')['errorCode'] == 'generation_changed'
    assert job(generation_job)['scanned'] == 0
    print('PASS replaced lease and changed generation fence work already in flight', flush=True)

    # An interrupted transaction must not advance a checkpoint past stored data.
    sql(f"DELETE FROM events WHERE project_id='{project}' AND release='durable@1';")
    events(250)
    sql("CREATE FUNCTION slow_job_event() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_sleep(0.02); RETURN NEW; END $$; CREATE TRIGGER slow_job_event BEFORE UPDATE ON events FOR EACH ROW EXECUTE FUNCTION slow_job_event();")
    crash = enqueue()['jobId']; activate(crash)
    progress = wait(lambda: (row if 25 <= (row := job(crash))['scanned'] < 250 else None), 'committed first batch')
    subprocess.run(['docker', 'kill', runtime()[0]], check=True, stdout=subprocess.DEVNULL)
    committed = int(sql(f"SELECT scanned FROM jobs WHERE id='{crash}';"))
    rewritten = int(sql(f"SELECT count(*) FROM events WHERE issue_id='{issue}' AND payload#>>'{{error,stack,0,file}}'='src/checkout.ts';"))
    assert committed == rewritten and committed >= progress['scanned'], (committed, rewritten, progress)
    restart(); owner = login()
    sql(f"UPDATE jobs SET lease_until=now()-interval '1 second' WHERE id='{crash}';")
    result = terminal(crash)
    assert result['attempt'] == 2 and result['scanned'] == result['rewritten'] == 250, result
    print('PASS SIGKILL recovery resumes committed cursor and payloads under a fresh lease', flush=True)

    sql(f"UPDATE events SET payload='{raw_payload}'::jsonb WHERE issue_id='{issue}';")
    cancelled = enqueue()['jobId']; activate(cancelled)
    wait(lambda: job(cancelled)['scanned'] >= 25, 'running cancellation checkpoint')
    call(path + '/jobs/' + cancelled + '/cancel', 'POST', {})
    result = terminal(cancelled, 'cancelled')
    count = sql(f"SELECT count(*) FROM events WHERE issue_id='{issue}' AND payload#>>'{{error,stack,0,file}}'='src/checkout.ts';")
    time.sleep(.5)
    assert sql(f"SELECT count(*) FROM events WHERE issue_id='{issue}' AND payload#>>'{{error,stack,0,file}}'='src/checkout.ts';") == count
    assert 0 < result['scanned'] < 250, result
    sql('DROP TRIGGER slow_job_event ON events; DROP FUNCTION slow_job_event();')
    print('PASS running cancellation stops future payload writes at a checkpoint', flush=True)

    sql(f"UPDATE events SET payload='{raw_payload}'::jsonb WHERE issue_id='{issue}';")
    sql("CREATE FUNCTION fail_job_event() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected event rewrite failure'; END $$; CREATE TRIGGER fail_job_event BEFORE UPDATE ON events FOR EACH ROW EXECUTE FUNCTION fail_job_event();")
    retry = enqueue()['jobId']; activate(retry)
    wait(lambda: job(retry)['state'] == 'retry_wait', 'bounded retry')
    assert job(retry)['scanned'] == 0
    sql(f"UPDATE jobs SET attempt=max_attempts-1,next_attempt_at=now() WHERE id='{retry}';")
    assert terminal(retry, 'failed')['errorCode'] == 'processing_failed'
    sql('DROP TRIGGER fail_job_event ON events; DROP FUNCTION fail_job_event();')
    expired = enqueue()['jobId']
    sql(f"UPDATE jobs SET state='running',lease_owner=gen_random_uuid(),lease_until=now()-interval '1 second',attempt=max_attempts WHERE id='{expired}';")
    assert terminal(expired, 'failed')['errorCode'] == 'attempts_exhausted'
    print('PASS atomic batch rollback, retry ceiling and exhausted crashed lease', flush=True)

    stale = enqueue()['jobId']
    sql('UPDATE instance_settings SET generation=generation+1;')
    assert terminal(stale, 'failed')['errorCode'] == 'generation_changed'
    assert job(stale)['scanned'] == 0
    enqueue('same-request', status=409)
    # Queue capacity is enforced inside the same project lock as insertion.
    sql(f"INSERT INTO jobs(id,project_id,release_id,generation,kind) SELECT gen_random_uuid(),'{project}','{release}',(SELECT generation FROM instance_settings),'resymbolicate' FROM generate_series(1,256);")
    enqueue(status=503)
    upload(503)
    assert len(call(path + '/jobs?limit=999')['jobs']) == 100
    assert len(call(path + '/jobs?releaseId=' + other_release)['jobs']) == 0
    sql(f"DELETE FROM jobs WHERE project_id='{project}' AND state='queued';")
    sql('DROP TRIGGER delay_jobs ON jobs; DROP FUNCTION delay_jobs();')
    print('PASS generation fence, bounded queue and filtered bounded listing', flush=True)
