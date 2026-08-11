//! 运行历史：`~/.config/runbox/history`，每行一条，去重 + 上限 30 条。
//!
//! root 与普通用户的 HOME 不同，历史天然按身份分开存储，无需特殊处理。

use std::fs;
use std::path::PathBuf;

const MAX: usize = 30;

fn history_path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config"));
    base.join("runbox").join("history")
}

/// 读取全部历史（旧→新）。
pub fn load() -> Vec<String> {
    fs::read_to_string(history_path())
        .map(|s| s.lines().map(str::to_string).collect())
        .unwrap_or_default()
}

/// 记录一条命令：去重、移到最前、截断上限、写回磁盘。
pub fn record(cmdline: &str) {
    let cmdline = cmdline.trim();
    if cmdline.is_empty() {
        return;
    }
    let mut items = load();
    items.retain(|x| x != cmdline);
    items.insert(0, cmdline.to_string());
    items.truncate(MAX);

    let path = history_path();
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let _ = fs::write(path, items.join("\n") + "\n");
}
