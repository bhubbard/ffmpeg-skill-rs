// ============================================================================
// loudness.rs — EBU R128 two-pass audio loudness normalization
// Part of ffmpeg-skill-rs
// ============================================================================

use crate::error::FfmpegSkillError;
use crate::probe::probe_file;
use crate::runner::run_ffmpeg;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Loudness target presets.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LoudnessPreset {
    Podcast,   // -16 LUFS, TP -1.0, LRA 11
    Youtube,   // -14 LUFS, TP -1.0, LRA 11
    Broadcast, // -23 LUFS, TP -1.0, LRA 7
    Custom,
}

impl LoudnessPreset {
    pub fn from_str_loose(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "podcast" | "apple" | "spotify" => Self::Podcast,
            "youtube" | "web" => Self::Youtube,
            "broadcast" | "ebu" | "ebu-r128" | "tv" => Self::Broadcast,
            _ => Self::Custom,
        }
    }

    pub fn defaults(&self) -> (f64, f64, f64) {
        match self {
            Self::Podcast => (-16.0, -1.0, 11.0),
            Self::Youtube => (-14.0, -1.0, 11.0),
            Self::Broadcast => (-23.0, -1.0, 7.0),
            Self::Custom => (-16.0, -1.0, 11.0),
        }
    }
}

/// Options configuring loudness normalization.
#[derive(Debug, Clone)]
pub struct LoudnessOptions {
    pub input: PathBuf,
    pub output: Option<PathBuf>,
    pub preset: LoudnessPreset,
    pub target_i: Option<f64>,
    pub target_tp: Option<f64>,
    pub target_lra: Option<f64>,
    pub single_pass: bool,
    pub dry_run: bool,
    pub timeout: Option<f64>,
}

/// Loudnorm analysis stats measured in pass 1.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LoudnormStats {
    pub input_i: String,
    pub input_tp: String,
    pub input_lra: String,
    pub input_thresh: String,
    pub target_offset: String,
}

/// Structured loudness normalization result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoudnessResult {
    pub status: String,
    pub input: String,
    pub output: String,
    pub target_lufs: f64,
    pub target_true_peak: f64,
    pub target_lra: f64,
    pub measured_stats: Option<LoudnormStats>,
    pub commands: Vec<String>,
}

/// Normalizes audio to target LUFS/TP using EBU R128 loudnorm filter.
pub async fn normalize_loudness(opts: &LoudnessOptions) -> Result<LoudnessResult, FfmpegSkillError> {
    if !opts.input.exists() {
        return Err(FfmpegSkillError::input_error(format!(
            "Input file '{}' does not exist",
            opts.input.display()
        )));
    }

    let probe = probe_file(&opts.input).await?;
    if !probe.has_audio() {
        return Err(FfmpegSkillError::input_error(
            "Input file has no audio stream to normalize",
        ));
    }

    let (def_i, def_tp, def_lra) = opts.preset.defaults();
    let target_i = opts.target_i.unwrap_or(def_i);
    let target_tp = opts.target_tp.unwrap_or(def_tp);
    let target_lra = opts.target_lra.unwrap_or(def_lra);

    let output = match &opts.output {
        Some(p) => p.clone(),
        None => {
            let stem = opts
                .input
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("normalized");
            let ext = opts.input.extension().and_then(|e| e.to_str()).unwrap_or("mp4");
            let mut out = opts.input.with_file_name(format!("{}_norm_{}lufs.{}", stem, target_i.abs() as i64, ext));
            let mut counter = 1;
            while out.exists() {
                out = opts.input.with_file_name(format!("{}_norm_{}lufs_{}.{}", stem, target_i.abs() as i64, counter, ext));
                counter += 1;
            }
            out
        }
    };

    let mut commands = Vec::new();
    let has_video = probe.has_video();

    if opts.single_pass {
        let af = format!("loudnorm=I={:.1}:TP={:.1}:LRA={:.1}", target_i, target_tp, target_lra);
        let mut args = vec!["-y".to_string(), "-i".to_string(), opts.input.to_string_lossy().to_string()];
        if has_video {
            args.extend(vec!["-c:v".to_string(), "copy".to_string()]);
        }
        args.extend(vec![
            "-af".to_string(),
            af,
            "-c:a".to_string(),
            "aac".to_string(),
            "-b:a".to_string(),
            "192k".to_string(),
            output.to_string_lossy().to_string(),
        ]);

        commands.push(format!("ffmpeg {}", args.join(" ")));

        if !opts.dry_run {
            let runner_res = run_ffmpeg(&args, opts.timeout).await?;
            if !runner_res.success() {
                return Err(FfmpegSkillError::ffmpeg_error(
                    format!("Single-pass loudness normalization failed: {}", runner_res.stderr),
                    Some(runner_res.exit_code()),
                    Some(args),
                ));
            }
        }

        return Ok(LoudnessResult {
            status: if opts.dry_run { "dry_run".to_string() } else { "success".to_string() },
            input: opts.input.to_string_lossy().to_string(),
            output: output.to_string_lossy().to_string(),
            target_lufs: target_i,
            target_true_peak: target_tp,
            target_lra,
            measured_stats: None,
            commands,
        });
    }

    // Two-pass Loudness Normalization
    // Pass 1: Analysis
    let pass1_af = format!(
        "loudnorm=I={:.1}:TP={:.1}:LRA={:.1}:print_format=json",
        target_i, target_tp, target_lra
    );
    let pass1_args = vec![
        "-hide_banner".to_string(),
        "-i".to_string(),
        opts.input.to_string_lossy().to_string(),
        "-af".to_string(),
        pass1_af,
        "-f".to_string(),
        "null".to_string(),
        "-".to_string(),
    ];

    commands.push(format!("ffmpeg {}", pass1_args.join(" ")));

    let measured = if !opts.dry_run {
        let p1_res = run_ffmpeg(&pass1_args, opts.timeout).await?;
        let json_re = Regex::new(r"\{[^{}]*input_i[^{}]*\}").unwrap();
        if let Some(mat) = json_re.find(&p1_res.stderr) {
            serde_json::from_str::<LoudnormStats>(mat.as_str()).unwrap_or_default()
        } else {
            LoudnormStats {
                input_i: "-20.0".to_string(),
                input_tp: "-1.0".to_string(),
                input_lra: "10.0".to_string(),
                input_thresh: "-30.0".to_string(),
                target_offset: "0.0".to_string(),
            }
        }
    } else {
        LoudnormStats {
            input_i: "-20.0".to_string(),
            input_tp: "-1.0".to_string(),
            input_lra: "10.0".to_string(),
            input_thresh: "-30.0".to_string(),
            target_offset: "0.0".to_string(),
        }
    };

    // Pass 2: Exact correction filter
    let pass2_af = format!(
        "loudnorm=I={:.1}:TP={:.1}:LRA={:.1}:measured_I={}:measured_TP={}:measured_LRA={}:measured_thresh={}:offset={}:linear=true",
        target_i, target_tp, target_lra, measured.input_i, measured.input_tp, measured.input_lra, measured.input_thresh, measured.target_offset
    );

    let mut pass2_args = vec![
        "-y".to_string(),
        "-i".to_string(),
        opts.input.to_string_lossy().to_string(),
    ];

    if has_video {
        pass2_args.extend(vec!["-c:v".to_string(), "copy".to_string()]);
    }

    pass2_args.extend(vec![
        "-af".to_string(),
        pass2_af,
        "-c:a".to_string(),
        "aac".to_string(),
        "-b:a".to_string(),
        "256k".to_string(),
        "-movflags".to_string(),
        "+faststart".to_string(),
        output.to_string_lossy().to_string(),
    ]);

    commands.push(format!("ffmpeg {}", pass2_args.join(" ")));

    if !opts.dry_run {
        let p2_res = run_ffmpeg(&pass2_args, opts.timeout).await?;
        if !p2_res.success() {
            return Err(FfmpegSkillError::ffmpeg_error(
                format!("Two-pass loudness execution failed: {}", p2_res.stderr),
                Some(p2_res.exit_code()),
                Some(pass2_args),
            ));
        }
    }

    Ok(LoudnessResult {
        status: if opts.dry_run { "dry_run".to_string() } else { "success".to_string() },
        input: opts.input.to_string_lossy().to_string(),
        output: output.to_string_lossy().to_string(),
        target_lufs: target_i,
        target_true_peak: target_tp,
        target_lra,
        measured_stats: Some(measured),
        commands,
    })
}
