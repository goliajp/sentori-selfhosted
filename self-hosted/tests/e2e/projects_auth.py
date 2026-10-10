"""Project isolation, stale-write protection and real password/session transactions.

Loaded by instance.py --domain-test self-hosted/tests/e2e/projects_auth.py.
The SQL hook only touches that invocation's disposable database.
"""
import concurrent.futures
import hashlib
import uuid


def run(*, base, owner, sql, password, client, request):
    def call(http, path, method='GET', data=None, status=200):
        return request(http, base, path, method, data, status)

    def login(email, secret=password, status=200):
        http = client()
        call(http, '/auth/login', 'POST', {'email': email, 'password': secret}, status)
        return http

    projects = '/admin/api/projects'
    no_platform = call(owner, projects, 'POST', {'name': '  No platform  '}, 201)
    a = call(owner, projects, 'POST', {'name': 'Project A', 'platform': 'web'}, 201)
    b = call(owner, projects, 'POST', {'name': 'Project B', 'platform': 'ios'}, 201)
    assert no_platform['platform'] is None and no_platform['name'] == 'No platform'
    assert call(owner, projects + '/' + no_platform['id'])['platform'] is None
    assert any(p['id'] == no_platform['id'] and p['platform'] is None for p in call(owner, projects)['projects'])
    for bad in ['', '   ', 'x' * 201]:
        call(owner, projects, 'POST', {'name': bad}, 400)
    for bad in ['', '  ', 'x' * 201]:
        call(owner, projects + '/' + a['id'], 'PATCH', {'name': bad, 'expectedVersion': 1}, 400)
    call(owner, projects, 'POST', {'name': 'Platform too long', 'platform': 'x' * 81}, 400)

    # Each request uses the same frozen version: exactly one can commit.
    def edit(name):
        try:
            return call(owner, projects + '/' + a['id'], 'PATCH', {'name': name, 'expectedVersion': 1})
        except AssertionError as error:
            assert error.args[0][2] == 409, error
            return None
    with concurrent.futures.ThreadPoolExecutor(2) as pool:
        edited = list(pool.map(edit, ['Winner A', 'Winner B']))
    assert sum(result is not None for result in edited) == 1
    assert call(owner, projects + '/' + a['id'])['version'] == 2
    call(owner, projects + '/' + a['id'] + '?expectedVersion=1', 'DELETE', status=409)
    call(owner, projects + '/' + a['id'], 'PATCH', {'platform': '', 'expectedVersion': 2})
    assert call(owner, projects + '/' + a['id'])['platform'] is None

    member_info = call(owner, '/admin/api/users', 'POST', {'email': 'domain-member@example.test', 'password': password}, 201)
    member = login('domain-member@example.test')
    call(owner, '/admin/api/users/' + member_info['id'] + '/projects/' + a['id'], 'PUT', {})
    assert [p['id'] for p in call(member, projects)['projects']] == [a['id']]
    call(member, projects + '/' + a['id'])
    call(member, projects + '/' + b['id'], status=403)
    call(member, projects + '/' + b['id'] + '/health', status=403)
    call(member, projects, 'POST', {'name': 'Forbidden'}, 403)
    call(member, projects + '/' + a['id'], 'PATCH', {'name': 'Forbidden'}, 403)
    call(member, projects + '/' + a['id'], 'DELETE', status=403)
    call(member, projects + '/' + b['id'] + '/tokens', 'POST', {'name': 'Forbidden', 'scope': 'ingest'}, 403)
    token = call(member, projects + '/' + a['id'] + '/tokens', 'POST', {'name': 'SDK key', 'scope': 'ingest'}, 201)
    foreign_token = call(owner, projects + '/' + b['id'] + '/tokens', 'POST', {'name': 'Foreign key', 'scope': 'ingest'}, 201)
    call(member, '/admin/api/tokens/' + foreign_token['id'], 'DELETE', status=403)
    call(member, '/admin/api/tokens/' + token['id'], 'DELETE')
    assert next(t for t in call(member, projects + '/' + a['id'] + '/tokens')['tokens'] if t['id'] == token['id'])['revokedAt']
    call(owner, projects + '/' + no_platform['id'] + '?expectedVersion=1', 'DELETE')
    call(owner, projects + '/' + no_platform['id'], status=404)

    # Inject an audit failure to prove the project mutation rolls back with it.
    sql("CREATE FUNCTION fail_v5_audit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.action IN ('project.update','auth.password_reset') THEN RAISE EXCEPTION 'injected audit failure'; END IF; RETURN NEW; END $$; CREATE TRIGGER fail_v5_audit BEFORE INSERT ON audit_logs FOR EACH ROW EXECUTE FUNCTION fail_v5_audit();")
    before = call(owner, projects + '/' + a['id'])
    call(owner, projects + '/' + a['id'], 'PATCH', {'name': 'Must roll back', 'expectedVersion': before['version']}, 500)
    assert call(owner, projects + '/' + a['id']) == before

    account = call(owner, '/admin/api/users', 'POST', {'email': 'recovery@example.test', 'password': password}, 201)
    first, second = login('recovery@example.test'), login('recovery@example.test')
    changed = 'changed-password-' + uuid.uuid4().hex
    call(first, '/auth/change-password', 'POST', {'currentPassword': 'wrong', 'newPassword': changed}, 401)
    call(first, '/auth/change-password', 'POST', {'currentPassword': password, 'newPassword': 'too-short'}, 400)
    call(second, '/auth/me')
    call(first, '/auth/change-password', 'POST', {'currentPassword': password, 'newPassword': changed})
    call(first, '/auth/me')
    call(second, '/auth/me', status=401)
    login('recovery@example.test', password, 401)
    third = login('recovery@example.test', changed)

    def reset_token(expired=False):
        wire = 'fixture-' + uuid.uuid4().hex
        digest = hashlib.sha256(wire.encode()).hexdigest()
        direction = '-' if expired else '+'
        sql(f"INSERT INTO password_resets(id,user_id,token_hash,expires_at) VALUES ('{uuid.uuid4()}','{account['id']}',decode('{digest}','hex'),now() {direction} interval '10 minutes');")
        return wire

    def reset(wire, secret, status=200):
        return call(client(), '/auth/reset-password', 'POST', {'token': wire, 'newPassword': secret}, status)

    recovered = 'recovered-password-' + uuid.uuid4().hex
    reset(reset_token(expired=True), recovered, 400)
    working, sibling = reset_token(), reset_token()
    reset(working, recovered, 500)
    # Failed audit leaves the token reusable, password unchanged and sessions live.
    call(first, '/auth/me'); call(third, '/auth/me'); login('recovery@example.test', changed)
    sql('DROP TRIGGER fail_v5_audit ON audit_logs; DROP FUNCTION fail_v5_audit();')
    def race(_):
        try:
            reset(working, recovered)
            return True
        except AssertionError as error:
            assert error.args[0][2] == 400, error
            return False
    with concurrent.futures.ThreadPoolExecutor(2) as pool:
        outcomes = list(pool.map(race, range(2)))
    assert sum(outcomes) == 1
    reset(working, recovered, 400); reset(sibling, recovered, 400)
    call(first, '/auth/me', status=401); call(third, '/auth/me', status=401)
    login('recovery@example.test', changed, 401)
    recovered_session = login('recovery@example.test', recovered)
    call(recovered_session, '/auth/me')
    call(recovered_session, '/auth/logout', 'POST', {})
    call(recovered_session, '/auth/me', status=401)
    # Recovery requests don't reveal whether the address exists or expose a token.
    known = call(client(), '/auth/forgot-password', 'POST', {'email': 'recovery@example.test'})
    unknown = call(client(), '/auth/forgot-password', 'POST', {'email': 'nobody@example.test'})
    assert known == unknown == {'ok': True}
    assert sql(f"SELECT count(*) FROM password_resets WHERE user_id='{account['id']}' AND used_at IS NULL AND expires_at > now();") == '1'
    assert sql(f"SELECT count(*) FROM audit_logs WHERE actor_user_id='{account['id']}' AND action='auth.password_reset';") == '1'
    print('PASS: nullable project platform, real CRUD, stale update/delete, member object isolation, token revoke isolation, project/audit rollback, password change revocation, reset expiry/reuse/concurrency/audit rollback, logout and non-enumerating recovery')
