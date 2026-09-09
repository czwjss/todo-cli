# AGENTS.md —— todo CLI 安装指引（面向 AI Agent）

本文件用于让 AI Agent 自动安装并验证 todo CLI。请严格按以下步骤执行，命令可直接复制运行。

## 1. 检测环境

```bash
# 操作系统：Darwin = macOS，Linux = Linux，MINGW* = Windows
uname -s
# 架构：arm64 / x86_64
uname -m
```

## 2. 下载并安装

二进制来自 GitHub Release（发布后 URL 格式固定为下方形式）。

```bash
# 设置版本（发布后替换为实际 tag，如 v0.1.0）
VERSION=v0.1.0

# 按第 1 步检测结果组合下载名：
#   todo-aarch64-apple-darwin    macOS ARM
#   todo-x86_64-apple-darwin     macOS Intel
#   todo-x86_64-unknown-linux-gnu    Linux x86_64
#   todo-x86_64-pc-windows-msvc.exe  Windows x86_64
URL="https://github.com/czwjss/todo-cli/releases/download/${VERSION}/<平台文件名>"

# 安装目录（确保在 PATH 中；不存在则创建）
INSTALL_DIR="${HOME}/.local/bin"
mkdir -p "$INSTALL_DIR"

# 下载并安装
curl -fsSL "$URL" -o "$INSTALL_DIR/todo"
chmod +x "$INSTALL_DIR/todo"

# 若 INSTALL_DIR 不在 PATH，需将其加入 PATH（写入 shell 配置并重新加载）
if ! echo "$PATH" | grep -q "$INSTALL_DIR"; then
  echo "export PATH=\"$INSTALL_DIR:\$PATH\"" >> "${HOME}/.zshrc"
  export PATH="$INSTALL_DIR:$PATH"
fi
```

## 3. 验证

```bash
todo --version   # 应输出：todo 0.1.0
todo --help      # 应输出中文帮助，含 add/list/done/delete/stats 子命令
```

## 4. 常见问题处理

| 现象 | 处理 |
| --- | --- |
| `command not found: todo` | PATH 未包含安装目录，执行第 2 步的 PATH 配置 |
| 下载 404 | 版本号错误，先查最新 Release：`curl -s https://api.github.com/repos/czwjss/todo-cli/releases/latest` |
| macOS "无法打开" | 需对二进制执行 `xattr -d com.apple.quarantine <文件路径>` 或右键打开（未签名产物） |

## 5. 更新

```bash
# 重新执行第 2 步（下载新版本覆盖），或：
cargo install todo --force   # 开发者选项，需已安装 Rust 工具链
```

## 6. 卸载

```bash
rm "${HOME}/.local/bin/todo"   # 删除二进制即可；如需清理数据，再删 ~/.todo/
```

> 注：安装说明依赖发布产物，Release 未发布前 URL 不可用。发布后本文件无需改动（URL 模板固定）。
