# WinGIF

WinGIF is a lightweight Windows screen recorder that captures a selected region or window and exports to GIF (with PNG frames). It uses an egui-based UI and focuses on speed and minimal distraction.

**Features**
- Drag-to-select region capture or click-to-select window capture
- PNG frame capture with GIF export
- Recording status and frame counter
- egui-based UI

**System Requirements**
- Windows 10 1903 (May 2019 Update) or later
- 64-bit system
- Windows Graphics Capture API support

**Build and Run** (from repo root)
```cmd
cargo build --release
cargo run --release
cargo test
```

**Usage**
1. Click "Record".
2. Drag to select a region or click a window.
3. Click "Stop" to finish.
4. Export as GIF.

**Project Structure**
```
WinGIF/
├─ Cargo.toml
├─ README.md
├─ assets/
│  └─ icon.ico
└─ crates/
   ├─ app/          # UI, tray, app state
   ├─ overlay/      # selection overlay and rendering
   ├─ capture_wgc/  # WGC + D3D11 capture pipeline
   └─ export/       # GIF/PNG export
```

**License**
MIT License
