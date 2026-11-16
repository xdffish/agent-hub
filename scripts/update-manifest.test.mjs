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
function build(
  { source, output, notes },
  missingSignature = false,
  macNames = ["Agent Hub_aarch64.app.tar.gz", "Agent Hub_x86_64.app.tar.gz"],
) {
  mkdirSync(source);
  for (const name of [...macNames, "Agent Hub.exe", "Agent Hub.AppImage"]) {
    writeFileSync(join(source, name), "bundle");
    if (!missingSignature || !name.endsWith(".exe"))
      writeFileSync(join(source, name + ".sig"), `signature:${name}`);
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
      const filename = decodeURIComponent(
        new URL(entry.url).pathname.split("/").pop(),
      );
      assert.equal(
        entry.signature,
        readFileSync(join(ctx.output, filename + ".sig"), "utf8"),
      );
      assert.ok(
        existsSync(
          join(
            ctx.output,
            decodeURIComponent(new URL(entry.url).pathname.split("/").pop()),
          ),
        ),
      );
    }
    assert.match(
      manifest.platforms["darwin-aarch64"].url,
      /_aarch64\.app\.tar\.gz$/,
    );
    assert.match(
      manifest.platforms["darwin-x86_64"].url,
      /_x86_64\.app\.tar\.gz$/,
    );
  }));
test("missing signature prevents publishing a manifest", () =>
  fixture((ctx) => {
    const result = build(ctx, true);
    assert.notEqual(result.status, 0);
    assert.equal(existsSync(join(ctx.output, "latest.json")), false);
  }));
test("unlabelled macOS bundles cannot be assigned to both architectures", () =>
  fixture((ctx) => {
    const result = build(ctx, false, ["Agent Hub.app.tar.gz"]);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /Missing macOS architecture/);
    assert.equal(existsSync(join(ctx.output, "latest.json")), false);
  }));
test("both macOS architectures are required before publishing", () =>
  fixture((ctx) => {
    const result = build(ctx, false, ["Agent Hub_aarch64.app.tar.gz"]);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /Missing updater target: darwin-x86_64/);
    assert.equal(existsSync(join(ctx.output, "latest.json")), false);
  }));
