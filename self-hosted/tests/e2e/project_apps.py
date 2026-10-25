"""Automatic app discovery and app-owned release metadata through real ingest."""
import concurrent.futures
import datetime
import json
import urllib.error
import urllib.parse
import urllib.request
import uuid


def run(*, base, owner, sql, password, client, request):
    def call(path, method='GET', data=None, status=200, http=owner):
        return request(http, base, path, method, data, status)

    project = call('/admin/api/projects', 'POST', {'name': 'One project, many apps', 'platform': 'ios'}, 201)['id']
    path = '/admin/api/projects/' + project
    token = call(path + '/tokens', 'POST', {'name': 'Shared project key', 'scope': 'ingest'}, 201)['token']
    assert call(path + '/apps')['apps'] == []
    assert call(path + '/health')['apps'] == []
    assert call(path + '/health')['artifactSummary'] == []
    now = datetime.datetime.now(datetime.timezone.utc).isoformat()

    def ingest(route, data, status=202):
        req = urllib.request.Request(base + route, data=json.dumps(data).encode(), headers={'Authorization': 'Bearer ' + token, 'Content-Type': 'application/json'})
        try:
            response = urllib.request.urlopen(req, timeout=15)
        except urllib.error.HTTPError as error:
            response = error
        result = json.load(response)
        assert response.code == status, (route, response.code, result)
        return result

    def event(platform, release, **extra):
        return {'id': str(uuid.uuid4()), 'kind': 'error', 'occurredAt': now, 'platform': platform, 'release': release,
                'payload': {'error': {'type': 'AppFixture', 'message': platform + ' fixture'}}, **extra}

    first = event('ios', 'mobile@1.0')
    ingest('/v1/events', first)
    ingest('/v1/events', first)  # A retry does not create another app or version.
    ingest('/v1/events', event('android', 'mobile@1.0'))
    ingest('/v1/events', event('web', 'storefront@2.0'))
    ingest('/v1/events', event('web', 'console@3.0'))
    ingest('/v1/events', event('ios', 'mobile@1.1'))
    rows = call(path + '/apps')['apps']
    assert len(rows) == 4, rows
    apps = {(app['platform'], app['identifier']): app for app in rows}
    ios, android, storefront, console = [apps[key] for key in [('ios', 'mobile'), ('android', 'mobile'), ('web', 'storefront'), ('web', 'console')]]
    assert ios['releaseCount'] == 2 and ios['latestRelease']['name'] == 'mobile@1.1'
    assert android['releaseCount'] == 1 and android['latestRelease']['name'] == 'mobile@1.0'
    assert ios['events24h'] == 2 and android['events24h'] == 1
    assert all(app['identitySource'] == 'release' for app in rows)
    assert all(app['lastEventAt'] for app in rows)
    assert {app['id'] for app in call(path + '/health')['apps']} == {app['id'] for app in rows}
    assert call(path + '/releases?appId=' + ios['id'])['total'] == 2
    assert [r['name'] for r in call(path + '/releases?appId=' + android['id'])['releases']] == ['mobile@1.0']
    assert {a['id'] for a in call(path + '/releases?q=mobile%401.0')['releases'][0]['apps']} == {ios['id'], android['id']}
    assert call(path + '/releases?appId=' + str(uuid.uuid4()))['total'] == 0

    # API tokens select one project; App filters must also constrain aggregates.
    api_token = call(path + '/tokens', 'POST', {'name': 'Agent', 'scope': 'api'}, 201)['token']
    def api(route, key=api_token, status=200):
        req = urllib.request.Request(base + route, headers={'Authorization': 'Bearer ' + key})
        try:
            response = urllib.request.urlopen(req, timeout=15)
        except urllib.error.HTTPError as error:
            response = error
        result = json.load(response)
        assert response.code == status, (route, response.code, result)
        return result
    assert api('/api/project')['project']['id'] == project
    assert {a['id'] for a in api('/api/apps')['apps']} == {a['id'] for a in rows}
    assert ios['kind'] == 'ios' and storefront['kind'] == 'webapp'
    for route in ['/api/project', '/api/apps', '/api/issues']:
        api(route, token, 403)
    scoped = api('/api/issues?appId=' + ios['id'])
    assert scoped['total'] > 0 and sum(i['eventCount'] for i in scoped['issues']) == 2, scoped
    assert sum(i['eventCount'] for i in api('/api/issues?appId=' + android['id'])['issues']) == 1
    assert api('/api/issues?appId=' + ios['id'] + '&platform=android')['total'] == 0
    assert api('/api/issues?appId=' + ios['id'] + '&release=mobile%401.0')['issues'][0]['eventCount'] == 1
    api('/api/issues?appId=' + str(uuid.uuid4()), status=404)
    api('/api/issues?status=bogus', status=400)
    api('/api/issues?offset=-1', status=400)
    whole = api('/api/issues')
    assert whole['total'] >= 3
    pages = [api('/api/issues?limit=1&offset=' + str(i)) for i in range(whole['total'])]
    assert [p['issues'][0]['id'] for p in pages] == [i['id'] for i in whole['issues']]
    for i, page in enumerate(pages):
        assert page['total'] == whole['total'] and page['limit'] == 1 and page['offset'] == i
        assert page['nextOffset'] == (i + 1 if i + 1 < whole['total'] else None)
    beyond = api('/api/issues?limit=1&offset=10000')
    assert beyond['issues'] == [] and beyond['total'] == whole['total'] and beyond['nextOffset'] is None
    assert api('/api/issues?appId=' + ios['id'] + '&limit=1&offset=1')['total'] == scoped['total']

    call(path + '/apps/' + console['id'], 'PATCH', {'expectedVersion': 1, 'kind': 'webapi'})
    assert next(a for a in api('/api/apps')['apps'] if a['id'] == console['id'])['kind'] == 'webapi'
    call(path + '/apps/' + console['id'], 'PATCH', {'expectedVersion': 2, 'kind': 'bogus'}, 400)

    ingest('/v1/events', event('ios', 'mobile@1.0'))
    assert next(app for app in call(path + '/apps')['apps'] if app['id'] == ios['id'])['latestRelease']['name'] == 'mobile@1.1'

    # Sessions carry the same project key. App filtering must cover totals,
    # per-release groups and trend buckets, not merely the version list.
    sessions = [{'id': str(uuid.uuid4()), 'platform': platform, 'release': release, 'status': status,
                 'startedAt': now, 'durationMs': 4000, 'userKey': 'person-' + platform}
                for platform, release, status in [('ios', 'mobile@1.1', 'ok'), ('android', 'mobile@1.0', 'crashed'), ('weapp', 'mini@1', 'ok')]]
    ingest('/v1/events:batch', {'events': [], 'sessions': sessions}, 200)
    ingest('/v1/events:batch', {'events': [], 'sessions': sessions}, 200)
    for app, crashed in [(ios, 0), (android, 1)]:
        health = call('/admin/api/sessions/crash-free?hours=24&projectId=' + project + '&appId=' + app['id'])
        assert health['sessions'] == 1 and health['crashedSessions'] == crashed, health
        assert len(health['releases']) == 1 and health['releases'][0]['platform'] == app['platform']
        assert sum(row['sessions'] for row in health['trend']) == 1
        assert sum(row['crashedSessions'] for row in health['trend']) == crashed
    mini = next(app for app in call(path + '/apps')['apps'] if app['platform'] == 'weapp')
    assert mini['lastSessionAt'] and mini['lastEventAt'] is None

    # Identity metadata is optional. Two known native packages using a plain
    # version stay separate; unidentifiable plain versions share a platform app.
    for bundle in ['jp.example.reader', 'jp.example.editor']:
        ingest('/v1/events', event('ios', '4.0', payload={'app': {'bundleIdentifier': bundle}, 'message': bundle}))
    for release in ['1.0', '1.1']:
        ingest('/v1/events', event('javascript', release, payload={'app': {'identifier': {'malformed': True}}}))
    assert {app['identifier'] for app in call(path + '/apps')['apps'] if app['platform'] == 'ios'} == {'mobile', 'jp.example.reader', 'jp.example.editor'}
    generic = next(app for app in call(path + '/apps')['apps'] if app['platform'] == 'javascript')
    assert generic['identifier'] == '' and generic['releaseCount'] == 2
    before = len(call(path + '/apps')['apps'])
    forged_retry = {**first, 'release': 'must-not-appear@9', 'platform': 'web'}
    ingest('/v1/events', forged_retry)
    assert len(call(path + '/apps')['apps']) == before

    # App metadata uses optimistic writes and belongs only to this app.
    app_path = path + '/apps/' + ios['id']
    call(app_path, 'PATCH', {'expectedVersion': 1, 'name': 'Mobile iOS', 'repositoryUrl': 'https://github.com/example/mobile'})
    call(app_path, 'PATCH', {'expectedVersion': 1, 'name': 'Stale'}, 409)
    # Human-name search and exact historical-key lookup share app/project isolation.
    def by_names(names, suffix=''):
        return call(path + '/releases?names=' + urllib.parse.quote(json.dumps(names)) + suffix)
    assert {r['name'] for r in call(path + '/releases?q=Mobile%20iOS')['releases']} == {'mobile@1.0', 'mobile@1.1'}
    assert [r['name'] for r in call(path + '/releases?q=Mobile%201.0')['releases']] == ['mobile@1.0']
    assert call(path + '/releases?q=Mobile%20iOS&appId=' + android['id'])['total'] == 0
    assert [r['name'] for r in by_names(['mobile@1.0', 'missing@1'])['releases']] == ['mobile@1.0']
    assert by_names([])['total'] == 0
    assert by_names(['mobile@1.1'], '&appId=' + android['id'])['total'] == 0
    assert by_names(['mobile@1.0'], '&q=storefront')['total'] == 0
    assert {a['id'] for a in by_names(['mobile@1.0'])['releases'][0]['apps']} == {ios['id'], android['id']}
    for invalid in ['not-json', '{}', '[1]', json.dumps(['']), json.dumps(['x' * 201]), json.dumps(['x'] * 101)]:
        call(path + '/releases?names=' + urllib.parse.quote(invalid), status=400)

    for url in ['javascript:alert(1)', 'https://user:secret@example.test/repo', 'https://example.test/repo?token=secret']:
        call(app_path, 'PATCH', {'expectedVersion': 2, 'repositoryUrl': url}, 400)
    def edit(name):
        try:
            call(app_path, 'PATCH', {'expectedVersion': 2, 'name': name})
            return True
        except AssertionError as error:
            assert error.args[0][2] == 409, error
            return False
    with concurrent.futures.ThreadPoolExecutor(2) as executor:
        assert sum(executor.map(edit, ['Mobile iOS A', 'Mobile iOS B'])) == 1
    updated = next(app for app in call(path + '/apps')['apps'] if app['id'] == ios['id'])
    assert updated['identitySource'] == 'configured'
    assert updated['version'] == 3 and updated['repositoryUrl'] == 'https://github.com/example/mobile'
    assert next(app for app in call(path + '/apps')['apps'] if app['id'] == android['id'])['repositoryUrl'] is None
    call(app_path, 'PATCH', {'expectedVersion': 3, 'platform': 'android'}, 422)
    sql("CREATE FUNCTION fail_app_audit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.action='app.update' THEN RAISE EXCEPTION 'injected app audit failure'; END IF; RETURN NEW; END $$; CREATE TRIGGER fail_app_audit BEFORE INSERT ON audit_logs FOR EACH ROW EXECUTE FUNCTION fail_app_audit();")
    call(app_path, 'PATCH', {'expectedVersion': 3, 'name': 'Must roll back'}, 500)
    assert next(app for app in call(path + '/apps')['apps'] if app['id'] == ios['id']) == updated
    sql('DROP TRIGGER fail_app_audit ON audit_logs; DROP FUNCTION fail_app_audit();')

    other = call('/admin/api/projects', 'POST', {'name': 'Foreign project'}, 201)['id']
    call('/admin/api/projects/' + other + '/apps/' + ios['id'], 'PATCH', {'expectedVersion': 3, 'name': 'Cross project'}, 404)
    assert call('/admin/api/projects/' + other + '/releases?appId=' + ios['id'])['total'] == 0
    assert call('/admin/api/projects/' + other + '/releases?names=' + urllib.parse.quote(json.dumps(['mobile@1.0'])))['total'] == 0
    other_token = call('/admin/api/projects/' + other + '/tokens', 'POST', {'name': 'Other agent', 'scope': 'api'}, 201)['token']
    assert api('/api/apps', other_token)['apps'] == []
    assert api('/api/issues', other_token)['total'] == 0
    api('/api/issues?appId=' + ios['id'], other_token, 404)
    # Configured apps exist before telemetry and ingest reuses their identity.
    configured = {'platform': 'web', 'identifier': 'admin-panel', 'kind': 'webapp', 'name': 'Admin panel'}
    created = call(path + '/apps', 'POST', configured, 201)
    call(path + '/apps', 'POST', configured, 409)
    call(path + '/apps', 'POST', {**configured, 'identifier': 'x', 'kind': 'bogus'}, 400)
    ingest('/v1/events', event('web', 'admin@1', payload={'app': {'identifier': 'admin-panel'}, 'error': {'message': 'Configured app'}}))
    assert next(a for a in api('/api/apps')['apps'] if a['id'] == created['id'])['events24h'] == 1
    assert api('/api/issues?appId=' + created['id'])['total'] == 1
    member = call('/admin/api/users', 'POST', {'email': 'app-member@example.test', 'password': password}, 201)
    http = client()
    call('/auth/login', 'POST', {'email': 'app-member@example.test', 'password': password}, http=http)
    call(path + '/apps', status=403, http=http)
    call('/admin/api/users/' + member['id'] + '/projects/' + project, 'PUT', {})
    call(path + '/apps', http=http)
    call(app_path, 'PATCH', {'expectedVersion': 3, 'name': 'Member write'}, 403, http)
    call(path + '/apps', 'POST', {**configured, 'identifier': 'member-app'}, 403, http)
    call('/admin/api/projects/' + other + '/apps', status=403, http=http)

    # Discovering a stable identifier preserves an operator-configured role.
    ingest('/v1/events', event('javascript', 'role-api@1'))
    alias = next(a for a in api('/api/apps')['apps'] if a['identifier'] == 'role-api')
    call(path + '/apps/' + alias['id'], 'PATCH', {'expectedVersion': 1, 'kind': 'webapi'})
    ingest('/v1/events', event('javascript', 'different@1', payload={'app': {'identifier': 'stable-api'}}))
    target = next(a for a in api('/api/apps')['apps'] if a['identifier'] == 'stable-api')
    ingest('/v1/events', event('javascript', 'role-api@1', payload={'app': {'identifier': 'stable-api'}}))
    roles = api('/api/apps')['apps']
    assert not any(a['id'] == alias['id'] for a in roles)
    merged = next(a for a in roles if a['id'] == target['id'])
    assert merged['kind'] == 'webapi' and merged['version'] > 2

    # Native uploads establish platform evidence before a device connects.
    # A sourcemap alone cannot tell whether JavaScript runs in RN or the web.
    def artifact(release, kind, contents):
        ingest('/v1/deploys', {'release': release})
        release_id = next(r['id'] for r in call(path + '/releases')['releases'] if r['name'] == release)
        boundary = 'app-artifact-' + uuid.uuid4().hex
        body = (f'--{boundary}\r\nContent-Disposition: form-data; name="kind"\r\n\r\n{kind}\r\n'
                f'--{boundary}\r\nContent-Disposition: form-data; name="file"; filename="fixture"\r\nContent-Type: application/octet-stream\r\n\r\n{contents}\r\n--{boundary}--\r\n').encode()
        req = urllib.request.Request(base + path + '/releases/' + release_id + '/artifacts', data=body, headers={'Content-Type': 'multipart/form-data; boundary=' + boundary})
        with owner.open(req, timeout=15) as response:
            assert response.code == 201
            return json.load(response)
    artifact('native-only@1', 'proguard', 'com.example.Real -> a:\n    void run() -> a\n')
    native = next(app for app in call(path + '/apps')['apps'] if app['identifier'] == 'native-only')
    assert native['platform'] == 'android' and native['lastEventAt'] is None and native['lastSessionAt'] is None
    assert native['artifactSummary'][0]['usable']
    broken = artifact('broken-only@1', 'dsym', 'not a Mach-O symbol file')
    assert broken['usable'] is False
    bad_app = next(app for app in call(path + '/apps')['apps'] if app['identifier'] == 'broken-only')
    assert bad_app['artifactSummary'][0]['broken'] and not bad_app['artifactSummary'][0]['usable']

    def session(release):
        return {'id': str(uuid.uuid4()), 'platform': 'android', 'release': release,
                'status': 'ok', 'startedAt': now, 'durationMs': 1000}

    def identified(release, identifier, message='Identified native app'):
        return event('android', release, payload={'app': {'packageName': identifier},
                     'error': {'type': 'Discovery', 'message': message}})

    # Symbols, sessions and explicit package metadata can arrive in any order.
    # Refining an inferred app retains its ID and operator metadata.
    early = session('native-only@1')
    ingest('/v1/events:batch', {'sessions': [early]}, 200)
    call(path + '/apps/' + native['id'], 'PATCH', {'expectedVersion': 1,
         'name': 'Native Android', 'repositoryUrl': 'https://github.com/example/native'})
    ingest('/v1/events', identified('native-only@1', 'jp.example.native'))
    refined = next(app for app in call(path + '/apps')['apps'] if app['identifier'] == 'jp.example.native')
    assert refined['id'] == native['id'] and refined['name'] == 'Native Android'
    assert refined['repositoryUrl'] == 'https://github.com/example/native'
    assert sql(f"SELECT app_id FROM sessions WHERE id='{early['id']}';") == native['id']
    artifact('native-only@2', 'proguard', 'com.example.Real -> a:\n    void run() -> a\n')
    ingest('/v1/events:batch', {'sessions': [session('native-only@2')]}, 200)
    assert not any(app['identifier'] == 'native-only' for app in call(path + '/apps')['apps'])
    assert next(app for app in call(path + '/apps')['apps'] if app['id'] == native['id'])['releaseCount'] == 2

    # An already known package can acquire a different build prefix. Merge its
    # inferred row, sessions and metadata into the existing canonical app.
    ingest('/v1/events', identified('previous-build@1', 'jp.example.renamed'))
    canonical = next(app for app in call(path + '/apps')['apps'] if app['identifier'] == 'jp.example.renamed')
    artifact('renamed-build@2', 'proguard', 'com.example.Real -> a:\n    void run() -> a\n')
    alias = next(app for app in call(path + '/apps')['apps'] if app['identifier'] == 'renamed-build')
    late = session('renamed-build@2')
    ingest('/v1/events:batch', {'sessions': [late]}, 200)
    inferred_event = event('android', 'renamed-build@2')
    ingest('/v1/events', inferred_event)
    call(path + '/apps/' + alias['id'], 'PATCH', {'expectedVersion': 1,
         'name': 'Renamed build', 'repositoryUrl': 'https://github.com/example/renamed'})
    ingest('/v1/events', identified('renamed-build@2', 'jp.example.renamed'))
    merged = next(app for app in call(path + '/apps')['apps'] if app['id'] == canonical['id'])
    assert merged['name'] == 'Renamed build' and merged['repositoryUrl'] == 'https://github.com/example/renamed'
    assert merged['releaseCount'] == 2 and not any(app['id'] == alias['id'] for app in call(path + '/apps')['apps'])
    assert sql(f"SELECT app_id FROM sessions WHERE id='{late['id']}';") == canonical['id']
    assert sql(f"SELECT app_id FROM events WHERE id='{inferred_event['id']}';") == canonical['id']

    # Competing first reports, session reports without package metadata and a
    # concurrent symbol replacement must converge without a duplicate or 5xx.
    artifact('race-native@1', 'proguard', 'com.example.Real -> a:\n    void run() -> a\n')
    provisional = next(app for app in call(path + '/apps')['apps'] if app['identifier'] == 'race-native')
    race_sessions = [session('race-native@1') for _ in range(8)]
    def race(index):
        if index < 8:
            ingest('/v1/events', identified('race-native@1', 'jp.example.concurrent', str(index)))
        elif index < 16:
            ingest('/v1/events:batch', {'sessions': [race_sessions[index - 8]]}, 200)
        else:
            artifact('race-native@1', 'proguard', 'com.example.Real -> a:\n    void run() -> a\n')
    with concurrent.futures.ThreadPoolExecutor(max_workers=8) as executor:
        list(executor.map(race, range(17)))
    raced = [app for app in call(path + '/apps')['apps'] if app['identifier'] in ('race-native', 'jp.example.concurrent')]
    assert len(raced) == 1 and raced[0]['id'] == provisional['id'] and raced[0]['events24h'] == 8, raced
    assert sql(f"SELECT count(*) FROM sessions WHERE app_id='{provisional['id']}';") == '8'

    # Strong identifiers never collapse merely because two packages use the
    # same version namespace, including an identifier equal to that namespace.
    for identifier in ['shared-native', 'jp.example.other']:
        ingest('/v1/events', identified('shared-native@1', identifier))
    separate = {app['identifier']: app['id'] for app in call(path + '/apps')['apps']
                if app['identifier'] in ('shared-native', 'jp.example.other')}
    assert len(separate) == 2
    ambiguous = session('shared-native@1')
    ingest('/v1/events:batch', {'sessions': [ambiguous]}, 200)
    unknown = sql(f"SELECT app_id FROM sessions WHERE id='{ambiguous['id']}';")
    assert unknown not in separate.values()
    ingest('/v1/events', identified('shared-native@1', 'jp.example.other'))
    assert sql(f"SELECT app_id FROM sessions WHERE id='{ambiguous['id']}';") == unknown

    # A missing crash version is shared across applications. One identified
    # crash must not absorb earlier or later unidentifiable reports.
    unknown_before = event('weapp', 'unknown')
    ingest('/v1/events', unknown_before)
    unassigned = sql(f"SELECT app_id FROM events WHERE id='{unknown_before['id']}';")
    for identifier in ['jp.example.first', 'jp.example.second']:
        ingest('/v1/events', event('weapp', 'unknown', payload={'app': {'identifier': identifier}}))
        unknown_after = event('weapp', 'unknown')
        ingest('/v1/events', unknown_after)
        assert sql(f"SELECT app_id FROM events WHERE id='{unknown_before['id']}';") == unassigned
        assert sql(f"SELECT app_id FROM events WHERE id='{unknown_after['id']}';") == unassigned
    identities = {app['identifier']: app for app in call(path + '/apps')['apps'] if app['platform'] == 'weapp'}
    assert set(identities) == {'mini', '', 'jp.example.first', 'jp.example.second'}, identities
    assert identities['']['identitySource'] == 'unknown'
    assert identities['']['events24h'] == 3
    assert all(identities[identifier]['identitySource'] == 'reported' for identifier in ['jp.example.first', 'jp.example.second'])

    before = len(call(path + '/apps')['apps'])
    artifact('javascript-build@1', 'sourcemap', json.dumps({'version': 3, 'sources': ['app.js'], 'names': [], 'mappings': 'AAAA'}))
    assert len(call(path + '/apps')['apps']) == before
    assert next(row for row in call(path + '/health')['artifactSummary'] if row['kind'] == 'sourcemap')['usable']

    # Discovery survives retention / quiet periods; activity counts do not.
    sql(f"UPDATE events SET received_at=now()-interval '3 days' WHERE project_id='{project}'; UPDATE sessions SET received_at=now()-interval '3 days' WHERE project_id='{project}'; UPDATE apps SET last_event_at=CASE WHEN last_event_at IS NOT NULL THEN now()-interval '3 days' END,last_session_at=CASE WHEN last_session_at IS NOT NULL THEN now()-interval '3 days' END WHERE project_id='{project}';")
    quiet = call(path + '/health')
    assert len(quiet['apps']) == before and all(app['events24h'] == 0 for app in quiet['apps'])
    assert next(app for app in quiet['apps'] if app['id'] == ios['id'])['lastEventAt']
    print('PASS automatic app discovery, shared project ingest, app versions and repositories, app crash-free isolation, artifact evidence, quiet history, stale writes, audit rollback and project authorization', flush=True)
