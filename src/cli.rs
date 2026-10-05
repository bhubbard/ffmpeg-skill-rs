// ============================================================================
// cli.rs — Command-line interface definitions and arguments parser
// Part of ffmpeg-skill-rs
// ============================================================================

use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "ffmpeg-skill",
    bin_name = "ffmpeg-skill",
    author = "Brandon Hubbard",
    version = env!("CARGO_PKG_VERSION"),
    about = "High-performance native Rust fork of ffmpeg-skill: deterministic FFmpeg tools and Model Context Protocol (MCP) server for AI agents",
    long_about = "A native Rust video engineering toolkit and Model Context Protocol (MCP) server providing deterministic, machine-readable FFmpeg execution for AI agents and human creators."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Force JSON output for stdout
    #[arg(long, global = true)]
    pub json: bool,

    /// Verbose output logging
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Execution timeout in seconds
    #[arg(long, global = true)]
    pub timeout: Option<f64>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Inspect local FFmpeg environment, encoders, filters, and GPU hardware acceleration engines
    Doctor,

    /// Extract media metadata, streams, codecs, HDR, VFR, and duration
    Probe(ProbeArgs),

    /// Trim media segments with lossless stream-copy or frame-accurate re-encode
    Cut(CutArgs),

    /// Conform aspect ratio, scale (pad/crop/blur), normalize frame rate, or rotate
    Fit(FitArgs),

    /// Crop exact pixel rectangular bounding box
    Crop(CropArgs),

    /// Detect black bars, letterboxing, or pillarboxing and recommend crop coordinates
    Cropdetect(CropdetectArgs),

    /// Encode and deliver video using optimized social/platform presets
    Export(ExportArgs),

    /// Concatenate media clips with optional crossfade transitions (fade, dissolve, wipe, slide)
    Join(JoinArgs),

    /// EBU R128 audio loudness normalization (two-pass high precision or single-pass)
    Loudness(LoudnessArgs),

    /// Detect dead air silence gaps and automatically generate tight jump cuts
    Silence(SilenceArgs),

    /// Adjust playback speed with smooth pitch-preserved audio time-stretching
    Speedramp(SpeedrampArgs),

    /// Reverse video and/or audio playback backwards in time
    Reverse(ReverseArgs),

    /// Render visual audio waveforms or spectrograms as PNG images or animated MP4 video
    Waveform(WaveformArgs),

    /// Detect scene cuts and transitions throughout video with optional keyframe thumbnail generation
    Scenes(ScenesArgs),

    /// Validate pre-delivery specification compliance against platform delivery matrices
    Check(CheckArgs),

    /// Emit machine-readable contract specification JSON adhering to contract_version: "1.0"
    Contract(ContractArgs),

    /// Start Model Context Protocol (MCP) stdio JSON-RPC 2.0 server
    Mcp,
}

#[derive(Args, Debug)]
pub struct ProbeArgs {
    /// Input media file path
    pub input: PathBuf,

    /// Dotted query field selector (e.g. 'duration', 'resolution', 'video[0].codec')
    #[arg(short, long)]
    pub field: Option<String>,

    /// Compact single-line JSON format
    #[arg(short, long)]
    pub compact: bool,
}

#[derive(Args, Debug)]
pub struct CutArgs {
    /// Input media file path
    pub input: PathBuf,

    /// Output destination file path (optional)
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Start time in seconds or HH:MM:SS
    #[arg(short, long)]
    pub start: Option<String>,

    /// End time in seconds or HH:MM:SS
    #[arg(short, long)]
    pub end: Option<String>,

    /// Cut duration in seconds
    #[arg(short, long)]
    pub duration: Option<f64>,

    /// Frame-accurate re-encode instead of keyframe stream copy
    #[arg(long)]
    pub accurate: bool,

    /// Preview commands without executing
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args, Debug)]
pub struct FitArgs {
    /// Input media file path
    pub input: PathBuf,

    /// Destination output path (optional)
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Target aspect ratio (e.g. '16:9', '9:16', '1:1', '4:3')
    #[arg(short = 'a', long)]
    pub target_aspect: Option<String>,

    /// Fit mode: 'pad', 'crop', or 'blur' (default: pad)
    #[arg(short = 'm', long, default_value = "pad")]
    pub mode: String,

    /// Enforce constant frame rate (e.g. 30 or 60)
    #[arg(long)]
    pub fps: Option<f64>,

    /// Rotate clockwise degrees (90, 180, 270)
    #[arg(long)]
    pub rotate: Option<u32>,

    /// Flip horizontally
    #[arg(long)]
    pub flip_h: bool,

    /// Flip vertically
    #[arg(long)]
    pub flip_v: bool,

    /// Preview commands without executing
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args, Debug)]
pub struct CropArgs {
    /// Input media file path
    pub input: PathBuf,

    /// Output destination file path (optional)
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Crop rectangle width in pixels
    #[arg(short = 'W', long)]
    pub width: u32,

    /// Crop rectangle height in pixels
    #[arg(short = 'H', long)]
    pub height: u32,

    /// Top-left X coordinate
    #[arg(short, long, default_value_t = 0)]
    pub x: u32,

    /// Top-left Y coordinate
    #[arg(short, long, default_value_t = 0)]
    pub y: u32,

    /// Constant Rate Factor (CRF, default: 18)
    #[arg(long)]
    pub crf: Option<u32>,

    /// Preview commands without executing
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args, Debug)]
pub struct CropdetectArgs {
    /// Input media file path
    pub input: PathBuf,

    /// Seconds to analyze from start (optional)
    #[arg(short, long)]
    pub limit: Option<f64>,

    /// Dimension multiple rounding (default: 16)
    #[arg(short, long, default_value_t = 16)]
    pub round: u32,
}

#[derive(Args, Debug)]
pub struct ExportArgs {
    /// Input media file path
    pub input: PathBuf,

    /// Destination output path (optional)
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Platform preset: youtube, youtube4k, reels, tiktok, shorts, linkedin, facebook, x, prores, h265, gif, copy
    #[arg(short, long, default_value = "youtube")]
    pub preset: String,

    /// Constant Rate Factor (CRF) override
    #[arg(long)]
    pub crf: Option<u32>,

    /// Hardware acceleration mode: auto, videotoolbox, nvenc, none
    #[arg(long, default_value = "auto")]
    pub hwaccel: String,

    /// Frame rate override
    #[arg(long)]
    pub fps: Option<f64>,

    /// Scaling dimension override (e.g. '1920:1080')
    #[arg(long)]
    pub scale: Option<String>,

    /// Preview commands without executing
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args, Debug)]
pub struct JoinArgs {
    /// Input media files to join in sequential order
    #[arg(required = true)]
    pub inputs: Vec<PathBuf>,

    /// Destination output path
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Transition style: none, fade, dissolve, wipeleft, wiperight, slideleft, slideright
    #[arg(short, long, default_value = "none")]
    pub transition: String,

    /// Transition duration in seconds (default: 1.0)
    #[arg(long, default_value_t = 1.0)]
    pub transition_duration: f64,

    /// Lossless concat demuxer copy when formats and codecs match
    #[arg(long)]
    pub stream_copy: bool,

    /// Preview commands without executing
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args, Debug)]
pub struct LoudnessArgs {
    /// Input media file path
    pub input: PathBuf,

    /// Destination output path (optional)
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Preset profile: podcast (-16 LUFS), youtube (-14 LUFS), broadcast (-23 LUFS)
    #[arg(short, long, default_value = "podcast")]
    pub preset: String,

    /// Target integrated loudness in LUFS (e.g. -16.0)
    #[arg(short = 'i', long)]
    pub target_i: Option<f64>,

    /// Target true peak maximum in dBTP (default: -1.0)
    #[arg(long)]
    pub target_tp: Option<f64>,

    /// Single-pass normalization instead of precision two-pass
    #[arg(long)]
    pub single_pass: bool,

    /// Preview commands without executing
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args, Debug)]
pub struct SilenceArgs {
    /// Input media file path
    pub input: PathBuf,

    /// Destination output path for jump cut video (optional)
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Noise floor threshold in dB (default: -30.0)
    #[arg(short, long, default_value_t = -30.0)]
    pub noise_db: f64,

    /// Minimum silence duration in seconds to trigger detection (default: 0.5)
    #[arg(short, long, default_value_t = 0.5)]
    pub min_duration: f64,

    /// Cut and remove silence to export jump-cut video
    #[arg(short, long)]
    pub remove: bool,

    /// Audio padding preserved around cuts in seconds (default: 0.1)
    #[arg(long, default_value_t = 0.1)]
    pub padding: f64,

    /// Preview commands without executing
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args, Debug)]
pub struct SpeedrampArgs {
    /// Input media file path
    pub input: PathBuf,

    /// Destination output path
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Speed multiplier factor (e.g. 0.5 for 2x slow-mo, 2.0 for 2x fast)
    #[arg(short, long)]
    pub speed: f64,

    /// Preview commands without executing
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args, Debug)]
pub struct ReverseArgs {
    /// Input media file path
    pub input: PathBuf,

    /// Destination output path
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Reverse video stream only
    #[arg(long)]
    pub video_only: bool,

    /// Reverse audio stream only
    #[arg(long)]
    pub audio_only: bool,

    /// Preview commands without executing
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args, Debug)]
pub struct WaveformArgs {
    /// Input media file path
    pub input: PathBuf,

    /// Destination output path (.png or .mp4)
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Visualization mode: waveform (PNG), spectrogram (PNG), video (MP4)
    #[arg(short, long, default_value = "waveform")]
    pub mode: String,

    /// Image/video width in pixels
    #[arg(short = 'W', long, default_value_t = 1280)]
    pub width: u32,

    /// Image/video height in pixels
    #[arg(short = 'H', long, default_value_t = 720)]
    pub height: u32,

    /// Waveform color (hex code or FFmpeg color name)
    #[arg(short, long, default_value = "#00FFAA")]
    pub color: String,

    /// Preview commands without executing
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args, Debug)]
pub struct ScenesArgs {
    /// Input media file path
    pub input: PathBuf,

    /// Scene cut threshold 0.0 to 1.0 (default: 0.3)
    #[arg(short, long, default_value_t = 0.3)]
    pub threshold: f64,

    /// Directory to extract scene thumbnail images into
    #[arg(short = 'd', long)]
    pub thumbnails_dir: Option<PathBuf>,

    /// Analyze only first N seconds
    #[arg(short, long)]
    pub limit: Option<f64>,

    /// Preview commands without executing
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args, Debug)]
pub struct CheckArgs {
    /// Input media file path
    pub input: PathBuf,

    /// Target platform profile: youtube, tiktok, reels, x, broadcast, general
    #[arg(short, long, default_value = "general")]
    pub platform: String,
}

#[derive(Args, Debug)]
pub struct ContractArgs {
    /// Output contract specification strictly formatted as JSON
    #[arg(long, default_value_t = true)]
    pub json: bool,
}
