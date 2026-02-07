use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use rocket::fairing::{Fairing, Info, Kind};
use rocket::http::Header;
use rocket::request::{FromRequest, Outcome, Request};
use rocket::Response;

/// Fixed-window rate limiter keyed by arbitrary string (e.g. client IP).
///
/// Each key gets a counter that resets every `window` duration.
pub struct RateLimiter {
    window: Duration,
    default_limit: u64,
    /// key → (window_start, count)
    buckets: Mutex<HashMap<String, (Instant, u64)>>,
}

/// Client IP address extracted from the request.
///
/// Checks (in order):
/// 1. `X-Forwarded-For` header (first IP — set by reverse proxies / Cloudflare Tunnel)
/// 2. `X-Real-Ip` header
/// 3. Socket peer address
///
/// Falls back to "unknown" if none are available.
#[derive(Debug, Clone)]
pub struct ClientIp(pub String);

#[rocket::async_trait]
impl<'r> FromRequest<'r> for ClientIp {
    type Error = std::convert::Infallible;

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        // 1. X-Forwarded-For (first entry is the real client)
        if let Some(xff) = request.headers().get_one("X-Forwarded-For") {
            if let Some(first_ip) = xff.split(',').next() {
                let ip = first_ip.trim();
                if !ip.is_empty() {
                    return Outcome::Success(ClientIp(ip.to_string()));
                }
            }
        }

        // 2. X-Real-Ip
        if let Some(real_ip) = request.headers().get_one("X-Real-Ip") {
            let ip = real_ip.trim();
            if !ip.is_empty() {
                return Outcome::Success(ClientIp(ip.to_string()));
            }
        }

        // 3. Socket peer address
        if let Some(addr) = request.client_ip() {
            return Outcome::Success(ClientIp(addr.to_string()));
        }

        Outcome::Success(ClientIp("unknown".to_string()))
    }
}

/// Result of a rate limit check.
/// Stored in request-local state so the response fairing can attach headers.
#[derive(Clone)]
pub struct RateLimitResult {
    /// Whether the request is allowed.
    pub allowed: bool,
    /// Configured limit for this key.
    pub limit: u64,
    /// Requests remaining in the current window (used by headers fairing + tests).
    #[allow(dead_code)]
    pub remaining: u64,
    /// Seconds until the current window resets.
    pub reset_secs: u64,
}

/// Rocket fairing that attaches rate limit headers to every response.
/// Reads `RateLimitResult` from request-local state (set by the auth guard).
/// Currently unused — will be wired up when more endpoints need rate limit headers.
#[allow(dead_code)]
pub struct RateLimitHeaders;

#[rocket::async_trait]
impl Fairing for RateLimitHeaders {
    fn info(&self) -> Info {
        Info {
            name: "Rate Limit Response Headers",
            kind: Kind::Response,
        }
    }

    async fn on_response<'r>(&self, request: &'r Request<'_>, response: &mut Response<'r>) {
        if let Some(rl) = request.local_cache(|| Option::<RateLimitResult>::None) {
            response.set_header(Header::new("X-RateLimit-Limit", rl.limit.to_string()));
            response.set_header(Header::new(
                "X-RateLimit-Remaining",
                rl.remaining.to_string(),
            ));
            response.set_header(Header::new("X-RateLimit-Reset", rl.reset_secs.to_string()));
        }
    }
}

impl RateLimiter {
    /// Create a new rate limiter with the given window duration and default limit.
    pub fn new(window: Duration, default_limit: u64) -> Self {
        RateLimiter {
            window,
            default_limit,
            buckets: Mutex::new(HashMap::new()),
        }
    }

    /// Check (and consume) one request for `key_id` using the default limit.
    pub fn check_default(&self, key_id: &str) -> RateLimitResult {
        self.check(key_id, self.default_limit)
    }

    /// Check (and consume) one request for `key_id` with the given `limit`.
    ///
    /// Returns a `RateLimitResult` indicating whether the request is allowed
    /// and the current rate limit state for response headers.
    pub fn check(&self, key_id: &str, limit: u64) -> RateLimitResult {
        let now = Instant::now();
        let mut buckets = self.buckets.lock().unwrap();

        let entry = buckets
            .entry(key_id.to_string())
            .or_insert_with(|| (now, 0));

        // If the window has elapsed, reset.
        if now.duration_since(entry.0) >= self.window {
            *entry = (now, 0);
        }

        let reset_secs = self
            .window
            .checked_sub(now.duration_since(entry.0))
            .unwrap_or(Duration::ZERO)
            .as_secs();

        if entry.1 >= limit {
            RateLimitResult {
                allowed: false,
                limit,
                remaining: 0,
                reset_secs,
            }
        } else {
            entry.1 += 1;
            RateLimitResult {
                allowed: true,
                limit,
                remaining: limit.saturating_sub(entry.1),
                reset_secs,
            }
        }
    }

    /// Periodically prune stale entries to prevent unbounded memory growth.
    #[allow(dead_code)]
    pub fn prune_stale(&self) {
        let now = Instant::now();
        let mut buckets = self.buckets.lock().unwrap();
        buckets.retain(|_, (start, _)| now.duration_since(*start) < self.window);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_under_limit() {
        let rl = RateLimiter::new(Duration::from_secs(60), 10);
        let r = rl.check("key1", 10);
        assert!(r.allowed);
        assert_eq!(r.remaining, 9);
        assert_eq!(r.limit, 10);
    }

    #[test]
    fn blocks_at_limit() {
        let rl = RateLimiter::new(Duration::from_secs(60), 5);
        for _ in 0..5 {
            rl.check("key1", 5);
        }
        let r = rl.check("key1", 5);
        assert!(!r.allowed);
        assert_eq!(r.remaining, 0);
    }

    #[test]
    fn separate_keys_independent() {
        let rl = RateLimiter::new(Duration::from_secs(60), 5);
        for _ in 0..5 {
            rl.check("key1", 5);
        }
        assert!(!rl.check("key1", 5).allowed);
        assert!(rl.check("key2", 5).allowed);
    }

    #[test]
    fn check_default_uses_default_limit() {
        let rl = RateLimiter::new(Duration::from_secs(60), 3);
        assert!(rl.check_default("ip1").allowed); // 1 of 3
        assert!(rl.check_default("ip1").allowed); // 2 of 3
        assert!(rl.check_default("ip1").allowed); // 3 of 3
        assert!(!rl.check_default("ip1").allowed); // 4 - blocked
        // Different IP is independent
        assert!(rl.check_default("ip2").allowed);
    }
}
