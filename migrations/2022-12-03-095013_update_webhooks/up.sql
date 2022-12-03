-- Your SQL goes here
-- one to one relationship between webhooks and chats
ALTER TABLE webhooks
    ADD COLUMN chat_id INTEGER REFERENCES chats(id) ON DELETE CASCADE;
