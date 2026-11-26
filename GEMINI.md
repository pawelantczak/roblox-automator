# Roblox Watcher (Windows)

Roblox Watcher is a high-performance native Windows automation and monitoring suite designed to maintain 24/7 uptime for Roblox games (such as *Pet Simulator 99*) across multiple concurrent accounts. It detects disconnections via on-device Windows Media OCR, automatically reconnects or relaunches, navigates back to active event areas, and prevents 20-minute idle kicks using stealth Anti-AFK character jumps.

---

## Architecture & Project Structure

- **Single Source of Truth for Versioning**:
  - `VERSION`: Root plain-text file holding the semver string (e.g. `0.1.0`).
  - Synchronized into `Cargo.toml` (`version = "..."`), binary metadata, and release archives.
- **Components**:
  - `src/models/`: Versioned `Config` (schema v2 with `target_process_names`, `fallback_window_title`, `ui_strings`, `disconnect_keywords`, `reconnect_button_phrases`, `ignore_phrases`, `event_target_keywords`, and validation/migration), typed `UiCommand` / `UiEvent` IPC contracts, `InstanceState` / `EngineMetrics` / `BoundedLogQueue` state models, and `CapturedFrame` / `DetectedText` vision entities.
  - `src/window/`: `WindowDiscovery` trait and `Win32WindowService` for `RobloxPlayerBeta.exe` discovery and stable `InstanceId` tracking (keeping raw `HWND`s out of JS).
  - `src/capture/`: `ScreenCapture` trait and `Win32CaptureService` for silent `PrintWindow` / `BitBlt` capture, sampled frame-difference hashing, and quality-bounded JPEG thumbnail encoding.
  - `src/ocr/`: `OcrProvider` trait and `WindowsMediaOcrService` for timeout-guarded Windows Media OCR (`OcrEngine`) and super-sampled left sidebar recognition.
  - `src/input/`: `InputAutomation` trait and `Win32InputService` with `INPUT_LOCK` serialization, RAII `ForegroundAndCursorGuard` restoration, hardware scancode spacebar jumps (`0x0039`), and explicit `FailureReason` reporting.
  - `src/engine/`: Dedicated worker thread (`EngineController` / `EngineWorker`), data-driven `InstanceTracker` state machine & deadlines (`scheduler.rs`), `Classifier` (`classifier.rs`), and `IconTracer` (`tracer.rs`) with bounded command channels and frame-hash caching.
  - `src/ui/`: Main-thread UI layer (`run_app`) and hardened `WebViewManager` (`webview.rs`) with local bundled assets, CSP, disabled remote navigation/release devtools, and WebView2 runtime check.
  - `src/power.rs`: Windows system power execution state (`SetThreadExecutionState`) display sleep and auto-lock prevention.
  - `src/storage.rs`: Atomic JSON configuration (`config.json`) writes and versioned schema migration.
  - `src/main.rs`: Pure startup and composition root (WebView2 runtime check, RAII Win32 Named Mutex guard, config loading, and UI launch).
  - `resources/`: Locally bundled `index.html`, `styles.css`, and `app.js` with strict Content Security Policy and typed IPC bridging (zero CDN dependencies).
  - `bump.ps1` & `bump.sh`: Automated semver bumping scripts that update `VERSION`, `Cargo.toml`, and synchronize `CHANGELOG.md` headers.
  - `.github/workflows/build.yml`: GitHub Actions pipeline running on GitHub-hosted `windows-latest` runners, compiling release binaries, publishing standalone `roblox-watcher.exe` alongside native ZIP archives, extracting release notes, and publishing GitHub Releases.

---

## Core Capabilities

1. **Multi-Instance CCTV Monitoring Grid**:
   - Discovers and monitors all active `RobloxPlayerBeta.exe` client windows simultaneously on Windows.
   - Streams live, downscaled background thumbnails, PID labels, health badges, and individual Anti-AFK countdowns to an embedded WebView2 dashboard.
2. **Silent Background Screen Capture**:
   - Captures each window's graphics backbuffer using GDI `PrintWindow` with `PW_RENDERFULLCONTENT` without bringing windows to the front, interrupting desktop work, or capturing user desktop screens.
3. **On-Device Windows Media OCR**:
   - Uses Windows 10/11 native `windows::Media::Ocr::OcrEngine` for ultra-fast, local, hardware-accelerated text recognition without external models or dependencies.
   - Detects disconnect dialogs, error codes (`277`, `279`, `267`, `271`, `268`, `273`), server shutdown notices, and localized disconnect/reconnect button phrases across multi-language client accounts (e.g. English, Polish, Spanish, German, etc.).
4. **Universal Event Navigation & Border Tracing**:
   - Analyzes stylized in-game typography using phonetic normalization and Levenshtein edit distance (e.g. matching "Turkcy" or "Hung" to "Turkey Hunt").
   - Dynamically traces dark cartoon outlines (`UIStroke`, luminance $< 48$) and uses column-gap isolation to target the exact geometric centroid of event icons.
5. **Stealth Anti-AFK Keep-Alive Engine**:
   - Sends an atomic Focus-Click-Space-Restore sequence with random spatial jitter ($\pm 6\text{px}$) every 10 minutes.
   - Dispatches OEM physical hardware keyboard scan codes (`0x0039`) alongside Win32 `PostMessage` dual-dispatch and a 140ms physics hold duration, ensuring compatibility with Roblox's DirectInput and Raw Input engine.
   - Instantly restores previous hardware cursor position and foreground application focus.
6. **Windows System Guardians**:
   - Prevents screensaver locks and display dimming via `SetThreadExecutionState`.
   - Restricts execution to a single running instance via Win32 Named Mutex (`Global\RobloxWatcher_Mutex`).

---

## Strict Rules for LLMs & AI Agents

> [!IMPORTANT]
> Any AI agent, LLM, or automated assistant working on this repository must strictly adhere to the following rules:

1. **NEVER TAG CODE**:
   - Do **NOT** create, move, delete, or push Git tags (never run `git tag`, `git tag -f`, `git push origin <tag>`, or `git push --tags`).
   - Tagging is strictly reserved for the human user.
2. **ALWAYS UPDATE CHANGELOG.md**:
   - Every feature, fix, or breaking change must be documented in [CHANGELOG.md](file:///C:/Users/pawel/Sources/roblox-watcher/CHANGELOG.md) adhering to the [Keep a Changelog](https://keepachangelog.com/en/1.0.0/) format.
   - **End-User Friendly Only**: Write changelog entries strictly from the end-user's perspective focusing on functional aspects, observable behavior, and user benefits. Never include low-level technical jargon, internal code identifiers, framework names, internal file names, or implementation details.
3. **ALWAYS UPDATE GEMINI.md**:
   - Any architectural change, new script, configuration field, or rule modification must be recorded in this file ([GEMINI.md](file:///C:/Users/pawel/Sources/roblox-watcher/GEMINI.md)).
4. **NO AUTOMATIC VERSION BUMPING**:
   - **NEVER** bump the version automatically.
   - Always keep new additions and fixes under `## [Unreleased]` in [CHANGELOG.md](file:///C:/Users/pawel/Sources/roblox-watcher/CHANGELOG.md) and wait for explicit instruction from the user before bumping `VERSION` or running `./bump.ps1` / `./bump.sh`.
   - When the user explicitly requests a version bump, determine the semver increment strictly from the release entries in [CHANGELOG.md](file:///C:/Users/pawel/Sources/roblox-watcher/CHANGELOG.md):
     - If there are **only fixes** (`### Fixed`), bump **patch** (`x.y.Z`).
     - If there are any **new features or capabilities** (`### Added`), bump **minor** (`x.Y.0`).
     - If there are **breaking architectural changes**, bump **major** (`X.0.0`).
5. **MODULAR ARCHITECTURE & FILE BUDGET**:
   - Keep individual Rust files clean and modular. Split files whenever responsibilities diverge.
   - All shared domain services, game automation, OCR, window finding, input simulation, and persistence must reside in dedicated modules (`src/`). Never bundle unrelated concerns into `src/main.rs`.
6. **NO AUTOMATIC COMMITS**:
   - Do **NOT** run `git commit` automatically.
   - Always propose a copyable `git commit` command with a descriptive commit message considering all uncommitted changes for the user to review and run.
7. **ALWAYS BUILD AFTER CODE CHANGES**:
   - Automatically run `cargo test` and `cargo build --release` after every code change before concluding.
   - If `target/release/roblox-watcher.exe` is locked by a running instance (`os error 5`), report it clearly and run `cargo check --release` to verify the release build.
