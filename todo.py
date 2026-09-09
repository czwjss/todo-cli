#!/usr/bin/env python3
"""
todo —— 练手用命令行任务清单工具（Python 版本）

功能：
    todo add "任务内容" [--priority high|medium|low]   添加任务
    todo list [--status pending|done] [--json]          列出任务
    todo done <id>                                      标记任务完成
    todo delete <id>                                    删除任务
    todo stats                                          统计信息

设计要点（对应 clig.dev 指南）：
    - 使用标准库 argparse 解析参数，自动获得 -h/--help 和用法错误
    - 成功退出码为 0，业务错误退出码为 1，参数错误由 argparse 返回 2
    - 正常输出走 stdout，错误信息走 stderr
    - 提供 --json 机器可读输出（管道/grep 友好）

存储：
    默认保存到 ~/.todo/tasks.json；可用环境变量 TODO_DIR 或 TODO_FILE 覆盖
    （覆盖方便测试，避免污染真实数据）
"""

import argparse
import json
import os
import sys
from datetime import date
from typing import Optional
import unicodedata


# ---------------------------------------------------------------------------
# 常量与数据存储
# ---------------------------------------------------------------------------

# 优先级：中文标签 + 排序权重 + 终端显示颜色（ANSI 转义码）
PRIORITIES = {
    "high":   {"label": "高", "weight": 0, "color": "\033[31m"},  # 红色
    "medium": {"label": "中", "weight": 1, "color": "\033[33m"},  # 黄色
    "low":    {"label": "低", "weight": 2, "color": "\033[32m"},  # 绿色
}
RESET = "\033[0m"  # 重置颜色

# 数据文件位置：优先取环境变量（便于测试），否则 ~/.todo/tasks.json
DATA_DIR = os.environ.get("TODO_DIR", os.path.join(os.path.expanduser("~"), ".todo"))
DATA_FILE = os.environ.get("TODO_FILE", os.path.join(DATA_DIR, "tasks.json"))


def display_width(text: str) -> int:
    """计算字符串在终端中的显示宽度。

    中文字符等全角/宽字符在终端占 2 列，ASCII 等占 1 列。
    len() 按字符数统计，直接用它对齐会出现错位。
    """
    width = 0
    for ch in text:
        # East Asian Wide（W）与 Fullwidth（F）字符按 2 列计
        if unicodedata.east_asian_width(ch) in ("W", "F"):
            width += 2
        else:
            width += 1
    return width


def pad(text: str, width: int) -> str:
    """按显示宽度把文本补齐到指定宽度（不足则补空格）。"""
    return text + " " * max(0, width - display_width(text))


def load_data() -> dict:
    """从 JSON 文件加载全部任务数据。

    文件不存在时返回空结构（首次运行）；文件损坏时给出可理解错误并退出。
    返回结构：
        {"next_id": int, "tasks": [{"id", "text", "priority", "status", "created_at"}]}
    """
    if not os.path.exists(DATA_FILE):
        # 首次运行：返回空数据骨架
        return {"next_id": 1, "tasks": []}
    try:
        with open(DATA_FILE, encoding="utf-8") as f:
            return json.load(f)
    except (json.JSONDecodeError, OSError) as e:
        # 把底层异常重写为人类可读的错误，并给出排查方向
        sys.stderr.write(f"错误：无法读取数据文件 {DATA_FILE}（{e}）\n")
        sys.stderr.write("提示：文件损坏时可删除该文件后重试。\n")
        sys.exit(1)


def save_data(data: dict) -> None:
    """把任务数据写回 JSON 文件。

    自动创建目录；写入失败（如权限不足）时输出可理解错误。
    """
    try:
        os.makedirs(DATA_DIR, exist_ok=True)  # 目录不存在则创建
        with open(DATA_FILE, "w", encoding="utf-8") as f:
            json.dump(data, f, ensure_ascii=False, indent=2)
    except OSError as e:
        sys.stderr.write(f"错误：无法写入数据文件 {DATA_FILE}（{e}）\n")
        sys.exit(1)


# ---------------------------------------------------------------------------
# 业务逻辑（每个命令一个函数，返回值用于退出码）
# ---------------------------------------------------------------------------

def cmd_add(text: str, priority: str) -> int:
    """添加一条任务并返回退出码。

    text: 任务内容（已由调用方校验非空）
    priority: 优先级，取值来自 PRIORITIES 的键
    """
    data = load_data()
    task = {
        "id": data["next_id"],          # 自增 ID，永不复用（删除后不回收）
        "text": text,
        "priority": priority,
        "status": "pending",            # 新任务默认未完成
        "created_at": date.today().isoformat(),  # 创建日期，如 2026-09-08
    }
    data["tasks"].append(task)
    data["next_id"] += 1
    save_data(data)
    print(f"已添加任务 #{task['id']}: {text}")
    return 0


def cmd_list(status: Optional[str], as_json: bool) -> int:
    """列出任务。

    status: 过滤条件，None 表示全部，'pending' 或 'done' 表示只看某种状态
    as_json: True 时输出 JSON（机器可读），否则输出人可读表格
    """
    data = load_data()
    # 按状态过滤；未指定则不过滤
    tasks = [t for t in data["tasks"] if status is None or t["status"] == status]
    # 排序：未完成的在前，同状态下高优先级在前（weight 越小优先级越高）
    tasks.sort(key=lambda t: (t["status"] != "pending", PRIORITIES[t["priority"]]["weight"]))

    if as_json:
        # 机器可读输出：整个任务列表输出为 JSON，供脚本/jq 消费
        print(json.dumps(tasks, ensure_ascii=False, indent=2))
        return 0

    if not tasks:
        # 空列表时给出友好提示（而不是输出空表格）
        print("（没有任务" + (f"，状态：{status}" if status else "") + "）")
        return 0

    # 人可读表格：ID、状态、优先级、内容、创建日期
    # 用显示宽度（中文按 2 列）计算各列宽度，保证对齐
    id_w = max(display_width("ID"), max(display_width(str(t["id"])) for t in tasks))
    text_w = max(display_width("任务"), max(display_width(t["text"]) for t in tasks))
    header = f"{pad('ID', id_w)}  {pad('状态', 4)} {pad('优先级', 4)} {pad('任务', text_w)}  创建日期"
    print(header)
    print("-" * display_width(header))
    for t in tasks:
        p = PRIORITIES[t["priority"]]
        # 终端是 TTY 时用颜色区分优先级，管道/重定向时保持纯文本
        prio = f"{p['color']}{p['label']}{RESET}" if sys.stdout.isatty() else p["label"]
        status = "完成" if t["status"] == "done" else "待办"
        print(f"{pad(str(t['id']), id_w)}  {pad(status, 4)} {pad(prio, 4)} {pad(t['text'], text_w)}  {t['created_at']}")
    return 0


def _find_task(data: dict, task_id: int) -> dict:
    """按 ID 查找任务；找不到则输出错误并退出（返回类型实际为 dict 或 None）。"""
    for t in data["tasks"]:
        if t["id"] == task_id:
            return t
    sys.stderr.write(f"错误：不存在 ID 为 {task_id} 的任务。可用 todo list 查看。\n")
    sys.exit(1)


def cmd_done(task_id: int) -> int:
    """把任务标记为已完成（幂等：已完成再标记不报错）。"""
    data = load_data()
    task = _find_task(data, task_id)
    if task["status"] == "done":
        print(f"任务 #{task_id} 已是完成状态。")
    else:
        task["status"] = "done"
        save_data(data)
        print(f"已完成任务 #{task_id}: {task['text']}")
    return 0


def cmd_delete(task_id: int) -> int:
    """删除一条任务。"""
    data = load_data()
    task = _find_task(data, task_id)  # 找不到会退出
    data["tasks"].remove(task)        # 按对象移除
    save_data(data)
    print(f"已删除任务 #{task_id}: {task['text']}")
    return 0


def cmd_stats() -> int:
    """输出统计信息：总数、完成数、待办数、按优先级分布。"""
    data = load_data()
    tasks = data["tasks"]
    total = len(tasks)
    done = sum(1 for t in tasks if t["status"] == "done")
    pending = total - done
    print(f"任务总数：{total}")
    print(f"已完成：{done}")
    print(f"待办：{pending}")
    if total:
        # 按优先级统计未完成任务的分布，输出进度条效果
        print("待办优先级分布：")
        for key in ("high", "medium", "low"):
            p = PRIORITIES[key]
            count = sum(1 for t in tasks if t["status"] == "pending" and t["priority"] == key)
            # 简易进度条：每项任务一个 # 符号
            bar = "#" * count if count else "-"
            print(f"  {p['label']}优先级：{count}  {bar}")
    return 0


# ---------------------------------------------------------------------------
# 参数解析与入口
# ---------------------------------------------------------------------------

def build_parser() -> argparse.ArgumentParser:
    """构造命令行参数解析器（含子命令与各自的 flags）。"""
    parser = argparse.ArgumentParser(
        prog="todo",
        description="练手用命令行任务清单工具",
    )
    # addsubparsers 创建子命令；required=True 保证不带子命令时输出用法错误
    sub = parser.add_subparsers(dest="command", required=True, help="子命令")

    # todo add <text> [--priority high|medium|low]
    p_add = sub.add_parser("add", help="添加任务")
    p_add.add_argument("text", help="任务内容")                       # 位置参数
    p_add.add_argument("-p", "--priority", choices=PRIORITIES.keys(),  # 短/长 flag
                       default="medium", help="优先级：high/medium/low（默认 medium）")

    # todo list [--status pending|done] [--json]
    p_list = sub.add_parser("list", help="列出任务")
    p_list.add_argument("--status", choices=["pending", "done"], help="按状态过滤")
    p_list.add_argument("--json", action="store_true", help="以 JSON 输出（机器可读）")

    # todo done <id>
    p_done = sub.add_parser("done", help="标记任务完成")
    p_done.add_argument("id", type=int, help="任务 ID")

    # todo delete <id>
    p_delete = sub.add_parser("delete", help="删除任务")
    p_delete.add_argument("id", type=int, help="任务 ID")

    # todo stats
    sub.add_parser("stats", help="显示统计信息")
    return parser


def main() -> int:
    """程序入口：解析参数后分发到对应命令函数，统一返回退出码。"""
    parser = build_parser()
    args = parser.parse_args()  # 参数错误时 argparse 自动打印用法并 exit(2)

    if args.command == "add":
        # 业务校验：空任务内容没有意义，给出明确错误（先于写文件）
        if not args.text.strip():
            sys.stderr.write("错误：任务内容不能为空。\n")
            return 1
        return cmd_add(args.text.strip(), args.priority)
    if args.command == "list":
        return cmd_list(args.status, args.json)
    if args.command == "done":
        return cmd_done(args.id)
    if args.command == "delete":
        return cmd_delete(args.id)
    if args.command == "stats":
        return cmd_stats()
    # 理论上到不了这里（argparse required=True 已拦截）
    return 0


if __name__ == "__main__":
    sys.exit(main())
