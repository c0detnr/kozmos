# Findings
Startup previously restored a global recent-project registry and accepted a project argument. Sidebar and project page allowed switching projects. Discovery scanned HOME and external custom sources. Settings and telemetry exports must become project-local. Resource symlinks need containment checks.

Implemented project-local settings and resource containment (including existing ancestors for new paths). MCP cwd overrides are limited to the project; MCP file reads and CSV exports recheck containment. Tests passed before final MCP-boundary additions. System metrics continue to use OS data as before.
