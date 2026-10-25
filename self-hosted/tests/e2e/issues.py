"""Issue query contracts against an isolated PostgreSQL database via instance.py."""
import uuid
import urllib.request
import urllib.error
from urllib.parse import urlencode


def run(*, base, owner, sql, password, client, request):
    def call(http, path, method='GET', data=None, status=200):
        return request(http, base, path, method, data, status)

    a, b = [call(owner, '/admin/api/projects', 'POST', {'name': name}, 201)['id']
            for name in ['Issue query A', 'Issue query B']]
    sql(f"""INSERT INTO issues (id,project_id,fingerprint,kind,group_title,message_sample,first_seen,last_seen,event_count,users_count,max_per_user,last_release)
      SELECT gen_random_uuid(),'{a}',i::text,'error','Case '||i,'literal %_ marker',now()-interval '2 days',now(),2,1,2,'1.0'
      FROM generate_series(1,507) i;
      INSERT INTO issues (id,project_id,fingerprint,kind,group_title,first_seen,last_seen,event_count,last_release)
      VALUES ('{uuid.uuid4()}','{b}','foreign','warn','Private case',now(),now(),9999,'secret');""")

    def listing(http=owner, status=200, **params):
        return call(http, '/admin/api/issues?' + urlencode({'project_id': a, **params}), status=status)

    default = listing()
    assert len(default['issues']) == 100 and default['total'] == 507 and default['nextOffset'] == 100
    assert default['summary'] == {'events': 1014, 'open': 507, 'regressed': 0, 'releases': 1}
    all_ids = []
    for offset in range(0, 507, 37):
        page = listing(limit=37, offset=offset)
        all_ids.extend(row['id'] for row in page['issues'])
        assert page['total'] == 507
        assert page['nextOffset'] == (offset + 37 if offset + 37 < 507 else None)
    assert len(set(all_ids)) == len(all_ids) == 507
    assert all_ids == sorted(all_ids, reverse=True), 'stable UUID tie-break across pages'
    assert len(listing(limit=999)['issues']) == 500
    for offset, limit in [(0, 0), (999, 30)]:
        page = listing(offset=offset, limit=limit)
        assert page['issues'] == [] and page['total'] == 507 and page['nextOffset'] is None
    for params in [{'offset': -1}, {'limit': -1}, {'sort': 'sql'}, {'group': 'invalid'}, {'q': 'x'*201}, {'status': 'invalid'}, {'kind': 'fatal'}]:
        # status names are query fields rather than expected HTTP status here.
        call(owner, '/admin/api/issues?' + urlencode({'project_id': a, **params}), status=400)
    assert listing(q='%_')['total'] == 507, 'search characters are literal'
    assert listing(q='missing')['total'] == 0
    target = all_ids[200]
    assert listing(q=target.upper())['issues'][0]['id'] == target
    sql(f"UPDATE issues SET status='resolved',first_seen=now(),last_seen=now()+interval '1 hour' WHERE id='{target}';")
    assert listing()['total'] == 506
    assert listing(**{'status': 200, 'q': target})['total'] == 0
    mixed = call(owner, '/admin/api/issues?' + urlencode({'project_id': a, 'status': 'all', 'sort': 'newest'}))
    assert mixed['total'] == 507 and mixed['issues'][0]['id'] == target
    assert listing(group='fresh')['total'] == 0
    sql(f"UPDATE issues SET status='open',regressed_at=now() WHERE id='{target}';")
    assert listing(group='fresh')['total'] == listing(group='regressed')['total'] == 1
    assert listing()['issues'][0]['id'] == target
    sql(f"""INSERT INTO events (id, project_id, issue_id,kind,platform,occurred_at,environment,release,payload)
        VALUES ('{uuid.uuid4()}','{a}','{target}','error','javascript',now(),'qa','2.0','{{"context":{{"tenant":"A"}}}}');""")
    assert listing(environment='qa', release='2.0', context_key='tenant', context_value='A')['total'] == 1
    assert listing(context_key='tenant', context_value='B')['total'] == 0
    assert listing(release='missing')['total'] == 0

    # Sort the entire filtered result before paging, independently of impact.
    sorted_project = call(owner, '/admin/api/projects', 'POST', {'name': 'Issue sorting'}, 201)['id']
    sort_ids = [str(uuid.uuid4()) for _ in range(7)]
    display_titles = {issue_id: f'Headline {index}' if index % 2 == 0 else f'Sort {6-index}' for index, issue_id in enumerate(sort_ids)}
    for index, issue_id in enumerate(sort_ids):
        sql(f"""INSERT INTO issues(id,project_id,fingerprint,kind,group_title,message_sample,status,first_seen,last_seen,event_count,users_count,max_per_user)
          VALUES ('{issue_id}','{sorted_project}','sort-{index}','{['error','warn','trace'][index % 3]}',
          '{'TypeError' if index % 2 == 0 else f'Sort {6-index}'}','{display_titles[issue_id]}','{['open','resolved','ignored'][index % 3]}','2025-01-01T00:00:00Z',
          '2026-01-01T00:00:00Z'::timestamptz + interval '{index} hours',{20-index},{index % 4},1);""")
    def sorted_listing(**params):
        return call(owner, '/admin/api/issues?' + urlencode({'project_id': sorted_project, 'status': 'all', **params}))
    source = sorted_listing(limit=50)['issues']
    assert [row['id'] for row in source] == list(reversed(sort_ids)), 'default follows occurrence time, not event count'
    fields = {'title': 'title', 'kind': 'kind', 'status': 'status', 'events': 'eventCount', 'users': 'usersCount'}
    for sort, field, descending in [('newest', 'lastSeen', True), ('oldest', 'lastSeen', False)] + [
            (f'{name}_{direction}', field, direction == 'desc') for name, field in fields.items() for direction in ['asc', 'desc']]:
        # Secondary recency and UUID order remain descending for equal values.
        expected = sorted(sorted(source, key=lambda row: (row['lastSeen'], row['id']), reverse=True), key=lambda row: display_titles[row['id']] if field == 'title' else row[field], reverse=descending)
        actual = [row for offset in range(0, 7, 2) for row in sorted_listing(sort=sort, offset=offset, limit=2)['issues']]
        assert [row['id'] for row in actual] == [row['id'] for row in expected], sort
    assert sorted_listing(sort='impact')['issues'][0]['usersCount'] == 3
    # Filtering changes the aggregate values used for both display and ordering.
    for index, issue_id in enumerate(sort_ids[:2]):
        for occurrence in range(index + 1):
            sql(f"""INSERT INTO events(id,project_id,issue_id,kind,platform,occurred_at,received_at,environment,release,payload)
              VALUES (gen_random_uuid(),'{sorted_project}','{issue_id}','error','javascript',
              '2026-02-01T00:00:00Z'::timestamptz - interval '{index} hours',
              '2026-03-01T00:00:00Z'::timestamptz + interval '{index} hours','sort-qa','1.0','{{}}');""")
    assert sorted_listing(environment='sort-qa')['issues'][0]['id'] == sort_ids[0], 'device occurrence, not receive time or unfiltered recency'
    assert sorted_listing(environment='sort-qa', sort='events_desc')['issues'][0]['id'] == sort_ids[1]
    assert sorted_listing(environment='sort-qa', sort='events_asc')['issues'][0]['id'] == sort_ids[0]
    print('PASS: default occurrence recency, every column in both directions, full-set pagination, stable ties and filtered aggregates')

    account = call(owner, '/admin/api/users', 'POST', {'email': 'issue-member@example.test', 'password': password}, 201)
    member = client()
    call(member, '/auth/login', 'POST', {'email': 'issue-member@example.test', 'password': password})
    assert listing(http=member)['total'] == 0
    call(owner, f"/admin/api/users/{account['id']}/projects/{a}", 'PUT', {})
    visible = call(member, '/admin/api/issues?status=all&limit=500')
    assert visible['total'] == 507 and visible['summary']['events'] == 1014
    assert {row['projectId'] for row in visible['issues']} == {a}
    assert listing(http=member, project_id=b)['total'] == 0
    call(member, '/admin/api/issues/' + sql(f"SELECT id FROM issues WHERE project_id='{b}';"), status=403)
    # Only project-visible people are candidates, and activity commits with state.
    outsider = call(owner, '/admin/api/users', 'POST', {'email': 'issue-outsider@example.test', 'password': password}, 201)
    detail_path = '/admin/api/issues/' + target
    detail = call(member, detail_path)
    assert account['id'] in {user['id'] for user in detail['assignees']}
    assert outsider['id'] not in {user['id'] for user in detail['assignees']}
    call(member, detail_path + '/assign', 'POST', {'userId': outsider['id']}, 400)
    call(member, detail_path + '/assign', 'POST', {'userId': str(uuid.uuid4())}, 400)
    call(member, detail_path + '/assign', 'POST', {'userId': account['id']})
    call(member, detail_path + '/assign', 'POST', {'userId': account['id']})
    assert len(call(member, detail_path)['activity']) == 1
    for invalid in ['', '   ', 'x'*10001]:
        call(member, detail_path + '/notes', 'POST', {'body': invalid}, 400)
    call(member, detail_path + '/notes', 'POST', {'body': 'A real note'}, 201)
    assert call(member, detail_path)['activity'][0]['body']['text'] == 'A real note'
    sql("CREATE FUNCTION fail_issue_activity() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected activity failure'; END $$; CREATE TRIGGER fail_issue_activity BEFORE INSERT ON issue_activity FOR EACH ROW EXECUTE FUNCTION fail_issue_activity();")
    before = call(member, detail_path)
    for action, body in [('resolve', {'release': '3.0'}), ('ignore', {}), ('assign', {'userId': None}), ('notes', {'body': 'Must fail'})]:
        call(member, detail_path + '/' + action, 'POST', body, 500)
        assert call(member, detail_path) == before, action + ' rolls back with evidence'
    sql('DROP TRIGGER fail_issue_activity ON issue_activity; DROP FUNCTION fail_issue_activity();')
    call(member, detail_path + '/resolve', 'POST', {'release': ' 3.0 '})
    call(member, detail_path + '/resolve', 'POST', {'release': '3.0'})
    assert call(member, detail_path)['resolvedInRelease'] == '3.0'
    assert len(call(member, detail_path)['activity']) == 3
    call(member, detail_path + '/reopen', 'POST', {})
    call(member, detail_path + '/assign', 'POST', {'userId': None})
    assert call(member, detail_path)['assigneeUserId'] is None

    sql(f"INSERT INTO events (id,project_id,issue_id,kind,platform,occurred_at,received_at) SELECT gen_random_uuid(),'{a}','{target}','error','javascript',now()-interval '2 months',now()+interval '2 hours' FROM generate_series(1,120);")
    first = call(member, detail_path + '/events?limit=50')
    second = call(member, detail_path + '/events?limit=50&offset=50')
    third = call(member, detail_path + '/events?limit=50&offset=100')
    event_ids = [event['id'] for page in [first,second,third] for event in page['events']]
    assert len(set(event_ids)) == len(event_ids) == 121
    assert first['total'] == 121 and first['nextOffset'] == 50 and second['nextOffset'] == 100 and third['nextOffset'] is None
    assert first['events'][0]['occurredAt'] != first['events'][0]['receivedAt']
    for suffix in ['?limit=0','?offset=200']:
        page = call(member, detail_path + '/events' + suffix)
        assert page['total'] == 121 and page['events'] == [] and page['nextOffset'] is None
    call(member, detail_path + '/events?offset=-1', status=400)
    call(member, detail_path + '/events?limit=-1', status=400)
    own_release, foreign_release = str(uuid.uuid4()), str(uuid.uuid4())
    sql(f"INSERT INTO releases (id,project_id,name) VALUES ('{own_release}','{a}','own@1'),('{foreign_release}','{b}','private@1'); INSERT INTO releases (id,project_id,name) SELECT gen_random_uuid(),'{a}','release-'||i FROM generate_series(1,205) i;")
    release_base = f'/admin/api/projects/{a}/releases'
    page = call(member, release_base)
    assert page['total'] == 206 and len(page['releases']) == 200 and page['nextOffset'] == 200
    assert len(call(member, release_base + '?offset=200')['releases']) == 6
    assert call(member, release_base + '?q=OWN%40')['total'] == 1
    assert call(member, release_base + '?limit=0')['total'] == 206
    assert call(member, release_base + '?limit=0')['releases'] == []
    call(member, release_base + '/' + foreign_release + '/artifacts', status=404)
    call(member, release_base + '/' + own_release + '/artifacts')
    call(member, '/admin/api/releases/' + foreign_release, 'DELETE', status=403)
    boundary = 'sentori-test-boundary'
    multipart = (f'--{boundary}\r\nContent-Disposition: form-data; name="kind"\r\n\r\nsourcemap\r\n'
                 f'--{boundary}\r\nContent-Disposition: form-data; name="file"; filename="test.map"\r\nContent-Type: application/json\r\n\r\nnot a source map\r\n--{boundary}--\r\n').encode()
    def upload(release_id, expected):
        req = urllib.request.Request(base + release_base + '/' + release_id + '/artifacts', data=multipart,
            headers={'Content-Type': 'multipart/form-data; boundary=' + boundary})
        try: response = member.open(req)
        except urllib.error.HTTPError as error: response = error
        assert response.code == expected, response.read()
        response.close()
    upload(foreign_release, 404)
    assert sql(f"SELECT count(*) FROM release_artifacts WHERE release_id='{foreign_release}';") == '0'
    upload(own_release, 201)
    assert call(member, release_base + '/' + own_release + '/artifacts')['artifacts'][0]['usable'] is False
    summary = call(member, release_base + '?q=own%401')['releases'][0]['artifactSummary']
    assert summary == [{'kind': 'sourcemap', 'count': 1, 'usable': False, 'broken': True, 'unchecked': False}]
    sql(f"""INSERT INTO release_artifacts(id,release_id,kind,name,content_hash,usable)
      VALUES ('{uuid.uuid4()}','{own_release}','dsym','unchecked','legacy',NULL),
             ('{uuid.uuid4()}','{own_release}','dsym','usable','valid',true),
             ('{uuid.uuid4()}','{own_release}','dsym','broken','invalid',false),
             ('{uuid.uuid4()}','{foreign_release}','proguard','private','foreign',true);""")
    summary = call(member, release_base + '?q=own%401')['releases'][0]['artifactSummary']
    assert summary[0] == {'kind': 'dsym', 'count': 3, 'usable': True, 'broken': True, 'unchecked': True}
    assert len(summary) == 2, 'summary does not leak a foreign release or invent empty kinds'
    empty_release = next(row for row in call(member, release_base)['releases'] if row['id'] != own_release)
    assert empty_release['artifactSummary'] == [], 'empty and unchecked artifact coverage remain distinct'
    print('PASS: bounded release artifact summaries preserve usable, broken, unchecked and project isolation')
    print('PASS: release pagination, search, admin artifact upload/read project-release binding, foreign delete rejection and unreadable-file truth')
    print('PASS: project-scoped assignees, note bounds, atomic state/activity rollback, idempotent transitions and paginated occurrences with distinct clocks')
    print('PASS: issue pagination >500, totals on empty pages, stable tie-break, literal search, query validation, status/group/sort, historical environment/release/context and member isolation')
