# czwjss-todo-cli（Python 版）

练手用命令行任务清单工具。纯标准库实现，无第三方依赖。

## 安装

```bash
pip install czwjss-todo-cli
# 或从源码目录安装
pip install ./python
```

安装后命令为 `todo`：

```bash
todo add "写周报" --priority high
todo list
todo done 1
todo delete 2
todo stats
```

## 存储

默认 `~/.todo/tasks.json`，可用环境变量 `TODO_DIR` / `TODO_FILE` 覆盖。
