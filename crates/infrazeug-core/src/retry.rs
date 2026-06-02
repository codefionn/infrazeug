use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use tracing::debug;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RetryMode {
    Off,
    #[default]
    Auto,
    Force,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Backoff {
    Fixed(Duration),
    Exp {
        initial: Duration,
        max: Duration,
        jitter: bool,
    },
}

impl Default for Backoff {
    fn default() -> Self {
        Self::Exp {
            initial: Duration::from_secs(1),
            max: Duration::from_secs(30),
            jitter: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct RetryConfig {
    pub enabled: RetryMode,
    pub max_attempts: u32,
    pub backoff: Backoff,
}

impl RetryConfig {
    pub fn off() -> Self {
        Self {
            enabled: RetryMode::Off,
            max_attempts: 0,
            backoff: Backoff::default(),
        }
    }

    pub fn idempotent_default() -> Self {
        Self {
            enabled: RetryMode::Auto,
            max_attempts: 3,
            backoff: Backoff::Exp {
                initial: Duration::from_secs(1),
                max: Duration::from_secs(30),
                jitter: true,
            },
        }
    }

    pub fn should_retry(&self, idempotent: bool, attempt: u32) -> bool {
        if attempt >= self.max_attempts {
            return false;
        }
        match self.enabled {
            RetryMode::Off => false,
            RetryMode::Auto => idempotent,
            RetryMode::Force => true,
        }
    }

    pub fn is_off(&self) -> bool {
        matches!(self.enabled, RetryMode::Off)
    }

    pub async fn wait_before_retry(&self, attempt: u32) {
        let delay = self.delay_for_attempt(attempt);
        debug!(?delay, attempt, "retry backoff");
        tokio::time::sleep(delay).await;
    }

    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        match self.backoff {
            Backoff::Fixed(d) => d,
            Backoff::Exp {
                initial,
                max,
                jitter,
            } => {
                let capped_ms = exponential_backoff_ms(initial, max, attempt);
                if jitter && capped_ms > 0 {
                    let lo = capped_ms.saturating_mul(3) / 4;
                    let hi = capped_ms.saturating_mul(5) / 4;
                    let mix = (attempt as u64).wrapping_mul(0x9E3779B97F4A7C15);
                    let pseudo = mix ^ (mix >> 32);
                    let range = hi - lo;
                    let jittered = lo + (pseudo % (range + 1));
                    Duration::from_millis(jittered)
                } else {
                    Duration::from_millis(capped_ms)
                }
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PollConfig {
    pub check: PollCheck,
    pub every: Duration,
    pub timeout: Duration,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PollCheck {
    Command { argv: Vec<String> },
    FileExists { path: PathBuf },
    TcpConnect { host: String, port: u16 },
}

#[derive(Clone, Copy, Debug)]
pub struct ReconnectConfig {
    pub max_attempts: u32,
    pub backoff: Backoff,
}

impl Default for ReconnectConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            backoff: Backoff::Exp {
                initial: Duration::from_secs(2),
                max: Duration::from_secs(60),
                jitter: true,
            },
        }
    }
}

impl ReconnectConfig {
    pub fn reboot_default() -> Self {
        Self {
            max_attempts: 120,
            backoff: Backoff::Exp {
                initial: Duration::from_secs(2),
                max: Duration::from_secs(30),
                jitter: true,
            },
        }
    }

    pub fn should_reconnect(&self, attempt: u32) -> bool {
        attempt < self.max_attempts
    }

    pub async fn wait_before_reconnect(&self, attempt: u32) {
        let delay = self.delay_for_attempt(attempt);
        debug!(?delay, attempt, "reconnect backoff");
        tokio::time::sleep(delay).await;
    }

    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        match self.backoff {
            Backoff::Fixed(d) => d,
            Backoff::Exp {
                initial,
                max,
                jitter,
            } => {
                let capped_ms = exponential_backoff_ms(initial, max, attempt);
                if jitter && capped_ms > 0 {
                    let lo = capped_ms.saturating_mul(3) / 4;
                    let hi = capped_ms.saturating_mul(5) / 4;
                    let mix = (attempt as u64).wrapping_mul(0x517CC1B727220A95);
                    let pseudo = mix ^ (mix >> 32);
                    let range = hi - lo;
                    let jittered = lo + (pseudo % (range + 1));
                    Duration::from_millis(jittered)
                } else {
                    Duration::from_millis(capped_ms)
                }
            }
        }
    }
}

fn exponential_backoff_ms(initial: Duration, max: Duration, attempt: u32) -> u64 {
    duration_millis_u64(initial)
        .saturating_mul(2u64.saturating_pow(attempt))
        .min(duration_millis_u64(max))
}

fn duration_millis_u64(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_backoff_saturates_without_overflow() {
        let cfg = RetryConfig {
            enabled: RetryMode::Force,
            max_attempts: 120,
            backoff: Backoff::Exp {
                initial: Duration::from_secs(2),
                max: Duration::from_secs(30),
                jitter: false,
            },
        };

        assert_eq!(cfg.delay_for_attempt(119), Duration::from_secs(30));
    }

    #[test]
    fn reconnect_backoff_saturates_without_overflow() {
        let delay = ReconnectConfig::reboot_default().delay_for_attempt(119);

        assert!(delay >= Duration::from_millis(22_500));
        assert!(delay <= Duration::from_millis(37_500));
    }
}
