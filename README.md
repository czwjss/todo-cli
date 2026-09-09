# todo-cli

> 练手用命令行任务清单工具 · Rust / Python 双版本，命令与功能完全一致

![GitHub release](https://img.shields.io/github/v/release/czwjss/todo-cli)
![CI](https://img.shields.io/github/actions/workflow/status/czwjss/todo-cli/build.yml)
![crates.io](https://img.shields.io/crates/v/czwjss-todo-cli)
![PyPI](https://img.shields.io/pypi/v/czwjss-todo-cli)
![License](https://img.shields.io/github/license/czwjss/todo-cli?cacheSeconds=3600&v=2)

## ✨ 功能

- 添加 / 列出 / 完成 / 删除任务，一键统计
- 中文输出，终端自动配色；`--json` 机器可读输出
- 规范退出码：成功 `0`、业务错误 `1`、参数错误 `2`，错误信息走 stderr
- 数据存于 `~/.todo/tasks.json`，支持 `TODO_DIR` / `TODO_FILE` 环境变量覆盖
- Rust 版内置自更新：检测到新版本时交互提示，确认后自动更新

```console
$ todo add "写周报" --priority high
已添加任务 #1: 写周报

$ todo list
 1  [高] 写周报

$ todo done 1
任务 #1 已完成

$ todo stats
共 1 个任务：1 完成，0 未完成
```

## 📦 安装

安装后命令均为 `todo`，任选其一：

| 方式 | 命令 | 说明 |
| --- | --- | --- |
| cargo | `cargo install czwjss-todo-cli` | Rust 版，需 Rust 工具链 |
| pip | `pip install czwjss-todo-cli` | Python 版，纯标准库零依赖 |
| 预编译二进制 | 见 [AGENT-INSTALL-BINARY.md](AGENT-INSTALL-BINARY.md) | 免工具链，macOS / Linux / Windows |

## 🚀 使用

| 命令 | 说明 |
| --- | --- |
| `todo add "内容" [--priority high\|medium\|low]` | 添加任务 |
| `todo list [--status pending\|done] [--json]` | 列出任务 |
| `todo done <id>` | 标记完成 |
| `todo delete <id>` | 删除任务 |
| `todo stats` | 统计信息 |
| `todo --help` | 查看帮助 |

## 🗑️ 卸载

```bash
cargo uninstall czwjss-todo-cli     # cargo 安装
pip uninstall czwjss-todo-cli       # pip 安装
# 预编译二进制：直接删除下载的文件即可
```

可选清理数据：`rm -rf ~/.todo`

## 📁 项目结构

```
├── rust/        Rust 版（crates.io 包：czwjss-todo-cli）
├── python/      Python 版（PyPI 包：czwjss-todo-cli）
└── .github/     CI/CD 工作流
```

