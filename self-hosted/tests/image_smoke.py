#!/usr/bin/env python3
"""Verify the immutable image that deploy will promote, with no rebuild/mount override."""
import argparse
import hashlib
import http.cookiejar
import json
from pathlib import Path
import re
import signal
import subprocess
import sys
import time
import urllib.error
import urllib.request
import uuid


def main(argv=None):
    if sys.flags.optimize:
        raise SystemExit('assertions must be enabled')
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--image', required=True)
    parser.add_argument('--expected-version', required=True)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args(argv)
    if args.output.exists():
        parser.error('refusing to overwrite existing image evidence')

    def docker(*arguments):
        return subprocess.check_output(['docker', *arguments], text=True, stderr=subprocess.PIPE).strip()

    info = json.loads(docker('image', 'inspect', args.image))[0]
    image_id = info['Id']
    assert re.fullmatch(r'sha256:[0-9a-f]{64}', image_id), image_id
    assert (info['Config'].get('User') or '').split(':')[0] not in ('', '0', 'root'), 'image must default to nonroot'
    prefix = 'sentori-image-' + uuid.uuid4().hex[:12]
    db, app, volume = prefix + '-db', prefix + '-app', prefix + '-data'
    containers, checks = [], []
    password = 'fixture-' + uuid.uuid4().hex
    client = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))
    base = None

    def request(path, method='GET', data=None, status=200, auth=True):
        req = urllib.request.Request(base + path, method=method,
                                     data=None if data is None else json.dumps(data).encode(),
                                     headers={'Content-Type': 'application/json'})
        try:
            response = (client.open if auth else urllib.request.urlopen)(req, timeout=15)
        except urllib.error.HTTPError as error:
            response = error
        with response:
            body = response.read(2 * 1024 * 1024 + 1)
            assert len(body) <= 2 * 1024 * 1024, 'unexpected response size'
            assert response.status == status, (method, path, response.status, status)
            return json.loads(body) if response.headers.get_content_type() == 'application/json' else body

    def terminate(_signal, _frame):
        signal.signal(signal.SIGTERM, signal.SIG_IGN)
        raise KeyboardInterrupt('image verification interrupted')

    previous = signal.signal(signal.SIGTERM, terminate)
    try:
        docker('network', 'create', '--label', 'sentori.testsuite=' + prefix, prefix)
        docker('volume', 'create', '--label', 'sentori.testsuite=' + prefix, volume)
        docker('run', '-d', '--name', db, '--network', prefix,
               '--label', 'sentori.testsuite=' + prefix,
               '-e', 'POSTGRES_USER=sentori', '-e', 'POSTGRES_PASSWORD=test',
               '-e', 'POSTGRES_DB=sentori', 'postgres:18-alpine')
        containers.append(db)
        for _ in range(120):
            ready = subprocess.run(['docker', 'exec', db, 'pg_isready', '-h', '127.0.0.1', '-U', 'sentori'],
                                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            if ready.returncode == 0:
                break
            time.sleep(.25)
        else:
            raise AssertionError('Postgres did not become ready')
        docker('run', '-d', '--name', app, '--network', prefix, '-p', '127.0.0.1::8080',
               '--label', 'sentori.testsuite=' + prefix, '-v', volume + ':/data',
               '-e', f'DATABASE_URL=postgres://sentori:test@{db}/sentori',
               '-e', 'SENTORI_OWNER_EMAIL=owner@example.test', '-e', 'SENTORI_OWNER_PASSWORD=' + password,
               '-e', 'SENTORI_BASE_URL=http://localhost:8080', '-e', 'SENTORI_MASTER_KEY=' + '75' * 32,
               '-e', 'SENTORI_ATTACHMENT_STORE=fs:/data/blobs', '-e', 'SENTORI_BACKUP_DIR=/data/backups',
               image_id)
        containers.append(app)
        running = json.loads(docker('inspect', app))[0]
        assert running['Image'] == image_id
        assert not any(m['Destination'].startswith('/app') for m in running['Mounts']), 'SPA must come from image'
        base = 'http://127.0.0.1:' + docker('port', app, '8080/tcp').rsplit(':', 1)[1]
        for _ in range(120):
            try:
                health = request('/healthz', auth=False)
                break
            except (OSError, AssertionError):
                time.sleep(.5)
        else:
            raise AssertionError('candidate image did not become healthy')
        assert health['version'] == args.expected_version, health
        checks.append('exact-image-health-version-nonroot')
        html = request('/', auth=False)
        assert b'<html' in html.lower()
        assets = re.findall(rb'(?:src|href)="(/assets/[^"?#]+\.(?:js|css))"', html)
        assert assets, 'baked production assets were not referenced'
        asset_hashes = {}
        for asset_path in sorted(set(assets)):
            payload = request(asset_path.decode(), auth=False)
            assert isinstance(payload, bytes) and payload and b'<html' not in payload[:100].lower()
            asset_hashes[asset_path.decode()] = hashlib.sha256(payload).hexdigest()
        checks.append('baked-spa-and-assets')
        request('/admin/api/projects', status=401, auth=False)
        request('/auth/login', 'POST', {'email': 'owner@example.test', 'password': password})
        instance = request('/admin/api/instance')
        proof = request('/admin/api/instance/check', 'POST', {})
        assert all(proof[key] for key in ['database', 'blobStorage', 'durableStorage', 'originValid'])
        request('/admin/api/instance/initialize', 'POST', {'expectedVersion': instance['version']})
        project = request('/admin/api/projects', 'POST', {'name': 'Image acceptance'}, status=201)
        assert request('/admin/api/projects')['projects'][0]['id'] == project['id']
        checks.append('authentication-and-writable-durable-storage')
        docker('restart', app)
        # Docker can allocate a different host port when restarting a container
        # whose published port was requested dynamically.
        base = 'http://127.0.0.1:' + docker('port', app, '8080/tcp').rsplit(':', 1)[1]
        for _ in range(120):
            try:
                request('/healthz', auth=False)
                break
            except (OSError, AssertionError):
                time.sleep(.25)
        else:
            raise AssertionError('candidate image did not recover after restart')
        request('/auth/login', 'POST', {'email': 'owner@example.test', 'password': password})
        assert any(p['id'] == project['id'] for p in request('/admin/api/projects')['projects'])
        assert request('/admin/api/instance')['id'] == instance['id']
        checks.append('restart-preserves-instance-and-project')
        report = {'schema_version': 1, 'image_acceptance_version': 1,
                  'status': 'passed', 'image_id': image_id, 'version': args.expected_version,
                  'checks': checks, 'asset_sha256': asset_hashes,
                  'scope': 'image installation, health, bundled SPA, authentication, storage and restart; full domain tests are separate'}
        args.output.parent.mkdir(parents=True, exist_ok=True)
        with args.output.open('x') as stream:
            json.dump(report, stream, indent=2)
        print('PASS exact image ' + image_id + ': ' + ', '.join(checks), flush=True)
    except BaseException as error:
        # Containers are disposable, their first-failure diagnostics are not.
        # Retain bounded logs before finally removes only our own resources.
        logs = {}
        for name in containers:
            result = subprocess.run(['docker', 'logs', '--tail', '100', name],
                                    capture_output=True, text=True, timeout=15)
            logs[name] = (result.stdout + result.stderr)[-32768:]
        failure = {'schema_version': 1, 'status': 'failed', 'image_id': image_id,
                   'version': args.expected_version, 'checks': checks,
                   'reason': str(error), 'container_logs': logs}
        if isinstance(error, subprocess.CalledProcessError):
            failure['command_stderr'] = (error.stderr or '')[-8192:]
        args.output.parent.mkdir(parents=True, exist_ok=True)
        if not args.output.exists():
            with args.output.open('x') as stream:
                json.dump(failure, stream, indent=2)
        raise
    finally:
        for name in reversed(containers):
            subprocess.run(['docker', 'rm', '-f', name], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        subprocess.run(['docker', 'volume', 'rm', volume], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        subprocess.run(['docker', 'network', 'rm', prefix], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        signal.signal(signal.SIGTERM, previous)


if __name__ == '__main__':
    main()
