// ============================================================================
// main.rs — Command-line entry point for ffmpeg-skill
// ffmpeg-skill-rs
// ============================================================================

use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    ffmpeg_skill::run_cli().await
}
