# runbox — Linux 上的 Win+R

> 谁不想急头白脸地按下 `Win+R`，然后在 Linux 打开运行？

一个 Linux 原生的"运行"对话框：按下 `Super+R` 弹出，输入命令回车执行。
用 **Rust + GTK4 + libadwaita** 编写，外观自动跟随系统主题（Adwaita）。

按 **Ctrl+Shift+回车** 会以 **root 身份**运行当前命令：走 polkit（`pkexec`），
桌面环境的认证代理会弹出系统密码框，每次授权，无需额外配置。

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

## REPL 模式（无界面）

如果当前环境没有桌面（比如 SSH 会话、容器、纯终端），可以用 `--repl` 以纯命令行方式交互，
复用与 GUI 完全相同的执行与历史逻辑，无需 GTK 显示服务器：

```bash
./target/release/runbox --repl
```

- 启动时打印与 GUI 一致的说明文字，然后出现 `> ` 提示符等待输入；
- 回车执行命令：成功记入历史并退出（等同 GUI 回车执行后关窗）；
- 命令失败会打印错误信息，并**继续**等待下一次输入；
- **↑ / ↓** 在历史记录中切换（复用同一个历史文件）；
- 空行直接回车、或按 `Ctrl+C` / `Ctrl+D` 退出。

## 说明

- 本项目是 Linux-only（依赖 libadwaita，且全局热键依赖 X11/Wayland）。
- REPL 模式（`--repl`）不依赖桌面环境，可在无显示服务器时正常使用。
- Wayland 下第一次注册快捷键后，需要到系统设置的"键盘快捷键"里给 `runbox` 绑定触发键。
- `sudo` 启动时若窗口无法显示，用 `sudo -E` 或 `pkexec env DISPLAY=$DISPLAY XDG_RUNTIME_DIR=/run/user/$(id -u) ...` 保留图形环境变量。

## 许可

MIT
