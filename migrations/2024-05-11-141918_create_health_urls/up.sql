-- Your SQL goes here
CREATE TABLE health_urls (
    id SERIAL PRIMARY KEY,
    url TEXT NOT NULL,
    chat_id INT NOT NULL,
    status_code INT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
