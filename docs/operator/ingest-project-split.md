# Keep legacy clients working after a project split

An operator can delegate a specific existing ingest token to selected projects within the same instance. This is a transition mechanism for installed clients that shared a credential before a split. New clients and upload tools should use their destination project's credentials.

Set `SENTORI_INGEST_POLICY` to JSON, or mount a file and set `SENTORI_INGEST_POLICY_FILE`. Restart the instance to apply changes. Invalid JSON, missing files, overlapping selectors, missing projects and inactive source tokens prevent startup. An unset policy preserves ordinary project scoping.

```json
{
  "routes": [
    {
      "token": "00000000-0000-4000-8000-000000000001",
      "source": "00000000-0000-4000-8000-000000000002",
      "target": "00000000-0000-4000-8000-000000000003",
      "identifiers": ["example.home"],
      "namespaces": ["home"]
    }
  ],
  "trace_filters": [
    {
      "project": "00000000-0000-4000-8000-000000000003",
      "names": ["app.launch"]
    }
  ]
}
```

These UUIDs illustrate the format; replace them with existing IDs. `token` is a token record ID, never its plaintext credential. Routes are operator configuration and must not be writable by project members. They intentionally grant that legacy token ingestion and event-attachment access to the named destination. Do not delegate an untrusted token across an intended isolation boundary.

The server matches `payload.app.identifier`, `bundleIdentifier`, then `packageName`. With no usable explicit identity, it matches the release namespace before the final `@`. Explicit identity takes precedence, even when it does not match a route. Matching is exact and routing is one hop. An unknown identity or namespace stays in the token's original project.

Single events and every batch event are routed before symbolication and grouping. Sessions use their app metadata and release; assertion statistics use their release. A batch's backend health URL is applied only when its events and sessions identify one destination. Mixed batches cannot overwrite either project's health URL. Attachments uploaded before, during or after an event follow its destination, including the replay quota deduplication record.

Trace filters apply after routing and match only `kind: trace` and an exact event name. They acknowledge removal with `filtered: true` and `reason: retired_trace`; batch responses include a separate `filtered` count. They create no event, issue or event quota charge. Other kinds with the same name are retained. Remove the producer instrumentation too, so installed replacements stop spending network traffic on retired events.

This policy does not migrate historical records, API upload credentials, push registrations, provider credentials or management permissions. Back up and verify historical migration separately. Release upload tools must switch to each destination's API token. Push clients and senders must coordinate their project and provider setup; do not infer an old device's application from its user or platform alone.

Policy files are deployment configuration and are not included in database snapshots. Preserve them with the deployment backup. Before removing legacy credentials or projects, remove their routes and restart; keep compatibility until supported installed versions have migrated.
