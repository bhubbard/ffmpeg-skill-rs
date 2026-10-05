// ============================================================================
// lib.rs — High-performance native Rust FFmpeg toolkit & MCP server
// ffmpeg-skill-rs
// ============================================================================

pub mod check;
pub mod cli;
pub mod contract;
pub mod crop;
pub mod cropdetect;
pub mod cut;
pub mod doctor;
pub mod error;
pub mod export;
pub mod fit;
pub mod join;
pub mod loudness;
pub mod mcp;
pub mod probe;
pub mod reverse;
pub mod runner;
pub mod scenes;
pub mod silence;
pub mod speedramp;
pub mod waveform;

use clap::Parser;
use colored::Colorize;
use crate::check::{check_compliance, CheckOptions, TargetPlatform};
use crate::cli::{Cli, Commands};
use crate::contract::generate_contract_json;
use crate::crop::{crop_video, CropOptions};
use crate::cropdetect::{detect_crop, CropdetectOptions};
use crate::cut::{cut_media, parse_time_to_seconds, CutOptions};
use crate::doctor::inspect_capabilities;
use crate::error::{ExitCodes, FfmpegSkillError};
use crate::export::{export_media, ExportOptions, ExportPreset};
use crate::fit::{fit_media, FitMode, FitOptions};
use crate::join::{join_media, JoinOptions, JoinTransition};
use crate::loudness::{normalize_loudness, LoudnessOptions, LoudnessPreset};
use crate::mcp::run_mcp_server;
use crate::probe::{probe_file, FieldSelector};
use crate::reverse::{reverse_media, ReverseOptions};
use crate::scenes::{detect_scenes, SceneOptions};
use crate::silence::{process_silence, SilenceOptions};
use crate::speedramp::{change_speed, SpeedOptions};
use crate::waveform::{generate_waveform, WaveformMode, WaveformOptions};
use std::process::ExitCode;

/// Main entry point for the CLI applications (`ffmpeg-skill` and `fms`).
pub async fn run_cli() -> ExitCode {
    let cli = Cli::parse();

    if cli.verbose {
        let subscriber = tracing_subscriber::fmt()
            .with_env_filter("ffmpeg_skill=debug,info")
            .finish();
        let _ = tracing::subscriber::set_global_default(subscriber);
    }

    match run_app(cli).await {
        Ok(_) => ExitCode::from(ExitCodes::SUCCESS as u8),
        Err(err) => {
            eprintln!("{} {}", "error:".bold().red(), err);
            ExitCode::from(err.exit_code() as u8)
        }
    }
}

/// Dispatches parsed CLI commands to their respective engine functions.
pub async fn run_app(cli: Cli) -> Result<(), FfmpegSkillError> {
    let json_output = cli.json;
    let timeout = cli.timeout;

    match cli.command {
        Commands::Doctor => {
            let report = inspect_capabilities().await?;
            if json_output {
                println!("{}", serde_json::to_string_pretty(&report).unwrap());
            } else {
                report.print_human_readable();
            }
        }
        Commands::Probe(args) => {
            let probe = probe_file(&args.input).await?;
            let value = serde_json::to_value(&probe).unwrap();

            if let Some(field) = args.field {
                let sel = FieldSelector::new(&field);
                let extracted = sel.extract(&value);
                if args.compact {
                    println!("{}", serde_json::to_string(&extracted).unwrap());
                } else {
                    println!("{}", serde_json::to_string_pretty(&extracted).unwrap());
                }
            } else if args.compact {
                println!("{}", serde_json::to_string(&probe).unwrap());
            } else {
                println!("{}", serde_json::to_string_pretty(&probe).unwrap());
            }
        }
        Commands::Cut(args) => {
            let start = args.start.as_deref().and_then(parse_time_to_seconds);
            let end = args.end.as_deref().and_then(parse_time_to_seconds);

            let opts = CutOptions {
                input: args.input,
                output: args.output,
                start,
                end,
                duration: args.duration,
                segments: None,
                accurate: args.accurate,
                dry_run: args.dry_run,
                timeout,
            };

            let res = cut_media(&opts).await?;
            if json_output {
                println!("{}", serde_json::to_string_pretty(&res).unwrap());
            } else {
                println!("{} Cut completed: {}", "✔".green(), res.output);
                println!("  Duration: {:.2}s (delta: {:.3}s)", res.output_duration, res.duration_delta_seconds);
            }
        }
        Commands::Fit(args) => {
            let fit_mode = match args.mode.to_lowercase().as_str() {
                "crop" => FitMode::Crop,
                "blur" => FitMode::Blur,
                _ => FitMode::Pad,
            };

            let flip = if args.flip_h {
                Some("h".to_string())
            } else if args.flip_v {
                Some("v".to_string())
            } else {
                None
            };

            let opts = FitOptions {
                input: args.input,
                output: args.output,
                aspect: args.target_aspect,
                fit_mode,
                width: None,
                height: None,
                duration: None,
                duration_method: crate::fit::DurationMethod::Trim,
                rotate: args.rotate.map(|r| r as i32),
                flip,
                fps: args.fps,
                dry_run: args.dry_run,
                timeout,
            };

            let res = fit_media(&opts).await?;
            if json_output {
                println!("{}", serde_json::to_string_pretty(&res).unwrap());
            } else {
                println!("{} Conformed media: {}", "✔".green(), res.output);
                println!("  Dimensions: {}x{} -> {}x{}", res.original_width, res.original_height, res.output_width, res.output_height);
            }
        }
        Commands::Crop(args) => {
            let opts = CropOptions {
                input: args.input,
                output: args.output,
                width: args.width,
                height: args.height,
                x: args.x,
                y: args.y,
                crf: args.crf,
                preset: None,
                dry_run: args.dry_run,
                timeout,
            };

            let res = crop_video(&opts).await?;
            if json_output {
                println!("{}", serde_json::to_string_pretty(&res).unwrap());
            } else {
                println!("{} Cropped video: {}", "✔".green(), res.output);
                println!("  Dimensions: {}x{} at ({}, {})", res.crop_width, res.crop_height, res.crop_x, res.crop_y);
            }
        }
        Commands::Cropdetect(args) => {
            let opts = CropdetectOptions {
                input: args.input,
                limit: args.limit,
                round: Some(args.round),
                reset: None,
                timeout,
            };

            let res = detect_crop(&opts).await?;
            if json_output {
                println!("{}", serde_json::to_string_pretty(&res).unwrap());
            } else {
                println!("{} Letterbox detection:", "ℹ".cyan());
                if let Some(rec) = res.recommended_crop {
                    println!("  Recommended crop: {}x{} at ({},{}) [aspect: {}]", rec.width, rec.height, rec.x, rec.y, rec.aspect_ratio);
                    println!("  Filter: {}", rec.filter_arg.yellow());
                } else {
                    println!("  No letterbox bars detected (full frame active)");
                }
            }
        }
        Commands::Export(args) => {
            let preset = ExportPreset::from_str_loose(&args.preset).unwrap_or(ExportPreset::Youtube);
            let opts = ExportOptions {
                input: args.input,
                output: args.output,
                preset,
                crf: args.crf,
                hwaccel: Some(args.hwaccel),
                fps: args.fps,
                scale: args.scale,
                dry_run: args.dry_run,
                timeout,
            };

            let res = export_media(&opts).await?;
            if json_output {
                println!("{}", serde_json::to_string_pretty(&res).unwrap());
            } else {
                println!("{} Exported delivery asset: {}", "✔".green(), res.output);
                println!("  Preset: {} | Video: {} | Audio: {}", res.preset, res.video_codec, res.audio_codec);
                println!("  Size: {:.2} MB", res.file_size_bytes as f64 / (1024.0 * 1024.0));
            }
        }
        Commands::Join(args) => {
            let transition = JoinTransition::from_str_loose(&args.transition);
            let opts = JoinOptions {
                inputs: args.inputs,
                output: args.output,
                transition,
                transition_duration: args.transition_duration,
                stream_copy: args.stream_copy,
                dry_run: args.dry_run,
                timeout,
            };

            let res = join_media(&opts).await?;
            if json_output {
                println!("{}", serde_json::to_string_pretty(&res).unwrap());
            } else {
                println!("{} Joined {} inputs: {}", "✔".green(), res.inputs_count, res.output);
                println!("  Transition: {} | Duration: {:.2}s", res.transition, res.total_duration);
            }
        }
        Commands::Loudness(args) => {
            let preset = LoudnessPreset::from_str_loose(&args.preset);
            let opts = LoudnessOptions {
                input: args.input,
                output: args.output,
                preset,
                target_i: args.target_i,
                target_tp: args.target_tp,
                target_lra: None,
                single_pass: args.single_pass,
                dry_run: args.dry_run,
                timeout,
            };

            let res = normalize_loudness(&opts).await?;
            if json_output {
                println!("{}", serde_json::to_string_pretty(&res).unwrap());
            } else {
                println!("{} Normalized loudness: {}", "✔".green(), res.output);
                println!("  Target: {:.1} LUFS (True Peak: {:.1} dBTP)", res.target_lufs, res.target_true_peak);
            }
        }
        Commands::Silence(args) => {
            let opts = SilenceOptions {
                input: args.input,
                output: args.output,
                noise_db: args.noise_db,
                min_duration: args.min_duration,
                remove: args.remove,
                padding: args.padding,
                dry_run: args.dry_run,
                timeout,
            };

            let res = process_silence(&opts).await?;
            if json_output {
                println!("{}", serde_json::to_string_pretty(&res).unwrap());
            } else {
                println!("{} Silence Analysis:", "ℹ".cyan());
                println!("  Detected {} silent pauses ({:.2}s total silence)", res.silence_count, res.total_silence_duration);
                if let Some(out) = res.output {
                    println!("  {} Jump-cut video saved: {}", "✔".green(), out);
                }
            }
        }
        Commands::Speedramp(args) => {
            let opts = SpeedOptions {
                input: args.input,
                output: args.output,
                speed: args.speed,
                preserve_pitch: true,
                dry_run: args.dry_run,
                timeout,
            };

            let res = change_speed(&opts).await?;
            if json_output {
                println!("{}", serde_json::to_string_pretty(&res).unwrap());
            } else {
                println!("{} Speed changed to {:.2}x: {}", "✔".green(), res.speed_factor, res.output);
                println!("  Duration: {:.2}s -> {:.2}s", res.original_duration, res.new_duration);
            }
        }
        Commands::Reverse(args) => {
            let opts = ReverseOptions {
                input: args.input,
                output: args.output,
                video_only: args.video_only,
                audio_only: args.audio_only,
                dry_run: args.dry_run,
                timeout,
            };

            let res = reverse_media(&opts).await?;
            if json_output {
                println!("{}", serde_json::to_string_pretty(&res).unwrap());
            } else {
                println!("{} Reversed media playback: {}", "✔".green(), res.output);
            }
        }
        Commands::Waveform(args) => {
            let mode = WaveformMode::from_str_loose(&args.mode);
            let opts = WaveformOptions {
                input: args.input,
                output: args.output,
                mode,
                width: args.width,
                height: args.height,
                color: args.color,
                bg_color: "#000000".to_string(),
                dry_run: args.dry_run,
                timeout,
            };

            let res = generate_waveform(&opts).await?;
            if json_output {
                println!("{}", serde_json::to_string_pretty(&res).unwrap());
            } else {
                println!("{} Visualized waveform: {}", "✔".green(), res.output);
                println!("  Mode: {} | Size: {}x{}", res.mode, res.width, res.height);
            }
        }
        Commands::Scenes(args) => {
            let opts = SceneOptions {
                input: args.input,
                threshold: args.threshold,
                thumbnails_dir: args.thumbnails_dir,
                limit: args.limit,
                dry_run: args.dry_run,
                timeout,
            };

            let res = detect_scenes(&opts).await?;
            if json_output {
                println!("{}", serde_json::to_string_pretty(&res).unwrap());
            } else {
                println!("{} Detected {} scene cuts", "ℹ".cyan(), res.total_scenes_detected);
                for cut in res.cuts.iter().take(10) {
                    println!("  Cut #{}: timestamp {:.3}s (frame {:?})", cut.index, cut.timestamp_seconds, cut.frame_number);
                }
                if let Some(dir) = res.thumbnails_dir {
                    println!("  Thumbnails extracted to: {}", dir);
                }
            }
        }
        Commands::Check(args) => {
            let platform = TargetPlatform::from_str_loose(&args.platform);
            let opts = CheckOptions {
                input: args.input,
                platform,
            };

            let res = check_compliance(&opts).await?;
            if json_output {
                println!("{}", serde_json::to_string_pretty(&res).unwrap());
            } else {
                if res.passed {
                    println!("{} Specification check PASSED for platform '{}'", "✔".green(), res.platform);
                } else {
                    println!("{} Specification check FAILED for platform '{}'", "✖".red(), res.platform);
                }
                for err in &res.errors {
                    println!("  {} {}", "✖".red(), err);
                }
                for warn in &res.warnings {
                    println!("  {} {}", "⚠".yellow(), warn);
                }
            }
        }
        Commands::Contract(_) => {
            let contract = generate_contract_json();
            println!("{}", serde_json::to_string_pretty(&contract).unwrap());
        }
        Commands::Mcp => {
            run_mcp_server().await?;
        }
    }

    Ok(())
}
