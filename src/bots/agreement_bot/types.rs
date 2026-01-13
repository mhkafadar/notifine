use serde::{Deserialize, Serialize};

pub const DEFAULT_LANGUAGE: &str = "tr";
pub const DEFAULT_TIMEZONE: &str = "Europe/Istanbul";
pub const STATE_EXPIRY_MINUTES: i64 = 30;
pub const MAX_REMINDERS_PER_AGREEMENT: usize = 20;

pub fn sanitize_input(text: &str) -> String {
    text.trim()
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .collect::<String>()
        .trim()
        .to_string()
}

pub mod states {
    pub const RENT_SCOPE_CHECK: &str = "rent_scope_check";
    pub const RENT_TITLE: &str = "rent_title";
    pub const RENT_ROLE: &str = "rent_role";
    pub const RENT_START_DATE: &str = "rent_start_date";
    pub const RENT_AMOUNT: &str = "rent_amount";
    pub const RENT_CURRENCY: &str = "rent_currency";
    pub const RENT_DUE_DAY: &str = "rent_due_day";
    pub const RENT_MONTHLY_REMINDER: &str = "rent_monthly_reminder";
    pub const RENT_REMINDER_TIMING: &str = "rent_reminder_timing";
    pub const RENT_YEARLY_INCREASE: &str = "rent_yearly_increase";
    pub const RENT_SUMMARY: &str = "rent_summary";
    pub const CUSTOM_TITLE: &str = "custom_title";
    pub const CUSTOM_DESCRIPTION: &str = "custom_description";
    pub const CUSTOM_REMINDER_TITLE: &str = "custom_reminder_title";
    pub const CUSTOM_REMINDER_DATE: &str = "custom_reminder_date";
    pub const CUSTOM_REMINDER_AMOUNT: &str = "custom_reminder_amount";
    pub const CUSTOM_REMINDER_TIMING: &str = "custom_reminder_timing";
    pub const CUSTOM_REMINDER_LIST: &str = "custom_reminder_list";
    pub const CUSTOM_SUMMARY: &str = "custom_summary";
    pub const EDIT_TITLE: &str = "edit_title";
    pub const EDIT_AMOUNT: &str = "edit_amount";
    pub const EDIT_DUE_DAY: &str = "edit_due_day";
    pub const EDIT_DESCRIPTION: &str = "edit_description";
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RentDraft {
    pub title: Option<String>,
    pub user_role: Option<String>,
    pub start_date: Option<String>,
    pub currency: Option<String>,
    pub rent_amount: Option<String>,
    pub due_day: Option<i32>,
    pub has_monthly_reminder: Option<bool>,
    pub reminder_timing: Option<String>,
    pub has_yearly_increase_reminder: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CustomDraft {
    pub title: Option<String>,
    pub description: Option<String>,
    pub currency: Option<String>,
    pub reminders: Vec<CustomReminderDraft>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CustomReminderDraft {
    pub title: Option<String>,
    pub date: Option<String>,
    pub amount: Option<String>,
    pub timing: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EditDraft {
    pub agreement_id: i32,
    pub field: Option<String>,
}
