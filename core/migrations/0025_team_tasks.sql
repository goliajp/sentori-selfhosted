-- Work items are separate from background jobs. Only typed commands mutate them.
CREATE TABLE tasks (
    id UUID PRIMARY KEY,
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    issue_id UUID REFERENCES issues(id) ON DELETE SET NULL,
    assignee_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    title TEXT NOT NULL CHECK (length(title) BETWEEN 1 AND 200),
    description TEXT NOT NULL DEFAULT '',
    kind TEXT NOT NULL CHECK (kind IN ('task','bug')),
    priority TEXT NOT NULL CHECK (priority IN ('urgent','high','normal','low')),
    stage TEXT NOT NULL DEFAULT 'triage' CHECK (stage IN ('triage','ready','inProgress','review','verify','done','cancelled')),
    due_date DATE,
    reproduction TEXT NOT NULL DEFAULT '',
    root_cause TEXT NOT NULL DEFAULT '',
    fix_release TEXT NOT NULL DEFAULT '',
    verified BOOLEAN NOT NULL DEFAULT false,
    automation JSONB NOT NULL DEFAULT '{}',
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX tasks_project_page_idx ON tasks(project_id, updated_at DESC, id DESC);
CREATE UNIQUE INDEX tasks_issue_idx ON tasks(issue_id) WHERE issue_id IS NOT NULL;
CREATE TABLE task_events (
    id UUID PRIMARY KEY,
    task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    actor_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    action TEXT NOT NULL,
    body TEXT NOT NULL DEFAULT '',
    notify_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    category TEXT NOT NULL CHECK (category IN ('assignment','mention','task')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX task_events_page_idx ON task_events(task_id, created_at DESC, id DESC);
CREATE TABLE task_commands (
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    actor_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    command_id UUID NOT NULL,
    digest TEXT NOT NULL,
    response JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY(project_id, actor_user_id, command_id)
);
ALTER TABLE outbox_events ALTER COLUMN job_id DROP NOT NULL;
ALTER TABLE outbox_events DROP CONSTRAINT outbox_events_event_type_check;
ALTER TABLE outbox_events ADD COLUMN task_event_id UUID UNIQUE REFERENCES task_events(id) ON DELETE CASCADE;
ALTER TABLE outbox_events ADD CONSTRAINT outbox_event_source CHECK (
    (event_type='job.ready' AND job_id IS NOT NULL AND task_event_id IS NULL) OR
    (event_type='task.changed' AND job_id IS NULL AND task_event_id IS NOT NULL)
);
CREATE TABLE inbox_preferences (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    assignments BOOLEAN NOT NULL DEFAULT true,
    mentions BOOLEAN NOT NULL DEFAULT true,
    task_updates BOOLEAN NOT NULL DEFAULT true,
    monitors BOOLEAN NOT NULL DEFAULT true
);
CREATE TABLE inbox_items (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    source_event_id UUID NOT NULL REFERENCES task_events(id) ON DELETE CASCADE,
    category TEXT NOT NULL CHECK (category IN ('assignment','mention','task')),
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    read_at TIMESTAMPTZ,
    archived_at TIMESTAMPTZ,
    UNIQUE(user_id, source_event_id)
);
CREATE INDEX inbox_user_page_idx ON inbox_items(user_id, created_at DESC, id DESC);
