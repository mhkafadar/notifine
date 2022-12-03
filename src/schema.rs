// @generated automatically by Diesel CLI.

diesel::table! {
    chats (id) {
        id -> Int4,
        name -> Varchar,
        telegram_id -> Varchar,
        webhook_url -> Varchar,
    }
}

diesel::table! {
    webhooks (id) {
        id -> Int4,
        name -> Varchar,
        webhook_url -> Varchar,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        chat_id -> Nullable<Int4>,
    }
}

diesel::joinable!(webhooks -> chats (chat_id));

diesel::allow_tables_to_appear_in_same_query!(
    chats,
    webhooks,
);
