use crate::error::Result;
use crate::method::{NodeCtx, NodeMethod, PlanCtx, PlanMethodOutcome};
use crate::registry::MethodRegistry;
use crate::result::NativeResult;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub const NATIVE_PING: &str = "native.ping";
pub const NATIVE_ECHO: &str = "native.echo";

#[derive(Clone, Copy, Debug, Default)]
pub struct PingMethod;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PingInput {}

#[async_trait]
impl NodeMethod for PingMethod {
    type Input = PingInput;
    type Output = ();

    fn name(&self) -> &'static str {
        NATIVE_PING
    }

    fn idempotent(&self) -> bool {
        true
    }

    async fn plan(&self, _ctx: &PlanCtx, _input: &PingInput) -> Result<PlanMethodOutcome> {
        Ok(PlanMethodOutcome::Unchanged)
    }

    async fn execute(&self, _ctx: &NodeCtx, _input: PingInput) -> Result<NativeResult> {
        Ok(NativeResult::unchanged("pong"))
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct EchoMethod;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EchoInput {
    #[serde(default)]
    pub text: String,
}

#[async_trait]
impl NodeMethod for EchoMethod {
    type Input = EchoInput;
    type Output = String;

    fn name(&self) -> &'static str {
        NATIVE_ECHO
    }

    async fn execute(&self, _ctx: &NodeCtx, input: EchoInput) -> Result<NativeResult> {
        let output = serde_cbor::Value::Text(input.text.clone());
        Ok(NativeResult::changed(format!("echo: {}", input.text)).with_output(output))
    }
}

/// Stock agent + demo registry (ping + echo).
pub fn builtin_registry() -> MethodRegistry {
    let mut reg = MethodRegistry::new();
    reg.register(PingMethod);
    reg.register(EchoMethod);
    reg
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[tokio::test]
    async fn ping_unchanged() {
        let reg = builtin_registry();
        let ctx = NodeCtx::new(Uuid::new_v4(), Uuid::new_v4());
        let out = reg
            .execute(&ctx, NATIVE_PING, serde_cbor::Value::Null)
            .await
            .unwrap();
        assert_eq!(out.status, crate::NativeStatus::Unchanged);
    }

    #[tokio::test]
    async fn echo_round_trip() {
        let reg = builtin_registry();
        let ctx = NodeCtx::new(Uuid::new_v4(), Uuid::new_v4());
        let input =
            serde_cbor::Value::Bytes(serde_cbor::to_vec(&EchoInput { text: "hi".into() }).unwrap());
        let out = reg.execute(&ctx, NATIVE_ECHO, input).await.unwrap();
        assert_eq!(out.status, crate::NativeStatus::Changed);
    }
}
