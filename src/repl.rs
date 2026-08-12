//! REPL 模式：无 GUI 的命令行交互。
//!
//! 提供与图形界面一致的语义，但没有窗口：
//! - `> ` 提示符等待输入；
//! - 回车执行命令 → 成功记入历史并退出（等同 GUI 里回车执行后关窗）；
//! - 上下键在历史记录中切换（由 rustyline 提供）；
//! - 空行 / 直接回车 → 不执行，退出。
//!
//! 复用 `launch::run` 与 `history`，与 GUI 走同一套执行与记录逻辑。

use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

use crate::{history, launch};

/// 运行 REPL。返回进程退出码。
pub fn run() -> i32 {
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
    match rl.readline(prompt) {
        Ok(line) => {
            let cmdline = line.trim();
            if cmdline.is_empty() {
                return 0;
            }
            match launch::run(cmdline) {
                Ok(()) => {
                    history::record(cmdline);
                    0
                }
                Err(e) => {
                    eprintln!("{e}");
                    1
                }
            }
        }
        // Ctrl+C 或 Ctrl+D：视为放弃输入，正常退出。
        Err(ReadlineError::Interrupted | ReadlineError::Eof) => {
            println!();
            0
        }
        Err(e) => {
            eprintln!("runbox: 读取输入失败：{e}");
            1
        }
    }
}
