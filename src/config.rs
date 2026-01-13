use std::env;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub webhook_base_url: String,

    pub port: u16,
    pub admin_logs: String,
    pub admin_chat_id: Option<i64>,
    pub admin_log_level: u8,

    pub gitlab_token: Option<String>,
    pub github_token: Option<String>,
    pub beep_token: Option<String>,
    pub uptime_token: Option<String>,
    pub agreement_bot_token: Option<String>,
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

fn get_optional(name: &str) -> Option<String> {
    env::var(name).ok().filter(|v| !v.is_empty())
}

impl AppConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        let mut missing = Vec::new();
        let mut invalid = Vec::new();

        let database_url = get_required("DATABASE_URL", &mut missing);
        let webhook_base_url = get_required("WEBHOOK_BASE_URL", &mut missing);

        let admin_logs = get_optional("ADMIN_LOGS").unwrap_or_else(|| "NOT_ACTIVE".into());

        let admin_chat_id = get_optional("TELEGRAM_ADMIN_CHAT_ID").and_then(|s| {
            s.parse::<i64>()
                .map_err(|e| {
                    invalid.push(("TELEGRAM_ADMIN_CHAT_ID".into(), e.to_string()));
                })
                .ok()
        });

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

        let gitlab_token = get_optional("GITLAB_TELOXIDE_TOKEN");
        let github_token = get_optional("GITHUB_TELOXIDE_TOKEN");
        let beep_token = get_optional("BEEP_TELOXIDE_TOKEN");
        let uptime_token = get_optional("UPTIME_TELOXIDE_TOKEN");
        let agreement_bot_token = get_optional("AGREEMENT_BOT_TOKEN");

        let has_any_bot_token = gitlab_token.is_some()
            || github_token.is_some()
            || beep_token.is_some()
            || uptime_token.is_some()
            || agreement_bot_token.is_some();

        if !has_any_bot_token {
            invalid.push((
                "BOT_TOKENS".into(),
                "At least one bot token must be configured".into(),
            ));
        }

        if !missing.is_empty() || !invalid.is_empty() {
            return Err(ConfigError {
                missing_vars: missing,
                invalid_vars: invalid,
            });
        }

        Ok(Self {
            database_url: database_url.unwrap(),
            webhook_base_url: webhook_base_url.unwrap(),
            port,
            admin_logs,
            admin_chat_id,
            admin_log_level,
            gitlab_token,
            github_token,
            beep_token,
            uptime_token,
            agreement_bot_token,
        })
    }

    pub fn is_admin_logs_active(&self) -> bool {
        self.admin_logs == "ACTIVE"
    }
}
