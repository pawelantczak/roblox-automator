# Roblox Watcher

A lightweight Windows companion app that keeps your **Roblox** accounts online 24/7 — automatically reconnecting after disconnects and preventing the 20-minute idle kick across multiple game windows at once.

![Roblox Watcher Dashboard](docs/images/dashboard.png)

---

## Key Features

- **Automatic Reconnect on Disconnect**  
  Monitors every running Roblox window in the background and detects disconnect dialogs, server shutdowns, and connection errors (e.g. `277`, `279`, `529`). As soon as a disconnect appears, Roblox Watcher locates and clicks the **Reconnect** / **Retry** button automatically — including non-English Roblox clients (*Połącz ponownie*, *Reconectar*, *Erneut verbinden*, etc.).
- **Prevents the 20-Minute Anti-AFK Kick**  
  Periodically performs a quick character jump across each monitored Roblox window before Roblox's 20-minute idle timer expires, then immediately restores your previous window focus and mouse cursor position.
- **Compatible with TinyTask**  
  Designed to run cleanly alongside **TinyTask** macros. Background monitoring and text detection do not bring Roblox windows to the front, and Anti-AFK jumps return focus and cursor position right after jumping so your TinyTask loops can keep running without breaking.
- **Live Multi-Window Preview Grid**  
  Shows live thumbnail previews, connection health badges, restart counters, and zoomed target overlays for all active Roblox windows in a clean single-view dashboard.
- **Optional Event Icon Tracking**  
  Can watch the left side of the screen for a specific event button (for example, *Turkey Hunt*) and click it automatically when spotted.
- **Sleep & Screen-Lock Prevention**  
  While active, keeps Windows awake and prevents the display from turning off or locking overnight.

---

## Getting Started

1. Download **`roblox-watcher.exe`** (or the `.zip` archive) from the [Releases](../../releases) page.
2. Open one or more Roblox windows.
3. Launch **`roblox-watcher.exe`**.
4. Click **Start** to begin monitoring and Anti-AFK protection.

### Tips
- **Anti-AFK Interval**: Adjust the keep-alive interval (default: every `10` minutes) directly from the top **Anti-AFK Keep-Alive** card.
- **Scan Interval**: Adjust how often windows are checked (default: every `15` seconds) from the **Next Scan** card.
- **Settings**: Open **Settings** in the top-right to customize disconnect keywords, reconnect button phrases, or cooldown timings.
- **Logs**: Use **Download** in the Activity Telemetry console to save a log file for troubleshooting.
