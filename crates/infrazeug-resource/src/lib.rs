//! Provider-agnostic resource acquisition for infrazeug.
//!
//! A single [`Resource`] trait describes how to acquire one kind of cloud
//! resource (bucket, server, volume, user, …) in terms of four focused steps —
//! `observe` / `create` / `diff` / `reconcile`. Wrapping it in [`EnsureResource`]
//! yields a tier-1 [`NodeMethod`](infrazeug_native::NodeMethod), so the resource
//! becomes a first-class node in the infrazeug graph and reuses the existing
//! scheduling, dependency, capture→vault, and retry machinery unchanged.
//!
//! Provider crates (e.g. `infrazeug-ovh`, `infrazeug-ionos`) implement
//! [`Resource`] over their ext API client; the adapter owns the idempotency,
//! plan/diff preview, and reconcile/status boilerplate that every provider would
//! otherwise repeat.
//!
//! ```ignore
//! struct Bucket { client: Arc<Client> }
//!
//! #[async_trait]
//! impl Resource for Bucket {
//!     type Spec = BucketSpec;
//!     type State = BucketState;
//!     fn kind(&self) -> &'static str { "myprovider.ensure_bucket" }
//!     async fn observe(&self, _ctx, spec) -> ResourceResult<Option<BucketState>> { /* list+match */ }
//!     async fn create(&self, _ctx, spec) -> ResourceResult<BucketState> { /* POST */ }
//! }
//!
//! registry.register(EnsureResource::new(Bucket { client }));
//! ```

mod adapter;
mod resource;

pub use adapter::EnsureResource;
pub use resource::{Drift, Resource, ResourceCtx, ResourceError, ResourceInput, ResourceResult};

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use infrazeug_native::{
        NativeStatus, NodeCtx, NodeMethod, PlanCtx, PlanMethodOutcome, SecretSource,
    };
    use serde::{Deserialize, Serialize};
    use std::sync::{Arc, Mutex};
    use uuid::Uuid;

    #[derive(Clone, Default, Serialize, Deserialize)]
    struct Spec {
        region: String,
    }

    #[derive(Clone, Serialize, Deserialize)]
    struct State {
        id: String,
        region: String,
    }

    /// In-memory resource: `current` is the live state observed each call.
    struct Fake {
        current: Mutex<Option<State>>,
    }

    impl Fake {
        fn absent() -> Self {
            Self {
                current: Mutex::new(None),
            }
        }
        fn present(region: &str) -> Self {
            Self {
                current: Mutex::new(Some(State {
                    id: "existing".into(),
                    region: region.into(),
                })),
            }
        }
    }

    #[async_trait]
    impl Resource for Fake {
        type Spec = Spec;
        type State = State;

        fn kind(&self) -> &'static str {
            "test.fake"
        }

        async fn observe(&self, _ctx: &ResourceCtx, _spec: &Spec) -> ResourceResult<Option<State>> {
            Ok(self.current.lock().unwrap().clone())
        }

        async fn create(&self, _ctx: &ResourceCtx, spec: &Spec) -> ResourceResult<State> {
            let state = State {
                id: "created".into(),
                region: spec.region.clone(),
            };
            *self.current.lock().unwrap() = Some(state.clone());
            Ok(state)
        }

        fn diff(&self, spec: &Spec, current: &State) -> Drift {
            if spec.region == current.region {
                Drift::InSync
            } else {
                Drift::Drifted(format!("region {} → {}", current.region, spec.region))
            }
        }

        async fn reconcile(
            &self,
            _ctx: &ResourceCtx,
            spec: &Spec,
            mut current: State,
        ) -> ResourceResult<State> {
            current.region = spec.region.clone();
            *self.current.lock().unwrap() = Some(current.clone());
            Ok(current)
        }
    }

    fn plan_ctx() -> PlanCtx {
        PlanCtx::new(Uuid::nil(), Uuid::nil())
    }
    fn node_ctx() -> NodeCtx {
        NodeCtx::new(Uuid::nil(), Uuid::nil())
    }
    fn spec(region: &str) -> Spec {
        Spec {
            region: region.into(),
        }
    }

    #[tokio::test]
    async fn absent_plans_changed_and_creates() {
        let m = EnsureResource::new(Fake::absent());
        assert_eq!(
            m.plan(&plan_ctx(), &spec("GRA")).await.unwrap(),
            PlanMethodOutcome::Changed
        );
        let res = m.execute(&node_ctx(), spec("GRA")).await.unwrap();
        assert_eq!(res.status, NativeStatus::Changed);
        // Live state is captured for downstream vault writes.
        let json: serde_json::Value =
            serde_json::from_slice(&res.capture.expect("capture")).unwrap();
        assert_eq!(
            json.pointer("/id").and_then(|v| v.as_str()),
            Some("created")
        );
    }

    #[tokio::test]
    async fn present_in_sync_plans_unchanged() {
        let m = EnsureResource::new(Fake::present("GRA"));
        assert_eq!(
            m.plan(&plan_ctx(), &spec("GRA")).await.unwrap(),
            PlanMethodOutcome::Unchanged
        );
        let res = m.execute(&node_ctx(), spec("GRA")).await.unwrap();
        assert_eq!(res.status, NativeStatus::Unchanged);
    }

    #[tokio::test]
    async fn present_drifted_plans_changed_and_reconciles() {
        let m = EnsureResource::new(Fake::present("GRA"));
        assert_eq!(
            m.plan(&plan_ctx(), &spec("SBG")).await.unwrap(),
            PlanMethodOutcome::Changed
        );
        let res = m.execute(&node_ctx(), spec("SBG")).await.unwrap();
        assert_eq!(res.status, NativeStatus::Changed);
        assert!(res.message.unwrap().contains("reconciled"));
        let json: serde_json::Value =
            serde_json::from_slice(&res.capture.expect("capture")).unwrap();
        assert_eq!(
            json.pointer("/region").and_then(|v| v.as_str()),
            Some("SBG")
        );
    }

    /// Minimal in-memory secret source for context-threading tests.
    struct FakeSecrets;

    #[async_trait]
    impl SecretSource for FakeSecrets {
        async fn read_field(&self, file: &str, field: &str) -> infrazeug_native::Result<Vec<u8>> {
            Ok(format!("{file}:{field}").into_bytes())
        }
    }

    struct FakeInputs {
        capture_node: Uuid,
        capture_machine: Uuid,
    }

    #[async_trait]
    impl SecretSource for FakeInputs {
        fn has_vault(&self) -> bool {
            true
        }

        fn has_mutable_vault(&self) -> bool {
            true
        }

        fn has_node_captures(&self) -> bool {
            true
        }

        async fn read_field(&self, file: &str, field: &str) -> infrazeug_native::Result<Vec<u8>> {
            Ok(format!("{file}:{field}").into_bytes())
        }

        async fn read_mutable_field(
            &self,
            file: &str,
            field: &str,
        ) -> infrazeug_native::Result<Vec<u8>> {
            Ok(format!("mutable/{file}:{field}").into_bytes())
        }

        async fn read_node_capture(
            &self,
            node: Uuid,
            machine: Uuid,
        ) -> infrazeug_native::Result<Vec<u8>> {
            assert_eq!(node, self.capture_node);
            assert_eq!(machine, self.capture_machine);
            Ok(br#"{"endpoint":"https://api.example","nested":{"port":443}}"#.to_vec())
        }
    }

    /// Resource that reads a credential during `observe`; absent always.
    struct SecretReader;

    #[async_trait]
    impl Resource for SecretReader {
        type Spec = Spec;
        type State = State;
        fn kind(&self) -> &'static str {
            "test.secret_reader"
        }
        async fn observe(&self, ctx: &ResourceCtx, _spec: &Spec) -> ResourceResult<Option<State>> {
            ctx.read_secret_string("cloud/ovh.vault", "application_key")
                .await?;
            Ok(None)
        }
        async fn create(&self, _ctx: &ResourceCtx, spec: &Spec) -> ResourceResult<State> {
            Ok(State {
                id: "created".into(),
                region: spec.region.clone(),
            })
        }
    }

    #[tokio::test]
    async fn resource_ctx_reads_secret_from_source() {
        let ctx = NodeCtx::new(Uuid::nil(), Uuid::nil())
            .with_secrets(Some(Arc::new(FakeSecrets) as Arc<dyn SecretSource>));
        let rctx = ResourceCtx::from(&ctx);
        assert!(rctx.has_secrets());
        let value = rctx
            .read_secret_string("cloud/ovh.vault", "application_key")
            .await
            .unwrap();
        assert_eq!(value, "cloud/ovh.vault:application_key");
    }

    #[tokio::test]
    async fn missing_source_is_secrets_unavailable() {
        let rctx = ResourceCtx::from(&node_ctx());
        assert!(!rctx.has_secrets());
        assert!(matches!(
            rctx.read_secret("f", "k").await,
            Err(ResourceError::SecretsUnavailable)
        ));
    }

    #[tokio::test]
    async fn plan_without_vault_is_unknown_not_failure() {
        // Preview has no unlocked vault: a credential-backed resource must report
        // `Unknown` instead of failing the whole preview.
        let m = EnsureResource::new(SecretReader);
        assert_eq!(
            m.plan(&plan_ctx(), &spec("GRA")).await.unwrap(),
            PlanMethodOutcome::Unknown
        );
    }

    #[tokio::test]
    async fn apply_with_vault_reads_secret_and_creates() {
        let m = EnsureResource::new(SecretReader);
        let ctx = NodeCtx::new(Uuid::nil(), Uuid::nil())
            .with_secrets(Some(Arc::new(FakeSecrets) as Arc<dyn SecretSource>));
        let res = m.execute(&ctx, spec("GRA")).await.unwrap();
        assert_eq!(res.status, NativeStatus::Changed);
    }

    #[tokio::test]
    async fn resource_input_resolves_inline_vault_mutable_and_node_capture() {
        let node = Uuid::new_v4();
        let machine = Uuid::new_v4();
        let ctx = NodeCtx::new(machine, Uuid::new_v4()).with_secrets(Some(Arc::new(FakeInputs {
            capture_node: node,
            capture_machine: machine,
        })
            as Arc<dyn SecretSource>));
        let rctx = ResourceCtx::from(&ctx);

        assert!(rctx.has_secrets());
        assert!(rctx.has_mutable_secrets());
        assert!(rctx.has_node_captures());

        let inline = ResourceInput::inline("literal".to_string())
            .resolve(&rctx)
            .await
            .unwrap();
        assert_eq!(inline, "literal");

        let vault = ResourceInput::<String>::vault("cloud/provider.vault", "token")
            .resolve(&rctx)
            .await
            .unwrap();
        assert_eq!(vault, "cloud/provider.vault:token");

        let mutable =
            ResourceInput::<String>::mutable_vault("cloud/generated.vault", "credentials.key")
                .resolve(&rctx)
                .await
                .unwrap();
        assert_eq!(mutable, "mutable/cloud/generated.vault:credentials.key");

        let endpoint = ResourceInput::<String>::node(node)
            .json_pointer("/endpoint")
            .resolve(&rctx)
            .await
            .unwrap();
        assert_eq!(endpoint, "https://api.example");

        let port = ResourceInput::<u16>::node(node)
            .json_pointer("/nested/port")
            .resolve(&rctx)
            .await
            .unwrap();
        assert_eq!(port, 443);
    }
}
