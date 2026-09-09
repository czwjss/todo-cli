# todo-cli

练手用命令行任务清单工具，提供 **Rust** 与 **Python** 两个实现版本，功能对齐、命令一致。

## 功能

- 添加 / 列出 / 完成 / 删除任务，统计信息
- 中文输出，终端自动配色；`--json` 机器可读输出
- 成功退出码 0、业务错误 1、参数错误 2；错误信息走 stderr
- 默认存储 `~/.todo/tasks.json`，可用环境变量 `TODO_DIR` / `TODO_FILE` 覆盖
- Rust 版含运行时自更新：检测到 GitHub 新版本时交互提示，确认后自动更新

```
todo add "任务内容" [--priority high|medium|low]   添加任务
todo list [--status pending|done] [--json]        列出任务
todo done <id>                                    标记任务完成
todo delete <id>                                  删除任务
todo stats                                        统计信息
```

## 安装

三种方式安装后的命令均为 `todo`，任选其一：

| 方式 | 适用场景 | 命令 |
| --- | --- | --- |
| 预编译二进制 | 无需工具链，直接下载 | `curl -fsSL ... -o ~/.local/bin/todo && chmod +x ~/.local/bin/todo` |
| cargo install | 已有 Rust 工具链 | `cargo install czwjss-todo-cli` |
| pip install | 已有 Python 环境 | `pip install czwjss-todo-cli` |

### 预编译二进制下载地址

```bash
# macOS ARM（Intel 将文件名中的 aarch64 换成 x86_64）
curl -fsSL https://github.com/czwjss/todo-cli/releases/latest/download/todo-aarch64-apple-darwin -o ~/.local/bin/todo && chmod +x ~/.local/bin/todo
```

其他平台：`todo-x86_64-unknown-linux-gnu`（Linux x86_64）、`todo-x86_64-pc-windows-msvc.exe`（Windows x86_64）。

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
# 按安装方式选择其一：
rm "${HOME}/.local/bin/todo"          # 预编译二进制
cargo uninstall czwjss-todo-cli        # cargo 安装
pip uninstall czwjss-todo-cli          # pip 安装

# 可选：清理任务数据
rm -rf "${HOME}/.todo"
```

## 项目结构

```
├── rust/        Rust 版（发布为 crates.io 包 czwjss-todo-cli）
├── python/      Python 版（发布为 PyPI 包 czwjss-todo-cli，纯标准库零依赖）
└── .github/workflows/    CI/CD：Build / GitHub Release / crates.io / PyPI
```

## 许可

[MIT](rust/LICENSE)
