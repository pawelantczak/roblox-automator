# Roblox Automator (Windows)

Roblox Automator is a high-performance native Windows automation and monitoring suite designed to maintain 24/7 uptime for Roblox games (such as *Pet Simulator 99*) across multiple concurrent accounts. It detects disconnections via on-device Windows Media OCR, automatically reconnects or relaunches, navigates back to active event areas, and prevents 20-minute idle kicks using stealth Anti-AFK character jumps.

---

## Architecture & Project Structure

- **Single Source of Truth for Versioning**:
  - `VERSION`: Root plain-text file holding the semver string (e.g. `0.1.0`).
  - Synchronized into `Cargo.toml` (`version = "..."`), binary metadata, and release archives.
- **Components**:
  - `src/models.rs`: `Config`, `InstanceCard`, `UIStateBroadcast`, `IPCAction`, and `DetectedText` entities.
  - `src/window.rs`: Win32 process discovery (`RobloxPlayerBeta.exe`), window handle (`HWND`) enumeration, and unminimize/activation helpers.
  - `src/capture.rs`: Silent background window capture via Win32 GDI `PrintWindow` (`PW_RENDERFULLCONTENT`) and downscaled JPEG base64 thumbnail encoding.
  - `src/ocr.rs`: On-device Windows Media OCR (`windows::Media::Ocr::OcrEngine`) and super-sampled left sidebar recognition.
  - `src/classifier.rs`: Disconnect keyword filtering, non-fatal toast exclusion, and Levenshtein fuzzy word matching.
  - `src/tracer.rs`: Computer vision cartoon border (`UIStroke`) contour tracing and geometric centroid calculation.
  - `src/input.rs`: Atomic serialized input queue (`INPUT_LOCK`), hardware scancode spacebar jump (`0x0039`), and stealth Anti-AFK engine.
  - `src/engine.rs`: Multi-instance background monitoring loop and state coordinator.
  - `src/power.rs`: Windows system power execution state (`SetThreadExecutionState`) display sleep and auto-lock prevention.
  - `src/storage.rs`: Deterministic JSON configuration (`config.json`) loading and saving.
  - `src/tray.rs`: Native Windows notification area system tray icon, dynamic states, and context menu.
  - `src/main.rs`: Entry point, Tao event loop, Win32 Named Mutex single-instance guard, and WebView2 IPC dispatcher.
  - `resources/index.html`: Responsive dark-mode CCTV multi-window grid dashboard styled with Tailwind CSS.
  - `bump.ps1` & `bump.sh`: Automated semver bumping scripts that update `VERSION`, `Cargo.toml`, and synchronize `CHANGELOG.md` headers.
  - `.github/workflows/build.yml`: GitHub Actions pipeline running on GitHub-hosted `windows-latest` runners, compiling release binaries, publishing standalone `roblox-automator.exe` alongside native ZIP archives, extracting release notes, and publishing GitHub Releases.

---

## Core Capabilities

1. **Multi-Instance CCTV Monitoring Grid**:
   - Discovers and monitors all active `RobloxPlayerBeta.exe` client windows simultaneously on Windows.
   - Streams live, downscaled background thumbnails, PID labels, health badges, and individual Anti-AFK countdowns to an embedded WebView2 dashboard.
2. **Silent Background Screen Capture**:
   - Captures each window's graphics backbuffer using GDI `PrintWindow` with `PW_RENDERFULLCONTENT` without bringing windows to the front, interrupting desktop work, or capturing user desktop screens.
3. **On-Device Windows Media OCR**:
   - Uses Windows 10/11 native `windows::Media::Ocr::OcrEngine` for ultra-fast, local, hardware-accelerated text recognition without external models or dependencies.
   - Detects disconnect dialogs, error codes (`277`, `279`, `267`, `271`, `268`, `273`), and server shutdown notices.
4. **Universal Event Navigation & Border Tracing**:
   - Analyzes stylized in-game typography using phonetic normalization and Levenshtein edit distance (e.g. matching "Turkcy" or "Hung" to "Turkey Hunt").
   - Dynamically traces dark cartoon outlines (`UIStroke`, luminance $< 48$) and uses column-gap isolation to target the exact geometric centroid of event icons.
5. **Stealth Anti-AFK Keep-Alive Engine**:
   - Sends an atomic Focus-Click-Space-Restore sequence with random spatial jitter ($\pm 6\text{px}$) every 10 minutes.
   - Dispatches OEM physical hardware keyboard scan codes (`0x0039`) alongside Win32 `PostMessage` dual-dispatch and a 140ms physics hold duration, ensuring compatibility with Roblox's DirectInput and Raw Input engine.
   - Instantly restores previous hardware cursor position and foreground application focus.
6. **Windows System Guardians**:
   - Prevents screensaver locks and display dimming via `SetThreadExecutionState`.
   - Restricts execution to a single running instance via Win32 Named Mutex (`Global\RobloxAutomator_Mutex`).
   - Minimizes to the Windows System Tray on close (`X`), keeping automation active in the background.

---

## Strict Rules for LLMs & AI Agents

> [!IMPORTANT]
> Any AI agent, LLM, or automated assistant working on this repository must strictly adhere to the following rules:

1. **NEVER TAG CODE**:
   - Do **NOT** create, move, delete, or push Git tags (never run `git tag`, `git tag -f`, `git push origin <tag>`, or `git push --tags`).
   - Tagging is strictly reserved for the human user.
2. **ALWAYS UPDATE CHANGELOG.md**:
   - Every feature, fix, or breaking change must be documented in [CHANGELOG.md](file:///C:/Users/pawel/Sources/roblox-automator/CHANGELOG.md) adhering to the [Keep a Changelog](https://keepachangelog.com/en/1.0.0/) format.
   - **End-User Friendly Only**: Write changelog entries strictly from the end-user's perspective focusing on functional aspects, observable behavior, and user benefits. Never include low-level technical jargon, internal code identifiers, framework names, internal file names, or implementation details.
3. **ALWAYS UPDATE GEMINI.md**:
   - Any architectural change, new script, configuration field, or rule modification must be recorded in this file ([GEMINI.md](file:///C:/Users/pawel/Sources/roblox-automator/GEMINI.md)).
4. **NO AUTOMATIC VERSION BUMPING**:
   - **NEVER** bump the version automatically.
   - Always keep new additions and fixes under `## [Unreleased]` in [CHANGELOG.md](file:///C:/Users/pawel/Sources/roblox-automator/CHANGELOG.md) and wait for explicit instruction from the user before bumping `VERSION` or running `./bump.ps1` / `./bump.sh`.
   - When the user explicitly requests a version bump, determine the semver increment strictly from the release entries in [CHANGELOG.md](file:///C:/Users/pawel/Sources/roblox-automator/CHANGELOG.md):
     - If there are **only fixes** (`### Fixed`), bump **patch** (`x.y.Z`).
     - If there are any **new features or capabilities** (`### Added`), bump **minor** (`x.Y.0`).
     - If there are **breaking architectural changes**, bump **major** (`X.0.0`).
5. **MODULAR ARCHITECTURE & FILE BUDGET**:
   - Keep individual Rust files clean and modular. Split files whenever responsibilities diverge.
   - All shared domain services, game automation, OCR, window finding, input simulation, and persistence must reside in dedicated modules (`src/`). Never bundle unrelated concerns into `src/main.rs`.
6. **NO AUTOMATIC COMMITS**:
   - Do **NOT** run `git commit` automatically.
   - Always propose a copyable `git commit` command with a descriptive commit message considering all uncommitted changes for the user to review and run.
