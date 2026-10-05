// ============================================================================
// doctor.rs — System capability detection, hardware encoder audit, and healthcheck
// Part of ffmpeg-skill-rs
// ============================================================================

use crate::error::FfmpegSkillError;
use crate::runner::{resolve_binary, run_command};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// GPU hardware acceleration backend kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HardwareBackend {
    VideoToolbox, // Apple Silicon / macOS
    Nvenc,        // NVIDIA
    Vaapi,        // Linux
    Qsv,          // Intel QuickSync
    None,
}

/// Comprehensive system inspection report matching the ffmpeg-skill contract.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorReport {
    pub ok: bool,
    pub version: String,
    pub ffmpeg: Option<String>,
    pub ffmpeg_version: Option<String>,
    pub ffprobe: Option<String>,
    pub hardware_backend: HardwareBackend,
    pub gpu_encoders: Vec<String>,
    pub available_encoders: Vec<String>,
    pub available_filters: Vec<String>,
    pub missing_required: Vec<String>,
    pub missing_optional: Vec<String>,
    pub errors: Vec<String>,
}

impl DoctorReport {
    pub fn print_human_readable(&self) {
        println!(
            "{} v{}",
            "ffmpeg-skill-rs".bold().cyan(),
            env!("CARGO_PKG_VERSION")
        );
        println!(
            "ffmpeg:  {} ({})",
            self.ffmpeg.as_deref().unwrap_or("MISSING".red().to_string().as_str()),
            self.ffmpeg_version.as_deref().unwrap_or("unknown")
        );
        println!(
            "ffprobe: {}",
            self.ffprobe.as_deref().unwrap_or("MISSING".red().to_string().as_str())
        );

        let status_str = if self.ok {
            "HEALTHY (all required tools and filters operational)".green().bold()
        } else {
            "DEGRADED (missing essential components)".red().bold()
        };
        println!("Status:  {}", status_str);

        println!(
            "Hardware Acceleration: {:?} (available GPU encoders: {})",
            self.hardware_backend,
            if self.gpu_encoders.is_empty() {
                "none (CPU only)".to_string()
            } else {
                self.gpu_encoders.join(", ")
            }
        );

        if !self.missing_required.is_empty() {
            println!(
                "{} {}",
                "Missing Required Capabilities:".red().bold(),
                self.missing_required.join(", ")
            );
        }
        if !self.missing_optional.is_empty() {
            println!(
                "{} {}",
                "Missing Optional Capabilities:".yellow().bold(),
                self.missing_optional.join(", ")
            );
        }

        println!(
            "Core Filters: {} detected",
            self.available_filters.len()
        );
    }
}

/// Inspect capabilities and return DoctorReport.
pub async fn inspect_capabilities() -> Result<DoctorReport, FfmpegSkillError> {
    Ok(run_doctor().await)
}

/// Runs doctor capability audit on the host machine.
pub async fn run_doctor() -> DoctorReport {
    let mut missing_required = Vec::new();
    let mut missing_optional = Vec::new();
    let mut errors = Vec::new();

    let ffmpeg_path = resolve_binary("ffmpeg").ok();
    let ffprobe_path = resolve_binary("ffprobe").ok();

    if ffmpeg_path.is_none() {
        missing_required.push("ffmpeg".to_string());
        errors.push("ffmpeg binary not found in PATH".to_string());
    }
    if ffprobe_path.is_none() {
        missing_required.push("ffprobe".to_string());
        errors.push("ffprobe binary not found in PATH".to_string());
    }

    let mut ffmpeg_version = None;
    let mut encoders = HashSet::new();
    let mut filters = HashSet::new();

    if let Some(ref ffmpeg) = ffmpeg_path {
        // Probe version
        if let Ok(out) = run_command(ffmpeg, &["-version".to_string()], Some(5.0), false).await {
            if let Some(first_line) = out.stdout.lines().next() {
                ffmpeg_version = Some(first_line.to_string());
            }
        }

        // Probe encoders
        if let Ok(out) = run_command(ffmpeg, &["-encoders".to_string()], Some(5.0), false).await {
            for line in out.stdout.lines() {
                let trimmed = line.trim();
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() >= 2 && parts[0].len() >= 6 {
                    encoders.insert(parts[1].to_string());
                }
            }
        }

        // Probe filters
        if let Ok(out) = run_command(ffmpeg, &["-filters".to_string()], Some(5.0), false).await {
            for line in out.stdout.lines() {
                let trimmed = line.trim();
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() >= 2 && (parts[0].len() == 2 || parts[0].len() == 3) && !parts[0].starts_with('-') && !parts[0].starts_with('=') {
                    filters.insert(parts[1].to_string());
                }
            }
        }
    }

    // Identify GPU backend
    let mut gpu_encoders = Vec::new();
    let mut hardware_backend = HardwareBackend::None;

    #[cfg(target_os = "macos")]
    {
        for vt_enc in &["h264_videotoolbox", "hevc_videotoolbox", "prores_videotoolbox"] {
            if encoders.contains(*vt_enc) {
                gpu_encoders.push(vt_enc.to_string());
                hardware_backend = HardwareBackend::VideoToolbox;
            }
        }
    }

    for nv_enc in &["h264_nvenc", "hevc_nvenc", "av1_nvenc"] {
        if encoders.contains(*nv_enc) {
            gpu_encoders.push(nv_enc.to_string());
            if hardware_backend == HardwareBackend::None {
                hardware_backend = HardwareBackend::Nvenc;
            }
        }
    }

    for va_enc in &["h264_vaapi", "hevc_vaapi"] {
        if encoders.contains(*va_enc) {
            gpu_encoders.push(va_enc.to_string());
            if hardware_backend == HardwareBackend::None {
                hardware_backend = HardwareBackend::Vaapi;
            }
        }
    }

    for qsv_enc in &["h264_qsv", "hevc_qsv"] {
        if encoders.contains(*qsv_enc) {
            gpu_encoders.push(qsv_enc.to_string());
            if hardware_backend == HardwareBackend::None {
                hardware_backend = HardwareBackend::Qsv;
            }
        }
    }

    // Required filters check
    let required_filters = [
        "scale", "pad", "crop", "cropdetect", "silencedetect", "loudnorm",
        "scdet", "showwaves", "reverse", "areverse", "atempo", "setpts",
    ];
    for f in &required_filters {
        if !filters.contains(*f) {
            missing_required.push(format!("filter:{}", f));
        }
    }

    // Optional encoders check
    let optional_encoders = ["libx264", "libx265", "libvpx-vp9", "libsvtav1", "libmp3lame", "libopus"];
    for enc in &optional_encoders {
        if !encoders.contains(*enc) {
            missing_optional.push(format!("encoder:{}", enc));
        }
    }

    let ok = missing_required.is_empty() && ffmpeg_path.is_some() && ffprobe_path.is_some();

    let mut sorted_encoders: Vec<String> = encoders.into_iter().collect();
    sorted_encoders.sort();
    let mut sorted_filters: Vec<String> = filters.into_iter().collect();
    sorted_filters.sort();

    DoctorReport {
        ok,
        version: env!("CARGO_PKG_VERSION").to_string(),
        ffmpeg: ffmpeg_path.map(|p| p.to_string_lossy().to_string()),
        ffmpeg_version,
        ffprobe: ffprobe_path.map(|p| p.to_string_lossy().to_string()),
        hardware_backend,
        gpu_encoders,
        available_encoders: sorted_encoders,
        available_filters: sorted_filters,
        missing_required,
        missing_optional,
        errors,
    }
}
