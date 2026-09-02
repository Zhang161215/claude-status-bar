// 平台差异集中在这里。hooks 的核心逻辑（状态映射、写 JSON）本来就跨平台，
// 需要分支的只有"检测进程在不在"和"把 App 拉起来"这两件事。
//
// macOS：菜单栏 App（Swift），按 bundle id 用 open 拉起。
// Windows：托盘程序（Rust），没有 bundle 概念，只能按可执行文件路径启动。

const fs = require("fs");
const os = require("os");
const path = require("path");
const cp = require("child_process");

const isWin = process.platform === "win32";

// 进程名。macOS 上是 bundle 里的可执行文件名，Windows 上带 .exe
const EXEC = isWin ? "claude-statusbar.exe" : "ClaudeStatusBar";
const BUNDLE_ID = "com.local.claudestatusbar";

// Windows 没有统一的 App 安装位置，按约定顺序找：
//   1. 环境变量显式指定（便携版 / 自定义安装路径）
//   2. %LOCALAPPDATA%\ClaudeStatusBar\
//   3. %PROGRAMFILES%\ClaudeStatusBar\
function winExePath() {
  const explicit = process.env.CLAUDE_STATUSBAR_EXE;
  if (explicit && fs.existsSync(explicit)) return explicit;
  const roots = [
    process.env.LOCALAPPDATA && path.join(process.env.LOCALAPPDATA, "ClaudeStatusBar"),
    process.env.PROGRAMFILES && path.join(process.env.PROGRAMFILES, "ClaudeStatusBar"),
  ].filter(Boolean);
  for (const r of roots) {
    const p = path.join(r, EXEC);
    if (fs.existsSync(p)) return p;
  }
  return null;
}

// 状态栏/托盘程序是否在运行
function isRunning() {
  try {
    if (isWin) {
      // tasklist 找不到时不会返回非零，得看输出里有没有进程名
      const out = cp.execSync(`tasklist /FI "IMAGENAME eq ${EXEC}" /NH`, {
        encoding: "utf8",
        stdio: ["ignore", "pipe", "ignore"],
      });
      return out.toLowerCase().includes(EXEC.toLowerCase());
    }
    cp.execSync(`pgrep -x ${EXEC}`, { stdio: "ignore" });
    return true;
  } catch {
    return false;
  }
}

// 后台拉起，不抢焦点
function launch() {
  try {
    if (isWin) {
      const exe = winExePath();
      if (!exe) return false;
      // start 的第一个引号参数是窗口标题，必须留着占位，否则带空格的路径会被当成标题
      cp.spawn("cmd", ["/c", "start", "", "/min", exe], {
        stdio: "ignore",
        detached: true,
        windowsHide: true,
      }).unref();
    } else {
      cp.spawn("open", ["-g", "-b", BUNDLE_ID], { stdio: "ignore", detached: true }).unref();
    }
    return true;
  } catch {
    return false;
  }
}

function killApp() {
  try {
    if (isWin) {
      cp.execSync(`taskkill /IM ${EXEC} /F`, { stdio: "ignore" });
    } else {
      cp.execSync(`pkill -x ${EXEC}`, { stdio: "ignore" });
    }
  } catch {}
}

// macOS 早期版本用 launchd 常驻，卸载时要注销；Windows 上没有对应物，直接跳过
function removeLaunchAgent(label) {
  if (isWin) return;
  try {
    cp.execSync(`launchctl bootout gui/${process.getuid()}/${label}`, { stdio: "ignore" });
  } catch {}
}

module.exports = { isWin, EXEC, BUNDLE_ID, isRunning, launch, killApp, removeLaunchAgent, winExePath };
