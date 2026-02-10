# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

WinGIF 是一个 Windows 专用的轻量级屏幕录制转 GIF 工具，使用 Rust 编写。项目已完成核心功能实现。

**所有响应和操作必须使用中文。所有命令必须使用 cmd.exe（不使用 PowerShell 或 WSL）。**

## Technology Stack

- **Language:** Rust (edition 2021)
- **Platform:** Windows 10 1903+ (64-bit)
- **Screen Capture:** Windows Graphics Capture (WGC) + Direct3D 11
- **UI:** egui / eframe
- **GIF Export:** gifski crate
- **File Dialogs:** rfd crate
- **Threading:** crossbeam-channel, parking_lot

## Build Commands (cmd.exe)

```cmd
:: 构建 Release 版本
cargo build --release

:: 运行程序
cargo run --release

:: 运行测试
cargo test

:: 构建特定 crate
cargo build -p app --release
cargo build -p overlay --release
cargo build -p capture_wgc --release
cargo build -p export --release
```

可执行文件名称：`wingif.exe`（位于 `target/release/`）

## Project Structure

这是一个 Cargo workspace，包含 4 个 crate：

```
crates/
├── app/           # 主程序：UI、状态机、系统托盘、事件循环
├── overlay/       # 冻结截图 + 选区覆盖层 + 窗口枚举
├── capture_wgc/   # WGC 屏幕捕获 + D3D11 + 帧处理
└── export/        # GIF/PNG 导出（使用 gifski）
```

### Key Modules

**app/src/main_egui.rs**
- 入口点，初始化 DPI awareness（Per-Monitor V2）
- 创建 EguiUiState，启动 capture worker 和 result handler 线程
- 通过 crossbeam-channel 进行线程间通信
- 运行 eframe/egui 主循环，处理 Record/Stop/Export 回调

**app/src/state.rs**
- 状态机定义：`Idle → Selecting → Recording → Recorded → Exporting`
- RecordingSession 数据结构（target, region, temp_dir, frame_count, fps）
- RecordingTarget 枚举（Monitor 或 Window）

**app/src/ui_egui.rs**
- WinGIFApp：eframe::App 实现，egui 界面
- EguiUiState：共享状态（通过 Arc<Mutex<>>）
- 按钮与状态展示（Record, Stop, Export）

**overlay/src/window.rs**
- OverlayWindow::show() 创建全屏覆盖窗口
- 显示冻结截图 + 半透明遮罩
- 处理鼠标拖动选区或点击选择窗口

**overlay/src/selection.rs**
- 区域选择逻辑（拖动矩形框）
- 窗口选择逻辑（通过 Z-order 枚举，因为 overlay 覆盖了屏幕）

**overlay/src/screenshot.rs**
- 使用 GDI+ 捕获虚拟桌面截图（冻结效果）

**overlay/src/outline.rs**
- 录制时显示边框指示录制区域

**capture_wgc/src/capture.rs**
- CaptureController：启动/停止 WGC 捕获
- CaptureTarget 枚举（Monitor 或 Window）
- try_get_frame() 获取最新帧

**capture_wgc/src/d3d11.rs**
- D3D11Device：创建 D3D11 设备和 Direct3D 互操作

**capture_wgc/src/frame.rs**
- FrameProcessor：接收帧，裁剪（如果需要），保存为 PNG
- 帧保存到 %TEMP%\wingif_<uuid>\ 目录

**export/src/gif.rs**
- GifExporter::export_from_pngs() 使用 gifski 将 PNG 序列转换为 GIF

**export/src/png.rs**
- PngExporter（如果需要导出 PNG 序列）

## Architecture Notes

### State Machine
`Idle → Selecting → Recording → Recorded → Exporting`

状态转换在 `app/src/state.rs` 的 `StateMachine` 中定义，每个状态控制哪些按钮可用。

### Threading Model

1. **UI Thread（主线程）**
   - eframe/egui 事件循环
   - 处理按钮点击与 UI 更新

2. **Capture Worker Thread**
   - 在 `main_egui.rs:capture_worker()` 中运行
   - 接收 CaptureCommand（Start, Stop, Shutdown）
   - 调用 CaptureController 和 FrameProcessor
   - 发送 CaptureResult（Started, Progress, Stopped, Error）

3. **Result Handler Thread**
   - 接收 capture worker 的结果
   - 更新 UI 状态（通过 Arc<Mutex<EguiUiState>>）
   - egui 每帧读取状态并重绘

4. **Export Worker Thread**
   - 在 `main_egui.rs:on_export_click()` 中 spawn
   - 后台运行 gifski 编码
   - 完成后更新 UI

### Core Workflow

1. **Record 按钮** → 隐藏主窗口 → 显示 OverlayWindow
2. **Overlay** → 用户拖动选区或点击窗口 → 返回 SelectionOutcome
3. **Start Capture** → 发送 CaptureCommand::Start 到 worker
4. **Worker** → CaptureController + FrameProcessor 保存 PNG 帧到 %TEMP%
5. **Stop 按钮** → 发送 CaptureCommand::Stop
6. **Export 按钮** → 弹出文件保存对话框 → spawn export thread → gifski 编码

### Key Implementation Details

- **DPI Awareness:** 使用 `SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)`，所有 Rect 使用物理像素
- **Multi-Monitor:** Overlay 覆盖整个虚拟桌面；区域捕获使用包含选区中心点的显示器
- **Window Selection:** 通过 Z-order 枚举窗口（不能用 WindowFromPoint，因为 overlay 覆盖了屏幕）
- **Frame Storage:** 录制时帧保存为 PNG 序列到 `%TEMP%\wingif_<uuid>\frame_00000.png`
- **Crop Rect:** 区域捕获时，将虚拟桌面坐标转换为显示器相对坐标作为裁剪矩形
- **WinRT Initialization:** Capture worker 线程必须调用 `RoInitialize(RO_INIT_MULTITHREADED)`

## Common Patterns

### 跨线程通信
使用 crossbeam-channel 的 bounded channel 在主线程和 worker 之间通信：
```rust
let (cmd_tx, cmd_rx) = bounded(4);
let (result_tx, result_rx) = bounded(4);
```

### 共享状态
使用 `Arc<Mutex<UiState>>` 在多个回调和线程间共享状态。

### HWND 跨线程传递
HWND 不是 Send，通过 `isize` (raw pointer) 传递：
```rust
let hwnd_raw = hwnd.0 as isize;
// 在线程中重建
let hwnd = HWND(hwnd_raw as *mut std::ffi::c_void);
```

### UI 更新
使用 `post_update_state(hwnd)` 发送自定义消息触发 UI 重绘，避免在非 UI 线程直接操作窗口。

## Development Notes

- Windows API 错误处理：大多数 Win32 函数返回 `Result<(), windows::core::Error>`
- WGC 需要 Windows 10 1903+ 并且支持 Graphics Capture API
- Release 构建启用了 LTO 和 strip 以减小二进制大小
- 项目使用 `#![windows_subsystem = "windows"]` 隐藏控制台窗口
- build.rs 使用 embed-resource 嵌入图标资源
