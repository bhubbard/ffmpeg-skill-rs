// ============================================================================
// check.rs — Pre-delivery media specification and compliance validator
// Part of ffmpeg-skill-rs
// ============================================================================

use crate::error::FfmpegSkillError;
use crate::probe::probe_file;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Target platform specification profiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TargetPlatform {
    Youtube,
    Tiktok,
    Reels,
    X,
    Broadcast,
    General,
}

impl TargetPlatform {
    pub fn from_str_loose(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "youtube" | "yt" => Self::Youtube,
            "tiktok" | "tt" => Self::Tiktok,
            "reels" | "instagram" | "ig" => Self::Reels,
            "x" | "twitter" => Self::X,
            "broadcast" | "tv" | "ebu" => Self::Broadcast,
            _ => Self::General,
        }
    }
}

/// Options configuring pre-flight compliance check.
#[derive(Debug, Clone)]
pub struct CheckOptions {
    pub input: PathBuf,
    pub platform: TargetPlatform,
}

/// Structured compliance check result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    pub status: String,
    pub input: String,
    pub platform: String,
    pub passed: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub file_size_mb: f64,
    pub duration_seconds: Option<f64>,
    pub resolution: Option<(u32, u32)>,
    pub aspect_ratio: Option<String>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub is_vfr: bool,
}

/// Validates media against target platform delivery specifications.
pub async fn check_compliance(opts: &CheckOptions) -> Result<CheckResult, FfmpegSkillError> {
    if !opts.input.exists() {
        return Err(FfmpegSkillError::input_error(format!(
            "Input file '{}' does not exist",
            opts.input.display()
        )));
    }

    let probe = probe_file(&opts.input).await?;
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    let meta = std::fs::metadata(&opts.input).map_err(|e| {
        FfmpegSkillError::input_error(format!("Failed to read input metadata: {}", e))
    })?;
    let file_size_mb = meta.len() as f64 / (1024.0 * 1024.0);

    let v_stream = probe.video.as_ref();
    let a_stream = probe.audio.as_ref();

    let video_codec = v_stream.and_then(|v| v.codec.as_deref().map(|s| s.to_lowercase()));
    let audio_codec = a_stream.and_then(|a| a.codec.as_deref().map(|s| s.to_lowercase()));
    let duration = Some(probe.duration);
    let is_vfr = v_stream.map(|v| v.variable_frame_rate_suspected).unwrap_or(false);

    if is_vfr {
        warnings.push("Variable Frame Rate (VFR) detected; constant frame rate (CFR) is strongly recommended for editing and streaming stability.".to_string());
    }

    match opts.platform {
        TargetPlatform::Youtube => {
            if let Some(dur) = duration {
                if dur > 12.0 * 3600.0 {
                    errors.push(format!("Duration {:.1}s exceeds YouTube maximum limit of 12 hours", dur));
                }
            }
            if let Some(ref vc) = video_codec {
                if !["h264", "hevc", "vp9", "av1", "prores"].contains(&vc.as_str()) {
                    warnings.push(format!("Video codec '{}' is not optimal for YouTube (h264/hevc/av1 recommended)", vc));
                }
            }
        }
        TargetPlatform::Tiktok => {
            if let Some((w, h)) = probe.resolution() {
                if h < w {
                    warnings.push(format!("Landscape resolution {}x{} detected; TikTok strongly favors 9:16 vertical (1080x1920)", w, h));
                }
            }
            if let Some(dur) = duration {
                if dur < 3.0 {
                    errors.push(format!("Duration {:.1}s is shorter than TikTok minimum of 3 seconds", dur));
                } else if dur > 600.0 {
                    errors.push(format!("Duration {:.1}s exceeds TikTok 10-minute maximum limit", dur));
                }
            }
            if file_size_mb > 500.0 {
                warnings.push(format!("File size {:.1}MB is large for mobile upload; consider under 287MB", file_size_mb));
            }
        }
        TargetPlatform::Reels => {
            if let Some((w, h)) = probe.resolution() {
                if h < w {
                    warnings.push(format!("Resolution {}x{} is not 9:16 vertical orientation standard for Reels", w, h));
                }
            }
            if let Some(dur) = duration {
                if dur > 90.0 {
                    warnings.push(format!("Duration {:.1}s exceeds Instagram Reels 90s standalone limit (will be posted as general video)", dur));
                }
            }
            if file_size_mb > 4000.0 {
                errors.push(format!("File size {:.1}MB exceeds Instagram 4GB upload limit", file_size_mb));
            }
        }
        TargetPlatform::X => {
            if let Some(dur) = duration {
                if dur > 140.0 {
                    errors.push(format!("Duration {:.1}s exceeds standard X/Twitter 140-second limit", dur));
                }
            }
            if file_size_mb > 512.0 {
                errors.push(format!("File size {:.1}MB exceeds X/Twitter 512MB limit", file_size_mb));
            }
            if let Some((w, h)) = probe.resolution() {
                if w > 1920 || h > 1920 {
                    warnings.push(format!("Resolution {}x{} exceeds standard 1080p maximum recommendation for X", w, h));
                }
            }
        }
        TargetPlatform::Broadcast => {
            if is_vfr {
                errors.push("Variable Frame Rate (VFR) is strictly prohibited in broadcast delivery specifications.".to_string());
            }
            if let Some(ref vc) = video_codec {
                if !["prores", "dnxhd", "dnxhr", "xdcam", "mpeg2video", "h264"].contains(&vc.as_str()) {
                    warnings.push(format!("Video codec '{}' is unusual for broadcast master (ProRes/DNxHD/XDCAM standard)", vc));
                }
            }
        }
        TargetPlatform::General => {
            if v_stream.is_none() && a_stream.is_none() {
                errors.push("File contains neither valid video nor audio streams".to_string());
            }
        }
    }

    let passed = errors.is_empty();

    Ok(CheckResult {
        status: if passed { "passed".to_string() } else { "failed".to_string() },
        input: opts.input.to_string_lossy().to_string(),
        platform: format!("{:?}", opts.platform).to_lowercase(),
        passed,
        errors,
        warnings,
        file_size_mb,
        duration_seconds: duration,
        resolution: probe.resolution(),
        aspect_ratio: probe.aspect_ratio(),
        video_codec,
        audio_codec,
        is_vfr,
    })
}
