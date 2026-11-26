# Changelog

All notable changes to **Roblox Watcher** will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Fixed
- **Fixed Preview Card Height**: Moved the failure/reconnect cooldown status bar inside the bottom of the preview image as an overlay so preview cards keep a constant height regardless of window state.
- **Reconnect Button Positioning**: Improved reconnect button detection to pinpoint the exact matched button text rather than whole merged button rows or error description text, and added an upscaled center-region scan fallback when disconnect text is seen across modals of varying heights.

### Changed
- **Balanced Preview Grid & Fixed Window Width**: Updated the preview grid to fit 1–4 windows in a single row, switch to 3×2 equal rows for 6 windows (and 3×3 for 9), scale cards fluidly, lock the application window width to `1040px` while keeping height resizable, and always show the full preview section without scrolling even as new windows appear at runtime.
- **Initial & Idle Preview Refresh**: Captured initial window preview thumbnails immediately on startup and refreshed them periodically (every 10s when frames change) while idle, updated default scan interval to 15s and UI refresh interval to 2s (configured in seconds), renamed the event ribbon label to *Event Icon Text*, shortened the log download button label to *Download*, and removed process/instance IDs from preview cards and console log lines.
- **UI Streamlining & Scrollbar Polish**: Removed the *Scan* and *Diagnostics* buttons, removed *Target Place ID* from the dashboard and settings, and styled the settings dialog scrollbar to match the dark theme.
- **Rebranded to Roblox Watcher**: Renamed the application to *Roblox Watcher*, added the custom bundled app icon to both the window titlebar/taskbar and dashboard header, and removed the "Windows Multi-Instance CCTV" header subtitle.

### Added
- **Reconnect Button Crop Overlay**: When a disconnect/reconnect button is recognized, a small `44×44` zoomed crop centered on its click target (with a rose border) is overlaid in the bottom-right corner of the window preview card and logged to JSONL.
- **Single-Row 5-Box Status Ribbon**: Merged *Session Uptime* and *Active Windows* into a single *Session* card (showing full `window`/`windows` text) and locked the top ribbon to a single horizontal row (`Watcher Status`, `Session`, `Next Scan`, `Anti-AFK Keep-Alive`, `Event Icon Text`).
- **JSONL Log Persistence & Download**: All console log messages—including base64-encoded crops of detected event icons—are now recorded in JSONL format and can be downloaded via the new **Download** button in the Activity Telemetry header.
- **Console Scroll Pause & Resume Button**: Scrolling up in the Activity Telemetry console automatically pauses live log updates to preserve your scroll position, and a **Resume Logs** button next to *Clear Console* resumes live updates and catches up.
- **Event Target Crop Overlay**: When an event icon is recognized, a small `44×44` zoomed crop centered on the traced target point (with a cyan crosshair) is overlaid in the bottom-right corner of the window preview card.
- **Detailed Engine Activity Logs**: Added real-time console log entries for window discovery, OCR scans, disconnect/cooldown status, reconnect button clicks, event text searches/matches per window (`#1`, `#2`, etc.), and Anti-AFK jumps.
- **Target Event Ribbon Box & Ticker**: Moved *Target Event Keywords* out of the Settings dialog into a dedicated top ribbon card with an inline enable/disable toggle and instant keyword updates.
- **Summary Stat Ribbon & Global Anti-AFK Timer**: Added a top status ribbon combining watcher status and queue state, displaying active window count, target Place ID, and a global Anti-AFK live countdown with inline up/down interval controls that apply immediately while keeping restart counts per window.
- **Fully Externalized Settings & Strings**: All target process names, disconnect dialog keywords, ignore phrases, reconnect button phrases, event keywords, status labels, tray menu labels, and application title are now configurable via the Settings dialog and `config.json`.
- **Offline Bundled Dashboard & Diagnostics View**: All dashboard styles and scripts are now bundled directly inside the application with zero external network dependencies, alongside a new *Diagnostics* modal displaying runtime version details, capture/OCR counters, and recovery metrics.
- **Configurable UI Refresh & Validation**: Added a configurable dashboard refresh interval (250–5000 ms), automatic settings validation with clear error feedback, and seamless migration for older configuration files.
- **Per-Window State & Cooldown Tracking**: Each monitored window now displays deterministic state badges (*Idle*, *Healthy*, *Disconnected*, *Reconnecting*, *Navigating*, *Error*) and active reconnect cooldown countdowns.
- **Startup Runtime Verification**: Added a startup check that detects missing WebView2 runtimes and displays a clear installation prompt.
- **Multi-Language Disconnect & Reconnect Button Support**: Added a configurable setting allowing users to specify multiple button phrases (such as *Reconnect*, *Reconectar*, *Połącz ponownie*, *Erneut verbinden*, etc.) separated by commas in the Settings dialog. This enables the watcher to accurately identify and click the reconnect button on accounts running in different languages.
- **Visual Button Disconnect Detection**: Automatically recognizes disconnect dialogs across non-English accounts whenever a recognized reconnect button phrase is visible on-screen, ensuring seamless recovery even when localized game client disconnect messages vary.
- **Hardware Scancode Spacebar Jump**: Anti-AFK character jumps now supply physical OEM keyboard scan codes (`0x0039`) alongside virtual keys and Win32 `PostMessage` dual-dispatch, ensuring character jumps register in Roblox's DirectInput and Raw Input engine.
- **Extended Jump Dwell Duration**: Key hold duration extended to 140ms with a 150ms post-jump buffer to guarantee Roblox physics step frames process character jumps before restoring desktop focus.
- **Active Focus Lock Verification**: Verifies active window foreground state before dispatching keystrokes, eliminating dropped inputs during rapid application switching.

### Removed
- **System Tray Integration**: Removed the Windows system tray icon, tray menu, and minimize-to-tray behavior; closing the dashboard window now exits the application directly.

### Fixed
- **Accurate Event Icon Centering**: Isolated matched keyword word boxes (excluding trailing counters/adjacent columns) and added bottom-up row-gap isolation in the border tracer so icons above the target (such as "Ready!" banners) are never included in the traced center.
- **Sidebar OCR Dimension Cap & Fuzzy Merged Matching**: Capped upscaled sidebar dimensions within Windows OCR limits (`<= 2200px`), switched high-contrast text isolation to luminance thresholding, and added punctuation splitting plus fuzzy sliding-window matching for merged event labels (e.g. `Turkey'Hunt`, `TurkoyHunt`).
- **First-Cycle High-Contrast Event OCR**: Added a 3x upscaled black-on-white high-contrast sidebar OCR fallback pass so stylized white event labels are recognized on the first scan cycle without being obscured by colorful game backgrounds.
- **Locked Window Preview Grid Height**: Prevented the Activity Telemetry console from shrinking the window preview section when log lines accumulate, keeping preview cards at full height without unwanted scrollbars.
- **Event Icon Click Execution**: Dispatched mouse clicks to the traced event icon center whenever event text is matched, guarded by a per-window cooldown to prevent repeated clicks.
- **Event Icon Sidebar Detection**: Expanded the left-sidebar scan region to cover multi-column event buttons, added full-frame fallback matching, and handled merged OCR words (e.g. "TurkeyHunt") for reliable event icon recognition.
- **Aspect-Ratio Preserved Previews & Adaptive Grid**: Window preview thumbnails now preserve their exact original aspect ratio without stretching or scaling when resizing the dashboard, the window grid adapts columns cleanly, and the Activity Telemetry panel sticks directly below the windows grid.
- **Reliable Reconnect Button Clicking**: Fixed an issue where reconnect clicks were offset by the window title bar and borders, and updated mouse click timing so the game client reliably registers button hover and press events.
- **Screen Text Recognition Engine**: Resolved an issue where background text detection failed silently on captured game frames, preventing the watcher from identifying disconnect dialogs and in-game buttons.
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
- **Single-Instance Enforcement**: Protects against duplicate processes via Windows Named Mutex (`Global\RobloxWatcher_Mutex`), activating the existing dashboard if reopened.
- **Display Sleep & Screen Lock Prevention**: Engages Windows system power execution state flags (`ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED`) to prevent screen savers, monitor sleep, or auto-lock while monitoring is active.
