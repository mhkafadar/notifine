// @generated automatically by Diesel CLI.

diesel::table! {
    chats (id) {
        id -> Int4,
        #[max_length = 255]
        name -> Varchar,
        #[max_length = 255]
        telegram_id -> Varchar,
        #[max_length = 255]
        webhook_url -> Nullable<Varchar>,
        #[max_length = 255]
        thread_id -> Nullable<Varchar>,
        #[max_length = 5]
        language -> Varchar,
    }
}

diesel::table! {
    health_urls (id) {
        id -> Int4,
        url -> Text,
        chat_id -> Int4,
        status_code -> Int4,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    tesla_auth (id) {
        id -> Int4,
        chat_id -> Int8,
        access_token -> Text,
        refresh_token -> Text,
        expires_in -> Int8,
        token_type -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        monitoring_enabled -> Bool,
    }
}

diesel::table! {
    tesla_orders (id) {
        id -> Int4,
        chat_id -> Int8,
        order_data -> Jsonb,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    trello_tokens (id) {
        id -> Int4,
        #[max_length = 255]
        access_token -> Nullable<Varchar>,
        #[max_length = 255]
        access_token_secret -> Nullable<Varchar>,
        #[max_length = 255]
        token_key -> Nullable<Varchar>,
        #[max_length = 255]
        token_secret -> Nullable<Varchar>,
        #[max_length = 255]
        telegram_user_id -> Nullable<Varchar>,
    }
}

diesel::table! {
    webhooks (id) {
        id -> Int4,
        #[max_length = 255]
        name -> Varchar,
        #[max_length = 255]
        webhook_url -> Varchar,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        chat_id -> Nullable<Int4>,
    }
}

diesel::joinable!(webhooks -> chats (chat_id));

diesel::allow_tables_to_appear_in_same_query!(
    chats,
    health_urls,
    tesla_auth,
    tesla_orders,
    trello_tokens,
    webhooks,
);
