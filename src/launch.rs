//! 命令执行层。
//!
//! 设计定稿（与用户确认）：
//! - 不用 shell，直接 `Command::spawn`；
//! - 子进程继承 runbox 的启动身份与环境（`sudo runbox` = root 执行，普通启动 = 当前用户）；
//! - 不做 Windows→Linux 命令映射，输入什么执行什么；
//! - 失败时给出 Windows 风味的报错。

use std::path::Path;
use std::process::{Command, Stdio};

/// 界面语言是否为中文（跟随 LANG 环境变量）。
pub fn is_zh() -> bool {
    std::env::var("LANG")
        .unwrap_or_default()
        .to_lowercase()
        .starts_with("zh")
}

/// 用系统默认应用打开 URL 或路径（走 `xdg-open`，Linux 的标准打开机制）。
///
/// `xdg-open` 会按系统默认应用分发：http/https 交给浏览器，目录交给
/// 文件管理器（nautilus/dolphin 等），文件交给对应默认程序。
/// 命令不存在或启动失败时给出面向用户的错误。
fn open_with_xdg(target: &str) -> Result<(), String> {
    let mut cmd = Command::new("xdg-open");
    cmd.arg(target);
    match cmd.spawn() {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(if is_zh() {
            "需要 xdg-open 来打开目标，但系统中没有找到该程序。".to_string()
        } else {
            "xdg-open is required to open the target, but it was not found.".to_string()
        }),
        Err(e) => Err(if is_zh() {
            format!("无法打开目标：{e}")
        } else {
            format!("Failed to open the target: {e}")
        }),
    }
}

/// 系统里已安装的 shell 名集合。
///
/// 读取 `/etc/shells`（Linux 记录可用 shell 的标准文件）并取每个路径的
/// basename。不硬编码列表：新安装的 shell 只要注册进 `/etc/shells` 就自动生效。
fn known_shells() -> std::collections::HashSet<String> {
    let mut set = std::collections::HashSet::new();
    if let Ok(content) = std::fs::read_to_string("/etc/shells") {
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(name) = Path::new(line).file_name() {
                set.insert(name.to_string_lossy().into_owned());
            }
        }
    }
    set
}

/// 在默认终端模拟器中运行指定 shell。
///
/// 只走 `x-terminal-emulator`（Debian/Ubuntu 的 alternatives 统一入口，
/// 由系统指向用户设置的默认终端），不做终端列表探测。多数终端用 `-e`。
fn open_in_terminal(shell: &str) -> Result<(), String> {
    let mut cmd = Command::new("x-terminal-emulator");
    cmd.arg("-e").arg(shell);
    match cmd.spawn() {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(if is_zh() {
            "需要 x-terminal-emulator 来打开终端，但系统中没有找到该程序。".to_string()
        } else {
            "x-terminal-emulator is required to open a terminal, but it was not found.".to_string()
        }),
        Err(e) => Err(if is_zh() {
            format!("启动终端失败：{e}")
        } else {
            format!("Failed to start terminal: {e}")
        }),
    }
}

/// 取用户主目录。优先 `HOME`，取不到时退回 `/`。
fn home_dir() -> std::path::PathBuf {
    std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::PathBuf::from("/"))
}

/// 若输入指向一个**存在的目录或文件**，返回其绝对化路径；否则返回 `None`。
///
/// 只有"看起来像路径"的输入才当作待打开目标：绝对路径、含 `/` 的路径、
/// 或 `.`/`..`。纯命令名（不含路径特征）返回 `None`，走命令层，避免把
/// `vim` 这类命令名误判成路径去打开。
///
/// 相对路径以 `$HOME` 为基准解析，与命令执行的固定 CWD 保持一致
/// （这样 `.` = $HOME，`..` = $HOME 的上一级）。
fn resolve_existing_target(input: &str) -> Option<std::path::PathBuf> {
    let p = Path::new(input);
    let looks_like_path =
        p.is_absolute() || input.contains('/') || input == "." || input == "..";
    if !looks_like_path {
        return None;
    }
    let full = if p.is_absolute() {
        p.to_path_buf()
    } else {
        home_dir().join(p)
    };
    if full.exists() {
        Some(full)
    } else {
        None
    }
}

/// 解析并执行一条命令行。
///
/// 成功返回 `Ok(())`，失败返回面向用户的错误信息（可直接弹对话框）。
pub fn run(cmdline: &str) -> Result<(), String> {
    // 1. 以协议头开头（http:// / https://）→ 直接用默认浏览器打开
    let trimmed = cmdline.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return open_with_xdg(trimmed);
    }

    // 2. 输入指向一个存在的目录或文件（如 `.`、`..`、`/path`、`x.txt`）→
    //    用系统默认应用打开。这就是 Windows 运行框里"输入 . 打开当前目录"
    //    的对应行为：Windows 走 ShellExecute，Linux 用 xdg-open 分发——
    //    目录交给文件管理器，.txt/.html/.docx 等文件交给各自的默认软件。
    //    纯命令名（不含路径特征）不被拦截，直接交给命令层执行。
    if let Some(path) = resolve_existing_target(trimmed) {
        return open_with_xdg(&path.to_string_lossy());
    }

    // 3. 其余按命令解析执行。
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

    // 4. 裸的 shell 名（如 bash / zsh / fish，不带参数）→ 打开终端窗口承载它。
    //    shell 集合不硬编码，而是读系统 /etc/shells 自动发现：
    //    新装的 shell 只要注册进去就自动生效，无需更新代码。
    if argv.len() == 1 {
        let name = Path::new(&argv[0])
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| argv[0].clone());
        if known_shells().contains(&name) {
            return open_in_terminal(&name);
        }
    }

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
