use crate::adapter::StreamError;

use reqwest::{Client, Response};
use std::sync::LazyLock;
use std::time::{Duration, Instant};

static HTTP_CLIENT: LazyLock<Client> = LazyLock::new(Client::new);

pub trait RateLimiter: Send + Sync {
    /// Prepare for a request with given weight. Returns wait time if needed.
    fn prepare_request(&mut self, weight: usize) -> Option<Duration>;

    /// Update the limiter with response data (e.g., rate limit headers)
    fn update_from_response(&mut self, response: &Response, weight: usize);

    /// Check if response indicates rate limiting and should exit
    fn should_exit_on_response(&self, response: &Response) -> bool;
}

pub async fn http_request_with_limiter<L: RateLimiter>(
    url: &str,
    limiter: &tokio::sync::Mutex<L>,
    weight: usize,
) -> Result<String, StreamError> {
    let mut limiter_guard = limiter.lock().await;

    if let Some(wait_time) = limiter_guard.prepare_request(weight) {
        tokio::time::sleep(wait_time).await;
    }

    let response = HTTP_CLIENT
        .get(url)
        .send()
        .await
        .map_err(StreamError::FetchError)?;

    if limiter_guard.should_exit_on_response(&response) {
        eprintln!("Rate limit exceeded for: {}", url);
        std::process::exit(1);
    }

    limiter_guard.update_from_response(&response, weight);

    response.text().await.map_err(StreamError::FetchError)
}

/// Limiter for a fixed window rate
pub struct FixedWindowBucket {
    max_tokens: usize,
    available_tokens: usize,
    last_refill: Instant,
    refill_rate: Duration,
}

impl FixedWindowBucket {
    pub fn new(max_tokens: usize, refill_rate: Duration) -> Self {
        Self {
            max_tokens,
            available_tokens: max_tokens,
            last_refill: Instant::now(),
            refill_rate,
        }
    }

    fn refill(&mut self) {
        if let Ok(current_time) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        {
            let now = Instant::now();
            let period_seconds = self.refill_rate.as_secs();
            let seconds_in_current_period = current_time.as_secs() % period_seconds;

            let elapsed = now.duration_since(self.last_refill);
            if elapsed >= self.refill_rate || seconds_in_current_period < 1 {
                self.available_tokens = self.max_tokens;
                self.last_refill = now;
            }
        }
    }

    pub fn calculate_wait_time(&mut self, tokens: usize) -> Option<Duration> {
        self.refill();

        if self.available_tokens >= tokens {
            self.available_tokens -= tokens;
            return None;
        }

        let wait_time = self
            .refill_rate
            .saturating_sub(Instant::now().duration_since(self.last_refill));
        Some(wait_time)
    }

    pub fn consume_tokens(&mut self, tokens: usize) {
        self.refill();
        self.available_tokens -= tokens.min(self.available_tokens);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DynamicLimitReason {
    HeaderRate,
    FixedWindowRate,
}

/// Limiter that can be used when source reports the rate-limit usage
///
/// Can fallback to fixed window bucket
pub struct DynamicBucket {
    max_weight: usize,
    current_used_weight: usize,
    last_updated: Instant,
    fallback_bucket: FixedWindowBucket,
}

impl DynamicBucket {
    pub fn new(max_weight: usize) -> Self {
        Self {
            max_weight,
            current_used_weight: 0,
            last_updated: Instant::now(),
            fallback_bucket: FixedWindowBucket::new(max_weight, Duration::from_secs(60)),
        }
    }

    pub fn update_weight(&mut self, new_weight: usize) {
        if new_weight > 0 {
            self.current_used_weight = new_weight;
            self.last_updated = Instant::now();
        }
    }

    pub fn prepare_request(
        &mut self,
        weight: usize,
    ) -> (Option<Duration>, Option<DynamicLimitReason>) {
        let now = Instant::now();
        let elapsed_since_last_update = now.duration_since(self.last_updated);
        let can_use_header_data =
            elapsed_since_last_update <= Duration::from_secs(60) && self.current_used_weight > 0;

        if can_use_header_data {
            let available_weight = self.max_weight.saturating_sub(self.current_used_weight);

            if available_weight >= weight {
                (None, None)
            } else if let Ok(current_time) =
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            {
                let seconds_in_current_minute = current_time.as_secs() % 60;
                let time_until_next_minute = Duration::from_secs(60 - seconds_in_current_minute);

                let wait_time = time_until_next_minute.saturating_add(Duration::from_millis(100));
                (Some(wait_time), Some(DynamicLimitReason::HeaderRate))
            } else {
                (None, Some(DynamicLimitReason::HeaderRate))
            }
        } else {
            match self.fallback_bucket.calculate_wait_time(weight) {
                None => (None, None),
                Some(wait_time) => (Some(wait_time), Some(DynamicLimitReason::FixedWindowRate)),
            }
        }
    }
}
