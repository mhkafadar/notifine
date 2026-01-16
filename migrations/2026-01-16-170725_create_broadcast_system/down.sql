-- Rollback broadcast system

DROP INDEX IF EXISTS idx_pending_deactivations_status;
DROP INDEX IF EXISTS idx_broadcast_jobs_status;
DROP INDEX IF EXISTS idx_chat_bot_subs_reachable;
DROP INDEX IF EXISTS idx_chat_bot_subs_telegram_id;

DROP TABLE IF EXISTS pending_deactivations;
DROP TABLE IF EXISTS broadcast_jobs;
DROP TABLE IF EXISTS chat_bot_subscriptions;
