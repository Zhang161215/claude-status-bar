// 系统托盘。只在非 macOS 平台编译 —— macOS 那边菜单栏由 Swift App 负责，
// 两个都显示会重复。Windows 上没有菜单栏，托盘就是主入口。
//
// 托盘图标用状态色画的圆点：Windows 托盘只有 16x16，宠物的精细美术缩到那个尺寸
// 认不出来，纯色点反而一眼能分辨。

use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    Icon, TrayIcon, TrayIconBuilder,
};

pub struct Tray {
    _icon: TrayIcon,
    pub id_toggle_float: String,
    pub id_quit: String,
    last_state: String,
}

fn dot_rgba(size: u32, rgb: (u8, u8, u8)) -> Vec<u8> {
    let mut buf = vec![0u8; (size * size * 4) as usize];
    let c = size as f32 / 2.0;
    let r = size as f32 * 0.38;
    for y in 0..size {
        for x in 0..size {
            let (dx, dy) = (x as f32 + 0.5 - c, y as f32 + 0.5 - c);
            let d = (dx * dx + dy * dy).sqrt();
            // 边缘做一像素抗锯齿，否则 16x16 下锯齿很扎眼
            let a = if d <= r {
                255.0
            } else if d <= r + 1.0 {
                (r + 1.0 - d) * 255.0
            } else {
                0.0
            };
            let i = ((y * size + x) * 4) as usize;
            buf[i] = rgb.0;
            buf[i + 1] = rgb.1;
            buf[i + 2] = rgb.2;
            buf[i + 3] = a as u8;
        }
    }
    buf
}

fn icon_for(state: &str) -> Option<Icon> {
    let rgb = match state {
        "error" => (229, 57, 53),
        "permission" => (242, 186, 46),
        "thinking" | "tool" => (217, 119, 87),
        _ => (76, 175, 80), // 完成/空闲：绿
    };
    Icon::from_rgba(dot_rgba(32, rgb), 32, 32).ok()
}

impl Tray {
    pub fn new() -> Option<Self> {
        let menu = Menu::new();
        let toggle = MenuItem::new("显示/隐藏浮窗", true, None);
        let quit = MenuItem::new("退出", true, None);
        let id_toggle_float = toggle.id().0.clone();
        let id_quit = quit.id().0.clone();
        menu.append(&toggle).ok()?;
        menu.append(&PredefinedMenuItem::separator()).ok()?;
        menu.append(&quit).ok()?;

        let icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Claude Status Bar")
            .with_icon(icon_for("idle")?)
            .build()
            .ok()?;

        Some(Self { _icon: icon, id_toggle_float, id_quit, last_state: "idle".into() })
    }

    // 状态变了才换图标：托盘图标更新是系统调用，没必要每帧都下发
    pub fn sync(&mut self, state: &str, label: &str) {
        if state == self.last_state {
            return;
        }
        self.last_state = state.to_owned();
        if let Some(ic) = icon_for(state) {
            let _ = self._icon.set_icon(Some(ic));
        }
        let _ = self._icon.set_tooltip(Some(if label.is_empty() {
            "Claude Status Bar".to_string()
        } else {
            label.to_string()
        }));
    }

    // 取出待处理的菜单点击
    pub fn poll_menu() -> Vec<String> {
        let mut out = Vec::new();
        while let Ok(ev) = MenuEvent::receiver().try_recv() {
            out.push(ev.id.0);
        }
        out
    }
}
