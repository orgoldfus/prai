use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc::UnboundedSender;

use super::provider::{AgentProvider, AgentResult};
use super::stream::StreamChunk;

/// Cursor CLI agent — invokes `cursor-agent` (Cursor's CLI binary) in print mode.
#[derive(Debug, Default)]
pub struct CursorAgent;

impl CursorAgent {
    /// The Cursor CLI binary name.
    const BIN: &'static str = "cursor-agent";

    /// Compile-time default model list — only used on the very first run
    /// before a successful live fetch has populated the disk cache.
    const DEFAULTS: &'static [&'static str] =
        &["claude-4-sonnet", "claude-4-opus", "gpt-5", "cursor-small"];

    /// Path to the on-disk model cache: `~/.config/prai/models_cache.json`.
    fn cache_path() -> Option<std::path::PathBuf> {
        dirs::config_dir().map(|d| d.join("prai").join("models_cache.json"))
    }

    /// Attempt to read the model list from the disk cache.
    fn read_cache() -> Option<Vec<String>> {
        let path = Self::cache_path()?;
        let contents = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&contents).ok()
    }

    /// Persist a successfully-fetched model list to the disk cache.
    fn write_cache(models: &[String]) {
        if let Some(path) = Self::cache_path() {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(json) = serde_json::to_string(models) {
                let _ = std::fs::write(path, json);
            }
        }
    }

    /// Parse the raw stdout of `cursor-agent models` into a model name list.
    ///
    /// Expected format (one model per line):
    /// ```text
    /// model-id - Display Name
    /// model-id - Display Name  (current, default)
    /// ```
    /// Header/footer lines ("Available models", "Tip: ...") are ignored.
    fn parse_models_output(raw: &str) -> Vec<String> {
        raw.lines()
            .filter_map(|line| {
                let line = line.trim();
                // Each model line contains " - " separating id from display name.
                let (id, _rest) = line.split_once(" - ")?;
                let id = id.trim();
                // Sanity-check: model ids are non-empty, contain a letter or
                // digit, and have no spaces.
                if id.is_empty() || id.contains(' ') {
                    return None;
                }
                Some(id.to_owned())
            })
            .collect()
    }

    /// Return the best immediately-available fallback list (disk cache, then
    /// compile-time defaults). Does no I/O beyond a single file read.
    pub fn fallback_models() -> Vec<String> {
        if let Some(cached) = Self::read_cache() {
            if !cached.is_empty() {
                return cached;
            }
        }
        Self::DEFAULTS.iter().map(|s| s.to_string()).collect()
    }

    pub async fn execute_with_stream(
        prompt: &str,
        model: Option<&str>,
        working_dir: &Path,
        log_tx: UnboundedSender<StreamChunk>,
    ) -> Result<AgentResult> {
        let mut cmd = tokio::process::Command::new(Self::BIN);
        cmd.current_dir(working_dir)
            .arg("-p")
            .arg("--output-format")
            .arg("stream-json")
            .arg("--stream-partial-output")
            .arg(prompt)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(model) = model {
            cmd.arg("--model").arg(model);
        }

        let mut child = cmd.spawn().context("failed to execute Cursor CLI agent")?;

        let stdout_task = child.stdout.take().map(|stdout| {
            let tx = log_tx.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(stdout).lines();
                let mut collected = Vec::new();
                while let Ok(Some(line)) = lines.next_line().await {
                    let _ = tx.send(StreamChunk::Stdout(line.clone()));
                    collected.push(line);
                }
                collected
            })
        });

        let stderr_task = child.stderr.take().map(|stderr| {
            let tx = log_tx.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                let mut collected = Vec::new();
                while let Ok(Some(line)) = lines.next_line().await {
                    let _ = tx.send(StreamChunk::Stderr(line.clone()));
                    collected.push(line);
                }
                collected
            })
        });

        let status = child
            .wait()
            .await
            .context("failed waiting for Cursor CLI")?;
        let _ = log_tx.send(StreamChunk::System(format!(
            "process exited with status: {}",
            status.code().unwrap_or(-1)
        )));

        let stdout_lines = match stdout_task {
            Some(task) => task.await.unwrap_or_default(),
            None => Vec::new(),
        };
        let stderr_lines = match stderr_task {
            Some(task) => task.await.unwrap_or_default(),
            None => Vec::new(),
        };

        let stdout = stdout_lines.join("\n");
        let stderr = stderr_lines.join("\n");

        if status.success() {
            Ok(AgentResult {
                success: true,
                message: if stdout.is_empty() {
                    "Agent completed successfully.".to_owned()
                } else {
                    stdout
                },
            })
        } else {
            Ok(AgentResult {
                success: false,
                message: if stderr.is_empty() { stdout } else { stderr },
            })
        }
    }
}

impl AgentProvider for CursorAgent {
    fn name(&self) -> &str {
        "Cursor"
    }

    fn is_available(&self) -> Result<bool> {
        Ok(Command::new(Self::BIN)
            .arg("--version")
            .output()
            .is_ok_and(|o| o.status.success()))
    }

    /// Fetch available models from the Cursor CLI, with a two-level fallback:
    ///
    /// 1. **Live**: `cursor-agent models` — on success, result is persisted to
    ///    disk so future fallbacks stay up-to-date.
    /// 2. **Disk cache**: `~/.config/prai/models_cache.json` — populated by the
    ///    last successful live fetch.
    /// 3. **Compile-time defaults**: used only on the very first run.
    async fn supported_models(&self) -> Result<Vec<String>> {
        // ── 1. Live fetch ─────────────────────────────────────────────────
        let output = tokio::process::Command::new(Self::BIN)
            .arg("models")
            .output()
            .await;

        if let Ok(output) = output {
            if output.status.success() {
                let raw = String::from_utf8_lossy(&output.stdout);
                let models = Self::parse_models_output(&raw);
                if !models.is_empty() {
                    // Persist so the disk cache stays fresh.
                    Self::write_cache(&models);
                    return Ok(models);
                }
            }
        }

        // ── 2. Disk cache ─────────────────────────────────────────────────
        if let Some(cached) = Self::read_cache() {
            if !cached.is_empty() {
                return Ok(cached);
            }
        }

        // ── 3. Compile-time defaults ──────────────────────────────────────
        Ok(Self::DEFAULTS.iter().map(|s| s.to_string()).collect())
    }

    async fn execute(
        &self,
        prompt: &str,
        model: Option<&str>,
        working_dir: &Path,
    ) -> Result<AgentResult> {
        let mut cmd = tokio::process::Command::new(Self::BIN);
        cmd.current_dir(working_dir).arg("-p").arg(prompt);

        if let Some(model) = model {
            cmd.arg("--model").arg(model);
        }

        let output = cmd
            .output()
            .await
            .context("failed to execute Cursor CLI agent")?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if output.status.success() {
            Ok(AgentResult {
                success: true,
                message: if stdout.is_empty() {
                    "Agent completed successfully.".to_owned()
                } else {
                    stdout
                },
            })
        } else {
            Ok(AgentResult {
                success: false,
                message: if stderr.is_empty() { stdout } else { stderr },
            })
        }
    }
}
