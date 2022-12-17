-- Your SQL goes here
CREATE TABLE trello_tokens (
    id SERIAL PRIMARY KEY,
    access_token VARCHAR(255),
    access_token_secret VARCHAR(255),
    token_key VARCHAR(255),
    token_secret VARCHAR(255),
    telegram_user_id VARCHAR(255)
);
