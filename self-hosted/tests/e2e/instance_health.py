"""Read-only diagnostics, actual migration corruption and standalone operator failures."""
import json
import os
import pathlib
import subprocess
import uuid

NEEDS_RUNTIME = True


def run(*, base, owner, sql, password, client, request, restart, login, runtime):
    endpoint = '/admin/api/instance/health'
    def call(http=owner, status=200):
        return request(http, runtime()[1], endpoint, status=status)
    member = login('member@example.test')
    call(client(), 401)
    call(member, 403)
    assert request(owner, runtime()[1], '/admin/api/system')['capabilities']['instance.healthcheck']['allowed']
    assert not request(member, runtime()[1], '/admin/api/system')['capabilities']['instance.healthcheck']['allowed']
    before = sql("SELECT row_to_json(i)::text FROM instance_settings i;")
    audits = sql("SELECT count(*) FROM audit_logs WHERE action LIKE 'instance.%';")
    report = call()
    by_id = {c['id']: c for c in report['checks']}
    assert report['schemaVersion'] == 1 and report['scope'] == 'read-only'
    assert set(by_id) == {'database', 'schema', 'instance', 'backup', 'operations', 'storage', 'recovery'}
    assert by_id['schema']['status'] == 'pass' and by_id['storage']['status'] == 'pass'
    assert by_id['backup']['code'] == 'backup_record_missing'
    assert by_id['recovery']['status'] == 'unknown' and report['status'] != 'pass'
    assert before == sql("SELECT row_to_json(i)::text FROM instance_settings i;")
    assert audits == sql("SELECT count(*) FROM audit_logs WHERE action LIKE 'instance.%';")
    assert all(secret not in json.dumps(report) for secret in [password, '71' * 32, 'postgres://', '/data/'])
    print('PASS owner-only read-only health report with explicit unknown recovery and no secrets', flush=True)

    checksum = sql("SELECT encode(checksum,'hex') FROM _sqlx_migrations WHERE version=1;")
    try:
        sql("UPDATE _sqlx_migrations SET checksum=decode('00','hex') WHERE version=1;")
        altered = call()
        assert altered['status'] == 'fail'
        assert next(c for c in altered['checks'] if c['id'] == 'schema')['code'] == 'schema_mismatch'
    finally:
        sql(f"UPDATE _sqlx_migrations SET checksum=decode('{checksum}','hex') WHERE version=1;")
    assert next(c for c in call()['checks'] if c['id'] == 'schema')['status'] == 'pass'
    print('PASS actual migration checksum corruption fails and read-only checking never repairs it', flush=True)

    # These rows are explicitly synthetic metadata, not backup artifacts.
    operation = str(uuid.uuid4())
    try:
        sql(f"INSERT INTO instance_operations (id,instance_id,generation,command_id,request_hash,kind,state,finished_at) SELECT '{operation}',id,generation,'{uuid.uuid4()}','fixture','backup','ready',now()-interval '2 days' FROM instance_settings;")
        assert next(c for c in call()['checks'] if c['id']=='backup')['code']=='backup_record_stale'
        sql(f"UPDATE instance_operations SET finished_at=now() WHERE id='{operation}';")
        recent = call()
        assert next(c for c in recent['checks'] if c['id']=='backup')['code']=='recent_backup_record'
        assert next(c for c in recent['checks'] if c['id']=='recovery')['status']=='unknown'
        assert recent['status'] != 'pass'
        sql(f"UPDATE instance_operations SET generation=generation+1 WHERE id='{operation}';")
        assert next(c for c in call()['checks'] if c['id']=='backup')['code']=='backup_record_missing'
    finally:
        sql(f"DELETE FROM instance_operations WHERE id='{operation}';")
    print('PASS stale and wrong-generation backups are rejected; recent metadata never proves recovery', flush=True)

    app = runtime()[0]
    inspected = json.loads(subprocess.check_output(['docker','inspect',app],text=True))[0]
    network = next(iter(inspected['NetworkSettings']['Networks']))
    db_url = next(e.split('=',1)[1] for e in inspected['Config']['Env'] if e.startswith('DATABASE_URL='))
    cli = pathlib.Path(os.environ['SENTORI_TEST_CLI']).resolve(strict=True)
    def operator(url, path):
        completed = subprocess.run(['docker','run','--rm','--network',network,'--volumes-from',app,
            '-v',str(cli)+':/operator:ro','-e','DATABASE_URL='+url,'--entrypoint','/operator','ubuntu:24.04',
            'healthcheck','--blob-dir',path],capture_output=True,text=True,timeout=35)
        assert password not in completed.stdout and db_url not in completed.stdout
        return completed.returncode,json.loads(completed.stdout)
    code, result = operator(db_url, '/data/blobs')
    assert code == 1 and result['status'] == 'warning'
    assert {c['id']:c['code'] for c in result['checks']} == {c['id']:c['code'] for c in call()['checks']}
    code, result = operator('postgres://secret:must-not-leak@127.0.0.1:1/missing','/data/not-created')
    assert code == 2 and result['status'] == 'fail'
    assert 'must-not-leak' not in json.dumps(result)
    assert subprocess.run(['docker','exec',app,'test','-e','/data/not-created'],capture_output=True).returncode != 0
    print('PASS operator and HTTP share diagnostics; unavailable database and missing storage cannot pass', flush=True)
