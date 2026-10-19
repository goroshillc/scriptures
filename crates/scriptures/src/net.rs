//! A polite HTTP client for the fetch steps (Commons, eBible, orthocal).
//!
//! > *"Ask, and it shall be given you; seek, and ye shall find; knock, and it shall be
//! > opened unto you:"* - Matthew 7:7, KJV
//!
//! WHY a wrapper: Wikimedia asks every client for a descriptive User-Agent with a
//! contact point (this project's repository, never a personal address) and for backing off on 429. One place enforces a minimum gap
//! between requests, honours `Retry-After`, and caps body sizes, so no fetch step
//! can hammer a free public service.

use crate::{Error, Result};
use std::fmt::Write as _;
use std::time::{Duration, Instant};

/// Identifies the build to the services it reads.
pub const USER_AGENT: &str =
    "GoroshiScripturesBuild/0.1 (+https://github.com/goroshillc/scriptures)";

/// Largest body accepted (Commons masters stay well under this).
const BODY_LIMIT: u64 = 64 * 1024 * 1024;

/// HTTP client with pacing and retries.
pub struct Net {
    agent: ureq::Agent,
    gap: Duration,
    last: Option<Instant>,
}

impl Net {
    /// A client that waits at least `gap` between requests.
    #[must_use]
    pub fn new(gap: Duration) -> Self {
        let config = ureq::Agent::config_builder()
            .user_agent(USER_AGENT)
            .timeout_global(Some(Duration::from_secs(90)))
            .http_status_as_error(false)
            .build();
        Self {
            agent: ureq::Agent::new_with_config(config),
            gap,
            last: None,
        }
    }

    fn pace(&mut self) {
        if let Some(last) = self.last {
            let since = last.elapsed();
            if since < self.gap {
                std::thread::sleep(self.gap.saturating_sub(since));
            }
        }
        self.last = Some(Instant::now());
    }

    /// GET a URL and return the body, retrying 429 and 5xx up to five times.
    ///
    /// # Errors
    /// Fails on transport errors, on a non-success status after retries, or on a body
    /// larger than 64 MiB.
    pub fn get(&mut self, url: &str) -> Result<Vec<u8>> {
        let mut wait = Duration::from_secs(5);
        for attempt in 0..6u32 {
            self.pace();
            let mut resp = self.agent.get(url).call().map_err(|e| Error::Http {
                url: url.to_string(),
                reason: e.to_string(),
            })?;
            let status = resp.status().as_u16();
            if status == 200 {
                return resp
                    .body_mut()
                    .with_config()
                    .limit(BODY_LIMIT)
                    .read_to_vec()
                    .map_err(|e| Error::Http {
                        url: url.to_string(),
                        reason: e.to_string(),
                    });
            }
            let retryable = status == 429 || status >= 500;
            if !retryable || attempt == 5 {
                return Err(Error::Http {
                    url: url.to_string(),
                    reason: format!("status {status}"),
                });
            }
            let hinted = resp
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.trim().parse::<u64>().ok())
                .map(Duration::from_secs);
            let sleep = hinted.unwrap_or(wait).min(Duration::from_secs(120));
            eprintln!(
                "  {status} from {url}; waiting {}s (attempt {})",
                sleep.as_secs(),
                attempt + 1
            );
            std::thread::sleep(sleep);
            wait = wait.saturating_mul(2);
        }
        Err(Error::Http {
            url: url.to_string(),
            reason: "retries exhausted".to_string(),
        })
    }

    /// GET a URL and parse JSON.
    ///
    /// # Errors
    /// Fails as [`Net::get`] does, or when the body is not JSON.
    pub fn get_json(&mut self, url: &str) -> Result<serde_json::Value> {
        let body = self.get(url)?;
        serde_json::from_slice(&body).map_err(|e| Error::json(url, e))
    }
}

/// Percent-encodes a query component (RFC 3986 unreserved characters pass through).
#[must_use]
pub fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(b));
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_escapes_spaces_pipes_and_accents() {
        assert_eq!(encode("File:Doré a|b"), "File%3ADor%C3%A9%20a%7Cb");
    }
}
