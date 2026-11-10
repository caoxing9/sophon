//! The UI's languages: English (the source strings) and Simplified Chinese.
//!
//! `t("Settings")` gives the string in the active language. The English
//! string is the key, so an untranslated one shows in English. The language
//! is the user's choice in Settings, or the system's (`Lang::Auto`); a
//! change re-mounts the UI (`ui::shell` reads [`revision`]).

use std::{
    collections::HashMap,
    sync::{
        OnceLock,
        atomic::{AtomicU8, Ordering},
    },
};

use nsh_core::prelude::*;

/// The language the user chose.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum Lang {
    /// The system's.
    #[default]
    Auto,
    En,
    Zh,
}

impl Lang {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Lang::Auto => "auto",
            Lang::En => "en",
            Lang::Zh => "zh",
        }
    }

    pub(crate) fn parse(s: &str) -> Lang {
        match s {
            "en" => Lang::En,
            "zh" => Lang::Zh,
            _ => Lang::Auto,
        }
    }
}

/// The choice, as a signal the shell re-mounts on.
#[derive(Clone, Default)]
struct Choice(Lang);

/// Whether strings come out in Chinese: 0 unset, 1 English, 2 Chinese. An
/// atomic so `t` works off the UI thread and outside a reactive scope.
static ZH: AtomicU8 = AtomicU8::new(0);

pub(crate) fn lang() -> Lang {
    global::<Choice>().with(|c| c.0)
}

pub(crate) fn lang_untracked() -> Lang {
    global::<Choice>().with_untracked(|c| c.0)
}

pub(crate) fn set_lang(lang: Lang) {
    ZH.store(if resolve(lang) { 2 } else { 1 }, Ordering::Relaxed);
    global::<Choice>().set(Choice(lang));
}

/// Whether the UI is in Chinese now.
pub(crate) fn is_zh() -> bool {
    match ZH.load(Ordering::Relaxed) {
        0 => {
            let zh = resolve(Lang::Auto);
            ZH.store(if zh { 2 } else { 1 }, Ordering::Relaxed);
            zh
        }
        v => v == 2,
    }
}

fn resolve(lang: Lang) -> bool {
    // `SOPHON_LANG=en|zh` wins over the setting too (tests, screenshots).
    if let Ok(env) = std::env::var("SOPHON_LANG") {
        return env.to_ascii_lowercase().starts_with("zh");
    }
    match lang {
        Lang::En => false,
        Lang::Zh => true,
        Lang::Auto => system_is_zh(),
    }
}

/// The system's first preferred language: macOS's list, else the locale
/// variables. Unknown is English.
fn system_is_zh() -> bool {
    static SYSTEM: OnceLock<bool> = OnceLock::new();
    *SYSTEM.get_or_init(|| {
        // `SOPHON_LANG=en|zh` overrides the system (tests, screenshots).
        if let Ok(lang) = std::env::var("SOPHON_LANG") {
            return lang.to_ascii_lowercase().starts_with("zh");
        }
        #[cfg(target_os = "macos")]
        if let Ok(out) = std::process::Command::new("defaults").args(["read", "-g", "AppleLanguages"]).output()
            && let Some(first) = String::from_utf8_lossy(&out.stdout)
                .split(|c: char| c == '"' || c == ',' || c.is_whitespace() || c == '(' || c == ')')
                .find(|s| !s.is_empty())
        {
            return first.to_ascii_lowercase().starts_with("zh");
        }
        ["LC_ALL", "LC_MESSAGES", "LANG"]
            .iter()
            .filter_map(|k| std::env::var(k).ok())
            .find(|v| !v.is_empty() && v != "C" && v != "POSIX")
            .is_some_and(|v| v.to_ascii_lowercase().starts_with("zh"))
    })
}

/// `en` in the active language.
pub(crate) fn t(en: &'static str) -> &'static str {
    if !is_zh() {
        return en;
    }
    table().get(en).copied().unwrap_or(en)
}

/// `en` from the daemon side (a notice), in the active language when the
/// table has it.
pub(crate) fn tr(en: &str) -> String {
    if !is_zh() {
        return en.to_owned();
    }
    table().get(en).map(|s| (*s).to_owned()).unwrap_or_else(|| en.to_owned())
}

/// `en`, a template with `{}` placeholders, in the active language, with
/// `args` in its placeholders in order.
pub(crate) fn tf(en: &'static str, args: &[&str]) -> String {
    let mut out = String::new();
    let mut args = args.iter();
    let mut parts = t(en).split("{}").peekable();
    while let Some(part) = parts.next() {
        out.push_str(part);
        if parts.peek().is_some() {
            out.push_str(args.next().copied().unwrap_or(""));
        }
    }
    out
}

fn table() -> &'static HashMap<&'static str, &'static str> {
    static TABLE: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    TABLE.get_or_init(|| ZH_HANS.iter().copied().collect())
}

/// English → Simplified Chinese.
const ZH_HANS: &[(&str, &str)] = &[
    // Navigation and screens
    ("New workspace", "新建工作区"),
    ("History", "历史"),
    ("Search", "搜索"),
    ("Schedules", "定时任务"),
    ("Settings", "设置"),
    ("Not connected to the service", "未连接到服务"),
    ("Start service", "启动服务"),
    ("Starting the service…", "正在启动服务…"),
    ("Couldn't start the service: {}", "无法启动服务：{}"),
    ("Cannot reach the Sophon service", "无法连接 Sophon 服务"),
    ("Downloading {}%", "正在下载 {}%"),
    ("Installing…", "正在安装…"),
    ("Update failed · Retry", "更新失败 · 重试"),
    ("Get {}", "获取 {}"),
    ("Update to {}", "更新到 {}"),
    ("New chat", "新对话"),
    ("Press ⌘Q again to quit Sophon", "再按一次 ⌘Q 退出 Sophon"),
    ("Settings…", "设置…"),
    ("Quit Sophon", "退出 Sophon"),
    ("File", "文件"),
    ("Close tab", "关闭标签页"),
    ("Edit", "编辑"),
    ("View", "显示"),
    ("Window", "窗口"),
    ("The working tree is clean", "工作区很干净"),
    ("Commits show here once there are some", "有提交后会显示在这里"),
    ("Not a Git repository", "不是 Git 仓库"),
    ("Changes and commits show for folders under Git; the files are still here", "这个文件夹没有用 Git 管理，看不到改动和提交，但文件仍可浏览"),
    ("Browse files", "浏览文件"),
    ("Couldn't read Git", "无法读取 Git"),
    ("New terminal  ⌃⇧`", "新终端  ⌃⇧`"),
    ("Hide terminal  ⌃`", "隐藏终端  ⌃`"),
    ("Toggle terminal", "显示/隐藏终端"),
    ("Changes since {}", "自 {} 以来的更新内容"),
    ("Restart to update", "重启以更新"),
    ("Later", "稍后"),
    ("Retry", "重试"),
    ("Download", "下载"),
    ("No notes for this release", "这个版本没有更新说明"),
    ("Update the app", "更新应用"),
    ("Usage", "用量"),
    ("Add project", "添加项目"),
    ("Projects", "项目"),
    ("Project", "项目"),
    ("Workspaces", "工作区"),
    ("Agents", "对话"),
    ("Hosts", "主机"),
    ("Host", "主机"),
    ("This Mac", "本机"),
    ("The Sophon service on this Mac", "本机的 Sophon 服务"),
    ("Add host", "添加主机"),
    ("Add host…", "添加主机…"),
    ("Address or pairing link", "地址或配对链接"),
    ("Pair a device", "配对设备"),
    ("Pair a device…", "配对设备…"),
    ("Pair…", "配对…"),
    ("Remove this host", "移除此主机"),
    ("Select a workspace", "选择一个工作区"),
    ("Pick one in the sidebar, or start a new workspace with an agent", "在侧边栏中选择，或新建一个工作区开始对话"),
    ("No projects yet", "还没有项目"),
    ("Back", "返回"),
    ("Toggle sidebar", "切换侧边栏"),
    ("Toggle sidebar  ⌘B", "切换侧边栏  ⌘B"),
    ("Toggle sidebar ⌘B", "切换侧边栏 ⌘B"),
    ("Toggle explorer", "切换资源面板"),
    ("Toggle terminal  ⌃`", "切换终端  ⌃`"),
    ("Command palette", "命令面板"),
    ("Search workspaces, agents and actions…", "搜索工作区、对话和操作…"),
    ("Search agents, workspaces, branches and projects", "搜索对话、工作区、分支和项目"),
    ("No results", "没有结果"),
    // Projects and workspaces
    ("Project actions", "项目操作"),
    ("Project settings", "项目设置"),
    ("New workspace in project", "在此项目中新建工作区"),
    ("Rename", "重命名"),
    ("Rename workspace", "重命名工作区"),
    ("Rename project", "重命名项目"),
    ("Rename agent", "重命名对话"),
    ("Remove from Sophon", "从 Sophon 中移除"),
    ("Archive", "归档"),
    ("Archive workspace", "归档工作区"),
    ("Archive agent", "归档对话"),
    ("Archived", "已归档"),
    ("Pin", "置顶"),
    ("Unpin", "取消置顶"),
    ("Mark as unread", "标为未读"),
    ("Labels", "标签"),
    ("Add label", "添加标签"),
    ("New label", "新建标签"),
    ("Label name", "标签名称"),
    ("No labels yet; create one below", "还没有标签，在下方新建一个"),
    ("Choose a project", "选择项目"),
    ("Choose a project folder", "选择项目文件夹"),
    ("Choose folder…", "选择文件夹…"),
    ("Add a project folder", "添加项目文件夹"),
    ("Existing folder", "已有文件夹"),
    ("Pick a folder on this computer; Sophon lists it with your projects", "选择这台电脑上的文件夹，Sophon 会把它列入你的项目"),
    ("Clone from GitHub", "从 GitHub 克隆"),
    ("Clone", "克隆"),
    ("owner/repository", "所有者/仓库"),
    ("Choose where it goes", "选择存放位置"),
    ("Folder name", "文件夹名称"),
    ("Local", "本地"),
    ("Worktree", "Worktree"),
    ("Import session", "导入会话"),
    ("Import a session", "导入会话"),
    ("Continue a Claude Code or Codex session you started outside Sophon", "继续你在 Sophon 之外开始的 Claude Code 或 Codex 会话"),
    ("Search sessions", "搜索会话"),
    ("No sessions yet", "还没有会话"),
    ("No sessions found", "没有找到会话"),
    ("What should the agent do?", "要 Agent 做什么？"),
    ("Creating workspace…", "正在创建工作区…"),
    ("New branch (optional)", "新分支（可选）"),
    ("Start from", "起点"),
    ("Default branch", "默认分支"),
    ("From default branch", "从默认分支"),
    ("Search branches", "搜索分支"),
    ("Create", "创建"),
    // Workspace
    ("New tab", "新标签页"),
    ("New agent", "新对话"),
    ("New terminal", "新终端"),
    ("Terminal", "终端"),
    ("Close terminal", "关闭终端"),
    ("Close file", "关闭文件"),
    ("Close diff", "关闭差异"),
    ("Close commit diff", "关闭提交差异"),
    ("Close sub-agent", "关闭子 Agent"),
    ("Sub-agent", "子 Agent"),
    ("Changes", "更改"),
    ("Claude models run only in Claude Code, on your own Claude account: not through another client's subscription", "Claude 模型只能在 Claude Code 里、用你自己的 Claude 账号运行，不能经其他客户端的订阅使用"),
    ("The service is not installed on this Mac", "这台 Mac 上没有安装服务"),
    ("The service on this Mac did not start: {}", "本机服务没能启动：{}"),
    ("Try again", "重试"),
    ("Through Magpie", "经 Magpie"),
    ("Still this agent's harness; the requests go to another provider", "仍用这个 agent 的 harness，请求转发给其他服务商"),
    ("Subscription", "订阅"),
    ("Search models", "搜索模型"),
    ("Starred", "收藏"),
    ("Star", "收藏"),
    ("Unstar", "取消收藏"),
    ("No starred models yet", "还没有收藏的模型"),
    ("No matching models", "没有匹配的模型"),
    ("{} is not available", "{} 不可用"),
    ("Not installed on this computer, or it can't start", "这台电脑上没有安装，或者无法启动"),
    ("Continues in {}: a new agent, with this conversation", "将在 {} 中继续：新建智能体，带上这段对话"),
    ("Too large to preview here", "文件太大，无法在这里预览"),
    ("Open in its app", "用默认应用打开"),
    ("Too large to open here: download it", "文件太大，请下载后打开"),
    ("Fetching “{}”…", "正在获取「{}」…"),
    ("Could not preview: {}", "无法预览：{}"),
    ("No workspace is open", "没有打开的工作区"),
    ("Remote host", "远程主机"),
    ("Rename this host…", "重命名此主机…"),
    ("Rename host", "重命名主机"),
    ("Continue in another agent", "在其他智能体中继续"),
    ("A new agent, with this conversation", "新建智能体，带上这段对话"),
    ("Download…", "下载…"),
    ("Downloading “{}”…", "正在下载「{}」…"),
    ("Saved “{}”", "已保存「{}」"),
    ("Saved “{}” to Files › Sophon", "已保存「{}」到「文件 › Sophon」"),
    ("Could not download “{}”: {}", "无法下载「{}」：{}"),
    ("This folder is empty", "此文件夹为空"),
    ("This file can't be shown here", "此文件无法在这里预览"),
    ("The rest of the file is in its download", "文件太长，只显示开头，下载可获取完整内容"),
    ("Files", "文件"),
    ("Commits", "提交"),
    ("All changes", "全部更改"),
    ("No changes", "没有更改"),
    ("No uncommitted changes", "没有未提交的更改"),
    ("No commits yet", "还没有提交"),
    ("Loading…", "加载中…"),
    ("Loading files…", "正在加载文件…"),
    ("Loading commits…", "正在加载提交…"),
    ("Loading changes…", "正在加载更改…"),
    ("Load more", "加载更多"),
    ("Show files", "显示文件"),
    ("Show changes", "显示更改"),
    ("Split", "分栏"),
    ("Unified", "单栏"),
    ("Send review", "发送评审"),
    ("Comment on these lines", "评论这些行"),
    ("Leave a comment for the agent", "给 Agent 留言"),
    ("Comment", "评论"),
    ("Delete comment", "删除评论"),
    ("Ask the agent about these changes, or + on a line to comment…", "就这些更改询问 Agent，或点行旁的 + 发表评论…"),
    ("Send 1 line comment, with a note…", "发送 1 条行评论，可附说明…"),
    ("New file", "新建文件"),
    ("New folder", "新建文件夹"),
    ("Delete", "删除"),
    ("Duplicate", "复制"),
    ("Save", "保存"),
    ("Saving…", "正在保存…"),
    ("Unsaved", "未保存"),
    ("Open", "打开"),
    ("Recover files", "恢复文件"),
    ("If its folder went missing", "如果它的文件夹丢失了"),
    // Git
    ("Git", "Git"),
    ("Commit", "提交"),
    ("Commit all changes", "提交全部更改"),
    ("Commit message (leave empty to generate one)", "提交信息（留空则自动生成）"),
    ("Push", "推送"),
    ("Pull", "拉取"),
    ("Switch branch", "切换分支"),
    ("Stash changes", "储藏更改"),
    ("Pop latest stash", "恢复最近的储藏"),
    ("Create pull request", "创建拉取请求"),
    ("Open pull request", "打开拉取请求"),
    ("Merge pull request", "合并拉取请求"),
    ("Squash and merge", "压缩并合并"),
    ("Merge into base", "合并到基础分支"),
    ("Merged", "已合并"),
    ("Closed", "已关闭"),
    ("Checks passed", "检查已通过"),
    ("Detached", "游离状态"),
    // Composer and timeline
    ("Message the agent…", "发消息…"),
    ("Attach", "附件"),
    ("Choose images…", "选择图片…"),
    ("Paste image", "粘贴图片"),
    ("Output", "输出"),
    ("Exit code {}", "退出码 {}"),
    ("Results", "结果"),
    ("Choose files…", "选择文件…"),
    ("Photo Library", "照片图库"),
    ("Choose Files", "选取文件"),
    ("Uploading…", "上传中…"),
    ("Remove file", "移除文件"),
    ("Wait for the files to finish uploading", "请等文件上传完"),
    ("“{}” is over 50 MB", "“{}” 超过 50 MB"),
    ("Could not upload “{}”: {}", "无法上传“{}”：{}"),
    ("Pasted image", "粘贴的图片"),
    ("Remove image", "移除图片"),
    ("No image on the clipboard", "剪贴板里没有图片"),
    ("Could not read the pasted image", "无法读取粘贴的图片"),
    ("Dictate", "听写"),
    ("Stop", "停止"),
    ("Mode", "模式"),
    ("Thinking", "思考"),
    ("Steer", "引导"),
    ("Interrupt", "打断"),
    ("Queue", "排队"),
    ("Queued", "已排队"),
    ("Remove from queue", "移出队列"),
    ("Sending", "发送"),
    ("Submit", "提交"),
    ("Your answer", "你的回答"),
    ("Allow", "允许"),
    ("Deny", "拒绝"),
    ("Plan ready for review", "计划已就绪，等待审阅"),
    ("No messages yet", "还没有消息"),
    ("Load earlier messages", "加载更早的消息"),
    ("Message actions", "消息操作"),
    ("Rewind to here", "回退到这里"),
    ("Fork into new agent", "分叉为新对话"),
    ("Conversation and files", "对话和文件"),
    ("Conversation only", "仅对话"),
    ("Files only", "仅文件"),
    ("Find in conversation", "在对话中查找"),
    ("Previous match", "上一个匹配"),
    ("Next match", "下一个匹配"),
    ("Close find", "关闭查找"),
    ("Tasks", "任务"),
    ("No details", "没有详情"),
    ("Your messages", "你的消息"),
    // Statuses
    ("Running", "运行中"),
    ("Running · unhealthy", "运行中 · 异常"),
    ("Idle", "空闲"),
    ("Ready", "就绪"),
    ("Done", "完成"),
    ("Completed", "已完成"),
    ("Failed", "失败"),
    ("Stopped", "已停止"),
    ("Needs input", "需要输入"),
    ("Paused", "已暂停"),
    ("Active", "活跃"),
    ("Disabled", "已停用"),
    ("Offline", "离线"),
    ("Unavailable", "不可用"),
    ("Not available", "不可用"),
    ("Starting…", "正在启动…"),
    ("Connecting…", "正在连接…"),
    ("Draft", "草稿"),
    // Schedules
    ("New schedule", "新建定时任务"),
    ("No schedules yet", "还没有定时任务"),
    ("What should the agent do each time?", "每次要 Agent 做什么？"),
    ("Cadence", "频率"),
    ("Hourly", "每小时"),
    ("Cron expression, e.g. 0 9 * * 1-5", "Cron 表达式，例如 0 9 * * 1-5"),
    ("Run now", "立即运行"),
    ("Pause", "暂停"),
    ("Resume", "恢复"),
    ("Delete schedule", "删除定时任务"),
    ("Other", "其他"),
    // Settings
    ("Language", "语言"),
    ("System", "跟随系统"),
    ("Appearance", "外观"),
    ("Theme", "主题"),
    ("Light, dark, or as the system is set", "浅色、深色，或跟随系统设置"),
    ("Light", "浅色"),
    ("Dark", "深色"),
    ("The language of Sophon's interface", "Sophon 界面的语言"),
    ("Host behavior", "主机行为"),
    ("While an agent is running", "Agent 运行期间"),
    ("What Return does with your message", "按回车时如何处理你的消息"),
    ("The Sophon service this app talks to", "此应用连接的 Sophon 服务"),
    ("Service version", "服务版本"),
    ("Update", "更新"),
    ("Update service", "更新服务"),
    ("Installs the latest service release; Sophon reconnects on its own", "安装最新版本的服务，Sophon 会自动重新连接"),
    ("Restart", "重启"),
    ("Restart service", "重启服务"),
    ("Restarts the service. Sophon reconnects on its own", "重启服务，Sophon 会自动重新连接"),
    ("Running agents stop and resume when the service is back, if resume on startup is on. Terminals keep running", "运行中的 Agent 会停止；若开启了「启动时恢复」，服务恢复后会继续。终端保持运行"),
    ("Resume on startup", "启动时恢复"),
    ("Pick up running agents when the service restarts", "服务重启后继续运行中的 Agent"),
    ("Free idle agents", "释放空闲 Agent"),
    ("Stop idle agent runtimes to save memory; they resume on demand", "停止空闲 Agent 的运行时以节省内存，需要时自动恢复"),
    ("Archive after merge", "合并后归档"),
    ("Archive a workspace once its branch is merged", "分支合并后自动归档工作区"),
    ("Inject Sophon tools", "注入 Sophon 工具"),
    ("Agents can open terminals, start agents and more through MCP", "Agent 可以通过 MCP 打开终端、启动其他 Agent 等"),
    ("Extra system prompt", "额外的系统提示词"),
    ("Appended to every agent's instructions", "追加到每个 Agent 的指令中"),
    ("e.g. Answer in Chinese. Run the tests before you finish", "例如：用中文回答。完成前先运行测试"),
    ("Save prompt", "保存提示词"),
    ("Remote relay", "远程中转"),
    ("Reach this host from your phone through the encrypted relay", "通过加密中转从手机访问这台主机"),
    ("Usage reports", "用量报告"),
    ("Collect token and cost usage per provider", "按服务商统计 token 和费用"),
    ("Providers", "服务商"),
    ("No usage reported by your providers", "服务商没有报告用量"),
    ("Setup and teardown scripts, services and other project settings, saved in the project", "初始化与清理脚本、服务和其他项目设置，保存在项目中"),
    // Pairing and hosts
    ("Scan with your iPhone's camera and open it in Sophon. On the same Wi-Fi it connects directly; anywhere else through the encrypted relay. Both are end to end encrypted", "用 iPhone 相机扫描并在 Sophon 中打开。同一 Wi-Fi 下直接连接，其他网络通过中转连接，均为端到端加密"),
    ("Anywhere · relay", "任何网络 · 中转"),
    ("Copy link", "复制链接"),
    ("Copy relay link", "复制中转链接"),
    ("Pairing link copied", "已复制配对链接"),
    ("Relay pairing link copied", "已复制中转配对链接"),
    ("Paste a pairing link (Settings → Pair a device on the other machine) to connect from anywhere through the encrypted relay, or a LAN / Tailscale address, or a tunnel URL", "粘贴配对链接（在另一台机器的 设置 → 配对设备 中获取）即可通过加密中转从任何地方连接；也可以填局域网 / Tailscale 地址或隧道 URL"),
    ("On that machine, let the service listen beyond localhost (daemon.listen, e.g. 0.0.0.0:6767) and set a password first", "先在那台机器上让服务监听本机以外的地址（daemon.listen，例如 0.0.0.0:6767）并设置密码"),
    ("Address (192.168.1.20:6767) or a pairing link", "地址（192.168.1.20:6767）或配对链接"),
    ("Name, e.g. Studio Mac", "名称，例如 Studio Mac"),
    ("Name (optional)", "名称（可选）"),
    ("Service password", "服务密码"),
    ("Paste", "粘贴"),
    ("Connect", "连接"),
    ("Neither the LAN bridge nor the relay is available: turn on Remote relay in Settings", "局域网桥接和中转都不可用，请在设置中开启远程中转"),
    ("The Sophon service isn't running on this Mac", "本机的 Sophon 服务未运行"),
    // Templates ({} in order)
    ("This {} file can't be shown as text", "这个 {} 文件无法以文本显示"),
    ("{} · not available", "{} · 不可用"),
    ("Dictation stopped: {}", "听写已停止：{}"),
    ("{} failing", "{} 项失败"),
    ("{} running", "{} 项运行中"),
    ("{} commits to push", "{} 个提交待推送"),
    ("{} commits behind", "落后 {} 个提交"),
    ("Into {}", "合并到 {}"),
    ("{} models", "{} 个模型"),
    ("Resets {}", "{} 重置"),
    ("Same Wi-Fi · {}", "同一 Wi-Fi · {}"),
    ("Tasks {}/{}", "任务 {}/{}"),
    ("Sub-agents · {}", "子 Agent · {}"),
    ("{} files", "{} 个文件"),
    ("Send {} line comments, with a note…", "发送 {} 条行评论，可附说明…"),
    ("Line {}", "第 {} 行"),
    ("Lines {}–{}", "第 {}–{} 行"),
    ("Continues from “{}” · runs in {}", "接续「{}」· 运行于 {}"),
    ("Runs in {}", "运行于 {}"),
    ("Scripts", "脚本"),
    ("click to stop", "点击停止"),
    ("click to start", "点击启动"),
    ("Open {}", "打开 {}"),
    ("A new worktree branched from {}", "从 {} 分出新的 worktree"),
    ("From {}", "从 {}"),
    ("Connecting to {}…", "正在连接 {}…"),
    ("{} offline", "{} 离线"),
    ("1 file", "1 个文件"),
    // Notices
    ("Start an agent in this workspace to send it a review", "先在此工作区开始一个对话，再发送评审"),
    ("Review sent to “{}”", "评审已发送给「{}」"),
    ("Paired", "已配对"),
    ("Updating the service…", "正在更新服务…"),
    ("Restarting the service…", "正在重启服务…"),
    ("Importing session…", "正在导入会话…"),
    ("“{}” is ready", "「{}」已就绪"),
    ("Could not list {}: {}", "无法列出 {}：{}"),
    ("{} failed: {}", "{} 失败：{}"),
    ("Pull request created: {}", "已创建拉取请求：{}"),
    ("{} done", "{} 完成"),
    ("Project settings saved", "项目设置已保存"),
    ("Rewind failed: {}", "回退失败：{}"),
    ("Rewound", "已回退"),
    ("Fork failed: {}", "分叉失败：{}"),
    ("Nothing to fork yet", "还没有可分叉的内容"),
    ("Commit all changes…", "提交全部更改…"),
    ("Switch branch…", "切换分支…"),
    ("New file…", "新建文件…"),
    ("New folder…", "新建文件夹…"),
    ("Rename…", "重命名…"),
    ("Labels…", "标签…"),
    ("Daily 9:00", "每天 9:00"),
    ("Weekdays 9:00", "工作日 9:00"),
    ("Background", "后台"),
    ("Outline", "大纲"),
    // Generic
    ("Cancel", "取消"),
    ("Dismiss", "关闭"),
    ("Undo", "撤销"),
    ("Actions", "操作"),
    ("Change…", "更改…"),
    ("Connect to your computer", "连接你的电脑"),
    ("Your agents run on your computer; Sophon on the phone follows and answers them.", "Agent 在你的电脑上运行；手机上的 Sophon 用来查看进度、回应它们。"),
    ("On the computer, open Sophon → Settings → Pair a device", "在电脑上打开 Sophon → 设置 → 配对设备"),
    ("Scan the QR code with the iPhone's camera, and open the link in Sophon", "用 iPhone 相机扫描二维码，在 Sophon 中打开链接"),
    ("Or paste the pairing link", "或粘贴配对链接"),
    ("Paired before", "之前配对过"),
    ("My computer", "我的电脑"),
    ("Close", "关闭"),
    ("Connected", "已连接"),
    ("Computers", "电脑"),
    ("Pair another computer", "配对另一台电脑"),
    ("Forget this computer", "忘记这台电脑"),
    ("Can't reach the computer. Make sure it's awake and Sophon is running there. ({})", "连不上电脑。请确认电脑没有休眠，并且 Sophon 正在运行。（{}）"),
    ("No workspaces yet. Start one on the computer.", "还没有工作区，请在电脑上新建。"),
    ("Agent actions", "对话操作"),
    ("Swipe right, or tap the list, to see your computer's workspaces", "向右滑动或点击列表，查看电脑上的工作区"),
    ("Computer", "电脑"),
    ("Pair it again from its QR code", "之后可以重新扫码配对"),
    ("Forget", "忘记"),
    ("Welcome to Sophon", "欢迎使用 Sophon"),
    ("Connect your computer to get started", "连接你的电脑即可开始"),
    ("Scan QR code", "扫描二维码"),
    ("Direct connection", "直接连接"),
    ("Paste pairing link", "粘贴配对链接"),
    ("That isn't a Sophon pairing code", "这不是 Sophon 的配对二维码"),
    ("This device has no camera", "这台设备没有相机"),
    ("Pairing…", "正在配对…"),
    ("Point the camera at the QR code in Sophon on your computer", "将相机对准电脑上 Sophon 显示的二维码"),
    ("Camera permission", "相机权限"),
    ("Allow camera access to scan the pairing QR code from your computer.", "允许访问相机，以扫描电脑上的配对二维码。"),
    ("Open Settings", "打开设置"),
    ("Can't reach the computer · retrying", "连不上电脑，正在重试"),
    ("Reconnecting…", "正在重新连接…"),
    ("Search conversations", "搜索对话"),
    ("Search projects", "搜索项目"),
    ("Chats", "对话"),
    ("New agent in", "新对话于"),
    ("No conversations yet", "还没有对话"),
    ("1 agent", "1 个对话"),
    ("{} agents", "{} 个对话"),
    ("now", "刚刚"),
    ("Attach and options", "附件与选项"),
    ("Photos", "照片"),
    ("Model · {}", "模型 · {}"),
    ("New conversation", "新建对话"),
    ("Search workspaces", "搜索工作区"),
    ("Relay server", "中转服务器"),
    ("Where phones reach this Mac away from home. Empty: the default relay. Applies after a service restart", "手机在外时通过它连这台 Mac。留空使用默认中转。重启服务后生效"),
    ("The service restarted", "服务已重启"),
    ("Thinking · {}", "思考 · {}"),
    ("Mode · {}", "模式 · {}"),
    ("Model", "模型"),
    ("Thinking…", "思考中…"),
    ("Working…", "工作中…"),
    ("The service comes with the desktop daemon app and updates with it", "服务随守护进程 App 一起安装，也随它一起更新"),
    ("Find a folder on the computer", "在电脑上查找文件夹"),
    ("Added {}", "已添加 {}"),
    ("Add “{}”", "添加「{}」"),
    ("Up", "上一级"),
    ("No folders here", "这里没有文件夹"),
    ("No matching folders", "没有匹配的文件夹"),
    ("Add a folder of the computer", "添加电脑上的文件夹"),
    ("Copy", "复制"),
    ("Rewind conversation to here", "对话回退到这里"),
    ("Rewind files to here", "文件回退到这里"),
    ("Rewind both to here", "对话和文件都回退到这里"),
    ("Agent settings", "Agent 设置"),
    ("Workspace", "工作区"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn templates_fill_in_order() {
        assert_eq!(tf("{} of {}", &["1", "2"]), "1 of 2");
    }

    #[test]
    fn keys_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for (en, _) in ZH_HANS {
            assert!(seen.insert(*en), "duplicate key {en}");
        }
    }
}
