use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum PullMode {
    #[default]
    OneShot,
    Daemon {
        interval: Duration,
        #[serde(default)]
        jitter: Duration,
    },
}

impl PullMode {
    pub fn from_poll_interval(poll: Option<Duration>) -> Self {
        match poll {
            Some(interval) => Self::Daemon {
                interval,
                jitter: Duration::from_secs(0),
            },
            None => Self::OneShot,
        }
    }
}
