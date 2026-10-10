#!/usr/bin/env python3
"""Run the opt-in Rust database contract against one owned disposable container."""
import json
import os
import pathlib
import subprocess
import time
import uuid


def main():
    root = pathlib.Path(__file__).resolve().parents[3]
    name = 'sentori-project-import-' + uuid.uuid4().hex[:12]
    password = uuid.uuid4().hex
    docker = lambda *args: subprocess.check_output(['docker', *args], text=True).strip()
    try:
        docker('run', '-d', '--name', name, '-e', 'POSTGRES_PASSWORD=' + password,
               '-p', '127.0.0.1::5432', 'postgres:18-alpine')
        # The image starts a temporary Unix-socket-only server during initdb.
        # Wait for its final TCP listener before running the host-side Rust test.
        deadline = time.monotonic() + 60
        while subprocess.run(['docker', 'exec', name, 'pg_isready', '-h', '127.0.0.1', '-U', 'postgres'], capture_output=True).returncode:
            if time.monotonic() > deadline:
                raise RuntimeError('disposable PostgreSQL failed to become ready')
            time.sleep(.2)
        ports = json.loads(docker('inspect', '--format', '{{json .NetworkSettings.Ports}}', name))
        port = ports['5432/tcp'][0]['HostPort']
        environment = {**os.environ, 'SENTORI_IMPORT_TEST_DATABASE_URL': f'postgres://postgres:{password}@127.0.0.1:{port}/postgres'}
        subprocess.run(['cargo', 'test', '--manifest-path', 'self-hosted/snapshot/Cargo.toml',
                        '--test', 'project_import', '--', '--ignored', '--nocapture'],
                       cwd=root, env=environment, check=True)
    finally:
        subprocess.run(['docker', 'rm', '-f', '-v', name], capture_output=True)


if __name__ == '__main__':
    main()
