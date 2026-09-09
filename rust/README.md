# czwjss-todo-cli

练手用命令行任务清单工具（Rust 版本）。

## 安装

```bash
cargo install czwjss-todo-cli
```

安装后命令为 `todo`：

```bash
todo add "写周报" --priority high --due "2026-09-09 18:00"
todo list --sort due
todo done 1
todo undo 1
todo edit 1 --priority low
todo search "周报"
todo delete 2
todo stats
```

也可从 GitHub Release 下载预编译二进制，见仓库根目录 `AGENT-INSTALL-BINARY.md`。

## 存储

默认 `~/.todo/tasks.json`，可用环境变量 `TODO_DIR` / `TODO_FILE` 覆盖。
