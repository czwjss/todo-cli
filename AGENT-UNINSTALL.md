# AGENT-UNINSTALL.md —— todo CLI 卸载指引（面向 AI Agent）

本文件用于让 AI Agent 完整卸载 todo CLI。命令可直接复制运行。

## 1. 确认安装方式

```bash
which todo        # 二进制位置（如 ~/.local/bin/todo）
ls ~/.cargo/bin/todo 2>/dev/null   # 是否通过 cargo 安装过
```

## 2. 删除二进制

```bash
rm "${HOME}/.local/bin/todo"   # Release 二进制安装位置（与安装指引一致）
# 若还通过 cargo 安装过，一并卸载：
cargo uninstall todo 2>/dev/null
```

## 3. 清理 PATH 配置（如安装时添加过）

安装脚本在 `~/.zshrc` 末尾追加过一行 `export PATH="$HOME/.local/bin:$PATH"`。
若该目录不再需要（没有其他工具使用），移除这一行：

```bash
# 删除 ~/.zshrc 中指向 ~/.local/bin 的 PATH 行（仅当确认无其他工具使用）
sed -i '' '/.local\/bin/d' "${HOME}/.zshrc"
# 重新加载配置
source "${HOME}/.zshrc"
```

> 若其他工具也装在 ~/.local/bin，跳过此步，仅删除 todo 文件。

## 4. 清理数据（可选）

```bash
rm -rf "${HOME}/.todo"   # 任务数据（tasks.json）。确认不再需要再删
```

## 5. 验证卸载完成

```bash
which todo && echo "仍存在" || echo "已卸载"   # 期望输出：已卸载
todo --version 2>/dev/null && echo "仍可运行" || echo "命令已不可用"  # 期望输出：命令已不可用
```
