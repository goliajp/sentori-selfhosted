#!/usr/bin/env python3
"""Actual v4 binary -> v5 upgrade, v5 quarantine backup, then v4 backup rollback.

Every database/container/volume is disposable. Rollback restores the pre-upgrade
DB+blob copy, while a verified v5 recovery archive preserves post-upgrade writes.
"""
import argparse
import datetime
import hashlib
import http.cookiejar
import json
import pathlib
import subprocess
import tempfile
import time
import urllib.error
import urllib.request
import uuid


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--v4-binary', required=True, type=pathlib.Path)
    parser.add_argument('--v5-binary', required=True, type=pathlib.Path)
    parser.add_argument('--operator-binary', required=True, type=pathlib.Path)
    parser.add_argument('--expected-version', required=True)
    args = parser.parse_args()
    binaries = [str(value.resolve(strict=True)) for value in [args.v4_binary, args.v5_binary, args.operator_binary]]
    prefix = 'sentori-upgrade-' + uuid.uuid4().hex[:12]
    db, app = prefix+'-db', prefix+'-app'
    live_volume, rollback_volume = prefix+'-live', prefix+'-rollback'
    password = 'upgrade-fixture-' + uuid.uuid4().hex
    containers, volumes = [], []
    master = '6d'*32
    base = None

    def docker(*arguments, **kwargs):
        return subprocess.check_output(['docker', *arguments], **kwargs)

    def sql(query, database='live'):
        return docker('exec', '-i', db, 'psql', '-U', 'sentori', '-d', database, '-v', 'ON_ERROR_STOP=1', '-qtA', input=query, text=True).strip()

    def client():
        return urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))

    def call(http, path, data=None, status=200, token=None, headers=None):
        payload = data if isinstance(data, bytes) else json.dumps(data).encode() if data is not None else None
        req = urllib.request.Request(base+path, data=payload, headers={**({'Content-Type':'application/json'} if payload is not None else {}), **({'Authorization':'Bearer '+token} if token else {}), **(headers or {})})
        try:
            response = http.open(req, timeout=30)
        except urllib.error.HTTPError as error:
            response = error
        value = response.read()
        assert response.code == status, (path, response.code, status, value[:1500])
        return json.loads(value) if response.headers.get_content_type() == 'application/json' else value

    def stop():
        if app in containers:
            docker('rm', '-f','-v', app)
            containers.remove(app)

    def start(binary, database='live', volume=live_volume, blob_dir='/data/blobs', expected=None):
        nonlocal base
        stop()
        docker('run', '-d', '--name', app, '--network', prefix, '-p', '127.0.0.1::8080',
               '-v', binary+':/sentori:ro', '-v', volume+':/data',
               '-e', 'DATABASE_URL=postgres://sentori:test@'+db+'/'+database,
               '-e', 'SENTORI_OWNER_EMAIL=owner@example.test', '-e', 'SENTORI_OWNER_PASSWORD='+password,
               '-e', 'SENTORI_MASTER_KEY='+master, '-e', 'SENTORI_ATTACHMENT_STORE=fs:'+blob_dir,
               '--entrypoint', '/sentori', 'ubuntu:24.04')
        containers.append(app)
        base = 'http://127.0.0.1:'+docker('port', app, '8080/tcp', text=True).strip().split(':')[-1]
        for _ in range(100):
            try:
                health = call(client(), '/healthz')
                if expected:
                    assert health['version'] == expected, health
                return
            except (OSError, AssertionError):
                if docker('inspect', '-f', '{{.State.Running}}', app, text=True).strip() == 'false':
                    break
                time.sleep(.2)
        raise AssertionError('server not healthy: '+docker('logs', app, text=True)[-5000:])

    def login():
        http = client()
        call(http, '/auth/login', {'email':'owner@example.test','password':password})
        return http

    def event(message):
        return {'id':str(uuid.uuid4()), 'kind':'error', 'occurredAt':datetime.datetime.now(datetime.timezone.utc).isoformat(), 'platform':'javascript', 'release':'upgrade@1', 'payload':{'error':{'type':'UpgradeError','message':message}}}

    def invoke(*arguments, database='live'):
        result = subprocess.run(['docker','run','--rm','--network',prefix,'-v',live_volume+':/data','-v',binaries[2]+':/operator:ro',
                                 '-e','DATABASE_URL=postgres://sentori:test@'+db+'/'+database,'-e','SENTORI_MASTER_KEY='+master,
                                 '--entrypoint','/operator','ubuntu:24.04',*arguments], capture_output=True, text=True)
        assert result.returncode == 0, (arguments, result.stderr)
        return result.stdout

    try:
        docker('network', 'create', prefix)
        for volume in [live_volume, rollback_volume]:
            docker('volume', 'create', volume)
            volumes.append(volume)
        docker('run','-d','--name',db,'--network',prefix,'-e','POSTGRES_USER=sentori','-e','POSTGRES_PASSWORD=test','-e','POSTGRES_DB=live','postgres:18-alpine')
        containers.append(db)
        for _ in range(100):
            if subprocess.run(['docker','exec',db,'pg_isready','-h','127.0.0.1','-U','sentori'],capture_output=True).returncode == 0:
                break
            time.sleep(.2)
        start(binaries[0], expected='4.1.1')
        owner = login()
        project = call(owner, '/admin/api/projects', {'name':'Preserved v4 / 既有项目'}, 201)['id']
        token = call(owner, '/admin/api/projects/'+project+'/tokens', {'name':'Existing SDK', 'scope':'ingest'}, 201)['token']
        old = event('Before major upgrade')
        call(owner, '/v1/events', old, 202, token)
        replay = b'{"kind":"keyframe","at":0,"width":320,"height":640,"nodes":[]}\n'
        boundary = 'sentori-upgrade-fixture'
        payload = (f'--{boundary}\r\nContent-Disposition: form-data; name="source"\r\n\r\njs\r\n--{boundary}\r\nContent-Disposition: form-data; name="file"; filename="replay.ndjson"\r\nContent-Type: application/x-ndjson\r\n\r\n').encode()+replay+f'\r\n--{boundary}--\r\n'.encode()
        attached = call(owner, '/v1/events/'+old['id']+'/attachments/replay', payload, 202, token, {'Content-Type':'multipart/form-data; boundary='+boundary})
        reference = attached['refId']
        assert call(owner, '/admin/api/attachments/'+reference) == replay
        tables = ['users','projects','tokens','issues','events','event_attachments','_sqlx_migrations']
        # Remember v4's actual columns; new defaulted compatibility columns are
        # checked by the other migration tests without changing this projection.
        columns = {table:sql("SELECT string_agg(quote_ident(attname),',' ORDER BY attnum) FROM pg_attribute WHERE attrelid='"+table+"'::regclass AND attnum>0 AND NOT attisdropped;") for table in tables}
        def projection(table, database='live'):
            return json.loads(sql("SELECT coalesce(jsonb_agg(to_jsonb(row) ORDER BY to_jsonb(row)::text),'[]'::jsonb) FROM (SELECT "+columns[table]+" FROM "+table+") row;", database))
        before = {table:projection(table) for table in tables}
        assert len(before['events']) == 1 and len(before['_sqlx_migrations']) == 21
        stop()
        # This is a paired offline v4 DB + blob backup, not a database-only copy.
        dump = docker('exec',db,'pg_dump','-U','sentori','-d','live','-Fc')
        assert dump.startswith(b'PGDMP')
        docker('run','--rm','-v',live_volume+':/source:ro','-v',rollback_volume+':/restore','ubuntu:24.04','cp','-a','/source/blobs','/restore/blobs')
        with tempfile.TemporaryDirectory(prefix='sentori-v4-rollback-') as temporary:
            backup = pathlib.Path(temporary)/'before-v5.dump'
            backup.write_bytes(dump)
            backup.chmod(0o600)
            assert hashlib.sha256(backup.read_bytes()).digest() == hashlib.sha256(dump).digest()
            start(binaries[1], expected=args.expected_version)
            for table in tables[:-1]:
                assert projection(table) == before[table], 'v4 row drift: '+table
            owner = login()
            assert int(sql('SELECT max(version) FROM _sqlx_migrations;')) >= 35
            assert call(owner, '/admin/api/attachments/'+reference) == replay
            assert call(owner, '/admin/api/events/'+old['id'])['id'] == old['id']
            after = event('Accepted after v5 upgrade')
            call(owner, '/v1/events', after, 202, token)
            assert sql("SELECT count(*) FROM events WHERE id='"+after['id']+"';") == '1'
            print('PASS actual v4.1.1 binary -> current upgrade: credentials, projects, issue/event rows, attachment bytes and existing SDK token preserved', flush=True)
            identity = call(owner, '/admin/api/instance')
            transfer = str(uuid.uuid4())
            endpoint = '/admin/api/transfers'
            call(owner, endpoint, dict(commandId=str(uuid.uuid4()), id=transfer, direction='source', instanceId=identity['id'], generation=identity['generation'], sourceInstanceId=identity['id'], sourceGeneration=identity['generation'], targetInstanceId=str(uuid.uuid4()), targetGeneration=1, projectIds=[project]))
            def transfer_command(action, **extra):
                row = next(item for item in call(owner, endpoint)['items'] if item['id'] == transfer)
                return call(owner, endpoint+'/'+transfer+'/commands', dict(commandId=str(uuid.uuid4()), instanceId=identity['id'], generation=identity['generation'], version=row['version'], action=action, **extra))
            initial_job = transfer_command('export_copy', recoveryKey='92'*32)['jobId']
            for _ in range(300):
                state = sql("SELECT state FROM project_transfer_jobs WHERE id='"+initial_job+"';")
                if state == 'ready':
                    break
                assert state in ['queued','running'], state
                time.sleep(.1)
            else:
                raise AssertionError('initial migration export timed out')
            transfer_command('seal', rollbackPlan='Preserve all new v5 events in an encrypted archive and quarantine them before restoring the v4 database and blob snapshot.')
            guard = subprocess.Popen(['docker','exec','-i',db,'psql','-U','sentori','-d','live','-v','ON_ERROR_STOP=1','-qtA'], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
            guard.stdin.write("SET idle_in_transaction_session_timeout='90s'; BEGIN; DO $$ BEGIN PERFORM pg_advisory_xact_lock(6000288202639953491); END $$; SELECT 'held';\n")
            guard.stdin.flush()
            assert guard.stdout.readline().strip() == 'held'
            invoke('keygen','--out','/data/upgrade-recovery.key')
            try:
                pending_job = transfer_command('export_final', recoveryKey='92'*32)['jobId']
                assert sql("SELECT state='queued' AND key_blob IS NOT NULL FROM project_transfer_jobs WHERE id='"+pending_job+"';") == 't'
                manifest = json.loads(invoke('dump','--out','/data/after-upgrade.sentori','--blob-dir','/data/blobs','--key-file','/data/upgrade-recovery.key'))
            finally:
                if guard.poll() is None:
                    guard.stdin.write('ROLLBACK;\n'); guard.stdin.flush(); guard.stdin.close(); guard.wait(timeout=10)
            assert manifest['tables']['project_transfers']['rows'] == 1
            assert manifest['tables']['project_transfer_jobs']['rows'] == 2

            assert manifest['tables']['events']['rows'] == 2
            assert json.loads(invoke('verify','--from','/data/after-upgrade.sentori','--key-file','/data/upgrade-recovery.key')) == manifest
            stop()
            sql('CREATE DATABASE quarantine;', 'postgres')
            invoke('migrate', database='quarantine')
            invoke('restore','--from','/data/after-upgrade.sentori','--key-file','/data/upgrade-recovery.key','--blob-dir','/data/quarantine',database='quarantine')
            start(binaries[1], database='quarantine', blob_dir='/data/quarantine', expected=args.expected_version)
            restored = login()
            assert call(restored, '/admin/api/instance')['operatingMode'] == 'restore_validation'
            assert sql("SELECT state FROM project_transfers WHERE id='"+transfer+"';", 'quarantine') == 'cancelled'
            assert sql("SELECT count(*) FROM project_transfer_jobs WHERE transfer_id='"+transfer+"' AND state='unavailable' AND key_blob IS NULL;", 'quarantine') == '2'
            assert sql("SELECT migration_fence IS NULL FROM projects WHERE id='"+project+"';", 'quarantine') == 't'
            assert sql('SELECT count(*) FROM project_transfer_routes;', 'quarantine') == '0'

            assert call(restored, '/admin/api/events/'+after['id'])['id'] == after['id']
            assert call(restored, '/admin/api/attachments/'+reference) == replay
            stop()
            print('PASS post-upgrade writes preserved in independently verified v5 archive and quarantined restore; sealed transfer cancelled, fenced project released and queued key erased', flush=True)
            sql('CREATE DATABASE rollback;', 'postgres')
            docker('exec','-i',db,'pg_restore','-U','sentori','-d','rollback','--exit-on-error','--no-owner',input=backup.read_bytes())
            for table in tables:
                assert projection(table,'rollback') == before[table], 'restored v4 row drift: '+table
            assert sql("SELECT count(*) FROM events WHERE id='"+after['id']+"';",'rollback') == '0'
            assert sql("SELECT count(*) FROM events WHERE id='"+after['id']+"';",'quarantine') == '1'
            start(binaries[0], database='rollback', volume=rollback_volume, expected='4.1.1')
            rolled_back = login()
            assert call(rolled_back, '/admin/api/attachments/'+reference) == replay
            assert call(rolled_back, '/admin/api/events/'+old['id'])['id'] == old['id']
            call(rolled_back, '/v1/events', event('Accepted after verified v4 rollback'), 202, token)
            assert sql('SELECT count(*) FROM events;','rollback') == '2'
            assert sql("SELECT count(*) FROM events WHERE id='"+after['id']+"';",'quarantine') == '1'
            print('PASS actual v4 rollback from paired database/blob backup: exact v4 schema, login, replay bytes and new SDK event; v5-only data remains in quarantine', flush=True)
    finally:
        for name in reversed(containers):
            subprocess.run(['docker','rm','-f','-v',name],capture_output=True)
        for name in volumes:
            subprocess.run(['docker','volume','rm','-f',name],capture_output=True)
        subprocess.run(['docker','network','rm',prefix],capture_output=True)


if __name__ == '__main__':
    main()
