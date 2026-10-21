# Kozmos
## Goal
Deliver and launch a polished native Rust Linux dashboard with real telemetry.
## Phases
- [x] Inspect system and select native eframe renderer.
- [x] Implement live telemetry, orbital visualization, process view and focus timer.
- [x] Build, test, visually inspect and launch; add desktop entry and README.
## Next Step
Complete. Application running; source and README delivered.
## Errors
Initial cargo fetch had no source target; resolved by writing main.rs.
Initial shell launch used a not-yet-created cwd; created directory from parent.

Clippy range-loop warning fixed with enumerate. Old Hyprland dispatch syntax rejected; used installed Lua dispatchers.
