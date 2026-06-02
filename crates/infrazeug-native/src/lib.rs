//! Tier-1 native node methods (SOUL §3.3.4).
//!
//! Playbooks register [`NodeMethod`] implementations in a [`MethodRegistry`].
//! Local machines execute them in-process on the controller; push-mode remotes
//! dispatch via agent RPC ([`builtin_registry`] on the stock agent).

mod builtins;
mod encode;
mod error;
mod method;
mod registry;
mod result;
mod secret;

pub use builtins::{
    builtin_registry, EchoInput, EchoMethod, PingInput, PingMethod, NATIVE_ECHO, NATIVE_PING,
};
pub use encode::encode_input;
pub use error::{NativeError, Result};
pub use method::{NodeCtx, NodeMethod, PlanCtx, PlanMethodOutcome};
pub use registry::MethodRegistry;
pub use result::{NativeResult, NativeStatus};
pub use secret::SecretSource;
