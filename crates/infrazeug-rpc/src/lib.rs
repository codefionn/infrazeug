//! Postcard RPC framing for agent stdin/stdout (SOUL §4.1).
//!
//! # Protocol microarchitecture
//!
//! The RPC layer implements the push-mode agent protocol. The controller
//! spawns a remote `infrazeug-agent serve-rpc` process over SSH and
//! communicates via length-prefixed postcard frames on stdin/stdout:
//!
//! ```text
//!   stdin:  uvarint(len) || postcard(RpcRequest)
//!   stdout: uvarint(len) || postcard(AgentFrame)
//! ```
//!
//! The stdout stream is multiplexed: each [`AgentFrame`] is either a
//! [`Response`](AgentFrame::Response) correlated to the in-flight request or an
//! out-of-band [`Event`](AgentFrame::Event) the agent emits on its own (e.g.
//! [`AgentMetrics`]). The controller's reader demuxes the two.
//!
//! The [`frame`] module handles encode/decode; [`messages`] defines the
//! request/response/event enums. See `docs/protocol.md` for the full
//! microarchitecture diagram.

pub mod frame;
pub mod messages;

pub use frame::{decode_one, encode, FrameError};
pub use messages::{
    AgentEvent, AgentFrame, AgentMetrics, RpcNodeGraphEntry, RpcNodeStatus, RpcRequest, RpcResponse,
};
