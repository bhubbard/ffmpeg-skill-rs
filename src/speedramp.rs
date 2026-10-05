// ============================================================================
// speedramp.rs — Playback speed manipulation and atempo audio time-stretching
// Part of ffmpeg-skill-rs
// ============================================================================

use crate::error::FfmpegSkillError;
use crate::probe::probe_file;
use crate::runner::run_ffmpeg;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Options configuring speed change.
#[derive(Debug, Clone)]
pub struct SpeedOptions {
    pub input: PathBuf,
    pub output: Option<PathBuf>,
    pub speed: f64,
    pub preserve_pitch: bool,
    pub dry_run: bool,
    pub timeout: Option<f64>,
}

/// Structured speed modification result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeedResult {
    pub status: String,
    pub input: String,
    pub output: String,
    pub speed_factor: f64,
    pub original_duration: f64,
    pub new_duration: f64,
    pub commands: Vec<String>,
}

/// Builds cascaded atempo filter chain to support arbitrary speed factors (0.1x to 100x).
pub fn build_atempo_chain(mut speed: f64) -> String {
    let mut filters = Vec::new();
    if speed >= 1.0 {
        while speed > 2.0 {
            filters.push("atempo=2.0".to_string());
            speed /= 2.0;
        }
        filters.push(format!("atempo={:.4}", speed));
    } else {
        while speed < 0.5 {
            filters.push("atempo=0.5".to_string());
            speed /= 0.5;
        }
        filters.push(format!("atempo={:.4}", speed));
    }
    filters.join(",")
}

/// Modifies playback speed of video and audio in lockstep.
pub async fn change_speed(opts: &SpeedOptions) -> Result<SpeedResult, FfmpegSkillError> {
    if !opts.input.exists() {
        return Err(FfmpegSkillError::input_error(format!(
            "Input file '{}' does not exist",
            opts.input.display()
        )));
    }

    if opts.speed <= 0.0 {
        return Err(FfmpegSkillError::input_error("Speed factor must be greater than 0"));
    }

    let probe = probe_file(&opts.input).await?;
    let orig_dur = probe.duration;
    let expected_dur = if opts.speed > 0.0 { orig_dur / opts.speed } else { 0.0 };

    let output = match &opts.output {
        Some(p) => p.clone(),
        None => {
            let stem = opts.input.file_stem().and_then(|s| s.to_str()).unwrap_or("speed");
            let ext = opts.input.extension().and_then(|e| e.to_str()).unwrap_or("mp4");
            let mut out = opts.input.with_file_name(format!("{}_{:.2}x.{}", stem, opts.speed, ext));
            let mut counter = 1;
            while out.exists() {
                out = opts.input.with_file_name(format!("{}_{:.2}x_{}.{}", stem, opts.speed, counter, ext));
                counter += 1;
            }
            out
        }
    };

    let pts_factor = 1.0 / opts.speed;
    let vf = format!("setpts={:.6}*PTS", pts_factor);
    let af = build_atempo_chain(opts.speed);

    let has_video = probe.has_video();
    let has_audio = probe.has_audio();

    let mut args = vec!["-y".to_string(), "-i".to_string(), opts.input.to_string_lossy().to_string()];

    if has_video {
        args.extend(vec![
            "-vf".to_string(),
            vf,
            "-c:v".to_string(),
            "libx264".to_string(),
            "-crf".to_string(),
            "18".to_string(),
            "-preset".to_string(),
            "fast".to_string(),
        ]);
    }

    if has_audio {
        args.extend(vec![
            "-af".to_string(),
            af,
            "-c:a".to_string(),
            "aac".to_string(),
            "-b:a".to_string(),
            "192k".to_string(),
        ]);
    }

    args.extend(vec![
        "-movflags".to_string(),
        "+faststart".to_string(),
        output.to_string_lossy().to_string(),
    ]);

    let cmd_str = format!("ffmpeg {}", args.join(" "));
    let commands = vec![cmd_str];

    if !opts.dry_run {
        let runner_res = run_ffmpeg(&args, opts.timeout).await?;
        if !runner_res.success() {
            return Err(FfmpegSkillError::ffmpeg_error(
                format!("Speed modification execution failed: {}", runner_res.stderr),
                Some(runner_res.exit_code()),
                Some(args),
            ));
        }

        let out_probe = probe_file(&output).await.map_err(|e| {
            FfmpegSkillError::verification_error(format!("Failed to probe speed modified output: {}", e))
        })?;

        Ok(SpeedResult {
            status: "success".to_string(),
            input: opts.input.to_string_lossy().to_string(),
            output: output.to_string_lossy().to_string(),
            speed_factor: opts.speed,
            original_duration: orig_dur,
            new_duration: out_probe.duration,
            commands,
        })
    } else {
        Ok(SpeedResult {
            status: "dry_run".to_string(),
            input: opts.input.to_string_lossy().to_string(),
            output: output.to_string_lossy().to_string(),
            speed_factor: opts.speed,
            original_duration: orig_dur,
            new_duration: expected_dur,
            commands,
        })
    }
}
