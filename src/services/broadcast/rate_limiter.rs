use std::time::Duration;
use tokio::time::Instant;

pub struct RateLimiter {
    rate_per_second: u32,
    interval: Duration,
    last_request: Option<Instant>,
}

impl RateLimiter {
    pub fn new(rate_per_second: u32) -> Self {
        let interval = if rate_per_second > 0 {
            Duration::from_millis(1000 / rate_per_second as u64)
        } else {
            Duration::from_millis(100)
        };

        Self {
            rate_per_second,
            interval,
            last_request: None,
        }
    }

    pub async fn acquire(&mut self) {
        if let Some(last) = self.last_request {
            let elapsed = last.elapsed();
            if elapsed < self.interval {
                tokio::time::sleep(self.interval - elapsed).await;
            }
        }
        self.last_request = Some(Instant::now());
    }

    pub fn rate_per_second(&self) -> u32 {
        self.rate_per_second
    }
}
