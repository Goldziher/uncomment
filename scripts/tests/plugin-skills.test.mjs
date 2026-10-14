import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";
import plugin from "../../plugin/.opencode/plugins/uncomment.js";

const root = path.resolve(import.meta.dirname, "../..");

test("OpenCode registers the installed Uncomment skill once with readable references", async () => {
  const registered = new Map();
  const context = {
    skill: {
      transform(callback) {
        callback({ get: (id) => registered.get(id), add: (skill) => registered.set(skill.id, skill) });
      },
    },
  };
  await plugin.setup(context);
  await plugin.setup(context);
  assert.equal(registered.size, 1);
  const skill = registered.get("uncomment/uncomment");
  assert.equal(skill.name, "uncomment");
  assert.match(skill.content, /uncomment scan/u);
  for (const resource of ["configuration", "library"]) {
    const content = await readFile(path.join(path.dirname(skill.path), `references/${resource}.md`), "utf8");
    assert.ok(content.length > 100, `resource missing content: ${resource}`);
  }
});

test("Pi package ships every declared skill and its two references", async () => {
  const manifest = JSON.parse(await readFile(path.join(root, "plugin/package.json"), "utf8"));
  assert.deepEqual(manifest.pi.skills, ["./.pi/skills/uncomment"]);
  assert.deepEqual(manifest.pi.prompts, []);
  for (const item of ["SKILL.md", "index.md", ...["configuration", "library"].map((name) => `references/${name}.md`)]) {
    const file = `.pi/skills/uncomment/${item}`;
    assert.ok(manifest.files.includes(file), `npm file list omitted ${file}`);
    assert.ok((await readFile(path.join(root, "plugin", file))).length > 0);
  }
});
