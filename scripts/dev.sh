#!/usr/bin/env bash
set -Eeuo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUNTIME_DIR="$ROOT/.dev-runtime"
PID_FILE="$RUNTIME_DIR/tauri-dev.pid"
LOG_FILE="$RUNTIME_DIR/tauri-dev.log"

is_running() {
  [[ -f "$PID_FILE" ]] || return 1
  local pid
  pid="$(<"$PID_FILE")"
  [[ "$pid" =~ ^[0-9]+$ ]] && kill -0 "$pid" 2>/dev/null
}

start_dev() {
  if is_running; then
    echo "开发服务已经运行 (PID $(<"$PID_FILE"))"
    return
  fi
  mkdir -p "$RUNTIME_DIR"
  rm -f "$PID_FILE"
  echo "正在启动 Tauri 开发模式…"
  (cd "$ROOT" && nohup npm run tauri dev) >>"$LOG_FILE" 2>&1 &
  echo "$!" >"$PID_FILE"
  sleep 2
  if is_running; then
    echo "已启动 (PID $(<"$PID_FILE"))"
    echo "日志：$LOG_FILE"
  else
    echo "启动失败，请查看日志：$LOG_FILE"
    rm -f "$PID_FILE"
    tail -n 30 "$LOG_FILE" || true
  fi
}

stop_dev() {
  if ! is_running; then
    echo "开发服务当前未运行。"
    rm -f "$PID_FILE"
    return
  fi
  local pid
  pid="$(<"$PID_FILE")"
  echo "正在停止开发服务 (PID $pid)…"
  kill -TERM "$pid" 2>/dev/null || true
  for _ in {1..10}; do
    kill -0 "$pid" 2>/dev/null || break
    sleep 1
  done
  if kill -0 "$pid" 2>/dev/null; then
    echo "进程未能及时退出；为避免误杀其他进程，未强制结束。"
    return 1
  fi
  rm -f "$PID_FILE"
  echo "已停止。"
}

show_status() {
  if is_running; then
    echo "运行中 (PID $(<"$PID_FILE"))"
  else
    echo "未运行"
    rm -f "$PID_FILE"
  fi
  echo "日志：$LOG_FILE"
}

while true; do
  echo
  echo "Agent Hub 开发调试"
  echo "1) 启动 / 重启开发模式"
  echo "2) 停止开发模式"
  echo "3) 查看运行状态"
  echo "4) 查看实时日志"
  echo "5) 退出"
  read -r -p "请选择 [1-5]: " choice || exit 0
  case "$choice" in
    1) start_dev ;;
    2) stop_dev ;;
    3) show_status ;;
    4) mkdir -p "$RUNTIME_DIR"; touch "$LOG_FILE"; tail -n 40 -f "$LOG_FILE" ;;
    5) exit 0 ;;
    *) echo "请输入 1 到 5。" ;;
  esac
done
