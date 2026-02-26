use std::future::Future;
use std::path::Path;

use anyhow::Result;

/// The result of dispatching work to an AI agent.
#[derive(Debug)]
pub struct AgentResult {
    pub success: bool,
    pub message: String,
}

/// Abstraction over an AI coding agent (Cursor, Claude Code, Aider, etc.).
///
/// Each implementation knows how to invoke its CLI and construct prompts.
#[allow(dead_code)]
pub trait AgentProvider: Send + Sync {
    /// Human-readable name shown in the UI.
    fn name(&self) -> &str;

    /// Check whether the agent's CLI is installed and reachable.
    fn is_available(&self) -> Result<bool>;

    /// List the models this agent supports.
    ///
    /// Implementations should attempt to fetch the list dynamically (e.g. from
    /// the agent CLI) and fall back to a cached / compile-time default list on
    /// failure.
    fn supported_models(&self) -> impl Future<Output = Result<Vec<String>>> + Send;

    /// Send a prompt to the agent and wait for it to finish.
    fn execute(
        &self,
        prompt: &str,
        model: Option<&str>,
        working_dir: &Path,
    ) -> impl Future<Output = Result<AgentResult>> + Send;
}
