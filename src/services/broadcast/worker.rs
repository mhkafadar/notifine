use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use teloxide::prelude::*;

use notifine::db::DbPool;

use super::bot_sender::BotSender;
use super::db::{
    create_pending_deactivation, get_all_broadcast_targets, get_pending_or_running_jobs,
    update_job_error, update_job_progress, update_job_status, update_job_total_chats,
    upsert_chat_bot_subscription,
};
use super::rate_limiter::RateLimiter;
use super::types::{BroadcastJob, BroadcastStatus};

const PROGRESS_REPORT_INTERVAL: i32 = 100;

pub struct BroadcastWorker {
    pool: DbPool,
    bot_sender: BotSender,
    shutdown: Arc<AtomicBool>,
    admin_chat_id: Option<i64>,
    admin_bot: Option<Bot>,
}

impl BroadcastWorker {
    pub fn new(pool: DbPool, admin_chat_id: Option<i64>) -> Self {
        let bot_sender = BotSender::new();

        let admin_bot = admin_chat_id.and_then(|_| {
            bot_sender
                .get_bot(super::types::BotType::Gitlab)
                .cloned()
                .or_else(|| bot_sender.get_bot(super::types::BotType::Uptime).cloned())
        });

        Self {
            pool,
            bot_sender,
            shutdown: Arc::new(AtomicBool::new(false)),
            admin_chat_id,
            admin_bot,
        }
    }

    pub fn shutdown_handle(&self) -> Arc<AtomicBool> {
        self.shutdown.clone()
    }

    pub async fn run(&self) {
        tracing::info!("Broadcast worker started");

        if !self.bot_sender.has_bots() {
            tracing::warn!("No bots available for broadcast worker");
            return;
        }

        loop {
            if self.shutdown.load(Ordering::Relaxed) {
                tracing::info!("Broadcast worker shutting down");
                break;
            }

            match get_pending_or_running_jobs(&self.pool) {
                Ok(jobs) => {
                    if let Some(job) = jobs.into_iter().next() {
                        self.process_job(job).await;
                    } else {
                        tokio::time::sleep(Duration::from_secs(5)).await;
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to fetch jobs: {:?}", e);
                    tokio::time::sleep(Duration::from_secs(10)).await;
                }
            }
        }
    }

    async fn process_job(&self, job: BroadcastJob) {
        tracing::info!("Processing broadcast job #{}", job.id);

        let targets = match get_all_broadcast_targets(&self.pool) {
            Ok(t) => t,
            Err(e) => {
                tracing::error!("Failed to get broadcast targets: {:?}", e);
                let _ =
                    update_job_error(&self.pool, job.id, &format!("Failed to get targets: {}", e));
                return;
            }
        };

        if targets.is_empty() {
            tracing::warn!("No broadcast targets found");
            let _ = update_job_error(&self.pool, job.id, "No targets found");
            return;
        }

        if let Err(e) = update_job_total_chats(&self.pool, job.id, targets.len() as i32) {
            tracing::error!("Failed to update job total: {:?}", e);
        }

        if let Err(e) = update_job_status(&self.pool, job.id, BroadcastStatus::Running) {
            tracing::error!("Failed to update job status: {:?}", e);
            return;
        }

        self.send_admin_message(&format!(
            "📢 Broadcast #{} started\nMessage: {}\nTargets: {} chats\nRate: {} msg/sec",
            job.id,
            truncate_message(&job.message, 100),
            targets.len(),
            job.rate_limit_per_sec
        ))
        .await;

        let mut rate_limiter = RateLimiter::new(job.rate_limit_per_sec as u32);

        let start_index = if let Some(last_id) = job.last_processed_chat_id {
            targets
                .iter()
                .position(|t| t.telegram_chat_id > last_id)
                .unwrap_or(0)
        } else {
            0
        };

        for (idx, target) in targets.iter().enumerate().skip(start_index) {
            if self.shutdown.load(Ordering::Relaxed) {
                tracing::info!("Broadcast job #{} paused due to shutdown", job.id);
                let _ = update_job_status(&self.pool, job.id, BroadcastStatus::Paused);
                return;
            }

            rate_limiter.acquire().await;

            let result = self
                .bot_sender
                .send_to_target(target, &job.message, job.discovery_mode)
                .await;

            for bot_type in &result.successful_bots {
                let _ = upsert_chat_bot_subscription(
                    &self.pool,
                    target.telegram_chat_id,
                    *bot_type,
                    true,
                );
            }

            let failed_bots: Vec<_> = result
                .attempted_bots
                .iter()
                .filter(|b| !result.successful_bots.contains(b))
                .copied()
                .collect();

            for bot_type in &failed_bots {
                let _ = upsert_chat_bot_subscription(
                    &self.pool,
                    target.telegram_chat_id,
                    *bot_type,
                    false,
                );
            }

            if !result.success {
                let _ = create_pending_deactivation(
                    &self.pool,
                    target.telegram_chat_id,
                    Some(job.id),
                    &failed_bots,
                    result.error_message.as_deref(),
                );
            }

            let unreachable = !result.success && !result.attempted_bots.is_empty();
            match update_job_progress(
                &self.pool,
                job.id,
                target.telegram_chat_id,
                result.success,
                unreachable,
            ) {
                Ok(updated_job) => {
                    if (idx + 1) % PROGRESS_REPORT_INTERVAL as usize == 0 {
                        self.send_progress_report(&updated_job).await;
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to update job progress: {:?}", e);
                }
            }
        }

        if let Err(e) = update_job_status(&self.pool, job.id, BroadcastStatus::Completed) {
            tracing::error!("Failed to mark job as completed: {:?}", e);
        }

        if let Ok(Some(final_job)) = super::db::get_job_by_id(&self.pool, job.id) {
            self.send_completion_report(&final_job).await;
        }
    }

    async fn send_progress_report(&self, job: &BroadcastJob) {
        let progress_pct = if job.total_chats > 0 {
            (job.processed_count as f64 / job.total_chats as f64 * 100.0) as u32
        } else {
            0
        };

        let message = format!(
            "📊 Broadcast #{} Progress\n\
            Processed: {}/{} ({}%)\n\
            ✅ Success: {} | ❌ Failed: {} | 🚫 Unreachable: {}",
            job.id,
            job.processed_count,
            job.total_chats,
            progress_pct,
            job.success_count,
            job.failed_count,
            job.unreachable_count
        );

        self.send_admin_message(&message).await;
    }

    async fn send_completion_report(&self, job: &BroadcastJob) {
        let success_pct = if job.total_chats > 0 {
            (job.success_count as f64 / job.total_chats as f64 * 100.0) as u32
        } else {
            0
        };

        let message = format!(
            "✅ Broadcast #{} Completed!\n\
            Total: {} chats\n\
            ✅ Success: {} ({}%)\n\
            ❌ Failed: {}\n\
            🚫 Unreachable: {}\n\n\
            Use /pendinglist to see unreachable chats.",
            job.id,
            job.total_chats,
            job.success_count,
            success_pct,
            job.failed_count,
            job.unreachable_count
        );

        self.send_admin_message(&message).await;
    }

    async fn send_admin_message(&self, message: &str) {
        if let (Some(bot), Some(chat_id)) = (&self.admin_bot, self.admin_chat_id) {
            if let Err(e) = bot.send_message(ChatId(chat_id), message).await {
                tracing::error!("Failed to send admin message: {:?}", e);
            }
        }
    }
}

fn truncate_message(msg: &str, max_len: usize) -> String {
    if msg.chars().count() <= max_len {
        msg.to_string()
    } else {
        format!("{}...", msg.chars().take(max_len).collect::<String>())
    }
}
