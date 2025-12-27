ALTER TABLE agreement_users ADD CONSTRAINT unique_telegram_user_id UNIQUE (telegram_user_id);
CREATE INDEX idx_agreement_users_telegram_user_id ON agreement_users(telegram_user_id);
