Keep native UI/WebView ownership on the main thread

Create and maintain the tao event loop, window, tray, menu, and wry WebView from the main thread.
Perform polling, capture, OCR, input automation, and reconnect operations on dedicated worker threads.
Never execute SendInput, PrintWindow, Win32 blocking calls, or OCR from the event-loop callback.
Use EventLoopProxy for worker-to-UI messages only.
Separate UI, orchestration, and platform code

Organize code into layers:
ui/: window, WebView, tray, IPC, and event-loop lifecycle
engine/: monitoring state machine and scheduling
capture/, ocr/, input/, window/: platform services
models/: typed configuration and IPC contracts
Do not let HTML/JavaScript execute platform actions directly.
Keep Win32 HWND handles and window identities out of JavaScript.
Move application behavior out of main.rs; keep main.rs as composition and startup code.
Replace raw-string IPC with typed commands and events

Define Rust serializable command and event types instead of matching free-form JSON strings.
Deserialize commands defensively and never trust UI input.
Validate numeric ranges, place IDs, keyword lists, and configuration values.
Use one-way UI updates for telemetry and one explicit request/response pattern for configuration.
Separate command/action names from UI implementation details.
Decouple the worker loop from UI refresh

Monitor windows and refresh UI independently.
Do not block workers while updating the WebView.
Use a bounded channel or queued proxy events instead of spawning unbounded work.
Coalesce duplicate telemetry updates.
Add a configurable UI refresh interval, typically 500–1000 ms.
Skip engine work completely while the watcher is stopped.
Use bounded, predictable concurrency

Assign each window a stable instance ID and cancel or supersede stale work.
Process instances serially or with a small fixed worker pool rather than one task per window.
Protect shared state with targeted mutexes or lock-free atomics.
Keep locks held only for copying or short updates; never hold a lock during OCR or input.
Ensure shutdown atomics and join handles stop all workers cleanly.
Split expensive capture/OCR operations

Capture frames at a lower monitoring cadence than UI updates.
Cache thumbnails, OCR results, and screenshots instead of recomputing or encoding every poll.
Avoid JPEG/base64 encoding on the main thread.
Compress thumbnails with bounded dimensions and JPEG quality.
Consider region-only OCR and frame-difference detection before invoking full OCR.
Add per-service timeouts and skip work that falls behind.
Make native automation robust and testable

Wrap PrintWindow, BitBlt, SendInput, SetForegroundWindow, and process/window discovery in service APIs.
Prefer stable window IDs and process IDs; track Windows across title or geometry changes.
Avoid modifying shared cursor and foreground state unnecessarily.
Remove or isolate simulated random delays and jitter.
Centralize focus, mouse, and keyboard operations under an input lock.
Define explicit failure states for focus, capture, process launch, and reconnect failures.
Add a real application state machine

Track at least Idle, Discovering, Healthy, Disconnected, Reconnecting, Navigating, and Error.
Store transition timestamps and cooldowns per window.
Prevent duplicate reconnect or anti-AFK actions with per-instance locks.
Make reconnect cooldowns deterministic rather than relying on elapsed state from coarse polling.
Record restart and OCR failure counts for diagnostics.
Make scheduling and cadence data-driven

Separate scan interval, UI interval, reconnect cooldown, and anti-AFK interval.
Stop anti-AFK timers while the watcher is idle.
Initialize timers safely for newly discovered processes.
Use deadlines or Instant values instead of repeatedly reconstructing countdown strings.
Do not interpret UI-local countdown formatting as backend state.
Reduce lock contention

Replace broad shared locks with a single snapshot or immutable state model.
Build UI state in the worker and publish snapshots instead of exposing mutable engine internals.
Store logs with a bounded queue and clear/update operations.
Update configuration atomically and log when effective configuration changes.
Improve configuration handling

Store configuration as validated typed Rust data.
Separate default, persisted, and runtime configuration.
Apply a versioned JSON schema and migration strategy.
Validate paths and numerical ranges before saving.
Write atomically instead of overwriting configuration in place.
Remove hard-coded fallback keywords/place IDs where possible.
Bundle and secure WebView assets

Bundle HTML, JavaScript, Tailwind CSS, and icons locally.
Do not load application dependencies from CDNs.
Configure an appropriate WebView2 origin/content security policy.
Disable remote navigation and development features in release builds.
Use JS bridging for native commands only; do not expose a general command-execution API.
Escape or build all dynamic HTML safely.
Improve shutdown and lifecycle handling

Stop workers before closing the WebView.
Join or signal every thread during shutdown.
Release the named mutex, sleep-prevention request, tray state, and timers.
Handle close-to-tray separately from application exit.
Restore foreground and cursor state reliably even when automation is interrupted.
Improve reliability and observability

Add structured logs with timestamps and instance IDs.
Add counters for captures, OCR runs, reconnect attempts, anti-AFK jumps, and failures.
Avoid logging secrets or large frame data.
Add diagnostic export or a developer-only diagnostics view.
Include application and WebView2 runtime versions in diagnostic information.
Add testing where it matters

Unit test classifier and configuration parsing.
Unit test state transitions and cooldown calculations.
Model OCR and capture results behind traits.
Test IPC serialization and UI update snapshots.
Use synthetic frame fixtures and mock window services for automation tests.
Keep hardware/Win32 integration tests Windows-only and clearly separated.
Optimize packaging and releases

Continue building on Windows and publishing standalone binaries.
Add architecture and runtime checks before launch.
Ensure WebView2 is available or provide a clear bootstrap/error path.
Add code signing and reproducible versioning before broader distribution.
Package assets inside the executable or release archive without relying on the repository layout.
Preserve maintainability in the data flow

main.rs: startup/composition only
UI thread: event loop and native UI state
Worker: discovery, monitoring, state transitions, and native operations
IPC: typed commands and snapshots
Domain models: configuration and engine state
HTML/JS: presentation and user interactions only
