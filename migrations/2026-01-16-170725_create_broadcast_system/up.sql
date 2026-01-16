-- Broadcast System Tables

-- 1. Track which bot can reach which chat (many-to-many)
CREATE TABLE chat_bot_subscriptions (
    id SERIAL PRIMARY KEY,
    telegram_chat_id BIGINT NOT NULL,
    bot_type VARCHAR(20) NOT NULL,
    is_reachable BOOLEAN NOT NULL DEFAULT TRUE,
    last_success_at TIMESTAMPTZ,
    last_failure_at TIMESTAMPTZ,
    failure_count INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(telegram_chat_id, bot_type)
);

-- 2. Broadcast job queue (for crash recovery)
CREATE TABLE broadcast_jobs (
    id SERIAL PRIMARY KEY,
    message TEXT NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    created_by_chat_id BIGINT NOT NULL,
    total_chats INT NOT NULL DEFAULT 0,
    processed_count INT NOT NULL DEFAULT 0,
    success_count INT NOT NULL DEFAULT 0,
    failed_count INT NOT NULL DEFAULT 0,
    unreachable_count INT NOT NULL DEFAULT 0,
    last_processed_chat_id BIGINT,
    rate_limit_per_sec INT NOT NULL DEFAULT 10,
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 3. Pending deactivations awaiting manual approval
CREATE TABLE pending_deactivations (
    id SERIAL PRIMARY KEY,
    telegram_chat_id BIGINT NOT NULL UNIQUE,
    source_broadcast_job_id INT REFERENCES broadcast_jobs(id) ON DELETE SET NULL,
    failed_bots TEXT[] NOT NULL DEFAULT '{}',
    last_error TEXT,
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    reviewed_at TIMESTAMPTZ,
    reviewed_by_chat_id BIGINT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes for performance
CREATE INDEX idx_chat_bot_subs_telegram_id ON chat_bot_subscriptions(telegram_chat_id);
CREATE INDEX idx_chat_bot_subs_reachable ON chat_bot_subscriptions(is_reachable) WHERE is_reachable = TRUE;
CREATE INDEX idx_broadcast_jobs_status ON broadcast_jobs(status);
CREATE INDEX idx_pending_deactivations_status ON pending_deactivations(status);
