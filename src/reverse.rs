// ============================================================================
// reverse.rs — Video and audio playback reversal processor
// Part of ffmpeg-skill-rs
// ============================================================================

use crate::error::FfmpegSkillError;
use crate::probe::probe_file;
use crate::runner::run_ffmpeg;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Options configuring media reversal.
#[derive(Debug, Clone)]
pub struct ReverseOptions {
    pub input: PathBuf,
    pub output: Option<PathBuf>,
    pub video_only: bool,
    pub audio_only: bool,
    pub dry_run: bool,
    pub timeout: Option<f64>,
}

/// Structured reversal result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReverseResult {
    pub status: String,
    pub input: String,
    pub output: String,
    pub reversed_video: bool,
    pub reversed_audio: bool,
    pub duration: f64,
    pub commands: Vec<String>,
}

/// Reverses media playback backwards in time.
pub async fn reverse_media(opts: &ReverseOptions) -> Result<ReverseResult, FfmpegSkillError> {
    if !opts.input.exists() {
        return Err(FfmpegSkillError::input_error(format!(
            "Input file '{}' does not exist",
            opts.input.display()
        )));
    }

    let probe = probe_file(&opts.input).await?;
    let has_video = probe.has_video();
    let has_audio = probe.has_audio();

    let rev_v = has_video && !opts.audio_only;
    let rev_a = has_audio && !opts.video_only;

    if !rev_v && !rev_a {
        return Err(FfmpegSkillError::input_error(
            "Nothing to reverse with the specified options",
        ));
    }

    let output = match &opts.output {
        Some(p) => p.clone(),
        None => {
            let stem = opts.input.file_stem().and_then(|s| s.to_str()).unwrap_or("reversed");
            let ext = opts.input.extension().and_then(|e| e.to_str()).unwrap_or("mp4");
            let mut out = opts.input.with_file_name(format!("{}_reversed.{}", stem, ext));
            let mut counter = 1;
            while out.exists() {
                out = opts.input.with_file_name(format!("{}_reversed_{}.{}", stem, counter, ext));
                counter += 1;
            }
            out
        }
    };

    let mut args = vec!["-y".to_string(), "-i".to_string(), opts.input.to_string_lossy().to_string()];

    if rev_v {
        args.extend(vec![
            "-vf".to_string(),
            "reverse".to_string(),
            "-c:v".to_string(),
            "libx264".to_string(),
            "-crf".to_string(),
            "18".to_string(),
            "-preset".to_string(),
            "fast".to_string(),
        ]);
    } else if has_video {
        args.extend(vec!["-c:v".to_string(), "copy".to_string()]);
    }

    if rev_a {
        args.extend(vec![
            "-af".to_string(),
            "areverse".to_string(),
            "-c:a".to_string(),
            "aac".to_string(),
            "-b:a".to_string(),
            "192k".to_string(),
        ]);
    } else if has_audio {
        args.extend(vec!["-c:a".to_string(), "copy".to_string()]);
    }

    args.extend(vec![
        "-movflags".to_string(),
        "+faststart".to_string(),
        output.to_string_lossy().to_string(),
    ]);

    let commands = vec![format!("ffmpeg {}", args.join(" "))];

    if !opts.dry_run {
        let runner_res = run_ffmpeg(&args, opts.timeout).await?;
        if !runner_res.success() {
            return Err(FfmpegSkillError::ffmpeg_error(
                format!("Media reversal execution failed: {}", runner_res.stderr),
                Some(runner_res.exit_code()),
                Some(args),
            ));
        }

        let out_probe = probe_file(&output).await.map_err(|e| {
            FfmpegSkillError::verification_error(format!("Failed to probe reversed output: {}", e))
        })?;

        Ok(ReverseResult {
            status: "success".to_string(),
            input: opts.input.to_string_lossy().to_string(),
            output: output.to_string_lossy().to_string(),
            reversed_video: rev_v,
            reversed_audio: rev_a,
            duration: out_probe.duration,
            commands,
        })
    } else {
        Ok(ReverseResult {
            status: "dry_run".to_string(),
            input: opts.input.to_string_lossy().to_string(),
            output: output.to_string_lossy().to_string(),
            reversed_video: rev_v,
            reversed_audio: rev_a,
            duration: probe.duration,
            commands,
        })
    }
}
