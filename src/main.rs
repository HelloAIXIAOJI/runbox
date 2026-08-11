mod history;
mod hotkey;
mod launch;
mod ui;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use gtk::prelude::*;
use gtk::{glib, Application};

fn main() -> glib::ExitCode {
    let app = Application::builder()
        .application_id("org.windowshit.runbox")
        .build();

    // 缓存当前窗口：热键重开时若窗口还在，只重新弹出，不新建。
    let current_window: Rc<RefCell<Option<gtk::ApplicationWindow>>> = Rc::new(RefCell::new(None));

    {
        let current_window = current_window.clone();
        app.connect_activate(move |app| {
            if let Some(w) = current_window.borrow().as_ref() {
                w.present();
                return;
            }

            // 执行逻辑：spawn → 成功记历史并关闭窗口，失败弹 Windows 风味报错
            let on_run: Rc<dyn Fn(&gtk::ApplicationWindow, &str)> = Rc::new(|window, cmdline| {
                match launch::run(cmdline) {
                    Ok(_) => {
                        history::record(cmdline);
                        window.close();
                    }
                    Err(e) => ui::show_error(window, &e),
                }
            });

            let window = ui::build(app, on_run);

            // 窗口关闭后清掉缓存，下次热键时重建
            let cached = current_window.clone();
            window.connect_close_request(move |_| {
                *cached.borrow_mut() = None;
                gtk::glib::Propagation::Proceed
            });

            *current_window.borrow_mut() = Some(window.clone());
            window.present();
        });
    }

    // 全局热键 Super+R：呼出运行框。
    // 热键在后台线程触发，GTK 对象不可跨线程（!Send）：
    // glib 的线程安全投递（invoke/idle_add）都要求回调 Send，
    // 而不要求 Send 的（idle_add_local）只能主线程调用。
    // 所以用 std mpsc：热键线程只发 ()，主线程 timeout 轮询收到后 activate。
    let (tx, rx) = std::sync::mpsc::channel::<()>();
    let on_hotkey = move || {
        let _ = tx.send(());
    };
    let on_hotkey: Arc<dyn Fn() + Send + Sync + 'static> = Arc::new(on_hotkey);
    hotkey::start(on_hotkey);

    let app_for_poll = app.clone();
    gtk::glib::timeout_add_local(std::time::Duration::from_millis(40), move || {
        match rx.try_recv() {
            Ok(()) => {
                app_for_poll.activate();
                gtk::glib::ControlFlow::Continue
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => gtk::glib::ControlFlow::Continue,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => gtk::glib::ControlFlow::Break,
        }
    });

    app.run()
}
