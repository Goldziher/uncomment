// AI-RULEZ :: GENERATED FILE — DO NOT EDIT
// Content-Hash: blake3:8e7b7fe306eb825bd4f49d153d774fa79d04c409011b70ba04103d61704ebec0
// Source-Hash: blake3:ae46a09e039c77c54e729803aa1659a4c57d30dd8d348c69b66be5ab3179b618
// Schema-Version: v1

/**
 * OpenCode v2 adapter for uncomment.
 *
 * To add OpenCode-specific tools or hooks:
 *
 * 1. Create .ai-rulez/opencode/index.js.
 * 2. Default-export { id, setup(ctx) } from that source file. OpenCode v2 does not
 *    run v1 plugins (an exported function returning hooks).
 * 3. If the plugin bundles skills, commands, or agents, call
 *    registerBundledContent(ctx) from ../ai-rulez-content.js in setup.
 * 4. Run ai-rulez generate --plugin --dry-run, then ai-rulez generate --plugin.
 *
 * Keep shared skills, commands, agents, and MCP configuration in their normal
 * .ai-rulez sources. Validate all external input and never interpolate untrusted
 * values into shell commands.
 */
import { registerBundledContent } from "../ai-rulez-content.js"

/** @type {import("@opencode/plugin").Plugin.Plugin} */
export default {
  id: "uncomment",
  async setup(ctx) {
    await registerBundledContent(ctx)
  },
}
