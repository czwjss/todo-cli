#!/usr/bin/env python3
"""
todo —— 练手用命令行任务清单工具（Python 版本）

功能（与 Rust 版本完全对齐）：
    todo add "任务内容" [--priority high|medium|low] [--due "2026-09-09 18:00"]   添加任务
    todo list [--status pending|done] [--sort priority|created|due] [--json]      列出任务
    todo done <id> [<id> ...]                                                     标记完成（支持多个）
    todo undo <id>                                                                恢复待办
    todo delete <id> [<id> ...]                                                    删除任务（支持多个）
    todo edit <id> [--text 新内容] [--priority high] [--due 截止] [--no-due]      修改任务
    todo search <关键词> [--status pending|done] [--json]                         搜索任务
    todo stats                                                                    统计信息（含逾期数）

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
from datetime import date, datetime
from typing import List, Optional
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
RESET = "\033[0m"   # 重置颜色
RED = "\033[31m"    # 红色（逾期标记）

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
        {"next_id": int, "tasks": [{"id", "text", "priority", "status", "due", "created_at"}]}
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
# 截止时间（due）辅助
# ---------------------------------------------------------------------------

def parse_due(text: str) -> Optional[datetime]:
    """解析截止时间字符串。

    支持 "YYYY-MM-DD"（当天 23:59:59 截止）与 "YYYY-MM-DD HH:MM" 两种格式；
    格式非法返回 None（由调用方给出错误信息）。
    """
    for fmt in ("%Y-%m-%d %H:%M", "%Y-%m-%d"):
        try:
            return datetime.strptime(text, fmt)
        except ValueError:
            continue
    return None


def due_overdue(task: dict) -> bool:
    """任务是否已逾期：未完成且截止时间早于当前时间。"""
    if task["status"] != "pending" or not task.get("due"):
        return False
    due = parse_due(task["due"])
    return due is not None and due < datetime.now()


def due_cell(task: dict) -> str:
    """截止时间单元格文本：无截止显示 "-"，逾期追加红色 "!"（非终端不加色）。"""
    due = task.get("due")
    if not due:
        return "-"
    if due_overdue(task):
        if sys.stdout.isatty():
            return f"{RED}{due}!{RESET}"
        return f"{due}!"
    return due


# ---------------------------------------------------------------------------
# 业务逻辑（每个命令一个函数，返回值用于退出码）
# ---------------------------------------------------------------------------

def cmd_add(text: str, priority: str, due: Optional[str]) -> int:
    """添加一条任务并返回退出码。

    text: 任务内容（已由调用方校验非空）
    priority: 优先级，取值来自 PRIORITIES 的键
    due: 截止时间字符串；None 表示不设置；空白视为未提供
    """
    if due and due.strip():
        if parse_due(due.strip()) is None:
            sys.stderr.write("错误：截止时间格式应为 YYYY-MM-DD 或 YYYY-MM-DD HH:MM。\n")
            return 1
        due = due.strip()
    else:
        due = None
    data = load_data()
    task = {
        "id": data["next_id"],          # 自增 ID，永不复用（删除后不回收）
        "text": text,
        "priority": priority,
        "status": "pending",            # 新任务默认未完成
        "due": due,
        "created_at": date.today().isoformat(),  # 创建日期，如 2026-09-08
    }
    data["tasks"].append(task)
    data["next_id"] += 1
    save_data(data)
    print(f"已添加任务 #{task['id']}: {text}")
    return 0


def collect_tasks(data: dict, status: Optional[str], keyword: Optional[str]) -> List[dict]:
    """按状态/关键词收集任务（关键词匹配文本，不区分大小写）。"""
    kw = keyword.lower() if keyword else None
    tasks = []
    for t in data["tasks"]:
        if status is not None and t["status"] != status:
            continue
        if kw is not None and kw not in t["text"].lower():
            continue
        tasks.append(t)
    return tasks


def sort_tasks(tasks: List[dict], sort: str) -> None:
    """排序：待办始终在前；同状态下按 --sort 指定的键排序。"""
    def key(t: dict):
        pending = t["status"] != "pending"
        if sort == "created":
            return (pending, t["created_at"], t["id"])
        if sort == "due":
            due = t.get("due") or ""  # 无截止的排最后
            return (pending, not due, due, t["id"])
        # priority（默认）
        return (pending, PRIORITIES[t["priority"]]["weight"], t["id"])
    tasks.sort(key=key)


def render_tasks(tasks: List[dict], status: Optional[str], as_json: bool) -> int:
    """渲染任务列表：--json 输出 JSON，否则输出对齐表格。"""
    if as_json:
        # 机器可读输出：整个任务列表输出为 JSON，供脚本/jq 消费
        print(json.dumps(tasks, ensure_ascii=False, indent=2))
        return 0

    if not tasks:
        # 空列表时给出友好提示（而不是输出空表格）
        print("（没有任务" + (f"，状态：{status}" if status else "") + "）")
        return 0

    # 人可读表格：ID、状态、优先级、任务、截止、创建日期
    # 用显示宽度（中文按 2 列）计算各列宽度，保证对齐
    id_w = max(display_width("ID"), max(display_width(str(t["id"])) for t in tasks))
    text_w = max(display_width("任务"), max(display_width(t["text"]) for t in tasks))
    due_w = max(display_width("截止"), max(display_width(due_cell(t)) for t in tasks))
    header = f"{pad('ID', id_w)}  {pad('状态', 4)} {pad('优先级', 4)} {pad('任务', text_w)}  {pad('截止', due_w)}  创建日期"
    print(header)
    print("-" * display_width(header))
    for t in tasks:
        p = PRIORITIES[t["priority"]]
        # 终端是 TTY 时用颜色区分优先级，管道/重定向时保持纯文本
        prio = f"{p['color']}{p['label']}{RESET}" if sys.stdout.isatty() else p["label"]
        status_text = "完成" if t["status"] == "done" else "待办"
        print(f"{pad(str(t['id']), id_w)}  {pad(status_text, 4)} {pad(prio, 4)} {pad(t['text'], text_w)}  {pad(due_cell(t), due_w)}  {t['created_at']}")
    return 0


def cmd_list(status: Optional[str], as_json: bool, sort: str) -> int:
    """列出任务。

    status: 过滤条件，None 表示全部，'pending' 或 'done' 表示只看某种状态
    as_json: True 时输出 JSON（机器可读）
    sort: 排序方式 priority/created/due
    """
    data = load_data()
    tasks = collect_tasks(data, status, None)
    sort_tasks(tasks, sort)
    return render_tasks(tasks, status, as_json)


def cmd_search(keyword: str, status: Optional[str], as_json: bool) -> int:
    """按关键词搜索任务。"""
    if not keyword.strip():
        sys.stderr.write("错误：搜索关键词不能为空。\n")
        return 1
    data = load_data()
    tasks = collect_tasks(data, status, keyword.strip())
    sort_tasks(tasks, "priority")
    return render_tasks(tasks, status, as_json)


def _find_task(data: dict, task_id: int) -> dict:
    """按 ID 查找任务；找不到则输出错误并退出（返回类型实际为 dict 或 None）。"""
    for t in data["tasks"]:
        if t["id"] == task_id:
            return t
    sys.stderr.write(f"错误：不存在 ID 为 {task_id} 的任务。可用 todo list 查看。\n")
    sys.exit(1)


def cmd_done(ids: List[int]) -> int:
    """批量标记为已完成（幂等：已完成再标记不报错；任一 ID 不存在则不修改）。"""
    data = load_data()
    for tid in ids:
        _find_task(data, tid)  # 全部验证存在，避免部分成功
    for tid in ids:
        task = _find_task(data, tid)
        if task["status"] == "done":
            print(f"任务 #{tid} 已是完成状态。")
        else:
            task["status"] = "done"
            print(f"已完成任务 #{tid}: {task['text']}")
    save_data(data)
    return 0


def cmd_undo(task_id: int) -> int:
    """把任务恢复为待办。"""
    data = load_data()
    task = _find_task(data, task_id)
    if task["status"] == "pending":
        print(f"任务 #{task_id} 已是待办状态。")
    else:
        task["status"] = "pending"
        save_data(data)
        print(f"已恢复任务 #{task_id}: {task['text']}")
    return 0


def cmd_delete(ids: List[int]) -> int:
    """批量删除任务（任一 ID 不存在则不修改）。"""
    data = load_data()
    for tid in ids:
        _find_task(data, tid)  # 全部验证存在
    for tid in ids:
        task = _find_task(data, tid)
        data["tasks"].remove(task)
        print(f"已删除任务 #{tid}: {task['text']}")
    save_data(data)
    return 0


def cmd_edit(task_id: int, new_text: Optional[str], new_priority: Optional[str],
             new_due: Optional[str], no_due: bool) -> int:
    """修改任务文本/优先级/截止时间（至少提供一项）。"""
    if new_text is None and new_priority is None and new_due is None and not no_due:
        sys.stderr.write("错误：请至少提供一项修改：--text / --priority / --due / --no-due。\n")
        return 1
    data = load_data()
    task = _find_task(data, task_id)
    if new_text is not None:
        if not new_text.strip():
            sys.stderr.write("错误：任务内容不能为空。\n")
            return 1
        task["text"] = new_text.strip()
    if new_priority is not None:
        task["priority"] = new_priority
    if no_due:
        task["due"] = None
    elif new_due is not None:
        if new_due.strip():
            if parse_due(new_due.strip()) is None:
                sys.stderr.write("错误：截止时间格式应为 YYYY-MM-DD 或 YYYY-MM-DD HH:MM。\n")
                return 1
            task["due"] = new_due.strip()
        else:
            task["due"] = None
    save_data(data)
    print(f"已更新任务 #{task_id}: {task['text']}")
    return 0


def cmd_stats() -> int:
    """输出统计信息：总数、完成数、待办数、逾期数、按优先级分布。"""
    data = load_data()
    tasks = data["tasks"]
    total = len(tasks)
    done = sum(1 for t in tasks if t["status"] == "done")
    pending = total - done
    overdue = sum(1 for t in tasks if due_overdue(t))
    print(f"任务总数：{total}")
    print(f"已完成：{done}")
    print(f"待办：{pending}")
    if pending:
        print(f"已逾期：{overdue}")
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

    # todo add <text> [--priority high|medium|low] [--due ...]
    p_add = sub.add_parser("add", help="添加任务")
    p_add.add_argument("text", help="任务内容")                       # 位置参数
    p_add.add_argument("-p", "--priority", choices=PRIORITIES.keys(),  # 短/长 flag
                       default="medium", help="优先级：high/medium/low（默认 medium）")
    p_add.add_argument("--due", help="截止时间：YYYY-MM-DD 或 YYYY-MM-DD HH:MM")

    # todo list [--status pending|done] [--sort priority|created|due] [--json]
    p_list = sub.add_parser("list", help="列出任务")
    p_list.add_argument("--status", choices=["pending", "done"], help="按状态过滤")
    p_list.add_argument("--sort", choices=["priority", "created", "due"],
                        default="priority", help="排序：priority/created/due（默认 priority）")
    p_list.add_argument("--json", action="store_true", help="以 JSON 输出（机器可读）")

    # todo done <id> [<id> ...]
    p_done = sub.add_parser("done", help="标记任务完成（支持多个 ID）")
    p_done.add_argument("ids", type=int, nargs="+", help="任务 ID（可多个）")

    # todo undo <id>
    p_undo = sub.add_parser("undo", help="恢复任务为待办")
    p_undo.add_argument("id", type=int, help="任务 ID")

    # todo delete <id> [<id> ...]
    p_delete = sub.add_parser("delete", help="删除任务（支持多个 ID）")
    p_delete.add_argument("ids", type=int, nargs="+", help="任务 ID（可多个）")

    # todo edit <id> [--text ...] [--priority ...] [--due ...] [--no-due]
    p_edit = sub.add_parser("edit", help="修改任务")
    p_edit.add_argument("id", type=int, help="任务 ID")
    p_edit.add_argument("--text", help="新任务内容")
    p_edit.add_argument("--priority", choices=PRIORITIES.keys(), help="新优先级")
    p_edit.add_argument("--due", help="新截止时间：YYYY-MM-DD 或 YYYY-MM-DD HH:MM")
    p_edit.add_argument("--no-due", action="store_true", help="清除截止时间")

    # todo search <关键词> [--status pending|done] [--json]
    p_search = sub.add_parser("search", help="按关键词搜索任务")
    p_search.add_argument("keyword", help="搜索关键词")
    p_search.add_argument("--status", choices=["pending", "done"], help="按状态过滤")
    p_search.add_argument("--json", action="store_true", help="以 JSON 输出（机器可读）")

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
        return cmd_add(args.text.strip(), args.priority, args.due)
    if args.command == "list":
        return cmd_list(args.status, args.json, args.sort)
    if args.command == "done":
        return cmd_done(args.ids)
    if args.command == "undo":
        return cmd_undo(args.id)
    if args.command == "delete":
        return cmd_delete(args.ids)
    if args.command == "edit":
        return cmd_edit(args.id, args.text, args.priority, args.due, args.no_due)
    if args.command == "search":
        return cmd_search(args.keyword, args.status, args.json)
    if args.command == "stats":
        return cmd_stats()
    # 理论上到不了这里（argparse required=True 已拦截）
    return 0


if __name__ == "__main__":
    sys.exit(main())
