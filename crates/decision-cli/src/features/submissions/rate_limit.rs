//! Per-repo token-bucket rate limit for `POST /submissions` (FT-094).
//!
//! Slice 1 ships a loose default (60 req / minute / repo). Slice 3+
//! moves the policy onto a graph-resident artifact so the meta-loop
//! can revise it without a code change.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::auth::RepoIdentity;

/// Slice-1 default bucket capacity per repo (requests / minute).
pub const DEFAULT_RATE_LIMIT_PER_MINUTE: u32 = 60;

/// Knobs for [`RateLimiter`]. Cloneable so axum handlers can hand it
/// off to multiple state shards.
#[derive(Debug, Clone, Copy)]
pub struct RateLimitConfig {
    /// Bucket capacity (max requests in one full window).
    pub capacity: u32,
    /// Refill window. Each elapsed window adds `capacity` tokens.
    pub window: Duration,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            capacity: DEFAULT_RATE_LIMIT_PER_MINUTE,
            window: Duration::from_secs(60),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct BucketState {
    tokens: u32,
    last_refill: Instant,
}

/// Per-repo token-bucket rate limiter. Refills `capacity` tokens every
/// `window` interval.
pub struct RateLimiter {
    inner: Mutex<HashMap<String, BucketState>>,
    config: RateLimitConfig,
}

impl RateLimiter {
    /// Construct a limiter from explicit config.
    #[must_use]
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
            config,
        }
    }

    /// Slice-1 default (60 req / minute).
    #[must_use]
    pub fn with_default_policy() -> Self {
        Self::new(RateLimitConfig::default())
    }

    /// A pathological limiter for tests: zero capacity, so every call
    /// rejects. Used to drive the 429 path deterministically.
    #[must_use]
    pub fn deny_all() -> Self {
        Self::new(RateLimitConfig {
            capacity: 0,
            window: Duration::from_secs(60),
        })
    }

    /// Attempt to consume one token for `identity`. Returns `true` on a
    /// hit, `false` when the bucket is empty.
    pub fn try_acquire(&self, identity: &RepoIdentity) -> bool {
        let now = Instant::now();
        let mut guard = match self.inner.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        let key = identity.as_str().to_string();
        let entry = guard.entry(key).or_insert(BucketState {
            tokens: self.config.capacity,
            last_refill: now,
        });
        refill_bucket(entry, now, self.config);
        if entry.tokens == 0 {
            false
        } else {
            entry.tokens -= 1;
            true
        }
    }
}

fn refill_bucket(state: &mut BucketState, now: Instant, config: RateLimitConfig) {
    if config.window.is_zero() {
        return;
    }
    let elapsed = now.duration_since(state.last_refill);
    if elapsed < config.window {
        return;
    }
    state.tokens = config.capacity;
    state.last_refill = now;
}

#[cfg(test)]
mod rate_limit_unit_tests {
    use super::*;

    #[test]
    fn rate_limiter_admits_within_capacity() {
        let id = RepoIdentity::new("https://github.com/example/repo").expect("id");
        let limiter = RateLimiter::new(RateLimitConfig {
            capacity: 2,
            window: Duration::from_secs(60),
        });
        assert!(limiter.try_acquire(&id));
        assert!(limiter.try_acquire(&id));
        assert!(!limiter.try_acquire(&id));
    }

    #[test]
    fn deny_all_limiter_refuses_immediately() {
        let id = RepoIdentity::new("https://github.com/example/repo").expect("id");
        let limiter = RateLimiter::deny_all();
        assert!(!limiter.try_acquire(&id));
    }
}
