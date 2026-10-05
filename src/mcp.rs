// ============================================================================
// mcp.rs — Model Context Protocol (MCP) stdio JSON-RPC 2.0 server
// Part of ffmpeg-skill-rs
// ============================================================================

use crate::check::{check_compliance, CheckOptions, TargetPlatform};
use crate::crop::{crop_video, CropOptions};
use crate::cropdetect::{detect_crop, CropdetectOptions};
use crate::cut::{cut_media, CutOptions};
use crate::doctor::inspect_capabilities;
use crate::error::FfmpegSkillError;
use crate::export::{export_media, ExportOptions, ExportPreset};
use crate::fit::{fit_media, FitMode, FitOptions};
use crate::join::{join_media, JoinOptions, JoinTransition};
use crate::loudness::{normalize_loudness, LoudnessOptions, LoudnessPreset};
use crate::probe::{probe_file, FieldSelector};
use crate::reverse::{reverse_media, ReverseOptions};
use crate::scenes::{detect_scenes, SceneOptions};
use crate::silence::{process_silence, SilenceOptions};
use crate::speedramp::{change_speed, SpeedOptions};
use crate::waveform::{generate_waveform, WaveformMode, WaveformOptions};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: &'static str,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i64,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

/// Runs the MCP stdio JSON-RPC server loop.
pub async fn run_mcp_server() -> Result<(), FfmpegSkillError> {
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let reader = BufReader::new(stdin);
    let mut lines = reader.lines();

    while let Ok(Some(line)) = lines.next_line().await {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let req: JsonRpcRequest = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(e) => {
                let err_resp = json!({
                    "jsonrpc": "2.0",
                    "id": Value::Null,
                    "error": {
                        "code": -32700,
                        "message": format!("Parse error: {}", e)
                    }
                });
                let mut out_str = serde_json::to_string(&err_resp).unwrap();
                out_str.push('\n');
                let _ = stdout.write_all(out_str.as_bytes()).await;
                let _ = stdout.flush().await;
                continue;
            }
        };

        let req_id = req.id.unwrap_or(Value::Null);

        match req.method.as_str() {
            "initialize" => {
                let init_res = json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "tools": {}
                    },
                    "serverInfo": {
                        "name": "ffmpeg-skill-rs",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                });
                send_response(&mut stdout, req_id, Some(init_res), None).await?;
            }
            "notifications/initialized" | "initialized" => {
                // MCP notification; no response required
            }
            "ping" => {
                send_response(&mut stdout, req_id, Some(json!({})), None).await?;
            }
            "tools/list" => {
                let contract = crate::contract::generate_contract_json();
                let tools_raw = contract.get("tools").cloned().unwrap_or(json!([]));
                let tools_formatted: Vec<Value> = tools_raw
                    .as_array()
                    .unwrap_or(&vec![])
                    .iter()
                    .map(|t| {
                        json!({
                            "name": t.get("name"),
                            "description": t.get("description"),
                            "inputSchema": t.get("input_schema")
                        })
                    })
                    .collect();

                send_response(&mut stdout, req_id, Some(json!({ "tools": tools_formatted })), None).await?;
            }
            "tools/call" => {
                let tool_name = req.params.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let args = req.params.get("arguments").cloned().unwrap_or(json!({}));

                match dispatch_tool_call(tool_name, &args).await {
                    Ok(val) => {
                        let result_payload = json!({
                            "content": [
                                {
                                    "type": "text",
                                    "text": serde_json::to_string_pretty(&val).unwrap_or_default()
                                }
                            ]
                        });
                        send_response(&mut stdout, req_id, Some(result_payload), None).await?;
                    }
                    Err(e) => {
                        let err_payload = json!({
                            "content": [
                                {
                                    "type": "text",
                                    "text": format!("Error: {}", e)
                                }
                            ],
                            "isError": true
                        });
                        send_response(&mut stdout, req_id, Some(err_payload), None).await?;
                    }
                }
            }
            other => {
                send_response(
                    &mut stdout,
                    req_id,
                    None,
                    Some(JsonRpcError {
                        code: -32601,
                        message: format!("Method not found: {}", other),
                        data: None,
                    }),
                )
                .await?;
            }
        }
    }

    Ok(())
}

async fn send_response(
    stdout: &mut tokio::io::Stdout,
    id: Value,
    result: Option<Value>,
    error: Option<JsonRpcError>,
) -> Result<(), FfmpegSkillError> {
    let resp = JsonRpcResponse {
        jsonrpc: "2.0",
        id,
        result,
        error,
    };
    let mut out_str = serde_json::to_string(&resp).map_err(|e| FfmpegSkillError::output_error(e.to_string()))?;
    out_str.push('\n');
    stdout.write_all(out_str.as_bytes()).await.map_err(|e| FfmpegSkillError::output_error(e.to_string()))?;
    stdout.flush().await.map_err(|e| FfmpegSkillError::output_error(e.to_string()))?;
    Ok(())
}

async fn dispatch_tool_call(name: &str, args: &Value) -> Result<Value, FfmpegSkillError> {
    match name {
        "doctor" => {
            let res = inspect_capabilities().await?;
            Ok(serde_json::to_value(res).unwrap())
        }
        "probe" => {
            let input = args.get("input").and_then(|v| v.as_str()).ok_or_else(|| {
                FfmpegSkillError::input_error("Missing required argument 'input'")
            })?;
            let probe = probe_file(input).await?;
            if let Some(field) = args.get("field").and_then(|v| v.as_str()) {
                let sel = FieldSelector::new(field);
                let val = sel.extract(&serde_json::to_value(&probe).unwrap());
                Ok(val)
            } else {
                Ok(serde_json::to_value(probe).unwrap())
            }
        }
        "cut" => {
            let input = args.get("input").and_then(|v| v.as_str()).ok_or_else(|| {
                FfmpegSkillError::input_error("Missing required argument 'input'")
            })?;
            let output = args.get("output").and_then(|v| v.as_str()).map(PathBuf::from);
            let start = args.get("start").and_then(|v| v.as_f64());
            let end = args.get("end").and_then(|v| v.as_f64());
            let duration = args.get("duration").and_then(|v| v.as_f64());
            let accurate = args.get("accurate").and_then(|v| v.as_bool()).unwrap_or(false);
            let dry_run = args.get("dry_run").and_then(|v| v.as_bool()).unwrap_or(false);

            let opts = CutOptions {
                input: PathBuf::from(input),
                output,
                start,
                end,
                duration,
                segments: None,
                accurate,
                dry_run,
                timeout: None,
            };
            let res = cut_media(&opts).await?;
            Ok(serde_json::to_value(res).unwrap())
        }
        "fit" => {
            let input = args.get("input").and_then(|v| v.as_str()).ok_or_else(|| {
                FfmpegSkillError::input_error("Missing required argument 'input'")
            })?;
            let output = args.get("output").and_then(|v| v.as_str()).map(PathBuf::from);
            let target_aspect = args.get("target_aspect").and_then(|v| v.as_str()).map(String::from);
            let fit_mode_str = args.get("fit_mode").and_then(|v| v.as_str()).unwrap_or("pad");
            let fit_mode = match fit_mode_str {
                "crop" => FitMode::Crop,
                "blur" => FitMode::Blur,
                _ => FitMode::Pad,
            };
            let fps = args.get("fps").and_then(|v| v.as_f64());
            let rotate = args.get("rotate").and_then(|v| v.as_i64()).map(|n| n as i32);
            let flip = if args.get("flip_h").and_then(|v| v.as_bool()).unwrap_or(false) {
                Some("h".to_string())
            } else if args.get("flip_v").and_then(|v| v.as_bool()).unwrap_or(false) {
                Some("v".to_string())
            } else {
                None
            };
            let dry_run = args.get("dry_run").and_then(|v| v.as_bool()).unwrap_or(false);

            let opts = FitOptions {
                input: PathBuf::from(input),
                output,
                aspect: target_aspect,
                fit_mode,
                width: None,
                height: None,
                duration: None,
                duration_method: crate::fit::DurationMethod::Trim,
                rotate,
                flip,
                fps,
                dry_run,
                timeout: None,
            };
            let res = fit_media(&opts).await?;
            Ok(serde_json::to_value(res).unwrap())
        }
        "crop" => {
            let input = args.get("input").and_then(|v| v.as_str()).ok_or_else(|| {
                FfmpegSkillError::input_error("Missing required argument 'input'")
            })?;
            let output = args.get("output").and_then(|v| v.as_str()).map(PathBuf::from);
            let width = args.get("width").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            let height = args.get("height").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            let x = args.get("x").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            let y = args.get("y").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            let crf = args.get("crf").and_then(|v| v.as_u64()).map(|n| n as u32);
            let dry_run = args.get("dry_run").and_then(|v| v.as_bool()).unwrap_or(false);

            let opts = CropOptions {
                input: PathBuf::from(input),
                output,
                width,
                height,
                x,
                y,
                crf,
                preset: None,
                dry_run,
                timeout: None,
            };
            let res = crop_video(&opts).await?;
            Ok(serde_json::to_value(res).unwrap())
        }
        "cropdetect" => {
            let input = args.get("input").and_then(|v| v.as_str()).ok_or_else(|| {
                FfmpegSkillError::input_error("Missing required argument 'input'")
            })?;
            let limit = args.get("limit").and_then(|v| v.as_f64());
            let round = args.get("round").and_then(|v| v.as_u64()).map(|n| n as u32);

            let opts = CropdetectOptions {
                input: PathBuf::from(input),
                limit,
                round,
                reset: None,
                timeout: None,
            };
            let res = detect_crop(&opts).await?;
            Ok(serde_json::to_value(res).unwrap())
        }
        "export" => {
            let input = args.get("input").and_then(|v| v.as_str()).ok_or_else(|| {
                FfmpegSkillError::input_error("Missing required argument 'input'")
            })?;
            let output = args.get("output").and_then(|v| v.as_str()).map(PathBuf::from);
            let preset_str = args.get("preset").and_then(|v| v.as_str()).unwrap_or("youtube");
            let preset = ExportPreset::from_str_loose(preset_str).unwrap_or(ExportPreset::Youtube);
            let crf = args.get("crf").and_then(|v| v.as_u64()).map(|n| n as u32);
            let hwaccel = args.get("hwaccel").and_then(|v| v.as_str()).map(String::from);
            let dry_run = args.get("dry_run").and_then(|v| v.as_bool()).unwrap_or(false);

            let opts = ExportOptions {
                input: PathBuf::from(input),
                output,
                preset,
                crf,
                hwaccel,
                fps: None,
                scale: None,
                dry_run,
                timeout: None,
            };
            let res = export_media(&opts).await?;
            Ok(serde_json::to_value(res).unwrap())
        }
        "join" => {
            let inputs_arr = args.get("inputs").and_then(|v| v.as_array()).ok_or_else(|| {
                FfmpegSkillError::input_error("Missing required argument 'inputs'")
            })?;
            let inputs: Vec<PathBuf> = inputs_arr
                .iter()
                .filter_map(|v| v.as_str().map(PathBuf::from))
                .collect();
            let output = args.get("output").and_then(|v| v.as_str()).map(PathBuf::from);
            let transition_str = args.get("transition").and_then(|v| v.as_str()).unwrap_or("none");
            let transition = JoinTransition::from_str_loose(transition_str);
            let transition_duration = args.get("transition_duration").and_then(|v| v.as_f64()).unwrap_or(1.0);
            let stream_copy = args.get("stream_copy").and_then(|v| v.as_bool()).unwrap_or(false);
            let dry_run = args.get("dry_run").and_then(|v| v.as_bool()).unwrap_or(false);

            let opts = JoinOptions {
                inputs,
                output,
                transition,
                transition_duration,
                stream_copy,
                dry_run,
                timeout: None,
            };
            let res = join_media(&opts).await?;
            Ok(serde_json::to_value(res).unwrap())
        }
        "loudness" => {
            let input = args.get("input").and_then(|v| v.as_str()).ok_or_else(|| {
                FfmpegSkillError::input_error("Missing required argument 'input'")
            })?;
            let output = args.get("output").and_then(|v| v.as_str()).map(PathBuf::from);
            let preset_str = args.get("preset").and_then(|v| v.as_str()).unwrap_or("podcast");
            let preset = LoudnessPreset::from_str_loose(preset_str);
            let target_i = args.get("target_i").and_then(|v| v.as_f64());
            let target_tp = args.get("target_tp").and_then(|v| v.as_f64());
            let single_pass = args.get("single_pass").and_then(|v| v.as_bool()).unwrap_or(false);
            let dry_run = args.get("dry_run").and_then(|v| v.as_bool()).unwrap_or(false);

            let opts = LoudnessOptions {
                input: PathBuf::from(input),
                output,
                preset,
                target_i,
                target_tp,
                target_lra: None,
                single_pass,
                dry_run,
                timeout: None,
            };
            let res = normalize_loudness(&opts).await?;
            Ok(serde_json::to_value(res).unwrap())
        }
        "silence" => {
            let input = args.get("input").and_then(|v| v.as_str()).ok_or_else(|| {
                FfmpegSkillError::input_error("Missing required argument 'input'")
            })?;
            let output = args.get("output").and_then(|v| v.as_str()).map(PathBuf::from);
            let noise_db = args.get("noise_db").and_then(|v| v.as_f64()).unwrap_or(-30.0);
            let min_duration = args.get("min_duration").and_then(|v| v.as_f64()).unwrap_or(0.5);
            let remove = args.get("remove").and_then(|v| v.as_bool()).unwrap_or(false);
            let padding = args.get("padding").and_then(|v| v.as_f64()).unwrap_or(0.1);
            let dry_run = args.get("dry_run").and_then(|v| v.as_bool()).unwrap_or(false);

            let opts = SilenceOptions {
                input: PathBuf::from(input),
                output,
                noise_db,
                min_duration,
                remove,
                padding,
                dry_run,
                timeout: None,
            };
            let res = process_silence(&opts).await?;
            Ok(serde_json::to_value(res).unwrap())
        }
        "speedramp" => {
            let input = args.get("input").and_then(|v| v.as_str()).ok_or_else(|| {
                FfmpegSkillError::input_error("Missing required argument 'input'")
            })?;
            let output = args.get("output").and_then(|v| v.as_str()).map(PathBuf::from);
            let speed = args.get("speed").and_then(|v| v.as_f64()).unwrap_or(1.0);
            let dry_run = args.get("dry_run").and_then(|v| v.as_bool()).unwrap_or(false);

            let opts = SpeedOptions {
                input: PathBuf::from(input),
                output,
                speed,
                preserve_pitch: true,
                dry_run,
                timeout: None,
            };
            let res = change_speed(&opts).await?;
            Ok(serde_json::to_value(res).unwrap())
        }
        "reverse" => {
            let input = args.get("input").and_then(|v| v.as_str()).ok_or_else(|| {
                FfmpegSkillError::input_error("Missing required argument 'input'")
            })?;
            let output = args.get("output").and_then(|v| v.as_str()).map(PathBuf::from);
            let video_only = args.get("video_only").and_then(|v| v.as_bool()).unwrap_or(false);
            let audio_only = args.get("audio_only").and_then(|v| v.as_bool()).unwrap_or(false);
            let dry_run = args.get("dry_run").and_then(|v| v.as_bool()).unwrap_or(false);

            let opts = ReverseOptions {
                input: PathBuf::from(input),
                output,
                video_only,
                audio_only,
                dry_run,
                timeout: None,
            };
            let res = reverse_media(&opts).await?;
            Ok(serde_json::to_value(res).unwrap())
        }
        "waveform" => {
            let input = args.get("input").and_then(|v| v.as_str()).ok_or_else(|| {
                FfmpegSkillError::input_error("Missing required argument 'input'")
            })?;
            let output = args.get("output").and_then(|v| v.as_str()).map(PathBuf::from);
            let mode_str = args.get("mode").and_then(|v| v.as_str()).unwrap_or("waveform");
            let mode = WaveformMode::from_str_loose(mode_str);
            let width = args.get("width").and_then(|v| v.as_u64()).unwrap_or(1280) as u32;
            let height = args.get("height").and_then(|v| v.as_u64()).unwrap_or(720) as u32;
            let color = args.get("color").and_then(|v| v.as_str()).unwrap_or("#00FFAA").to_string();
            let dry_run = args.get("dry_run").and_then(|v| v.as_bool()).unwrap_or(false);

            let opts = WaveformOptions {
                input: PathBuf::from(input),
                output,
                mode,
                width,
                height,
                color,
                bg_color: "#000000".to_string(),
                dry_run,
                timeout: None,
            };
            let res = generate_waveform(&opts).await?;
            Ok(serde_json::to_value(res).unwrap())
        }
        "scenes" => {
            let input = args.get("input").and_then(|v| v.as_str()).ok_or_else(|| {
                FfmpegSkillError::input_error("Missing required argument 'input'")
            })?;
            let threshold = args.get("threshold").and_then(|v| v.as_f64()).unwrap_or(0.3);
            let thumbnails_dir = args.get("thumbnails_dir").and_then(|v| v.as_str()).map(PathBuf::from);
            let limit = args.get("limit").and_then(|v| v.as_f64());
            let dry_run = args.get("dry_run").and_then(|v| v.as_bool()).unwrap_or(false);

            let opts = SceneOptions {
                input: PathBuf::from(input),
                threshold,
                thumbnails_dir,
                limit,
                dry_run,
                timeout: None,
            };
            let res = detect_scenes(&opts).await?;
            Ok(serde_json::to_value(res).unwrap())
        }
        "check" => {
            let input = args.get("input").and_then(|v| v.as_str()).ok_or_else(|| {
                FfmpegSkillError::input_error("Missing required argument 'input'")
            })?;
            let platform_str = args.get("platform").and_then(|v| v.as_str()).unwrap_or("general");
            let platform = TargetPlatform::from_str_loose(platform_str);

            let opts = CheckOptions {
                input: PathBuf::from(input),
                platform,
            };
            let res = check_compliance(&opts).await?;
            Ok(serde_json::to_value(res).unwrap())
        }
        unknown => Err(FfmpegSkillError::input_error(format!(
            "Unknown tool: {}",
            unknown
        ))),
    }
}
