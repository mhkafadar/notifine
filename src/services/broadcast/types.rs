use chrono::{DateTime, Utc};
use diesel::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BotType {
    Gitlab,
    Github,
    Beep,
    Uptime,
    Agreement,
}

impl BotType {
    pub fn as_str(&self) -> &'static str {
        match self {
            BotType::Gitlab => "gitlab",
            BotType::Github => "github",
            BotType::Beep => "beep",
            BotType::Uptime => "uptime",
            BotType::Agreement => "agreement",
        }
    }

    pub fn parse(s: &str) -> Option<BotType> {
        match s.to_lowercase().as_str() {
            "gitlab" => Some(BotType::Gitlab),
            "github" => Some(BotType::Github),
            "beep" => Some(BotType::Beep),
            "uptime" => Some(BotType::Uptime),
            "agreement" => Some(BotType::Agreement),
            _ => None,
        }
    }

    pub fn env_token_name(&self) -> &'static str {
        match self {
            BotType::Gitlab => "GITLAB_TELOXIDE_TOKEN",
            BotType::Github => "GITHUB_TELOXIDE_TOKEN",
            BotType::Beep => "BEEP_TELOXIDE_TOKEN",
            BotType::Uptime => "UPTIME_TELOXIDE_TOKEN",
            BotType::Agreement => "AGREEMENT_BOT_TOKEN",
        }
    }

    pub fn all_ordered() -> Vec<BotType> {
        vec![
            BotType::Gitlab,
            BotType::Github,
            BotType::Beep,
            BotType::Uptime,
            BotType::Agreement,
        ]
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            BotType::Gitlab => "GitLab",
            BotType::Github => "GitHub",
            BotType::Beep => "Beep",
            BotType::Uptime => "Uptime",
            BotType::Agreement => "Agreement",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BroadcastStatus {
    Pending,
    Running,
    Paused,
    Completed,
    Cancelled,
    Failed,
}

impl BroadcastStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            BroadcastStatus::Pending => "pending",
            BroadcastStatus::Running => "running",
            BroadcastStatus::Paused => "paused",
            BroadcastStatus::Completed => "completed",
            BroadcastStatus::Cancelled => "cancelled",
            BroadcastStatus::Failed => "failed",
        }
    }

    pub fn parse(s: &str) -> Option<BroadcastStatus> {
        match s {
            "pending" => Some(BroadcastStatus::Pending),
            "running" => Some(BroadcastStatus::Running),
            "paused" => Some(BroadcastStatus::Paused),
            "completed" => Some(BroadcastStatus::Completed),
            "cancelled" => Some(BroadcastStatus::Cancelled),
            "failed" => Some(BroadcastStatus::Failed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeactivationStatus {
    Pending,
    Approved,
    Rejected,
}

impl DeactivationStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            DeactivationStatus::Pending => "pending",
            DeactivationStatus::Approved => "approved",
            DeactivationStatus::Rejected => "rejected",
        }
    }
}

#[derive(Debug, Clone, Queryable)]
pub struct BroadcastJob {
    pub id: i32,
    pub message: String,
    pub status: String,
    pub created_by_chat_id: i64,
    pub total_chats: i32,
    pub processed_count: i32,
    pub success_count: i32,
    pub failed_count: i32,
    pub unreachable_count: i32,
    pub last_processed_chat_id: Option<i64>,
    pub rate_limit_per_sec: i32,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub discovery_mode: bool,
}

impl BroadcastJob {
    pub fn status_enum(&self) -> Option<BroadcastStatus> {
        BroadcastStatus::parse(&self.status)
    }
}

#[derive(Debug, Clone, Queryable)]
pub struct ChatBotSubscription {
    pub id: i32,
    pub telegram_chat_id: i64,
    pub bot_type: String,
    pub is_reachable: bool,
    pub last_success_at: Option<DateTime<Utc>>,
    pub last_failure_at: Option<DateTime<Utc>>,
    pub failure_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ChatBotSubscription {
    pub fn bot_type_enum(&self) -> Option<BotType> {
        BotType::parse(&self.bot_type)
    }
}

#[derive(Debug, Clone, Queryable)]
pub struct PendingDeactivation {
    pub id: i32,
    pub telegram_chat_id: i64,
    pub source_broadcast_job_id: Option<i32>,
    pub failed_bots: Vec<Option<String>>,
    pub last_error: Option<String>,
    pub status: String,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub reviewed_by_chat_id: Option<i64>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct BroadcastTarget {
    pub telegram_chat_id: i64,
    pub thread_id: Option<i32>,
    pub known_working_bots: Vec<BotType>,
}

#[derive(Debug, Clone)]
pub struct DeliveryResult {
    pub telegram_chat_id: i64,
    pub success: bool,
    pub successful_bot: Option<BotType>,
    pub successful_bots: Vec<BotType>,
    pub attempted_bots: Vec<BotType>,
    pub error_message: Option<String>,
}

#[derive(Debug)]
pub enum BroadcastError {
    Database(String),
    AllBotsUnavailable,
    Cancelled,
    NoPendingJobs,
}

impl std::fmt::Display for BroadcastError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BroadcastError::Database(e) => write!(f, "Database error: {}", e),
            BroadcastError::AllBotsUnavailable => write!(f, "No bots available"),
            BroadcastError::Cancelled => write!(f, "Broadcast cancelled"),
            BroadcastError::NoPendingJobs => write!(f, "No pending jobs"),
        }
    }
}

impl std::error::Error for BroadcastError {}
