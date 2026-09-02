// Claude Code 状态浮窗（跨平台）
// 状态和外观都不自己定：状态读 ~/.claude/statusbar/state.d/*.json（hooks 写的），
// 外观读同目录的 config.json（菜单栏 App 写的）。所以菜单栏改设置，这边跟着变。

mod crab;

use eframe::egui;
use serde::Deserialize;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const WIN_H_MIN: f32 = 34.0; // 窗口最小高度；图标调大时窗口跟着长高
const GAP: f32 = 7.0;
const PAD_X: f32 = 11.0;
const DOT: f32 = 16.0; // 圆点模式的图标宽度
// 内置螃蟹原图 36px 高，默认取精确一半——像素风落在非整数倍上边缘会糊出光晕。
// 自定义 GIF 是插画，往往需要更大才看得清，所以实际高度可由 config 的 animHeight 覆盖。
const ICON_H_DEFAULT: f32 = 18.0;

#[derive(Deserialize, Debug, Default, Clone)]
#[serde(default)]
struct Session {
    state: String,
    label: String,
    #[serde(rename = "startedAt")]
    started_at: f64,
    ts: f64,
}

// 菜单栏 App 写的共享外观配置
#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
struct Cfg {
    #[serde(rename = "backdropAlpha")]
    backdrop_alpha: f32,
    #[serde(rename = "backdropRadius")]
    backdrop_radius: f32,
    #[serde(rename = "animStyle")]
    anim_style: String,
    // 菜单栏那边的「浮窗」开关。关掉时这个进程自己退出，不用菜单栏去 kill。
    #[serde(rename = "floatWindow")]
    float_window: bool,
    // 动画图标高度。内置像素螃蟹 18 就够，精细插画通常要 24~32 才看得清。
    #[serde(rename = "animHeight")]
    anim_height: f32,
    // 播放帧率。0 = 用 GIF 自带的帧延迟；>0 则覆盖它。
    // 待机动画一轮 3 秒左右才自然，16 帧的片子跑到 12fps 就成了原地乱扭。
    #[serde(rename = "animFps")]
    anim_fps: f32,
    // codex 宠物 id。"auto" 或空 = 自动跟随 Codex 当前用的那只；"none" = 不用宠物。
    // 素材不随本项目分发，只读用户自己装好的，版权归各宠物作者。
    #[serde(rename = "petId")]
    pet_id: String,
    // 跟菜单栏的「显示计时器」同一个开关
    #[serde(rename = "showTimer")]
    show_timer: bool,
    // 菜单栏的「颜色 → 跟随系统」。为真时文字改用中性色，跟菜单栏图标变成
    // 自适应黑白保持一致，而不是继续用彩色状态字。
    #[serde(rename = "iconSystem")]
    icon_system: bool,
    // 思考词风格由 update.js 消费（词是它选好写进 label 的），浮窗用不到。
    // 显式声明出来只为了不落进 extra 触发误报。
    #[serde(rename = "wordStyle")]
    _word_style: Option<String>,
    // 兜住本端没有对应字段的键。菜单栏加了新设置而这边忘了跟进时，
    // 它会落到这里而不是被静默丢弃，启动日志里能一眼看到。
    #[serde(flatten)]
    extra: std::collections::HashMap<String, serde_json::Value>,
}

impl Default for Cfg {
    fn default() -> Self {
        // float_window 默认 true：配置里没这个键时（老版本/首次运行）应当正常显示，
        // 只有菜单栏明确写了 false 才退出。
        Self {
            backdrop_alpha: 0.22,
            backdrop_radius: 5.0,
            anim_style: "web".into(),
            float_window: true,
            anim_height: ICON_H_DEFAULT,
            anim_fps: 0.0,
            pet_id: "auto".into(),
            show_timer: true,
            icon_system: false,
            _word_style: None,
            extra: std::collections::HashMap::new(),
        }
    }
}

fn now() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

fn statusbar_dir() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude/statusbar"))
}

fn read_cfg() -> Cfg {
    let Some(p) = statusbar_dir().map(|d| d.join("config.json")) else {
        return Cfg::default();
    };
    std::fs::read_to_string(p)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

// 只在启动时报一次：配置里有本端不认识的键，通常意味着菜单栏加了新设置而浮窗没跟进
fn warn_unknown_keys(cfg: &Cfg) {
    if !cfg.extra.is_empty() {
        let mut keys: Vec<_> = cfg.extra.keys().cloned().collect();
        keys.sort();
        eprintln!("配置中有本端未使用的键: {}", keys.join(", "));
    }
}

fn read_sessions() -> Vec<Session> {
    let Some(dir) = statusbar_dir().map(|d| d.join("state.d")) else {
        return vec![];
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return vec![];
    };
    entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| std::fs::read_to_string(e.path()).ok())
        .filter_map(|s| serde_json::from_str::<Session>(&s).ok())
        .collect()
}

// 与菜单栏版一致：出错 > 等授权 > 干活 > 其他
fn priority(state: &str) -> u8 {
    match state {
        "error" => 3,
        "permission" => 2,
        "thinking" | "tool" => 1,
        _ => 0,
    }
}

fn color_of(state: &str) -> egui::Color32 {
    match state {
        "error" => egui::Color32::from_rgb(229, 57, 53),
        "permission" => egui::Color32::from_rgb(242, 186, 46),
        "thinking" | "tool" => egui::Color32::from_rgb(217, 119, 87),
        // 完成、空闲、未知状态一律绿：没在跑就是"没问题"，跟红黄绿那套灯语一致
        _ => egui::Color32::from_rgb(76, 175, 80),
    }
}

fn elapsed(secs: i64) -> String {
    if secs >= 60 { format!("{}m {}s", secs / 60, secs % 60) } else { format!("{}s", secs) }
}

// 一段序列帧动画。内置螃蟹和用户自带的 GIF 走同一套结构。
struct Anim {
    texs: Vec<egui::TextureHandle>,
    fps: f32,
    aspect: f32, // 宽/高，用来按固定高度反推宽度
}

impl Anim {
    // fps_override > 0 时用它，否则用 GIF 自带的帧率
    fn frame_at(&self, t: f64, fps_override: f32) -> Option<&egui::TextureHandle> {
        if self.texs.is_empty() {
            return None;
        }
        let fps = if fps_override > 0.0 { fps_override } else { self.fps };
        self.texs.get(((t * fps as f64) as usize) % self.texs.len())
    }
}

// 用户自定义动画：把任意 GIF 放成 ~/.claude/statusbar/anim.gif 即可，
// 存在就优先用它，删掉就回到内置螃蟹。播放速度直接取 GIF 自己的帧延迟。
fn load_gif(ctx: &egui::Context) -> Option<Anim> {
    use image::AnimationDecoder;
    let p = statusbar_dir()?.join("anim.gif");
    let f = std::fs::File::open(p).ok()?;
    let dec = image::codecs::gif::GifDecoder::new(std::io::BufReader::new(f)).ok()?;
    let frames = dec.into_frames().collect_frames().ok()?;
    if frames.is_empty() {
        return None;
    }
    let (num, den) = frames[0].delay().numer_denom_ms();
    let delay = if den > 0 { num as f32 / den as f32 } else { 0.0 };
    let fps = if delay > 1.0 { 1000.0 / delay } else { 12.0 };
    let mut aspect = 1.0;
    let texs: Vec<_> = frames
        .iter()
        .enumerate()
        .map(|(i, fr)| {
            let buf = fr.buffer();
            let (w, h) = (buf.width(), buf.height());
            aspect = w as f32 / h as f32;
            let ci =
                egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], buf.as_raw());
            ctx.load_texture(format!("gif{i}"), ci, egui::TextureOptions::NEAREST)
        })
        .collect();
    eprintln!("自定义动画: {} 帧 @ {:.1}fps", texs.len(), fps);
    Some(Anim { texs, fps, aspect })
}

// codex-pets 的雪碧图规格（逆向得出，官方无文档）：
// 单帧 192x208、固定 8 列、9 或 11 行（spriteVersionNumber 1/2 的差别）；
// 每行是一段独立动作，帧数不等，行尾用空白格填满 8 列。
const PET_FW: u32 = 192;
const PET_FH: u32 = 208;

// 雪碧图的 9 个标准动作行，取自 Codex 的 hatch-pet skill（SKILL.md）：
//   0 idle  1 running-right  2 running-left  3 waving  4 jumping
//   5 failed  6 waiting  7 running  8 review
// 行 9-10 是 16 个朝向，静态窗口用不上。
// 映射依据 skill 里对各状态的定义，例如 waiting 的原文是
// "show that Codex needs approval, help, or user input"，正好是我们的 permission。
fn row_name(r: usize) -> &'static str {
    ["idle","running-right","running-left","waving","jumping","failed","waiting","running","review"]
        .get(r).copied().unwrap_or("look")
}

const ROW_WAVING: usize = 3;

fn pet_row_for(state: &str) -> usize {
    match state {
        "__hover" => ROW_WAVING, // 鼠标悬停：挥手
        "error" => 5,      // failed
        "permission" => 6, // waiting
        "tool" => 7,       // running
        "thinking" => 8,   // review
        "done" => 3,       // waving
        _ => 0,            // idle
    }
}

// Codex 把当前选中的宠物写在 ~/.codex/config.toml 的 selected-avatar-id，
// 形如 "custom:<pet-id>"；内置宠物则是 "codex"，那个没有本地素材可读。
// （注意别用 global-state 里的 first-awake-pet-notification-avatar-ids —— 那是
// 安装历史的累积列表，不随切换更新，取它会永远停在第一只。）
fn detect_codex_pet() -> Option<String> {
    let home = dirs::home_dir()?;
    if let Ok(toml) = std::fs::read_to_string(home.join(".codex/config.toml")) {
        for line in toml.lines() {
            let l = line.trim();
            if !l.starts_with("selected-avatar-id") {
                continue;
            }
            let Some(val) = l.split('=').nth(1) else { continue };
            let val = val.trim().trim_matches('"');
            return match val.strip_prefix("custom:") {
                // 选的是自定义宠物：装了才用
                Some(id) => pet_sheet_path(id).map(|_| id.to_owned()),
                // 选的是内置 codex 宠物，本地没有素材，交给 GIF/螃蟹兜底
                None => None,
            };
        }
    }
    // config 里没写过：扫目录取第一个，好过什么都不显示
    let dir = codex_pets_dir()?;
    let mut names: Vec<_> = std::fs::read_dir(dir).ok()?
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| pet_sheet_path(n).is_some())
        .collect();
    names.sort();
    names.into_iter().next()
}

fn codex_pets_dir() -> Option<std::path::PathBuf> {
    if let Ok(h) = std::env::var("CODEX_HOME") {
        return Some(std::path::PathBuf::from(h).join("pets"));
    }
    dirs::home_dir().map(|h| h.join(".codex/pets"))
}

fn pet_sheet_path(id: &str) -> Option<std::path::PathBuf> {
    let p = codex_pets_dir()?.join(id).join("spritesheet.webp");
    p.exists().then_some(p)
}

// 切出指定行，丢掉行尾的空白格。传入已解码的图，避免每次换动作都重新解 WebP。
fn slice_row(ctx: &egui::Context, img: &image::RgbaImage, row: usize) -> Option<Anim> {
    let (w, h) = img.dimensions();
    let cols = (w / PET_FW) as usize;
    let rows = (h / PET_FH) as usize;
    if cols == 0 || row >= rows {
        return None;
    }
    let mut texs = Vec::new();
    for c in 0..cols {
        let sub = image::imageops::crop_imm(
            img, c as u32 * PET_FW, row as u32 * PET_FH, PET_FW, PET_FH).to_image();
        // 抽样判断是不是空白格：整行末尾通常是填充
        let opaque = sub.pixels().step_by(97).filter(|p| p.0[3] > 8).count();
        if opaque < 3 {
            continue;
        }
        let ci = egui::ColorImage::from_rgba_unmultiplied(
            [PET_FW as usize, PET_FH as usize], sub.as_raw());
        texs.push(ctx.load_texture(format!("pet{row}_{c}"), ci, egui::TextureOptions::NEAREST));
    }
    if texs.is_empty() {
        return None;
    }
    Some(Anim { texs, fps: 6.0, aspect: PET_FW as f32 / PET_FH as f32 })
}

fn load_crab(ctx: &egui::Context) -> Anim {
    let mut aspect = 51.0 / 36.0;
    let texs: Vec<_> = crab::CRAB_FRAMES
        .iter()
        .enumerate()
        .filter_map(|(i, bytes)| {
            let img = image::load_from_memory(bytes).ok()?.to_rgba8();
            let (w, h) = img.dimensions();
            aspect = w as f32 / h as f32;
            let ci = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], img.as_raw());
            // NEAREST 而不是 LINEAR：像素风精灵图用线性过滤会把抗锯齿边缘插值成
            // 一圈半透明光晕，看上去就是「白边」。
            Some(ctx.load_texture(format!("crab{i}"), ci, egui::TextureOptions::NEAREST))
        })
        .collect();
    Anim { texs, fps: 12.0, aspect }
}

struct App {
    lead: Option<Session>,
    cfg: Cfg,
    last_poll: f64,
    last_w: f32, // 宽度真变了才发 InnerSize，否则每帧下发会抖
    anim: Option<Anim>,                       // 非宠物来源（自定义 GIF / 内置螃蟹）
    anim_is_custom: bool,
    gif_mtime: Option<std::time::SystemTime>, // GIF 换了就重新加载，不用重启
    pet_img: Option<image::RgbaImage>,        // 宠物雪碧图，整张只解码一次
    pet_loaded_id: String,
    pet_rows: std::collections::HashMap<usize, Anim>, // 行 -> 动画，按需切分并缓存
}

impl App {
    fn new() -> Self {
        Self {
            lead: None,
            cfg: read_cfg(),
            last_poll: 0.0,
            last_w: 0.0,
            anim: None,
            anim_is_custom: false,
            gif_mtime: None,
            pet_img: None,
            pet_loaded_id: String::new(),
            pet_rows: std::collections::HashMap::new(),
        }
    }

    fn poll(&mut self) {
        let t = now();
        if t - self.last_poll < 0.4 {
            return; // 跟菜单栏版一样 0.4s 一轮，别每帧读盘
        }
        self.last_poll = t;
        self.cfg = read_cfg();
        self.lead = read_sessions()
            .into_iter()
            .max_by(|a, b| priority(&a.state).cmp(&priority(&b.state)).then(a.ts.total_cmp(&b.ts)))
            .filter(|s| if s.state == "done" { now() - s.ts < 5.0 } else { s.state != "idle" });
    }

    fn gif_mtime_now() -> Option<std::time::SystemTime> {
        std::fs::metadata(statusbar_dir()?.join("anim.gif")).ok()?.modified().ok()
    }

    // 是否使用宠物由菜单栏的「动画」样式决定：选了内置动画就不用宠物，
    // 这样两端由同一个开关控制，不会出现菜单栏是螃蟹、浮窗是猫的情况。
    fn effective_pet_id(&self) -> Option<String> {
        if self.cfg.anim_style != "pet" {
            return None;
        }
        let want = self.cfg.pet_id.trim();
        if want.eq_ignore_ascii_case("none") {
            return None;
        }
        if want.is_empty() || want.eq_ignore_ascii_case("auto") {
            detect_codex_pet()
        } else {
            pet_sheet_path(want).map(|_| want.to_owned())
        }
    }

    // 取当前状态该用的那一行动画。宠物在所有状态下都显示，只是换动作。
    fn pet_anim(&mut self, ctx: &egui::Context, state: &str) -> Option<&Anim> {
        let id = self.effective_pet_id()?;
        if id != self.pet_loaded_id {
            let path = pet_sheet_path(&id)?;
            self.pet_img = image::open(&path).ok().map(|i| i.to_rgba8());
            self.pet_loaded_id = id.clone();
            self.pet_rows.clear();
            eprintln!("codex 宠物: {id}");
        }
        let row = pet_row_for(state);
        if !self.pet_rows.contains_key(&row) {
            let img = self.pet_img.as_ref()?;
            // 有些宠物只有 9 行（v1），越界时退回 idle 行
            let a = slice_row(ctx, img, row).or_else(|| slice_row(ctx, img, 0))?;
            eprintln!("  行{row} ({}): {} 帧", row_name(row), a.texs.len());
            self.pet_rows.insert(row, a);
        }
        self.pet_rows.get(&row)
    }

    // 没有宠物时的退路：自定义 GIF > 内置螃蟹
    fn fallback_anim(&mut self, ctx: &egui::Context) -> Option<&Anim> {
        let mt = Self::gif_mtime_now();
        if self.anim.is_none() || mt != self.gif_mtime {
            self.gif_mtime = mt;
            self.anim = match load_gif(ctx) {
                Some(a) => { self.anim_is_custom = true; Some(a) }
                None => { self.anim_is_custom = false; Some(load_crab(ctx)) }
            };
        }
        self.anim.as_ref()
    }
}

impl eframe::App for App {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0] // 窗口全透明，可见部分只有下面那颗胶囊
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll();

        // 菜单栏把开关关掉了就自行退出。由被控方主动退出，菜单栏那边不必去找 pid 杀进程。
        if !self.cfg.float_window {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        let (state, text, started_at) = match &self.lead {
            Some(s) => (s.state.clone(), s.label.clone(), s.started_at),
            None => ("idle".into(), "空闲 (－ω－)".into(), 0.0),
        };
        // 选了「跟随系统」就不再用彩色状态字，改中性浅灰，跟菜单栏那边的自适应图标呼应
        let tint = if self.cfg.icon_system {
            egui::Color32::from_gray(225)
        } else {
            color_of(&state)
        };
        let animating = matches!(state.as_str(), "thinking" | "tool");
        let t = ctx.input(|i| i.time);
        // 没有活跃会话时收成"纯宠物"形态：不显示文字、去掉胶囊底、把宠物放大，
        // 让它更像桌面上的一只宠物而不是一条状态条。
        let idle_only = self.lead.is_none();
        let base_h = self.cfg.anim_height.clamp(10.0, 64.0);
        let icon_h = if idle_only { (base_h * 1.8).clamp(16.0, 110.0) } else { base_h };
        // 空闲态窗口只包住宠物本身，留一点余量给缩放误差
        let win_h = if idle_only { icon_h + 4.0 } else { WIN_H_MIN.max(icon_h + 12.0) };

        let timer_text = if self.cfg.show_timer && started_at > 0.0 {
            elapsed((now() - started_at) as i64)
        } else {
            String::new()
        };

        // 宠物在所有状态下都显示，只换动作行；没有宠物时才退回 GIF/螃蟹，
        // 那两者只是待机装饰，没有分状态的素材，所以仍只在忙碌时出现。
        let fps_override = self.cfg.anim_fps;
        let has_pet = self.effective_pet_id().is_some();
        // 鼠标移上来就挥手打招呼，跟 Codex 里戳宠物的手感对齐
        let hovered = ctx.input(|i| i.pointer.latest_pos())
            .map(|p| ctx.screen_rect().contains(p))
            .unwrap_or(false)
            && ctx.input(|i| i.pointer.has_pointer());
        let anim_state = if hovered { "__hover".to_string() } else { state.clone() };
        let (sprite, crab_w) = if has_pet {
            match self.pet_anim(ctx, &anim_state) {
                Some(a) => (a.frame_at(t, fps_override).cloned(), (icon_h * a.aspect).round().max(8.0)),
                None => (None, DOT),
            }
        } else if animating && (Self::gif_mtime_now().is_some() || self.cfg.anim_style == "crab") {
            match self.fallback_anim(ctx) {
                Some(a) => (a.frame_at(t, fps_override).cloned(), (icon_h * a.aspect).round().max(8.0)),
                None => (None, DOT),
            }
        } else {
            (None, DOT)
        };

        // 先量文字再定窗口宽：无边框固定尺寸窗口不主动改就会留一截空白
        let f_main = egui::FontId::proportional(14.0);
        let f_timer = egui::FontId::monospace(13.0);
        let (text_w, timer_w) = ctx.fonts(|f| {
            let a = f.layout_no_wrap(text.clone(), f_main.clone(), tint).size().x;
            let b = if timer_text.is_empty() {
                0.0
            } else {
                f.layout_no_wrap(timer_text.clone(), f_timer.clone(), tint).size().x
            };
            (a, b)
        });
        let want_w = if idle_only {
            (crab_w + 4.0).ceil().max(16.0)
        } else {
            (PAD_X * 2.0
                + crab_w
                + GAP
                + text_w
                + if timer_w > 0.0 { GAP + timer_w } else { 0.0 }
                + 2.0)
                .ceil()
                .max(96.0)
        };
        if (want_w - self.last_w).abs() > 1.0 {
            self.last_w = want_w;
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(want_w, win_h)));
        }

        // 底色和圆角都取菜单栏那边的设置，两处外观保持一致
        let alpha = (self.cfg.backdrop_alpha.clamp(0.0, 1.0) * 255.0) as u8;
        let radius = self.cfg.backdrop_radius.max(0.0).min(win_h / 2.0);
        let panel = if idle_only {
            // 纯宠物形态：不画底，桌面直接透出来
            egui::Frame::none().inner_margin(egui::Margin::symmetric(2.0, 0.0))
        } else {
            egui::Frame::none()
                .fill(egui::Color32::from_rgba_unmultiplied(20, 20, 22, alpha.max(8)))
                .rounding(egui::Rounding::same(radius))
                .inner_margin(egui::Margin::symmetric(PAD_X, 0.0))
        };

        egui::CentralPanel::default().frame(panel).show(ctx, |ui| {
            // 全部改用 painter 手绘。用 label/horizontal_centered 时，行高由字体的
            // ascent/descent 决定，中文字体这两个值不对称，控件矩形居中了、墨迹看着仍偏上。
            // painter 的 Align2::LEFT_CENTER 直接按文字中线对齐 cy，视觉上才是正的。
            let rect = ui.max_rect();
            let cy = rect.center().y;
            let mut x = rect.left();

            if let Some(tex) = &sprite {
                let r = egui::Rect::from_center_size(
                    egui::pos2(x + crab_w / 2.0, cy),
                    egui::vec2(crab_w, icon_h),
                );
                egui::Image::new(tex).paint_at(ui, r);
                x += crab_w;
            } else {
                let c = egui::pos2(x + DOT / 2.0, cy);
                if animating {
                    let n = 28;
                    let start = (t * 2.2) as f32;
                    let sweep = std::f32::consts::TAU * 0.72;
                    let r = DOT * 0.38;
                    let pts: Vec<egui::Pos2> = (0..=n)
                        .map(|i| {
                            let a = start + sweep * (i as f32 / n as f32);
                            c + egui::vec2(a.cos() * r, a.sin() * r)
                        })
                        .collect();
                    ui.painter().add(egui::Shape::line(pts, egui::Stroke::new(2.2, tint)));
                } else {
                    ui.painter().circle_filled(c, DOT * 0.31, tint);
                }
                x += DOT;
            }

            // 按墨迹中心对齐，而不是按行盒中心。Align2::LEFT_CENTER 居中的是 galley.rect
            // （含字体 ascent+descent 的完整行高），中文的 descent 留得很大却几乎不用，
            // 结果盒子居中了、字看着偏上 4pt。mesh_bounds 是真实字形包围盒，拿它才准。
            let draw_centered = |p: &egui::Painter, gx: f32, g: std::sync::Arc<egui::Galley>, col: egui::Color32| -> f32 {
                let dy = g.mesh_bounds.center().y;
                let w = g.rect.width();
                p.galley(egui::pos2(gx, cy - dy), g, col);
                gx + w
            };

            // 空闲态到此为止，只留宠物
            if !idle_only {
            x += GAP;
            let g_main = ctx.fonts(|f| f.layout_no_wrap(text.clone(), f_main.clone(), tint));
            let after = draw_centered(ui.painter(), x, g_main, tint);

            if !timer_text.is_empty() {
                // 计时器与状态同色，只压暗一档，不引入第二种配色
                let dim = tint.gamma_multiply(0.75);
                let g_t = ctx.fonts(|f| f.layout_no_wrap(timer_text.clone(), f_timer.clone(), dim));
                draw_centered(ui.painter(), after + GAP, g_t, dim);
            }
            } // end !idle_only

            // 整块可拖：无边框窗口没标题栏，不给拖拽区就挪不动
            let hit =
                ui.interact(ui.max_rect(), ui.id().with("drag"), egui::Sense::click_and_drag());
            if hit.drag_started() {
                ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
        });

        // 有动画时要够顺滑，静止时没必要空转
        ctx.request_repaint_after(Duration::from_millis(if animating { 33 } else { 500 }));
    }
}

// 单一字体盖不全颜文字，得叠一条回退链，顺序即优先级：
//   Hiragino       中文本体，以及 ・ノー﹏～￣
//   Menlo          ✧ ✦ ✍ ♨ ∀（Hiragino 没有）
//   Arial Unicode  兜底。半角片假名标点 ･(U+FF65) ｡(U+FF61) 只有它有 ——
//                  注意 ･ 与全角的 ・(U+30FB) 是两个码位，词库里用的是半角那个。
//                  放最后是因为它中文字形不如 Hiragino，只在前面都缺字形时才轮到。
fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let candidates: &[(&str, &str)] = &[
        ("cjk", "/System/Library/Fonts/Hiragino Sans GB.ttc"),
        ("sym", "/System/Library/Fonts/Menlo.ttc"),
        ("uni", "/System/Library/Fonts/Supplemental/Arial Unicode.ttf"),
        ("cjk_w", "C:\\Windows\\Fonts\\msyh.ttc"),
        ("sym_w", "C:\\Windows\\Fonts\\seguisym.ttf"),
        ("uni_w", "C:\\Windows\\Fonts\\arialuni.ttf"),
        ("cjk_l", "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc"),
    ];
    let mut loaded: Vec<String> = vec![];
    for (name, path) in candidates {
        if let Ok(data) = std::fs::read(path) {
            fonts.font_data.insert((*name).to_owned(), egui::FontData::from_owned(data));
            loaded.push((*name).to_owned());
            eprintln!("字体: {path}");
        }
    }
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        let list = fonts.families.entry(family).or_default();
        for (i, n) in loaded.iter().enumerate() {
            list.insert(i, n.clone());
        }
    }
    ctx.set_fonts(fonts);
}

fn main() -> eframe::Result<()> {
    // 初始窗口高度也要照配置来，否则启动瞬间会闪一下默认尺寸再跳变
    let boot = read_cfg();
    let boot_h = WIN_H_MIN.max(boot.anim_height.clamp(10.0, 64.0) + 12.0);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([200.0, boot_h])
            .with_decorations(false)
            .with_transparent(true)
            .with_always_on_top()
            .with_resizable(false)
            .with_taskbar(false),
        ..Default::default()
    };
    eframe::run_native(
        "Claude 状态浮窗",
        options,
        Box::new(|cc| {
            install_fonts(&cc.egui_ctx);
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            warn_unknown_keys(&boot);
            Ok(Box::new(App::new()))
        }),
    )
}
