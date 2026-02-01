# TinyCapture

轻量级 Windows 屏幕录制转 GIF 工具。

## 系统要求

- Windows 10 1903 (May 2019 Update) 或更高版本
- 64 位操作系统
- 支持 Windows Graphics Capture API

## 构建步骤

使用 **cmd.exe** 执行以下命令：

```cmd
:: 进入项目目录
cd /d E:\codebase\TinyCapture

:: 构建 Release 版本
cargo build --release

:: 运行程序
cargo run --release

:: 运行测试
cargo test
```

## 使用说明

### 基本操作

1. **开始录制**
   - 点击主面板的「录制」按钮
   - 主面板会自动隐藏，屏幕变为冻结截图模式

2. **选择区域**
   - **框选区域**：按住鼠标左键拖动选择录制区域
   - **选择窗口**：单击目标窗口
   - 按 **Enter** 确认选区并开始录制
   - 按 **Esc** 取消

3. **停止录制**
   - 点击「停止」按钮结束录制

4. **导出 GIF**
   - 点击「导出 GIF」按钮
   - 选择保存路径
   - 等待导出完成

### 系统托盘

- 关闭主窗口会最小化到系统托盘
- 双击托盘图标显示主窗口
- 右键托盘图标显示菜单

## 项目结构

```
TinyCapture/
├── Cargo.toml                    # Workspace 配置
├── README.md                     # 本文件
├── assets/
│   └── icon.ico                  # 应用图标
└── crates/
    ├── app/                      # 主程序
    │   └── src/
    │       ├── main.rs           # 入口点
    │       ├── ui.rs             # 主面板 UI
    │       ├── tray.rs           # 系统托盘
    │       └── state.rs          # 状态机
    ├── overlay/                  # 选区覆盖层
    │   └── src/
    │       ├── lib.rs            # 模块导出
    │       ├── window.rs         # 覆盖窗口
    │       ├── screenshot.rs     # 截图
    │       ├── selection.rs      # 选区逻辑
    │       └── render.rs         # 渲染
    ├── capture_wgc/              # WGC 屏幕捕获
    │   └── src/
    │       ├── lib.rs            # 模块导出
    │       ├── capture.rs        # 捕获核心
    │       ├── d3d11.rs          # D3D11 设备
    │       └── frame.rs          # 帧处理
    └── export/                   # 导出模块
        └── src/
            ├── lib.rs            # 模块导出
            ├── gif.rs            # GIF 导出
            └── png.rs            # PNG 导出
```

## 技术特性

- **DPI 感知**：Per-Monitor DPI V2 支持
- **多显示器**：支持虚拟桌面跨多显示器
- **窗口捕获**：使用 WGC API 高效捕获
- **GIF 导出**：使用 gifski 高质量编码
- **多线程**：UI 线程与捕获/导出线程分离

## 许可证

MIT License
