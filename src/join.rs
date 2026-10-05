// ============================================================================
// join.rs — Media concatenator and transition crossfade compositor
// Part of ffmpeg-skill-rs
// ============================================================================

use crate::error::FfmpegSkillError;
use crate::probe::probe_file;
use crate::runner::run_ffmpeg;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

/// Transition styles supported between joined media clips.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JoinTransition {
    None,
    Fade,
    Dissolve,
    Wipeleft,
    Wiperight,
    Slideleft,
    Slideright,
    Circlecrop,
}

impl JoinTransition {
    pub fn from_str_loose(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "fade" => Self::Fade,
            "dissolve" => Self::Dissolve,
            "wipeleft" => Self::Wipeleft,
            "wiperight" => Self::Wiperight,
            "slideleft" => Self::Slideleft,
            "slideright" => Self::Slideright,
            "circlecrop" => Self::Circlecrop,
            _ => Self::None,
        }
    }
}

/// Options configuring media concatenation.
#[derive(Debug, Clone)]
pub struct JoinOptions {
    pub inputs: Vec<PathBuf>,
    pub output: Option<PathBuf>,
    pub transition: JoinTransition,
    pub transition_duration: f64,
    pub stream_copy: bool,
    pub dry_run: bool,
    pub timeout: Option<f64>,
}

/// Structured join result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinResult {
    pub status: String,
    pub inputs_count: usize,
    pub output: String,
    pub transition: String,
    pub total_duration: f64,
    pub commands: Vec<String>,
}

/// Concatenates multiple media clips into one.
pub async fn join_media(opts: &JoinOptions) -> Result<JoinResult, FfmpegSkillError> {
    if opts.inputs.len() < 2 {
        return Err(FfmpegSkillError::input_error(
            "At least two input files are required to join",
        ));
    }

    for input in &opts.inputs {
        if !input.exists() {
            return Err(FfmpegSkillError::input_error(format!(
                "Input file '{}' does not exist",
                input.display()
            )));
        }
    }

    // Determine output file
    let output = match &opts.output {
        Some(p) => p.clone(),
        None => {
            let first = &opts.inputs[0];
            let stem = first
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("joined");
            let ext = first.extension().and_then(|e| e.to_str()).unwrap_or("mp4");
            let mut out = first.with_file_name(format!("{}_joined.{}", stem, ext));
            let mut counter = 1;
            while out.exists() {
                out = first.with_file_name(format!("{}_joined_{}.{}", stem, counter, ext));
                counter += 1;
            }
            out
        }
    };

    let mut commands = Vec::new();

    // If stream copy is requested and no transition is specified
    if opts.stream_copy && opts.transition == JoinTransition::None {
        let temp_dir = tempfile::Builder::new()
            .prefix("fms_join_")
            .tempdir()
            .map_err(|e| FfmpegSkillError::output_error(e.to_string()))?;
        let list_path = temp_dir.path().join("concat_list.txt");

        {
            let mut f = File::create(&list_path)
                .map_err(|e| FfmpegSkillError::output_error(e.to_string()))?;
            for input in &opts.inputs {
                let canonical = input.canonicalize().unwrap_or_else(|_| input.clone());
                // escape single quotes
                let escaped = canonical.to_string_lossy().replace('\'', "'\\''");
                writeln!(f, "file '{}'", escaped)
                    .map_err(|e| FfmpegSkillError::output_error(e.to_string()))?;
            }
        }

        let args = vec![
            "-y".to_string(),
            "-f".to_string(),
            "concat".to_string(),
            "-safe".to_string(),
            "0".to_string(),
            "-i".to_string(),
            list_path.to_string_lossy().to_string(),
            "-c".to_string(),
            "copy".to_string(),
            "-movflags".to_string(),
            "+faststart".to_string(),
            output.to_string_lossy().to_string(),
        ];

        let cmd_str = format!("ffmpeg {}", args.join(" "));
        commands.push(cmd_str);

        if !opts.dry_run {
            let runner_res = run_ffmpeg(&args, opts.timeout).await?;
            if !runner_res.success() {
                return Err(FfmpegSkillError::ffmpeg_error(
                    format!("Join concat execution failed: {}", runner_res.stderr),
                    Some(runner_res.exit_code()),
                    Some(args),
                ));
            }

            let out_probe = probe_file(&output).await.map_err(|e| {
                FfmpegSkillError::verification_error(format!("Failed to probe joined output: {}", e))
            })?;

            return Ok(JoinResult {
                status: "success".to_string(),
                inputs_count: opts.inputs.len(),
                output: output.to_string_lossy().to_string(),
                transition: "none".to_string(),
                total_duration: out_probe.duration,
                commands,
            });
        } else {
            return Ok(JoinResult {
                status: "dry_run".to_string(),
                inputs_count: opts.inputs.len(),
                output: output.to_string_lossy().to_string(),
                transition: "none".to_string(),
                total_duration: 0.0,
                commands,
            });
        }
    }

    // Re-encoding or transition path
    // Probe all files to get durations and dimensions
    let mut durations = Vec::new();
    let mut max_w = 1920;
    let mut max_h = 1080;

    for inp in &opts.inputs {
        let p = probe_file(inp).await?;
        durations.push(p.duration);
        if let Some((w, h)) = p.resolution() {
            if w > max_w { max_w = w; }
            if h > max_h { max_h = h; }
        }
    }

    // Make sure dimensions are even
    if max_w % 2 != 0 { max_w -= 1; }
    if max_h % 2 != 0 { max_h -= 1; }

    let n = opts.inputs.len();
    let mut args = vec!["-y".to_string()];
    for inp in &opts.inputs {
        args.push("-i".to_string());
        args.push(inp.to_string_lossy().to_string());
    }

    let mut filter_complex = String::new();

    if opts.transition == JoinTransition::None {
        // Simple filter_complex concat
        for i in 0..n {
            filter_complex.push_str(&format!(
                "[{}:v]scale={}:{}:force_original_aspect_ratio=decrease,pad={}:{}:(ow-iw)/2:(oh-ih)/2:black,setsar=1,fps=30[v{}];",
                i, max_w, max_h, max_w, max_h, i
            ));
            filter_complex.push_str(&format!("[{}:a]aformat=sample_rates=48000:channel_layouts=stereo[a{}];", i, i));
        }
        for i in 0..n {
            filter_complex.push_str(&format!("[v{}][a{}]", i, i));
        }
        filter_complex.push_str(&format!("concat=n={}:v=1:a=1[v][a]", n));
    } else {
        // xfade transitions
        let trans_str = format!("{:?}", opts.transition).to_lowercase();
        let trans_d = opts.transition_duration.max(0.1);

        for i in 0..n {
            filter_complex.push_str(&format!(
                "[{}:v]scale={}:{}:force_original_aspect_ratio=decrease,pad={}:{}:(ow-iw)/2:(oh-ih)/2:black,setsar=1,fps=30[v{}];",
                i, max_w, max_h, max_w, max_h, i
            ));
            filter_complex.push_str(&format!("[{}:a]aformat=sample_rates=48000:channel_layouts=stereo[a{}];", i, i));
        }

        let mut current_v = "v0".to_string();
        let mut current_a = "a0".to_string();
        let mut accum_offset = durations[0] - trans_d;

        for i in 1..n {
            let next_v = format!("v{}", i);
            let next_a = format!("a{}", i);
            let out_v = format!("vx{}", i);
            let out_a = format!("ax{}", i);

            let offset = accum_offset.max(0.1);

            filter_complex.push_str(&format!(
                "[{}][{}]xfade=transition={}:duration={:.2}:offset={:.2}[{}];",
                current_v, next_v, trans_str, trans_d, offset, out_v
            ));
            filter_complex.push_str(&format!(
                "[{}][{}]acrossfade=d={:.2}:c1=tri:c2=tri[{}];",
                current_a, next_a, trans_d, out_a
            ));

            current_v = out_v;
            current_a = out_a;
            accum_offset = offset + durations[i] - trans_d;
        }

        // Rename final outputs
        filter_complex.push_str(&format!("[{}]copy[v];", current_v));
        filter_complex.push_str(&format!("[{}]copy[a]", current_a));
    }

    args.extend(vec![
        "-filter_complex".to_string(),
        filter_complex,
        "-map".to_string(),
        "[v]".to_string(),
        "-map".to_string(),
        "[a]".to_string(),
        "-c:v".to_string(),
        "libx264".to_string(),
        "-crf".to_string(),
        "18".to_string(),
        "-preset".to_string(),
        "fast".to_string(),
        "-c:a".to_string(),
        "aac".to_string(),
        "-b:a".to_string(),
        "192k".to_string(),
        "-movflags".to_string(),
        "+faststart".to_string(),
        output.to_string_lossy().to_string(),
    ]);

    commands.push(format!("ffmpeg {}", args.join(" ")));

    if !opts.dry_run {
        let runner_res = run_ffmpeg(&args, opts.timeout).await?;
        if !runner_res.success() {
            return Err(FfmpegSkillError::ffmpeg_error(
                format!("Join composition execution failed: {}", runner_res.stderr),
                Some(runner_res.exit_code()),
                Some(args),
            ));
        }

        let out_probe = probe_file(&output).await.map_err(|e| {
            FfmpegSkillError::verification_error(format!("Failed to probe joined output: {}", e))
        })?;

        Ok(JoinResult {
            status: "success".to_string(),
            inputs_count: opts.inputs.len(),
            output: output.to_string_lossy().to_string(),
            transition: format!("{:?}", opts.transition).to_lowercase(),
            total_duration: out_probe.duration,
            commands,
        })
    } else {
        Ok(JoinResult {
            status: "dry_run".to_string(),
            inputs_count: opts.inputs.len(),
            output: output.to_string_lossy().to_string(),
            transition: format!("{:?}", opts.transition).to_lowercase(),
            total_duration: durations.iter().sum(),
            commands,
        })
    }
}
