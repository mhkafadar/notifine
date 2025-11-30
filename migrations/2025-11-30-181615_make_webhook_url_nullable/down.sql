UPDATE chats SET webhook_url = telegram_id WHERE webhook_url IS NULL;
ALTER TABLE chats ALTER COLUMN webhook_url SET NOT NULL;
ALTER TABLE chats ADD CONSTRAINT chats_webhook_url_key UNIQUE (webhook_url);
