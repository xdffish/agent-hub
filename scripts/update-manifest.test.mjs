import { test } from "node:test";
import assert from "node:assert/strict";
import {
  mkdtempSync,
  writeFileSync,
  readFileSync,
  existsSync,
  rmSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";

function fixture(run) {
  const dir = mkdtempSync(join(tmpdir(), "agent-hub-updater-"));
  try {
    const notes = join(dir, "notes.json");
    writeFileSync(notes, JSON.stringify({ body: "更新说明" }));
    const source = join(dir, "input");
    const output = join(dir, "output");
    return run({ dir, source, output, notes });
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}
import { mkdirSync } from "node:fs";
function build({ source, output, notes }, missingSignature = false) {
  mkdirSync(source);
  for (const name of [
    "Agent Hub.app.tar.gz",
    "Agent Hub.exe",
    "Agent Hub.AppImage",
  ]) {
    writeFileSync(join(source, name), "bundle");
    if (!missingSignature || !name.endsWith(".exe"))
      writeFileSync(join(source, name + ".sig"), "test-signature");
  }
  return spawnSync(
    process.execPath,
    ["scripts/update-manifest.mjs", source, output],
    {
      encoding: "utf8",
      env: {
        ...process.env,
        RELEASE_TAG: "v1.2.3",
        GH_REPO: "xdffish/agent-hub",
        RELEASE_NOTES_FILE: notes,
      },
    },
  );
}
test("manifest maps every supported target to an uploaded filename and signature", () =>
  fixture((ctx) => {
    const result = build(ctx);
    assert.equal(result.status, 0, result.stderr);
    const manifest = JSON.parse(
      readFileSync(join(ctx.output, "latest.json"), "utf8"),
    );
    assert.equal(manifest.version, "1.2.3");
    assert.equal(manifest.notes, "更新说明");
    assert.equal(Object.keys(manifest.platforms).length, 4);
    for (const entry of Object.values(manifest.platforms)) {
      assert.equal(entry.signature, "test-signature");
      assert.ok(
        existsSync(
          join(
            ctx.output,
            decodeURIComponent(new URL(entry.url).pathname.split("/").pop()),
          ),
        ),
      );
    }
    assert.deepEqual(
      manifest.platforms["darwin-aarch64"],
      manifest.platforms["darwin-x86_64"],
    );
  }));
test("missing signature prevents publishing a manifest", () =>
  fixture((ctx) => {
    const result = build(ctx, true);
    assert.notEqual(result.status, 0);
    assert.equal(existsSync(join(ctx.output, "latest.json")), false);
  }));
