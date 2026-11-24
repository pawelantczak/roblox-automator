# Changelog

All notable changes to **Roblox Automator** will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Added
- **Multi-Language Disconnect & Reconnect Button Support**: Added a configurable setting allowing users to specify multiple button phrases (such as *Reconnect*, *Reconectar*, *Połącz ponownie*, *Erneut verbinden*, etc.) separated by commas in the Settings dialog. This enables the automator to accurately identify and click the reconnect button on accounts running in different languages.
- **Visual Button Disconnect Detection**: Automatically recognizes disconnect dialogs across non-English accounts whenever a recognized reconnect button phrase is visible on-screen, ensuring seamless recovery even when localized game client disconnect messages vary.
- **Hardware Scancode Spacebar Jump**: Anti-AFK character jumps now supply physical OEM keyboard scan codes (`0x0039`) alongside virtual keys and Win32 `PostMessage` dual-dispatch, ensuring character jumps register in Roblox's DirectInput and Raw Input engine.
- **Extended Jump Dwell Duration**: Key hold duration extended to 140ms with a 150ms post-jump buffer to guarantee Roblox physics step frames process character jumps before restoring desktop focus.
- **Active Focus Lock Verification**: Verifies active window foreground state before dispatching keystrokes, eliminating dropped inputs during rapid application switching.

### Fixed
- **Screen Text Recognition Engine**: Resolved an issue where background text detection failed silently on captured game frames, preventing the automator from identifying disconnect dialogs and in-game buttons.
- **Join Error & HTTP Error Detection**: Added automatic detection for join failures and HTTP connection errors (such as Error Code 529), including support for "Retry" buttons to immediately re-attempt server connection.
- **Multilingual Disconnect Detection**: Expanded built-in disconnect keywords and button phrases to natively support non-English Roblox clients (such as Polish "Rozłączono" and "Kod błędu: 277") without requiring manual keyword configuration.

---

## [0.1.0] - 2026-11-23

### Added
- **Multi-Instance CCTV Monitoring Grid**: Visual dashboard displaying live thumbnail previews of all running Roblox windows simultaneously on Windows, showing individual process IDs, game health badges, and Anti-AFK countdowns.
- **Unified Master Actions**: One-click controls (*Start All*, *Stop All*, *Test Jump*, *Rescan*) that coordinate automation and settings uniformly across all active accounts.
- **Silent Background Screen Capture**: Captures game frames silently in the background without bringing windows to the front, interrupting desktop work, or capturing personal desktop content.
- **On-Device Windows Media OCR**: Fast local text recognition using built-in Windows 10/11 OCR to detect disconnect banners and error codes (`277`, `279`, `267`, etc.) in milliseconds without external tools or cloud APIs.
- **Dynamic Cartoon Border (`UIStroke`) Tracing**: Computer vision border tracer that isolates the dark cartoon stroke of in-game event icons to calculate and click the exact visual center across any display scale.
- **Fuzzy In-Game Typography Matching**: Phonetic normalization and Levenshtein edit distance algorithms that reliably match stylized game fonts (e.g. "Turkey Hunt") even on low-resolution or blurred windows.
- **Stealth Anti-AFK Keep-Alive**: Prevents 20-minute idle kicks using an atomic Focus-Click-Space-Restore sequence with random spatial jitter ($\pm 6\text{px}$) and instant restoration of previous application focus and hardware cursor location.
- **Native Windows System Tray Integration**: Minimizes cleanly to the Windows notification tray on window close, featuring a dynamic Cyan/Gray status indicator icon and right-click context menu.
- **Single-Instance Enforcement**: Protects against duplicate processes via Windows Named Mutex (`Global\RobloxAutomator_Mutex`), activating the existing dashboard if reopened.
- **Display Sleep & Screen Lock Prevention**: Engages Windows system power execution state flags (`ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED`) to prevent screen savers, monitor sleep, or auto-lock while monitoring is active.
