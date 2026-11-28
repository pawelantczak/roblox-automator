# Changelog

Technical changelog for **Roblox Automator** maintainers and contributors.  
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/) and [Semantic Versioning](https://semver.org/spec/v2.0.0.html).  
End-user release announcements are maintained in [RELEASE_NOTES.md](RELEASE_NOTES.md).

---

## [Unreleased]

### Added
- `README.md`: added the **Code Signing Policy** section acknowledging SignPath.io and SignPath Foundation for open-source code signing.

---

## [0.4.1] - 2026-11-28

### Fixed
- `WindowsMediaOcrService::scan_left_sidebar_high_contrast` (`src/ocr/mod.rs`): added a low-saturation filter (`lum >= 205 && max_rgb - min_rgb <= 55`) so bright colored game backgrounds (such as the light-blue spawn walkway) stay white paper instead of turning black alongside white UI captions.
- `IconTracer::trace_icon_bounds` & `Classifier::find_event_target` (`src/engine/tracer.rs`, `src/engine/classifier.rs`): added frame-relative minimum ROI dimensions (`roi_w`, `roi_h`) and a minimum `label_height` (`0.032`) fallback so tight single-line OCR text boxes (e.g. `8–9px` tall on `800×600` windows) still trace and click the event icon above the label instead of clicking just above/on the text.

---

## [0.4.0] - 2026-11-27

### Added
- `.github/workflows/build.yml`: added optional SignPath code-signing pipeline (`signpath/github-action-submit-signing-request@v3`, gated by `refs/tags/v*` and `vars.SIGNPATH_ENABLED == 'true'`) plus `Get-AuthenticodeSignature` verification before packaging `Roblox-Automator-v<ver>.zip`.

### Changed
- `.github/workflows/build.yml`: packaged `roblox-automator.exe` directly at the root of `Roblox-Automator-v<ver>.zip` instead of nesting it inside a `Roblox-Automator-v<ver>/` subfolder.

---

## [0.3.0] - 2026-11-27

### Added
- `ConfigLogSnapshot` & `LogEntry.config` (`src/models/state.rs`, `src/engine/mod.rs`): logs a sanitized configuration snapshot (redacting `vip_server_url` and summarizing `macro_steps` count/duration) to JSONL on startup and on every config-mutating action without cluttering the UI console.

### Fixed
- `Classifier::word_matches` (`src/engine/classifier.rs`): used `target.chars().count()` instead of UTF-8 byte length (`target.len()`) when selecting the Levenshtein edit-distance threshold, preventing 2-character multi-byte tokens (such as `"다시"` / `"연결"` in `"다시 연결"`) from receiving a `dist <= 2` budget and false-matching arbitrary 1–2 character in-game OCR fragments.

---

## [0.2.0] - 2026-11-27

### Added
- Added right-aligned Discord invite pill (`https://discord.gg/ce5yGvTf9`, `.footer-discord`) to `.app-footer` in `resources/index.html` & `resources/styles.css`, whitelisted for external browser launch in `src/ui/webview.rs`.
- `.github/ISSUE_TEMPLATE/bug_report.yml` & `feature_request.yml`: added GitHub Issue Form templates for bug reports and feature requests.
- `LogEntry.version` (`src/models/state.rs`): included `CARGO_PKG_VERSION` in every JSONL log record.
- Macro recording, playback & uniform window sizing:
  - `MacroStep` / `MouseBtn` (`src/models/macro_seq.rs`), `Config` persistence (`macro_enabled`, `macro_replay_on_reconnect`, `macro_replay_on_event`, `uniform_window_size`, `uniform_window_width` = `800`, `uniform_window_height` = `600`, `macro_steps`), `UiCommand` (`StartMacroRecording`, `StopMacroRecording`, `TestMacro`, `ClearMacro`, `SetMacroEnabled`), and `UIStateSnapshot` macro telemetry (`src/models/ipc.rs`, `src/models/state.rs`).
  - `src/input/recorder.rs`: Win32 `WH_KEYBOARD_LL` + `WH_MOUSE_LL` macro recorder with circular `3..2..1` click-through GDI countdown overlay and `"Move mouse outside window to stop"` hint pill (`WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE`), window focusing/cursor centering, normalized client coordinates, move throttling, and auto-stop when cursor leaves the target Roblox window.
  - `InputAutomation::play_macro` (`src/input/mod.rs`), async `TestMacro` / `StopMacroPlayback` playback (`src/models/ipc.rs`, `src/models/state.rs`, `src/engine/mod.rs`), `WindowDiscovery::resize_window_in_place` (`src/window/mod.rs`), and `EngineWorker` post-reconnect / post-event macro replay triggers (`src/engine/mod.rs`, `src/engine/scheduler.rs`), wired to `resources/app.js` (with `#macroClearBtn` switching to a red stop square `■` during playback).
- `UIStateSnapshot.app_version` (`src/models/state.rs`, `src/engine/mod.rs` via `CARGO_PKG_VERSION`), `.app-footer` (`resources/index.html`, `resources/styles.css`, `resources/app.js`), and `with_navigation_handler` browser launch for `https://ps99clan.xyz` (`src/ui/webview.rs`).

### Changed
- Replaced `resources/app_icon.png` (`256×256` PNG) with a simplified, full-bleed **Roblox Automator** icon (bold cyan 8-tooth gear around the tilted Roblox square, no white outer border, sized well within WebView2's `NavigateToString` 2 MB HTML limit).
- Highlighted all user-initiated UI commands (`SaveConfig`, `SetAntiAfkInterval`, `SetScanInterval`, `SetEventTracking`, `ExportLogsJsonl`, `ClearMacro`, `SetMacroEnabled`) with `LogLevel::Action` in `src/engine/mod.rs`.
- Renamed application/package from `roblox-watcher` (`Roblox Watcher`) to `roblox-automator` (`Roblox Automator`) across `Cargo.toml`, `src/main.rs` (`Global\RobloxAutomator_Mutex`), `src/models/config.rs`, `src/models/ipc.rs` (`StartAutomator` / `StopAutomator`), `src/engine/mod.rs`, `src/input/recorder.rs`, `resources/`, `README.md`, `GEMINI.md`, and `.github/workflows/build.yml`.
- Updated `.github/workflows/build.yml` to emit only `Roblox-Automator-v<version>.zip` containing `Roblox-Automator-v<version>/roblox-automator.exe` (unversioned `.exe` filename).
- Redesigned `.stats-ribbon` (`resources/index.html`, `resources/styles.css`, `resources/app.js`) into `Automator` (displaying live uptime timer in `#pulseLabel` when running, plus window/rejoin counts) and `Next Scan` with muted `2px` top borders, plus 3 tiered integration cards: `Anti-AFK` (`.tier-1`), `Smart Click` (`.tier-2`), and `Macro` (`.tier-3` UI controls `#macroRecordBtn`, `#macroWindowSelect`, green `#macroPlayBtn`, `#macroClearBtn`).
- Enlarged `.brand-title` (`Roblox Watcher` next to header icon) to `20px`.
- Increased UI typography scale across `resources/styles.css`, `resources/index.html`, and `resources/app.js` (header, buttons, `.stats-ribbon`, `.card` overlays/footers, `.logs-console`, and settings modal).

### Removed
- Removed `Application Title` (`#cfgAppTitle`) input from the Global Settings modal (`resources/index.html`, `resources/app.js`).

---

## [0.1.0] - 2026-11-27

### Added
- Core modular architecture: `src/window/` (`Win32WindowService`), `src/capture/` (`Win32CaptureService` via `PrintWindow` `PW_RENDERFULLCONTENT`), `src/ocr/` (`WindowsMediaOcrService`), `src/input/` (`Win32InputService` with `INPUT_LOCK` and `ForegroundAndCursorGuard`), `src/engine/` (`EngineController`, `EngineWorker`, `InstanceTracker`, `Classifier`, `IconTracer`), `src/power.rs` (`SetThreadExecutionState`), `src/storage.rs`, and `src/ui/` (`tao` + `wry` WebView2).
- Single-instance Windows Named Mutex (`Global\RobloxWatcher_Mutex`) in `src/main.rs`.
- GitHub Actions CI/CD workflow (`.github/workflows/build.yml`) and version bump scripts (`bump.ps1`, `bump.sh`).
- `README.md` (`docs/images/dashboard.png` screenshot path): added end-user documentation covering auto-reconnect, 20-minute Anti-AFK kick prevention, TinyTask compatibility, and quick-start usage without mixing assets into `resources/`.
- `InstanceCard.reconnect_crop_base64` (`src/models/state.rs`) and `InstanceTracker.reconnect_crop_base64` (`src/engine/scheduler.rs`): captures `40px` radius JPEG crops via `encode_point_crop_base64` around `btn.click_point()`, logs via `BoundedLogQueue::push_with_icon`, and renders `.card-thumb .reconnect-crop-overlay` (`44×44`, `--rose-500` border) in `resources/app.js` / `resources/styles.css`.
- `BoundedLogQueue::new_with_file` & `export_jsonl` (`src/models/state.rs`), `UiCommand::ExportLogsJsonl` / `UiEvent::LogsJsonlExported` (`src/models/ipc.rs`): persists structured JSONL logs (`timestamp`, `level`, `instanceId`, `message`, `iconBase64`) to `roblox-watcher.jsonl` and streams export to `#downloadLogsBtn` in `resources/app.js`.
- `InstanceCard.event_crop_base64` (`src/models/state.rs`) and `.card-thumb .event-crop-overlay` (`resources/styles.css`, `resources/app.js`): overlays a `44×44` crop centered on the traced event target point.
- Console scroll-lock in `resources/app.js` (`logsPaused`, `#resumeLogsBtn` `.btn-resume`) to pause DOM updates when scrolled away from bottom.
- Single-row 5-box `.stats-ribbon` (`resources/index.html`, `resources/styles.css`, `resources/app.js`) combining `Session` (`#sessionUptime`, `#activeWinsCount`, `#activeWinsUnit`), `Next Scan` inline stepper (`#scanUpBtn`/`#scanDownBtn`), `Anti-AFK Keep-Alive` inline stepper (`#afkUpBtn`/`#afkDownBtn`), and `Event Icon Text` toggle/input (`#ribbonEventEnabled`, `#ribbonEventKeywords`).
- Externalized `Config` schema v2 (`src/models/config.rs`) with `target_process_names`, `fallback_window_title`, `ui_strings`, `disconnect_keywords`, `reconnect_button_phrases`, `ignore_phrases`, `event_target_keywords`, validation, and `schema_version` migration in `src/storage.rs`.
- WebView2 runtime check (`src/ui/webview.rs`), offline bundled assets (`resources/index.html`, `resources/styles.css`, `resources/app.js`) with strict CSP, and `OcrProvider` multi-language reconnect phrase matching (`src/engine/classifier.rs`, `src/models/config.rs`).
- `Win32InputService` (`src/input/mod.rs`) hardware scancode `0x0039` spacebar dispatch with `PostMessage` dual-dispatch, 140ms hold / 150ms post-jump dwell, and foreground verification.

### Changed
- `EngineWorker::capture_idle_thumbnails` (`src/engine/mod.rs`): captures initial thumbnails on startup (`force_refresh = false`) and refreshes changed frames every `10s` while idle (`force_refresh = true`, skipping unchanged `frame_hash`).
- Balanced preview grid & fixed window geometry (`resources/app.js`, `resources/styles.css`, `src/ui/mod.rs`):
  - Dynamic `--grid-cols` (`1–4` → `N`, `5–6` or `9` → `3`, `7–8`/`10+` → `4`) and `data-rows` with row-aware `.card-thumb` `max-height` (`165px` / `120px` / `105px`).
  - Removed `.grid-scroll` `max-height` / `overflow-y: auto` so the preview grid stays visible at all times while `.logs-panel` (`min-height: 110px`) scrolls internally.
  - Locked `WindowBuilder` width to `1040.0` (`min_inner_size` `1040×520`, `max_inner_size` `1040×4096`).
- Config defaults (`src/models/config.rs`): `check_interval_seconds` = `15.0`, `ui_refresh_interval_ms` = `2000` (exposed in seconds in UI), removed PID/instanceId labels from preview cards and console output.
- Rebranded UI/assets (`Cargo.toml`, `resources/index.html`, `resources/app_icon.png`, `src/ui/mod.rs`) to `Roblox Watcher`, removed `Scan`, `Diagnostics`, `Target Place ID`, and `tray-icon` integration.

### Fixed
- `.card-error-line` overlay (`resources/app.js`, `resources/styles.css`): moved inside `.card-thumb` (`position: absolute; bottom: 0`) with `.card-thumb.has-error` shifting bottom overlays up to `24px`, keeping `.card` height invariant across states.
- `Classifier::find_reconnect_button` (`src/engine/classifier.rs`) & `OcrProvider::scan_region_upscaled` (`src/ocr/mod.rs`, `src/engine/mod.rs`):
  - Scores exact button-label matches and shorter lines above longer error body sentences, narrows `bounding_box` and `target_point` to the matched contiguous `word_boxes` span (preventing midpoint clicks on merged `"Cancel Retry"` / `"Opuść Połącz ponownie"` lines).
  - Runs a `1.5x–2.5x` `CatmullRom` upscaled OCR pass on `NormalizedRect(0.15, 0.15, 0.85, 0.90)` when `disconnect_match.is_some()` and 1x `scan_frame` misses the button.
- `Classifier::find_event_target` & `IconTracer` (`src/engine/classifier.rs`, `src/engine/tracer.rs`, `src/ocr/mod.rs`):
  - Isolated matched keyword `word_boxes`, added bottom-up row-gap isolation in `IconTracer`, capped sidebar upscale at `2200px`, added luminance thresholding (`lum >= 180`) in `scan_left_sidebar_high_contrast`, and added fuzzy sliding-window matching (`edit_distance <= 2`).
- Coordinate mapping in `src/engine/mod.rs` and `Win32CaptureService` (`src/capture/mod.rs`) for window-relative click dispatch and aspect-ratio preserved thumbnails.




