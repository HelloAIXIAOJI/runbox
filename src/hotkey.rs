//! 全局快捷键（Super+R）呼出运行框。
//!
//! - Wayland：走 XDG Desktop Portal 的 `GlobalShortcuts`（ashpd），
//!   首次运行需在各桌面设置里授权绑定该快捷键；
//! - X11：直接 `XGrabKey`（x11rb），Mod4 + R。
//!
//! 无论哪种后端，回调都调度到 GTK 主线程执行。

use std::sync::Arc;

/// 热键回调类型：在 GTK 主线程执行。
pub type HotkeyCallback = Arc<dyn Fn() + Send + Sync + 'static>;

/// 启动热键监听线程。X11 与 Wayland 按会话类型自动选择。
pub fn start(on_hotkey: HotkeyCallback) {
    std::thread::spawn(move || {
        #[cfg(target_os = "linux")]
        {
            let is_wayland = std::env::var_os("WAYLAND_DISPLAY").is_some()
                || std::env::var("XDG_SESSION_TYPE")
                    .map(|s| s == "wayland")
                    .unwrap_or(false);
            if is_wayland {
                wayland_hotkey(on_hotkey);
            } else {
                x11_hotkey(on_hotkey);
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            drop(on_hotkey);
        }
    });
}

/// 把回调调度到 GTK 主线程执行。
///
/// 热键线程不是主线程，`idle_add_local` 会 panic；
/// 全局 `MainContext::invoke` 线程安全，可跨线程投递。
fn notify_main(f: HotkeyCallback) {
    gtk::glib::MainContext::default().invoke(move || f());
}

#[cfg(target_os = "linux")]
fn x11_hotkey(on_hotkey: HotkeyCallback) {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{ConnectionExt, GrabMode, KeyButMask, ModMask};
    use x11rb::protocol::Event;

    // X11 keysym：Super_L=0xffeb，Super_R=0xffec，'r'=0x72
    const XK_SUPER_L: u32 = 0xffeb;
    const XK_SUPER_R: u32 = 0xffec;
    const XK_R: u32 = 0x72;

    let (conn, screen_num) = match x11rb::connect(None) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("runbox: X11 连接失败：{e}");
            return;
        }
    };
    let setup = conn.setup();
    let root = setup.roots[screen_num].root;
    let min_kc = setup.min_keycode;
    let count = setup.max_keycode - min_kc + 1;

    let mapping = match conn.get_keyboard_mapping(min_kc, count) {
        Ok(cookie) => match cookie.reply() {
            Ok(reply) => reply,
            Err(e) => {
                eprintln!("runbox: 获取键盘映射失败：{e}");
                return;
            }
        },
        Err(e) => {
            eprintln!("runbox: 获取键盘映射失败：{e}");
            return;
        }
    };
    let per = mapping.keysyms_per_keycode as usize;
    if per == 0 {
        return;
    }

    // 从键盘映射里找出 Super 键与 R 键的 keycode
    let mut r_keycode = 0u8;
    let mut has_super = false;
    for i in 0..count as usize {
        let kc = min_kc + i as u8;
        let syms = &mapping.keysyms[i * per..(i + 1) * per];
        if syms.contains(&XK_SUPER_L) || syms.contains(&XK_SUPER_R) {
            has_super = true;
        }
        if syms.contains(&XK_R) {
            r_keycode = kc;
        }
    }
    if r_keycode == 0 {
        eprintln!("runbox: 找不到 R 键的 keycode");
        return;
    }
    if !has_super {
        eprintln!("runbox: 警告：键盘映射中未找到 Super 键");
    }

    // Mod4(Super)+R；再抓一个含 CapsLock 的组合，防止大写锁定状态下失灵
    let _ = conn.grab_key(false, root, ModMask::M4, r_keycode, GrabMode::ASYNC, GrabMode::ASYNC);
    let _ = conn.grab_key(false, root, ModMask::M4 | ModMask::LOCK, r_keycode, GrabMode::ASYNC, GrabMode::ASYNC);
    let _ = conn.flush();

    eprintln!("runbox: X11 热键已注册（Super+R）");

    loop {
        match conn.poll_for_event() {
            Ok(Some(Event::KeyPress(e))) => {
                if e.detail == r_keycode && e.state.contains(KeyButMask::MOD4) {
                    notify_main(on_hotkey.clone());
                }
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("runbox: X11 事件循环退出：{e}");
                break;
            }
        }
        // 轻微让步，避免忙轮询吃满 CPU
        std::thread::sleep(std::time::Duration::from_millis(4));
    }
}

#[cfg(target_os = "linux")]
fn wayland_hotkey(on_hotkey: HotkeyCallback) {
    use ashpd::desktop::global_shortcuts::{
        BindShortcutsOptions, GlobalShortcuts, NewShortcut,
    };
    use ashpd::desktop::CreateSessionOptions;
    use futures_util::StreamExt;

    let rt = match tokio::runtime::Runtime::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("runbox: tokio runtime 创建失败：{e}");
            return;
        }
    };
    rt.block_on(async move {
        let gs = match GlobalShortcuts::new().await {
            Ok(g) => g,
            Err(e) => {
                eprintln!("runbox: 连接 GlobalShortcuts portal 失败：{e}");
                return;
            }
        };
        let session = match gs.create_session(CreateSessionOptions::default()).await {
            Ok(s) => s,
            Err(e) => {
                eprintln!("runbox: 创建快捷键会话失败：{e}");
                return;
            }
        };
        let req = match gs
            .bind_shortcuts(
                &session,
                &[NewShortcut::new("runbox", "打开运行框")],
                None,
                BindShortcutsOptions::default(),
            )
            .await
        {
            Ok(r) => r,
            Err(e) => {
                eprintln!("runbox: 绑定快捷键失败：{e}");
                return;
            }
        };
        if let Err(e) = req.response() {
            eprintln!("runbox: 快捷键绑定被拒绝：{e}");
            return;
        }
        let mut activated = match gs.receive_activated().await {
            Ok(s) => s,
            Err(e) => {
                eprintln!("runbox: 订阅快捷键激活信号失败：{e}");
                return;
            }
        };
        eprintln!("runbox: Wayland 快捷键已注册，请在系统设置中为 runbox 绑定快捷键");
        while let Some(ev) = activated.next().await {
            if ev.shortcut_id() == "runbox" {
                notify_main(on_hotkey.clone());
            }
        }
    });
}
