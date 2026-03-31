# Changelog

All notable changes to `@claude-view/plugin` will be documented in this file.

## [0.27.0] - 2026-03-25

### Changed
- Updated OpenAPI spec and regenerated tools
- Refactored to single client instance with improved naming

## [0.26.1] - 2026-03-25

### Fixed
- Correct README tool counts, release build step, and skill allowed-tools

## [0.26.0] - 2026-03-24

### Fixed
- Codegen quality: destructive hints, dedup, URL encoding, descriptions, optional query params
- Address wiring audit findings: settings endpoints, prepublishOnly, CI message
- Read MCP server version from package.json instead of hardcoded 0.8.0

## [0.25.0] - 2026-03-24

### Changed
- Updated plugin manifest and README for 90-tool architecture

## [0.24.0] - 2026-03-23

### Added
- 6 new skill templates: insights, team-status, search, export-data, coaching, project-overview
- Skill template system with preamble and tool list injection

## [0.23.0] - 2026-03-18

### Added
- Full API to MCP tool exposure via OpenAPI codegen (78 auto-generated tools)
- Typed request bodies for coaching and telemetry annotations

### Changed
- Extracted shared constants to scripts/shared.ts (DRY)

## [0.22.0] - 2026-03-17

### Added
- Wire generated tools into MCP server with hand-written precedence

## [0.21.0] - 2026-03-16

### Added
- OpenAPI codegen pipeline for auto-generating MCP tools from API spec

## [0.20.0] - 2026-03-14

### Added
- Hand-written tools: list_sessions, get_session, search_sessions, get_stats, get_fluency_score, get_token_stats, list_live_sessions, get_live_summary

## [0.13.0] - 2026-03-10

### Added
- Validation script for plugin structure

## [0.12.0] - 2026-03-08

### Added
- /session-recap, /daily-cost, /standup skills

## [0.11.0] - 2026-03-08

### Added
- SessionStart hook for auto-starting claude-view server
- MCP bundle script and .mcp.json config

## [0.8.0] - 2026-03-07

### Added
- Initial release: scaffold @claude-view/plugin package
