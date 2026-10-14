import assert from "node:assert/strict";

import { readFile } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";
import test from "node:test";
import plugin from "../../plugin/.opencode/plugins/uncomment.js";

const sdk = path.resolve(process.env.NATIVE_SDK_DIRECTORY ?? ".native/sdk");

const { Skill } = await import(pathToFileURL(path.join(sdk, "node_modules/@opencode/plugin/dist/promise/index.js")));
const Schema = await import(
  pathToFileURL(process.env.NATIVE_EFFECT_SCHEMA ?? path.join(sdk, "node_modules/effect/dist/Schema.js"))
);
const piEntry = path.join(sdk, "node_modules/@earendil-works/pi-coding-agent/dist/index.js");
const { loadSkillsFromDir } = await import(pathToFileURL(path.join(path.dirname(piEntry), "core/skills.js")));
const root = path.resolve(import.meta.dirname, "../../plugin");

test("Pi discovers one installed skill with two readable references", async () => {
  const result = loadSkillsFromDir({ dir: path.join(root, ".pi/skills"), source: "local" });
  assert.equal(result.skills.length, 1);
  assert.deepEqual(result.diagnostics, []);
  assert.equal(result.skills[0].name, "uncomment");
  for (const reference of ["configuration", "library"]) {
    const resource = path.join(root, ".pi/skills/uncomment/references", `${reference}.md`);
    assert.ok((await readFile(resource)).length > 100);
  }
});

test("OpenCode accepts the registered native skill and rejects malformed content", async () => {
  const registered = new Map();
  await plugin.setup({
    skill: {
      transform(callback) {
        callback({
          get: (id) => registered.get(id),
          add: (skill) => registered.set(skill.id, Schema.decodeUnknownSync(Skill.Info)(skill)),
        });
      },
    },
  });
  assert.equal(registered.size, 1);
  const skill = registered.get("uncomment/uncomment");
  assert.equal(skill.name, "uncomment");
  assert.throws(() => Schema.decodeUnknownSync(Skill.Info)({ ...skill, content: 42 }));
});
