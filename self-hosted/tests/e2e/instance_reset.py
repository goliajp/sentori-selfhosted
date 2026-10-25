"""Real owner authorization, backup-first offline reset and interrupted cleanup."""
import base64
import concurrent.futures
import hashlib
import json
import os
import pathlib
import subprocess
import time
import uuid

NEEDS_RUNTIME = True


def run(*, base, owner, sql, password, client, request, restart, login, runtime):
    app, _ = runtime()
    cli = pathlib.Path(os.environ['SENTORI_TEST_CLI']).resolve(strict=True)
    inspect = json.loads(subprocess.check_output(['docker', 'inspect', app], text=True))[0]
    network = next(iter(inspect['NetworkSettings']['Networks']))
    db_url = next(e.split('=', 1)[1] for e in inspect['Config']['Env'] if e.startswith('DATABASE_URL='))
    db_container = db_url.split('@', 1)[1].split('/', 1)[0]
    recovery_volume = 'sentori-reset-recovery-' + uuid.uuid4().hex[:12]
    docker = lambda *args: subprocess.check_output(['docker', *args], text=True).strip()
    docker('volume', 'create', recovery_volume)

    def call(path, method='GET', data=None, status=200, http=None):
        return request(http or owner, runtime()[1], path, method, data, status)

    def shell(script):
        return docker('run', '--rm', '--volumes-from', app, '-v', recovery_volume + ':/recovery',
                      '--entrypoint', 'sh', 'ubuntu:24.04', '-c', script)

    def write(path, content):
        shell('printf %s ' + base64.b64encode(content).decode() + ' | base64 -d > ' + path)

    def command(*args, database='fresh', name=None):
        return ['docker', 'run', '--rm', '--user', '65532:65532', '--network', network, '--volumes-from', app,
                '-v', str(cli) + ':/cli:ro', '-v', recovery_volume + ':/recovery',
                '-e', 'DATABASE_URL=' + db_url.rsplit('/', 1)[0] + '/' + database,
                '-e', 'SENTORI_MASTER_KEY=' + '71' * 32,
                *(['--name', name] if name else []), '--entrypoint', '/cli', 'ubuntu:24.04', *args]

    def invoke(*args, ok=True, **kw):
        result = subprocess.run(command(*args, **kw), capture_output=True, text=True, timeout=90)
        assert (result.returncode == 0) == ok, (args, result.returncode, result.stderr)
        return result.stdout if ok else result.stderr

    endpoint = '/admin/api/instance/reset/prepare'
    instance = call('/admin/api/instance')
    body = dict(instanceId=instance['id'], instanceName=instance['name'], generation=instance['generation'], currentPassword=password)
    reset = lambda tag: ['reset', '--permit-file', '/data/reset.permit', '--instance-id', instance['id'],
                        '--instance-name', instance['name'], '--generation', str(instance['generation']),
                        '--blob-dir', '/data/blobs', '--backup', f'/recovery/{tag}.sentori',
                        '--journal', f'/recovery/{tag}.json', '--key-file', '/data/reset.key', '--confirm-independent-storage']
    cli_container = 'sentori-reset-command-' + uuid.uuid4().hex[:12]
    holder = None
    try:
        shell('chown 65532:65532 /data /recovery')
        call(endpoint, 'POST', body, 401, client())
        call(endpoint, 'POST', body, 403, login('member@example.test'))
        assert not call('/admin/api/system', http=login('member@example.test'))['capabilities']['instance.reset']['allowed']
        for patch, status in [({'currentPassword': 'wrong'}, 400), ({'instanceName': 'wrong'}, 409),
                              ({'instanceId': str(uuid.uuid4())}, 409), ({'generation': instance['generation'] + 1}, 409)]:
            call(endpoint, 'POST', {**body, **patch}, status)
        assert sql('SELECT count(*) FROM instance_reset_intents;') == '0'
        permit = call(endpoint, 'POST', body)
        assert len(permit['permit']) == 64
        assert sql("SELECT permit_hash FROM instance_reset_intents;") == hashlib.sha256(permit['permit'].encode()).hexdigest()
        write('/data/reset.permit', permit['permit'].encode())
        invoke('keygen', '--out', '/data/reset.key')
        assert 'stop every server' in invoke(*reset('running'), ok=False)
        docker('stop', app)
        assert 'independent' in invoke(*reset('ack')[:-1], ok=False)
        sql("UPDATE instance_reset_intents SET expires_at=now()-interval '1 second';")
        assert 'expired' in invoke(*reset('expired'), ok=False)
        assert shell('test ! -e /recovery/expired.sentori && echo absent') == 'absent'
        restart(); owner = login()
        old = call(endpoint, 'POST', body)
        permit = call(endpoint, 'POST', body)
        assert sql('SELECT count(*) FROM instance_reset_intents;') == '1'
        write('/data/reset.permit', old['permit'].encode())
        docker('stop', app)
        assert 'revoked' in invoke(*reset('superseded'), ok=False)
        write('/data/reset.permit', permit['permit'].encode())
        sql("UPDATE instance_reset_intents SET password_fingerprint='changed';")
        assert 'revoked' in invoke(*reset('password'), ok=False)
        sql("UPDATE instance_reset_intents r SET password_fingerprint=encode(sha256(convert_to(u.password_hash,'UTF8')),'hex') FROM users u WHERE u.id=r.actor_user_id;")
        sql('DELETE FROM auth_sessions WHERE id_hash IN (SELECT session_hash FROM instance_reset_intents);')
        assert 'revoked' in invoke(*reset('logout'), ok=False)
        restart(); owner = login()
        project = call('/admin/api/projects', 'POST', {'name': 'Reset recovery evidence'}, 201)['id']
        token = call(f'/admin/api/projects/{project}/tokens', 'POST', dict(name='Reset SDK', scope='ingest'), 201)['token']
        sdk = client(); sdk.addheaders = [('Authorization', 'Bearer ' + token)]
        event = call('/v1/events', 'POST', dict(kind='error', occurredAt='2026-10-10T00:00:00Z', platform='javascript', payload={'message': 'Recover me after reset'}), 202, sdk)['eventId']
        replay = b'{"kind":"keyframe","at":0,"width":320,"height":640,"nodes":[]}\n'
        digest = hashlib.sha256(replay).hexdigest(); reference = str(uuid.uuid4())
        shell('mkdir -p /data/blobs/' + digest[:2]); write('/data/blobs/' + digest[:2] + '/' + digest[2:] + '.bin', replay)
        sql(f"INSERT INTO event_attachments(ref,project_id,event_id,kind,media_type,size_bytes,blob_hash,source,captured_at) VALUES('{reference}','{project}','{event}','replay','application/x-ndjson',{len(replay)},'{digest}','javascript',now());")
        permit = call(endpoint, 'POST', body); write('/data/reset.permit', permit['permit'].encode())
        docker('stop', app)
        shell('chown -R 65532:65532 /data/blobs')
        assert 'outside' in invoke(*[('/data/blobs/bad.sentori' if x == '/recovery/path.sentori' else x) for x in reset('path')], ok=False)
        write('/data/blobs/operator-note', b'preserve')
        assert 'unknown file' in invoke(*reset('unknown'), ok=False)
        shell('rm /data/blobs/operator-note')
        blob_path = '/data/blobs/' + digest[:2] + '/' + digest[2:] + '.bin'
        shell('mv ' + blob_path + ' /data/missing.bin')
        assert 'referenced blob missing' in invoke(*reset('missing'), ok=False)
        assert sql(f"SELECT count(*) FROM events WHERE id='{event}';") == '1'
        shell('mv /data/missing.bin ' + blob_path)
        print('PASS reset authorization, expiry, session/password revocation, running-server exclusion and backup/path fail-closed checks', flush=True)

        # A PostgreSQL barrier stops deletion only, after the fresh archive and
        # journal have been durably written. No production fault switch needed.
        lock = 8182753032
        sql(f"CREATE FUNCTION reset_test_barrier() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_advisory_xact_lock({lock}); RETURN NULL; END $$; CREATE TRIGGER reset_test_barrier BEFORE DELETE ON events FOR EACH STATEMENT EXECUTE FUNCTION reset_test_barrier();")
        def hold():
            nonlocal holder
            holder = subprocess.Popen(['docker', 'exec', '-i', db_container, 'psql', '-v', 'ON_ERROR_STOP=1', '-U', 'sentori', '-d', 'fresh', '-qtA'], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
            holder.stdin.write(f"SET idle_in_transaction_session_timeout='90s'; BEGIN; SELECT pg_advisory_xact_lock({lock}); SELECT 'held';\n"); holder.stdin.flush()
            while holder.stdout.readline().strip() != 'held':
                assert holder.poll() is None
        def release():
            nonlocal holder
            if holder and holder.poll() is None:
                holder.stdin.write('COMMIT;\n'); holder.stdin.flush(); holder.stdin.close(); holder.wait(timeout=10)
            holder = None
        def blocked():
            for _ in range(300):
                if sql("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname='fresh' AND query='DELETE FROM events' AND wait_event_type='Lock');") == 't': return
                time.sleep(.05)
            raise AssertionError('reset did not reach the deletion barrier')

        hold()
        process = subprocess.Popen(command(*reset('crashed'), name=cli_container), stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        try:
            blocked()
            assert shell('test -f /recovery/crashed.json && test -f /recovery/crashed.sentori && echo saved') == 'saved'
            docker('kill', cli_container)  # exact container created by this test
            process.communicate(timeout=10)
            assert process.returncode != 0
        finally:
            release()
            if process.poll() is None:
                docker('kill', cli_container); process.communicate(timeout=10)
        assert sql('SELECT reset_pending IS NULL FROM instance_settings;') == 't'
        assert sql(f"SELECT count(*) FROM events WHERE id='{event}';") == '1'
        assert shell('test -f ' + blob_path + ' && echo present') == 'present'
        print('PASS actual operator crash rolls database changes back while preserving independent archive and journal', flush=True)

        # Force an external receipt I/O failure after the database commits and
        # blob cleanup runs. Startup and ordinary operator paths must stay fenced.
        hold()
        with concurrent.futures.ThreadPoolExecutor(1) as executor:
            pending = executor.submit(invoke, *reset('interrupted'), ok=False)
            try:
                blocked(); shell('mkdir /recovery/interrupted.complete.json')
            finally: release()
            assert 'reset remains fenced' in pending.result(timeout=90)
        sql('DROP TRIGGER reset_test_barrier ON events; DROP FUNCTION reset_test_barrier();')
        assert sql('SELECT reset_pending IS NOT NULL FROM instance_settings;') == 't'
        assert sql('SELECT count(*) FROM users;') == '0'
        docker('start', app)
        for _ in range(60):
            if docker('inspect', '-f', '{{.State.Running}}', app) == 'false': break
            time.sleep(.1)
        else: raise AssertionError('incomplete reset server started')
        assert 'reset is incomplete' in subprocess.check_output(['docker', 'logs', app], text=True, stderr=subprocess.STDOUT)
        assert 'reset is incomplete' in invoke('migrate', ok=False)
        assert 'reset is incomplete' in invoke('import', ok=False)
        invoke('keygen', '--out', '/data/wrong-reset.key')
        invoke('finish-reset', '--journal', '/recovery/interrupted.json', '--key-file', '/data/wrong-reset.key', ok=False)
        assert sql('SELECT reset_pending IS NOT NULL FROM instance_settings;') == 't'
        shell('rmdir /recovery/interrupted.complete.json')
        journal = json.loads(invoke('finish-reset', '--journal', '/recovery/interrupted.json', '--key-file', '/data/reset.key'))
        assert journal['old_generation'] == instance['generation'] and journal['new_generation'] == instance['generation'] + 1
        invoke('finish-reset', '--journal', '/recovery/interrupted.json', '--key-file', '/data/reset.key')
        assert sql('SELECT reset_pending IS NULL FROM instance_settings;') == 't'
        assert sql('SELECT count(*) FROM projects;') == '0'
        assert sql('SELECT count(*) FROM auth_sessions;') == '0'
        for table in sql("SELECT tablename FROM pg_tables WHERE schemaname='public' AND tablename NOT IN ('_sqlx_migrations','instance_settings','audit_logs');").splitlines():
            assert sql('SELECT count(*) FROM "' + table.replace('"', '""') + '";') == '0', table
        assert sql("SELECT count(*) FROM audit_logs WHERE action='instance.reset.completed';") == '1'
        assert shell('find /data/blobs -type f | wc -l') == '0'
        manifest = json.loads(invoke('verify', '--from', '/recovery/interrupted.sentori', '--key-file', '/data/reset.key'))
        assert manifest['tables']['events']['rows'] >= 1 and manifest['blob_bytes'] >= len(replay)
        receipt = json.loads(shell('cat /recovery/interrupted.complete.json'))
        assert receipt['status'] == 'complete' and receipt['reset'] == journal
        restart(); owner = login()
        current = call('/admin/api/instance')
        assert current['id'] == instance['id'] and current['generation'] == instance['generation'] + 1 and current['initializedAt'] is None
        call('/v1/events', 'POST', {'kind': 'error', 'payload': {'message': 'old SDK must fail'}}, 401, sdk)
        print('PASS interrupted cleanup fences startup/operators, rejects wrong key, resumes idempotently and invalidates old sessions/tokens', flush=True)

        sql('CREATE DATABASE reset_recovery;', 'postgres')
        invoke('migrate', database='reset_recovery')
        invoke('restore', '--from', '/recovery/interrupted.sentori', '--key-file', '/data/reset.key', '--blob-dir', '/data/reset-restored', database='reset_recovery')
        assert sql('SELECT count(*) FROM instance_reset_intents;', 'reset_recovery') == '0'
        assert sql('SELECT reset_pending IS NULL FROM instance_settings;', 'reset_recovery') == 't'
        restart('reset_recovery', 'fs:/data/reset-restored'); owner = login()
        assert call('/admin/api/instance')['operatingMode'] == 'restore_validation'
        assert owner.open(runtime()[1] + '/admin/api/attachments/' + reference).read() == replay
        assert call('/admin/api/events/' + event)['projectId'] == project
        restart(); owner = login()
        print('PASS independently restored reset archive preserves actual event/replay evidence and never revives reset permits', flush=True)
    finally:
        if holder and holder.poll() is None:
            holder.stdin.write('ROLLBACK;\n'); holder.stdin.flush(); holder.stdin.close(); holder.wait(timeout=10)
        subprocess.run(['docker', 'rm', '-f','-v', cli_container], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        subprocess.run(['docker', 'volume', 'rm', recovery_volume], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
