//! [`EnsureResource`] — turns any [`Resource`] into a tier-1 [`NodeMethod`].

use crate::resource::{Drift, Resource, ResourceCtx, ResourceError, ResourceResult};
use async_trait::async_trait;
use infrazeug_native::{
    encode_input, NativeError, NativeResult, NodeCtx, NodeMethod, PlanCtx, PlanMethodOutcome,
    Result,
};
use std::time::Duration;

/// How often [`EnsureResource`] retries a [`ResourceError::Transient`] failure.
///
/// The default makes 5 attempts with 2s, 4s, 8s and 16s pauses in between
/// (about 30s of waiting), which rides out a short provider hiccup without
/// stalling the run for long when the provider is really down.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransientRetry {
    /// Total attempts, including the first. `1` disables retrying.
    pub attempts: u32,
    /// Pause after the first failure; doubles each attempt up to `max_delay`.
    pub initial_delay: Duration,
    pub max_delay: Duration,
}

impl Default for TransientRetry {
    fn default() -> Self {
        Self {
            attempts: 5,
            initial_delay: Duration::from_secs(2),
            max_delay: Duration::from_secs(30),
        }
    }
}

impl TransientRetry {
    pub fn off() -> Self {
        Self {
            attempts: 1,
            ..Self::default()
        }
    }

    /// Pause after failed attempt number `attempt` (1-based).
    fn delay(&self, attempt: u32) -> Duration {
        let factor = 2u32.saturating_pow(attempt.saturating_sub(1));
        self.initial_delay
            .saturating_mul(factor)
            .min(self.max_delay)
    }
}

/// Adapter that drives a [`Resource`] through the converge loop and exposes it
/// as a [`NodeMethod`].
///
/// `plan` previews via `observe` + `diff` (absent → `Changed`, drift → `Changed`,
/// match → `Unchanged`). `execute` runs the same observation, then creates when
/// absent or reconciles when drifted, and always attaches the live state as both
/// the node output and a JSON capture (for downstream vault writes). Marked
/// idempotent so `RetryMode::Auto` may retry it.
///
/// Both run the whole step again on [`ResourceError::Transient`], per
/// [`TransientRetry`]. Retrying starts over at `observe`, so a create that the
/// provider half-finished before failing is seen as present (or drifted) on the
/// next attempt rather than created twice.
pub struct EnsureResource<R: Resource> {
    inner: R,
    retry: TransientRetry,
}

impl<R: Resource> EnsureResource<R> {
    pub fn new(inner: R) -> Self {
        Self {
            inner,
            retry: TransientRetry::default(),
        }
    }

    /// Override the transient-error retry policy (chainable).
    pub fn with_transient_retry(mut self, retry: TransientRetry) -> Self {
        self.retry = retry;
        self
    }

    /// Borrow the wrapped resource (handy for provider registries/tests).
    pub fn inner(&self) -> &R {
        &self.inner
    }

    async fn plan_once(
        &self,
        rctx: &ResourceCtx,
        spec: &R::Spec,
    ) -> ResourceResult<PlanMethodOutcome> {
        Ok(match self.inner.observe(rctx, spec).await? {
            None => PlanMethodOutcome::Changed,
            Some(current) => match self.inner.diff(spec, &current) {
                Drift::InSync => PlanMethodOutcome::Unchanged,
                Drift::Drifted(_) => PlanMethodOutcome::Changed,
            },
        })
    }

    async fn converge_once(
        &self,
        rctx: &ResourceCtx,
        spec: &R::Spec,
    ) -> ResourceResult<(R::State, NativeResult)> {
        let kind = self.inner.kind();
        Ok(match self.inner.observe(rctx, spec).await? {
            None => {
                let state = self.inner.create(rctx, spec).await?;
                (state, NativeResult::changed(format!("created {kind}")))
            }
            Some(current) => match self.inner.diff(spec, &current) {
                Drift::InSync => (current, NativeResult::unchanged(format!("{kind} in sync"))),
                Drift::Drifted(why) => {
                    let state = self.inner.reconcile(rctx, spec, current).await?;
                    (
                        state,
                        NativeResult::changed(format!("reconciled {kind}: {why}")),
                    )
                }
            },
        })
    }

    /// Sleep before the next attempt if `err` is transient and attempts remain.
    /// Returns `false` when the caller should give up.
    async fn back_off(&self, err: &ResourceError, attempt: u32) -> bool {
        if !err.is_transient() || attempt >= self.retry.attempts {
            return false;
        }
        let delay = self.retry.delay(attempt);
        tracing::warn!(
            kind = self.inner.kind(),
            attempt,
            max_attempts = self.retry.attempts,
            ?delay,
            error = %err,
            "transient resource error, retrying"
        );
        tokio::time::sleep(delay).await;
        true
    }

    fn give_up(&self, err: ResourceError, attempt: u32) -> NativeError {
        if attempt > 1 {
            NativeError::other(format!("{err} (gave up after {attempt} attempts)"))
        } else {
            err.into()
        }
    }
}

#[async_trait]
impl<R: Resource> NodeMethod for EnsureResource<R> {
    type Input = R::Spec;
    type Output = R::State;

    fn name(&self) -> &'static str {
        self.inner.kind()
    }

    fn idempotent(&self) -> bool {
        true
    }

    async fn plan(&self, ctx: &PlanCtx, spec: &Self::Input) -> Result<PlanMethodOutcome> {
        let rctx = ctx.into();
        let mut attempt = 1;
        loop {
            match self.plan_once(&rctx, spec).await {
                Ok(outcome) => return Ok(outcome),
                // A read-only preview without an unlocked vault cannot observe a
                // credential-backed resource; report it as unknown rather than failing.
                Err(ResourceError::SecretsUnavailable | ResourceError::InputsUnavailable) => {
                    return Ok(PlanMethodOutcome::Unknown)
                }
                Err(e) if self.back_off(&e, attempt).await => attempt += 1,
                Err(e) => return Err(self.give_up(e, attempt)),
            }
        }
    }

    async fn execute(&self, ctx: &NodeCtx, spec: Self::Input) -> Result<NativeResult> {
        let rctx = ctx.into();
        let mut attempt = 1;
        let (state, result) = loop {
            match self.converge_once(&rctx, &spec).await {
                Ok(done) => break done,
                Err(e) if self.back_off(&e, attempt).await => attempt += 1,
                Err(e) => return Err(self.give_up(e, attempt)),
            }
        };

        let output = encode_input(&state)?;
        result
            .with_output(output)
            .with_json_capture(&state)
            .map_err(|e| NativeError::other(e.to_string()))
    }
}
