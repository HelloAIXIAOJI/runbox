//! REPL 模式：无 GUI 的命令行交互。
//!
//! 与图形界面共享同一套执行与历史逻辑，但没有窗口：
//! - 启动时打印与 GUI 一致的说明文字；
//! - `> ` 提示符等待输入；
//! - 回车执行命令 → 成功记入历史并退出（等同 GUI 里回车执行后关窗）；
//! - 失败 → 打印错误信息，并继续等待下一次输入；
//! - 上下键在历史记录中切换（由 rustyline 提供）；
//! - 空行 / 直接回车 → 不执行，退出；Ctrl+C / Ctrl+D → 退出。

use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

use crate::{history, launch};

/// 运行 REPL。返回进程退出码。
pub fn run() -> i32 {
    // 打印与 GUI 一致的说明文字（中英文跟随 LANG）。
    println!(
        "{}",
        if launch::is_zh() {
            "Linux 将根据你所输入的名称，为你打开相应的程序、文件夹、文档或 Internet 资源。"
        } else {
            "Type the name of a program, folder, document, or Internet resource you want to open, and Linux will open it."
        }
    );

    let mut rl = match DefaultEditor::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("runbox: 无法初始化命令行输入：{e}");
            return 1;
        }
    };

    // 预载历史，让 ↑/↓ 能翻到之前的记录。
    for line in history::load() {
        let _ = rl.add_history_entry(line.as_str());
    }

    let prompt = "> ";
    loop {
        let line = match rl.readline(prompt) {
            Ok(line) => line,
            // Ctrl+C 或 Ctrl+D：放弃输入，正常退出。
            Err(ReadlineError::Interrupted | ReadlineError::Eof) => {
                println!();
                return 0;
            }
            Err(e) => {
                eprintln!("runbox: 读取输入失败：{e}");
                return 1;
            }
        };

        let cmdline = line.trim();
        if cmdline.is_empty() {
            return 0;
        }

        match launch::run(cmdline) {
            Ok(()) => {
                history::record(cmdline);
                return 0;
            }
            Err(e) => {
                // 失败：打印错误，继续循环等待下一次输入。
                eprintln!("{e}");
                println!();
            }
        }
    }
}
