DROP INDEX IF EXISTS idx_agreement_users_telegram_user_id;
ALTER TABLE agreement_users DROP CONSTRAINT IF EXISTS unique_telegram_user_id;
