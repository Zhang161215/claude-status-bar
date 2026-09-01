#!/usr/bin/env node
// Maps a Claude Code hook event to this session's file: ~/.claude/statusbar/state.d/<session_id>.json
// Usage: node update.js <prompt|pre|post|notify|permreq|stop|fail>

const fs = require("fs");
const os = require("os");
const path = require("path");
const cp = require("child_process");

const dir = path.join(os.homedir(), ".claude", "statusbar");
const stateDir = path.join(dir, "state.d");
// Written by the app's Quit menu item; suppresses the relaunch below so Quit sticks.
// lifecycle.js removes it on the next SessionStart (a new session = fresh consent).
const quitMarker = path.join(dir, "quit-intent");
const event = process.argv[2] || "";

const TOOL_LABELS = {
  Bash: "执行命令", Edit: "编辑文件", Write: "写入文件", MultiEdit: "编辑文件",
  NotebookEdit: "编辑文件", Read: "读取文件", Grep: "搜索中", Glob: "搜索中",
  WebFetch: "浏览网页", WebSearch: "联网搜索", Task: "分派任务",
  TodoWrite: "规划中",
};

const safeId = (s) => String(s || "").replace(/[^A-Za-z0-9_.-]/g, "").slice(0, 64) || "unknown";

let raw = "";
process.stdin.on("data", (d) => (raw += d));
process.stdin.on("end", () => {
  let p = {};
  try { p = JSON.parse(raw || "{}"); } catch {}

  // Off by default; CLAUDE_STATUSBAR_DEBUG=1 logs every hook invocation to hooks.log.
  if (process.env.CLAUDE_STATUSBAR_DEBUG === "1") {
    try {
      fs.mkdirSync(dir, { recursive: true });
      fs.appendFileSync(path.join(dir, "hooks.log"),
        `${new Date().toISOString()} [${event}] tool=${p.tool_name || "-"} mode=${p.permission_mode || "-"} msg=${JSON.stringify(p.message || "").slice(0, 160)} keys=${Object.keys(p).join(",")}\n`);
    } catch {}
  }

  // This session's own file is the unit of state AND the liveness marker. Writing it on any
  // event also tracks sessions that predate the hook install (never fired SessionStart).
  const sid = safeId(p.session_id);
  const statePath = path.join(stateDir, sid + ".json");

  let prev = {};
  try { prev = JSON.parse(fs.readFileSync(statePath, "utf8")); } catch {}

  const project = p.cwd ? path.basename(p.cwd) : prev.project || "";
  // The app reads <cwd>/.git/HEAD for the branch and disambiguates same-named projects by
  // parent folder; carried over from prev for events whose payload omits cwd.
  const cwd = p.cwd || prev.cwd || "";
  const ts = Math.floor(Date.now() / 1000);
  let state = "idle", label = "", startedAt = prev.startedAt || 0;

  switch (event) {
    case "prompt":
      state = "thinking"; label = "思考中…"; startedAt = ts; break;
    case "pre": {
      const t = p.tool_name || "";
      state = "tool"; label = TOOL_LABELS[t] || "调用工具";
      if (!startedAt) startedAt = ts;
      break;
    }
    case "post":
      state = "thinking"; label = "思考中…";
      if (!startedAt) startedAt = ts;
      break;
    case "notify": {
      // Only a permission prompt drives the icon here (CLI path; desktop uses permreq). Ignore
      // every other Notification (esp. the idle_prompt "Claude is waiting for your input") so the
      // icon rests instead of parking on a confusing "Waiting for you".
      const m = (p.message || "").toLowerCase();
      const isPerm = p.notification_type === "permission_prompt" ||
        m.includes("permission") || m.includes("approve") || m.includes("allow");
      if (!isPerm) return;
      // 保留本轮起点而不是抹成 0：applyTitle 的条件是 startedAt > 0，置 0 会让计时器在整个
      // 等待授权期间消失——而授权确认恰恰是最想知道"卡了多久"的时候。
      state = "permission"; label = "等待授权";
      if (!startedAt) startedAt = ts;
      break;
    }
    case "permreq":
      // Desktop-app permission signal; not redundant with notify (that's CLI-only).
      state = "permission"; label = "等待授权";
      if (!startedAt) startedAt = ts;
      break;
    case "stop":
      state = "done"; label = "完成"; startedAt = 0; break;
    case "fail": {
      // 两个来源共用：StopFailure（API 层错误，rate_limit/overloaded/server_error…，无 tool_name）
      // 和 PostToolUseFailure（工具调用失败，带 tool_name）。后者能指出是哪一步崩的，信息量更大。
      const t = p.tool_name || "";
      const what = TOOL_LABELS[t] || t;
      state = "error";
      label = what ? `${what}失败 (×_×)` : "出错了 (×_×)";
      startedAt = 0;
      break;
    }
    default:
      return;
  }

  // CLAUDE_CODE_ENTRYPOINT tags the surface running this session ("cli", "claude-desktop", …);
  // carried over from prev for the odd event where the env var isn't set.
  const entrypoint = process.env.CLAUDE_CODE_ENTRYPOINT || prev.entrypoint || "";
  // TERM_PROGRAM identifies the terminal app for a CLI session (Apple_Terminal, iTerm.app,
  // vscode, WezTerm, …); the app uses it to bring that terminal to the front on a row click.
  const termProgram = process.env.TERM_PROGRAM || prev.term_program || "";
  // process.ppid IS this session's `claude` process (verified: hooks are spawned directly by it,
  // stable for the session's life, on both CLI and desktop). The app uses kill(pid,0) for liveness.
  // started:true — any update.js event (prompt/tool/permission/stop) is real activity, so the session
  // graduates from "merely opened" to visible in the dropdown. Clicking a conversation never fires here.
  const out = { state, label, tool: p.tool_name || "", project, cwd, sessionId: p.session_id || "", transcript: p.transcript_path || prev.transcript || "", entrypoint, term_program: termProgram, pid: process.ppid, started: true, startedAt, ts };
  try {
    fs.mkdirSync(stateDir, { recursive: true });
    const tmp = statePath + "." + process.pid + ".tmp";
    fs.writeFileSync(tmp, JSON.stringify(out));
    fs.renameSync(tmp, statePath);
  } catch {}

  // Self-heal: a session with live state but no app to show it relaunches the app. Covers
  // install-while-a-session-is-already-open (that session never fires SessionStart, the only
  // other opener) and an app killed/crashed mid-session. Skipped after an explicit menu Quit.
  try {
    if (!fs.existsSync(quitMarker)) {
      cp.execSync("pgrep -x ClaudeStatusBar", { stdio: "ignore" });
    }
  } catch {
    try { cp.spawn("open", ["-g", "-b", "com.local.claudestatusbar"], { stdio: "ignore", detached: true }).unref(); } catch {}
  }
});
