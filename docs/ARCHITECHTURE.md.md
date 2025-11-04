# Architecture Document — Lightweight System Resource Monitor

## State Model
The application state is stored in the `LightMon` struct:
- `sys: System` — system information handle (from `sysinfo`)
- `cpu_usage: f32` — current CPU usage
- `memory_used: u64` — used memory
- `memory_total: u64` — total memory
- `current_screen: Screen` — active screen (Overview, Processes, Settings)
- `dark_mode: bool` — theme toggle

## Message Flow
- **Tick** (background) → refreshes system data every 1s
- **GoToOverview / GoToProcesses / GoToSettings** (user) → switches screens
- **ToggleTheme** (user) → flips between light/dark mode

Messages are handled in `update()`, which mutates state and triggers UI re‑render.

## Data Sources
- **System stats**: [`sysinfo`](https://crates.io/crates/sysinfo) crate  
  - CPU usage, memory usage, process table  
- **Configuration** (planned Week 5): TOML file in user’s home directory  
- **UI**: [`iced`](https://crates.io/crates/iced) crate for rendering

## Error Handling Strategy
- **Graceful defaults**: If system data is unavailable, display `0.0%` or “N/A” instead of panicking  
- **Safe state modification**: Only theme/refresh settings are changeable, no destructive actions (e.g., killing processes)  
- **Future logging**: Errors will be logged to a file for debugging  
- **Config parsing**: Use `serde` with default values if config is missing or malformed

## Current Spike (Week 2)
- **Overview screen**: Displays live CPU and memory usage  
- **Processes screen**: Displays top 10 processes by CPU usage  
- **Settings screen**: Theme toggle (light/dark)  
- **Navigation bar**: Switches between screens  
- **Background task**: 1s subscription refresh loop

This spike confirms that Rust + Iced + sysinfo can support the planned features.

## Crate Selection
- `iced` — GUI framework  
- `sysinfo` — system statistics and process table  
- `tokio` — async runtime for timers  
- `serde` + `toml` — configuration persistence (planned Week 5)

## Target OS
- **Primary**: Windows 11
- **Secondary**: LinuxUbuntu(0.22.7), macOS (if time permits)

## Risks & Mitigation
- **Rust/Iced learning curve** → Start with minimal skeleton, expand incrementally  
- **Async refresh complexity** → Use Iced subscriptions for safe background updates  
- **Scope creep** → Stick to read‑only monitoring; exclude destructive actions, such as killing process
- **Cross‑platform issues** → Test on Windows first, try to make it work on Linux/MacOS with Docker if time permits