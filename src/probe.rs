// ============================================================================
// probe.rs — Deep media analysis and stream inspection using ffprobe
// Part of ffmpeg-skill-rs
// ============================================================================

use crate::error::FfmpegSkillError;
use crate::runner::run_ffprobe;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Detailed video stream parameters extracted from ffprobe.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VideoStreamInfo {
    pub codec: Option<String>,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub pix_fmt: Option<String>,
    pub hdr: bool,
    pub bt2020_or_hdr: bool,
    pub color_transfer: Option<String>,
    pub color_primaries: Option<String>,
    pub rotation: i32,
    pub variable_frame_rate_suspected: bool,
}

/// Detailed audio stream parameters extracted from ffprobe.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioStreamInfo {
    pub codec: Option<String>,
    pub channels: u32,
    pub sample_rate: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bitrate: Option<u64>,
}

/// Top-level media inspection document adhering to the ffmpeg-skill contract.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProbeResult {
    pub file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    pub duration: f64,
    pub size_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bitrate: Option<u64>,
    pub video: Option<VideoStreamInfo>,
    pub audio: Option<AudioStreamInfo>,
    pub subtitle_streams: usize,
    pub data_streams: usize,
}

impl ProbeResult {
    /// Formats a single human-readable line summary matching `--compact`.
    pub fn to_compact_string(&self) -> String {
        let mut line = format!("{}: {:.3}s", self.file, self.duration);
        if let Some(ref v) = self.video {
            line.push_str(&format!(
                " | {}x{} @ {:.1}fps {} {}",
                v.width,
                v.height,
                v.fps,
                v.codec.as_deref().unwrap_or("unknown"),
                v.pix_fmt.as_deref().unwrap_or("unknown")
            ));
            if v.variable_frame_rate_suspected {
                line.push_str(" (VFR?)");
            }
            if v.hdr {
                line.push_str(" [HDR]");
            } else if v.bt2020_or_hdr {
                line.push_str(" [BT.2020]");
            }
        } else {
            line.push_str(" | no video");
        }

        if let Some(ref a) = self.audio {
            line.push_str(&format!(
                " | audio {} {}ch {}Hz",
                a.codec.as_deref().unwrap_or("unknown"),
                a.channels,
                a.sample_rate
            ));
        } else {
            line.push_str(" | no audio");
        }

        line
    }

    pub fn resolution(&self) -> Option<(u32, u32)> {
        self.video.as_ref().map(|v| (v.width, v.height))
    }

    pub fn duration_seconds(&self) -> Option<f64> {
        Some(self.duration)
    }

    pub fn aspect_ratio(&self) -> Option<String> {
        self.video.as_ref().map(|v| {
            let gcd_val = gcd_probe(v.width, v.height);
            format!("{}:{}", v.width / gcd_val, v.height / gcd_val)
        })
    }

    pub fn has_video(&self) -> bool {
        self.video.is_some()
    }

    pub fn has_audio(&self) -> bool {
        self.audio.is_some()
    }

    pub fn video_codec(&self) -> Option<String> {
        self.video.as_ref().and_then(|v| v.codec.clone())
    }

    pub fn audio_codec(&self) -> Option<String> {
        self.audio.as_ref().and_then(|a| a.codec.clone())
    }

    /// Evaluates a dotted field query (e.g. "duration" or "video.fps") returning a JSON string.
    pub fn query_field(&self, field_path: &str) -> Option<String> {
        let val = serde_json::to_value(self).ok()?;
        let parts: Vec<&str> = field_path.split('.').collect();
        let mut curr = &val;
        for part in parts {
            curr = curr.get(part)?;
        }
        if let Some(s) = curr.as_str() {
            Some(s.to_string())
        } else {
            Some(curr.to_string())
        }
    }
}

fn gcd_probe(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    if a == 0 { 1 } else { a }
}

/// Dotted field query selector for arbitrary JSON values.
#[derive(Debug, Clone)]
pub struct FieldSelector {
    path: String,
}

impl FieldSelector {
    pub fn new(path: impl Into<String>) -> Self {
        Self { path: path.into() }
    }

    pub fn extract(&self, val: &serde_json::Value) -> serde_json::Value {
        let parts: Vec<&str> = self.path.split('.').collect();
        let mut curr = val;
        for part in parts {
            if let Some((field, idx_str)) = part.split_once('[') {
                if let Some(idx_str) = idx_str.strip_suffix(']') {
                    if let Ok(idx) = idx_str.parse::<usize>() {
                        if let Some(field_val) = curr.get(field) {
                            if let Some(item) = field_val.get(idx) {
                                curr = item;
                                continue;
                            }
                        }
                    }
                }
            }
            if let Some(child) = curr.get(part) {
                curr = child;
            } else {
                return serde_json::Value::Null;
            }
        }
        curr.clone()
    }
}

/// Helper function to parse fractional rates like "30000/1001" or "25/1".
fn parse_rate(rate_str: &str) -> Option<f64> {
    if let Some((num, den)) = rate_str.split_once('/') {
        let n: f64 = num.trim().parse().ok()?;
        let d: f64 = den.trim().parse().ok()?;
        if d > 0.0 {
            return Some(n / d);
        }
    } else if let Ok(val) = rate_str.trim().parse::<f64>() {
        return Some(val);
    }
    None
}

/// Probes a media file using ffprobe.
pub async fn probe_file(path: impl AsRef<Path>) -> Result<ProbeResult, FfmpegSkillError> {
    let p = path.as_ref();
    if !p.exists() {
        return Err(FfmpegSkillError::Input {
            message: format!("input file not found: {}", p.display()),
            hint: Some("Check the input file path.".to_string()),
        });
    }

    let abs_path = p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    let file_str = abs_path.to_string_lossy().to_string();
    let size_bytes = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);

    let args = vec![
        "-print_format".to_string(),
        "json".to_string(),
        "-show_format".to_string(),
        "-show_streams".to_string(),
        "-show_chapters".to_string(),
        file_str.clone(),
    ];

    let out = run_ffprobe(&args, None).await?;
    if !out.success() {
        return Err(FfmpegSkillError::Ffmpeg {
            exit_code: out.status,
            message: format!("ffprobe failed on {}:\n{}", file_str, out.stderr.trim()),
            hint: Some("Verify that the input is a valid supported media container.".to_string()),
        });
    }

    let parsed: serde_json::Value = serde_json::from_str(&out.stdout).map_err(|e| {
        FfmpegSkillError::Output {
            message: format!("ffprobe returned invalid JSON: {}", e),
            hint: None,
        }
    })?;

    let format_obj = parsed.get("format");
    let streams = parsed
        .get("streams")
        .and_then(|s| s.as_array())
        .cloned()
        .unwrap_or_default();

    let mut video_info = None;
    let mut audio_info = None;
    let mut subtitle_streams = 0;
    let mut data_streams = 0;

    for stream in &streams {
        let codec_type = stream
            .get("codec_type")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        match codec_type {
            "video" if video_info.is_none() => {
                let width = stream.get("width").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                let height = stream.get("height").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                let codec = stream
                    .get("codec_name")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let pix_fmt = stream
                    .get("pix_fmt")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let r_rate = stream
                    .get("r_frame_rate")
                    .and_then(|v| v.as_str())
                    .and_then(parse_rate)
                    .unwrap_or(0.0);
                let avg_rate = stream
                    .get("avg_frame_rate")
                    .and_then(|v| v.as_str())
                    .and_then(parse_rate)
                    .unwrap_or(r_rate);

                let fps = if avg_rate > 0.0 { avg_rate } else { r_rate };
                let vfr = r_rate > 0.0 && avg_rate > 0.0 && (r_rate - avg_rate).abs() > 0.05;

                let color_transfer = stream
                    .get("color_transfer")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let color_primaries = stream
                    .get("color_primaries")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let is_hdr = color_transfer.as_deref().map_or(false, |t| {
                    t.contains("smpte2084") || t.contains("arib-std-b67") || t.contains("linear")
                });
                let is_bt2020 = color_primaries.as_deref().map_or(false, |p| {
                    p.contains("bt2020")
                }) || is_hdr;

                // Rotation tags / side_data_list
                let mut rotation = 0;
                if let Some(tags) = stream.get("tags") {
                    if let Some(rot_str) = tags.get("rotate").and_then(|v| v.as_str()) {
                        rotation = rot_str.parse().unwrap_or(0);
                    }
                }
                if rotation == 0 {
                    if let Some(side_data) = stream.get("side_data_list").and_then(|v| v.as_array()) {
                        for sd in side_data {
                            if let Some(rot) = sd.get("rotation").and_then(|v| v.as_i64()) {
                                rotation = rot as i32;
                                break;
                            }
                        }
                    }
                }

                video_info = Some(VideoStreamInfo {
                    codec,
                    width,
                    height,
                    fps,
                    pix_fmt,
                    hdr: is_hdr,
                    bt2020_or_hdr: is_bt2020,
                    color_transfer,
                    color_primaries,
                    rotation,
                    variable_frame_rate_suspected: vfr,
                });
            }
            "audio" if audio_info.is_none() => {
                let codec = stream
                    .get("codec_name")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let channels = stream
                    .get("channels")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32;
                let sample_rate = stream
                    .get("sample_rate")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
                let bitrate = stream
                    .get("bit_rate")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse().ok());

                audio_info = Some(AudioStreamInfo {
                    codec,
                    channels,
                    sample_rate,
                    bitrate,
                });
            }
            "subtitle" => subtitle_streams += 1,
            "data" => data_streams += 1,
            _ => {}
        }
    }

    let duration: f64 = format_obj
        .and_then(|f| f.get("duration"))
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.0);

    let format_name = format_obj
        .and_then(|f| f.get("format_name"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let bitrate = format_obj
        .and_then(|f| f.get("bit_rate"))
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok());

    Ok(ProbeResult {
        file: file_str,
        format: format_name,
        duration,
        size_bytes,
        bitrate,
        video: video_info,
        audio: audio_info,
        subtitle_streams,
        data_streams,
    })
}
