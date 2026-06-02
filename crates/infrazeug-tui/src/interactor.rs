use async_trait::async_trait;
use infrazeug_core::error::Result;
use infrazeug_core::interactor::{Interaction, InteractionResp, Interactor};
use tokio::sync::{mpsc, oneshot};

/// Prompt delivered to the TUI loop; respond via the oneshot.
pub struct PendingInteraction {
    pub req: Interaction,
    resp_tx: oneshot::Sender<Result<InteractionResp>>,
}

impl PendingInteraction {
    pub fn respond(self, resp: Result<InteractionResp>) {
        let _ = self.resp_tx.send(resp);
    }
}

/// Routes `Interactor::ask` calls to the TUI prompt channel (SOUL §6ter.2).
pub struct TuiInteractor {
    tx: mpsc::UnboundedSender<PendingInteraction>,
}

impl TuiInteractor {
    pub fn pair() -> (
        std::sync::Arc<Self>,
        mpsc::UnboundedReceiver<PendingInteraction>,
    ) {
        let (tx, rx) = mpsc::unbounded_channel();
        (std::sync::Arc::new(Self { tx }), rx)
    }
}

#[async_trait]
impl Interactor for TuiInteractor {
    async fn ask(&self, req: Interaction) -> Result<InteractionResp> {
        let (resp_tx, resp_rx) = oneshot::channel();
        self.tx
            .send(PendingInteraction { req, resp_tx })
            .map_err(|e| infrazeug_core::CoreError::other(e.to_string()))?;
        resp_rx
            .await
            .map_err(|_| infrazeug_core::CoreError::other("prompt channel closed"))?
    }
}
