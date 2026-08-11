# runbox — Linux 上的 Win+R

> 谁不想急头白脸地按下 `Win+R`，然后在 Linux 打开运行？

一个 Linux 原生的"运行"对话框：按下 `Super+R` 弹出，输入命令回车执行。
用 **Rust + GTK4 + libadwaita** 编写，外观自动跟随系统主题（Adwaita）。

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
