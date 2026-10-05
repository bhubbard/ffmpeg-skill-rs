// ============================================================================
// scenes.rs — Scene cut detection and keyframe thumbnail extractor
// Part of ffmpeg-skill-rs
// ============================================================================

use crate::error::FfmpegSkillError;
use crate::probe::probe_file;
use crate::runner::run_ffmpeg;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A detected scene cut point.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneCut {
    pub index: usize,
    pub timestamp_seconds: f64,
    pub frame_number: Option<u64>,
    pub score: Option<f64>,
}

/// Options configuring scene detection.
#[derive(Debug, Clone)]
pub struct SceneOptions {
    pub input: PathBuf,
    pub threshold: f64,
    pub thumbnails_dir: Option<PathBuf>,
    pub limit: Option<f64>,
    pub dry_run: bool,
    pub timeout: Option<f64>,
}

/// Structured scene cut detection result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneResult {
    pub status: String,
    pub input: String,
    pub threshold: f64,
    pub total_scenes_detected: usize,
    pub cuts: Vec<SceneCut>,
    pub thumbnails_dir: Option<String>,
    pub commands: Vec<String>,
}

/// Detects scene transitions and cuts throughout a video.
pub async fn detect_scenes(opts: &SceneOptions) -> Result<SceneResult, FfmpegSkillError> {
    if !opts.input.exists() {
        return Err(FfmpegSkillError::input_error(format!(
            "Input file '{}' does not exist",
            opts.input.display()
        )));
    }

    let probe = probe_file(&opts.input).await?;
    if !probe.has_video() {
        return Err(FfmpegSkillError::input_error(
            "Input file has no video stream to analyze for scenes",
        ));
    }

    let threshold = if opts.threshold <= 0.0 || opts.threshold >= 1.0 {
        0.3
    } else {
        opts.threshold
    };

    let filter = format!("select='gt(scene,{})',showinfo", threshold);
    let mut args = vec!["-hide_banner".to_string()];

    if let Some(limit_sec) = opts.limit {
        args.push("-t".to_string());
        args.push(limit_sec.to_string());
    }

    args.extend(vec![
        "-i".to_string(),
        opts.input.to_string_lossy().to_string(),
        "-vf".to_string(),
        filter,
        "-f".to_string(),
        "null".to_string(),
        "-".to_string(),
    ]);

    let mut commands = vec![format!("ffmpeg {}", args.join(" "))];
    let runner_res = run_ffmpeg(&args, opts.timeout).await?;
    let stderr = runner_res.stderr;

    // Parse showinfo lines:
    // [Parsed_showinfo_1 @ 0x...] n:   42 pts:  70070 pts_time:2.33567 ...
    let time_re = Regex::new(r"n:\s*(\d+).*?pts_time:\s*([0-9.]+)").unwrap();
    let mut cuts = Vec::new();
    let mut idx = 1;

    for line in stderr.lines() {
        if line.contains("Parsed_showinfo") {
            if let Some(caps) = time_re.captures(line) {
                let frame: u64 = caps[1].parse().unwrap_or(0);
                let pts_time: f64 = caps[2].parse().unwrap_or(0.0);
                cuts.push(SceneCut {
                    index: idx,
                    timestamp_seconds: pts_time,
                    frame_number: Some(frame),
                    score: None,
                });
                idx += 1;
            }
        }
    }

    let mut thumb_out = None;

    if let Some(ref dir) = opts.thumbnails_dir {
        std::fs::create_dir_all(dir).map_err(|e| {
            FfmpegSkillError::output_error(format!("Failed to create thumbnails dir: {}", e))
        })?;

        let thumb_pattern = dir.join("scene_%03d.jpg");
        let extract_args = vec![
            "-y".to_string(),
            "-i".to_string(),
            opts.input.to_string_lossy().to_string(),
            "-vf".to_string(),
            format!("select='gt(scene,{})'", threshold),
            "-vsync".to_string(),
            "vfr".to_string(),
            "-q:v".to_string(),
            "2".to_string(),
            thumb_pattern.to_string_lossy().to_string(),
        ];

        commands.push(format!("ffmpeg {}", extract_args.join(" ")));

        if !opts.dry_run {
            let thumb_res = run_ffmpeg(&extract_args, opts.timeout).await?;
            if !thumb_res.success() {
                return Err(FfmpegSkillError::ffmpeg_error(
                    format!("Scene thumbnail extraction failed: {}", thumb_res.stderr),
                    Some(thumb_res.exit_code()),
                    Some(extract_args),
                ));
            }
        }

        thumb_out = Some(dir.to_string_lossy().to_string());
    }

    Ok(SceneResult {
        status: if opts.dry_run { "dry_run".to_string() } else { "success".to_string() },
        input: opts.input.to_string_lossy().to_string(),
        threshold,
        total_scenes_detected: cuts.len(),
        cuts,
        thumbnails_dir: thumb_out,
        commands,
    })
}
