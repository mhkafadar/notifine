-- This file should undo anything in `up.sql`
ALTER TABLE webhooks
    DROP CONSTRAINT webhooks_chat_id_fkey;
