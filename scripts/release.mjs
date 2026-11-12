#!/usr/bin/env node
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";
import * as readline from "node:readline/promises";
import { stdin as input, stdout as output } from "node:process";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const rootDir = resolve(__dirname, "..");

// 终端色彩辅助
const color = {
  cyan: (t) => `\x1b[36m${t}\x1b[0m`,
  green: (t) => `\x1b[32m${t}\x1b[0m`,
  yellow: (t) => `\x1b[33m${t}\x1b[0m`,
  red: (t) => `\x1b[31m${t}\x1b[0m`,
  bold: (t) => `\x1b[1m${t}\x1b[0m`,
  dim: (t) => `\x1b[2m${t}\x1b[0m`,
};

function banner() {
  console.log(
    `\n${color.cyan(color.bold("================================================"))}`,
  );
  console.log(
    `  ${color.green(color.bold("🚀 Agent Hub 多平台构建与发布交互助手"))}`,
  );
  console.log(
    `${color.cyan(color.bold("================================================"))}\n`,
  );
}

// 读取当前版本
function getCurrentVersion() {
  const pkgPath = resolve(rootDir, "package.json");
  const pkg = JSON.parse(readFileSync(pkgPath, "utf-8"));
  return pkg.version;
}

// 计算建议的语义化版本号
function getNextVersions(current) {
  const parts = current.split(".").map(Number);
  const [major, minor, patch] = parts;
  return {
    patch: `${major}.${minor}.${patch + 1}`,
    minor: `${major}.${minor + 1}.0`,
    major: `${major + 1}.0.0`,
  };
}

// 先准备所有内容，再同步版本及锁文件；不改变依赖解析结果。
function updateVersions(newVer) {
  const numeric = "(?:0|[1-9]\\d*)";
  const prerelease = "(?:0|[1-9]\\d*|\\d*[A-Za-z-][0-9A-Za-z-]*)";
  const semver = new RegExp(
    `^${numeric}\\.${numeric}\\.${numeric}(?:-${prerelease}(?:\\.${prerelease})*)?(?:\\+[0-9A-Za-z-]+(?:\\.[0-9A-Za-z-]+)*)?$`,
  );
  if (!semver.test(newVer))
    throw new Error("版本号必须符合 SemVer，例如 1.0.0 或 1.1.0-beta.1");
  console.log(`\n${color.cyan("⚙️ 正在同步版本号到各配置文件...")}`);
  const updates = [];
  const stage = (path, content) => updates.push({ path, content });

  // 1. package.json
  const pkgPath = resolve(rootDir, "package.json");
  const pkg = JSON.parse(readFileSync(pkgPath, "utf-8"));
  pkg.version = newVer;
  stage(pkgPath, JSON.stringify(pkg, null, 2) + "\n");

  const lockPath = resolve(rootDir, "package-lock.json");
  if (existsSync(lockPath)) {
    const lock = JSON.parse(readFileSync(lockPath, "utf-8"));
    lock.version = newVer;
    if (lock.packages) {
      if (!lock.packages[""]) throw new Error("package-lock.json 缺少根包记录");
      lock.packages[""].version = newVer;
    }
    stage(lockPath, JSON.stringify(lock, null, 2) + "\n");
  }

  // 2. src-tauri/tauri.conf.json
  const tauriConfPath = resolve(rootDir, "src-tauri/tauri.conf.json");
  if (existsSync(tauriConfPath)) {
    const tauriConf = JSON.parse(readFileSync(tauriConfPath, "utf-8"));
    tauriConf.version = newVer;
    stage(tauriConfPath, JSON.stringify(tauriConf, null, 2) + "\n");
  }

  // 3. src-tauri/Cargo.toml (仅替换 [package] 下的 version)
  const cargoPath = resolve(rootDir, "src-tauri/Cargo.toml");
  if (existsSync(cargoPath)) {
    let cargoContent = readFileSync(cargoPath, "utf-8");
    let matched = false;
    cargoContent = cargoContent.replace(
      /(^\[package\]\s*$)([\s\S]*?)(?=^\[|$(?![\s\S]))/m,
      (section) =>
        section.replace(/^(version\s*=\s*)"[^"]+"/m, (_, prefix) => {
          matched = true;
          return `${prefix}"${newVer}"`;
        }),
    );
    if (!matched) throw new Error("Cargo.toml 缺少 package.version");
    stage(cargoPath, cargoContent);
  }

  const cargoLockPath = resolve(rootDir, "src-tauri/Cargo.lock");
  if (existsSync(cargoLockPath)) {
    let matched = 0;
    const content = readFileSync(cargoLockPath, "utf-8").replace(
      /^\[\[package\]\][\s\S]*?(?=^\[\[package\]\]|$(?![\s\S]))/gm,
      (section) => {
        const name = section.match(/^name\s*=\s*"([^"]+)"/m)?.[1];
        if (name !== pkg.name || /^source\s*=/m.test(section)) return section;
        return section.replace(/^(version\s*=\s*)"[^"]+"/m, (_, prefix) => {
          matched++;
          return `${prefix}"${newVer}"`;
        });
      },
    );
    if (matched !== 1)
      throw new Error("Cargo.lock 中未找到唯一的本地应用包记录");
    stage(cargoLockPath, content);
  }
  for (const { path, content } of updates) {
    if (readFileSync(path, "utf-8") !== content) writeFileSync(path, content);
    console.log(
      `  ✓ ${color.green(path.slice(rootDir.length + 1))} -> ${newVer}`,
    );
  }
}

async function main() {
  banner();
  const rl = readline.createInterface({ input, output });

  try {
    const currentVer = getCurrentVersion();
    const nextVers = getNextVersions(currentVer);

    // --- 第一步：版本号配置 ---
    console.log(`${color.bold("【第 1 步】版本管理")}`);
    console.log(`当前版本号: ${color.yellow(color.bold(currentVer))}`);
    console.log(`  [1] 保持当前版本 (${currentVer})`);
    console.log(`  [2] Patch 补丁升级 (${color.green(nextVers.patch)})`);
    console.log(`  [3] Minor 特性升级 (${color.green(nextVers.minor)})`);
    console.log(`  [4] Major 重大升级 (${color.green(nextVers.major)})`);
    console.log(`  [5] 自定义输入版本号`);

    const verChoice = await rl.question(
      color.cyan("\n请选择版本更新策略 [默认 1]: "),
    );
    let targetVer = currentVer;
    if (verChoice.trim() === "2") targetVer = nextVers.patch;
    else if (verChoice.trim() === "3") targetVer = nextVers.minor;
    else if (verChoice.trim() === "4") targetVer = nextVers.major;
    else if (verChoice.trim() === "5") {
      const customVer = await rl.question(
        color.cyan("请输入自定义版本号 (例如 1.0.0): "),
      );
      if (customVer.trim()) targetVer = customVer.trim();
    }

    if (targetVer === currentVer) {
      console.log(`${color.dim("保持版本号不变: " + targetVer)}`);
    }
    updateVersions(targetVer);

    const host = { darwin: "macOS", win32: "Windows", linux: "Linux" }[
      process.platform
    ];
    if (!host) throw new Error(`不支持的构建主机：${process.platform}`);
    console.log(`\n${color.bold("【第 2 步】构建方式")}`);
    console.log(`  [1] 在本机上构建 ${host} 安装包`);
    console.log("  [2] 准备 macOS、Windows、Linux 三平台 CI 打包");
    const choice = (await rl.question("请选择 [默认 1]: ")).trim() || "1";
    if (choice === "2") {
      const workflowPath = resolve(rootDir, ".github/workflows/release.yml");
      if (!existsSync(workflowPath))
        throw new Error("缺少 .github/workflows/release.yml");
      console.log("\n三平台打包配置：.github/workflows/release.yml");
      console.log(
        "提交变更并推送到 GitHub 后，可手动运行 Desktop packages 工作流。",
      );
      console.log(`也可通过推送版本标签 v${targetVer} 触发。`);
      console.log(
        "安装包保存在 Actions artifacts 中；工作流不会自动公开发布。",
      );
      return;
    }
    if (choice !== "1") throw new Error("请选择 1 或 2");
    const args = [
      resolve(rootDir, "node_modules/@tauri-apps/cli/tauri.js"),
      "build",
    ];
    if (process.platform === "darwin") {
      console.log(
        "\nmacOS 架构：[1] 本机 [2] Apple Silicon [3] Intel [4] Universal",
      );
      const arch = (await rl.question("请选择 [默认 1]: ")).trim() || "1";
      const targets = {
        2: "aarch64-apple-darwin",
        3: "x86_64-apple-darwin",
        4: "universal-apple-darwin",
      };
      if (!["1", "2", "3", "4"].includes(arch))
        throw new Error("无效的架构选项");
      if (targets[arch]) args.push("--target", targets[arch]);
    }
    const confirm = (
      await rl.question(`开始构建 ${host} 安装包？(y/n) [默认 y]: `)
    )
      .trim()
      .toLowerCase();
    if (confirm === "n") return;
    const result = spawnSync(process.execPath, args, {
      cwd: rootDir,
      stdio: "inherit",
    });
    if (result.error) throw result.error;
    if (result.status !== 0)
      throw new Error(`构建失败：${result.signal ?? result.status}`);
    console.log(
      "构建完成。安装包位于 src-tauri/target 下对应架构的 release/bundle 目录。",
    );
  } finally {
    rl.close();
  }
}

main().catch((err) => {
  console.error(color.red("\n❌ 执行失败:"), err);
  process.exit(1);
});
