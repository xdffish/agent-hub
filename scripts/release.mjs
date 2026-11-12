#!/usr/bin/env node
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync, spawn } from "node:child_process";
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
  console.log(`\n${color.cyan(color.bold("================================================"))}`);
  console.log(`  ${color.green(color.bold("🚀 Agent Hub 多平台构建与发布交互助手"))}`);
  console.log(`${color.cyan(color.bold("================================================"))}\n`);
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
  const semver = new RegExp(`^${numeric}\\.${numeric}\\.${numeric}(?:-${prerelease}(?:\\.${prerelease})*)?(?:\\+[0-9A-Za-z-]+(?:\\.[0-9A-Za-z-]+)*)?$`);
  if (!semver.test(newVer)) throw new Error("版本号必须符合 SemVer，例如 1.0.0 或 1.1.0-beta.1");
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
    cargoContent = cargoContent.replace(/(^\[package\]\s*$)([\s\S]*?)(?=^\[|$(?![\s\S]))/m,
      (section) => section.replace(/^(version\s*=\s*)"[^"]+"/m, (_, prefix) => {
        matched = true;
        return `${prefix}"${newVer}"`;
      }));
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
      });
    if (matched !== 1) throw new Error("Cargo.lock 中未找到唯一的本地应用包记录");
    stage(cargoLockPath, content);
  }
  for (const { path, content } of updates) {
    if (readFileSync(path, "utf-8") !== content) writeFileSync(path, content);
    console.log(`  ✓ ${color.green(path.slice(rootDir.length + 1))} -> ${newVer}`);
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

    const verChoice = await rl.question(color.cyan("\n请选择版本更新策略 [默认 1]: "));
    let targetVer = currentVer;
    if (verChoice.trim() === "2") targetVer = nextVers.patch;
    else if (verChoice.trim() === "3") targetVer = nextVers.minor;
    else if (verChoice.trim() === "4") targetVer = nextVers.major;
    else if (verChoice.trim() === "5") {
      const customVer = await rl.question(color.cyan("请输入自定义版本号 (例如 1.0.0): "));
      if (customVer.trim()) targetVer = customVer.trim();
    }

    if (targetVer === currentVer) {
      console.log(`${color.dim("保持版本号不变: " + targetVer)}`);
    }
    updateVersions(targetVer);

    // --- 第二步：目标平台选择 ---
    console.log(`\n${color.bold("【第 2 步】目标构建平台")}`);
    console.log(`  [1] ${color.bold("仅构建 macOS 产物")} (本机直接打包 .dmg / .app)`);
    console.log(`  [2] ${color.bold("仅构建 Windows 产物")} (生成 GitHub Actions 触发或说明)`);
    console.log(`  [3] ${color.bold("双端全量发布 (macOS + Windows)")} ${color.green("★ 推荐 GitHub Actions 矩阵云编译")}`);

    const platformChoice = (await rl.question(color.cyan("\n请选择构建目标 [默认 1]: "))).trim() || "1";

    if (platformChoice === "1") {
      // 本机 macOS 构建
      console.log(`\n${color.bold("【macOS 架构选择】")}`);
      console.log(`  [1] 默认/当前主机架构 (${process.arch})`);
      console.log(`  [2] Apple Silicon (aarch64-apple-darwin)`);
      console.log(`  [3] Intel x86_64 (x86_64-apple-darwin)`);
      console.log(`  [4] 通用二进制 Universal (x86_64 + aarch64 合并包)`);

      const archChoice = (await rl.question(color.cyan("\n请选择目标架构 [默认 1]: "))).trim() || "1";
      let targetFlag = "";
      if (archChoice === "2") targetFlag = "--target aarch64-apple-darwin";
      else if (archChoice === "3") targetFlag = "--target x86_64-apple-darwin";
      else if (archChoice === "4") targetFlag = "--target universal-apple-darwin";

      console.log(`\n${color.green("👉 即将执行打包命令:")} ${color.bold(`npx tauri build ${targetFlag}`)}`);
      const confirm = await rl.question(color.yellow("是否立即开始打包? (y/n) [默认 y]: "));
      if (confirm.trim().toLowerCase() === "n") {
        console.log(color.dim("打包操作已取消。"));
        return;
      }

      console.log(`\n${color.cyan("🚀 开始编译构建 macOS 产物，请稍候...")}\n`);
      const args = ["tauri", "build"];
      if (targetFlag) args.push(...targetFlag.split(" "));

      const child = spawn("npx", args, {
        cwd: rootDir,
        stdio: "inherit",
        shell: true,
      });

      child.on("close", (code) => {
        if (code === 0) {
          console.log(`\n${color.green(color.bold("🎉 macOS 打包成功！"))}`);
          console.log(`产物目录位于: ${color.cyan("src-tauri/target/release/bundle/dmg/")}`);
        } else {
          console.log(`\n${color.red(color.bold(`❌ 构建退出，退出代码: ${code}`))}`);
        }
      });
    } else if (platformChoice === "2" || platformChoice === "3") {
      // Windows 或 双端发布
      console.log(`\n${color.cyan(color.bold("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"))}`);
      console.log(`  ${color.yellow(color.bold("💡 跨平台编译架构建议 (macOS -> Windows)"))}`);
      console.log(`${color.cyan(color.bold("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"))}`);
      console.log(`Tauri 深度依赖原生 WebView2、C++ 运行库与 NSIS 打包器。`);
      console.log(`在 macOS 本机直接交叉编译 Windows 容易因动态链接和 SDK 缺失失败。`);
      console.log(`行业生产级的标准做法是使用 ${color.green("GitHub Actions 跨平台矩阵流水线")}：`);
      console.log(`  • macOS Runner: 原生编译生成 .dmg 与 .app`);
      console.log(`  • Windows Runner: 原生编译生成 .exe (NSIS) 与 .msi\n`);

      const createWorkflow = await rl.question(
        color.cyan("是否一键在工程中生成/更新 GitHub Actions 双端发布流水线 (.github/workflows/release.yml)? (y/n) [默认 y]: ")
      );

      if (createWorkflow.trim().toLowerCase() !== "n") {
        ensureReleaseWorkflow();
        console.log(`\n${color.green("✓ 已生成完整的跨平台自动构建工作流:")} ${color.bold(".github/workflows/release.yml")}`);
        console.log(`\n${color.bold("【如何触发双端自动云打包】:")}`);
        console.log(`  1. 提交本次版本修改:`);
        console.log(`     ${color.dim(`git add . && git commit -m "chore: release v${targetVer}"`)}`);
        console.log(`  2. 打标签并推送到 GitHub:`);
        console.log(`     ${color.green(`git tag v${targetVer} && git push origin v${targetVer}`)}`);
        console.log(`  3. GitHub Actions 将自动并行拉起 macOS 与 Windows 虚拟机完成双端打包，并在 GitHub Releases 发布安装包！`);
      }
    }
  } finally {
    rl.close();
  }
}

// 自动生成符合 Tauri v2 规范的 GitHub Actions 跨平台发布配置文件
function ensureReleaseWorkflow() {
  const workflowDir = resolve(rootDir, ".github/workflows");
  const workflowPath = resolve(workflowDir, "release.yml");
  const content = `name: 'Release Tauri App'

on:
  push:
    tags:
      - 'v*'
  workflow_dispatch:

jobs:
  release:
    permissions:
      contents: write
    strategy:
      fail-fast: false
      matrix:
        include:
          - platform: 'macos-latest'
            args: '--target universal-apple-darwin'
          - platform: 'windows-latest'
            args: ''

    runs-on: \${{ matrix.platform }}
    steps:
      - name: Checkout repository
        uses: actions/checkout@v4

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: 22
          cache: 'npm'

      - name: Install Rust stable
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: \${{ matrix.platform == 'macos-latest' && 'aarch64-apple-darwin,x86_64-apple-darwin' || '' }}

      - name: Rust cache
        uses: swatinem/rust-cache@v2
        with:
          workspaces: './src-tauri -> target'

      - name: Install dependencies
        run: npm install

      - name: Build and Publish Tauri Application
        uses: tauri-apps/tauri-action@v0
        env:
          GITHUB_TOKEN: \${{ secrets.GITHUB_TOKEN }}
        with:
          tagName: \${{ github.ref_name }}
          releaseName: 'Agent Hub v__VERSION__'
          releaseBody: 'See the assets to download this release and install.'
          releaseDraft: true
          prerelease: false
          args: \${{ matrix.args }}
`;
  writeFileSync(workflowPath, content);
}

main().catch((err) => {
  console.error(color.red("\n❌ 执行失败:"), err);
  process.exit(1);
});
