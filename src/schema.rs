// @generated automatically by Diesel CLI.

diesel::table! {
    agreement_conversation_states (id) {
        id -> Int4,
        telegram_user_id -> Int8,
        #[max_length = 50]
        state -> Varchar,
        state_data -> Nullable<Jsonb>,
        expires_at -> Timestamptz,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    agreement_users (id) {
        id -> Int4,
        telegram_user_id -> Int8,
        telegram_chat_id -> Int8,
        #[max_length = 255]
        username -> Nullable<Varchar>,
        #[max_length = 255]
        first_name -> Nullable<Varchar>,
        #[max_length = 255]
        last_name -> Nullable<Varchar>,
        #[max_length = 5]
        language -> Varchar,
        #[max_length = 50]
        timezone -> Varchar,
        disclaimer_accepted -> Bool,
        disclaimer_accepted_at -> Nullable<Timestamptz>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    agreements (id) {
        id -> Int4,
        user_id -> Int4,
        #[max_length = 20]
        agreement_type -> Varchar,
        #[max_length = 50]
        title -> Varchar,
        #[max_length = 20]
        user_role -> Nullable<Varchar>,
        start_date -> Nullable<Date>,
        #[max_length = 3]
        currency -> Varchar,
        rent_amount -> Nullable<Numeric>,
        due_day -> Nullable<Int4>,
        has_monthly_reminder -> Bool,
        #[max_length = 20]
        reminder_timing -> Nullable<Varchar>,
        reminder_days_before -> Nullable<Int4>,
        has_yearly_increase_reminder -> Bool,
        description -> Nullable<Text>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

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
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        is_active -> Bool,
        deactivated_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    daily_stats (id) {
        id -> Int4,
        date -> Date,
        messages_sent -> Int4,
        webhooks_received -> Int4,
        github_webhooks -> Int4,
        gitlab_webhooks -> Int4,
        beep_webhooks -> Int4,
        new_chats -> Int4,
        churned_chats -> Int4,
        uptime_checks -> Int4,
        uptime_failures -> Int4,
        errors_count -> Int4,
        created_at -> Timestamptz,
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
    reminders (id) {
        id -> Int4,
        agreement_id -> Int4,
        #[max_length = 20]
        reminder_type -> Varchar,
        #[max_length = 100]
        title -> Varchar,
        amount -> Nullable<Numeric>,
        due_date -> Date,
        reminder_date -> Date,
        #[max_length = 20]
        status -> Varchar,
        snooze_count -> Int4,
        snoozed_until -> Nullable<Timestamptz>,
        sent_at -> Nullable<Timestamptz>,
        completed_at -> Nullable<Timestamptz>,
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

diesel::joinable!(agreements -> agreement_users (user_id));
diesel::joinable!(reminders -> agreements (agreement_id));
diesel::joinable!(webhooks -> chats (chat_id));

diesel::allow_tables_to_appear_in_same_query!(
    agreement_conversation_states,
    agreement_users,
    agreements,
    chats,
    daily_stats,
    health_urls,
    reminders,
    tesla_auth,
    tesla_orders,
    trello_tokens,
    webhooks,
);
