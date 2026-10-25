"""Two disposable runtimes held for an external Chromium/WebKit browser driver.

Set SENTORI_TRANSFER_BROWSER_STATE to a private file. The driver reads its port
mapping through SSH and creates <file>.done when finished; all containers are
then removed. A bounded timeout also cleans up after an interrupted driver.
"""
import json
import os
import pathlib
import subprocess
import time
import uuid

NEEDS_RUNTIME = True


def run(*, base, owner, sql, password, client, request, restart, login, runtime):
    del owner, restart, login
    def docker(*args):
        return subprocess.check_output(['docker', *args], text=True).strip()
    app, _ = runtime()
    info = json.loads(docker('inspect', app))[0]
    network = next(iter(info['NetworkSettings']['Networks']))
    database = next(value.split('=', 1)[1] for value in info['Config']['Env'] if value.startswith('DATABASE_URL='))
    binary = next(value['Source'] for value in info['Mounts'] if value['Destination'] == '/sentori')
    name = 'sentori-transfer-browser-' + uuid.uuid4().hex[:12]
    volume = name + '-data'
    state = pathlib.Path(os.environ['SENTORI_TRANSFER_BROWSER_STATE'])
    assert not state.exists(), 'unique state path required'
    sql('CREATE DATABASE transfer_browser;', 'postgres')
    docker('volume', 'create', volume)
    try:
        docker('run', '-d', '--name', name, '--network', network, '-p', '127.0.0.1::8080',
               '-v', binary+':/sentori:ro', '-v', volume+':/data',
               '-e', 'DATABASE_URL='+database.rsplit('/', 1)[0]+'/transfer_browser',
               '-e', 'SENTORI_OWNER_EMAIL=owner@example.test', '-e', 'SENTORI_OWNER_PASSWORD='+password,
               '-e', 'SENTORI_MASTER_KEY='+'71'*32, '-e', 'SENTORI_ATTACHMENT_STORE=fs:/data/blobs',
               '--entrypoint', '/sentori', 'ubuntu:24.04')
        target = 'http://127.0.0.1:'+docker('port', name, '8080/tcp').split(':')[-1]
        for _ in range(300):
            try:
                request(client(), target, '/healthz')
                break
            except (OSError, AssertionError):
                time.sleep(.1)
        else:
            raise AssertionError('target failed to start')
        fd = os.open(state, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(fd, 'w') as handle:
            json.dump({'source': base, 'target': target, 'password': password}, handle)
        print('READY browser fixture '+str(state), flush=True)
        until = time.monotonic() + 1800
        while time.monotonic() < until:
            if pathlib.Path(str(state)+'.done').exists():
                break
            time.sleep(1)
        else:
            raise AssertionError('browser driver timeout')
    finally:
        subprocess.run(['docker', 'rm', '-f','-v', name], stdout=subprocess.DEVNULL, check=False)
        subprocess.run(['docker', 'volume', 'rm', '-f', volume], stdout=subprocess.DEVNULL, check=False)
        state.unlink(missing_ok=True)
        pathlib.Path(str(state)+'.done').unlink(missing_ok=True)
