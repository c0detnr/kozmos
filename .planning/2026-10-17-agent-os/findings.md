# Findings
Existing app uses eframe 0.33, native Glow/Wayland and Linux telemetry. Preserve it as a system view.
Global skill directories exist for .agents, .claude and .codex; Claude desktop JSON exists. Discover known paths without logging file contents or secrets. Config formats will be JSON and TOML, with full-document validation and optimistic conflict detection before writing. Never execute discovered MCP commands.
