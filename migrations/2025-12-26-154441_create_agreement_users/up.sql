CREATE TABLE agreement_users (
    id SERIAL PRIMARY KEY,
    telegram_user_id BIGINT NOT NULL UNIQUE,
    telegram_chat_id BIGINT NOT NULL,
    username VARCHAR(255),
    first_name VARCHAR(255),
    last_name VARCHAR(255),
    language VARCHAR(5) NOT NULL DEFAULT 'tr',
    timezone VARCHAR(50) NOT NULL DEFAULT 'Europe/Istanbul',
    disclaimer_accepted BOOLEAN NOT NULL DEFAULT FALSE,
    disclaimer_accepted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_agreement_users_telegram_user_id ON agreement_users(telegram_user_id);
CREATE INDEX idx_agreement_users_telegram_chat_id ON agreement_users(telegram_chat_id);
