-- Your SQL goes here
CREATE TABLE chats (
    id SERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    telegram_id VARCHAR(255) NOT NULL,
    webhook_url VARCHAR(255) NOT NULL UNIQUE
);
