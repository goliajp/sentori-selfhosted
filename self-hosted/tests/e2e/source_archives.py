"""Real sentori CLI -> ZIP upload -> stored-byte verification -> source context."""
import datetime
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid
import zipfile


def run(*, base, owner, sql, password, client, request):
    def call(path, method='GET', data=None, status=200):
        return request(owner, base, path, method, data, status)
    project = call('/admin/api/projects', 'POST', {'name': 'Source archives', 'platform': 'ios'}, 201)['id']
    prefix = '/admin/api/projects/' + project
    token = call(prefix + '/tokens', 'POST', {'name': 'Build artifacts', 'scope': 'api'}, 201)['token']
    ingest_token = call(prefix + '/tokens', 'POST', {'name': 'App', 'scope': 'ingest'}, 201)['token']
    cli_package = Path(__file__).resolve().parents[3] / 'sdk/cli'
    release = 'source-app@1.0+7'
    env = {**os.environ, 'SENTORI_TOKEN': token, 'SENTORI_RELEASE': release}
    def upload(release_name, data, status=201):
        boundary = 'source-' + uuid.uuid4().hex
        body = (f'--{boundary}\r\nContent-Disposition: form-data; name="kind"\r\n\r\nsrcbundle\r\n'
                f'--{boundary}\r\nContent-Disposition: form-data; name="file"; filename="sources.zip"\r\n\r\n').encode() + data + f'\r\n--{boundary}--\r\n'.encode()
        req = urllib.request.Request(base + '/v1/releases/' + urllib.parse.quote(release_name, safe='') + '/artifacts', data=body,
            headers={'Authorization': 'Bearer ' + token, 'Content-Type': 'multipart/form-data; boundary=' + boundary})
        try: response = urllib.request.urlopen(req, timeout=15)
        except urllib.error.HTTPError as error: response = error
        result = json.load(response)
        assert response.code == status, (response.code, result)
        return result
    def event(platform, name=release, file='/build/Sources/Pay.swift'):
        event_id = str(uuid.uuid4())
        data = {'id': event_id, 'kind': 'error', 'platform': platform, 'release': name,
            'occurredAt': datetime.datetime.now(datetime.timezone.utc).isoformat(),
            'payload': {'error': {'type': 'SourceTest', 'message': 'source context',
                'stack': [{'file': file, 'line': 2, 'function': 'pay', 'inApp': True}]}}}
        req = urllib.request.Request(base + '/v1/events', data=json.dumps(data).encode(), headers={'Authorization': 'Bearer '+ingest_token, 'Content-Type': 'application/json'})
        with urllib.request.urlopen(req, timeout=15) as response: assert response.code == 202
        return event_id
    def context(event_id):
        value = sql(f"SELECT payload#>'{{error,stack,0,contextLine}}' FROM events WHERE id='{event_id}';")
        return json.loads(value) if value else ''
    old_event = event('ios')
    assert context(old_event) == ''
    with tempfile.TemporaryDirectory(prefix='sentori-source-integration-') as directory:
        root = Path(directory)
        # Exercise the consumer package and local npm bin, as documented; a
        # source-tree entry point alone misses missing files and broken bin links.
        (root/'package.json').write_text(json.dumps({'name':'sentori-cli-consumer','private':True}))
        packed = subprocess.run(['npm','pack',str(cli_package),'--ignore-scripts','--json','--pack-destination',str(root)],
            cwd=root, capture_output=True, text=True, timeout=60)
        assert packed.returncode == 0, (packed.stdout, packed.stderr)
        metadata = json.loads(packed.stdout)
        entries = metadata if isinstance(metadata, list) else list(metadata.values())
        assert len(entries) == 1
        tarball = root/entries[0]['filename']
        installed = subprocess.run(['npm','install','--save-dev','--save-exact','--ignore-scripts','--no-audit','--no-fund',str(tarball)],
            cwd=root, capture_output=True, text=True, timeout=180)
        assert installed.returncode == 0, (installed.stdout, installed.stderr)
        (root/'Sources').mkdir()
        (root/'Sources/Pay.swift').write_text('func pay() {\n  charge()\n}\n')
        (root/'mapping.txt').write_text('com.example.Pay -> a:\n    void pay() -> a\n')
        (root/'index.map').write_text(json.dumps({'version':3,'file':'index.js','sources':['Sources/Pay.swift'],'names':[],'mappings':'AAAA'}))
        def command(*args, expected=0):
            result = subprocess.run(['npx', '--no-install', 'sentori', *args], cwd=root, env=env, capture_output=True, text=True, timeout=40)
            assert result.returncode == expected, (args, result.stdout, result.stderr)
            return result
        version = json.loads((cli_package/'package.json').read_text())['version']
        assert command('--version').stdout.strip() == 'sentori '+version
        command('init', '--api-url', base, '--sources', 'Sources', '--mapping', 'mapping.txt', '--sourcemap', 'index.map')
        assert 'artifact protocol 1' in command('doctor').stdout
        command('config', 'validate')
        command('verify', expected=1)
        command('upload', 'all', '--strict')
        command('verify')
        archive = next((root/'.sentori').glob('sources-*.zip')).read_bytes()
        summary = call(prefix+'/releases')['releases'][0]['artifactSummary']
        assert next(item for item in summary if item['kind']=='srcbundle')['usable'] is True
        # A declared build cannot silently be stored under a different release.
        upload('wrong-build@1', archive, 400)
        assert sql(f"SELECT count(*) FROM releases WHERE project_id='{project}' AND name='wrong-build@1';") == '0'
        # An older file of the same kind cannot satisfy the next local plan.
        (root/'mapping.txt').write_text('com.example.New -> b:\n    void run() -> b\n')
        command('verify', expected=1)
        for _ in range(60):
            if context(old_event) == '  charge()': break
            time.sleep(.25)
        assert context(old_event) == '  charge()', 'late source upload must persist context even without newly symbolicated frames'
        for platform in ['ios','android','web']:
            assert context(event(platform)) == '  charge()'
        # Plain ZIP and historical JSON remain accepted through the same endpoint.
        raw = io.BytesIO()
        with zipfile.ZipFile(raw, 'w', zipfile.ZIP_DEFLATED) as z: z.writestr('Sources/Pay.swift', 'one\nplain source\nthree')
        assert upload('plain@1', raw.getvalue())['usable'] is True
        assert context(event('ios','plain@1')) == 'plain source'
        assert upload('legacy@1', json.dumps({'Sources/Pay.swift':'one\nlegacy source\nthree'}).encode())['usable'] is True
        assert context(event('ios','legacy@1')) == 'legacy source'
        raw = io.BytesIO()
        with zipfile.ZipFile(raw, 'w') as z: z.writestr('../escape.swift', 'bad')
        upload(release, raw.getvalue(), 400)
        raw = io.BytesIO()
        with zipfile.ZipFile(raw, 'w') as z: z.writestr('too-big.swift', 'x'*(512*1024+1))
        upload(release, raw.getvalue(), 400)
        # A generated bundle and an original source with conflicting directory
        # paths must not be treated as the same file merely by basename.
        raw = io.BytesIO()
        with zipfile.ZipFile(raw, 'w') as z: z.writestr('src/index.js', 'one\noriginal source\nthree')
        upload('path-match@1', raw.getvalue())
        assert context(event('web', 'path-match@1', 'https://cdn.example/assets/index.js')) == ''
        assert context(event('web', 'path-match@1', 'https://cdn.example/src/index.js')) == 'original source'
        # Duplicate basenames must not choose arbitrary source code.
        raw = io.BytesIO()
        with zipfile.ZipFile(raw, 'w') as z:
            z.writestr('a/Pay.swift', 'one\nwrong A')
            z.writestr('b/Pay.swift', 'one\nwrong B')
        upload('ambiguous@1', raw.getvalue())
        assert context(event('ios','ambiguous@1')) == ''
    print('PASS: sentori configuration, ZIP upload, installed package, version, compatibility, exact-file verification, release binding, archive limits and late source context')
