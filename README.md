# Lumen

Desktop time tracking utility for Windows. Tracks foreground app usage via `SetWinEventHook` with minimal CPU/RAM overhead.

## Features

- Tracks active window changes in real time (WinEvent hook, no polling)
- Detects idle time, fullscreen games
- Per-app duration with proportional accent bars
- Compact overlay window with custom titlebar (no system chrome)
- System tray integration — closes to tray, runs in background
- Local SQLite storage

## Stack

- **UI**: `winit` + `tiny-skia` + `softbuffer` — fully custom renderer
- **Text**: `fontdue` — manual glyph rasterization
- **Storage**: `rusqlite` with chrono integration
- **Tracking**: Windows `SetWinEventHook` via `windows-rs`

## Build

```sh
cargo build --release
```

Requires Windows (Win32 API dependencies).

## Usage

Run `lumen.exe`. The overlay window appears; close it to hide to tray. Right-click the tray icon for menu.
