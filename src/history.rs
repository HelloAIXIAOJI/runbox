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
///
/// 健壮性：对每条记录做清洗，剔除任何可能导致渲染或布局异常的脏数据——
/// 空白行、不可见控制字符、内部换行/制表符、超长垃圾行。
/// 保证单条异常记录不会影响其它历史条目或整个列表。
pub fn load() -> Vec<String> {
    let content = match fs::read_to_string(history_path()) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    // 单条历史上限（字符数），超长视为垃圾数据丢弃
    const MAX_LEN: usize = 512;

    content
        .lines()
        .map(sanitize_line)   // 清洗：去控制字符、压内部空白、去首尾空白
        .filter(|l| !l.is_empty())
        .filter(|l| l.chars().count() <= MAX_LEN)
        .collect()
}

/// 清洗单条历史行：
/// - 去掉所有不可见控制字符（包括 NUL、转义序列等，会破坏显示）；
/// - 把内部换行/制表符/连续空白压成单个空格（历史应是单行）；
/// - 去掉首尾空白。
fn sanitize_line(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    for ch in line.chars() {
        // 剔除控制字符（保留常见可见字符与普通空白）
        if ch.is_control() {
            // 换行/回车/制表符这类"格式性"控制字符用空格替代，其余直接丢弃
            if ch == '\n' || ch == '\r' || ch == '\t' {
                out.push(' ');
            }
            continue;
        }
        out.push(ch);
    }
    // 把连续空白压成单个空格
    let collapsed: String = out
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    collapsed.trim().to_string()
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
