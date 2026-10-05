// ============================================================================
// cut.rs — Frame-accurate and lossless stream-copy media segment cutter
// Part of ffmpeg-skill-rs
// ============================================================================

use crate::error::FfmpegSkillError;
use crate::probe::probe_file;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Options configuring a cut operation.
#[derive(Debug, Clone)]
pub struct CutOptions {
    pub input: PathBuf,
    pub output: Option<PathBuf>,
    pub start: Option<f64>,
    pub end: Option<f64>,
    pub duration: Option<f64>,
    pub segments: Option<String>,
    pub accurate: bool,
    pub dry_run: bool,
    pub timeout: Option<f64>,
}

/// Structured execution result of cut.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CutResult {
    pub status: String,
    pub input: String,
    pub output: String,
    pub mode: String,
    pub requested_start: Option<f64>,
    pub requested_end: Option<f64>,
    pub requested_duration: Option<f64>,
    pub output_duration: f64,
    pub duration_delta_seconds: f64,
    pub commands: Vec<String>,
}

/// Parses time strings (e.g. "00:01:23.45" or "83.45" or "1:23") into seconds.
pub fn parse_time_to_seconds(time_str: &str) -> Option<f64> {
    let trimmed = time_str.trim();
    if let Ok(secs) = trimmed.parse::<f64>() {
        return Some(secs);
    }
    let parts: Vec<&str> = trimmed.split(':').collect();
    match parts.len() {
        2 => {
            let mins: f64 = parts[0].parse().ok()?;
            let secs: f64 = parts[1].parse().ok()?;
            Some(mins * 60.0 + secs)
        }
        3 => {
            let hours: f64 = parts[0].parse().ok()?;
            let mins: f64 = parts[1].parse().ok()?;
            let secs: f64 = parts[2].parse().ok()?;
            Some(hours * 3600.0 + mins * 60.0 + secs)
        }
        _ => None,
    }
}

/// Cut media reference wrapper.
pub async fn cut_media(opts: &CutOptions) -> Result<CutResult, FfmpegSkillError> {
    execute_cut(opts.clone()).await
}

/// Executes a media cut.
pub async fn execute_cut(opts: CutOptions) -> Result<CutResult, FfmpegSkillError> {
    let meta = probe_file(&opts.input).await?;
    let in_str = opts.input.to_string_lossy().to_string();

    let start_sec = opts.start.unwrap_or(0.0);
    let end_sec = if let Some(dur) = opts.duration {
        start_sec + dur
    } else if let Some(e) = opts.end {
        e
    } else {
        meta.duration
    };

    let requested_duration = end_sec - start_sec;
    if requested_duration <= 0.0 {
        return Err(FfmpegSkillError::Input {
            message: format!("Invalid cut duration: start ({}) >= end ({})", start_sec, end_sec),
            hint: Some("Specify --start before --end or a positive --duration.".to_string()),
        });
    }

    let out_path = opts.output.clone().unwrap_or_else(|| {
        let stem = opts.input.file_stem().and_then(|s| s.to_str()).unwrap_or("clip");
        let ext = opts.input.extension().and_then(|s| s.to_str()).unwrap_or("mp4");
        opts.input.with_file_name(format!("{}_cut.{}", stem, ext))
    });
    let out_str = out_path.to_string_lossy().to_string();

    // Check if extracting audio only (output extension is audio format while input has video)
    let is_audio_ext = match out_path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase().as_str() {
        "mp3" | "wav" | "m4a" | "aac" | "flac" | "ogg" | "opus" => true,
        _ => false,
    };

    let mut args = Vec::new();
    let mode = if opts.accurate || is_audio_ext {
        // Accurate frame-level re-encode
        args.push("-ss".to_string());
        args.push(format!("{:.3}", start_sec));
        args.push("-i".to_string());
        args.push(in_str.clone());
        args.push("-t".to_string());
        args.push(format!("{:.3}", requested_duration));

        if is_audio_ext {
            args.push("-vn".to_string());
            if out_str.ends_with(".wav") {
                args.push("-c:a".to_string());
                args.push("pcm_s16le".to_string());
            } else if out_str.ends_with(".m4a") || out_str.ends_with(".aac") {
                args.push("-c:a".to_string());
                args.push("aac".to_string());
                args.push("-b:a".to_string());
                args.push("192k".to_string());
            }
        } else {
            args.push("-c:v".to_string());
            args.push("libx264".to_string());
            args.push("-crf".to_string());
            args.push("18".to_string());
            args.push("-preset".to_string());
            args.push("medium".to_string());
            args.push("-c:a".to_string());
            args.push("copy".to_string());
        }
        "accurate"
    } else {
        // Fast lossless stream-copy
        args.push("-ss".to_string());
        args.push(format!("{:.3}", start_sec));
        args.push("-i".to_string());
        args.push(in_str.clone());
        args.push("-t".to_string());
        args.push(format!("{:.3}", requested_duration));
        args.push("-c".to_string());
        args.push("copy".to_string());
        "copy"
    };

    args.push("-y".to_string());
    args.push(out_str.clone());

    let proc_res = crate::runner::run_ffmpeg_dry(&args, opts.timeout, opts.dry_run).await?;
    if !proc_res.success() && !opts.dry_run {
        return Err(FfmpegSkillError::Ffmpeg {
            exit_code: proc_res.status,
            message: format!("ffmpeg cut failed:\n{}", proc_res.stderr.trim()),
            hint: Some("Try --accurate mode to re-encode past non-keyframe boundaries.".to_string()),
        });
    }

    let output_duration = if opts.dry_run {
        requested_duration
    } else {
        match probe_file(&out_path).await {
            Ok(out_meta) => out_meta.duration,
            Err(_) => requested_duration,
        }
    };

    let delta = (output_duration - requested_duration).abs();

    Ok(CutResult {
        status: "ok".to_string(),
        input: in_str,
        output: out_str,
        mode: mode.to_string(),
        requested_start: Some(start_sec),
        requested_end: Some(end_sec),
        requested_duration: Some(requested_duration),
        output_duration,
        duration_delta_seconds: delta,
        commands: vec![proc_res.command_line],
    })
}
