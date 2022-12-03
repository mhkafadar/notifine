-- Your SQL goes here
DROP trigger set_timestamp on webhooks;
DROP function trigger_set_timestamp();

ALTER TABLE webhooks
    DROP COLUMN created_at,
    DROP COLUMN updated_at;
