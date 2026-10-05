# ffmpeg-skill-rs (`fms`)

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust: 1.80+](https://img.shields.io/badge/Rust-1.80%2B-orange.svg)](https://www.rust-lang.org)

High-performance native Rust fork and complete port of [`kajisho5/ffmpeg-skill`](https://github.com/kajisho5/ffmpeg-skill). 

`ffmpeg-skill-rs` turns FFmpeg into a reliable, machine-readable tool system for autonomous AI agents and video creators. It compiles into a single static native binary (`ffmpeg-skill` with alias `fms`), eliminating Python/Node runtime overhead, and comes equipped with a built-in **Model Context Protocol (MCP)** JSON-RPC stdio server.

---

## Why Rust for FFmpeg Agent Skills?

1. **Zero Runtime Dependencies:** No virtual environments, pip packages, or Python version mismatches. Download and run.
2. **Deterministic Contract:** Emits and enforces strict JSON execution guarantees (`contract_version: "1.0"`).
3. **Sub-millisecond Agent Calls:** Instant tool dispatching without interpreter cold-starts.
4. **Hardware Acceleration First:** Autodetects Apple Silicon VideoToolbox (`h264_videotoolbox`, `hevc_videotoolbox`, `prores_videotoolbox`), Nvidia NVENC, and Linux VAAPI.
5. **Native MCP Server:** Connects directly into Claude Desktop, Cursor, Antigravity, Open WebUI, and other AI pair programmers via `fms mcp`.

---

## Installation

### From Source

```bash
git clone https://github.com/bhubbard/ffmpeg-skill-rs.git
cd ffmpeg-skill-rs
cargo build --release
sudo cp target/release/ffmpeg-skill target/release/fms /usr/local/bin/
```

### Pre-requisites
- `ffmpeg` and `ffprobe` installed on system `$PATH` (e.g. `brew install ffmpeg`).

---

## Command Matrix

| Command | Functionality | Key Flags |
| :--- | :--- | :--- |
| `fms doctor` | Inspects FFmpeg version, encoders, filters & GPU hardware acceleration engines | `--json` |
| `fms probe` | Deep inspection of media streams, duration, codecs, HDR, VFR | `--field`, `--compact`, `--json` |
| `fms cut` | Lossless keyframe trim or frame-accurate re-encode | `--start`, `--end`, `--duration`, `--accurate` |
| `fms fit` | Conforms aspect ratio (pad, crop, blur), rotation & constant frame rate | `-a 9:16`, `-m blur`, `--fps 60`, `--rotate 90` |
| `fms crop` | Crops exact rectangular bounding box | `-W`, `-H`, `-x`, `-y`, `--crf` |
| `fms cropdetect`| Analyzes video stream to detect black bars, letterbox or pillarbox | `--limit 30`, `--round 16` |
| `fms export` | Optimized delivery presets (YouTube, Reels, TikTok, ProRes, GIF) | `--preset reels`, `--hwaccel auto` |
| `fms join` | Concatenates media with crossfade transitions | `--transition dissolve`, `--transition-duration 1.0` |
| `fms loudness` | EBU R128 two-pass precision loudness normalization | `--preset podcast`, `--target-i -16.0` |
| `fms silence` | Detects dead air silence gaps and generates tight jump cuts | `--noise-db -30`, `--min-duration 0.5`, `--remove` |
| `fms speedramp`| Adjusts playback speed with smooth pitch-preserving atempo chains | `--speed 1.5` |
| `fms reverse` | Reverses video and/or audio backwards in time | `--video-only`, `--audio-only` |
| `fms waveform` | Renders visual audio waveform/spectrogram PNGs or animated video | `--mode waveform`, `-W 1280`, `-H 720` |
| `fms scenes` | Detects scene cuts and extracts keyframe thumbnails | `--threshold 0.3`, `-d ./thumbnails` |
| `fms check` | Validates pre-delivery spec compliance against platform matrices | `--platform youtube\|tiktok\|reels\|x\|broadcast` |
| `fms contract` | Emits machine-readable contract specification JSON | `--json` |
| `fms mcp` | Starts Model Context Protocol stdio JSON-RPC 2.0 server | Stdio transport |

---

## Model Context Protocol (MCP) Setup

To connect `ffmpeg-skill-rs` to your AI assistant:

### Claude Desktop (`claude_desktop_config.json`)
```json
{
  "mcpServers": {
    "ffmpeg-skill": {
      "command": "/usr/local/bin/fms",
      "args": ["mcp"]
    }
  }
}
```

### Cursor / Antigravity / Open WebUI
Configure standard I/O command:
- Command: `fms`
- Arguments: `["mcp"]`

---

## Machine-Readable Contract

Agents can verify tool capabilities and execution invariants at runtime:

```bash
fms contract --json
```

Output:
```json
{
  "contract_version": "1.0",
  "skill_name": "ffmpeg-skill-rs",
  "engine": "native-rust",
  "binaries": {
    "required": ["ffmpeg", "ffprobe"],
    "aliases": ["ffmpeg-skill", "fms"]
  },
  "tools": [...]
}
```

---

## License

MIT License. Copyright (c) 2026 Brandon Hubbard.
Based on and forked from [`kajisho5/ffmpeg-skill`](https://github.com/kajisho5/ffmpeg-skill).
