//! GTK4 界面：模仿 Windows 10 的运行对话框。
//!
//! 所有信号闭包采用手动 downgrade/upgrade（不用 glib::clone! 宏），
//! 原因：nightly 上 glib 0.20 的 clone! 旧语法被 deprecated 且多变量
//! 并列 `@weak X, @weak Y` 展开时降级行为不稳定。
//!
//! 窗口类型选择：`gtk::ApplicationWindow`（非 AdwWindow），
//! 这样能保留原生标题栏 + 关闭/最小化/最大化按钮，
//! 比 AdwWindow 的 client-side decoration 更贴近 Windows 原版。

use std::rc::Rc;

use gtk::gdk;
use gtk::prelude::*;
use gtk::{
    glib, Align, ApplicationWindow, Box, Button, EventControllerKey, FileChooserAction,
    FileChooserNative, Image, Label, ListBox, ListBoxRow, MessageDialog, Orientation,
    Popover, PositionType, ResponseType, SearchEntry, SelectionMode,
};

use crate::history;
use crate::launch;

/// 构建运行框窗口。
pub fn build(
    app: &gtk::Application,
    on_run: Rc<dyn Fn(&ApplicationWindow, &str)>,
) -> ApplicationWindow {
    let window = ApplicationWindow::builder()
        .application(app)
        .title(if launch::is_zh() { "运行" } else { "Run" })
        .default_width(380)
        .default_height(180)
        .resizable(false)
        .decorated(true)        // 显式打开 SSD 装饰（顶栏按钮）
        .deletable(true)        // 顶栏 X 按钮可关
        .build();

    // ── 顶部：图标 + 说明文字 ─────────────────────────────
    // 图标：用 theme fallback 链（同名有 symbolic 优先），保证各 Linux 桌面都有
    let icon = Image::from_icon_name("applications-system");
    icon.set_pixel_size(32);
    icon.set_valign(Align::Start);
    icon.set_icon_size(gtk::IconSize::Large);

    let explain = Label::new(Some(
        if launch::is_zh() {
            "Linux 将根据你所输入的名称，为你打开相应的\n程序。"
        } else {
            "Type the name of a program and Linux will open it."
        }
    ));
    explain.set_wrap(true);
    explain.set_xalign(0.0);
    explain.set_yalign(0.5);

    let header = Box::new(Orientation::Horizontal, 10);
    header.append(&icon);
    header.append(&explain);

    // ── 中间："打开(O):" + 输入框 + 下拉历史按钮 ──────────
    let label = Label::new(Some(if launch::is_zh() { "打开(O):" } else { "Open(O):" }));
    label.set_width_chars(9);
    label.set_xalign(1.0);
    label.set_valign(Align::Center);

    let entry = SearchEntry::new();
    entry.set_hexpand(true);
    // 自动填入上一次运行的命令（去重后最新的那条），省得每次重新敲
    if let Some(last) = history::load().into_iter().next() {
        entry.set_text(&last);
    }

    // 历史 popover（点击下拉按钮展开）
    let history_list = ListBox::new();
    history_list.set_selection_mode(SelectionMode::Single);
    refresh_history_list(&history_list);

    let popover = Popover::new();
    popover.set_position(PositionType::Bottom);
    popover.set_autohide(true);
    popover.set_child(Some(&history_list));

    let dropdown = gtk::MenuButton::new();
    dropdown.set_icon_name("pan-down-symbolic");
    dropdown.set_popover(Some(&popover));
    dropdown.set_tooltip_text(Some(if launch::is_zh() { "历史" } else { "History" }));

    let entry_box = Box::new(Orientation::Horizontal, 0);
    entry_box.append(&entry);
    entry_box.append(&dropdown);

    let input_row = Box::new(Orientation::Horizontal, 6);
    input_row.append(&label);
    input_row.append(&entry_box);

    // ── 底部按钮栏（右对齐） ──────────────────────────────
    let btn_browse = Button::with_label(if launch::is_zh() { "浏览(B)..." } else { "Browse(B)..." });
    let btn_cancel = Button::with_label(if launch::is_zh() { "取消" } else { "Cancel" });
    let btn_ok = Button::with_label(if launch::is_zh() { "确定" } else { "OK" });
    btn_ok.add_css_class("suggested-action");

    let buttons = Box::new(Orientation::Horizontal, 6);
    buttons.set_halign(Align::End);
    buttons.set_margin_top(4);
    buttons.append(&btn_browse);
    buttons.append(&btn_cancel);
    buttons.append(&btn_ok);

    // ── 装配 ──────────────────────────────────────────────
    let content = Box::new(Orientation::Vertical, 10);
    content.set_margin_top(12);
    content.set_margin_bottom(12);
    content.set_margin_start(12);
    content.set_margin_end(12);
    content.append(&header);
    content.append(&input_row);
    content.append(&buttons);
    window.set_child(Some(&content));

    // ── 事件（手动 downgrade/upgrade，避开 clone! 旧语法） ──

    // 确定：执行命令
    {
        let window = window.downgrade();
        let entry = entry.downgrade();
        let on_run = on_run.clone();
        btn_ok.connect_clicked(move |_| {
            let (Some(window), Some(entry)) = (window.upgrade(), entry.upgrade()) else {
                return;
            };
            let text = entry.text().to_string();
            if !text.trim().is_empty() {
                on_run(&window, &text);
            }
        });
    }

    // 取消：关闭窗口
    {
        let window = window.downgrade();
        btn_cancel.connect_clicked(move |_| {
            if let Some(window) = window.upgrade() {
                window.close();
            }
        });
    }

    // 浏览：打开文件选择器，选完填进输入框
    {
        let window = window.downgrade();
        let entry = entry.downgrade();
        btn_browse.connect_clicked(move |_| {
            let Some(window) = window.upgrade() else { return; };
            let dialog = FileChooserNative::new(
                Some(if launch::is_zh() { "浏览" } else { "Browse" }),
                Some(&window),
                FileChooserAction::Open,
                None,
                None,
            );
            let entry = entry.clone();
            dialog.connect_response(move |dialog, response| {
                if response == ResponseType::Accept {
                    if let Some(file) = dialog.file() {
                        if let Some(path) = file.path() {
                            if let Some(entry) = entry.upgrade() {
                                entry.set_text(&path.to_string_lossy());
                            }
                        }
                    }
                }
                dialog.destroy();
            });
            dialog.show();
        });
    }

    // 回车执行（与确定按钮等价）
    {
        let window = window.downgrade();
        let entry_weak = entry.downgrade();
        let on_run = on_run.clone();
        entry.connect_activate(move |_| {
            let (Some(window), Some(entry)) = (window.upgrade(), entry_weak.upgrade()) else {
                return;
            };
            let text = entry.text().to_string();
            if !text.trim().is_empty() {
                on_run(&window, &text);
            }
        });
    }

    // 历史行点击 → 填入并关闭 popover
    {
        let entry = entry.downgrade();
        let popover = popover.clone();
        history_list.connect_row_activated(move |_, row| {
            let text = row_text(row);
            if !text.is_empty() {
                if let Some(entry) = entry.upgrade() {
                    entry.set_text(&text);
                }
                popover.popdown();
            }
        });
    }

    // 输入变化 → 过滤历史
    let history_list_weak = history_list.downgrade();
    entry.connect_search_changed(move |e| {
        let q = e.text().to_lowercase();
        if let Some(list) = history_list_weak.upgrade() {
            refresh_history_list_filtered(&list, &q);
        }
    });

    // Esc 关闭（popover 打开时优先关 popover），↓ 打开历史 popover
    let key = EventControllerKey::new();
    key.set_propagation_phase(gtk::PropagationPhase::Capture);
    {
        let window = window.downgrade();
        let popover = popover.clone();
        let history_list = history_list.clone();
        key.connect_key_pressed(move |_, keyval, _, _| {
            let Some(window) = window.upgrade() else {
                return glib::Propagation::Stop;
            };
            match keyval {
                gdk::Key::Escape => {
                    if popover.is_visible() {
                        popover.popdown();
                    } else {
                        window.close();
                    }
                    glib::Propagation::Stop
                }
                gdk::Key::Down if !popover.is_visible() => {
                    refresh_history_list(&history_list);
                    popover.popup();
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        });
    }
    entry.add_controller(key);

    window
}

/// 读取全部历史填充列表。
fn refresh_history_list(list: &ListBox) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
    for line in history::load() {
        append_history_row(list, &line);
    }
}

/// 按过滤词筛选历史。
fn refresh_history_list_filtered(list: &ListBox, filter: &str) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
    for line in history::load() {
        if filter.is_empty() || line.to_lowercase().contains(filter) {
            append_history_row(list, &line);
        }
    }
}

fn append_history_row(list: &ListBox, text: &str) {
    let row = ListBoxRow::new();
    let label = Label::new(Some(text));
    label.set_xalign(0.0);
    label.set_margin_start(8);
    label.set_margin_end(8);
    label.set_margin_top(4);
    label.set_margin_bottom(4);
    row.set_child(Some(&label));
    list.append(&row);
}

fn row_text(row: &ListBoxRow) -> String {
    row.child()
        .and_then(|c| c.downcast::<Label>().ok())
        .map(|l| l.label().to_string())
        .unwrap_or_default()
}

/// 错误对话框（Windows 风味文案）。
///
/// GTK4 改了行为：按按钮后不会自动关闭，必须手动 `destroy()`。
/// 这里把所有响应（按钮 / Esc / 顶栏关闭）都路由到 destroy。
pub fn show_error(window: &ApplicationWindow, message: &str) {
    let dialog = MessageDialog::new(
        Some(window),
        gtk::DialogFlags::MODAL,
        gtk::MessageType::Error,
        gtk::ButtonsType::Ok,
        message,
    );
    dialog.set_title(Some(if launch::is_zh() { "运行错误" } else { "Error" }));
    dialog.connect_response(|dialog, _| dialog.destroy());
    dialog.present();
}