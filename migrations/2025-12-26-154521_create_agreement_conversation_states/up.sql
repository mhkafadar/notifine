CREATE TABLE agreement_conversation_states (
    id SERIAL PRIMARY KEY,
    telegram_user_id BIGINT NOT NULL UNIQUE,
    state VARCHAR(50) NOT NULL DEFAULT 'idle',
    state_data JSONB,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_agreement_conv_states_user_id ON agreement_conversation_states(telegram_user_id);
CREATE INDEX idx_agreement_conv_states_expires ON agreement_conversation_states(expires_at);
