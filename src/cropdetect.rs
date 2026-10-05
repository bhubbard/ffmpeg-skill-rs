// ============================================================================
// cropdetect.rs — Black bar and letterbox detection analyzer
// Part of ffmpeg-skill-rs
// ============================================================================

use crate::error::FfmpegSkillError;
use crate::probe::probe_file;
use crate::runner::run_ffmpeg;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Options configuring crop detection.
#[derive(Debug, Clone)]
pub struct CropdetectOptions {
    pub input: PathBuf,
    pub limit: Option<f64>,
    pub round: Option<u32>,
    pub reset: Option<u32>,
    pub timeout: Option<f64>,
}

/// A detected crop window recommendation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CropRecommendation {
    pub width: u32,
    pub height: u32,
    pub x: u32,
    pub y: u32,
    pub aspect_ratio: String,
    pub count: usize,
    pub filter_arg: String,
}

/// Structured cropdetect result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CropdetectResult {
    pub status: String,
    pub input: String,
    pub original_width: u32,
    pub original_height: u32,
    pub has_letterbox: bool,
    pub recommended_crop: Option<CropRecommendation>,
    pub all_detections: Vec<CropRecommendation>,
}

/// Runs cropdetect filter and parses bounding suggestions.
pub async fn detect_crop(opts: &CropdetectOptions) -> Result<CropdetectResult, FfmpegSkillError> {
    if !opts.input.exists() {
        return Err(FfmpegSkillError::input_error(format!(
            "Input file '{}' does not exist",
            opts.input.display()
        )));
    }

    let probe = probe_file(&opts.input).await?;
    let (orig_w, orig_h) = probe
        .resolution()
        .ok_or_else(|| FfmpegSkillError::input_error("Input file has no video stream to analyze"))?;

    let round = opts.round.unwrap_or(16);
    let reset = opts.reset.unwrap_or(0);
    let filter = format!("cropdetect=24:{}:{}", round, reset);

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

    let runner_res = run_ffmpeg(&args, opts.timeout).await?;
    let stderr = runner_res.stderr;

    // Pattern: crop=w:h:x:y
    let re = Regex::new(r"crop=(\d+):(\d+):(\d+):(\d+)").unwrap();
    let mut counts: HashMap<(u32, u32, u32, u32), usize> = HashMap::new();

    for cap in re.captures_iter(&stderr) {
        let w: u32 = cap[1].parse().unwrap_or(0);
        let h: u32 = cap[2].parse().unwrap_or(0);
        let x: u32 = cap[3].parse().unwrap_or(0);
        let y: u32 = cap[4].parse().unwrap_or(0);

        if w > 0 && h > 0 {
            *counts.entry((w, h, x, y)).or_insert(0) += 1;
        }
    }

    let mut detections: Vec<CropRecommendation> = counts
        .into_iter()
        .map(|((w, h, x, y), count)| {
            let gcd_val = gcd(w, h);
            let aspect = format!("{}:{}", w / gcd_val, h / gcd_val);
            let filter_arg = format!("crop={}:{}:{}:{}", w, h, x, y);
            CropRecommendation {
                width: w,
                height: h,
                x,
                y,
                aspect_ratio: aspect,
                count,
                filter_arg,
            }
        })
        .collect();

    // Sort by frequency descending
    detections.sort_by(|a, b| b.count.cmp(&a.count));

    let recommended = detections.first().cloned();
    let has_letterbox = if let Some(ref rec) = recommended {
        rec.width < orig_w || rec.height < orig_h
    } else {
        false
    };

    Ok(CropdetectResult {
        status: "success".to_string(),
        input: opts.input.to_string_lossy().to_string(),
        original_width: orig_w,
        original_height: orig_h,
        has_letterbox,
        recommended_crop: recommended,
        all_detections: detections,
    })
}

fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    if a == 0 { 1 } else { a }
}
