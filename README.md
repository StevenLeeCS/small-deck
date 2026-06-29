# XPAD Small Deck (host agent)

A tiny background tray app that turns the **F13–F24** keys you assigned on the
**Small Deck** tab of the XPAD web configurator into software launchers — the
same idea as Stream Deck's "Open" action, but self-contained (no AutoHotkey /
Keyboard Maestro needed).

## How it works

1. The XPAD sends a normal HID keyboard key (F13–F24). These keys do nothing on
   any OS by default, so they make clean, conflict-free triggers.
2. This agent registers F13–F24 as **global hotkeys**. When one fires, it
   launches the program/file/URL you bound to it — regardless of which app is
   focused. The device does **not** need to be connected for launching to work.
3. On startup (and on **Read XPAD**) the agent reads the device over USB
   (read-only, command `0x33 READ_KEYS`) to discover *which* F13–F24 keys you
   actually mapped, so the UI only shows those rows. Use **Show all F13–F24** to
   configure without the device plugged in.

Bindings are stored as JSON in the per-user app config dir
(`…/com.xtia.xpad.smalldeck/mappings.json`), keyed by `F13`…`F24`.

## Prerequisites

- **Rust** (stable) + Cargo — https://rustup.rs
- **Node.js** (for the Tauri CLI)
- Platform deps for Tauri v2: see https://tauri.app/start/prerequisites/
  - macOS: Xcode Command Line Tools
  - Windows: WebView2 (preinstalled on Win11) + MSVC build tools
  - Linux: `libwebkit2gtk-4.1`, `libusb-1.0-0-dev`, `pkg-config`, etc.

## First-time setup

```bash
npm install                       # installs @tauri-apps/cli

# Generate the app/tray icons from the bundled source image (one time).
# Replace icon-source.png with real branding when ready.
npm run tauri icon src-tauri/icons/icon-source.png
```

## Run / build

```bash
npm run dev      # tauri dev — live-reload development build
npm run build    # tauri build — produces an installer in src-tauri/target/release/bundle
```

## Notes & limits

- **Launching only.** This agent launches software; it does not deep-control
  apps (OBS/Spotify scenes etc.) and shows no per-key LCD — XPAD has no key
  screens. Haptic feedback and bidirectional control are intentionally out of
  scope for this version.
- **macOS & F21–F24**: macOS has no global-hotkey scancode for F21–F24, so those
  four cannot trigger launches there (F13–F20 work). The agent detects which keys
  registered and greys out the unusable ones. Windows supports all twelve.
- **USB access**: on Windows the device binds WinUSB automatically (via the
  firmware's MS OS 2.0 descriptors), so `rusb`/libusb works. On Linux you may
  need a udev rule granting access to VID `1209` / PID `0001`.
- VID/PID and the vendor endpoints are pinned in `src-tauri/src/xpad.rs` and
  must match the firmware (`src/usb/usb_descriptors.c`).
