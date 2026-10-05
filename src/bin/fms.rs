// ============================================================================
// fms.rs — Alias command-line entry point for fms
// ffmpeg-skill-rs
// ============================================================================

use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    ffmpeg_skill::run_cli().await
}
