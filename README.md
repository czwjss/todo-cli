# todo-cli

练手用命令行任务清单工具，提供 **Rust** 与 **Python** 两个实现版本，功能对齐、命令一致。

## 功能

```
todo add "任务内容" [--priority high|medium|low]   添加任务
todo list [--status pending|done] [--json]        列出任务
todo done <id>                                    标记任务完成
todo delete <id>                                  删除任务
todo stats                                        统计信息
```

- 中文输出，终端自动配色；`--json` 提供机器可读输出
- 成功退出码 0、业务错误 1、参数错误 2；错误走 stderr
- 默认存储 `~/.todo/tasks.json`，可用环境变量 `TODO_DIR` / `TODO_FILE` 覆盖
- Rust 版含运行时自更新：检测到 GitHub 新版本时交互提示，确认后自动更新

## 安装

任选一种方式：

### 1. 预编译二进制（GitHub Release，无需工具链）

```bash
# macOS ARM
curl -fsSL https://github.com/czwjss/todo-cli/releases/latest/download/todo-aarch64-apple-darwin -o ~/.local/bin/todo && chmod +x ~/.local/bin/todo
```

其他平台文件名：`todo-x86_64-apple-darwin`（macOS Intel）、`todo-x86_64-unknown-linux-gnu`（Linux x86_64）、`todo-x86_64-pc-windows-msvc.exe`（Windows x86_64）。

### 2. cargo install（Rust 版）

```bash
cargo install czwjss-todo-cli
```

### 3. pip install（Python 版）

```bash
pip install czwjss-todo-cli
```

> 三种方式安装后的命令均为 `todo`。Rust 与 Python 版功能一致，选其一安装即可。

## 使用示例

```bash
todo add "写周报" --priority high    # 已添加任务 #1: 写周报
todo add "买菜"
todo list                            # 列出未完成任务
todo done 1                          # 标记任务完成
todo stats                           # 统计信息
```

## 验证

```bash
todo --help    # 输出中文帮助，含 add/list/done/delete/stats
```

## 卸载

```bash
# 预编译二进制
rm "${HOME}/.local/bin/todo"

# 或 cargo 安装的版本
cargo uninstall czwjss-todo-cli

# 或 pip 安装的版本
pip uninstall czwjss-todo-cli

# 可选：清理任务数据
rm -rf "${HOME}/.todo"
```

## 项目结构

```
├── rust/        Rust 版（crates.io 发布为 czwjss-todo-cli）
├── python/      Python 版（PyPI 发布为 czwjss-todo-cli，纯标准库零依赖）
└── .github/workflows/    CI/CD：Build / GitHub Release / crates.io / PyPI
```

## 发布

打 tag 即自动发布三渠道（GitHub Release、crates.io、PyPI）：

```bash
git tag v0.1.1 && git push origin v0.1.1
```

版本号需同步更新 `rust/Cargo.toml` 与 `python/pyproject.toml`。

## 许可

[MIT](rust/LICENSE)
