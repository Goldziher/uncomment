# Uncomment Coding-agent Plugins

Skills for AST-based comment inventory, preservation, removal and linting.
Install the Uncomment CLI 3.10 or newer separately. This plugin has no MCP server.

The authored skill lives in `.ai-rulez/skills/uncomment/`. Run `task plugin:generate`
from the repository root to regenerate the runtime bundles and portable `skills/`
export. Edit source files rather than generated manifests or native skills.

## Installation

- Claude Code: add `Goldziher/uncomment` as a marketplace, then install `uncomment@uncomment`.
- Codex: `codex plugin marketplace add Goldziher/uncomment`, then `codex plugin add uncomment@uncomment`.
- Cursor: use the generated `.cursor-plugin/` manifest and bundled skills, or the release's Cursor team marketplace.
- OpenCode: load this package's `.opencode/plugins/uncomment.js` with the OpenCode v2 plugin loader.
- Pi: install the local package directory with `pi install /absolute/path/to/uncomment/plugin`.
- Hermes: copy `.hermes/plugins/uncomment/` into the Hermes home's `plugins/uncomment/`,
  enable `uncomment`, and explicitly load the skill `uncomment:uncomment`.
- Gemini CLI: install this directory as an extension; `assets/GEMINI.md` is its context entry.
- Kimi CLI, Factory and Copilot: use their generated native manifests in this directory.
- Agent Plugins: use `plugin.json` and `skills/uncomment/`, or verified release archives.

The portable `skills/uncomment/` directory includes its configuration and Rust
library references. Keep supporting resources together with `SKILL.md`.

## Verification

`task ai:validate` checks both authored roots, pinned content, native bundle drift,
CLI/plugin version agreement and public skill resource bytes. Consumer tests check
native Pi discovery, OpenCode v2 skill schemas and Hermes skill registration.
