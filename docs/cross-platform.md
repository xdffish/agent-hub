# 桌面平台适配

更新日期：2026-11-12。

Agent Hub 面向 macOS、Windows 和 Linux。平台相关实现集中在 `src-tauri/src/platform.rs` 和 `src-tauri/src/platform/windows.rs`，业务层保留相同的预览、备份、应用与撤销流程。

## 实现范围

| 项目       | macOS / Linux                                 | Windows                                                                                         |
| ---------- | --------------------------------------------- | ----------------------------------------------------------------------------------------------- |
| 文件权限   | 私有目录 `0700`、备份及配置文件 `0600`        | 受保护 DACL，仅当前用户与 SYSTEM 可访问                                                         |
| 临时进程   | 独立进程组，结束或超时后清理                  | 挂起启动，加入带终止策略的 Job Object 后恢复，关闭句柄时清理进程树                              |
| 链接与路径 | 拒绝写入含符号链接的路径                      | 同时拒绝 junction 等 reparse point，支持 Unicode、UNC 与长路径                                  |
| 程序发现   | macOS 应用目录、Linux 常见安装位置，以及 PATH | 用户级与系统级安装位置，以及 PATH 中的 `.exe`、`.cmd`、`.bat`                                   |
| MCP 命令   | 保留受限环境及显式配置的环境变量              | 另保留用户目录、AppData、临时目录、系统及命令解释器所需变量；支持从配置的 PATH 查找命令包装脚本 |
| 系统操作   | 使用 Tauri 原生文件选择器和 opener            | 使用同一组跨平台插件                                                                            |
| 界面       | 显示实际操作系统                              | 同时处理反斜杠路径和项目名称                                                                    |

Tauri capabilities 对三平台的本地 `main` 窗口开放相同的应用命令，不授权远程页面，也不向前端开放通用文件、Shell 或 HTTP 权限。

配置路径仍按各 Agent 的用户目录和项目目录组织，并遵循 `CODEX_HOME`。应用发现使用已知安装位置和 PATH，不遍历整台机器，也不执行 Agent 来判断安装状态。自定义安装位置、容器与 WSL 中的 Agent 不保证能被宿主机发现。

## Skill 交换格式

快照内的相对路径统一使用 `/`。导入时拒绝路径穿越、Windows 设备名、数据流和其他无法安全表示的名称；Windows 还拒绝大小写重复路径及文件与子目录冲突。

Windows 不提供 Unix 可执行位，导入时会将该字段按本机语义归一化。依赖可执行位的脚本从 Windows 再导出到 Unix 后，需要检查其执行权限。脚本和 MCP 命令本身仍须适用于目标系统，应用不会转换 Shell、PowerShell 或平台专属二进制文件。

## 构建与 CI

`.github/workflows/check.yml` 在 macOS、Windows 和 Ubuntu 24.04 上运行前端构建、Rust 格式检查、测试、类型绑定一致性检查和桌面应用编译。

`.github/workflows/release.yml` 在检查通过后生成以下安装包：

| 构建目标                                 | 产物                        |
| ---------------------------------------- | --------------------------- |
| macOS Universal（Apple Silicon / Intel） | `.app`、`.dmg`，上传 `.dmg` |
| Windows x64                              | NSIS `.exe`                 |
| Linux x64（Ubuntu 24.04）                | `.deb`、`.rpm`、`.AppImage` |

安装包工作流可手动执行，也可由 `v*` 标签触发。产物上传到 Actions artifacts，不自动创建公开 Release。当前未配置代码签名、macOS 公证或自动更新。

`npm run release` 可在三个系统上同步版本并运行本机构建；通过 Node.js 直接调用 Tauri CLI，避免依赖 Unix Shell。`scripts/dev.sh` 是 macOS/Linux 的可选开发辅助脚本；Windows 使用 `npm run tauri dev`。

构建环境请参照 [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)。Windows 进程管理依据 [Microsoft Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects)。

## 验证记录与限制

本轮在 macOS 主机上完成前端构建、Rust 编译检查、31 项测试及 `.app` 调试包构建。新增测试覆盖嵌套目录交换、已有文件替换、非法跨平台文件名，以及进程树清理。Windows 专用测试覆盖受保护 ACL、长路径、命令包装脚本查找、junction 拒绝和大小写冲突；这些专用测试已通过交叉编译检查，尚未在 Windows 上执行。

平台层、文件快照及数据模型还通过独立检查工程，针对 `x86_64-pc-windows-gnu` 和 `x86_64-unknown-linux-gnu` 进行交叉编译检查。该检查覆盖原生 API 的类型及平台分支，不包含 Tauri、WebView 或完整安装包链接。

当前仓库未配置远程地址，三平台 CI 尚未实际运行；本机也没有可用的 Windows/Linux 运行环境。因此，Windows/Linux 的完整应用构建、安装、原生对话框和真实 Agent/MCP 操作仍需在对应系统验收。交叉编译检查不能替代这些运行验证。
