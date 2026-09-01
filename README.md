# Claude Status Bar · 汉化优化版

在 macOS 菜单栏上显示 Claude Code 的实时状态：**橙灯干活中、黄灯等你授权、绿灯跑完了、红灯出错了**。

> **本项目 Fork 自 [m1ckc3s/claude-status-bar](https://github.com/m1ckc3s/claude-status-bar)**，原作者 **Mick Cesanek**，MIT 协议。
> 原版英文文档见 [README.en.md](README.en.md)。本分支做了完整中文化，并修复了若干上游问题、新增了状态配色等功能。
> 非官方项目，与 Anthropic 无关联。

---

## 相比原版做了什么

### 完整中文化

菜单、状态文案、工具标签全部汉化。思考词从原版 168 个英文动名词换成了 40 个中文词，可选带颜文字：

```
🔶 摸鱼中… (￣▽￣)  1m 23s
🔶 炼丹中… ♨        2m 07s
🟡 等待授权 (・・?)  0m 45s
🔴 编辑文件失败 (×_×)
```

### 新增功能

| 功能 | 说明 |
|---|---|
| **思考词四档风格** | 可爱（带颜文字）／简洁（纯中文）／英文原版／关闭 |
| **文字随状态变色** | 干活橙、等授权黄、完成绿、出错红，不用盯图标就能分辨 |
| **红灯：错误状态** | 接入 `StopFailure` + `PostToolUseFailure`，API 报错和工具失败都会亮红灯 |
| **完成绿灯** | 跑完亮 5 秒绿灯再退回静默，不长期占菜单栏 |
| **文字底色** | 半透明衬底，可调不透明度和圆角。菜单栏透明时彩色文字容易糊在壁纸上 |

### 修复的上游问题

这三个是原版（英文版同样存在）的真实缺陷：

1. **图标位置记不住** — 原版从未设置 `NSStatusItem.autosaveName`，这是 macOS 持久化状态项位置的唯一机制。缺了它，用户 Cmd+拖拽调整图标位置后，一重启就弹回默认位置。
2. **等待授权时计时器消失** — `update.js` 和 `main.swift` 两处都把 `startedAt` 硬编码成 `0`，而计时器的显示条件是 `startedAt > 0`。结果开了「显示计时器」也看不到，恰恰在最想知道「卡了多久」的授权确认环节。
3. **纯 CommandLineTools 环境编译失败** — `build.sh` 强制构建 x86_64 slice，而只装了 Command Line Tools（没装完整 Xcode）的机器缺 Swift 兼容库的 Intel 版，整个构建会失败。现在会自动降级为仅本机架构。

---

## 安装

### 下载构建好的版本

到 [Releases](https://github.com/Zhang161215/claude-status-bar/releases) 下载 `ClaudeStatusBar-zh.zip`，解压后把 `Claude Status Bar.app` 拖进「应用程序」，**打开一次**（这一步会自动写入 Claude Code hooks）。

> ⚠️ CI 构建的版本是 **ad-hoc 签名**（没有 Apple Developer ID），首次打开会被 Gatekeeper 拦。两种方式放行：
> - 右键点图标 → 打开 → 再点「打开」
> - 或执行：`xattr -dr com.apple.quarantine "/Applications/Claude Status Bar.app"`

### 从源码编译

```bash
git clone https://github.com/Zhang161215/claude-status-bar.git
cd claude-status-bar
./build.sh
cp -R "build/Claude Status Bar.app" /Applications/
open "/Applications/Claude Status Bar.app"
```

**要求**：macOS 12+、Node.js（hooks 脚本用）、Xcode Command Line Tools（`xcode-select --install`）。

装了完整 Xcode 会输出 arm64 + x86_64 通用二进制；只有 Command Line Tools 则自动降级为仅本机架构，照常可用。

---

## 使用说明

首次启动会把 hooks 合并进 `~/.claude/settings.json`（**改前自动备份**），并把脚本安装到 `~/.claude/statusbar/`。已经开着的 Claude Code 会话要等下一次提问或工具调用才会被识别。

### 菜单结构

点菜单栏图标展开：

```
会话
  <各个会话，点击可跳转到对应终端/编辑器窗口>

选项
  显示计时器            当前这轮已经跑了多久
  思考词 ▸              可爱（带颜文字）/ 简洁 / 英文原版 / 关闭
  动画 ▸                Claude Spark / Claude Code / 螃蟹漫步
  颜色 ▸                橙色 / 跟随系统
  文字底色 ▸            不透明度：关闭/淡/中/浓/重     圆角：直角/小/中/药丸
  完成提示音 ▸          关闭 / 每轮都响 / 超过 1、5、15 分钟

版本 x.y.z（汉化版）
退出
```

### 状态与配色

| 状态 | 图标 | 文字颜色 | 说明 |
|---|---|---|---|
| 思考／调用工具 | 动画图标 | 橙 `#d97757` | 显示滚动思考词和计时 |
| 等待授权 | 黄点 | 黄 `#f2ba2e` | 同时显示已等待时长 |
| 已完成 | 绿点 | 绿 `#4caf50` | 停留 5 秒后退回静默 |
| **出错** | 红点 | 红 `#e53935` | 一直亮到下轮活动，能指出是哪个工具失败 |
| 空闲 | 静默图标 | — | 不显示文字 |

选「颜色 → 跟随系统」时，文字会一并回到系统自适应色（深色菜单栏白字、浅色黑字）。

### 把图标挪到想要的位置

按住 **Command** 键拖动菜单栏图标即可。本版本补上了 `autosaveName`，位置能扛住重启（原版不能）。

如果你装了 Ice / Bartender / Thaw 这类菜单栏管理工具，图标顺序由它们接管，系统的位置记忆会被覆盖 —— 这种情况下直接拖拽，由管理工具记住。

### 卸载

菜单里退出，然后删掉 App，再执行 `node "/Applications/Claude Status Bar.app/Contents/Resources/uninstall.js"` 清理 hooks。或者手动从 `~/.claude/settings.json` 里移掉指向 `~/.claude/statusbar/` 的那几条。

---

## 工作原理

Claude Code 在会话生命周期的各个节点触发 hooks，`update.js` 把事件写成 `~/.claude/statusbar/state.d/<session_id>.json`，App 轮询这些文件并渲染菜单栏。

本版本共注册 10 个 hook：

| Hook | 对应状态 |
|---|---|
| `SessionStart` / `SessionEnd` | 会话出现／消失 |
| `UserPromptSubmit` | 开始思考 |
| `PreToolUse` / `PostToolUse` | 调用工具／回到思考 |
| `Notification` / `PermissionRequest` | 等待授权 |
| `Stop` | 完成 |
| **`StopFailure`** | **出错**（API 层：限流、过载、服务端错误等） |
| **`PostToolUseFailure`** | **出错**（工具调用失败，会带上是哪个工具） |

后两个是本分支新增的，原版没有接入任何错误事件 —— 这也是原版做不到「红灯」的原因。

**隐私**：不发送任何遥测。仅有的网络请求是每天一次向 GitHub 公共 API 检查更新。状态文件只包含项目路径、分支名、会话状态和时间戳，不含代码或对话内容。

---

## 自动构建

`.github/workflows/build.yml` 在 macOS runner 上构建：

- **推送 tag（`v*`）** → 构建并自动创建 Release，附带 zip
- **手动触发**（Actions 页面的 Run workflow）→ 构建并上传 artifact
- **PR / push 到主分支** → 只验证能否编译通过

发布新版本：

```bash
git tag v0.4.4-zh.1
git push origin v0.4.4-zh.1
```

---

## 协议与致谢

MIT，沿用原项目协议。核心实现与全部视觉资源均来自原作者 **[Mick Cesanek](https://github.com/m1ckc3s)**，本分支只是在其基础上做中文化和若干修补。

协议仅覆盖源代码，不授予 Anthropic 任何商标或品牌权利。本项目与 Anthropic 无从属关系，未获其背书或赞助。
