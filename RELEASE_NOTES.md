# Release Notes

User-facing release notes for **Roblox Automator**. Each version section is published to GitHub Releases when a release tag is pushed.

---

## [Unreleased]

---

## [0.2.0] - 2026-11-27

### Added
- **Macro Recording & Multi-Window Replay**: Record keystrokes, mouse movements, clicks, and scrolling directly in one Roblox window (with a `3… 2… 1` countdown overlay and automatic stop when moving the mouse outside the window), then replay the recorded sequence automatically across every Roblox window after reconnecting or Smart Click.
- **Uniform Window Sizing**: Automatically resizes active Roblox windows to a consistent `800×600` layout so in-game buttons and macro positions align across every account.
- **Status Footer & Community Links**: Added a bottom bar displaying the app version badge, a clickable link to `ps99clan.xyz`, and a **Discord** invite button (`https://discord.gg/ce5yGvTf9`).
- **Versioned Log Exports**: Included the app version in downloaded JSONL log files to make troubleshooting easier.

### Changed
- **Rebranded to Roblox Automator**: Updated the application name, executable, and simplified full-bleed gear icon.
- **Redesigned Top Ribbon & Larger Typography**: Streamlined the 5-card ribbon (`Automator` with live uptime timer, `Next Scan`, `Anti-AFK`, `Smart Click`, and `Macro`), increased font sizes across the dashboard for better readability, and highlighted all user-initiated actions in green in the Activity Telemetry console.

### Removed
- **Application Title Setting**: Removed the redundant Application Title field from Global Settings.

---

## [0.1.0] - 2026-11-27

### Added
- **Multi-Instance CCTV Monitoring Grid**: Live thumbnail dashboard for all running Roblox windows with health badges, restart counters, and Anti-AFK countdowns.
- **Unified Master Actions**: One-click controls (*Start All*, *Stop All*, *Test Jump*) across all active accounts.
- **Silent Background Screen Capture & On-Device Windows Media OCR**: Captures game frames silently without bringing windows to the front and detects disconnect banners/error codes (`277`, `279`, `267`, `529`, etc.) locally.
- **Dynamic Cartoon Border Tracing & Fuzzy Typography Matching**: Automatically locates and clicks in-game event icons.
- **Stealth Anti-AFK Keep-Alive & TinyTask Compatibility**: Prevents 20-minute idle kicks using physical keyboard scan codes while restoring previous application focus and cursor position.
- **Single-Instance Enforcement & Sleep Prevention**: Protects against duplicate processes and prevents Windows display/system sleep while monitoring is active.
- **Reconnect Button Crop Overlay**: When a disconnect/reconnect button is recognized, a small `44×44` zoomed crop centered on its click target (with a rose border) is overlaid in the bottom-right corner of the window preview card and logged to JSONL.
- **Single-Row 5-Box Status Ribbon**: Merged *Session Uptime* and *Active Windows* into a single *Session* card (showing full `window`/`windows` text) and locked the top ribbon to a single horizontal row (`Watcher Status`, `Session`, `Next Scan`, `Anti-AFK Keep-Alive`, `Event Icon Text`).
- **JSONL Log Persistence & Download**: All console log messages—including base64-encoded crops of detected event icons—are now recorded in JSONL format and can be downloaded via the **Download** button in the Activity Telemetry header.
- **Console Scroll Pause & Resume Button**: Scrolling up in the Activity Telemetry console automatically pauses live log updates to preserve your scroll position, and a **Resume Logs** button next to *Clear Console* resumes live updates and catches up.
- **Event Target Crop Overlay**: When an event icon is recognized, a small `44×44` zoomed crop centered on the traced target point (with a cyan crosshair) is overlaid in the bottom-right corner of the window preview card.
- **Detailed Engine Activity Logs**: Added real-time console log entries for window discovery, OCR scans, disconnect/cooldown status, reconnect button clicks, event text searches/matches per window (`#1`, `#2`, etc.), and Anti-AFK jumps.
- **Target Event Ribbon Box & Ticker**: Added a dedicated top ribbon card for event keywords with an inline enable/disable toggle and instant keyword updates.
- **Summary Stat Ribbon & Global Anti-AFK Timer**: Added a top status ribbon displaying watcher status, active window count, and a global Anti-AFK live countdown with inline up/down interval controls.
- **Offline Bundled Dashboard & Multi-Language Support**: Bundled all dashboard assets locally with zero external dependencies, configurable settings/phrases, and multi-language disconnect/reconnect button support (*Reconnect*, *Retry*, *Reconectar*, *Połącz ponownie*, *Erneut verbinden*, etc.).
- **Hardware Scancode Spacebar Jump**: Anti-AFK character jumps use physical keyboard scan codes with extended dwell duration and active focus verification for reliable in-game jumps.

### Changed
- **Balanced Preview Grid & Fixed Window Width**: Updated the preview grid to fit 1–4 windows in a single row, switch to 3×2 equal rows for 6 windows (and 3×3 for 9), scale cards fluidly, lock the application window width to `1040px` while keeping height resizable, and always show the full preview section without scrolling even as new windows appear at runtime.
- **Initial & Idle Preview Refresh**: Captured initial window preview thumbnails immediately on startup and refreshed them periodically (every 10s when frames change) while idle, updated default scan interval to 15s and UI refresh interval to 2s, renamed the event ribbon label to *Event Icon Text*, shortened the log download button label to *Download*, and removed process/instance IDs from preview cards and console log lines.
- **UI Streamlining & Rebranding**: Renamed the application to *Roblox Watcher* with a custom bundled app icon, removed *Scan* / *Diagnostics* / *Target Place ID* clutter, and styled the settings dialog scrollbar to match the dark theme.

### Fixed
- **Fixed Preview Card Height**: Moved the failure/reconnect cooldown status bar inside the bottom of the preview image as an overlay so preview cards keep a constant height regardless of window state.
- **Reconnect Button Positioning**: Improved reconnect button detection to pinpoint the exact matched button text rather than whole merged button rows or error description text, and added an upscaled center-region scan fallback across modals of varying heights.
- **Event Icon Detection & Centering**: Improved sidebar OCR contrast/upscaling, fuzzy matching for stylized event text, and cartoon border tracing so event icon clicks hit the exact icon center.

### Removed
- **System Tray Integration**: Removed the Windows system tray icon and minimize-to-tray behavior; closing the dashboard window now exits the application directly.

