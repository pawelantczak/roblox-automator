# Changelog

Technical changelog for **Roblox Watcher** maintainers and contributors.  
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/) and [Semantic Versioning](https://semver.org/spec/v2.0.0.html).  
End-user release announcements are maintained in [RELEASE_NOTES.md](RELEASE_NOTES.md).

---

## [Unreleased]

### Added
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

---

## [0.1.0] - 2026-11-23

### Added
- Core modular architecture: `src/window/` (`Win32WindowService`), `src/capture/` (`Win32CaptureService` via `PrintWindow` `PW_RENDERFULLCONTENT`), `src/ocr/` (`WindowsMediaOcrService`), `src/input/` (`Win32InputService` with `INPUT_LOCK` and `ForegroundAndCursorGuard`), `src/engine/` (`EngineController`, `EngineWorker`, `InstanceTracker`, `Classifier`, `IconTracer`), `src/power.rs` (`SetThreadExecutionState`), `src/storage.rs`, and `src/ui/` (`tao` + `wry` WebView2).
- Single-instance Windows Named Mutex (`Global\RobloxWatcher_Mutex`) in `src/main.rs`.
- GitHub Actions CI/CD workflow (`.github/workflows/build.yml`) and version bump scripts (`bump.ps1`, `bump.sh`).
