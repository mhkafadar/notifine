use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

pub struct Metrics {
    pub messages_sent: AtomicU64,
    pub webhooks_received: AtomicU64,
    pub github_webhooks: AtomicU64,
    pub gitlab_webhooks: AtomicU64,
    pub beep_webhooks: AtomicU64,
    pub github_messages_sent: AtomicU64,
    pub gitlab_messages_sent: AtomicU64,
    pub beep_messages_sent: AtomicU64,
    pub uptime_messages_sent: AtomicU64,
    pub agreement_messages_sent: AtomicU64,
    pub new_chats: AtomicU64,
    pub churned_chats: AtomicU64,
    pub uptime_checks: AtomicU64,
    pub uptime_failures: AtomicU64,
    pub errors: AtomicU64,
    pub start_time: Instant,
}

#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    pub messages_sent: u64,
    pub webhooks_received: u64,
    pub github_webhooks: u64,
    pub gitlab_webhooks: u64,
    pub beep_webhooks: u64,
    pub github_messages_sent: u64,
    pub gitlab_messages_sent: u64,
    pub beep_messages_sent: u64,
    pub uptime_messages_sent: u64,
    pub agreement_messages_sent: u64,
    pub new_chats: u64,
    pub churned_chats: u64,
    pub uptime_checks: u64,
    pub uptime_failures: u64,
    pub errors: u64,
    pub uptime_secs: u64,
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            messages_sent: AtomicU64::new(0),
            webhooks_received: AtomicU64::new(0),
            github_webhooks: AtomicU64::new(0),
            gitlab_webhooks: AtomicU64::new(0),
            beep_webhooks: AtomicU64::new(0),
            github_messages_sent: AtomicU64::new(0),
            gitlab_messages_sent: AtomicU64::new(0),
            beep_messages_sent: AtomicU64::new(0),
            uptime_messages_sent: AtomicU64::new(0),
            agreement_messages_sent: AtomicU64::new(0),
            new_chats: AtomicU64::new(0),
            churned_chats: AtomicU64::new(0),
            uptime_checks: AtomicU64::new(0),
            uptime_failures: AtomicU64::new(0),
            errors: AtomicU64::new(0),
            start_time: Instant::now(),
        }
    }

    pub fn increment_messages_sent(&self) {
        self.messages_sent.fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_messages_sent_for_bot(&self, bot_name: &str) {
        self.messages_sent.fetch_add(1, Ordering::Relaxed);
        match bot_name.to_lowercase().as_str() {
            "github" => self.github_messages_sent.fetch_add(1, Ordering::Relaxed),
            "gitlab" => self.gitlab_messages_sent.fetch_add(1, Ordering::Relaxed),
            "beep" => self.beep_messages_sent.fetch_add(1, Ordering::Relaxed),
            "uptime" => self.uptime_messages_sent.fetch_add(1, Ordering::Relaxed),
            "agreement" => self.agreement_messages_sent.fetch_add(1, Ordering::Relaxed),
            _ => 0,
        };
    }

    pub fn increment_webhooks(&self, source: &str) {
        self.webhooks_received.fetch_add(1, Ordering::Relaxed);
        match source.to_lowercase().as_str() {
            "github" => self.github_webhooks.fetch_add(1, Ordering::Relaxed),
            "gitlab" => self.gitlab_webhooks.fetch_add(1, Ordering::Relaxed),
            "beep" => self.beep_webhooks.fetch_add(1, Ordering::Relaxed),
            _ => 0,
        };
    }

    pub fn increment_new_chat(&self) {
        self.new_chats.fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_churn(&self) {
        self.churned_chats.fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_uptime_check(&self) {
        self.uptime_checks.fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_uptime_failure(&self) {
        self.uptime_failures.fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_errors(&self) {
        self.errors.fetch_add(1, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            messages_sent: self.messages_sent.load(Ordering::Relaxed),
            webhooks_received: self.webhooks_received.load(Ordering::Relaxed),
            github_webhooks: self.github_webhooks.load(Ordering::Relaxed),
            gitlab_webhooks: self.gitlab_webhooks.load(Ordering::Relaxed),
            beep_webhooks: self.beep_webhooks.load(Ordering::Relaxed),
            github_messages_sent: self.github_messages_sent.load(Ordering::Relaxed),
            gitlab_messages_sent: self.gitlab_messages_sent.load(Ordering::Relaxed),
            beep_messages_sent: self.beep_messages_sent.load(Ordering::Relaxed),
            uptime_messages_sent: self.uptime_messages_sent.load(Ordering::Relaxed),
            agreement_messages_sent: self.agreement_messages_sent.load(Ordering::Relaxed),
            new_chats: self.new_chats.load(Ordering::Relaxed),
            churned_chats: self.churned_chats.load(Ordering::Relaxed),
            uptime_checks: self.uptime_checks.load(Ordering::Relaxed),
            uptime_failures: self.uptime_failures.load(Ordering::Relaxed),
            errors: self.errors.load(Ordering::Relaxed),
            uptime_secs: self.start_time.elapsed().as_secs(),
        }
    }

    pub fn reset_daily_counters(&self) {
        self.messages_sent.store(0, Ordering::Relaxed);
        self.webhooks_received.store(0, Ordering::Relaxed);
        self.github_webhooks.store(0, Ordering::Relaxed);
        self.gitlab_webhooks.store(0, Ordering::Relaxed);
        self.beep_webhooks.store(0, Ordering::Relaxed);
        self.github_messages_sent.store(0, Ordering::Relaxed);
        self.gitlab_messages_sent.store(0, Ordering::Relaxed);
        self.beep_messages_sent.store(0, Ordering::Relaxed);
        self.uptime_messages_sent.store(0, Ordering::Relaxed);
        self.agreement_messages_sent.store(0, Ordering::Relaxed);
        self.new_chats.store(0, Ordering::Relaxed);
        self.churned_chats.store(0, Ordering::Relaxed);
        self.uptime_checks.store(0, Ordering::Relaxed);
        self.uptime_failures.store(0, Ordering::Relaxed);
        self.errors.store(0, Ordering::Relaxed);
    }

    pub fn snapshot_and_reset(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            messages_sent: self.messages_sent.swap(0, Ordering::Relaxed),
            webhooks_received: self.webhooks_received.swap(0, Ordering::Relaxed),
            github_webhooks: self.github_webhooks.swap(0, Ordering::Relaxed),
            gitlab_webhooks: self.gitlab_webhooks.swap(0, Ordering::Relaxed),
            beep_webhooks: self.beep_webhooks.swap(0, Ordering::Relaxed),
            github_messages_sent: self.github_messages_sent.swap(0, Ordering::Relaxed),
            gitlab_messages_sent: self.gitlab_messages_sent.swap(0, Ordering::Relaxed),
            beep_messages_sent: self.beep_messages_sent.swap(0, Ordering::Relaxed),
            uptime_messages_sent: self.uptime_messages_sent.swap(0, Ordering::Relaxed),
            agreement_messages_sent: self.agreement_messages_sent.swap(0, Ordering::Relaxed),
            new_chats: self.new_chats.swap(0, Ordering::Relaxed),
            churned_chats: self.churned_chats.swap(0, Ordering::Relaxed),
            uptime_checks: self.uptime_checks.swap(0, Ordering::Relaxed),
            uptime_failures: self.uptime_failures.swap(0, Ordering::Relaxed),
            errors: self.errors.swap(0, Ordering::Relaxed),
            uptime_secs: self.start_time.elapsed().as_secs(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_increment_metrics() {
        let metrics = Metrics::new();
        metrics.increment_messages_sent();
        metrics.increment_messages_sent();
        assert_eq!(metrics.snapshot().messages_sent, 2);
    }

    #[test]
    fn test_increment_webhooks_by_source() {
        let metrics = Metrics::new();
        metrics.increment_webhooks("github");
        metrics.increment_webhooks("github");
        metrics.increment_webhooks("gitlab");
        metrics.increment_webhooks("beep");

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.webhooks_received, 4);
        assert_eq!(snapshot.github_webhooks, 2);
        assert_eq!(snapshot.gitlab_webhooks, 1);
        assert_eq!(snapshot.beep_webhooks, 1);
    }

    #[test]
    fn test_reset_daily_counters() {
        let metrics = Metrics::new();
        metrics.increment_errors();
        metrics.increment_new_chat();
        metrics.reset_daily_counters();

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.errors, 0);
        assert_eq!(snapshot.new_chats, 0);
    }

    #[test]
    fn test_increment_messages_sent_by_bot() {
        let metrics = Metrics::new();
        metrics.increment_messages_sent_for_bot("github");
        metrics.increment_messages_sent_for_bot("gitlab");
        metrics.increment_messages_sent_for_bot("github");
        metrics.increment_messages_sent_for_bot("beep");
        metrics.increment_messages_sent_for_bot("uptime");
        metrics.increment_messages_sent_for_bot("agreement");

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.messages_sent, 6);
        assert_eq!(snapshot.github_messages_sent, 2);
        assert_eq!(snapshot.gitlab_messages_sent, 1);
        assert_eq!(snapshot.beep_messages_sent, 1);
        assert_eq!(snapshot.uptime_messages_sent, 1);
        assert_eq!(snapshot.agreement_messages_sent, 1);
    }

    #[test]
    fn test_increment_messages_case_insensitive() {
        let metrics = Metrics::new();
        metrics.increment_messages_sent_for_bot("GitHub");
        metrics.increment_messages_sent_for_bot("GITLAB");
        metrics.increment_messages_sent_for_bot("Beep");

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.messages_sent, 3);
        assert_eq!(snapshot.github_messages_sent, 1);
        assert_eq!(snapshot.gitlab_messages_sent, 1);
        assert_eq!(snapshot.beep_messages_sent, 1);
    }

    #[test]
    fn test_unknown_bot_name_increments_total() {
        let metrics = Metrics::new();
        metrics.increment_messages_sent_for_bot("unknown_bot");

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.messages_sent, 1);
        assert_eq!(snapshot.github_messages_sent, 0);
        assert_eq!(snapshot.gitlab_messages_sent, 0);
    }

    #[test]
    fn test_snapshot_and_reset() {
        let metrics = Metrics::new();
        metrics.increment_messages_sent_for_bot("github");
        metrics.increment_messages_sent_for_bot("gitlab");
        metrics.increment_new_chat();
        metrics.increment_errors();

        let snapshot = metrics.snapshot_and_reset();
        assert_eq!(snapshot.messages_sent, 2);
        assert_eq!(snapshot.github_messages_sent, 1);
        assert_eq!(snapshot.gitlab_messages_sent, 1);
        assert_eq!(snapshot.new_chats, 1);
        assert_eq!(snapshot.errors, 1);

        let snapshot_after_reset = metrics.snapshot();
        assert_eq!(snapshot_after_reset.messages_sent, 0);
        assert_eq!(snapshot_after_reset.github_messages_sent, 0);
        assert_eq!(snapshot_after_reset.gitlab_messages_sent, 0);
        assert_eq!(snapshot_after_reset.new_chats, 0);
        assert_eq!(snapshot_after_reset.errors, 0);
    }
}
