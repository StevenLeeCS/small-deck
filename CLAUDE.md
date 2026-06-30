# XPAD Small Deck

A Tauri v2 desktop app that turns the **F13–F24** keys configured on an XPAD
device into a software launcher — the same idea as Stream Deck's "Open" action,
but fully self-contained.

## Tech stack

| Layer | Technology |
|-------|-----------|
| Shell | Tauri v2 (Rust backend) |
| Frontend | Vanilla JS + CSS + HTML (no framework, no bundler) |
| Packaging | `npx tauri build` → NSIS (.exe), MSI, DMG, AppImage |
| USB | `rusb` (libusb) via vendor interface — **read-only** |
| Hotkeys | `tauri-plugin-global-shortcut` (F13–F24) |
| Persistence | JSON file in per-user app config dir (`mappings.json`) |
| I18N | Lightweight ad-hoc engine — EN / 中文, stored in `localStorage` |

## Project layout

```
small-deck/
├── src/                    # Frontend (served directly, no build step)
│   ├── index.html           # Single-page UI structure
│   ├── app.js               # All UI logic: rendering, bindings, modal, Tauri bridge
│   ├── i18n.js              # Bilingual EN/中文 engine (window.t / window.setLang)
│   └── style.css            # All styles; CSS variables define the XPAD palette
├── src-tauri/
│   ├── src/
│   │   ├── main.rs          # Entry point, #[windows_subsystem] in release
│   │   ├── lib.rs           # App setup: tray, hotkeys, commands, plugins
│   │   ├── launcher.rs      # Cross-platform launcher (open/cmd start/xdg-open)
│   │   ├── xpad.rs          # USB device communication (read-only)
│   │   ├── store.rs         # JSON persistence (BTreeMap<F-key, Entry>)
│   │   └── icons.rs         # macOS-only: .icns → PNG data URL extraction
│   ├── capabilities/        # Tauri v2 permission grants
│   ├── tauri.conf.json      # Window config, CSP, bundle settings
│   ├── Cargo.toml           # Rust dependencies + release profile (size-opt)
│   └── build.rs             # tauri_build::build()
├── package.json             # Only devDep: @tauri-apps/cli
└── .github/workflows/       # CI: 3-platform build + GitHub Release
```

## How to run

```bash
npm install                # installs @tauri-apps/cli
npm run dev                # tauri dev — live-reload build
npm run build              # tauri build — production installers
npm run tauri icon <png>   # regenerate app icons
```

Prerequisites: **Rust** (stable, MSVC on Windows), **Node.js** (≥18),
platform deps per <https://tauri.app/start/prerequisites/>.

## Architecture notes

### Window & title bar

- `tauri.conf.json`: `decorations: false` + `titleBarStyle: Overlay` + `hiddenTitle: true`
- The `.titlebar` div in `index.html` provides a custom draggable strip
- **macOS**: traffic lights float over the transparent titlebar; custom controls hidden
- **Windows/Linux**: custom minimise / maximise / close buttons (`.win-ctrl`)
- Close hides to tray (does not quit) so hotkeys stay alive
- Login-time autostart passes `--hidden` → window starts invisible in the tray
- `show_main()` sets macOS `ActivationPolicy` to Regular on show, Accessory on hide

### Global hotkeys

- All 12 F-keys are registered on startup; unsupported ones log a warning but don't crash
- macOS F21–F24 have no scancode → `SupportedKeys` state tells the UI to grey them out
- Hotkey handler identifies the pressed key by value-comparing against the registered `Shortcut`

### USB (read-only)

- VID `0x1209` / PID `0x0001` — pid.codes test VID, must match XPAD firmware
- Vendor interface (IFACE=1), endpoints EP_OUT=0x02, EP_IN=0x82
- Commands: `0x32 READ_LAYOUT`, `0x33 READ_KEYS` — **no write capability**
- `with_handle()` ensures the interface is always released (RAII pattern)

### Data flow

1. User clicks **Read XPAD** → `invoke('read_device_matrix')` → Rust calls USB → JSON → UI renders matrix
2. User binds a key → `invoke('set_mapping', {key, path, name, kind})` → Store persists to `mappings.json`
3. F-key pressed → global-shortcut handler → looks up `Store` → `launcher::run_entry()` → OS shell

### State management (frontend)

- `mappings` (object: `{ 'F13': {path, name, kind}, ... }`) — source of truth
- `deviceKeys` / `deviceMatrix` — from USB, drives visibility in matrix mode
- `modalKey` — which F-key is open in the inspector panel
- `iconCache` — path → data URL, populated lazily
- `supportedKeys` — Set of labels that registered as global hotkeys on this OS
- `deckMode` — `'matrix'` (grid view, needs device) or `'list'` (flat, always works)

### i18n pattern

- Dictionary in `i18n.js` as `{ key: { en, zh } }`
- `t(key, vars)` for JS; `data-i18n="key"` attributes in HTML
- Language stored in `localStorage('xpad_lang')`, defaults to `'en'`
- `window.setLang('en'|'zh')` applies to all `[data-i18n]` elements and re-renders
- Tray menu items are relabelled via `invoke('set_tray_labels', {show, quit})`

### CSP & event handling

- CSP in `tauri.conf.json`: `script-src 'self' 'unsafe-inline'` (nonce injected by Tauri)
- All click handlers delegated via `[data-act]` attributes → `window[fnName]()` pattern
- No inline `onclick=""` in HTML — avoids CSP block after Tauri injects a nonce

## Code conventions

- **CSS**: variables in `:root` for the XPAD palette; platform selectors via `os-mac` / `os-win` / `os-linux` classes set on `<html>` by a UA sniff in `app.js`
- **Rust commands**: `#[tauri::command]` returning `Result<T, String>`; all use `State<Store>`/`State<SupportedKeys>` for shared data
- **Error messages**: user-facing, not raw debug strings
- **Comments**: English throughout; focused on "why" not "what"
- **Release profile**: `panic="abort"`, `lto=true`, `opt-level="s"`, `strip=true` — optimised for binary size
