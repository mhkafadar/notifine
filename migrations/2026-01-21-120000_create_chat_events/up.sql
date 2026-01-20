CREATE TABLE chat_events (
    id SERIAL PRIMARY KEY,
    telegram_chat_id BIGINT NOT NULL,
    event_type VARCHAR(20) NOT NULL,
    bot_type VARCHAR(20) NOT NULL,
    inviter_username VARCHAR(255),
    is_cross_bot_user BOOLEAN NOT NULL DEFAULT FALSE,
    other_bots VARCHAR(255),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_chat_events_created_at ON chat_events(created_at);
CREATE INDEX idx_chat_events_type ON chat_events(event_type);
CREATE INDEX idx_chat_events_chat_id ON chat_events(telegram_chat_id);
