import {
  readdirSync,
  readFileSync,
  mkdirSync,
  copyFileSync,
  writeFileSync,
  existsSync,
} from "node:fs";
import { join, basename } from "node:path";

const [source, destination] = process.argv.slice(2);
const tag = process.env.RELEASE_TAG;
const repo = process.env.GH_REPO;
if (
  !source ||
  !destination ||
  !/^v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(tag ?? "") ||
  !repo
) {
  throw new Error("Expected artifact directories, a RELEASE_TAG and GH_REPO");
}
function files(dir) {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) =>
    entry.isDirectory()
      ? files(join(dir, entry.name))
      : [join(dir, entry.name)],
  );
}
mkdirSync(destination, { recursive: true });
const platforms = {};
for (const file of files(source)) {
  const name = basename(file).replaceAll(" ", ".");
  const target = join(destination, name);
  if (existsSync(target))
    throw new Error(`Duplicate release artifact: ${name}`);
  copyFileSync(file, target);
  const targets = name.endsWith(".app.tar.gz")
    ? ["darwin-aarch64", "darwin-x86_64"]
    : name.endsWith(".AppImage")
      ? ["linux-x86_64"]
      : name.endsWith(".exe")
        ? ["windows-x86_64"]
        : [];
  if (!targets.length) continue;
  const signature = readFileSync(`${file}.sig`, "utf8").trim();
  if (!signature) throw new Error(`Missing signature: ${name}`);
  for (const platform of targets) {
    if (platforms[platform])
      throw new Error(`Duplicate updater target: ${platform}`);
    platforms[platform] = {
      signature,
      url: `https://github.com/${repo}/releases/download/${tag}/${encodeURIComponent(name)}`,
    };
  }
}
for (const platform of [
  "darwin-aarch64",
  "darwin-x86_64",
  "windows-x86_64",
  "linux-x86_64",
]) {
  if (!platforms[platform])
    throw new Error(`Missing updater target: ${platform}`);
}
const notes = JSON.parse(
  readFileSync(process.env.RELEASE_NOTES_FILE, "utf8"),
).body;
writeFileSync(
  join(destination, "latest.json"),
  JSON.stringify(
    {
      version: tag.slice(1),
      notes,
      pub_date: new Date().toISOString(),
      platforms,
    },
    null,
    2,
  ) + "\n",
);
