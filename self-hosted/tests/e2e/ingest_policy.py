"""Project split compatibility with real tokens, sessions and attachment uploads."""
import concurrent.futures
import datetime
import json
import urllib.request
import uuid

NEEDS_RUNTIME = True


def run(*, base, owner, sql, password, client, request, restart, login, runtime):
    def call(path, data):
        return request(owner, base, path, 'POST', data, 201)

    source, target, unrelated = [call('/admin/api/projects', {'name': name})['id']
                                 for name in ['Legacy', 'Home', 'Unrelated']]
    tokens = [call('/admin/api/projects/' + source + '/tokens', {'name': name, 'scope': 'ingest'})
              for name in ['Delegated', 'Not delegated']]
    token_id = sql("SELECT id FROM tokens WHERE project_id='" + source + "' AND name='Delegated'")
    policy = {'routes': [{'token': token_id, 'source': source, 'target': target,
                         'identifiers': ['home.app'], 'namespaces': ['consumer']}],
              'trace_filters': [{'project': target, 'names': ['app.launch']}]}
    restart(ingest_policy=policy)
    base = runtime()[1]
    now = datetime.datetime.now(datetime.timezone.utc).isoformat()

    def send(path, body, token=0, expected=202, content_type='application/json'):
        body = body if isinstance(body, bytes) else json.dumps(body).encode()
        req = urllib.request.Request(base + path, data=body, headers={
            'Authorization': 'Bearer ' + tokens[token]['token'], 'Content-Type': content_type})
        with urllib.request.urlopen(req, timeout=20) as response:
            assert response.status == expected
            return json.load(response)

    def event(**fields):
        return {'id': str(uuid.uuid4()), 'kind': 'error', 'platform': 'ios', 'release': 'consumer@1',
                'occurredAt': now, 'payload': {'error': {'type': 'SplitFixture'}}, **fields}

    def project(event_id):
        return sql("SELECT project_id FROM events WHERE id='" + event_id + "'")

    def upload(event_id):
        body = b'--split\r\nContent-Disposition: form-data; name="source"\r\n\r\nios\r\n--split\r\nContent-Disposition: form-data; name="file"; filename="tail.txt"\r\nContent-Type: text/plain\r\n\r\nevidence\r\n--split--\r\n'
        return send('/v1/events/' + event_id + '/attachments/logTail', body, expected=202,
                    content_type='multipart/form-data; boundary=split')

    first = event()
    upload(first['id'])
    send('/v1/events', first)
    send('/v1/events', first)
    upload(first['id'])
    assert project(first['id']) == target
    assert sql("SELECT count(DISTINCT project_id) FROM event_attachments WHERE event_id='" + first['id'] + "'") == '1'
    assert sql("SELECT min(project_id::text) FROM event_attachments WHERE event_id='" + first['id'] + "'") == target
    wrong_key = event()
    send('/v1/events', wrong_key, token=1)
    assert project(wrong_key['id']) == source
    explicit_other = event(payload={'app': {'identifier': 'main.app'}})
    send('/v1/events', explicit_other)
    assert project(explicit_other['id']) == source
    metadata = event(release='1', payload={'app': {'bundleIdentifier': 'home.app'}})
    main = event(release='main@1')
    retired = event(kind='trace', name='app.launch')
    warn = event(kind='warn', name='app.launch')
    session = {'id': str(uuid.uuid4()), 'platform': 'ios', 'release': 'consumer@1',
               'status': 'ok', 'startedAt': now, 'durationMs': 50}
    result = send('/v1/events:batch', {'events': [metadata, main, retired, warn], 'sessions': [session],
                  'assertStats': [{'name': 'healthy', 'release': 'consumer@1', 'passDelta': 3}],
                  'backendHealthUrl': 'https://example.test/health'}, expected=200)
    assert result['accepted'] == 3 and result['filtered'] == 1, result
    assert [project(e['id']) for e in [metadata, main, retired, warn]] == [target, source, '', target]
    assert sql("SELECT project_id FROM sessions WHERE id='" + session['id'] + "'") == target
    assert sql("SELECT project_id FROM assert_stats WHERE name='healthy'") == target
    assert sql("SELECT count(*) FROM projects WHERE id IN ('" + source + "','" + target + "') AND backend_health_url IS NOT NULL") == '0'
    send('/v1/events', retired)
    assert project(retired['id']) == ''
    assert sql("SELECT event_count FROM issues WHERE id=(SELECT issue_id FROM events WHERE id='" + first['id'] + "')") == '1'
    # Both orderings and concurrent uploads must agree on the routed project.
    for _ in range(8):
        ev = event()
        with concurrent.futures.ThreadPoolExecutor(2) as pool:
            futures = [pool.submit(upload, ev['id']), pool.submit(send, '/v1/events', ev)]
            for future in futures:
                future.result()
        assert sql("SELECT project_id FROM event_attachments WHERE event_id='" + ev['id'] + "'") == target
    assert sql("SELECT count(*) FROM events WHERE project_id='" + unrelated + "'") == '0'
    print('PASS ingest policy delegation, filtering, sessions, mixed batches and attachment races', flush=True)
