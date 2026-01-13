use std::env;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub gitlab_token: String,
    pub github_token: String,
    pub beep_token: String,
    pub uptime_token: String,
    pub agreement_bot_token: String,
    pub webhook_base_url: String,
    pub admin_logs: String,
    pub admin_chat_id: i64,
    pub admin_log_level: u8,
    pub port: u16,
}

#[derive(Debug)]
pub struct ConfigError {
    pub missing_vars: Vec<String>,
    pub invalid_vars: Vec<(String, String)>,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if !self.missing_vars.is_empty() {
            writeln!(f, "Missing required environment variables:")?;
            for var in &self.missing_vars {
                writeln!(f, "  - {}", var)?;
            }
        }
        if !self.invalid_vars.is_empty() {
            writeln!(f, "Invalid environment variables:")?;
            for (var, err) in &self.invalid_vars {
                writeln!(f, "  - {}: {}", var, err)?;
            }
        }
        Ok(())
    }
}

impl std::error::Error for ConfigError {}

fn get_required(name: &str, missing: &mut Vec<String>) -> Option<String> {
    match env::var(name) {
        Ok(v) if !v.is_empty() => Some(v),
        _ => {
            missing.push(name.to_string());
            None
        }
    }
}

impl AppConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        let mut missing = Vec::new();
        let mut invalid = Vec::new();

        let database_url = get_required("DATABASE_URL", &mut missing);
        let gitlab_token = get_required("GITLAB_TELOXIDE_TOKEN", &mut missing);
        let github_token = get_required("GITHUB_TELOXIDE_TOKEN", &mut missing);
        let beep_token = get_required("BEEP_TELOXIDE_TOKEN", &mut missing);
        let uptime_token = get_required("UPTIME_TELOXIDE_TOKEN", &mut missing);
        let agreement_bot_token = get_required("AGREEMENT_BOT_TOKEN", &mut missing);
        let webhook_base_url = get_required("WEBHOOK_BASE_URL", &mut missing);
        let admin_logs = get_required("ADMIN_LOGS", &mut missing);
        let admin_chat_id_str = get_required("TELEGRAM_ADMIN_CHAT_ID", &mut missing);

        let admin_chat_id = admin_chat_id_str
            .as_ref()
            .and_then(|s| {
                s.parse::<i64>()
                    .map_err(|e| {
                        invalid.push(("TELEGRAM_ADMIN_CHAT_ID".into(), e.to_string()));
                    })
                    .ok()
            })
            .unwrap_or(0);

        let admin_log_level = env::var("ADMIN_LOG_LEVEL")
            .unwrap_or_else(|_| "50".into())
            .parse::<u8>()
            .unwrap_or(50);

        let port = env::var("PORT")
            .unwrap_or_else(|_| "8080".into())
            .parse::<u16>()
            .map_err(|e| {
                invalid.push(("PORT".into(), e.to_string()));
            })
            .unwrap_or(8080);

        if !missing.is_empty() || !invalid.is_empty() {
            return Err(ConfigError {
                missing_vars: missing,
                invalid_vars: invalid,
            });
        }

        Ok(Self {
            database_url: database_url.unwrap(),
            gitlab_token: gitlab_token.unwrap(),
            github_token: github_token.unwrap(),
            beep_token: beep_token.unwrap(),
            uptime_token: uptime_token.unwrap(),
            agreement_bot_token: agreement_bot_token.unwrap(),
            webhook_base_url: webhook_base_url.unwrap(),
            admin_logs: admin_logs.unwrap(),
            admin_chat_id,
            admin_log_level,
            port,
        })
    }

    pub fn is_admin_logs_active(&self) -> bool {
        self.admin_logs == "ACTIVE"
    }
}
