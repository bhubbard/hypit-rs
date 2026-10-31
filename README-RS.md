# hypit-rs: Pure Rust Engine for Hypit

A high-performance Rust port and execution engine for [Hypit](https://github.com/hypit-ai/hypit).

`hypit-rs` parses Semantic Video Markup Language (**SVML**), compiles declarative scripts and visual cues into an execution dependency Directed Acyclic Graph (DAG), caches artifacts with cryptographic hashing (SHA-256), and synthesizes multi-layer video compositions using hardware-accelerated **FFmpeg** (Apple VideoToolbox / NVIDIA NVENC).

---

## ⚡ Key Features

1. **Native SVML Parser**:
   - Zero-copy / linear-time parsing of SVML syntax:
     - Header processing instructions: `<?svml using="@hypit/markup@1"?>`
     - Script segments and speaker turns (`<BOY>`, `<WIFE>`, `<HOST>`)
     - Cues (`@{cue-name}`), speech pauses (`||`), phonetic overrides (`<2012 | twenty twelve>`)
     - Component elements (`<audio:...>`, `<visual:...>`, `<film:...>`, `<text:Value>`)
     - Entity decoding and source range diagnostics
2. **DAG Compiler & Execution Planner**:
   - Translates SVML documents into execution graphs (`ExecutionPlan`).
   - Automatically schedules TTS synthesis, speech alignment (WhisperX), visual B-roll cues, kinetic captions, audio mastering, and final FFmpeg video encoding.
3. **Artifact Caching Engine (Kernel)**:
   - Content-addressed artifact hashing (`SHA-256(type, params, deps)`).
   - Skips re-computation on cache hits with sub-millisecond turnarounds.
4. **Hardware-Accelerated Media Runtime**:
   - Autodetects `ffmpeg`, `ffprobe`, and hardware encoders (`h264_videotoolbox` on macOS Apple Silicon, `h264_nvenc` on Linux/Windows).
   - Assembles 9:16 vertical video compositions directly for TikTok, Reels, and YouTube Shorts.

---

## 🚀 Quick Start

### Build

```bash
cargo build --release
```

The compiled binary will be located at `target/release/hypit`.

### Commands

#### 1. System & Runtime Information
```bash
hypit info
```

Displays detected FFmpeg binary, version, and hardware acceleration support:
```
⚡ Hypit-rs Engine Info
  • Version: 0.1.0
  • Architecture: Pure Rust 2021 Edition

🎥 Media Processing Runtime:
  • FFmpeg: /opt/homebrew/bin/ffmpeg (version 9.0.2)

🚀 Hardware Acceleration:
  • Apple VideoToolbox (macOS): Available ✓
```

#### 2. Validate SVML Syntax
```bash
hypit check examples/interview/reference.svml
```

```
✓ SVML Syntax Valid!
  • Using: @hypit/markup@1
  • Imports: 25 package(s)
  • Sections: 3 script section(s)
  • Dialog Lines: 16
  • Speakers: BOY, WIFE
  • Text Prompts: 14
```

#### 3. Inspect DAG Execution Plan
```bash
hypit plan examples/interview/reference.svml
```

Outputs the task graph, estimated speaking duration, and step dependencies. Pass `--json` for machine-readable output:
```bash
hypit plan examples/interview/reference.svml --json
```

#### 4. Run Video Generation Pipeline
```bash
# Dry run (process tasks & verify cache without encoding)
hypit run examples/interview/reference.svml --dry-run

# Full render to output.mp4
hypit run examples/interview/reference.svml -o output.mp4
```

#### 5. Parse AST to JSON
```bash
hypit parse examples/ranking-football/reference.svml
```

---

## 🧪 Testing

Run the full integration test suite:

```bash
cargo test
```
