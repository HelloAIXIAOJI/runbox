# runbox — Linux 上的 Win+R

> 谁不想急头白脸地按下 `Win+R`，然后在 Linux 打开运行？

一个 Linux 原生的"运行"对话框：按下 `Super+R` 弹出，输入命令回车执行。
用 **Rust + GTK4 + libadwaita** 编写，外观自动跟随系统主题（Adwaita）。

## 特性

- **以启动身份执行**：`sudo runbox` 启动 → 以 root 执行命令；普通启动 → 以当前用户执行。子进程完整继承 runbox 的身份与环境（不切用户、不丢图形会话）。
- **全局热键**：X11 下 `XGrabKey` 直接注册 `Super+R`；Wayland 下走 XDG Desktop Portal 的 `GlobalShortcuts`，需在系统设置中为 runbox 授权绑定快捷键。
- **运行历史**：`~/.config/runbox/history`，去重 + 上限 30 条。root 与普通用户的 HOME 不同，历史天然按身份分开。
- **Windows 风味报错**：找不到命令时弹 `'xxx' 不是内部或外部命令……`，味儿很正。
- **中英双语**：跟随 `LANG` 环境变量。

## 依赖（编译）

Debian / Ubuntu：

```bash
sudo apt install libgtk-4-dev libadwaita-1-dev
```

## 构建与运行

```bash
cargo build --release
./target/release/runbox            # 普通用户
sudo ./target/release/runbox       # root 身份
```

## 说明

- 本项目是 Linux-only（依赖 libadwaita，且全局热键依赖 X11/Wayland）。
- Wayland 下第一次注册快捷键后，需要到系统设置的"键盘快捷键"里给 `runbox` 绑定触发键。
- `sudo` 启动时若窗口无法显示，用 `sudo -E` 或 `pkexec env DISPLAY=$DISPLAY XDG_RUNTIME_DIR=/run/user/$(id -u) ...` 保留图形环境变量。

## 许可

MIT
