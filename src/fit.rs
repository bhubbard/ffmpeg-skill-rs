// ============================================================================
// fit.rs — Aspect ratio conforming, padding, letterboxing, and duration scaling
// Part of ffmpeg-skill-rs
// ============================================================================

use crate::error::FfmpegSkillError;
use crate::probe::probe_file;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Aspect ratio conforming strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FitMode {
    Pad,
    Crop,
    Blur,
}

/// Duration adjustment method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DurationMethod {
    Trim,
    Speed,
}

/// Options for the fit command.
#[derive(Debug, Clone)]
pub struct FitOptions {
    pub input: PathBuf,
    pub output: Option<PathBuf>,
    pub aspect: Option<String>,
    pub fit_mode: FitMode,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration: Option<f64>,
    pub duration_method: DurationMethod,
    pub rotate: Option<i32>,
    pub flip: Option<String>,
    pub fps: Option<f64>,
    pub dry_run: bool,
    pub timeout: Option<f64>,
}

/// Structured fit execution result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FitResult {
    pub status: String,
    pub input: String,
    pub output: String,
    pub original_width: u32,
    pub original_height: u32,
    pub output_width: u32,
    pub output_height: u32,
    pub duration: f64,
    pub commands: Vec<String>,
}

/// Helper to parse aspect ratio string like "16:9", "9:16", "1:1", "4:5".
fn parse_aspect(aspect_str: &str) -> Option<(u32, u32)> {
    let parts: Vec<&str> = aspect_str.split(':').collect();
    if parts.len() == 2 {
        let w: u32 = parts[0].trim().parse().ok()?;
        let h: u32 = parts[1].trim().parse().ok()?;
        if w > 0 && h > 0 {
            return Some((w, h));
        }
    }
    None
}

/// Computes filtergraph for aspect conforming (pad, crop, blur).
fn build_aspect_filter(
    _src_w: u32,
    _src_h: u32,
    target_w: u32,
    target_h: u32,
    mode: FitMode,
) -> String {
    match mode {
        FitMode::Pad => {
            // Scale keeping aspect ratio then pad
            format!(
                "scale={}:{}:force_original_aspect_ratio=decrease,pad={}:{}:(ow-iw)/2:(oh-ih)/2:black",
                target_w, target_h, target_w, target_h
            )
        }
        FitMode::Crop => {
            // Scale to fill then crop center
            format!(
                "scale={}:{}:force_original_aspect_ratio=increase,crop={}:{}",
                target_w, target_h, target_w, target_h
            )
        }
        FitMode::Blur => {
            // Split into background blur and foreground
            format!(
                "[0:v]scale={}:{}:force_original_aspect_ratio=increase,boxblur=luma_radius=min(h\\,w)/20:luma_power=2,crop={}:{}[bg];[0:v]scale={}:{}:force_original_aspect_ratio=decrease[fg];[bg][fg]overlay=(W-w)/2:(H-h)/2",
                target_w, target_h, target_w, target_h, target_w, target_h
            )
        }
    }
}

/// Generates atempo audio filter chain for arbitrary speed factors.
pub fn generate_atempo_chain(factor: f64) -> String {
    let mut parts = Vec::new();
    let mut remaining = factor;
    while remaining < 0.5 {
        parts.push("atempo=0.5".to_string());
        remaining /= 0.5;
    }
    while remaining > 100.0 {
        parts.push("atempo=100.0".to_string());
        remaining /= 100.0;
    }
    parts.push(format!("atempo={:.6}", remaining));
    parts.join(",")
}

/// Fit media reference wrapper.
pub async fn fit_media(opts: &FitOptions) -> Result<FitResult, FfmpegSkillError> {
    execute_fit(opts.clone()).await
}

/// Executes video fitting operation.
pub async fn execute_fit(opts: FitOptions) -> Result<FitResult, FfmpegSkillError> {
    let meta = probe_file(&opts.input).await?;
    let video = meta.video.as_ref().ok_or_else(|| FfmpegSkillError::Input {
        message: "input has no video stream to fit".to_string(),
        hint: Some("Provide a video input file.".to_string()),
    })?;

    let src_w = video.width;
    let src_h = video.height;

    // Determine target dimensions
    let (target_w, target_h) = if let (Some(w), Some(h)) = (opts.width, opts.height) {
        (w, h)
    } else if let Some(ref aspect) = opts.aspect {
        let (aw, ah) = parse_aspect(aspect).ok_or_else(|| FfmpegSkillError::Input {
            message: format!("Invalid aspect ratio '{}'. Supported: 16:9, 9:16, 1:1, 4:5", aspect),
            hint: None,
        })?;
        if let Some(w) = opts.width {
            let h = ((w as f64 * ah as f64) / aw as f64).round() as u32;
            (w, (h / 2) * 2) // ensure even
        } else if let Some(h) = opts.height {
            let w = ((h as f64 * aw as f64) / ah as f64).round() as u32;
            ((w / 2) * 2, h)
        } else {
            // Keep height, adjust width or vice versa
            let h = src_h;
            let w = ((h as f64 * aw as f64) / ah as f64).round() as u32;
            ((w / 2) * 2, h)
        }
    } else if let Some(w) = opts.width {
        let h = ((w as f64 * src_h as f64) / src_w as f64).round() as u32;
        (w, (h / 2) * 2)
    } else if let Some(h) = opts.height {
        let w = ((h as f64 * src_w as f64) / src_h as f64).round() as u32;
        ((w / 2) * 2, h)
    } else {
        (src_w, src_h)
    };

    let out_path = opts.output.clone().unwrap_or_else(|| {
        let stem = opts.input.file_stem().and_then(|s| s.to_str()).unwrap_or("clip");
        let ext = opts.input.extension().and_then(|s| s.to_str()).unwrap_or("mp4");
        opts.input.with_file_name(format!("{}_fit.{}", stem, ext))
    });
    let out_str = out_path.to_string_lossy().to_string();

    let in_str = opts.input.to_string_lossy().to_string();
    let mut args = Vec::new();
    args.push("-i".to_string());
    args.push(in_str.clone());

    let mut vf_filters = Vec::new();

    // Sizing / aspect filter
    if target_w != src_w || target_h != src_h || opts.aspect.is_some() {
        vf_filters.push(build_aspect_filter(src_w, src_h, target_w, target_h, opts.fit_mode));
    }

    // Rotation / Flip
    if let Some(deg) = opts.rotate {
        match deg {
            90 => vf_filters.push("transpose=1".to_string()),
            180 => vf_filters.push("transpose=2,transpose=2".to_string()),
            270 => vf_filters.push("transpose=2".to_string()),
            _ => {}
        }
    }
    if let Some(ref flip) = opts.flip {
        match flip.as_str() {
            "h" => vf_filters.push("hflip".to_string()),
            "v" => vf_filters.push("vflip".to_string()),
            _ => {}
        }
    }

    // Frame rate conforming
    if let Some(target_fps) = opts.fps {
        vf_filters.push(format!("fps={:.3}", target_fps));
    }

    // Duration fitting
    let mut af_filters = Vec::new();
    if let Some(target_dur) = opts.duration {
        if opts.duration_method == DurationMethod::Speed && meta.duration > 0.0 {
            let speed_factor = meta.duration / target_dur;
            vf_filters.push(format!("setpts={:.6}*PTS", 1.0 / speed_factor));
            af_filters.push(generate_atempo_chain(speed_factor));
        } else {
            args.push("-t".to_string());
            args.push(format!("{:.3}", target_dur));
        }
    }

    if !vf_filters.is_empty() {
        args.push("-vf".to_string());
        args.push(vf_filters.join(","));
    }

    if !af_filters.is_empty() {
        args.push("-af".to_string());
        args.push(af_filters.join(","));
    }

    // Encoder configuration
    args.push("-c:v".to_string());
    args.push("libx264".to_string());
    args.push("-crf".to_string());
    args.push("18".to_string());
    args.push("-preset".to_string());
    args.push("medium".to_string());
    args.push("-pix_fmt".to_string());
    args.push("yuv420p".to_string());

    if meta.audio.is_some() {
        if af_filters.is_empty() && opts.duration_method != DurationMethod::Speed {
            args.push("-c:a".to_string());
            args.push("copy".to_string());
        } else {
            args.push("-c:a".to_string());
            args.push("aac".to_string());
            args.push("-b:a".to_string());
            args.push("192k".to_string());
        }
    }

    args.push("-y".to_string());
    args.push(out_str.clone());

    let proc_res = crate::runner::run_ffmpeg_dry(&args, opts.timeout, opts.dry_run).await?;
    if !proc_res.success() && !opts.dry_run {
        return Err(FfmpegSkillError::Ffmpeg {
            exit_code: proc_res.status,
            message: format!("ffmpeg fit failed:\n{}", proc_res.stderr.trim()),
            hint: None,
        });
    }

    let final_dur = if opts.dry_run {
        opts.duration.unwrap_or(meta.duration)
    } else {
        match probe_file(&out_path).await {
            Ok(m) => m.duration,
            Err(_) => meta.duration,
        }
    };

    Ok(FitResult {
        status: "ok".to_string(),
        input: in_str,
        output: out_str,
        original_width: src_w,
        original_height: src_h,
        output_width: target_w,
        output_height: target_h,
        duration: final_dur,
        commands: vec![proc_res.command_line],
    })
}
