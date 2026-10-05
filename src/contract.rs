// ============================================================================
// contract.rs — Machine-readable contract specification generator
// Part of ffmpeg-skill-rs
// ============================================================================

use serde_json::{json, Value};

/// Generates the machine-readable contract specification JSON adhering to contract_version: "1.0".
pub fn generate_contract_json() -> Value {
    json!({
        "contract_version": "1.0",
        "skill_name": "ffmpeg-skill-rs",
        "version": env!("CARGO_PKG_VERSION"),
        "description": "High-performance native Rust fork of ffmpeg-skill: deterministic FFmpeg tools and Model Context Protocol (MCP) server for AI agents",
        "engine": "native-rust",
        "binaries": {
            "required": ["ffmpeg", "ffprobe"],
            "aliases": ["ffmpeg-skill", "fms"]
        },
        "mcp": {
            "protocol_version": "2024-11-05",
            "transport": "stdio"
        },
        "tools": [
            {
                "name": "doctor",
                "description": "Diagnose local FFmpeg capabilities, available video/audio encoders, filters, and hardware acceleration engines (VideoToolbox, NVENC, VAAPI, QSV)",
                "input_schema": {
                    "type": "object",
                    "properties": {}
                }
            },
            {
                "name": "probe",
                "description": "Extract comprehensive metadata, video streams, audio streams, codecs, resolution, aspect ratio, frame rate, VFR/CFR, and HDR status",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "input": { "type": "string", "description": "Absolute path to media file" },
                        "field": { "type": "string", "description": "Optional dotted field selector (e.g. 'duration', 'resolution', 'video[0].codec')" },
                        "compact": { "type": "boolean", "description": "Emit single-line compact JSON summary" }
                    },
                    "required": ["input"]
                }
            },
            {
                "name": "cut",
                "description": "Trim media segments with lossless stream-copy (-c copy) or frame-accurate re-encode (--accurate)",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "input": { "type": "string", "description": "Path to input media file" },
                        "output": { "type": "string", "description": "Destination file path (optional)" },
                        "start": { "type": "number", "description": "Start time in seconds or HH:MM:SS" },
                        "end": { "type": "number", "description": "End time in seconds or HH:MM:SS" },
                        "duration": { "type": "number", "description": "Cut duration in seconds" },
                        "accurate": { "type": "boolean", "description": "Frame-accurate re-encode instead of keyframe stream copy" },
                        "dry_run": { "type": "boolean", "description": "Preview commands without executing" }
                    },
                    "required": ["input"]
                }
            },
            {
                "name": "fit",
                "description": "Conform media aspect ratio, scale with pad/crop/blur modes, normalize frame rate, rotate, or fit target duration",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "input": { "type": "string", "description": "Input media path" },
                        "output": { "type": "string", "description": "Output media path (optional)" },
                        "target_aspect": { "type": "string", "description": "Target aspect ratio: '16:9', '9:16', '1:1', '4:3'" },
                        "fit_mode": { "type": "string", "enum": ["pad", "crop", "blur"], "description": "Aspect conforming strategy" },
                        "fps": { "type": "number", "description": "Enforce constant frame rate (e.g. 30 or 60)" },
                        "rotate": { "type": "integer", "enum": [90, 180, 270], "description": "Rotate degrees clockwise" },
                        "flip_h": { "type": "boolean", "description": "Flip horizontally" },
                        "flip_v": { "type": "boolean", "description": "Flip vertically" },
                        "dry_run": { "type": "boolean", "description": "Preview commands" }
                    },
                    "required": ["input"]
                }
            },
            {
                "name": "crop",
                "description": "Extract pixel-exact rectangular bounding box from video frame with even-dimension rounding",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "input": { "type": "string", "description": "Input media path" },
                        "output": { "type": "string", "description": "Output media path (optional)" },
                        "width": { "type": "integer", "description": "Crop width in pixels" },
                        "height": { "type": "integer", "description": "Crop height in pixels" },
                        "x": { "type": "integer", "description": "Top-left X coordinate" },
                        "y": { "type": "integer", "description": "Top-left Y coordinate" },
                        "crf": { "type": "integer", "description": "Constant Rate Factor (default: 18)" },
                        "dry_run": { "type": "boolean", "description": "Preview commands" }
                    },
                    "required": ["input", "width", "height", "x", "y"]
                }
            },
            {
                "name": "cropdetect",
                "description": "Analyze video stream to detect black bars, letterboxing, or pillarboxing and recommend exact crop parameters",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "input": { "type": "string", "description": "Input media path" },
                        "limit": { "type": "number", "description": "Seconds to analyze from start" },
                        "round": { "type": "integer", "description": "Dimension multiple rounding (default: 16)" }
                    },
                    "required": ["input"]
                }
            },
            {
                "name": "export",
                "description": "Encode and deliver video using optimized social/platform presets (youtube, reels, tiktok, shorts, x, prores, h265, gif, copy)",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "input": { "type": "string", "description": "Input media path" },
                        "output": { "type": "string", "description": "Output path (optional)" },
                        "preset": { "type": "string", "enum": ["youtube", "youtube4k", "reels", "tiktok", "shorts", "linkedin", "facebook", "x", "prores", "h265", "gif", "copy"] },
                        "crf": { "type": "integer", "description": "CRF override" },
                        "hwaccel": { "type": "string", "enum": ["auto", "videotoolbox", "nvenc", "none"], "description": "Hardware acceleration engine" },
                        "dry_run": { "type": "boolean", "description": "Preview commands" }
                    },
                    "required": ["input", "preset"]
                }
            },
            {
                "name": "join",
                "description": "Concatenate media clips using fast demuxer stream-copy or re-encoding with transition crossfades (fade, dissolve, wipeleft, slideleft)",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "inputs": { "type": "array", "items": { "type": "string" }, "description": "List of media file paths" },
                        "output": { "type": "string", "description": "Output file path" },
                        "transition": { "type": "string", "enum": ["none", "fade", "dissolve", "wipeleft", "wiperight", "slideleft", "slideright"], "description": "Transition style" },
                        "transition_duration": { "type": "number", "description": "Transition duration in seconds (default: 1.0)" },
                        "stream_copy": { "type": "boolean", "description": "Lossless concat demuxer copy when formats match" },
                        "dry_run": { "type": "boolean", "description": "Preview commands" }
                    },
                    "required": ["inputs"]
                }
            },
            {
                "name": "loudness",
                "description": "EBU R128 audio loudness normalization (two-pass high precision or single-pass quick)",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "input": { "type": "string", "description": "Input media path" },
                        "output": { "type": "string", "description": "Output file path (optional)" },
                        "preset": { "type": "string", "enum": ["podcast", "youtube", "broadcast", "custom"] },
                        "target_i": { "type": "number", "description": "Integrated loudness in LUFS (e.g. -16.0, -14.0, -23.0)" },
                        "target_tp": { "type": "number", "description": "True peak maximum in dBTP (default: -1.0)" },
                        "single_pass": { "type": "boolean", "description": "Single-pass fast normalization instead of two-pass" },
                        "dry_run": { "type": "boolean", "description": "Preview commands" }
                    },
                    "required": ["input"]
                }
            },
            {
                "name": "silence",
                "description": "Detect dead air silence gaps and automatically generate tight jump cuts removing pauses",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "input": { "type": "string", "description": "Input media path" },
                        "output": { "type": "string", "description": "Output jump-cut file path (optional)" },
                        "noise_db": { "type": "number", "description": "Noise floor threshold in dB (default: -30.0)" },
                        "min_duration": { "type": "number", "description": "Minimum silence duration in seconds (default: 0.5)" },
                        "remove": { "type": "boolean", "description": "Render and export dead air removed video" },
                        "padding": { "type": "number", "description": "Audio buffer padding in seconds (default: 0.1)" },
                        "dry_run": { "type": "boolean", "description": "Preview commands" }
                    },
                    "required": ["input"]
                }
            },
            {
                "name": "speedramp",
                "description": "Adjust media playback speed with smooth atempo audio time-stretching and pitch preservation",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "input": { "type": "string", "description": "Input media path" },
                        "output": { "type": "string", "description": "Output file path" },
                        "speed": { "type": "number", "description": "Speed factor (e.g. 0.5 for half speed, 2.0 for double speed)" },
                        "dry_run": { "type": "boolean", "description": "Preview commands" }
                    },
                    "required": ["input", "speed"]
                }
            },
            {
                "name": "reverse",
                "description": "Reverse video and/or audio playback backwards in time",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "input": { "type": "string", "description": "Input media path" },
                        "output": { "type": "string", "description": "Output file path" },
                        "video_only": { "type": "boolean", "description": "Reverse video stream only" },
                        "audio_only": { "type": "boolean", "description": "Reverse audio stream only" },
                        "dry_run": { "type": "boolean", "description": "Preview commands" }
                    },
                    "required": ["input"]
                }
            },
            {
                "name": "waveform",
                "description": "Render visual audio waveforms or spectrograms as PNG images or animated MP4 visualization video",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "input": { "type": "string", "description": "Input media path" },
                        "output": { "type": "string", "description": "Output path (.png or .mp4)" },
                        "mode": { "type": "string", "enum": ["waveform", "spectrogram", "video"] },
                        "width": { "type": "integer", "description": "Image/video width in pixels (default: 1280)" },
                        "height": { "type": "integer", "description": "Image/video height in pixels (default: 720)" },
                        "color": { "type": "string", "description": "Waveform hex or named color (default: '#00FFAA')" }
                    },
                    "required": ["input"]
                }
            },
            {
                "name": "scenes",
                "description": "Detect scene cuts and transitions throughout video, with optional keyframe thumbnail generation",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "input": { "type": "string", "description": "Input media path" },
                        "threshold": { "type": "number", "description": "Cut sensitivity threshold 0.0-1.0 (default: 0.3)" },
                        "thumbnails_dir": { "type": "string", "description": "Output directory for extracted scene thumbnails" },
                        "limit": { "type": "number", "description": "Analyze only first N seconds" }
                    },
                    "required": ["input"]
                }
            },
            {
                "name": "check",
                "description": "Pre-delivery specification validator against YouTube, TikTok, Reels, X, and Broadcast compliance profiles",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "input": { "type": "string", "description": "Input media path" },
                        "platform": { "type": "string", "enum": ["youtube", "tiktok", "reels", "x", "broadcast", "general"] }
                    },
                    "required": ["input"]
                }
            }
        ]
    })
}
