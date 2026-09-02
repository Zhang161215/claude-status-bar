// Claude Code 状态浮窗（跨平台）
// 状态和外观都不自己定：状态读 ~/.claude/statusbar/state.d/*.json（hooks 写的），
// 外观读同目录的 config.json（菜单栏 App 写的）。所以菜单栏改设置，这边跟着变。

mod crab;

use eframe::egui;
use serde::Deserialize;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const WIN_H: f32 = 34.0;
const GAP: f32 = 7.0;
const PAD_X: f32 = 11.0;
const DOT: f32 = 16.0; // 圆点模式的图标宽度
// 素材原图 36px 高，取它的精确一半。像素风一旦落在非整数倍上，边缘就会糊出一圈光晕。
const CRAB_H: f32 = 18.0;

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
    fn frame_at(&self, t: f64) -> Option<&egui::TextureHandle> {
        if self.texs.is_empty() {
            return None;
        }
        self.texs.get(((t * self.fps as f64) as usize) % self.texs.len())
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
    anim: Option<Anim>,
    anim_is_custom: bool,
    gif_mtime: Option<std::time::SystemTime>, // GIF 换了就重新加载，不用重启
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
        }
    }

    fn gif_mtime_now() -> Option<std::time::SystemTime> {
        std::fs::metadata(statusbar_dir()?.join("anim.gif")).ok()?.modified().ok()
    }

    // 自定义 GIF 优先；文件被换掉或删掉时自动重载
    fn anim(&mut self, ctx: &egui::Context) -> Option<&Anim> {
        let mt = Self::gif_mtime_now();
        let stale = self.anim.is_none() || mt != self.gif_mtime;
        if stale {
            self.gif_mtime = mt;
            self.anim = match load_gif(ctx) {
                Some(a) => {
                    self.anim_is_custom = true;
                    Some(a)
                }
                None => {
                    self.anim_is_custom = false;
                    Some(load_crab(ctx))
                }
            };
        }
        self.anim.as_ref()
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
        let tint = color_of(&state);
        let animating = matches!(state.as_str(), "thinking" | "tool");
        let t = ctx.input(|i| i.time);

        let timer_text =
            if started_at > 0.0 { elapsed((now() - started_at) as i64) } else { String::new() };

        // 放了自定义 GIF 就无条件用它（特意放的文件即意图），否则看菜单栏选的是不是螃蟹
        let want_sprite =
            animating && (Self::gif_mtime_now().is_some() || self.cfg.anim_style == "crab");
        let (sprite, crab_w) = if want_sprite {
            match self.anim(ctx) {
                Some(a) => (a.frame_at(t).cloned(), (CRAB_H * a.aspect).round().max(8.0)),
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
        let want_w = (PAD_X * 2.0
            + crab_w
            + GAP
            + text_w
            + if timer_w > 0.0 { GAP + timer_w } else { 0.0 }
            + 2.0)
            .ceil()
            .max(96.0);
        if (want_w - self.last_w).abs() > 1.0 {
            self.last_w = want_w;
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(want_w, WIN_H)));
        }

        // 底色和圆角都取菜单栏那边的设置，两处外观保持一致
        let alpha = (self.cfg.backdrop_alpha.clamp(0.0, 1.0) * 255.0) as u8;
        let radius = self.cfg.backdrop_radius.max(0.0).min(WIN_H / 2.0);
        let panel = egui::Frame::none()
            .fill(egui::Color32::from_rgba_unmultiplied(20, 20, 22, alpha.max(8)))
            .rounding(egui::Rounding::same(radius))
            .inner_margin(egui::Margin::symmetric(PAD_X, 0.0));

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
                    egui::vec2(crab_w, CRAB_H),
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

            x += GAP;
            let g_main = ctx.fonts(|f| f.layout_no_wrap(text.clone(), f_main.clone(), tint));
            let after = draw_centered(ui.painter(), x, g_main, tint);

            if !timer_text.is_empty() {
                // 计时器与状态同色，只压暗一档，不引入第二种配色
                let dim = tint.gamma_multiply(0.75);
                let g_t = ctx.fonts(|f| f.layout_no_wrap(timer_text.clone(), f_timer.clone(), dim));
                draw_centered(ui.painter(), after + GAP, g_t, dim);
            }

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

// 单一字体盖不全：中文和 ・ノー﹏～￣ 靠 Hiragino，✧ ✦ ✍ ♨ ∀ 只有 Menlo 有。
// 按顺序全部塞进同一个 family，egui 会在前一个缺字形时自动往后找。
fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let candidates: &[(&str, &str)] = &[
        ("cjk", "/System/Library/Fonts/Hiragino Sans GB.ttc"),
        ("sym", "/System/Library/Fonts/Menlo.ttc"),
        ("cjk_w", "C:\\Windows\\Fonts\\msyh.ttc"),
        ("sym_w", "C:\\Windows\\Fonts\\seguisym.ttf"),
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
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([200.0, WIN_H])
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
            Ok(Box::new(App::new()))
        }),
    )
}
