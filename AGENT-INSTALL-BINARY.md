# AGENT-INSTALL-BINARY.md —— todo CLI 预编译二进制安装

## 1. 检测环境

```bash
uname -s    # Darwin = macOS，Linux = Linux，MINGW* = Windows
uname -m    # arm64 / x86_64
```

## 2. 确定下载文件名

| 系统 | 架构 | 文件名 |
| --- | --- | --- |
| macOS | ARM | `todo-aarch64-apple-darwin` |
| macOS | Intel | `todo-x86_64-apple-darwin` |
| Linux | x86_64 | `todo-x86_64-unknown-linux-gnu` |
| Windows | x86_64 | `todo-x86_64-pc-windows-msvc.exe` |

## 3. 下载并安装

```bash
# 设置版本（替换为实际 tag，如 v0.1.1）
VERSION=v0.1.1
ASSET="<上表对应文件名>"

# 安装目录（确保在 PATH 中；不存在则创建）
INSTALL_DIR="${HOME}/.local/bin"
mkdir -p "$INSTALL_DIR"

# 下载并安装
curl -fsSL "https://github.com/czwjss/todo-cli/releases/download/${VERSION}/${ASSET}" -o "$INSTALL_DIR/todo"
chmod +x "$INSTALL_DIR/todo"

# 若 INSTALL_DIR 不在 PATH，写入 shell 配置并重新加载
if ! echo "$PATH" | grep -q "$INSTALL_DIR"; then
  echo "export PATH=\"$INSTALL_DIR:\$PATH\"" >> "${HOME}/.zshrc"
  export PATH="$INSTALL_DIR:$PATH"
fi
```

## 4. 验证

```bash
todo --version   # 应输出：todo 0.1.1
todo --help      # 应输出中文帮助，含 add/list/done/delete/stats
```

## 5. 常见问题

| 现象 | 处理 |
| --- | --- |
| `command not found: todo` | PATH 未包含安装目录，执行第 3 步的 PATH 配置 |
| 下载 404 | 版本号错误，先查最新 Release：`curl -s https://api.github.com/repos/czwjss/todo-cli/releases/latest` |
| macOS "无法打开" | 执行 `xattr -d com.apple.quarantine <文件路径>` 或右键打开（未签名产物） |
