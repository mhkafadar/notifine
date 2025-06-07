-- Create table for Tesla authentication tokens
CREATE TABLE tesla_auth (
    id SERIAL PRIMARY KEY,
    chat_id BIGINT NOT NULL UNIQUE,
    access_token TEXT NOT NULL,
    refresh_token TEXT NOT NULL,
    expires_in BIGINT NOT NULL,
    token_type TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW()
);

-- Create index on chat_id for faster lookups
CREATE INDEX idx_tesla_auth_chat_id ON tesla_auth(chat_id);

-- Create table for Tesla order cache
CREATE TABLE tesla_orders (
    id SERIAL PRIMARY KEY,
    chat_id BIGINT NOT NULL,
    order_data JSONB NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    FOREIGN KEY (chat_id) REFERENCES tesla_auth(chat_id) ON DELETE CASCADE
);

-- Create index on chat_id for faster lookups
CREATE INDEX idx_tesla_orders_chat_id ON tesla_orders(chat_id);