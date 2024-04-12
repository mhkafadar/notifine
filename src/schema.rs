// @generated automatically by Diesel CLI.

diesel::table! {
    chats (id) {
        id -> Int4,
        #[max_length = 255]
        name -> Varchar,
        #[max_length = 255]
        telegram_id -> Varchar,
        #[max_length = 255]
        webhook_url -> Varchar,
        #[max_length = 255]
        thread_id -> Nullable<Varchar>,
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

diesel::allow_tables_to_appear_in_same_query!(chats, trello_tokens, webhooks,);
