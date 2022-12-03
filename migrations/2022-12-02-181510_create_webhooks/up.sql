-- Your SQL goes here
CREATE TABLE webhooks (
   id SERIAL PRIMARY KEY,
   name VARCHAR(255) NOT NULL,
   webhook_url VARCHAR(255) NOT NULL UNIQUE
);
