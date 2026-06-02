//! [`EnsureResource`] — turns any [`Resource`] into a tier-1 [`NodeMethod`].

use crate::resource::{Drift, Resource, ResourceError};
use async_trait::async_trait;
use infrazeug_native::{
    encode_input, NativeError, NativeResult, NodeCtx, NodeMethod, PlanCtx, PlanMethodOutcome,
    Result,
};

/// Adapter that drives a [`Resource`] through the converge loop and exposes it
/// as a [`NodeMethod`].
///
/// `plan` previews via `observe` + `diff` (absent → `Changed`, drift → `Changed`,
/// match → `Unchanged`). `execute` runs the same observation, then creates when
/// absent or reconciles when drifted, and always attaches the live state as both
/// the node output and a JSON capture (for downstream vault writes). Marked
/// idempotent so `RetryMode::Auto` may retry it.
pub struct EnsureResource<R: Resource>(R);

impl<R: Resource> EnsureResource<R> {
    pub fn new(inner: R) -> Self {
        Self(inner)
    }

    /// Borrow the wrapped resource (handy for provider registries/tests).
    pub fn inner(&self) -> &R {
        &self.0
    }
}

#[async_trait]
impl<R: Resource> NodeMethod for EnsureResource<R> {
    type Input = R::Spec;
    type Output = R::State;

    fn name(&self) -> &'static str {
        self.0.kind()
    }

    fn idempotent(&self) -> bool {
        true
    }

    async fn plan(&self, ctx: &PlanCtx, spec: &Self::Input) -> Result<PlanMethodOutcome> {
        let rctx = ctx.into();
        match self.0.observe(&rctx, spec).await {
            Ok(None) => Ok(PlanMethodOutcome::Changed),
            Ok(Some(current)) => Ok(match self.0.diff(spec, &current) {
                Drift::InSync => PlanMethodOutcome::Unchanged,
                Drift::Drifted(_) => PlanMethodOutcome::Changed,
            }),
            // A read-only preview without an unlocked vault cannot observe a
            // credential-backed resource; report it as unknown rather than failing.
            Err(ResourceError::SecretsUnavailable | ResourceError::InputsUnavailable) => {
                Ok(PlanMethodOutcome::Unknown)
            }
            Err(e) => Err(e.into()),
        }
    }

    async fn execute(&self, ctx: &NodeCtx, spec: Self::Input) -> Result<NativeResult> {
        let rctx = ctx.into();
        let kind = self.0.kind();

        let (state, result) = match self.0.observe(&rctx, &spec).await? {
            None => {
                let state = self.0.create(&rctx, &spec).await?;
                (state, NativeResult::changed(format!("created {kind}")))
            }
            Some(current) => match self.0.diff(&spec, &current) {
                Drift::InSync => (current, NativeResult::unchanged(format!("{kind} in sync"))),
                Drift::Drifted(why) => {
                    let state = self.0.reconcile(&rctx, &spec, current).await?;
                    (
                        state,
                        NativeResult::changed(format!("reconciled {kind}: {why}")),
                    )
                }
            },
        };

        let output = encode_input(&state)?;
        result
            .with_output(output)
            .with_json_capture(&state)
            .map_err(|e| NativeError::other(e.to_string()))
    }
}
