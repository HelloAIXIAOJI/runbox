//! 命令执行层。
//!
//! 设计定稿（与用户确认）：
//! - 不用 shell，直接 `Command::spawn`；
//! - 子进程继承 runbox 的启动身份与环境（`sudo runbox` = root 执行，普通启动 = 当前用户）；
//! - 不做 Windows→Linux 命令映射，输入什么执行什么；
//! - 失败时给出 Windows 风味的报错。

use std::process::{Command, Stdio};

/// 界面语言是否为中文（跟随 LANG 环境变量）。
pub fn is_zh() -> bool {
    std::env::var("LANG")
        .unwrap_or_default()
        .to_lowercase()
        .starts_with("zh")
}

/// 取用户主目录。优先 `HOME`，取不到时退回 `/`。
fn home_dir() -> std::path::PathBuf {
    std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::PathBuf::from("/"))
}

/// 解析并执行一条命令行。
///
/// 成功返回 `Ok(())`，失败返回面向用户的错误信息（可直接弹对话框）。
pub fn run(cmdline: &str) -> Result<(), String> {
    // shlex 只做 POSIX 风格分词（引号处理），不做任何变量展开 / 管道 / 重定向
    let argv = match shlex::split(cmdline) {
        Some(v) if !v.is_empty() => v,
        _ => {
            return Err(if is_zh() {
                "命令不能为空。".to_string()
            } else {
                "The command cannot be empty.".to_string()
            })
        }
    };

    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..]);
    // 固定当前工作目录为用户主目录，保证无论从哪里启动 runbox，
    // 执行的命令都在 $HOME 下运行（贴近桌面启动的直觉）。
    cmd.current_dir(home_dir());
    // GUI 启动的程序不需要挂着终端 stdin
    cmd.stdin(Stdio::null());
    // 关键：这里不设置 user/group/env，子进程完整继承 runbox 的启动身份与环境。

    match cmd.spawn() {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            if is_zh() {
                Err(format!(
                    "'{}' 不是内部或外部命令，也不是可运行的程序或批处理文件。",
                    argv[0]
                ))
            } else {
                Err(format!(
                    "'{0}' is not recognized as an internal or external command,\noperable program or batch file.",
                    argv[0]
                ))
            }
        }
        Err(e) => Err(if is_zh() {
            format!("无法启动 '{}'：{e}", argv[0])
        } else {
            format!("Failed to start '{}': {e}", argv[0])
        }),
    }
}

/// 解析并以 root 身份执行一条命令行。
///
/// 走 polkit 的 `pkexec`：桌面环境的 polkit 认证代理会弹出一个
/// 系统密码框（正是 GNOME/KDE 里"临时以 root 运行"的原生体验），
/// 每次运行都需要授权，安全且无需额外配置。
///
/// `pkexec` 会重置环境变量，这里用 `/usr/bin/env` 把图形环境变量
/// 带进 root 会话，否则 root 启动的 GUI 程序连不上 X/Wayland 显示。
pub fn run_as_root(cmdline: &str) -> Result<(), String> {
    let argv = match shlex::split(cmdline) {
        Some(v) if !v.is_empty() => v,
        _ => {
            return Err(if is_zh() {
                "命令不能为空。".to_string()
            } else {
                "The command cannot be empty.".to_string()
            })
        }
    };

    let mut cmd = Command::new("pkexec");
    cmd.arg("/usr/bin/env");
    for k in ["DISPLAY", "WAYLAND_DISPLAY", "XDG_RUNTIME_DIR", "XAUTHORITY"] {
        if let Ok(v) = std::env::var(k) {
            if !v.is_empty() {
                cmd.arg(format!("{k}={v}"));
            }
        }
    }
    cmd.args(&argv);
    cmd.stdin(Stdio::null());

    match cmd.spawn() {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(if is_zh() {
            "以 root 身份运行需要 polkit（pkexec），但系统中没有找到该程序。".to_string()
        } else {
            "polkit (pkexec) is required to run as root, but it was not found.".to_string()
        }),
        Err(e) => Err(if is_zh() {
            format!("以 root 身份启动失败：{e}")
        } else {
            format!("Failed to start as root: {e}")
        }),
    }
}
