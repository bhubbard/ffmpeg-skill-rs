// ============================================================================
// silence.rs — Silence detection and automated jump-cut dead air removal
// Part of ffmpeg-skill-rs
// ============================================================================

use crate::error::FfmpegSkillError;
use crate::probe::probe_file;
use crate::runner::run_ffmpeg;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A detected silence segment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SilenceInterval {
    pub start: f64,
    pub end: f64,
    pub duration: f64,
}

/// Kept active speech segment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveSegment {
    pub start: f64,
    pub end: f64,
    pub duration: f64,
}

/// Options configuring silence detection and removal.
#[derive(Debug, Clone)]
pub struct SilenceOptions {
    pub input: PathBuf,
    pub output: Option<PathBuf>,
    pub noise_db: f64,
    pub min_duration: f64,
    pub remove: bool,
    pub padding: f64,
    pub dry_run: bool,
    pub timeout: Option<f64>,
}

/// Structured silence detection and jump-cut result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SilenceResult {
    pub status: String,
    pub input: String,
    pub output: Option<String>,
    pub original_duration: f64,
    pub total_silence_duration: f64,
    pub silence_count: usize,
    pub silences: Vec<SilenceInterval>,
    pub kept_segments: Vec<ActiveSegment>,
    pub commands: Vec<String>,
}

/// Detects silence and optionally removes it to generate tight jump cuts.
pub async fn process_silence(opts: &SilenceOptions) -> Result<SilenceResult, FfmpegSkillError> {
    if !opts.input.exists() {
        return Err(FfmpegSkillError::input_error(format!(
            "Input file '{}' does not exist",
            opts.input.display()
        )));
    }

    let probe = probe_file(&opts.input).await?;
    let total_duration = probe.duration;

    let af = format!("silencedetect=noise={}dB:d={}", opts.noise_db, opts.min_duration);
    let detect_args = vec![
        "-hide_banner".to_string(),
        "-i".to_string(),
        opts.input.to_string_lossy().to_string(),
        "-af".to_string(),
        af,
        "-f".to_string(),
        "null".to_string(),
        "-".to_string(),
    ];

    let mut commands = vec![format!("ffmpeg {}", detect_args.join(" "))];
    let runner_res = run_ffmpeg(&detect_args, opts.timeout).await?;
    let stderr = runner_res.stderr;

    let start_re = Regex::new(r"silence_start:\s*([0-9.]+)").unwrap();
    let end_re = Regex::new(r"silence_end:\s*([0-9.]+)\s*\|\s*silence_duration:\s*([0-9.]+)").unwrap();

    let mut silences = Vec::new();
    let mut current_start: Option<f64> = None;

    for line in stderr.lines() {
        if let Some(caps) = start_re.captures(line) {
            if let Ok(s) = caps[1].parse::<f64>() {
                current_start = Some(s);
            }
        } else if let Some(caps) = end_re.captures(line) {
            let end: f64 = caps[1].parse().unwrap_or(0.0);
            let dur: f64 = caps[2].parse().unwrap_or(0.0);
            let start = current_start.take().unwrap_or_else(|| (end - dur).max(0.0));
            silences.push(SilenceInterval {
                start,
                end,
                duration: dur,
            });
        }
    }

    // In case clip ends in silence
    if let Some(s) = current_start {
        if s < total_duration {
            silences.push(SilenceInterval {
                start: s,
                end: total_duration,
                duration: total_duration - s,
            });
        }
    }

    let total_silence: f64 = silences.iter().map(|s| s.duration).sum();

    // Compute active non-silent segments with optional padding
    let pad = opts.padding.max(0.0);
    let mut kept = Vec::new();
    let mut cur_pos = 0.0;

    for sil in &silences {
        let seg_end = (sil.start + pad).min(total_duration);
        if seg_end > cur_pos + 0.05 {
            kept.push(ActiveSegment {
                start: cur_pos,
                end: seg_end,
                duration: seg_end - cur_pos,
            });
        }
        cur_pos = (sil.end - pad).max(seg_end);
    }

    if cur_pos < total_duration - 0.05 {
        kept.push(ActiveSegment {
            start: cur_pos,
            end: total_duration,
            duration: total_duration - cur_pos,
        });
    }

    let mut output_path = None;

    if opts.remove && !kept.is_empty() {
        let resolved_out = match &opts.output {
            Some(p) => p.clone(),
            None => {
                let stem = opts.input.file_stem().and_then(|s| s.to_str()).unwrap_or("cut");
                let ext = opts.input.extension().and_then(|e| e.to_str()).unwrap_or("mp4");
                let mut out = opts.input.with_file_name(format!("{}_jumpcut.{}", stem, ext));
                let mut counter = 1;
                while out.exists() {
                    out = opts.input.with_file_name(format!("{}_jumpcut_{}.{}", stem, counter, ext));
                    counter += 1;
                }
                out
            }
        };

        // Build select condition
        // between(t, s0, e0)+between(t, s1, e1)+...
        let select_expr = kept
            .iter()
            .map(|seg| format!("between(t,{:.3},{:.3})", seg.start, seg.end))
            .collect::<Vec<_>>()
            .join("+");

        let vf = format!("select='{}',setpts=N/FRAME_RATE/TB", select_expr);
        let af = format!("aselect='{}',asetpts=N/SR/TB", select_expr);

        let has_video = probe.has_video();
        let mut remove_args = vec!["-y".to_string(), "-i".to_string(), opts.input.to_string_lossy().to_string()];

        if has_video {
            remove_args.extend(vec![
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

        remove_args.extend(vec![
            "-af".to_string(),
            af,
            "-c:a".to_string(),
            "aac".to_string(),
            "-b:a".to_string(),
            "192k".to_string(),
            "-movflags".to_string(),
            "+faststart".to_string(),
            resolved_out.to_string_lossy().to_string(),
        ]);

        commands.push(format!("ffmpeg {}", remove_args.join(" ")));

        if !opts.dry_run {
            let rem_res = run_ffmpeg(&remove_args, opts.timeout).await?;
            if !rem_res.success() {
                return Err(FfmpegSkillError::ffmpeg_error(
                    format!("Silence jumpcut execution failed: {}", rem_res.stderr),
                    Some(rem_res.exit_code()),
                    Some(remove_args),
                ));
            }
        }

        output_path = Some(resolved_out.to_string_lossy().to_string());
    }

    Ok(SilenceResult {
        status: if opts.dry_run { "dry_run".to_string() } else { "success".to_string() },
        input: opts.input.to_string_lossy().to_string(),
        output: output_path,
        original_duration: total_duration,
        total_silence_duration: total_silence,
        silence_count: silences.len(),
        silences,
        kept_segments: kept,
        commands,
    })
}
