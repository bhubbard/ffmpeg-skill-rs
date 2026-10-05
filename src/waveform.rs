// ============================================================================
// waveform.rs — Audio waveform and spectrogram image/video generator
// Part of ffmpeg-skill-rs
// ============================================================================

use crate::error::FfmpegSkillError;
use crate::probe::probe_file;
use crate::runner::run_ffmpeg;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Waveform visualization modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WaveformMode {
    Waveform,
    Spectrogram,
    Video,
}

impl WaveformMode {
    pub fn from_str_loose(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "spectrogram" | "spectrum" => Self::Spectrogram,
            "video" | "animated" => Self::Video,
            _ => Self::Waveform,
        }
    }
}

/// Options configuring waveform generation.
#[derive(Debug, Clone)]
pub struct WaveformOptions {
    pub input: PathBuf,
    pub output: Option<PathBuf>,
    pub mode: WaveformMode,
    pub width: u32,
    pub height: u32,
    pub color: String,
    pub bg_color: String,
    pub dry_run: bool,
    pub timeout: Option<f64>,
}

/// Structured waveform generation result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaveformResult {
    pub status: String,
    pub input: String,
    pub output: String,
    pub mode: String,
    pub width: u32,
    pub height: u32,
    pub commands: Vec<String>,
}

/// Generates an audio waveform or spectrogram.
pub async fn generate_waveform(opts: &WaveformOptions) -> Result<WaveformResult, FfmpegSkillError> {
    if !opts.input.exists() {
        return Err(FfmpegSkillError::input_error(format!(
            "Input file '{}' does not exist",
            opts.input.display()
        )));
    }

    let probe = probe_file(&opts.input).await?;
    if !probe.has_audio() {
        return Err(FfmpegSkillError::input_error(
            "Input file has no audio stream to visualize",
        ));
    }

    let ext = match opts.mode {
        WaveformMode::Video => "mp4",
        _ => "png",
    };

    let output = match &opts.output {
        Some(p) => p.clone(),
        None => {
            let stem = opts
                .input
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("waveform");
            let mode_str = format!("{:?}", opts.mode).to_lowercase();
            let mut out = opts.input.with_file_name(format!("{}_{}.{}", stem, mode_str, ext));
            let mut counter = 1;
            while out.exists() {
                out = opts.input.with_file_name(format!("{}_{}_{}.{}", stem, mode_str, counter, ext));
                counter += 1;
            }
            out
        }
    };

    let mut args = vec!["-y".to_string(), "-i".to_string(), opts.input.to_string_lossy().to_string()];

    match opts.mode {
        WaveformMode::Waveform => {
            let filter = format!(
                "showwavespic=s={}x{}:colors={}",
                opts.width, opts.height, opts.color
            );
            args.extend(vec![
                "-filter_complex".to_string(),
                filter,
                "-frames:v".to_string(),
                "1".to_string(),
                output.to_string_lossy().to_string(),
            ]);
        }
        WaveformMode::Spectrogram => {
            let filter = format!(
                "showspectrumpic=s={}x{}:color=rainbow:legend=1",
                opts.width, opts.height
            );
            args.extend(vec![
                "-filter_complex".to_string(),
                filter,
                "-frames:v".to_string(),
                "1".to_string(),
                output.to_string_lossy().to_string(),
            ]);
        }
        WaveformMode::Video => {
            let filter = format!(
                "showwaves=s={}x{}:mode=line:rate=30:colors={}",
                opts.width, opts.height, opts.color
            );
            args.extend(vec![
                "-filter_complex".to_string(),
                filter,
                "-c:v".to_string(),
                "libx264".to_string(),
                "-pix_fmt".to_string(),
                "yuv420p".to_string(),
                "-c:a".to_string(),
                "copy".to_string(),
                output.to_string_lossy().to_string(),
            ]);
        }
    }

    let commands = vec![format!("ffmpeg {}", args.join(" "))];

    if !opts.dry_run {
        let runner_res = run_ffmpeg(&args, opts.timeout).await?;
        if !runner_res.success() {
            return Err(FfmpegSkillError::ffmpeg_error(
                format!("Waveform generation failed: {}", runner_res.stderr),
                Some(runner_res.exit_code()),
                Some(args),
            ));
        }

        Ok(WaveformResult {
            status: "success".to_string(),
            input: opts.input.to_string_lossy().to_string(),
            output: output.to_string_lossy().to_string(),
            mode: format!("{:?}", opts.mode).to_lowercase(),
            width: opts.width,
            height: opts.height,
            commands,
        })
    } else {
        Ok(WaveformResult {
            status: "dry_run".to_string(),
            input: opts.input.to_string_lossy().to_string(),
            output: output.to_string_lossy().to_string(),
            mode: format!("{:?}", opts.mode).to_lowercase(),
            width: opts.width,
            height: opts.height,
            commands,
        })
    }
}
