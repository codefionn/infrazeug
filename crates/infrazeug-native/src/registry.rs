use crate::error::{NativeError, Result};
use crate::method::{erase, ErasedNodeMethod, NodeCtx, NodeMethod, PlanCtx, PlanMethodOutcome};
use crate::result::NativeResult;
use std::any::TypeId;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone, Default)]
pub struct MethodRegistry {
    methods: HashMap<String, Arc<dyn ErasedNodeMethod>>,
    /// Maps Rust method types to their registered `name()` (for typed node helpers).
    type_index: HashMap<TypeId, String>,
}

impl MethodRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<M: NodeMethod + 'static>(&mut self, method: M) -> &mut Self {
        let name = method.name().to_string();
        self.type_index.insert(TypeId::of::<M>(), name.clone());
        self.methods.insert(name, erase(method));
        self
    }

    /// Registered [`NodeMethod::name`] for type `M`, if [`register`](Self::register) was called.
    pub fn name_of<M: NodeMethod + 'static>(&self) -> Option<&str> {
        self.type_index.get(&TypeId::of::<M>()).map(|s| s.as_str())
    }

    pub fn register_erased(&mut self, method: Arc<dyn ErasedNodeMethod>) -> &mut Self {
        self.methods.insert(method.name().to_string(), method);
        self
    }

    pub fn merge(&mut self, other: MethodRegistry) -> &mut Self {
        self.methods.extend(other.methods);
        self.type_index.extend(other.type_index);
        self
    }

    pub fn contains(&self, method_id: &str) -> bool {
        self.methods.contains_key(method_id)
    }

    pub fn get(&self, method_id: &str) -> Option<&Arc<dyn ErasedNodeMethod>> {
        self.methods.get(method_id)
    }

    pub async fn execute(
        &self,
        ctx: &NodeCtx,
        method_id: &str,
        input: serde_cbor::Value,
    ) -> Result<NativeResult> {
        let method = self
            .methods
            .get(method_id)
            .ok_or_else(|| NativeError::NotFound {
                method: method_id.to_string(),
            })?;
        method.execute_erased(ctx, input).await
    }

    pub async fn plan(
        &self,
        ctx: &PlanCtx,
        method_id: &str,
        input: &serde_cbor::Value,
    ) -> Result<PlanMethodOutcome> {
        let method = self
            .methods
            .get(method_id)
            .ok_or_else(|| NativeError::NotFound {
                method: method_id.to_string(),
            })?;
        method.plan_erased(ctx, input).await
    }
}
