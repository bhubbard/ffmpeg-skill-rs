// ============================================================================
// runner.rs — Subprocess execution, binary resolution, and timeout management
// Part of ffmpeg-skill-rs
// ============================================================================

use crate::error::{ExitCodes, FfmpegSkillError};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tracing::{debug, warn};

/// Default execution timeout in seconds (matching upstream 1800s).
pub const DEFAULT_TIMEOUT_SECS: f64 = 1800.0;

/// Output of a completed process run.
#[derive(Debug, Clone)]
pub struct ProcessOutput {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
    pub command_line: String,
}

impl ProcessOutput {
    pub fn success(&self) -> bool {
        self.status == 0
    }

    pub fn exit_code(&self) -> i32 {
        self.status
    }
}

/// Resolves a binary name on the host system's PATH.
pub fn resolve_binary(name: &str) -> Result<PathBuf, FfmpegSkillError> {
    match which::which(name) {
        Ok(path) => Ok(path),
        Err(_) => Err(FfmpegSkillError::MissingTool {
            tool: name.to_string(),
            message: format!("'{}' was not found on PATH. Please install FFmpeg (e.g. `brew install ffmpeg` on macOS or `apt install ffmpeg` on Linux).", name),
            hint: Some(format!("Ensure '{}' is installed and in your environment PATH.", name)),
        }),
    }
}

/// Runs a command with the specified arguments, capturing stdout and stderr.
pub async fn run_command(
    program: impl AsRef<Path>,
    args: &[String],
    timeout_secs: Option<f64>,
    dry_run: bool,
) -> Result<ProcessOutput, FfmpegSkillError> {
    let prog_str = program.as_ref().to_string_lossy().to_string();
    let cmd_line = format!("{} {}", prog_str, args.join(" "));

    if dry_run {
        debug!(cmd = %cmd_line, "Dry-run: skipping process execution");
        return Ok(ProcessOutput {
            status: 0,
            stdout: String::new(),
            stderr: String::new(),
            command_line: cmd_line,
        });
    }

    let timeout_duration = Duration::from_secs_f64(
        timeout_secs
            .or_else(|| {
                std::env::var("FFMPEG_SKILL_TIMEOUT")
                    .ok()
                    .and_then(|v| v.parse().ok())
            })
            .unwrap_or(DEFAULT_TIMEOUT_SECS),
    );

    let mut child = Command::new(program.as_ref())
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| FfmpegSkillError::Ffmpeg {
            exit_code: ExitCodes::FAILURE,
            message: format!("Failed to spawn process '{}': {}", prog_str, e),
            hint: None,
        })?;

    let stdout_pipe = child.stdout.take();
    let stderr_pipe = child.stderr.take();

    let stdout_task = tokio::spawn(async move {
        let mut buf = Vec::new();
        if let Some(mut stream) = stdout_pipe {
            use tokio::io::AsyncReadExt;
            let _ = stream.read_to_end(&mut buf).await;
        }
        String::from_utf8_lossy(&buf).to_string()
    });

    let stderr_task = tokio::spawn(async move {
        let mut buf = Vec::new();
        if let Some(mut stream) = stderr_pipe {
            use tokio::io::AsyncReadExt;
            let _ = stream.read_to_end(&mut buf).await;
        }
        String::from_utf8_lossy(&buf).to_string()
    });

    let timeout_res = tokio::time::timeout(timeout_duration, child.wait()).await;

    match timeout_res {
        Ok(Ok(status)) => {
            let stdout = stdout_task.await.unwrap_or_default();
            let stderr = stderr_task.await.unwrap_or_default();
            let exit_code = status.code().unwrap_or(ExitCodes::FAILURE);

            Ok(ProcessOutput {
                status: exit_code,
                stdout,
                stderr,
                command_line: cmd_line,
            })
        }
        Ok(Err(e)) => {
            stdout_task.abort();
            stderr_task.abort();
            Err(FfmpegSkillError::Ffmpeg {
                exit_code: ExitCodes::FAILURE,
                message: format!("Error reading process output: {}", e),
                hint: None,
            })
        }
        Err(_) => {
            warn!(cmd = %cmd_line, timeout_secs = timeout_duration.as_secs_f64(), "Process timed out; killing child");
            let _ = child.kill().await;
            stdout_task.abort();
            stderr_task.abort();
            Err(FfmpegSkillError::Timeout {
                seconds: timeout_duration.as_secs_f64(),
                hint: Some("Pass a larger --timeout value or optimize video resolution/preset.".to_string()),
            })
        }
    }
}

/// Runs ffmpeg with standard flags (`-hide_banner`, `-nostdin`).
pub async fn run_ffmpeg(
    args: &[String],
    timeout_secs: Option<f64>,
) -> Result<ProcessOutput, FfmpegSkillError> {
    let ffmpeg = resolve_binary("ffmpeg")?;
    let mut full_args = vec!["-hide_banner".to_string(), "-nostdin".to_string()];
    full_args.extend_from_slice(args);
    run_command(ffmpeg, &full_args, timeout_secs, false).await
}

/// Runs ffmpeg with an explicit dry_run flag.
pub async fn run_ffmpeg_dry(
    args: &[String],
    timeout_secs: Option<f64>,
    dry_run: bool,
) -> Result<ProcessOutput, FfmpegSkillError> {
    let ffmpeg = resolve_binary("ffmpeg")?;
    let mut full_args = vec!["-hide_banner".to_string(), "-nostdin".to_string()];
    full_args.extend_from_slice(args);
    run_command(ffmpeg, &full_args, timeout_secs, dry_run).await
}

/// Runs ffprobe with standard flags (`-v error`).
pub async fn run_ffprobe(
    args: &[String],
    timeout_secs: Option<f64>,
) -> Result<ProcessOutput, FfmpegSkillError> {
    let ffprobe = resolve_binary("ffprobe")?;
    let mut full_args = vec!["-v".to_string(), "error".to_string()];
    full_args.extend_from_slice(args);
    run_command(ffprobe, &full_args, timeout_secs, false).await
}
