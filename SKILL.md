---
name: ffmpeg-skill-rs
description: High-performance native Rust fork of ffmpeg-skill. Deterministic FFmpeg media manipulation toolkit and Model Context Protocol (MCP) server for AI pair programmers and creators.
license: MIT
version: 0.1.0
compatibility:
  os: [macos, linux, windows]
  dependencies: [ffmpeg, ffprobe]
contract_version: "1.0"
tools:
  - doctor
  - probe
  - cut
  - fit
  - crop
  - cropdetect
  - export
  - join
  - loudness
  - silence
  - speedramp
  - reverse
  - waveform
  - scenes
  - check
  - contract
---

# `ffmpeg-skill-rs` (fms)

`ffmpeg-skill-rs` is a high-performance native Rust port and fork of `kajisho5/ffmpeg-skill`. It replaces brittle shell/Python wrapper scripts with a single standalone statically-linkable native binary offering strict type safety, sub-millisecond invocation latency, and a built-in Model Context Protocol (MCP) JSON-RPC 2.0 stdio server.

## Highlights

- **Zero-Runtime Overhead:** Single compiled native Rust binary (`ffmpeg-skill` / alias `fms`). No Python, Node.js, or pip environments required.
- **Built-in MCP Server:** Instant integration with Claude Desktop, Cursor, Antigravity, Open WebUI, and other AI agents (`fms mcp`).
- **Deterministic Machine-Readable Contract:** Emits `contract_version: "1.0"` JSON specifications for reliable tool execution by autonomous coding agents.
- **Hardware-Accelerated Presets:** Automatically leverages Apple Silicon VideoToolbox (`h264_videotoolbox`, `hevc_videotoolbox`, `prores_videotoolbox`), Nvidia NVENC, and VAAPI.
- **Smart Jump-Cutting & Two-Pass EBU R128:** Seamless dead-air removal and broadcast-grade audio loudness normalization.

## CLI Usage

```bash
# Capabilities and hardware acceleration inspection
fms doctor

# Deep media probing with dotted field selectors
fms probe input.mp4 --compact
fms probe input.mp4 --field "duration"
fms probe input.mp4 --field "video[0].codec"

# Lossless or frame-accurate trimming
fms cut input.mp4 --start 00:01:30 --duration 15.5
fms cut input.mp4 --start 10.0 --end 25.0 --accurate -o output.mp4

# Aspect ratio conforming (pad, crop, blur)
fms fit input.mp4 -a 9:16 -m pad --fps 60

# Exact rectangular bounding box crop
fms crop input.mp4 -W 1080 -H 1080 -x 420 -y 0

# Black bar / letterbox detection
fms cropdetect movie.mp4 --limit 30 --json

# Platform delivery presets (YouTube, Reels, TikTok, ProRes, GIF)
fms export input.mov --preset reels
fms export input.mov --preset youtube4k
fms export clip.mp4 --preset gif --fps 15

# Concatenate with crossfade transitions
fms join clip1.mp4 clip2.mp4 clip3.mp4 --transition dissolve --transition-duration 1.0

# EBU R128 two-pass loudness normalization
fms loudness podcast.wav --preset podcast -o podcast_master.wav

# Jump-cut dead air removal
fms silence raw_take.mp4 --noise-db -28 --min-duration 0.4 --remove

# Speed manipulation with atempo pitch preservation
fms speedramp video.mp4 --speed 1.5

# Visual waveform and spectrogram rendering
fms waveform audio.mp3 --mode waveform -W 1920 -H 400 -o waveform.png

# Scene cut detection and thumbnail extraction
fms scenes video.mp4 --threshold 0.35 -d ./thumbnails

# Pre-delivery specification compliance validator
fms check final_cut.mp4 --platform reels --json

# Emit machine-readable contract JSON
fms contract --json
```

## Model Context Protocol (MCP) Configuration

Add `ffmpeg-skill` to your MCP client config (e.g. `claude_desktop_config.json`, `gemini`, or `cursor`):

```json
{
  "mcpServers": {
    "ffmpeg-skill": {
      "command": "ffmpeg-skill",
      "args": ["mcp"]
    }
  }
}
```
