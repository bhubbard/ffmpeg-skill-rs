// ============================================================================
// crop.rs — Pixel-exact bounding box media cropper
// Part of ffmpeg-skill-rs
// ============================================================================

use crate::error::FfmpegSkillError;
use crate::probe::probe_file;
use crate::runner::run_ffmpeg;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Options configuring a crop operation.
#[derive(Debug, Clone)]
pub struct CropOptions {
    pub input: PathBuf,
    pub output: Option<PathBuf>,
    pub width: u32,
    pub height: u32,
    pub x: u32,
    pub y: u32,
    pub crf: Option<u32>,
    pub preset: Option<String>,
    pub dry_run: bool,
    pub timeout: Option<f64>,
}

/// Structured crop result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CropResult {
    pub status: String,
    pub input: String,
    pub output: String,
    pub original_width: u32,
    pub original_height: u32,
    pub crop_width: u32,
    pub crop_height: u32,
    pub crop_x: u32,
    pub crop_y: u32,
    pub duration: f64,
    pub commands: Vec<String>,
}

/// Crops a video to the specified bounding box rectangle.
pub async fn crop_video(opts: &CropOptions) -> Result<CropResult, FfmpegSkillError> {
    if !opts.input.exists() {
        return Err(FfmpegSkillError::input_error(format!(
            "Input file '{}' does not exist",
            opts.input.display()
        )));
    }

    // Inspect input dimensions
    let probe = probe_file(&opts.input).await?;
    let (orig_w, orig_h) = probe
        .resolution()
        .ok_or_else(|| FfmpegSkillError::input_error("Input file has no video stream to crop"))?;

    // Validate boundaries
    if opts.width == 0 || opts.height == 0 {
        return Err(FfmpegSkillError::input_error(
            "Crop width and height must be greater than 0",
        ));
    }

    if opts.x + opts.width > orig_w {
        return Err(FfmpegSkillError::input_error(format!(
            "Crop rectangle exceeds video width: x ({}) + width ({}) = {} > orig_width ({})",
            opts.x, opts.width, opts.x + opts.width, orig_w
        )));
    }

    if opts.y + opts.height > orig_h {
        return Err(FfmpegSkillError::input_error(format!(
            "Crop rectangle exceeds video height: y ({}) + height ({}) = {} > orig_height ({})",
            opts.y, opts.height, opts.y + opts.height, orig_h
        )));
    }

    // Ensure even dimensions for YUV420p video codecs
    let eff_w = if opts.width % 2 != 0 { opts.width - 1 } else { opts.width };
    let eff_h = if opts.height % 2 != 0 { opts.height - 1 } else { opts.height };

    // Resolve output path
    let output = match &opts.output {
        Some(p) => p.clone(),
        None => {
            let stem = opts
                .input
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("output");
            let ext = opts
                .input
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("mp4");
            let mut out = opts.input.with_file_name(format!(
                "{}_crop_{}x{}_{}_{}.{}",
                stem, eff_w, eff_h, opts.x, opts.y, ext
            ));
            let mut counter = 1;
            while out.exists() {
                out = opts.input.with_file_name(format!(
                    "{}_crop_{}x{}_{}_{}_{}.{}",
                    stem, eff_w, eff_h, opts.x, opts.y, counter, ext
                ));
                counter += 1;
            }
            out
        }
    };

    let crop_filter = format!("crop={}:{}:{}:{}", eff_w, eff_h, opts.x, opts.y);
    let crf = opts.crf.unwrap_or(18);
    let preset = opts.preset.as_deref().unwrap_or("fast");

    let args = vec![
        "-y".to_string(),
        "-i".to_string(),
        opts.input.to_string_lossy().to_string(),
        "-vf".to_string(),
        crop_filter,
        "-c:v".to_string(),
        "libx264".to_string(),
        "-crf".to_string(),
        crf.to_string(),
        "-preset".to_string(),
        preset.to_string(),
        "-c:a".to_string(),
        "copy".to_string(),
        "-movflags".to_string(),
        "+faststart".to_string(),
        output.to_string_lossy().to_string(),
    ];

    let full_command = format!("ffmpeg {}", args.join(" "));

    if !opts.dry_run {
        let runner_res = run_ffmpeg(&args, opts.timeout).await?;
        if !runner_res.success() {
            return Err(FfmpegSkillError::ffmpeg_error(
                format!("Crop execution failed: {}", runner_res.stderr),
                Some(runner_res.exit_code()),
                Some(args),
            ));
        }

        // Verify output exists and is probeable
        let out_probe = probe_file(&output).await.map_err(|e| {
            FfmpegSkillError::verification_error(format!(
                "Failed to probe cropped output file: {}",
                e
            ))
        })?;

        Ok(CropResult {
            status: "success".to_string(),
            input: opts.input.to_string_lossy().to_string(),
            output: output.to_string_lossy().to_string(),
            original_width: orig_w,
            original_height: orig_h,
            crop_width: eff_w,
            crop_height: eff_h,
            crop_x: opts.x,
            crop_y: opts.y,
            duration: out_probe.duration,
            commands: vec![full_command],
        })
    } else {
        Ok(CropResult {
            status: "dry_run".to_string(),
            input: opts.input.to_string_lossy().to_string(),
            output: output.to_string_lossy().to_string(),
            original_width: orig_w,
            original_height: orig_h,
            crop_width: eff_w,
            crop_height: eff_h,
            crop_x: opts.x,
            crop_y: opts.y,
            duration: probe.duration,
            commands: vec![full_command],
        })
    }
}
