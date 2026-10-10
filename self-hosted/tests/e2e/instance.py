#!/usr/bin/env python3
"""Real instance API and migration checks. Pass an already built Linux binary.

python3 self-hosted/tests/e2e/instance.py --binary self-hosted/server/target/debug/sentori-server
Containers, network and volumes belong to this invocation and are always removed.
"""
import argparse
import concurrent.futures
import hashlib
import http.cookiejar
import importlib.util
import json
import os
import pathlib
import subprocess
import time
import urllib.error
import urllib.request
import uuid


def docker(*args, input=None):
    return subprocess.check_output(['docker', *args], input=input, text=True).strip()


def client():
    return urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))


def request(http, base, path, method='GET', data=None, status=200):
    req = urllib.request.Request(base + path, method=method,
        data=None if data is None else json.dumps(data).encode(),
        headers={'Content-Type': 'application/json'})
    try:
        response = http.open(req, timeout=15)
    except urllib.error.HTTPError as failure:
        response = failure
    body = response.read().decode()
    assert response.code == status, (method, path, response.code, status, body)
    return json.loads(body) if response.headers.get_content_type() == "application/json" else body


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True, type=pathlib.Path)
    parser.add_argument('--runtime-image', default='ubuntu:24.04')
    parser.add_argument('--runtime-platform', choices=['linux/amd64', 'linux/arm64'])
    parser.add_argument('--browser-test', type=pathlib.Path, help='Optional Node browser test against the temporary instance')
    parser.add_argument('--domain-test', type=pathlib.Path, help='Optional domain API checks using this disposable fixture')
    parser.add_argument('--mailbox', action='store_true', help='Capture recovery emails in a disposable SMTP sink')
    parser.add_argument('--ca-bundle', type=pathlib.Path, help='PEM trust roots for the minimal runtime image')
    args = parser.parse_args()
    ca_bundle = args.ca_bundle or next((p for p in [pathlib.Path('/etc/ssl/certs/ca-certificates.crt'), pathlib.Path('/etc/ssl/cert.pem')] if p.is_file()), None)
    binary = args.binary.resolve(strict=True)
    with binary.open('rb') as handle:
        header = handle.read(20)
    runtime_platform = args.runtime_platform or {62: 'linux/amd64', 183: 'linux/arm64'}.get(int.from_bytes(header[18:20], 'little'))
    if header[:4] != b'\x7fELF' or not runtime_platform:
        parser.error('Expected an x86_64 or aarch64 Linux binary')
    prefix = 'sentori-instance-' + uuid.uuid4().hex[:12]
    db, app = prefix + '-db', prefix + '-app'
    containers = []
    volume = prefix + '-blobs'
    root = pathlib.Path(__file__).resolve().parents[3]
    password = 'instance-test-' + uuid.uuid4().hex
    base = None
    mailbox = None
    if args.mailbox:
        from fixture_mailbox import Mailbox
        mailbox = Mailbox()

    def sql(query, database='fresh'):
        return docker('exec', '-i', db, 'psql', '-v', 'ON_ERROR_STOP=1', '-U', 'sentori', '-d', database, '-qtA', input=query)

    def start(database='fresh', storage=None, secret_files=False):
        nonlocal base
        if app in containers:
            docker('rm', '-f', app)
            containers.remove(app)
        docker('run', '-d', '--platform', runtime_platform, '--name', app, '--network', prefix, '-p', '127.0.0.1::8080',
            '-v', str(binary) + ':/sentori:ro', '-v', volume + ':/data',
            '-e', 'DATABASE_URL=postgres://sentori:test@' + db + '/' + database,
            '-e', 'SENTORI_OWNER_EMAIL=owner@example.test', '-e', 'SENTORI_OWNER_PASSWORD=' + password,
            '-e', 'SENTORI_BASE_URL=http://localhost:8080',
            '-e', 'SENTORI_MASTER_KEY=' + ('' if secret_files else '71' * 32),
            *(['-e','SENTORI_MASTER_KEY_FILE=/data/instance-master.key'] if secret_files else []),
            '-e', 'SENTORI_BACKUP_DIR=/data/backups',
            *(['-v', str(ca_bundle.resolve()) + ':/etc/ssl/certs/ca-certificates.crt:ro',
               '-e', 'SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt'] if ca_bundle else []),
            '-e', 'SENTORI_ATTACHMENT_STORE=' + (storage or 'fs:/data/blobs'),
            *(['--add-host', 'host.docker.internal:host-gateway',
               '-e', 'SENTORI_SMTP_HOST=host.docker.internal', '-e', 'SENTORI_SMTP_PORT=' + str(mailbox.port),
               '-e', 'SENTORI_SMTP_TLS=plain', '-e', 'SENTORI_SMTP_FROM=sentori@example.test'] if mailbox else []),
            '--entrypoint', '/sentori', args.runtime_image)
        containers.append(app)
        port = docker('port', app, '8080/tcp').split(':')[-1]
        base = 'http://127.0.0.1:' + port
        for _ in range(60):
            try:
                request(client(), base, '/healthz')
                return
            except (OSError, AssertionError):
                time.sleep(.5)
        raise AssertionError('server failed to become healthy: ' + docker('logs', app))

    def login(email='owner@example.test'):
        http = client()
        request(http, base, '/auth/login', 'POST', {'email': email, 'password': password})
        return http

    try:
        docker('network', 'create', prefix)
        docker('volume', 'create', volume)
        docker('run', '-d', '--name', db, '--network', prefix, '-e', 'POSTGRES_USER=sentori',
            '-e', 'POSTGRES_PASSWORD=test', '-e', 'POSTGRES_DB=fresh', 'postgres:18-alpine')
        containers.append(db)
        for _ in range(60):
            if subprocess.run(['docker', 'exec', db, 'pg_isready', '-h', '127.0.0.1', '-U', 'sentori'], capture_output=True).returncode == 0:
                break
            time.sleep(.5)
        start()
        owner = login()
        endpoint = '/admin/api/instance'
        request(client(), base, endpoint, status=401)
        request(client(), base, '/admin/api/system', status=401)
        initial = request(owner, base, endpoint)
        assert initial['initializedAt'] is None
        identity = initial['id']
        request(owner, base, endpoint + '/initialize', 'POST', {'expectedVersion': 1}, status=409)
        request(owner, base, '/admin/api/users', 'POST', {'email': 'member@example.test', 'password': password}, status=201)
        member = login('member@example.test')
        assert request(member, base, endpoint)['id'] == identity
        assert not request(member, base, '/admin/api/system')['capabilities']['instance.configure']['allowed']
        for path, method, data in [(endpoint, 'PATCH', {'name': 'stolen', 'expectedVersion': 1}),
                                  (endpoint + '/check', 'POST', {}),
                                  (endpoint + '/initialize', 'POST', {'expectedVersion': 1})]:
            request(member, base, path, method, data, status=403)
        for invalid in ['', ' ', 'x' * 81]:
            request(owner, base, endpoint, 'PATCH', {'name': invalid, 'expectedVersion': 1}, status=400)
        request(owner, base, endpoint, 'PATCH', {'name': 'x', 'expectedVersion': 1, 'origin': 'https://bad.test'}, status=422)
        proof = request(owner, base, endpoint + '/check', 'POST', {})
        assert all(proof[key] for key in ['database', 'blobStorage', 'durableStorage', 'originValid'])
        # Same version races must yield one winner and one conflict, never two writes.
        def change(name):
            try:
                return request(owner, base, endpoint, 'PATCH', {'name': name, 'expectedVersion': 1})
            except AssertionError as failure:
                assert failure.args[0][2] == 409, failure
                return None
        with concurrent.futures.ThreadPoolExecutor(2) as pool:
            results = list(pool.map(change, ['Team A', 'Team B']))
        assert sum(value is not None for value in results) == 1
        current = request(owner, base, endpoint)
        assert current['version'] == 2 and current['lastCheck'] is None
        request(owner, base, endpoint + '/initialize', 'POST', {'expectedVersion': 2}, status=409)
        request(owner, base, endpoint + '/check', 'POST', {})
        sql("UPDATE instance_settings SET checked_at=now()-interval '11 minutes';")
        request(owner, base, endpoint + '/initialize', 'POST', {'expectedVersion': 2}, status=409)
        request(owner, base, endpoint + '/check', 'POST', {})
        ready = request(owner, base, endpoint + '/initialize', 'POST', {'expectedVersion': 2})
        assert ready['initializedAt'] and ready['id'] == identity
        request(owner, base, endpoint + '/initialize', 'POST', {'expectedVersion': 2})
        assert sql("SELECT count(*) FROM audit_logs WHERE action='instance.configure';") == '1'
        assert sql("SELECT count(*) FROM audit_logs WHERE action='instance.initialized';") == '1'
        if args.domain_test:
            spec = importlib.util.spec_from_file_location('sentori_domain_test', args.domain_test.resolve())
            domain = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(domain)
            domain.run(base=base, owner=owner, sql=sql, password=password, client=client, request=request,
                **({"restart": start, "login": login, "runtime": lambda: (app, base)} if getattr(domain, "NEEDS_RUNTIME", False) else {}))
        if args.browser_test:
            subprocess.run(['node', str(args.browser_test.resolve())], check=True, env={**os.environ, 'SENTORI_TEST_API': base, 'SENTORI_TEST_PASSWORD': password, **({'SENTORI_TEST_MAILBOX': mailbox.url} if mailbox else {})})
        start()
        assert request(login(), base, endpoint)['id'] == identity
        # A fresh instance using volatile storage cannot complete initialization.
        sql('CREATE DATABASE volatile;', 'postgres')
        start('volatile', 'memory')
        owner = login()
        assert not request(owner, base, endpoint + '/check', 'POST', {})['durableStorage']
        request(owner, base, endpoint + '/initialize', 'POST', {'expectedVersion': 1}, status=409)
        # Reconstruct the exact v4 schema/checksums before running the new migration.
        sql('CREATE DATABASE upgraded;', 'postgres')
        sql('CREATE TABLE _sqlx_migrations (version BIGINT PRIMARY KEY, description TEXT NOT NULL, installed_on TIMESTAMPTZ NOT NULL DEFAULT now(), success BOOLEAN NOT NULL, checksum BYTEA NOT NULL, execution_time BIGINT NOT NULL);', 'upgraded')
        for migration in sorted((root / 'core/migrations').glob('*.sql')):
            version, description = migration.stem.split('_', 1)
            if int(version) >= 22:
                continue
            content = migration.read_bytes()
            sql(content.decode(), 'upgraded')
            checksum = hashlib.sha384(content).hexdigest()
            sql(f"INSERT INTO _sqlx_migrations(version,description,success,checksum,execution_time) VALUES ({int(version)},'{description.replace('_', ' ')}',true,decode('{checksum}','hex'),0);", 'upgraded')
        original = sql("SELECT row_to_json(u) FROM users u WHERE email='owner@example.test';")
        user = json.loads(original)
        sql("INSERT INTO users(id,email,password_hash,role) VALUES ('" + user['id'] + "','owner@example.test','" + user['password_hash'] + "','superadmin');", 'upgraded')
        project_id, issue_id, event_id, token_id = [str(uuid.uuid4()) for _ in range(4)]
        sql(f"INSERT INTO projects(id,name,platform) VALUES ('{project_id}','Preserved v4 project','javascript');", 'upgraded')
        sql(f"INSERT INTO tokens(id,project_id,name,scope,token_hash) VALUES ('{token_id}','{project_id}','Existing SDK key','ingest','preserved-hash');", 'upgraded')
        sql(f"INSERT INTO issues(id,project_id,fingerprint,kind,group_title,first_seen,last_seen) VALUES ('{issue_id}','{project_id}','preserved-fingerprint','error','Preserved error',now(),now());", 'upgraded')
        sql(f"INSERT INTO events(id,project_id,issue_id,kind,platform,occurred_at,payload) VALUES ('{event_id}','{project_id}','{issue_id}','error','javascript',now(),'{{\"message\":\"preserved event\"}}');", 'upgraded')
        def contents(table):
            projection = "to_jsonb(row) - 'version' - 'webhook_secret_blob' - 'migration_fence'" if table == 'projects' else 'to_jsonb(row)'
            return sql(f"SELECT {projection} FROM (SELECT * FROM {table} ORDER BY id) row;", 'upgraded')
        before = {table: contents(table) for table in ['projects', 'tokens', 'issues', 'events']}
        start('upgraded')
        upgraded = request(login(), base, endpoint)
        assert upgraded['initializedAt'] and upgraded['id'] != identity
        assert {table: contents(table) for table in before} == before
        assert sql("SELECT version FROM projects WHERE id='" + project_id + "';", 'upgraded') == '1'
        assert sql("SELECT id FROM users WHERE email='owner@example.test';", 'upgraded') == user['id']
        print('PASS: fresh setup, authenticated reads, owner-only writes, strict input, concurrent conflict, check invalidation/expiry, idempotent initialization, atomic audit, restart identity, volatile storage refusal, v4 populated migration')
    finally:
        if mailbox:
            mailbox.close()
        for container in reversed(containers):
            subprocess.run(['docker', 'rm', '-f', container], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        subprocess.run(['docker', 'volume', 'rm', volume], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        subprocess.run(['docker', 'network', 'rm', prefix], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


if __name__ == '__main__':
    main()
