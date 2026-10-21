# Terminal project scope

## Goal
Kozmos starts from a terminal, uses only its launch directory as project root, and never restores or switches to another project.

## Phases
- [x] Inspect startup, discovery, UI and documentation.
- [x] Implement fixed launch root, local settings and bounded resource discovery.
- [x] Align UI and instructions with single-project workflow.
- [x] Run tests, formatting and clippy; review diff.

## Next Step
Complete; changes are ready for review.

## Errors
Target construction initially had ambiguous Result inference; simplified to direct paths.
Initial file listing included a nonexistent scripts directory; subsequent reads use known paths.
