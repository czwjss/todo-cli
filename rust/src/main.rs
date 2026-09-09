//! todo —— 练手用命令行任务清单工具（Rust 版本）
//!
//! 功能与 Python 版本完全对齐：
//!     todo add "任务内容" [--priority high|medium|low] [--due "2026-09-09 18:00"]
//!     todo list [--status pending|done] [--sort priority|created|due] [--json]
//!     todo done <id> [<id> ...]                         标记任务完成（支持多个）
//!     todo undo <id>                                    恢复任务为待办
//!     todo delete <id> [<id> ...]                       删除任务（支持多个）
//!     todo edit <id> [--text 新内容] [--priority high] [--due 截止] [--no-due]
//!     todo search <关键词> [--status pending|done] [--json]
//!     todo stats                                        统计信息（含逾期数）
//!
//! 设计要点（对应 clig.dev 指南）：
//!     - 使用 clap 解析参数：自动生成 -h/--help、用法错误（退出码 2）
//!     - 成功退出码 0，业务错误退出码 1，参数错误退出码 2
//!     - 正常输出走 stdout，错误信息走 stderr
//!     - 提供 --json 机器可读输出
//!
//! 存储：默认 ~/.todo/tasks.json；可用环境变量 TODO_DIR / TODO_FILE 覆盖（便于测试）

use std::{
    env,
    fs,
    io::{self, IsTerminal, Write}, // IsTerminal/Write：判断终端并刷新输出（更新提示用）
    path::PathBuf,
    process,
};

use chrono::{Local, NaiveDate, NaiveDateTime};
use clap::{Arg, ArgAction, CommandFactory, FromArgMatches, Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};
use unicode_width::UnicodeWidthStr;

// ---------------------------------------------------------------------------
// 数据模型
// ---------------------------------------------------------------------------

/// 任务优先级：带显示标签、终端颜色与排序权重
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")] // JSON 中存 "high"/"medium"/"low"（与 Python 版本一致）
#[value(rename_all = "lowercase")] // 命令行输入同样接受小写
enum Priority {
    High,
    Medium,
    Low,
}

impl Priority {
    /// 中文显示标签
    fn label(self) -> &'static str {
        match self {
            Priority::High => "高",
            Priority::Medium => "中",
            Priority::Low => "低",
        }
    }

    /// ANSI 颜色码：高=红、中=黄、低=绿
    fn color(self) -> &'static str {
        match self {
            Priority::High => "\x1b[31m",
            Priority::Medium => "\x1b[33m",
            Priority::Low => "\x1b[32m",
        }
    }

    /// 排序权重：数值越小优先级越高（用于 list 排序）
    fn weight(self) -> u8 {
        match self {
            Priority::High => 0,
            Priority::Medium => 1,
            Priority::Low => 2,
        }
    }
}

/// 任务状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
#[value(rename_all = "lowercase")]
enum Status {
    Pending, // 待办
    Done,    // 已完成
}

/// 列表排序方式
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "lowercase")]
enum SortBy {
    Priority, // 按优先级（默认）
    Created,  // 按创建日期
    Due,      // 按截止时间（无截止的排最后）
}

/// 单条任务
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Task {
    id: u64,
    text: String,
    priority: Priority,
    status: Status,
    #[serde(default)] // 兼容旧数据文件（无 due 字段）
    due: Option<String>, // 截止时间，如 "2026-09-09 18:00" 或 "2026-09-09"；None 表示无
    created_at: String, // 创建日期，如 "2026-09-08"
}

/// 整个数据文件的结构
#[derive(Debug, Serialize, Deserialize)]
struct Data {
    next_id: u64, // 下一个可用 ID（自增，删除后不复用）
    tasks: Vec<Task>,
}

// ---------------------------------------------------------------------------
// 数据存储
// ---------------------------------------------------------------------------

/// 解析数据文件路径：TODO_FILE > TODO_DIR/tasks.json > ~/.todo/tasks.json
fn data_file() -> PathBuf {
    if let Ok(path) = env::var("TODO_FILE") {
        return PathBuf::from(path);
    }
    let dir = env::var("TODO_DIR").map(PathBuf::from).unwrap_or_else(|_| {
        // 未指定时用用户主目录下的 .todo
        let home = env::var("HOME").expect("无法确定 HOME 目录");
        PathBuf::from(home).join(".todo")
    });
    dir.join("tasks.json")
}

/// 从 JSON 文件加载数据；文件不存在时返回空结构
fn load_data(path: &PathBuf) -> Data {
    if !path.exists() {
        return Data {
            next_id: 1,
            tasks: Vec::new(),
        };
    }
    let raw = fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("错误：无法读取数据文件 {}（{}）", path.display(), e);
        process::exit(1);
    });
    serde_json::from_str(&raw).unwrap_or_else(|e| {
        eprintln!("错误：数据文件 {} 解析失败（{}）", path.display(), e);
        eprintln!("提示：文件损坏时可删除该文件后重试。");
        process::exit(1);
    })
}

/// 把数据写回 JSON 文件（自动创建父目录）
fn save_data(path: &PathBuf, data: &Data) {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).unwrap_or_else(|e| {
            eprintln!("错误：无法创建目录 {}（{}）", dir.display(), e);
            process::exit(1);
        });
    }
    // serde_json 序列化为带缩进的 JSON，ensure_ascii=false 保留中文
    let raw = serde_json::to_string_pretty(data).expect("序列化不应失败");
    fs::write(path, raw).unwrap_or_else(|e| {
        eprintln!("错误：无法写入数据文件 {}（{}）", path.display(), e);
        process::exit(1);
    });
}

// ---------------------------------------------------------------------------
// 输出辅助
// ---------------------------------------------------------------------------

/// 颜色重置码
const RESET: &str = "\x1b[0m";
/// 红色（逾期标记）
const RED: &str = "\x1b[31m";

/// 带颜色的优先级标签；仅当 stdout 是终端时上色（管道/重定向时保持纯文本）
fn colored_priority(p: Priority) -> String {
    if std::io::stdout().is_terminal() {
        format!("{}{}{}", p.color(), p.label(), RESET)
    } else {
        p.label().to_string()
    }
}

/// 按显示宽度补齐：中文字符占 2 列，ASCII 占 1 列
fn pad(text: &str, width: usize) -> String {
    let pad_len = width.saturating_sub(text.width());
    format!("{}{}", text, " ".repeat(pad_len))
}

// ---------------------------------------------------------------------------
// 截止时间（due）辅助
// ---------------------------------------------------------------------------

/// 解析截止时间字符串为日期时间；支持 "YYYY-MM-DD"（当天 23:59:59 截止）与 "YYYY-MM-DD HH:MM"
fn parse_due(s: &str) -> Result<NaiveDateTime, String> {
    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M") {
        return Ok(dt);
    }
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Ok(d.and_hms_opt(23, 59, 59).expect("23:59:59 合法"));
    }
    Err("截止时间格式应为 YYYY-MM-DD 或 YYYY-MM-DD HH:MM。".to_string())
}

/// 任务是否已逾期（未完成且截止时间早于当前时间）
fn due_overdue(task: &Task) -> bool {
    if task.status != Status::Pending {
        return false;
    }
    match &task.due {
        Some(s) => parse_due(s)
            .map(|dt| dt < Local::now().naive_local())
            .unwrap_or(false),
        None => false,
    }
}

/// 截止时间单元格文本：无截止显示 "-"，逾期追加红色 "!"（非终端不加色）
fn due_cell(task: &Task) -> String {
    match &task.due {
        None => "-".to_string(),
        Some(s) => {
            if due_overdue(task) {
                if std::io::stdout().is_terminal() {
                    format!("{}{}!{}", RED, s, RESET)
                } else {
                    format!("{}!", s)
                }
            } else {
                s.clone()
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 各子命令实现（返回 Result，错误统一在 main 里处理）
// ---------------------------------------------------------------------------

/// todo add：添加任务
fn cmd_add(
    path: &PathBuf,
    text: &str,
    priority: Priority,
    due: Option<&str>,
) -> Result<(), String> {
    if text.trim().is_empty() {
        return Err("任务内容不能为空。".to_string());
    }
    // 校验并规范化截止时间（空白视为未提供）
    let due = due
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| parse_due(s).map(|_| s.to_string()))
        .transpose()?;

    let mut data = load_data(path);
    let task = Task {
        id: data.next_id,
        text: text.trim().to_string(),
        priority,
        status: Status::Pending,
        due,
        created_at: Local::now().format("%Y-%m-%d").to_string(),
    };
    data.next_id += 1;
    data.tasks.push(task.clone());
    save_data(path, &data);
    println!("已添加任务 #{}: {}", task.id, task.text);
    Ok(())
}

/// 按状态/关键词收集任务（关键词匹配文本，不区分大小写）
fn collect_tasks<'a>(
    data: &'a Data,
    status_filter: Option<Status>,
    keyword: Option<&str>,
) -> Vec<&'a Task> {
    let kw = keyword.map(str::to_lowercase);
    data.tasks
        .iter()
        .filter(|t| status_filter.is_none() || t.status == status_filter.unwrap())
        .filter(|t| match &kw {
            Some(k) => t.text.to_lowercase().contains(k.as_str()),
            None => true,
        })
        .collect()
}

/// 排序：待办始终在前；同状态下按 --sort 指定的键排序
fn sort_tasks(tasks: &mut Vec<&Task>, sort: SortBy) {
    match sort {
        SortBy::Priority => {
            tasks.sort_by_key(|t| (t.status != Status::Pending, t.priority.weight(), t.id))
        }
        SortBy::Created => {
            tasks.sort_by_key(|t| (t.status != Status::Pending, t.created_at.clone(), t.id))
        }
        SortBy::Due => tasks.sort_by_key(|t| {
            (
                t.status != Status::Pending,
                t.due.is_none(), // 无截止的排最后
                t.due.clone().unwrap_or_default(),
                t.id,
            )
        }),
    }
}

/// 渲染任务列表：--json 输出 JSON，否则输出对齐表格
fn render_tasks(
    tasks: &[&Task],
    status_filter: Option<Status>,
    as_json: bool,
) -> Result<(), String> {
    if as_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&tasks).map_err(|e| e.to_string())?
        );
        return Ok(());
    }

    if tasks.is_empty() {
        let hint = match status_filter {
            Some(s) => format!(
                "（没有任务，状态：{}）",
                if s == Status::Done { "done" } else { "pending" }
            ),
            None => "（没有任务）".to_string(),
        };
        println!("{}", hint);
        return Ok(());
    }

    // 计算各列显示宽度（中文按 2 列），保证对齐
    let id_w = tasks
        .iter()
        .map(|t| t.id.to_string().width())
        .max()
        .unwrap_or(2)
        .max(2);
    let text_w = tasks
        .iter()
        .map(|t| t.text.width())
        .max()
        .unwrap_or(2)
        .max(4);
    let due_w = tasks
        .iter()
        .map(|t| due_cell(t).width())
        .max()
        .unwrap_or(4)
        .max(4);

    let header = format!(
        "{}  {} {} {}  {}  {}",
        pad("ID", id_w),
        pad("状态", 4),
        pad("优先级", 4),
        pad("任务", text_w),
        pad("截止", due_w),
        "创建日期",
    );
    println!("{}", header);
    println!("{}", "-".repeat(header.width()));
    for t in tasks {
        let status = if t.status == Status::Done {
            "完成"
        } else {
            "待办"
        };
        println!(
            "{}  {} {} {}  {}  {}",
            pad(&t.id.to_string(), id_w),
            pad(status, 4),
            pad(&colored_priority(t.priority), 4),
            pad(&t.text, text_w),
            pad(&due_cell(t), due_w),
            t.created_at,
        );
    }
    Ok(())
}

/// todo list：列出任务
fn cmd_list(
    path: &PathBuf,
    status_filter: Option<Status>,
    as_json: bool,
    sort: SortBy,
) -> Result<(), String> {
    let data = load_data(path);
    let mut tasks = collect_tasks(&data, status_filter, None);
    sort_tasks(&mut tasks, sort);
    render_tasks(&tasks, status_filter, as_json)
}

/// todo search：按关键词搜索任务
fn cmd_search(
    path: &PathBuf,
    keyword: &str,
    status_filter: Option<Status>,
    as_json: bool,
) -> Result<(), String> {
    if keyword.trim().is_empty() {
        return Err("搜索关键词不能为空。".to_string());
    }
    let data = load_data(path);
    let mut tasks = collect_tasks(&data, status_filter, Some(keyword.trim()));
    sort_tasks(&mut tasks, SortBy::Priority);
    render_tasks(&tasks, status_filter, as_json)
}

/// 按 ID 查找任务；找不到返回错误信息
fn find_task(data: &Data, task_id: u64) -> Result<&Task, String> {
    data.tasks
        .iter()
        .find(|t| t.id == task_id)
        .ok_or_else(|| format!("不存在 ID 为 {} 的任务。可用 todo list 查看。", task_id))
}

/// todo done：批量标记完成（幂等；任一 ID 不存在则不修改并报错）
fn cmd_done(path: &PathBuf, ids: &[u64]) -> Result<(), String> {
    let mut data = load_data(path);
    // 先全部验证存在，避免部分成功
    for id in ids {
        find_task(&data, *id)?;
    }
    for id in ids {
        let task = data
            .tasks
            .iter_mut()
            .find(|t| t.id == *id)
            .expect("上一步已验证存在");
        if task.status == Status::Done {
            println!("任务 #{} 已是完成状态。", id);
        } else {
            println!("已完成任务 #{}: {}", task.id, task.text);
            task.status = Status::Done;
        }
    }
    save_data(path, &data);
    Ok(())
}

/// todo undo：把任务恢复为待办
fn cmd_undo(path: &PathBuf, task_id: u64) -> Result<(), String> {
    let mut data = load_data(path);
    let task = data
        .tasks
        .iter_mut()
        .find(|t| t.id == task_id)
        .ok_or_else(|| format!("不存在 ID 为 {} 的任务。可用 todo list 查看。", task_id))?;
    if task.status == Status::Pending {
        println!("任务 #{} 已是待办状态。", task_id);
    } else {
        println!("已恢复任务 #{}: {}", task.id, task.text);
        task.status = Status::Pending;
    }
    save_data(path, &data);
    Ok(())
}

/// todo delete：批量删除任务（任一 ID 不存在则不修改并报错）
fn cmd_delete(path: &PathBuf, ids: &[u64]) -> Result<(), String> {
    let mut data = load_data(path);
    for id in ids {
        find_task(&data, *id)?;
    }
    for id in ids {
        let removed = data
            .tasks
            .iter()
            .find(|t| t.id == *id)
            .cloned()
            .expect("上一步已验证存在");
        data.tasks.retain(|t| t.id != *id);
        println!("已删除任务 #{}: {}", removed.id, removed.text);
    }
    save_data(path, &data);
    Ok(())
}

/// todo edit：修改任务文本/优先级/截止时间（至少提供一项）
fn cmd_edit(
    path: &PathBuf,
    task_id: u64,
    new_text: Option<&str>,
    new_priority: Option<Priority>,
    new_due: Option<Option<String>>, // Some(Some(s)) 设置截止；Some(None) 清除截止
) -> Result<(), String> {
    if new_text.is_none() && new_priority.is_none() && new_due.is_none() {
        return Err("请至少提供一项修改：--text / --priority / --due / --no-due。".to_string());
    }
    let mut data = load_data(path);
    // 在一个块内完成修改并拷贝出要打印的信息，块结束后可变借用释放
    let (id, text) = {
        let task = data
            .tasks
            .iter_mut()
            .find(|t| t.id == task_id)
            .ok_or_else(|| format!("不存在 ID 为 {} 的任务。可用 todo list 查看。", task_id))?;

        if let Some(t) = new_text {
            if t.trim().is_empty() {
                return Err("任务内容不能为空。".to_string());
            }
            task.text = t.trim().to_string();
        }
        if let Some(p) = new_priority {
            task.priority = p;
        }
        if let Some(due_opt) = new_due {
            task.due = match due_opt {
                Some(s) => {
                    let s = s.trim().to_string();
                    if s.is_empty() {
                        None
                    } else {
                        Some(parse_due(&s).map(|_| s)?)
                    }
                }
                None => None,
            };
        }
        (task.id, task.text.clone())
    };
    save_data(path, &data);
    println!("已更新任务 #{}: {}", id, text);
    Ok(())
}

/// todo stats：统计信息（总数/完成/待办/逾期/优先级分布）
fn cmd_stats(path: &PathBuf) -> Result<(), String> {
    let data = load_data(path);
    let total = data.tasks.len();
    let done = data
        .tasks
        .iter()
        .filter(|t| t.status == Status::Done)
        .count();
    let pending = total - done;
    let overdue = data.tasks.iter().filter(|t| due_overdue(t)).count();
    println!("任务总数：{}", total);
    println!("已完成：{}", done);
    println!("待办：{}", pending);
    if pending > 0 {
        println!("已逾期：{}", overdue);
    }
    if total > 0 {
        println!("待办优先级分布：");
        for p in [Priority::High, Priority::Medium, Priority::Low] {
            let count = data
                .tasks
                .iter()
                .filter(|t| t.status == Status::Pending && t.priority == p)
                .count();
            // 简易进度条：每项一个 # 符号
            let bar = if count > 0 {
                "#".repeat(count)
            } else {
                "-".to_string()
            };
            println!("  {}优先级：{}  {}", p.label(), count, bar);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 命令行参数定义（clap derive）
// ---------------------------------------------------------------------------

/// 顶层命令
#[derive(Parser)]
#[command(
    name = "todo",
    version,
    about = "练手用命令行任务清单工具",
    disable_help_flag = true,           // 禁用内置英文 help，改由 localize_help 添加中文版
    disable_version_flag = true,        // 禁用内置英文 version，改由 localize_help 添加中文版
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

/// 子命令
#[derive(Subcommand)]
enum Commands {
    /// 添加任务
    #[command(disable_help_flag = true)]
    Add {
        /// 任务内容（位置参数）
        #[arg(help_heading = "选项")]
        text: String,
        /// 优先级：high/medium/low（默认 medium）
        #[arg(short, long, help_heading = "选项", default_value = "medium")]
        priority: Priority,
        /// 截止时间：YYYY-MM-DD 或 YYYY-MM-DD HH:MM
        #[arg(long, help_heading = "选项")]
        due: Option<String>,
    },
    /// 列出任务
    #[command(disable_help_flag = true)]
    List {
        /// 按状态过滤：pending/done
        #[arg(long, help_heading = "选项")]
        status: Option<Status>,
        /// 排序：priority/created/due（默认 priority）
        #[arg(long, help_heading = "选项", default_value = "priority")]
        sort: SortBy,
        /// 以 JSON 输出（机器可读）
        #[arg(long, help_heading = "选项")]
        json: bool,
    },
    /// 标记任务完成（支持多个 ID）
    #[command(disable_help_flag = true)]
    Done {
        /// 任务 ID（可多个）
        #[arg(help_heading = "选项", required = true, num_args = 1..)]
        ids: Vec<u64>,
    },
    /// 恢复任务为待办
    #[command(disable_help_flag = true)]
    Undo {
        /// 任务 ID
        #[arg(help_heading = "选项")]
        id: u64,
    },
    /// 删除任务（支持多个 ID）
    #[command(disable_help_flag = true)]
    Delete {
        /// 任务 ID（可多个）
        #[arg(help_heading = "选项", required = true, num_args = 1..)]
        ids: Vec<u64>,
    },
    /// 修改任务
    #[command(disable_help_flag = true)]
    Edit {
        /// 任务 ID
        #[arg(help_heading = "选项")]
        id: u64,
        /// 新任务内容
        #[arg(long, help_heading = "选项")]
        text: Option<String>,
        /// 新优先级：high/medium/low
        #[arg(long, help_heading = "选项")]
        priority: Option<Priority>,
        /// 新截止时间：YYYY-MM-DD 或 YYYY-MM-DD HH:MM
        #[arg(long, help_heading = "选项", conflicts_with = "no_due")]
        due: Option<String>,
        /// 清除截止时间
        #[arg(long, help_heading = "选项", conflicts_with = "due")]
        no_due: bool,
    },
    /// 按关键词搜索任务
    #[command(disable_help_flag = true)]
    Search {
        /// 搜索关键词（位置参数）
        #[arg(help_heading = "选项")]
        keyword: String,
        /// 按状态过滤：pending/done
        #[arg(long, help_heading = "选项")]
        status: Option<Status>,
        /// 以 JSON 输出（机器可读）
        #[arg(long, help_heading = "选项")]
        json: bool,
    },
    /// 显示统计信息
    #[command(disable_help_flag = true)]
    Stats,
}

// ---------------------------------------------------------------------------
// 入口
// ---------------------------------------------------------------------------

/// 统一本地化帮助文本：对所有命令（含子命令）设置中文模板与标题
fn localize_help(cmd: &mut clap::Command) {
    const TEMPLATE: &str = "{about-section}\n用法：{usage}\n\n{all-args}";
    // clap 的这些方法消耗 self（builder 模式），所以先取出所有权，处理后写回
    let mut c = std::mem::take(cmd);
    c = c
        .help_template(TEMPLATE) // 中文模板（硬编码"用法："）
        .subcommand_help_heading("子命令"); // 子命令列表标题
    // 顶层添加中文 -h/--help 与 -V/--version（builder 方式，绕开 derive 的 required 问题）
    c = c
        .arg(
            Arg::new("help")
                .short('h')
                .long("help")
                .help("显示帮助信息")
                .help_heading("选项")
                .action(ArgAction::Help),
        )
        .arg(
            Arg::new("version")
                .short('V')
                .long("version")
                .help("显示版本信息")
                .help_heading("选项")
                .action(ArgAction::Version),
        );
    // 递归处理所有子命令：中文模板/标题 + 中文 -h/--help
    for sub in c.get_subcommands_mut() {
        *sub = sub
            .clone() // 克隆一份出来链式修改，避免移动借用
            .help_template(TEMPLATE)
            .subcommand_help_heading("子命令")
            .arg(
                Arg::new("help")
                    .short('h')
                    .long("help")
                    .help("显示帮助信息")
                    .help_heading("选项")
                    .action(ArgAction::Help),
            );
    }
    *cmd = c; // 写回
}

/// 运行时检查更新：有新版本时提示用户，用户确认后再自动下载并替换自身。
/// 仅在交互终端执行，避免阻塞脚本/管道；每 24 小时最多检查一次。
fn maybe_check_update() {
    use self_update::check_interval::UpdateCheckGuard;
    use std::time::Duration;

    // 非交互（管道/脚本/重定向）不提示，保持原有行为
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return;
    }

    // 检查频率控制：避免每次运行都联网
    let stamp = env::temp_dir().join("todo-cli-update-check");
    let guard = UpdateCheckGuard::new(stamp, Duration::from_secs(24 * 60 * 60));
    if !guard.should_check().unwrap_or(true) {
        return;
    }

    let build_updater = |no_confirm: bool| {
        self_update::backends::github::Update::configure()
            .repo_owner("czwjss")
            .repo_name("todo-cli")
            .bin_name("todo")
            .current_version(env!("CARGO_PKG_VERSION"))
            .target(self_update::get_target())
            .show_output(false)
            .no_confirm(no_confirm)
            .build()
    };

    let updater = match build_updater(false) {
        Ok(u) => u,
        Err(_) => return,
    };

    let newer = match updater.is_update_available() {
        Ok(Some(release)) => release,
        Ok(None) => {
            let _ = guard.record_check();
            return;
        }
        Err(_) => {
            let _ = guard.record_check();
            return;
        }
    };

    print!(
        "发现新版本 v{}（当前 v{}），是否更新？[y/N] ",
        newer.version(),
        env!("CARGO_PKG_VERSION")
    );
    let _ = io::stdout().flush();

    let mut answer = String::new();
    if io::stdin().read_line(&mut answer).is_err() {
        let _ = guard.record_check();
        return;
    }
    if !matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
        println!("已跳过更新。");
        let _ = guard.record_check();
        return;
    }

    match build_updater(true).and_then(|u| u.update()) {
        Ok(status) => println!("已更新到 v{}", status.version()),
        Err(e) => eprintln!("更新失败：{e}"),
    }
    let _ = guard.record_check();
}

fn main() {
    // 先取 clap 自动生成的命令，做本地化后再解析
    let mut cmd = Cli::command();
    localize_help(&mut cmd);
    // 参数错误（未知参数/缺参数）时 clap 自动打印错误并退出（退出码 2）
    let mut matches = cmd.get_matches();
    let cli = Cli::from_arg_matches_mut(&mut matches).unwrap_or_else(|e| e.exit());
    let path = data_file();

    // 运行时自更新检查（仅在交互终端且有新版本时提示）
    maybe_check_update();

    // 分发子命令；Result 统一在这里处理错误与退出码
    let result = match cli.command {
        Commands::Add {
            text,
            priority,
            due,
        } => cmd_add(&path, &text, priority, due.as_deref()),
        Commands::List { status, sort, json } => cmd_list(&path, status, json, sort),
        Commands::Done { ids } => cmd_done(&path, &ids),
        Commands::Undo { id } => cmd_undo(&path, id),
        Commands::Delete { ids } => cmd_delete(&path, &ids),
        Commands::Edit {
            id,
            text,
            priority,
            due,
            no_due,
        } => {
            let new_due = if no_due { Some(None) } else { due.map(Some) };
            cmd_edit(&path, id, text.as_deref(), priority, new_due)
        }
        Commands::Search {
            keyword,
            status,
            json,
        } => cmd_search(&path, &keyword, status, json),
        Commands::Stats => cmd_stats(&path),
    };

    if let Err(msg) = result {
        // 业务错误：stderr 输出 + 退出码 1（clig.dev 约定：非零表示失败）
        eprintln!("错误：{}", msg);
        process::exit(1);
    }
    // 成功路径：main 正常返回，退出码 0
}
