//! Remote transport selection via CLI, env, and playbook defaults.

use infrazeug_core::transport::TransportChoice;

/// Default when nothing else is configured (playbook remote machines).
pub fn default_remote_transport() -> TransportChoice {
    parse_transport_name(&std::env::var("INFRZEUG_TRANSPORT").unwrap_or_else(|_| "agent".into()))
        .unwrap_or(TransportChoice::SshAgentPush)
}

pub fn parse_transport_name(name: &str) -> Option<TransportChoice> {
    match name.trim().to_lowercase().as_str() {
        "agent" | "push" | "agent-push" | "agent_push" => Some(TransportChoice::SshAgentPush),
        "agentless" | "ssh" | "ssh-agentless" => Some(TransportChoice::SshAgentless),
        _ => None,
    }
}

pub fn transport_name(choice: TransportChoice) -> &'static str {
    match choice {
        TransportChoice::SshAgentPush => "agent",
        TransportChoice::SshAgentless => "agentless",
        TransportChoice::Local => "local",
        TransportChoice::PullDaemon => "pull",
    }
}
