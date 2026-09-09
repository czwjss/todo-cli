//! todo —— 练手用命令行任务清单工具（Rust 版本）
//!
//! 功能与 Python 版本完全对齐：
//!     todo add "任务内容" --priority high|medium|low   添加任务
//!     todo list [--status pending|done] [--json]      列出任务
//!     todo done <id>                                  标记任务完成
//!     todo delete <id>                                删除任务
//!     todo stats                                      统计信息
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
    io::IsTerminal, // IsTerminal：判断 stdout 是否被终端读取（决定是否输出颜色）
    path::PathBuf,
    process,
};

use chrono::Local;
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

/// 单条任务
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Task {
    id: u64,
    text: String,
    priority: Priority,
    status: Status,
    created_at: String, // 创建日期，如 "2026-09-08"
}

/// 整个数据文件的结构
#[derive(Debug, Serialize, Deserialize)]
struct Data {
    next_id: u64,    // 下一个可用 ID（自增，删除后不复用）
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
    let dir = env::var("TODO_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            // 未指定时用用户主目录下的 .todo
            let home = env::var("HOME").expect("无法确定 HOME 目录");
            PathBuf::from(home).join(".todo")
        });
    dir.join("tasks.json")
}

/// 从 JSON 文件加载数据；文件不存在时返回空结构
fn load_data(path: &PathBuf) -> Data {
    if !path.exists() {
        return Data { next_id: 1, tasks: Vec::new() };
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
// 各子命令实现（返回 Result，错误统一在 main 里处理）
// ---------------------------------------------------------------------------

/// todo add：添加任务
fn cmd_add(path: &PathBuf, text: &str, priority: Priority) -> Result<(), String> {
    if text.trim().is_empty() {
        return Err("任务内容不能为空。".to_string());
    }
    let mut data = load_data(path);
    // 新任务：自增 ID + 当前日期
    let task = Task {
        id: data.next_id,
        text: text.trim().to_string(),
        priority,
        status: Status::Pending,
        created_at: Local::now().format("%Y-%m-%d").to_string(),
    };
    data.next_id += 1;
    data.tasks.push(task.clone());
    save_data(path, &data);
    println!("已添加任务 #{}: {}", task.id, task.text);
    Ok(())
}

/// todo list：列出任务
fn cmd_list(path: &PathBuf, status_filter: Option<Status>, as_json: bool) -> Result<(), String> {
    let data = load_data(path);
    // 按状态过滤；未指定则全部
    let mut tasks: Vec<&Task> = data
        .tasks
        .iter()
        .filter(|t| status_filter.is_none() || t.status == status_filter.unwrap())
        .collect();
    // 排序：待办在前；同状态下高优先级在前
    tasks.sort_by_key(|t| (t.status != Status::Pending, t.priority.weight()));

    if as_json {
        // 机器可读输出
        println!("{}", serde_json::to_string_pretty(&tasks).map_err(|e| e.to_string())?);
        return Ok(());
    }

    if tasks.is_empty() {
        let hint = match status_filter {
            Some(s) => format!("（没有任务，状态：{}）", if s == Status::Done { "done" } else { "pending" }),
            None => "（没有任务）".to_string(),
        };
        println!("{}", hint);
        return Ok(());
    }

    // 计算各列显示宽度（中文按 2 列），保证对齐
    let id_w = tasks.iter().map(|t| t.id.to_string().width()).max().unwrap_or(2).max(2);
    let text_w = tasks.iter().map(|t| t.text.width()).max().unwrap_or(2).max(4);

    let header = format!(
        "{}  {} {} {}  创建日期",
        pad("ID", id_w),
        pad("状态", 4),
        pad("优先级", 4),
        pad("任务", text_w),
    );
    println!("{}", header);
    println!("{}", "-".repeat(header.width()));
    for t in tasks {
        let status = if t.status == Status::Done { "完成" } else { "待办" };
        println!(
            "{}  {} {} {}  {}",
            pad(&t.id.to_string(), id_w),
            pad(status, 4),
            pad(&colored_priority(t.priority), 4),
            pad(&t.text, text_w),
            t.created_at,
        );
    }
    Ok(())
}

/// 按 ID 查找任务；找不到返回错误信息
fn find_task<'a>(data: &'a Data, task_id: u64) -> Result<&'a Task, String> {
    data.tasks
        .iter()
        .find(|t| t.id == task_id)
        .ok_or_else(|| format!("不存在 ID 为 {} 的任务。可用 todo list 查看。", task_id))
}

/// todo done：标记完成（幂等）
fn cmd_done(path: &PathBuf, task_id: u64) -> Result<(), String> {
    let mut data = load_data(path);
    // 先验证任务存在（不存在直接报错，避免下面 unwrap）
    find_task(&data, task_id)?;
    // 在一个块内完成"修改状态 + 取出要打印的信息"，
    // 块结束后可变借用即释放，之后才能安全地不可变借用 data 保存文件
    let (id, text, already_done) = {
        let task = data
            .tasks
            .iter_mut()
            .find(|t| t.id == task_id)
            .expect("上一步已验证存在");
        let already = task.status == Status::Done;
        if !already {
            task.status = Status::Done;
        }
        (task.id, task.text.clone(), already) // 拷贝出 id 和文本，避免借用残留
    };
    save_data(path, &data); // 此处 data 只剩不可变借用，合法
    if already_done {
        println!("任务 #{} 已是完成状态。", id);
    } else {
        println!("已完成任务 #{}: {}", id, text);
    }
    Ok(())
}

/// todo delete：删除任务
fn cmd_delete(path: &PathBuf, task_id: u64) -> Result<(), String> {
    let mut data = load_data(path);
    // retain：保留所有 id 不匹配的任务，即删除目标任务
    let removed = data.tasks.iter().find(|t| t.id == task_id).cloned();
    match removed {
        Some(task) => {
            data.tasks.retain(|t| t.id != task_id);
            save_data(path, &data);
            println!("已删除任务 #{}: {}", task.id, task.text);
            Ok(())
        }
        None => Err(format!("不存在 ID 为 {} 的任务。可用 todo list 查看。", task_id)),
    }
}

/// todo stats：统计信息
fn cmd_stats(path: &PathBuf) -> Result<(), String> {
    let data = load_data(path);
    let total = data.tasks.len();
    let done = data.tasks.iter().filter(|t| t.status == Status::Done).count();
    let pending = total - done;
    println!("任务总数：{}", total);
    println!("已完成：{}", done);
    println!("待办：{}", pending);
    if total > 0 {
        println!("待办优先级分布：");
        for p in [Priority::High, Priority::Medium, Priority::Low] {
            let count = data
                .tasks
                .iter()
                .filter(|t| t.status == Status::Pending && t.priority == p)
                .count();
            // 简易进度条：每项一个 # 符号
            let bar = if count > 0 { "#".repeat(count) } else { "-".to_string() };
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
    },
    /// 列出任务
    #[command(disable_help_flag = true)]
    List {
        /// 按状态过滤：pending/done
        #[arg(long, help_heading = "选项")]
        status: Option<Status>,
        /// 以 JSON 输出（机器可读）
        #[arg(long, help_heading = "选项")]
        json: bool,
    },
    /// 标记任务完成
    #[command(disable_help_flag = true)]
    Done {
        /// 任务 ID
        #[arg(help_heading = "选项")]
        id: u64,
    },
    /// 删除任务
    #[command(disable_help_flag = true)]
    Delete {
        /// 任务 ID
        #[arg(help_heading = "选项")]
        id: u64,
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
        .help_template(TEMPLATE)            // 中文模板（硬编码"用法："）
        .subcommand_help_heading("子命令");  // 子命令列表标题
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

fn main() {
    // 先取 clap 自动生成的命令，做本地化后再解析
    let mut cmd = Cli::command();
    localize_help(&mut cmd);
    // 参数错误（未知参数/缺参数）时 clap 自动打印错误并退出（退出码 2）
    let mut matches = cmd.get_matches();
    let cli = Cli::from_arg_matches_mut(&mut matches).unwrap_or_else(|e| e.exit());
    let path = data_file();

    // 分发子命令；Result 统一在这里处理错误与退出码
    let result = match cli.command {
        Commands::Add { text, priority } => cmd_add(&path, &text, priority),
        Commands::List { status, json } => cmd_list(&path, status, json),
        Commands::Done { id } => cmd_done(&path, id),
        Commands::Delete { id } => cmd_delete(&path, id),
        Commands::Stats => cmd_stats(&path),
    };

    if let Err(msg) = result {
        // 业务错误：stderr 输出 + 退出码 1（clig.dev 约定：非零表示失败）
        eprintln!("错误：{}", msg);
        process::exit(1);
    }
    // 成功路径：main 正常返回，退出码 0
}
