CREATE TABLE daily_stats (
    id SERIAL PRIMARY KEY,
    date DATE NOT NULL UNIQUE,
    messages_sent INTEGER NOT NULL DEFAULT 0,
    webhooks_received INTEGER NOT NULL DEFAULT 0,
    github_webhooks INTEGER NOT NULL DEFAULT 0,
    gitlab_webhooks INTEGER NOT NULL DEFAULT 0,
    beep_webhooks INTEGER NOT NULL DEFAULT 0,
    new_chats INTEGER NOT NULL DEFAULT 0,
    churned_chats INTEGER NOT NULL DEFAULT 0,
    uptime_checks INTEGER NOT NULL DEFAULT 0,
    uptime_failures INTEGER NOT NULL DEFAULT 0,
    errors_count INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_daily_stats_date ON daily_stats(date);
