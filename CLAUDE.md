# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

TinyCapture is a Windows-only lightweight screen recording to GIF tool written in Rust. The project is currently in specification phase with no implementation yet.

**All responses and operations must be in Chinese. All commands must use cmd.exe (not PowerShell or WSL).**

## Intended Technology Stack

- **Language:** Rust
- **Platform:** Windows 10+ only
- **Screen Capture:** Windows Graphics Capture (WGC) + Direct3D 11
- **UI:** Win32 API (native Windows)
- **GIF Export:** gifski crate
- **File Dialogs:** rfd crate
- **Threading:** Multi-threaded (UI thread, capture worker, export worker)

## Planned Project Structure

```
Cargo.toml (workspace)
├── crates/
│   ├── app/          # Main panel UI + system tray + state machine
│   ├── overlay/      # Frozen screenshot + selection overlay + info bar
│   ├── capture_wgc/  # WGC capture + framepool + resize + crop
│   └── export/       # gifski export, PNG export, optional ffmpeg
└── assets/icon.ico
```

## Build Commands (cmd.exe)

```cmd
cargo build --release
cargo run --release
cargo test
```

## Architecture Notes

### State Machine
`Idle → Selecting → Recording → Recorded → Exporting`

### Core Workflow
1. User clicks Record → main panel hides
2. Full virtual desktop screenshot captured (frozen effect)
3. Overlay window displays screenshot with selection UI
4. User drags region or clicks to select window
5. Overlay closes (with ~100ms delay), recording starts
6. Main panel shows again
7. User clicks Stop → enters Recorded state
8. User clicks Export → GIF/PNG/MP4 export via background worker

### Key Implementation Requirements
- **Selection Overlay:** Must use Z-order window enumeration for window selection (not WindowFromPoint, since overlay covers screen)
- **DPI:** Per-monitor DPI aware v2 (SetProcessDpiAwarenessContext), all rects in physical pixels
- **Multi-monitor:** Overlay covers virtual desktop; region capture uses monitor containing center point
- **Recording:** Frames saved as PNG sequence to %TEMP%, converted to GIF on export
- **Frame Size Changes:** Handle ContentSize changes via FramePool.Recreate()

### Threading Model
- UI thread: interaction and state display only
- Capture worker: receives StartCapture commands, writes PNG frames via channel
- Export worker: gifski composition in background, notifies UI on completion
