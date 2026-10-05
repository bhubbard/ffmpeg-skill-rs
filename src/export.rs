// ============================================================================
// export.rs — Platform delivery and multi-format encoding presets
// Part of ffmpeg-skill-rs
// ============================================================================

use crate::doctor::inspect_capabilities;
use crate::error::FfmpegSkillError;
use crate::probe::probe_file;
use crate::runner::run_ffmpeg;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Platform delivery preset profiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportPreset {
    Youtube,
    Youtube4k,
    Reels,
    Tiktok,
    Shorts,
    Linkedin,
    Facebook,
    X,
    Prores,
    H265,
    Gif,
    Copy,
}

impl ExportPreset {
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "youtube" | "yt" => Some(Self::Youtube),
            "youtube4k" | "yt4k" | "4k" => Some(Self::Youtube4k),
            "reels" | "instagram" | "ig" => Some(Self::Reels),
            "tiktok" | "tt" => Some(Self::Tiktok),
            "shorts" => Some(Self::Shorts),
            "linkedin" | "li" => Some(Self::Linkedin),
            "facebook" | "fb" => Some(Self::Facebook),
            "x" | "twitter" => Some(Self::X),
            "prores" | "apple" => Some(Self::Prores),
            "h265" | "hevc" => Some(Self::H265),
            "gif" => Some(Self::Gif),
            "copy" | "remux" => Some(Self::Copy),
            _ => None,
        }
    }
}

/// Options configuring media export.
#[derive(Debug, Clone)]
pub struct ExportOptions {
    pub input: PathBuf,
    pub output: Option<PathBuf>,
    pub preset: ExportPreset,
    pub crf: Option<u32>,
    pub hwaccel: Option<String>,
    pub fps: Option<f64>,
    pub scale: Option<String>,
    pub dry_run: bool,
    pub timeout: Option<f64>,
}

/// Structured export result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportResult {
    pub status: String,
    pub input: String,
    pub output: String,
    pub preset: String,
    pub format: String,
    pub video_codec: String,
    pub audio_codec: String,
    pub resolution: Option<(u32, u32)>,
    pub duration: f64,
    pub file_size_bytes: u64,
    pub commands: Vec<String>,
}

/// Exports a media file using an optimized platform profile.
pub async fn export_media(opts: &ExportOptions) -> Result<ExportResult, FfmpegSkillError> {
    if !opts.input.exists() {
        return Err(FfmpegSkillError::input_error(format!(
            "Input file '{}' does not exist",
            opts.input.display()
        )));
    }

    let probe = probe_file(&opts.input).await?;
    let caps = inspect_capabilities().await.ok();

    // Determine target extension
    let target_ext = match opts.preset {
        ExportPreset::Gif => "gif",
        ExportPreset::Prores => "mov",
        _ => "mp4",
    };

    let output = match &opts.output {
        Some(p) => p.clone(),
        None => {
            let stem = opts
                .input
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("exported");
            let preset_name = format!("{:?}", opts.preset).to_lowercase();
            let mut out = opts
                .input
                .with_file_name(format!("{}_{}.{}", stem, preset_name, target_ext));
            let mut counter = 1;
            while out.exists() {
                out = opts.input.with_file_name(format!(
                    "{}_{}_{}.{}",
                    stem, preset_name, counter, target_ext
                ));
                counter += 1;
            }
            out
        }
    };

    // Determine hardware acceleration availability
    let hw_mode = opts.hwaccel.as_deref().unwrap_or("auto");
    let has_videotoolbox = caps
        .as_ref()
        .map(|c| c.available_encoders.iter().any(|e| e.contains("videotoolbox")))
        .unwrap_or(false);
    let has_nvenc = caps
        .as_ref()
        .map(|c| c.available_encoders.iter().any(|e| e.contains("nvenc")))
        .unwrap_or(false);

    let use_videotoolbox = hw_mode == "videotoolbox" || (hw_mode == "auto" && has_videotoolbox);
    let use_nvenc = hw_mode == "nvenc" || (hw_mode == "auto" && !has_videotoolbox && has_nvenc);

    let mut args: Vec<String> = vec!["-y".to_string(), "-i".to_string(), opts.input.to_string_lossy().to_string()];
    let mut commands = Vec::new();

    match opts.preset {
        ExportPreset::Copy => {
            args.extend(vec![
                "-c".to_string(),
                "copy".to_string(),
                "-movflags".to_string(),
                "+faststart".to_string(),
                output.to_string_lossy().to_string(),
            ]);
        }
        ExportPreset::Gif => {
            let fps = opts.fps.unwrap_or(15.0);
            let scale_val = opts.scale.clone().unwrap_or_else(|| "480:-1".to_string());
            let vf = format!("fps={},scale={}:flags=lanczos,split[s0][s1];[s0]palettegen=max_colors=256:reserve_transparent=0[p];[s1][p]paletteuse=dither=bayer:bayer_scale=3", fps, scale_val);
            args.extend(vec![
                "-vf".to_string(),
                vf,
                "-loop".to_string(),
                "0".to_string(),
                output.to_string_lossy().to_string(),
            ]);
        }
        ExportPreset::Prores => {
            let enc = if use_videotoolbox {
                "prores_videotoolbox"
            } else {
                "prores_ks"
            };
            args.extend(vec![
                "-c:v".to_string(),
                enc.to_string(),
                "-profile:v".to_string(),
                "3".to_string(), // ProRes 422 HQ
                "-c:a".to_string(),
                "pcm_s24le".to_string(),
                output.to_string_lossy().to_string(),
            ]);
        }
        ExportPreset::H265 => {
            let enc = if use_videotoolbox {
                "hevc_videotoolbox"
            } else if use_nvenc {
                "hevc_nvenc"
            } else {
                "libx265"
            };

            let crf = opts.crf.unwrap_or(24);
            args.extend(vec![
                "-c:v".to_string(),
                enc.to_string(),
                "-tag:v".to_string(),
                "hvc1".to_string(), // Apple QuickTime compatibility
            ]);

            if enc == "libx265" {
                args.extend(vec![
                    "-crf".to_string(),
                    crf.to_string(),
                    "-preset".to_string(),
                    "medium".to_string(),
                ]);
            } else if enc == "hevc_videotoolbox" {
                args.extend(vec!["-q:v".to_string(), "65".to_string()]);
            }

            args.extend(vec![
                "-c:a".to_string(),
                "aac".to_string(),
                "-b:a".to_string(),
                "192k".to_string(),
                "-movflags".to_string(),
                "+faststart".to_string(),
                output.to_string_lossy().to_string(),
            ]);
        }
        ExportPreset::Youtube | ExportPreset::Youtube4k => {
            let is_4k = opts.preset == ExportPreset::Youtube4k;
            let enc = if use_videotoolbox {
                "h264_videotoolbox"
            } else if use_nvenc {
                "h264_nvenc"
            } else {
                "libx264"
            };

            let crf = opts.crf.unwrap_or(if is_4k { 16 } else { 18 });
            args.extend(vec![
                "-c:v".to_string(),
                enc.to_string(),
                "-pix_fmt".to_string(),
                "yuv420p".to_string(),
            ]);

            if enc == "libx264" {
                args.extend(vec![
                    "-crf".to_string(),
                    crf.to_string(),
                    "-preset".to_string(),
                    "slow".to_string(),
                ]);
            } else if enc == "h264_videotoolbox" {
                args.extend(vec!["-q:v".to_string(), "75".to_string()]);
            }

            args.extend(vec![
                "-c:a".to_string(),
                "aac".to_string(),
                "-b:a".to_string(),
                "384k".to_string(),
                "-ar".to_string(),
                "48000".to_string(),
                "-movflags".to_string(),
                "+faststart".to_string(),
                output.to_string_lossy().to_string(),
            ]);
        }
        ExportPreset::Reels | ExportPreset::Tiktok | ExportPreset::Shorts => {
            let enc = if use_videotoolbox {
                "h264_videotoolbox"
            } else if use_nvenc {
                "h264_nvenc"
            } else {
                "libx264"
            };

            let crf = opts.crf.unwrap_or(20);
            let mut vf_filters = vec!["scale=1080:1920:force_original_aspect_ratio=decrease".to_string(), "pad=1080:1920:(ow-iw)/2:(oh-ih)/2:black".to_string()];
            if let Some(fps_val) = opts.fps {
                vf_filters.push(format!("fps={}", fps_val));
            }

            args.extend(vec![
                "-vf".to_string(),
                vf_filters.join(","),
                "-c:v".to_string(),
                enc.to_string(),
                "-pix_fmt".to_string(),
                "yuv420p".to_string(),
            ]);

            if enc == "libx264" {
                args.extend(vec![
                    "-crf".to_string(),
                    crf.to_string(),
                    "-preset".to_string(),
                    "fast".to_string(),
                ]);
            } else if enc == "h264_videotoolbox" {
                args.extend(vec!["-q:v".to_string(), "68".to_string()]);
            }

            args.extend(vec![
                "-c:a".to_string(),
                "aac".to_string(),
                "-b:a".to_string(),
                "192k".to_string(),
                "-ar".to_string(),
                "44100".to_string(),
                "-movflags".to_string(),
                "+faststart".to_string(),
                output.to_string_lossy().to_string(),
            ]);
        }
        ExportPreset::Linkedin | ExportPreset::Facebook | ExportPreset::X => {
            let enc = if use_videotoolbox {
                "h264_videotoolbox"
            } else if use_nvenc {
                "h264_nvenc"
            } else {
                "libx264"
            };

            let crf = opts.crf.unwrap_or(22);
            args.extend(vec![
                "-c:v".to_string(),
                enc.to_string(),
                "-pix_fmt".to_string(),
                "yuv420p".to_string(),
            ]);

            if enc == "libx264" {
                args.extend(vec![
                    "-crf".to_string(),
                    crf.to_string(),
                    "-preset".to_string(),
                    "fast".to_string(),
                    "-profile:v".to_string(),
                    "main".to_string(),
                ]);
            } else if enc == "h264_videotoolbox" {
                args.extend(vec!["-q:v".to_string(), "65".to_string()]);
            }

            args.extend(vec![
                "-c:a".to_string(),
                "aac".to_string(),
                "-b:a".to_string(),
                "192k".to_string(),
                "-movflags".to_string(),
                "+faststart".to_string(),
                output.to_string_lossy().to_string(),
            ]);
        }
    }

    commands.push(format!("ffmpeg {}", args.join(" ")));

    if !opts.dry_run {
        let runner_res = run_ffmpeg(&args, opts.timeout).await?;
        if !runner_res.success() {
            return Err(FfmpegSkillError::ffmpeg_error(
                format!("Export execution failed: {}", runner_res.stderr),
                Some(runner_res.exit_code()),
                Some(args),
            ));
        }

        let out_probe = probe_file(&output).await.map_err(|e| {
            FfmpegSkillError::verification_error(format!(
                "Failed to probe exported media file: {}",
                e
            ))
        })?;

        let meta = std::fs::metadata(&output).map_err(|e| {
            FfmpegSkillError::output_error(format!("Failed to read output file metadata: {}", e))
        })?;

        Ok(ExportResult {
            status: "success".to_string(),
            input: opts.input.to_string_lossy().to_string(),
            output: output.to_string_lossy().to_string(),
            preset: format!("{:?}", opts.preset).to_lowercase(),
            format: out_probe.format.clone().unwrap_or_else(|| target_ext.to_string()),
            video_codec: out_probe.video_codec().unwrap_or_else(|| "none".to_string()),
            audio_codec: out_probe.audio_codec().unwrap_or_else(|| "none".to_string()),
            resolution: out_probe.resolution(),
            duration: out_probe.duration,
            file_size_bytes: meta.len(),
            commands,
        })
    } else {
        Ok(ExportResult {
            status: "dry_run".to_string(),
            input: opts.input.to_string_lossy().to_string(),
            output: output.to_string_lossy().to_string(),
            preset: format!("{:?}", opts.preset).to_lowercase(),
            format: target_ext.to_string(),
            video_codec: "pending".to_string(),
            audio_codec: "pending".to_string(),
            resolution: probe.resolution(),
            duration: probe.duration,
            file_size_bytes: 0,
            commands,
        })
    }
}
