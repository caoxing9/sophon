//! The app's state, one signal per concern so a streaming delta re-renders
//! the conversation and nothing else. Screens read through these functions
//! and change state only here: by applying daemon events or sending commands.
//!
//! Navigation: the sidebar lists projects and their
//! workspaces; a workspace shows its agents as tabs, plus a draft tab that
//! starts a new agent there. "New workspace" is its own screen.

use crate::i18n::{t, tf};
use std::{cell::RefCell, collections::HashMap};

use nsh_core::prelude::*;
use tokio::sync::mpsc::UnboundedSender;

use crate::daemon::{
    Agent, Command, Event, Item, Project, Provider, Workspace,
    connection::{CreateAgent, ImageInput, SlashCommand},
    model::{apply_live, carry_images},
};

#[derive(Clone, Debug, PartialEq, Default)]
pub(crate) enum Conn {
    #[default]
    Starting,
    Connecting(String),
    Connected { hostname: String, version: String, url: String },
    Down(String),
}

#[derive(Clone, Debug, PartialEq, Default)]
pub(crate) enum Screen {
    #[default]
    Home,
    NewWorkspace,
    Settings,
    History,
    Schedules,
    Usage,
    Workspace(String),
}

#[derive(Clone, Debug, PartialEq, Default)]
pub(crate) enum Tab {
    #[default]
    Draft,
    Agent(String),
    /// The workspace's uncommitted changes: one file, or all (`None`).
    Diff(Option<String>),
    File(String),
    /// A sub-agent of an agent, read-only: (parent, sub-agent).
    Subagent(String, String),
    /// One file of a commit: (sha, path).
    CommitDiff(String, String),
}

/// The workspace's file tree: listed directories and the expanded ones.
#[derive(Clone, Default)]
struct FileTree {
    cwd: String,
    dirs: HashMap<String, Vec<crate::daemon::connection::DirEntry>>,
    expanded: std::collections::HashSet<String>,
    /// Folders asked for and not listed yet.
    listing: std::collections::HashSet<String>,
}

/// A file open in a tab: what is on disk and what the editor holds.
#[derive(Clone, Debug, PartialEq, Default)]
pub(crate) struct OpenFile {
    pub(crate) loaded: bool,
    pub(crate) kind: String,
    pub(crate) disk: String,
    pub(crate) text: String,
    pub(crate) modified_at: String,
    pub(crate) revision: Option<String>,
    pub(crate) error: Option<String>,
    pub(crate) saving: bool,
}

impl OpenFile {
    pub(crate) fn dirty(&self) -> bool {
        self.loaded && self.text != self.disk
    }
}

#[derive(Clone, Default)]
struct Files(HashMap<String, OpenFile>);

/// One line of the file tree.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TreeRow {
    pub(crate) entry: crate::daemon::connection::DirEntry,
    pub(crate) depth: usize,
    pub(crate) expanded: bool,
}

const ROOT: &str = ".";

fn tree_cwd() -> Option<String> {
    current_workspace().map(|w| w.directory)
}

/// The tree, flattened in display order; asks for the root when unlisted.
pub(crate) fn tree_rows() -> Vec<TreeRow> {
    let Some(cwd) = tree_cwd() else { return vec![] };
    let same = global::<FileTree>().with(|t| t.cwd == cwd);
    if !same {
        untracked(|| global::<FileTree>().update(|t| *t = FileTree { cwd: cwd.clone(), ..Default::default() }));
        send(Command::ListDir { cwd: cwd.clone(), path: ROOT.into() });
    }
    global::<FileTree>().with(|t| {
        let mut rows = vec![];
        fn walk(t: &FileTree, dir: &str, depth: usize, rows: &mut Vec<TreeRow>) {
            let Some(entries) = t.dirs.get(dir) else { return };
            let mut sorted = entries.clone();
            sorted.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
            for e in sorted {
                let expanded = e.is_dir && t.expanded.contains(&e.path);
                let path = e.path.clone();
                rows.push(TreeRow { entry: e, depth, expanded });
                if expanded {
                    walk(t, &path, depth + 1, rows);
                }
            }
        }
        walk(t, ROOT, 0, &mut rows);
        rows
    })
}

#[derive(Clone, Default)]
struct CommitDiffState(Option<Result<crate::daemon::model::DiffFile, String>>);

pub(crate) fn commit_diff() -> Option<Result<crate::daemon::model::DiffFile, String>> {
    global::<CommitDiffState>().with(|c| c.0.clone())
}

#[derive(Clone, Default)]
struct UsageState(Option<Vec<crate::daemon::connection::ProviderUsage>>);

pub(crate) fn usage() -> Option<Vec<crate::daemon::connection::ProviderUsage>> {
    global::<UsageState>().with(|u| u.0.clone())
}

/// The open project settings: root, the JSON text being edited, revision,
/// and an error.
#[derive(Clone, Debug, PartialEq, Default)]
pub(crate) struct ProjectConfig {
    pub(crate) root: String,
    pub(crate) text: Option<String>,
    pub(crate) revision: serde_json::Value,
    pub(crate) error: Option<String>,
}

pub(crate) fn project_config() -> ProjectConfig {
    global::<ProjectConfig>().with(Clone::clone)
}

pub(crate) fn open_project_settings(root: &str, name: &str) {
    global::<ProjectConfig>().update(|c| *c = ProjectConfig { root: root.to_owned(), ..Default::default() });
    send(Command::ReadProjectConfig(root.to_owned()));
    open_modal(Modal::ProjectSettings { root: root.to_owned(), name: name.to_owned() });
}

pub(crate) fn edit_project_config(text: &str) {
    global::<ProjectConfig>().update(|c| {
        c.text = Some(text.to_owned());
        c.error = None;
    });
}

pub(crate) fn save_project_config() {
    let c = project_config();
    let Some(text) = c.text else { return };
    match serde_json::from_str::<serde_json::Value>(&text) {
        Ok(config) => send(Command::WriteProjectConfig { root: c.root, config, revision: c.revision }),
        Err(e) => global::<ProjectConfig>().update(|c| c.error = Some(format!("Not valid JSON: {e}"))),
    }
}

pub(crate) fn open_usage() {
    global::<Nav>().update(|n| n.screen = Screen::Usage);
    send(Command::WatchDiff(None));
    refresh_usage();
}

/// Asks the daemon for the plan limits and re-reads this computer's
/// session logs.
pub(crate) fn refresh_usage() {
    send(Command::Usage);
    refresh_local_usage();
}

/// The limit readings so far (`usage-history.json`), read on first use.
#[derive(Clone, Default)]
struct LimitHistory(Option<crate::usage::limits::History>);

pub(crate) fn limit_history() -> crate::usage::limits::History {
    let loaded = global::<LimitHistory>().with_untracked(|h| h.0.is_some());
    if !loaded {
        let h = crate::usage::limits::History::load();
        untracked(|| global::<LimitHistory>().set(LimitHistory(Some(h))));
    }
    global::<LimitHistory>().with(|h| h.0.clone().unwrap_or_default())
}

/// The key a window's readings are kept under.
pub(crate) fn limit_key(provider: &crate::daemon::connection::ProviderUsage, window: &crate::daemon::connection::UsageWindow) -> String {
    format!("{}/{}", provider.id, window.id)
}

/// Keeps a reading of each window the daemon reported, for the trend and
/// the forecast.
fn record_limits(list: &[crate::daemon::connection::ProviderUsage]) {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
    let mut history = untracked(limit_history);
    let mut changed = false;
    for p in list {
        for w in &p.windows {
            if let Some(pct) = w.used {
                let resets = w.resets_at.as_deref().and_then(crate::usage::parse::rfc3339);
                changed |= history.record(&limit_key(p, w), now, pct, resets);
            }
        }
    }
    if changed {
        history.save();
        global::<LimitHistory>().set(LimitHistory(Some(history)));
    }
}

/// This computer's session-log statistics: the last report, and whether a
/// read is under way.
#[derive(Clone, Default, PartialEq)]
pub(crate) struct LocalUsage {
    pub(crate) report: Option<crate::usage::Report>,
    pub(crate) loading: bool,
}

pub(crate) fn local_usage() -> LocalUsage {
    global::<LocalUsage>().with(Clone::clone)
}

/// Re-reads the session logs on a thread of its own (only new and changed
/// files are read), then shows the sums.
#[cfg(not(any(target_os = "ios", target_arch = "wasm32")))]
fn refresh_local_usage() {
    if global::<LocalUsage>().with_untracked(|u| u.loading) {
        return;
    }
    global::<LocalUsage>().update(|u| u.loading = true);
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    std::thread::spawn(move || {
        let _ = tx.send(crate::usage::scan::scan());
    });
    spawn_detached(async move {
        let report = rx.recv().await;
        global::<LocalUsage>().update(|u| {
            u.loading = false;
            if report.is_some() {
                u.report = report;
            }
        });
    });
}

/// A phone or a browser has no session logs: the host's Sophon reads its
/// own and sends the sums (through its bridge).
#[cfg(any(target_os = "ios", target_arch = "wasm32"))]
fn refresh_local_usage() {
    if global::<LocalUsage>().with_untracked(|u| u.loading) {
        return;
    }
    global::<LocalUsage>().update(|u| u.loading = true);
    send(Command::HostUsage);
}

/// Sends review notes on a diff to the workspace's newest agent.
pub(crate) fn send_review(path: Option<&str>, note: &str) -> bool {
    let Some(w) = current_workspace() else { return false };
    let Some(agent) = workspace_agents(&w.id).into_iter().last() else {
        notify(t("Start an agent in this workspace to send it a review").into());
        return false;
    };
    let what = path.map(|p| format!("`{p}`")).unwrap_or_else(|| "the uncommitted changes".into());
    let text = format!("Review of {what}:\n\n{}", note.trim());
    let behavior = agent.is_running().then_some("steer");
    send(Command::Send { agent_id: agent.id.clone(), text, behavior, images: vec![], attachments: vec![], message_id: uuid::Uuid::new_v4().to_string() });
    notify(tf("Review sent to “{}”", &[&agent.title]));
    true
}

pub(crate) fn file_op(op: &'static str, path: &str, name: &str) {
    let Some(cwd) = tree_cwd() else { return };
    if op == "delete" {
        close_file(path);
    }
    send(Command::FileOp { cwd, path: path.to_owned(), name: name.to_owned(), op });
}

pub(crate) fn toggle_dir(path: &str) {
    let Some(cwd) = tree_cwd() else { return };
    let (open, listed) = global::<FileTree>().with_untracked(|t| (t.expanded.contains(path), t.dirs.contains_key(path)));
    global::<FileTree>().update(|t| {
        if open {
            t.expanded.remove(path);
        } else {
            t.expanded.insert(path.to_owned());
        }
    });
    if !open && !listed {
        send(Command::ListDir { cwd, path: path.to_owned() });
    }
}

pub(crate) fn open_file(path: &str) {
    load_file(path);
    open_tab(Tab::File(path.to_owned()));
}

/// Reads the workspace's file `path` into [`file`] once, without a tab
/// (the phone shows it on a page of its own).
pub(crate) fn load_file(path: &str) {
    let Some(cwd) = tree_cwd() else { return };
    let known = global::<Files>().with_untracked(|f| f.0.contains_key(path));
    if !known {
        global::<Files>().update(|f| {
            f.0.insert(path.to_owned(), OpenFile::default());
        });
        send(Command::ReadFile { cwd, path: path.to_owned() });
    }
}

/// The open workspace's folder `dir` (`.`, its root): folders first, then
/// files, by name; `None` until listed, which this asks for.
pub(crate) fn dir_entries(dir: &str) -> Option<Vec<crate::daemon::connection::DirEntry>> {
    let cwd = tree_cwd()?;
    if global::<FileTree>().with_untracked(|t| t.cwd != cwd) {
        global::<FileTree>().update(|t| *t = FileTree { cwd: cwd.clone(), ..Default::default() });
    }
    let listed = global::<FileTree>().with(|t| t.dirs.get(dir).cloned());
    let Some(mut entries) = listed else {
        let asked = global::<FileTree>().with_untracked(|t| t.listing.contains(dir));
        if !asked {
            untracked(|| global::<FileTree>().update(|t| {
                t.listing.insert(dir.to_owned());
            }));
            send(Command::ListDir { cwd, path: dir.to_owned() });
        }
        return None;
    };
    entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Some(entries)
}

pub(crate) fn file(path: &str) -> Option<OpenFile> {
    global::<Files>().with(|f| f.0.get(path).cloned())
}

pub(crate) fn open_files() -> Vec<String> {
    let mut list: Vec<String> = global::<Files>().with(|f| f.0.keys().cloned().collect());
    list.sort();
    list
}

pub(crate) fn edit_file(path: &str, text: &str) {
    global::<Files>().update(|f| {
        if let Some(file) = f.0.get_mut(path) {
            file.text = text.to_owned();
        }
    });
}

pub(crate) fn save_file(path: &str) {
    let (Some(cwd), Some(f)) = (tree_cwd(), file(path)) else { return };
    if !f.dirty() || f.saving {
        return;
    }
    global::<Files>().update(|files| {
        if let Some(file) = files.0.get_mut(path) {
            file.saving = true;
        }
    });
    send(Command::WriteFile { cwd, path: path.to_owned(), content: f.text, modified_at: f.modified_at, revision: f.revision });
}

pub(crate) fn close_file(path: &str) {
    global::<Files>().update(|f| {
        f.0.remove(path);
    });
    if tab() == Tab::File(path.to_owned())
        && let Some(w) = current_workspace()
    {
        open_workspace(&w.id);
    }
}

/// The open workspace's git state.
#[derive(Clone, Debug, PartialEq, Default)]
pub(crate) struct Git {
    pub(crate) cwd: String,
    pub(crate) status: Option<crate::daemon::connection::GitStatus>,
    pub(crate) pr: Option<crate::daemon::connection::PullRequest>,
    /// The action running now.
    pub(crate) busy: Option<&'static str>,
}

pub(crate) fn git() -> Git {
    let cwd = current_workspace().map(|w| w.directory).unwrap_or_default();
    global::<Git>().with(|g| if g.cwd == cwd { g.clone() } else { Git { cwd, ..Default::default() } })
}

#[derive(Clone, Default)]
struct CommitsState(Option<(Vec<crate::daemon::connection::Commit>, Option<String>)>);
#[derive(Clone, Default)]
struct BranchesState(Vec<String>);

pub(crate) fn commits() -> Option<(Vec<crate::daemon::connection::Commit>, Option<String>)> {
    global::<CommitsState>().with(|c| c.0.clone())
}

pub(crate) fn load_commits() {
    if let Some(cwd) = current_workspace().map(|w| w.directory) {
        send(Command::Commits(cwd));
    }
}

pub(crate) fn branches() -> Vec<String> {
    global::<BranchesState>().with(|b| b.0.clone())
}

pub(crate) fn search_branches(query: &str) {
    let cwd = current_workspace().map(|w| w.directory).or_else(|| Some(form().path).filter(|p| !p.is_empty()));
    if let Some(cwd) = cwd {
        send(Command::Branches { cwd, query: query.to_owned() });
    }
}

pub(crate) fn git_action(action: crate::daemon::connection::GitAction) {
    let Some(cwd) = current_workspace().map(|w| w.directory) else { return };
    let label = action.label();
    global::<Git>().update(|g| g.busy = Some(label));
    send(Command::Git { cwd, action });
}

#[derive(Clone, Default)]
struct Terminals {
    cwd: String,
    list: Vec<crate::daemon::connection::TerminalInfo>,
}

/// Output waiting for a terminal view to draw it: a replay resets it.
#[derive(Default)]
struct TermBuffer {
    chunks: Vec<String>,
    reset: bool,
}

thread_local! {
    static TERM_OUT: RefCell<HashMap<String, TermBuffer>> = RefCell::new(HashMap::new());
}

/// Bumped when terminal output arrives (the views drain their buffers).
#[derive(Clone, Default)]
struct TermTick(u64);

pub(crate) fn terminals() -> Vec<crate::daemon::connection::TerminalInfo> {
    let cwd = current_workspace().map(|w| w.directory).unwrap_or_default();
    global::<Terminals>().with(|t| if t.cwd == cwd { t.list.clone() } else { vec![] })
}

pub(crate) fn term_tick() -> u64 {
    global::<TermTick>().with(|t| t.0)
}

/// The output `terminal_id` has waiting: (reset first, chunks).
pub(crate) fn drain_terminal(terminal_id: &str) -> (bool, Vec<String>) {
    TERM_OUT.with_borrow_mut(|out| {
        out.get_mut(terminal_id).map(|b| (std::mem::take(&mut b.reset), std::mem::take(&mut b.chunks))).unwrap_or_default()
    })
}

pub(crate) fn attach_terminal(id: &str) {
    TERM_OUT.with_borrow_mut(|out| {
        out.remove(id);
    });
    send(Command::AttachTerminal(id.to_owned()));
}

pub(crate) fn detach_terminal(id: &str) {
    send(Command::DetachTerminal(id.to_owned()));
}

pub(crate) fn terminal_input(id: &str, data: &str) {
    send(Command::TerminalInput { terminal_id: id.to_owned(), data: data.to_owned() });
}

pub(crate) fn terminal_resize(id: &str, rows: u16, cols: u16) {
    send(Command::TerminalResize { terminal_id: id.to_owned(), rows, cols });
}

pub(crate) fn new_terminal() {
    if let Some(w) = current_workspace() {
        send(Command::CreateTerminal { cwd: w.directory, workspace_id: w.id });
        set_terminal_panel(true);
    }
}

pub(crate) fn kill_terminal(id: &str) {
    global::<Terminals>().update(|t| t.list.retain(|x| x.id != id));
    send(Command::KillTerminal(id.to_owned()));
    // The last one closed folds the panel away, as VS Code's does.
    if terminals().is_empty() {
        set_terminal_panel(false);
    }
}

/// The desktop's terminal panel under the workspace (VS Code's): whether
/// it is open, its height, and the terminal it shows.
#[derive(Clone, PartialEq)]
struct TerminalPanel {
    open: bool,
    height: f32,
    active: Option<String>,
}

impl Default for TerminalPanel {
    fn default() -> Self {
        TerminalPanel { open: false, height: 280.0, active: None }
    }
}

pub(crate) fn terminal_panel_open() -> bool {
    global::<TerminalPanel>().with(|p| p.open)
}

pub(crate) fn terminal_panel_height() -> f32 {
    global::<TerminalPanel>().with(|p| p.height)
}

pub(crate) fn set_terminal_panel_height(h: f32) {
    global::<TerminalPanel>().update(|p| p.height = h.clamp(120.0, 900.0));
}

/// Keeps the height a drag ended on, for the next launch.
pub(crate) fn save_terminal_panel_height() {
    save_prefs();
}

/// The terminal the panel shows: the one picked last, else the first.
pub(crate) fn active_terminal() -> Option<String> {
    let list = terminals();
    let picked = global::<TerminalPanel>().with(|p| p.active.clone());
    picked.filter(|id| list.iter().any(|t| &t.id == id)).or_else(|| list.first().map(|t| t.id.clone()))
}

pub(crate) fn show_terminal(id: &str) {
    global::<TerminalPanel>().update(|p| {
        p.active = Some(id.to_owned());
        p.open = true;
    });
}

fn set_terminal_panel(open: bool) {
    if terminal_panel_open() != open {
        animate_panels();
        global::<TerminalPanel>().update(|p| p.open = open);
    }
}

/// ⌃` (and ⌘J): shows the panel, starting a terminal when there is none;
/// hides it when shown.
pub(crate) fn toggle_terminal_panel() {
    if current_workspace().is_none() {
        return;
    }
    if terminal_panel_open() {
        set_terminal_panel(false);
    } else {
        if terminals().is_empty() {
            new_terminal();
        }
        set_terminal_panel(true);
    }
}

/// The workspace's uncommitted changes, streamed while it is open.
#[derive(Clone, Debug, PartialEq, Default)]
pub(crate) struct Changes {
    pub(crate) cwd: String,
    pub(crate) files: Vec<crate::daemon::model::DiffFile>,
    pub(crate) error: Option<String>,
    pub(crate) too_large: bool,
    pub(crate) loaded: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(crate) enum ExplorerTab {
    #[default]
    Changes,
    Files,
    Commits,
}

#[derive(Clone, Debug, PartialEq)]
struct Explorer {
    open: bool,
    tab: ExplorerTab,
}

impl Default for Explorer {
    fn default() -> Self {
        Self { open: true, tab: ExplorerTab::Changes }
    }
}

pub(crate) fn changes() -> Changes {
    global::<Changes>().with(Clone::clone)
}

#[derive(Clone, Default)]
struct SidebarHidden(bool);

pub(crate) fn sidebar_hidden() -> bool {
    global::<SidebarHidden>().with(|s| s.0)
}

#[derive(Clone)]
struct SidebarWidth(f32);

impl Default for SidebarWidth {
    fn default() -> Self {
        Self(crate::ui::kit::SIDEBAR_WIDTH)
    }
}

pub(crate) const SIDEBAR_MIN: f32 = 220.0;
pub(crate) const SIDEBAR_MAX: f32 = 480.0;

/// The sidebar's width on wide windows, which its edge drags.
pub(crate) fn sidebar_width() -> f32 {
    global::<SidebarWidth>().with(|w| w.0)
}

pub(crate) fn set_sidebar_width(width: f32, save: bool) {
    global::<SidebarWidth>().set(SidebarWidth(width.clamp(SIDEBAR_MIN, SIDEBAR_MAX)));
    if save {
        save_prefs();
    }
}

#[derive(Clone)]
struct ExplorerWidth(f32);

impl Default for ExplorerWidth {
    fn default() -> Self {
        Self(300.0)
    }
}

pub(crate) const EXPLORER_MIN: f32 = 240.0;
pub(crate) const EXPLORER_MAX: f32 = 640.0;

/// The explorer's width, which its edge drags.
pub(crate) fn explorer_width() -> f32 {
    global::<ExplorerWidth>().with(|w| w.0)
}

pub(crate) fn set_explorer_width(width: f32, save: bool) {
    global::<ExplorerWidth>().set(ExplorerWidth(width.clamp(EXPLORER_MIN, EXPLORER_MAX)));
    if save {
        save_prefs();
    }
}

/// Panels sliding in and out: the frames that change ease over 280 ms.
pub(crate) fn animate_panels() {
    animate_layout(nsh_core::LayoutAnimation {
        create: None,
        update: Some(nsh_core::LayoutTransition::new(nsh_core::LayoutCurve::EaseInEaseOut, 280.0)),
        delete: None,
    });
}

/// A list growing or shrinking (a project folding, a project moved): the
/// rows that stay glide to their places, natively in the next commit. No
/// fade for rows that come or go: a list re-renders rows by position, so
/// a fade would flash rows that only changed what they show.
pub(crate) fn animate_rows() {
    animate_layout(nsh_core::LayoutAnimation {
        create: None,
        update: Some(nsh_core::LayoutTransition::new(nsh_core::LayoutCurve::EaseInEaseOut, 220.0)),
        delete: None,
    });
}

pub(crate) fn toggle_sidebar() {
    animate_panels();
    global::<SidebarHidden>().update(|s| s.0 = !s.0);
    save_prefs();
}

/// The open workspace's tabs, in strip order.
pub(crate) fn tab_list() -> Vec<Tab> {
    let Some(w) = current_workspace() else { return vec![] };
    let mut tabs: Vec<Tab> = workspace_agents(&w.id).into_iter().map(|a| Tab::Agent(a.id)).collect();
    tabs.extend(open_files().into_iter().map(Tab::File));
    tabs
}

pub(crate) fn save_current_file() {
    if let Tab::File(path) = tab() {
        save_file(&path);
    }
}

pub(crate) fn explorer_open() -> bool {
    global::<Explorer>().with(|e| e.open)
}

pub(crate) fn toggle_explorer() {
    animate_panels();
    global::<Explorer>().update(|e| e.open = !e.open);
    save_prefs();
}

pub(crate) fn explorer_tab() -> ExplorerTab {
    global::<Explorer>().with(|e| e.tab)
}

pub(crate) fn set_explorer_tab(tab: ExplorerTab) {
    // The pages slide to it (and the switch's thumb with them), whoever
    // asks: the switch, Browse files, the diff count over the composer.
    animate_layout(nsh_core::LayoutAnimation {
        create: None,
        update: Some(nsh_core::LayoutTransition::new(nsh_core::LayoutCurve::Spring { damping: 0.86 }, 320.0)),
        delete: None,
    });
    global::<Explorer>().update(|e| e.tab = tab);
    if tab == ExplorerTab::Commits {
        load_commits();
    }
}

#[derive(Clone, Default)]
struct ConnState(Conn, String);
#[derive(Clone, Default)]
struct Agents(Vec<Agent>);
#[derive(Clone, Default)]
struct Projects(Vec<Project>);
#[derive(Clone, Default)]
struct Workspaces(Vec<Workspace>);
#[derive(Clone, Default)]
struct Providers(Vec<Provider>);
#[derive(Clone, Default)]
struct Nav {
    screen: Screen,
    tab: Tab,
}
#[derive(Clone, Default)]
struct Timeline {
    agent_id: Option<String>,
    items: Vec<Item>,
    loaded: bool,
    /// The cursor of the page before `items`, while there is older history.
    older: Option<serde_json::Value>,
    loading_older: bool,
    /// The epoch and last seq `items` hold: live events at or before it
    /// are repeats.
    cursor: Option<(String, u64)>,
    /// `items` are this device's copy (`daemon::cache`), shown before the
    /// host has said what came since.
    syncing: bool,
    /// Tool-call groups the user opened (true) or closed (false), by the
    /// group's id; kept while the conversation is open.
    folds: HashMap<String, bool>,
}
/// Messages waiting for the running turn to end, per agent.
#[derive(Clone, Default)]
struct Queues(HashMap<String, Vec<String>>);
/// The agents whose queue waits: stopped with Escape, its messages stay
/// until they are sent on or taken back to be edited.
#[derive(Clone, Default)]
struct QueueHeld(std::collections::HashSet<String>);
#[derive(Clone, Default)]
struct Commands(HashMap<String, Vec<SlashCommand>>);
#[derive(Clone, Default)]
struct Suggestions {
    query: String,
    entries: Vec<(String, bool)>,
}

/// What Return does while the agent is running (the "Sending" setting).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(crate) enum SendBehavior {
    /// Adds the message to the running turn.
    #[default]
    Steer,
    /// Stops the turn and starts a new one with the message.
    Interrupt,
    /// Holds it until the turn ends.
    Queue,
}

/// The open dialog, if any.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Modal {
    RenameWorkspace { id: String, name: String },
    RenameAgent { id: String, name: String },
    RenameHost { id: String, name: String },
    /// A picture of a sent message, large.
    Picture { name: String, uri: String },
    /// Renames the first space on this Mac.
    RenameMySpace { name: String },
    /// Removes a space on this Mac (its service stops; its folder stays).
    RemoveSpace { id: String, name: String, folder: String },
    ArchiveWorkspace { id: String, name: String },
    Commit,
    Palette,
    NewSchedule,
    EditSchedule(crate::daemon::connection::Schedule),
    AddProject,
    RenameProject { id: String, name: String },
    SwitchBranch,
    ToolDetail { name: String, kind: String, detail: serde_json::Value, error: Option<String> },
    /// A newer version: what changed since this one, and a restart to it.
    Update,
    ImportSession,
    RestartDaemon,
    AddHost,
    /// Names a new space on this Mac.
    CreateSpace,
    PairDevice,
    /// A phone nearby asks to pair with this Mac, showing `code` too.
    ApprovePairing { id: u64, device: String, code: String },
    /// Pairing with a computer nearby ([`crate::lan::pairing`]).
    PairNearby,
    /// Every paired device must pair again.
    RevokeDevices,
    Labels { workspace: String },
    ProjectSettings { root: String, name: String },
    /// Names a new file or folder in `dir`, or renames `path`.
    FileName { op: &'static str, path: String, name: String },
    /// Another account of an agent CLI, as its own instance.
    AddAgentAccount,
    /// An agent instance on an API key.
    AddAgentKey,
    /// Removes an instance Sophon added.
    RemoveAgent { id: String, label: String },
}

#[derive(Clone, Default)]
struct ModalState(Option<Modal>);

pub(crate) fn modal() -> Option<Modal> {
    global::<ModalState>().with(|m| m.0.clone())
}

pub(crate) fn open_modal(m: Modal) {
    global::<ModalState>().update(|s| s.0 = Some(m));
}

pub(crate) fn close_modal() {
    let was = global::<ModalState>().with_untracked(|s| s.0.clone());
    global::<ModalState>().update(|s| s.0 = None);
    // Dismissed without an answer: the waiting side hears no at once.
    match was {
        Some(Modal::ApprovePairing { .. }) => crate::lan::answer_pairing(false, None),
        Some(Modal::PairNearby) => crate::lan::cancel_pairing(),
        _ => {}
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub(crate) struct Settings {
    pub(crate) send_behavior: SendBehavior,
    pub(crate) keep_awake: KeepAwake,
    /// This Mac runs agents of its own (its Paseo service). Off: it only
    /// connects to other computers (a partner's Mac with no service).
    pub(crate) local_service: LocalService,
    /// What the first space on this Mac is called, when renamed.
    pub(crate) my_space_name: Option<String>,
    /// The phone: push notifications from the computer it follows.
    pub(crate) push: PushWanted,
    /// The phone: notifications say only that something needs you.
    pub(crate) push_hide: bool,
    /// The phone's id at the computers it registers with, made once.
    pub(crate) push_device: String,
}

/// Whether the phone wants push notifications (on unless set off).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PushWanted(pub(crate) bool);

impl Default for PushWanted {
    fn default() -> Self {
        PushWanted(true)
    }
}

/// Whether this Mac runs a service of its own (on unless set off; a Mac
/// without Paseo starts off).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct LocalService(pub(crate) bool);

impl Default for LocalService {
    fn default() -> Self {
        LocalService(true)
    }
}

/// Whether this Mac runs agents of its own.
pub(crate) fn local_service() -> bool {
    !crate::ui::kit::MOBILE && settings().local_service.0
}

/// Turns this Mac's own service on or off: off, it goes to the first other
/// computer saved; on, back to its own.
pub(crate) fn set_local_service(on: bool) {
    edit_settings(|s| s.local_service = LocalService(on));
    if on {
        if current_host().is_some_and(|h| crate::spaces::home_of(&h.url).is_none()) {
            return;
        }
        start_bridge();
        switch_host(None);
        // The switch that did it stays on screen (a host switch goes home).
        global::<Nav>().update(|n| n.screen = Screen::Settings);
    } else if current_host().is_none_or(|h| crate::spaces::home_of(&h.url).is_some()) {
        let other = hosts().into_iter().find(|h| crate::spaces::home_of(&h.url).is_none()).map(|h| h.id);
        if other.is_some() {
            switch_host(other);
            global::<Nav>().update(|n| n.screen = Screen::Settings);
        }
    }
}

/// Whether this Mac stays awake while its agents run.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub(crate) enum KeepAwake {
    Off,
    /// Idle sleep held off; the display still sleeps.
    #[default]
    On,
    /// The display stays on too.
    Display,
}

/// Holds this Mac awake while an agent of its own runs, as the setting
/// says. A remote host's agents keep their own computer busy, not this one.
pub(crate) fn watch_keep_awake() {
    effect(|| {
        let mode = settings().keep_awake;
        let busy = on_this_mac() && global::<Agents>().with(|a| a.0.iter().any(|x| x.is_running()));
        #[cfg(target_os = "macos")]
        crate::awake::hold(match mode {
            KeepAwake::Off => None,
            _ if !busy => None,
            KeepAwake::On => Some(false),
            KeepAwake::Display => Some(true),
        });
        #[cfg(not(target_os = "macos"))]
        let _ = (mode, busy);
    });
}
/// What a notice's button does.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum NoticeAction {
    UndoArchive(String),
}

#[derive(Clone, Default)]
struct Notice(Option<(u64, String, Option<NoticeAction>)>);
/// Unsent composer text per agent.
#[derive(Clone, Default)]
struct Drafts(HashMap<String, String>);

/// What a new agent starts with: the new-workspace screen and a workspace's
/// draft tab share it.
#[derive(Clone, Debug, PartialEq, Default)]
pub(crate) struct Form {
    pub(crate) path: String,
    pub(crate) worktree: bool,
    pub(crate) base_branch: Option<String>,
    pub(crate) branch_name: String,
    pub(crate) provider: String,
    pub(crate) model: Option<String>,
    pub(crate) mode: Option<String>,
    pub(crate) thinking: Option<String>,
    pub(crate) prompt: String,
    pub(crate) busy: bool,
    pub(crate) error: Option<String>,
    /// A fork's chat history, carried into the new agent; and from whom.
    pub(crate) context: Option<(serde_json::Value, String)>,
    /// The provider's features for a new agent, with the values chosen.
    pub(crate) features: Vec<crate::daemon::model::Feature>,
}

thread_local! {
    static COMMANDS: RefCell<Option<UnboundedSender<Command>>> = const { RefCell::new(None) };
}

pub(crate) fn install_sender(tx: UnboundedSender<Command>) {
    COMMANDS.with_borrow_mut(|c| *c = Some(tx));
}

fn send(cmd: Command) {
    COMMANDS.with_borrow(|c| {
        if let Some(tx) = c {
            let _ = tx.send(cmd);
        }
    });
}

// ------------------------------------------------------------------ reads

pub(crate) fn conn() -> Conn {
    global::<ConnState>().with(|c| c.0.clone())
}

pub(crate) fn screen() -> Screen {
    global::<Nav>().with(|n| n.screen.clone())
}

pub(crate) fn tab() -> Tab {
    global::<Nav>().with(|n| n.tab.clone())
}

pub(crate) fn projects() -> Vec<Project> {
    global::<Projects>().with(|p| p.0.clone())
}

pub(crate) fn providers() -> Vec<Provider> {
    global::<Providers>().with(|p| p.0.clone())
}

pub(crate) fn provider(id: &str) -> Option<Provider> {
    global::<Providers>().with(|p| p.0.iter().find(|p| p.id == id).cloned())
}

pub(crate) fn workspace(id: &str) -> Option<Workspace> {
    global::<Workspaces>().with(|w| w.0.iter().find(|w| w.id == id).cloned())
}

pub(crate) fn current_workspace() -> Option<Workspace> {
    match screen() {
        Screen::Workspace(id) => workspace(&id),
        _ => None,
    }
}

/// Every agent this host has, archived ones included.
pub(crate) fn agents() -> Vec<Agent> {
    global::<Agents>().with(|a| a.0.clone())
}

/// The workspaces the lists show: not those being archived, and not a
/// schedule's runs ([`is_a_run`]).
pub(crate) fn workspaces() -> Vec<Workspace> {
    let runs = runs_hidden();
    global::<Workspaces>().with(|w| w.0.iter().filter(|w| !w.archiving && !runs.contains(&w.id)).cloned().collect())
}

/// The run workspaces the user opened from a schedule's runs: listed like
/// any other from then on. Kept with the preferences.
#[derive(Clone, Default)]
struct KeptRuns(Vec<String>);

/// The workspace to open once the host lists it again (a run's, being
/// restored from the archive), and its agent.
#[derive(Clone, Default)]
struct OpenWhenListed(Option<(String, String)>);

/// Counts the restored conversations opened ([`open_when_listed`]): the
/// phone, whose screens are a stack of its own, shows the conversation then.
#[derive(Clone, Copy, Default)]
struct RestoredOpens(u32);

pub(crate) fn restored_opens() -> u32 {
    global::<RestoredOpens>().with(|r| r.0)
}

/// Whether `agent` is a schedule's run the lists leave out: started by a
/// schedule, in a workspace the user did not open from the schedule's runs.
pub(crate) fn is_hidden_run(agent: &Agent) -> bool {
    agent.scheduled
        && global::<KeptRuns>().with(|k| !agent.workspace_id.as_ref().is_some_and(|ws| k.0.contains(ws)))
}

/// The workspaces that are a schedule's runs and stay out of the lists: all
/// of a workspace's agents were started by a schedule, and the user did not
/// open it. The service makes one per run and archives it when the run
/// ends, so listed they would come and go on their own, every minute for a
/// schedule that runs so. They are under the schedule's runs.
fn runs_hidden() -> Vec<String> {
    let kept = global::<KeptRuns>().with(|k| k.0.clone());
    let open = match screen() {
        Screen::Workspace(id) => Some(id),
        _ => None,
    };
    global::<Agents>().with(|a| {
        let mut of_runs: Vec<&str> = Vec::new();
        let mut of_others: Vec<&str> = Vec::new();
        for agent in a.0.iter().filter(|a| a.parent_id.is_none()) {
            let Some(ws) = agent.workspace_id.as_deref() else { continue };
            if agent.scheduled { of_runs.push(ws) } else { of_others.push(ws) }
        }
        of_runs
            .into_iter()
            .filter(|ws| !of_others.contains(ws) && !kept.iter().any(|k| k == ws) && open.as_deref() != Some(*ws))
            .map(str::to_owned)
            .collect()
    })
}

pub(crate) fn agent(id: &str) -> Option<Agent> {
    global::<Agents>().with(|a| a.0.iter().find(|a| a.id == id).cloned())
}

/// The agent whose tab is open.
pub(crate) fn selected_agent() -> Option<Agent> {
    match (screen(), tab()) {
        (Screen::Workspace(_), Tab::Agent(id)) => agent(&id),
        _ => None,
    }
}

pub(crate) fn selected_id() -> Option<String> {
    selected_agent().map(|a| a.id)
}

/// A workspace's agents, oldest first: its tabs.
pub(crate) fn workspace_agents(workspace_id: &str) -> Vec<Agent> {
    let mut list: Vec<Agent> = global::<Agents>()
        .with(|a| a.0.iter().filter(|a| a.workspace_id.as_deref() == Some(workspace_id)).cloned().collect());
    list.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    list
}

/// The workspace and agent open last, opened again at launch (desktop).
#[derive(Clone, Default)]
struct LastOpen {
    workspace: Option<String>,
    agent: Option<String>,
    /// Restored already (or the user moved on first).
    done: bool,
}

/// The sidebar's project order, as the user dragged it (project ids).
#[derive(Clone, Default)]
struct ProjectOrder(Vec<String>);

/// Projects whose workspaces the sidebar folds away.
#[derive(Clone, Default)]
struct Collapsed(Vec<String>);

/// Models starred in the model picker, as `provider:model`.
#[derive(Clone, Default)]
struct FavoriteModels(Vec<String>);

pub(crate) fn favorite_models() -> Vec<String> {
    global::<FavoriteModels>().with(|f| f.0.clone())
}

pub(crate) fn toggle_favorite_model(key: &str) {
    global::<FavoriteModels>().update(|f| {
        if let Some(i) = f.0.iter().position(|k| k == key) {
            f.0.remove(i);
        } else {
            f.0.push(key.to_owned());
        }
    });
    save_prefs();
}

pub(crate) fn is_collapsed(project_id: &str) -> bool {
    global::<Collapsed>().with(|c| c.0.iter().any(|id| id == project_id))
}

pub(crate) fn toggle_collapsed(project_id: &str) {
    // The groups reveal on their own (nsh/base's Collapsible, a clipped
    // height on a native spring), which a layout animation would fight.
    global::<Collapsed>().update(|c| {
        if let Some(i) = c.0.iter().position(|id| id == project_id) {
            c.0.remove(i);
        } else {
            c.0.push(project_id.to_owned());
        }
    });
    save_prefs();
}

/// Where each project's header sits in the sidebar's list, for drops.
#[derive(Clone, Default)]
struct ProjectTops(std::collections::HashMap<String, f32>);

pub(crate) fn set_project_row_top(project_id: &str, top: f32) {
    global::<ProjectTops>().update(|t| {
        t.0.insert(project_id.to_owned(), top);
    });
}

/// A project header let go `dy` points from where it was: it goes before
/// the first header below its new place.
pub(crate) fn drop_project(project_id: &str, dy: f32) {
    if dy.abs() < 4.0 {
        return;
    }
    let tops = global::<ProjectTops>().with_untracked(|t| t.0.clone());
    let Some(start) = tops.get(project_id).copied() else { return };
    let landed = start + dy;
    let others: Vec<String> = rows()
        .into_iter()
        .filter_map(|r| match r {
            Row::Project(p) if p.id != project_id => Some(p.id),
            _ => None,
        })
        .collect();
    let index = others.iter().filter(|id| tops.get(*id).is_some_and(|t| *t < landed)).count();
    move_project(project_id, index);
}

/// Moves a project to `index` among the projects, as dropped there.
pub(crate) fn move_project(project_id: &str, index: usize) {
    let mut ids: Vec<String> = rows()
        .into_iter()
        .filter_map(|r| match r {
            Row::Project(p) => Some(p.id),
            _ => None,
        })
        .collect();
    let Some(from) = ids.iter().position(|id| id == project_id) else { return };
    let id = ids.remove(from);
    ids.insert(index.min(ids.len()), id);
    animate_rows();
    global::<ProjectOrder>().set(ProjectOrder(ids));
    save_prefs();
}

/// One line of the sidebar.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Row {
    Section(String),
    Project(Project),
    Workspace(Workspace),
}

/// The sidebar: pinned workspaces, then projects (most recent activity
/// first) each with its workspaces (most recent first).
/// The sidebar's groups: the pinned workspaces (`project: None`), then
/// each project with its own, folded or not.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Group {
    pub(crate) project: Option<Project>,
    pub(crate) workspaces: Vec<Workspace>,
}

impl Group {
    pub(crate) fn key(&self) -> String {
        self.project.as_ref().map_or_else(|| "pinned".into(), |p| p.id.clone())
    }
}

pub(crate) fn groups() -> Vec<Group> {
    let projects = projects();
    let workspaces = workspaces();

    let mut project_list = projects;
    for w in &workspaces {
        if !project_list.iter().any(|p| p.id == w.project_id) {
            project_list.push(Project {
                id: w.project_id.clone(),
                name: if w.project_name.is_empty() {
                    crate::daemon::model::folder_name(&w.project_root)
                } else {
                    w.project_name.clone()
                },
                root_path: w.project_root.clone(),
                kind: String::new(),
            });
        }
    }
    let activity = |p: &Project| {
        workspaces.iter().filter(|w| w.project_id == p.id).map(|w| w.activity_at.clone()).max().unwrap_or_default()
    };
    project_list.sort_by(|a, b| activity(b).cmp(&activity(a)).then_with(|| a.name.cmp(&b.name)));
    // The user's order, from dragging: projects it does not have yet (new
    // ones) come first, by activity, then the ordered ones.
    let order = global::<ProjectOrder>().with(|o| o.0.clone());
    project_list.sort_by_key(|p| order.iter().position(|id| *id == p.id).map_or((0, 0), |i| (1, i)));

    let mut groups = Vec::new();
    let pinned: Vec<Workspace> = workspaces.iter().filter(|w| w.pinned).cloned().collect();
    if !pinned.is_empty() {
        groups.push(Group { project: None, workspaces: pinned });
    }
    for p in project_list {
        let mut ws: Vec<Workspace> = workspaces.iter().filter(|w| w.project_id == p.id && !w.pinned).cloned().collect();
        ws.sort_by(|a, b| b.activity_at.cmp(&a.activity_at));
        groups.push(Group { project: Some(p), workspaces: ws });
    }
    groups
}

/// The groups as one list of rows, folded projects without theirs.
pub(crate) fn rows() -> Vec<Row> {
    let collapsed = global::<Collapsed>().with(|c| c.0.clone());
    let mut rows = Vec::new();
    for g in groups() {
        let folded = g.project.as_ref().is_some_and(|p| collapsed.contains(&p.id));
        match g.project {
            None => rows.push(Row::Section("Pinned".into())),
            Some(p) => rows.push(Row::Project(p)),
        }
        if !folded {
            rows.extend(g.workspaces.into_iter().map(Row::Workspace));
        }
    }
    rows
}

pub(crate) fn item_count() -> usize {
    global::<Timeline>().with(|t| t.items.len())
}

pub(crate) fn item_at(index: usize) -> Option<Item> {
    global::<Timeline>().with(|t| t.items.get(index).cloned())
}

/// The conversation's rows as the list shows them: runs of tool calls
/// folded into one summary row each.
/// The list reads it through one memo, so it is worked out once per change
/// rather than once per row.
pub(crate) fn display_rows() -> Vec<crate::fold::Row> {
    global::<Timeline>().with(|t| {
        // The conversation's own agent, read without cloning it: this runs
        // on every streamed word.
        let running = t.agent_id.as_deref().is_some_and(|id| {
            global::<Agents>().with(|a| a.0.iter().any(|x| x.id == id && x.is_running()))
        });
        crate::fold::fold(&t.items, running, &t.folds)
    })
}

/// The list's keys for `rows`: the agent's id with each row's own.
pub(crate) fn row_keys(rows: &[crate::fold::Row]) -> Vec<String> {
    let agent = global::<Timeline>().with(|t| t.agent_id.clone().unwrap_or_default());
    rows.iter().map(|r| format!("{agent}:{}", r.key())).collect()
}

/// What a group's calls did, for its summary row.
pub(crate) fn group_summary(start: usize, end: usize) -> crate::fold::Summary {
    global::<Timeline>().with(|t| crate::fold::summarize(t.items.get(start..end).unwrap_or_default()))
}

/// Opens or folds a group of tool calls.
pub(crate) fn set_group_open(id: &str, open: bool) {
    global::<Timeline>().update(|t| {
        t.folds.insert(id.to_owned(), open);
    });
    if open {
        // At the end of the conversation the list keeps its end in place,
        // so the calls push the summary up out of view: bring it back near
        // the top once they are laid out, where it folds them again.
        let id = id.to_owned();
        set_timeout(std::time::Duration::from_millis(60), move || {
            let row = untracked(display_rows).iter().position(|r| matches!(r, crate::fold::Row::Group(g) if g.id == id));
            if let Some(row) = row {
                TIMELINE_LIST.with_borrow(|l| {
                    if let Some(list) = l {
                        list.scroll_to_index(row as u32, nsh_core::geometry::Percentage(0.1), px(0.0), true);
                    }
                });
            }
        });
    }
}

/// Asks the host's Sophon for the pictures of the user messages among
/// `items` that name one and have none here (sent from another device).
/// Each once per conversation opening; a host without them says nothing.
fn ask_pictures(items: &[Item], agent_id: Option<&str>) {
    // The conversation's folder: where the host reads the agent's record.
    let cwd = agent_id.and_then(agent).map(|a| a.cwd).unwrap_or_default();
    for item in items {
        let Item::User { text, message_id, images } = item else { continue };
        if images.is_empty() && crate::sent_pictures::names_a_picture(text) {
            send(Command::AskPictures { message_id: message_id.clone().unwrap_or_default(), text: text.clone(), cwd: cwd.clone() });
        }
    }
}

pub(crate) fn timeline_loaded() -> bool {
    global::<Timeline>().with(|t| t.loaded)
}

/// Whether the open conversation shows this device's copy still, the host
/// not having said yet what came since.
pub(crate) fn timeline_syncing() -> bool {
    global::<Timeline>().with(|t| t.syncing)
}

pub(crate) fn loading_older() -> bool {
    global::<Timeline>().with(|t| t.loading_older)
}

pub(crate) fn load_older() {
    let (agent_id, cursor) = global::<Timeline>().with_untracked(|t| (t.agent_id.clone(), t.older.clone()));
    let (Some(agent_id), Some(cursor)) = (agent_id, cursor) else { return };
    if loading_older() {
        return;
    }
    global::<Timeline>().update(|t| t.loading_older = true);
    send(Command::LoadOlder { agent_id, cursor });
}

/// Images waiting to go with the next message, per composer (an agent's
/// id, or "" for the new-agent form). Each is a chip in the composer's
/// text, which names it by `id`; the bytes stay here, out of the document.
#[derive(Clone, Default)]
struct PendingImages(HashMap<String, Vec<PendingImage>>);

#[derive(Clone, Debug, PartialEq)]
struct PendingImage {
    id: String,
    image: ImageInput,
    /// Its chip was deleted. Kept (not sent) rather than dropped, so an
    /// undo that brings the chip back finds the image again.
    detached: bool,
}

fn image_key() -> String {
    match (screen(), tab()) {
        (Screen::Workspace(_), Tab::Agent(id)) => id,
        _ => String::new(),
    }
}

pub(crate) fn pending_images() -> Vec<ImageInput> {
    let key = image_key();
    global::<PendingImages>()
        .with(|p| p.0.get(&key).map(|l| l.iter().filter(|i| !i.detached).map(|i| i.image.clone()).collect()).unwrap_or_default())
}

/// Adds an image to this composer's message: the id its chip names it by.
pub(crate) fn add_image(image: ImageInput) -> String {
    let id = uuid::Uuid::new_v4().to_string();
    let key = image_key();
    global::<PendingImages>()
        .update(|p| p.0.entry(key).or_default().push(PendingImage { id: id.clone(), image, detached: false }));
    id
}

/// The image a chip names, in whichever composer it waits.
pub(crate) fn pending_image(id: &str) -> Option<ImageInput> {
    global::<PendingImages>().with(|p| p.0.values().flatten().find(|i| i.id == id).map(|i| i.image.clone()))
}

/// A file going with the next message: uploading to the host until
/// `attachment` (what names it in the message) arrives.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PendingFile {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) mime: String,
    pub(crate) size: usize,
    attachment: Option<serde_json::Value>,
    /// Its chip was deleted (see [`PendingImage::detached`]).
    detached: bool,
}

impl PendingFile {
    pub(crate) fn uploading(&self) -> bool {
        self.attachment.is_none()
    }
}

#[derive(Clone, Default)]
struct PendingFiles(HashMap<String, Vec<PendingFile>>);

/// The largest file the host takes.
const FILE_MAX_BYTES: usize = 50 * 1024 * 1024;

pub(crate) fn pending_files() -> Vec<PendingFile> {
    let key = image_key();
    global::<PendingFiles>().with(|p| p.0.get(&key).map(|l| l.iter().filter(|f| !f.detached).cloned().collect()).unwrap_or_default())
}

/// The file a chip names, in whichever composer it waits.
pub(crate) fn pending_file(id: &str) -> Option<PendingFile> {
    global::<PendingFiles>().with(|p| p.0.values().flatten().find(|f| f.id == id).cloned())
}

/// Starts uploading `bytes` to the host; the file's chip shows it
/// meanwhile and it goes with the message once it is there. The id its
/// chip names it by; none when it is too large.
pub(crate) fn attach_file(name: String, mime: String, bytes: Vec<u8>) -> Option<String> {
    if bytes.len() > FILE_MAX_BYTES {
        notify(tf("“{}” is over 50 MB", &[&name]));
        return None;
    }
    let id = uuid::Uuid::new_v4().to_string();
    let key = image_key();
    let file = PendingFile { id: id.clone(), name: name.clone(), mime: mime.clone(), size: bytes.len(), attachment: None, detached: false };
    global::<PendingFiles>().update(|p| p.0.entry(key).or_default().push(file));
    send(Command::Upload { id: id.clone(), name, mime, bytes });
    Some(id)
}

/// This composer's attachments still to send (not detached), images then
/// files.
pub(crate) fn pending_attachment_ids() -> Vec<String> {
    let key = image_key();
    let mut ids: Vec<String> = global::<PendingImages>()
        .with_untracked(|p| p.0.get(&key).map(|l| l.iter().filter(|i| !i.detached).map(|i| i.id.clone()).collect()).unwrap_or_default());
    global::<PendingFiles>()
        .with_untracked(|p| ids.extend(p.0.get(&key).into_iter().flatten().filter(|f| !f.detached).map(|f| f.id.clone())));
    ids
}

/// Every image and file id that waits in a composer, sent or not yet
/// (tracked: the composer drops chips whose attachment went away, as one
/// that failed to upload or was just sent).
pub(crate) fn known_attachments() -> std::collections::HashSet<String> {
    let mut ids: std::collections::HashSet<String> =
        global::<PendingImages>().with(|p| p.0.values().flatten().map(|i| i.id.clone()).collect());
    global::<PendingFiles>().with(|p| ids.extend(p.0.values().flatten().map(|f| f.id.clone())));
    ids
}

/// Makes this composer's attachments those its chips name (`ids`, in the
/// text's order): one whose chip is gone is detached and not sent, one
/// whose chip came back (undo) attached again, and the message carries
/// them in the chips' order. Writes only when something changed, as it
/// runs on every edit.
pub(crate) fn sync_attachments(ids: &[String]) {
    let key = image_key();
    let images = global::<PendingImages>()
        .with_untracked(|p| p.0.get(&key).map(|l| l.iter().map(|i| (i.id.clone(), i.detached)).collect::<Vec<_>>()));
    if let Some(order) = images.and_then(|now| arrangement(&now, ids)) {
        global::<PendingImages>().update(|p| {
            if let Some(list) = p.0.get_mut(&key) {
                let mut old: Vec<Option<PendingImage>> = std::mem::take(list).into_iter().map(Some).collect();
                for (i, detached) in order {
                    if let Some(mut item) = old.get_mut(i).and_then(Option::take) {
                        item.detached = detached;
                        list.push(item);
                    }
                }
            }
        });
    }
    let files = global::<PendingFiles>()
        .with_untracked(|p| p.0.get(&key).map(|l| l.iter().map(|f| (f.id.clone(), f.detached)).collect::<Vec<_>>()));
    if let Some(order) = files.and_then(|now| arrangement(&now, ids)) {
        global::<PendingFiles>().update(|p| {
            if let Some(list) = p.0.get_mut(&key) {
                let mut old: Vec<Option<PendingFile>> = std::mem::take(list).into_iter().map(Some).collect();
                for (i, detached) in order {
                    if let Some(mut item) = old.get_mut(i).and_then(Option::take) {
                        item.detached = detached;
                        list.push(item);
                    }
                }
            }
        });
    }
}

/// How a list of `(id, detached)` should be arranged for chips `ids`: each
/// item's index and whether it is detached, the chips' first in their
/// order, then the detached in theirs; `None` when it already is.
fn arrangement(now: &[(String, bool)], ids: &[String]) -> Option<Vec<(usize, bool)>> {
    let rank = |id: &String| ids.iter().position(|x| x == id);
    let mut order: Vec<usize> = (0..now.len()).collect();
    order.sort_by_key(|&i| match rank(&now[i].0) {
        Some(r) => (0, r),
        None => (1, i),
    });
    let next: Vec<(usize, bool)> = order.into_iter().map(|i| (i, rank(&now[i].0).is_none())).collect();
    let same = next.iter().enumerate().all(|(n, &(i, detached))| n == i && now[i].1 == detached);
    (!same).then_some(next)
}

/// Files on their way here, by request id: their path in the workspace.
#[derive(Clone, Default)]
struct Downloads(HashMap<String, (String, bool)>);

/// Saves the workspace's file `path` on this device, whatever its size or
/// kind: on the Mac where the save panel says, on the phone in Sophon's
/// folder of the Files app.
pub(crate) fn download_file(path: &str) {
    let Some(cwd) = tree_cwd() else { return };
    if downloading(path) {
        return;
    }
    let id = uuid::Uuid::new_v4().to_string();
    global::<Downloads>().update(|d| {
        d.0.insert(id.clone(), (path.to_owned(), false));
    });
    notify(tf("Downloading “{}”…", &[file_name(path)]));
    send(Command::Download { id, cwd, path: path.to_owned() });
}

/// Whether `path` is on its way here.
pub(crate) fn downloading(path: &str) -> bool {
    global::<Downloads>().with(|d| d.0.values().any(|(p, preview)| p == path && !preview))
}

/// A file's bytes for its preview (a PDF, an image, a video): on their
/// way, here, or why not.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Preview {
    Loading,
    Ready(std::rc::Rc<Vec<u8>>),
    Failed(String),
    TooLarge,
}

#[derive(Clone, Default)]
struct Previews(HashMap<String, Preview>);

/// The largest file previewed; a larger one downloads.
pub(crate) const PREVIEW_MAX_BYTES: u64 = 100 * 1024 * 1024;

/// The open workspace's file `path` for its preview, fetched whole (the
/// read a text file takes stops at 2 MB) the first time it is asked for.
pub(crate) fn preview(path: &str) -> Preview {
    if let Some(p) = global::<Previews>().with(|p| p.0.get(path).cloned()) {
        return p;
    }
    let state = match (tree_cwd(), entry_size(path)) {
        (None, _) => Preview::Failed(t("No workspace is open").into()),
        (_, Some(size)) if size > PREVIEW_MAX_BYTES => Preview::TooLarge,
        (Some(cwd), _) => {
            let id = uuid::Uuid::new_v4().to_string();
            untracked(|| global::<Downloads>().update(|d| {
                d.0.insert(id.clone(), (path.to_owned(), true));
            }));
            send(Command::Download { id, cwd, path: path.to_owned() });
            Preview::Loading
        }
    };
    untracked(|| global::<Previews>().update(|p| {
        p.0.insert(path.to_owned(), state.clone());
    }));
    state
}

/// The size of the workspace's file `path` as its folder's listing gave it.
fn entry_size(path: &str) -> Option<u64> {
    let dir = path.rsplit_once('/').map(|(d, _)| d).unwrap_or(ROOT);
    global::<FileTree>().with_untracked(|t| {
        t.dirs.get(dir).and_then(|entries| entries.iter().find(|e| e.path == path)).map(|e| e.size)
    })
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn downloaded(id: String, result: Result<Vec<u8>, String>) {
    let mut entry = None;
    global::<Downloads>().update(|d| entry = d.0.remove(&id));
    let Some((path, for_preview)) = entry else { return };
    if for_preview {
        let state = match result {
            Ok(bytes) if bytes.len() as u64 > PREVIEW_MAX_BYTES => Preview::TooLarge,
            Ok(bytes) => Preview::Ready(std::rc::Rc::new(bytes)),
            Err(e) => Preview::Failed(e),
        };
        global::<Previews>().update(|p| {
            p.0.insert(path, state);
        });
        return;
    }
    let name = file_name(&path).to_owned();
    match result {
        Ok(bytes) => save_download(name, bytes),
        Err(e) => notify(tf("Could not download “{}”: {}", &[&name, &e])),
    }
}

/// The phone: into the app's Documents, which the Files app shows as
/// Sophon's folder (`UIFileSharingEnabled`), under a name not taken there.
#[cfg(target_os = "ios")]
fn save_download(name: String, bytes: Vec<u8>) {
    let dir = std::env::var_os("HOME").map(std::path::PathBuf::from).unwrap_or_default().join("Documents");
    let target = free_path(&dir, &name);
    match std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&target, &bytes)) {
        Ok(()) => notify(tf("Saved “{}” to Files › Sophon", &[&target.file_name().and_then(|n| n.to_str()).unwrap_or(&name)])),
        Err(e) => notify(tf("Could not download “{}”: {}", &[&name, &e.to_string()])),
    }
}

/// The desktop: where the save panel says.
#[cfg(all(not(target_os = "ios"), not(target_arch = "wasm32")))]
fn save_download(name: String, bytes: Vec<u8>) {
    spawn_detached(async move {
        let dialog = nsh_core::file_dialog::SaveDialog::new().file_name(name.clone());
        match pick_save_path(dialog).await {
            Ok(Some(target)) => match std::fs::write(&target, &bytes) {
                Ok(()) => notify(tf("Saved “{}”", &[target.file_name().and_then(|n| n.to_str()).unwrap_or(&name)])),
                Err(e) => notify(tf("Could not download “{}”: {}", &[&name, &e.to_string()])),
            },
            Ok(None) => {}
            Err(e) => notify(tf("Could not download “{}”: {}", &[&name, &e])),
        }
    });
}

#[cfg(target_arch = "wasm32")]
fn save_download(name: String, _bytes: Vec<u8>) {
    notify(tf("Could not download “{}”: {}", &[&name, "not in the browser yet"]));
}

/// `dir/name`, or `dir/name (2).ext`, ... when taken.
#[cfg(target_os = "ios")]
fn free_path(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s, format!(".{e}")),
        _ => (name, String::new()),
    };
    (1..)
        .map(|n| if n == 1 { dir.join(name) } else { dir.join(format!("{stem} ({n}){ext}")) })
        .find(|p| !p.exists())
        .expect("a free name")
}

fn uploaded(id: String, result: Result<serde_json::Value, String>) {
    global::<PendingFiles>().update(|p| {
        for list in p.0.values_mut() {
            if let Some(i) = list.iter().position(|f| f.id == id) {
                match &result {
                    Ok(att) => list[i].attachment = Some(att.clone()),
                    Err(e) => {
                        let f = list.remove(i);
                        if !f.detached {
                            notify(tf("Could not upload “{}”: {}", &[&f.name, e]));
                        }
                    }
                }
                return;
            }
        }
    });
}

/// Whether a file of this composer is still on its way to the host.
pub(crate) fn uploading() -> bool {
    pending_files().iter().any(PendingFile::uploading)
}

/// This composer's files, in their chips' order; the detached ones go.
fn take_files() -> Vec<serde_json::Value> {
    let key = image_key();
    let mut out = vec![];
    global::<PendingFiles>().update(|p| out = p.0.remove(&key).unwrap_or_default());
    out.into_iter().filter(|f| !f.detached).filter_map(|f| f.attachment).collect()
}

/// This composer's images, in their chips' order; the detached ones go.
fn take_images() -> Vec<ImageInput> {
    let key = image_key();
    let mut out = vec![];
    global::<PendingImages>().update(|p| out = p.0.remove(&key).unwrap_or_default());
    out.into_iter().filter(|i| !i.detached).map(|i| i.image).collect()
}

/// A saved remote daemon.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct HostProfile {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) url: String,
    pub(crate) password: Option<String>,
}

impl HostProfile {
    /// What the host is called: its name, or, while it has none of its own
    /// (an address pasted with no name; earlier versions stored the address
    /// as the name, a pairing link's keys and token included), the address's
    /// host, never the link.
    pub(crate) fn label(&self) -> String {
        if !self.name.trim().is_empty() && !is_address(&self.name) {
            return self.name.trim().to_owned();
        }
        address_label(&self.url)
    }

    /// Whether the host has a name of its own.
    pub(crate) fn named(&self) -> bool {
        !self.name.trim().is_empty() && !is_address(&self.name)
    }
}

/// A pasted address rather than a name.
fn is_address(text: &str) -> bool {
    let t = text.trim();
    t.contains("://") || t.contains("#offer=") || t.contains("token=")
}

/// A short label for a daemon's address: a pairing link's LAN address (no
/// port), an URL's host, else "Remote host" (a link through the relay only).
fn address_label(url: &str) -> String {
    let url = url.trim();
    if let Some(query) = url.strip_prefix("sophon://pair?") {
        if let Some(lan) = query.split('&').find_map(|kv| kv.strip_prefix("lan=")) {
            let host = lan.rsplit_once(':').map(|(h, _)| h).unwrap_or(lan);
            if !host.is_empty() {
                return host.to_owned();
            }
        }
        return t("Remote host").to_owned();
    }
    if url.contains("#offer=") {
        return t("Remote host").to_owned();
    }
    let rest = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
    let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    let host = host.rsplit_once(':').filter(|(_, p)| p.chars().all(|c| c.is_ascii_digit())).map(|(h, _)| h).unwrap_or(host);
    if host.is_empty() { t("Remote host").to_owned() } else { host.to_owned() }
}

/// Renames saved host `id`.
pub(crate) fn rename_host(id: &str, name: &str) {
    let name = name.trim();
    if name.is_empty() {
        return;
    }
    global::<Hosts>().update(|h| {
        if let Some(p) = h.list.iter_mut().find(|p| p.id == id) {
            p.name = name.to_owned();
        }
    });
    save_hosts();
}

#[derive(Clone, Default)]
struct Hosts {
    list: Vec<HostProfile>,
    /// The host in use; `None` is this machine's daemon.
    current: Option<String>,
}

/// The pairing link of the daemon in use, and a QR code of it (SVG).
#[derive(Clone, Debug, PartialEq, Default)]
pub(crate) struct Pairing {
    pub(crate) loaded: bool,
    pub(crate) url: Option<String>,
    /// Sophon's link: `sophon://pair?lan=…#offer=…`.
    pub(crate) sophon_url: Option<String>,
    pub(crate) qr_svg: Option<String>,
    pub(crate) relay_enabled: bool,
    pub(crate) error: Option<String>,
}

/// Starts the LAN bridge (desktops), so paired phones on this network skip
/// the relay, and phones nearby can ask to pair.
pub(crate) fn start_bridge() {
    static STARTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if !local_service() || STARTED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    #[cfg(not(target_arch = "wasm32"))]
    match crate::daemon::bridge::start() {
        Some((_, events)) => crate::lan::watch_bridge(events),
        None => log::warn!("the LAN bridge could not start; phones pair through the relay only"),
    }
}

/// The link a phone pairs with: the LAN bridge (at its addresses now) and
/// its place at the relay, Sophon's own (a device away from home reaches
/// the bridge, which sees and can cut it off), else the daemon's.
pub(crate) fn pairing_link(daemon_relay: Option<&str>) -> Option<String> {
    #[cfg(not(target_arch = "wasm32"))]
    let own = crate::daemon::bridge::relay_offer();
    #[cfg(target_arch = "wasm32")]
    let own: Option<String> = None;
    let relay_link = own.as_deref().or(daemon_relay);
    // The space in use: its own token, and its name for the device to show.
    let space = current_host().filter(|h| crate::spaces::home_of(&h.url).is_some());
    #[cfg(not(target_arch = "wasm32"))]
    let lan = {
        let home = space.as_ref().and_then(|h| crate::spaces::home_of(&h.url)).map(|h| h.to_string_lossy().into_owned());
        crate::daemon::bridge::info_for(home.as_deref()).map(|b| (b.addr, b.key, b.token))
    };
    #[cfg(target_arch = "wasm32")]
    let lan: Option<(String, [u8; 32], String)> = None;
    if lan.is_none() && relay_link.is_none() {
        return None;
    }
    let name = space.map(|h| format!("{} · {}", crate::lan::device_name(), h.label()));
    Some(crate::daemon::relay::PairingLink::format_named(
        lan.as_ref().map(|(a, k, t)| (a.as_str(), k, t.as_str())),
        relay_link,
        name.as_deref(),
    ))
}

pub(crate) fn bridge_addr() -> Option<String> {
    #[cfg(not(target_arch = "wasm32"))]
    return crate::daemon::bridge::info().map(|b| b.addr);
    #[cfg(target_arch = "wasm32")]
    None
}

/// Runs `future` owned by nothing, so it outlives what started it: a
/// dialog opened from a menu row must still answer once the menu closes
/// (or its list redraws), which disposes the row and every task under it.
pub(crate) fn spawn_detached(future: impl std::future::Future<Output = ()> + 'static) {
    Owner::detached().run(|| {
        spawn(future);
    });
}

/// Answers `sophon://` links: pairs from a scanned `sophon://pair…` link
/// (the system camera opens it), and opens a draft from a
/// `sophon://prompt…` one. One loop takes every link, so no other listener
/// can swallow one.
pub(crate) fn watch_pairing_links() {
    spawn(async move {
        loop {
            let url = next_url_scheme_open().await;
            if let Some(link) = crate::prompt_link::PromptLink::parse(&url) {
                open_prompt_link(link);
            } else if crate::daemon::relay::PairingLink::parse(&url).is_some() {
                if hosts().iter().any(|h| h.url == url) {
                    if let Some(h) = hosts().into_iter().find(|h| h.url == url) {
                        switch_host(Some(h.id));
                    }
                } else {
                    add_host("Paired Mac", &url, "");
                }
                notify(t("Paired").into());
            }
        }
    });
}

/// A prompt link's text waiting for the next new draft, and a link waiting
/// for the host's workspaces and providers (a link can launch the app).
#[derive(Clone, Default)]
struct PromptLinks {
    /// Taken by the next fresh form ([`reset_form`]).
    text: Option<String>,
    waiting: Option<crate::prompt_link::PromptLink>,
    workspaces_known: bool,
    providers_known: bool,
}

/// Opens `link` as a new draft: in its workspace when this host has it,
/// else the open one, else Home's new chat. It only fills the composer;
/// the user reads it and sends it.
fn open_prompt_link(link: crate::prompt_link::PromptLink) {
    let ready = global::<PromptLinks>().with_untracked(|p| p.workspaces_known && p.providers_known);
    if !ready {
        // Navigating now would be undone by restoring the last workspace
        // and by the first providers resetting the form.
        global::<PromptLinks>().update(|p| p.waiting = Some(link));
        // A host with no agents never lists providers: open it anyway
        // after a while rather than hold it for good.
        set_timeout(std::time::Duration::from_secs(5), || {
            prompt_links_known(|p| {
                p.workspaces_known = true;
                p.providers_known = true;
            })
        });
        return;
    }
    let current = untracked(|| current_workspace().map(|w| w.id));
    let target = link.workspace.as_deref().filter(|id| untracked(|| workspace(id)).is_some()).map(str::to_owned).or(current.clone());
    // A new-agent draft already on screen there (the workspace's draft tab,
    // or the new-workspace screen) stays as it is, its folder, branch and
    // model too: the link's text goes under what it holds.
    let in_place = untracked(|| match (screen(), tab()) {
        (Screen::Workspace(id), Tab::Draft) => target.as_deref() == Some(id.as_str()),
        (Screen::NewWorkspace, _) => target.is_none(),
        _ => false,
    });
    if in_place {
        edit_form(|f| {
            f.prompt = if f.prompt.trim().is_empty() { link.text } else { format!("{}\n\n{}", f.prompt.trim_end(), link.text) };
        });
    } else {
        global::<PromptLinks>().update(|p| p.text = Some(link.text));
        crate::ui::open_new_draft(target.as_deref());
    }
    notify(t("Opened a prompt link as a draft. Review it, then send").into());
}

/// Notes that the host's workspaces (or providers) arrived, and opens a
/// link that waited for them.
fn prompt_links_known(change: impl FnOnce(&mut PromptLinks)) {
    global::<PromptLinks>().update(change);
    let waiting = global::<PromptLinks>().with_untracked(|p| if p.workspaces_known && p.providers_known { p.waiting.clone() } else { None });
    if let Some(link) = waiting {
        global::<PromptLinks>().update(|p| p.waiting = None);
        open_prompt_link(link);
    }
}

/// A prompt link's text, once: the fresh draft that takes it.
fn take_prompt_link_text() -> Option<String> {
    let text = global::<PromptLinks>().with_untracked(|p| p.text.clone())?;
    global::<PromptLinks>().update(|p| p.text = None);
    Some(text)
}

/// The text of the draft on screen: the new agent's form, or the open
/// agent's unsent message.
pub(crate) fn current_draft_text() -> String {
    match (screen(), tab()) {
        (Screen::NewWorkspace, _) | (Screen::Workspace(_), Tab::Draft) => form().prompt,
        (Screen::Workspace(_), Tab::Agent(id)) => draft(&id),
        _ => String::new(),
    }
}

/// Copies a prompt link for the draft on screen.
pub(crate) fn copy_prompt_link() {
    let text = current_draft_text();
    if text.trim().is_empty() {
        notify(t("Write a draft first, then copy its link").into());
        return;
    }
    let ws = current_workspace().map(|w| w.id);
    match crate::prompt_link::PromptLink::format(&text, ws.as_deref()) {
        Some(link) => {
            set_clipboard(link);
            notify(t("Prompt link copied").into());
        }
        None => notify(t("This draft is too long for a link").into()),
    }
}

pub(crate) fn pairing() -> Pairing {
    global::<Pairing>().with(Clone::clone)
}

pub(crate) fn open_pairing() {
    global::<Pairing>().update(|p| *p = Pairing::default());
    send(Command::PairingOffer);
    open_modal(Modal::PairDevice);
}

fn qr_svg(text: &str) -> Option<String> {
    let code = qrcode::QrCode::new(text.as_bytes()).ok()?;
    Some(
        code.render::<qrcode::render::svg::Color>()
            .min_dimensions(240, 240)
            .quiet_zone(true)
            .dark_color(qrcode::render::svg::Color("#1f1e1d"))
            .light_color(qrcode::render::svg::Color("#ffffff"))
            .build(),
    )
}

pub(crate) fn hosts() -> Vec<HostProfile> {
    global::<Hosts>().with(|h| h.list.clone())
}

pub(crate) fn current_host() -> Option<HostProfile> {
    global::<Hosts>().with(|h| h.current.as_ref().and_then(|id| h.list.iter().find(|x| &x.id == id).cloned()))
}

/// The folder of the space on this Mac in use: `None` for the first space
/// (Paseo's own `~/.paseo`) or another computer.
pub(crate) fn current_space_home() -> Option<std::path::PathBuf> {
    current_host().and_then(|h| crate::spaces::home_of(&h.url))
}

/// Whether the service in use runs on this Mac: its first space or another.
/// What reads this Mac's files (the agents' own model lists, their CLIs,
/// the session logs) holds for every space alike.
pub(crate) fn on_this_mac() -> bool {
    local_service() && current_host().is_none_or(|h| crate::spaces::home_of(&h.url).is_some())
}

/// What the first space on this Mac is called: its name when renamed, "My
/// space" once there are others, else nothing (the host shows its name).
pub(crate) fn my_space_label() -> Option<String> {
    settings().my_space_name.clone().or_else(|| (!spaces().is_empty()).then(|| t("My space").to_owned()))
}

/// Renames the first space on this Mac (empty: its default name back).
pub(crate) fn rename_my_space(name: &str) {
    let name = name.trim().to_owned();
    edit_settings(|s| s.my_space_name = Some(name).filter(|n| !n.is_empty()));
}

/// The spaces on this Mac besides the first: saved hosts at `space:` addresses.
pub(crate) fn spaces() -> Vec<HostProfile> {
    hosts().into_iter().filter(|h| crate::spaces::home_of(&h.url).is_some()).collect()
}

/// Starts every space's service (Sophon starting on this Mac): Paseo's app
/// keeps only the first one running.
pub(crate) fn start_spaces() {
    if !local_service() {
        return;
    }
    #[cfg(not(any(target_os = "ios", target_arch = "wasm32")))]
    {
        let homes: Vec<_> = spaces().iter().filter_map(|h| crate::spaces::home_of(&h.url)).collect();
        if !homes.is_empty() {
            std::thread::spawn(move || {
                for home in homes {
                    if let Err(e) = crate::spaces::start(&home) {
                        log::warn!("space {}: {e}", home.display());
                    }
                }
            });
        }
    }
}

/// Makes a space called `name` and switches to it once its service runs.
pub(crate) fn create_space(name: &str) {
    #[cfg(not(any(target_os = "ios", target_arch = "wasm32")))]
    {
        let name = name.trim().to_owned();
        notify(tf("Creating “{}”…", &[&name]));
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let n = name.clone();
        std::thread::spawn(move || {
            let _ = tx.send(crate::spaces::create(&n));
        });
        spawn_detached(async move {
            match rx.recv().await {
                Some(Ok(home)) => add_host(&name, &crate::spaces::url_of(&home), ""),
                Some(Err(e)) => notify(tf("Cannot create the space: {}", &[&e])),
                None => {}
            }
        });
    }
    #[cfg(any(target_os = "ios", target_arch = "wasm32"))]
    let _ = name;
}

fn save_hosts() {
    let h = global::<Hosts>().with_untracked(Clone::clone);
    let json = serde_json::json!({
        "current": h.current,
        "hosts": h.list.iter().map(|x| serde_json::json!({ "id": x.id, "name": x.name, "url": x.url, "password": x.password })).collect::<Vec<_>>(),
    });
    if let Err(e) = crate::daemon::write_file("hosts.json", &json.to_string()) {
        log::warn!("cannot save hosts: {e}");
    }
}

/// Restores the saved hosts and connects to the one last used.
pub(crate) fn load_hosts() {
    let Some(text) = crate::daemon::read_file("hosts.json") else { return };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { return };
    let list: Vec<HostProfile> = v
        .get("hosts")
        .and_then(|h| h.as_array())
        .map(|l| {
            l.iter()
                .filter_map(|x| {
                    let s = |k: &str| x.get(k).and_then(|v| v.as_str()).map(str::to_owned);
                    Some(HostProfile { id: s("id")?, name: s("name").unwrap_or_default(), url: s("url")?, password: s("password") })
                })
                .collect()
        })
        .unwrap_or_default();
    let current = v.get("current").and_then(|c| c.as_str()).map(str::to_owned).filter(|c| list.iter().any(|x| &x.id == c));
    global::<Hosts>().update(|h| *h = Hosts { list, current });
    if !crate::ui::kit::MOBILE && !local_service() && current_host().is_none_or(|h| crate::spaces::home_of(&h.url).is_some()) {
        let other = hosts().into_iter().find(|h| crate::spaces::home_of(&h.url).is_none()).map(|h| h.id);
        global::<Hosts>().update(|h| h.current = other);
    }
    if let Some(host) = current_host() {
        crate::daemon::connection::switch_host(Some(crate::daemon::connection::RemoteHost { url: host.url, password: host.password }));
    }
}

/// Connects to `id` (`None`: this machine), starting fresh.
/// The host picked in a menu: nothing when it is the one in use (a switch
/// would drop its lists and what is open, to load the same again).
pub(crate) fn choose_host(id: Option<String>) {
    if global::<Hosts>().with_untracked(|h| h.current != id) {
        switch_host(id);
    }
}

/// Whether the workspace open last was looked for already: until then the
/// home screen stays empty, as that workspace is about to cover it.
pub(crate) fn last_open_restored() -> bool {
    global::<LastOpen>().with(|l| l.done)
}

pub(crate) fn switch_host(id: Option<String>) {
    global::<Hosts>().update(|h| h.current = id.clone());
    save_hosts();
    batch(|| {
        global::<Projects>().update(|p| p.0.clear());
        global::<Workspaces>().update(|w| w.0.clear());
        global::<Agents>().update(|a| a.0.clear());
        global::<Providers>().update(|p| p.0.clear());
        global::<Diagnostics>().update(|d| *d = Diagnostics::default());
        global::<DaemonConfig>().update(|c| c.0 = None);
        global::<Nav>().update(|n| *n = Nav::default());
        // The workspace open last is looked for in this host's too.
        global::<LastOpen>().update(|l| l.done = false);
    });
    let remote = current_host().map(|h| crate::daemon::connection::RemoteHost { url: h.url, password: h.password });
    crate::daemon::connection::switch_host(remote);
}

pub(crate) fn add_host(name: &str, url: &str, password: &str) {
    // A pairing link that names the computer (a space's) names it here.
    let named = crate::daemon::relay::PairingLink::parse(url).and_then(|l| l.name);
    let name = named.as_deref().unwrap_or(name);
    let id = format!("host_{}", uuid::Uuid::new_v4().simple());
    let profile = HostProfile {
        id: id.clone(),
        // No name: the host's own once connected (see `Event::Connected`).
        name: name.trim().to_owned(),
        url: url.trim().to_owned(),
        password: Some(password.to_owned()).filter(|p| !p.is_empty()),
    };
    global::<Hosts>().update(|h| h.list.push(profile));
    switch_host(Some(id));
}

pub(crate) fn remove_host(id: &str) {
    // A space's service stops with it; its folder, with its conversations,
    // stays.
    #[cfg(not(any(target_os = "ios", target_arch = "wasm32")))]
    if let Some(home) = hosts().into_iter().find(|h| h.id == id).and_then(|h| crate::spaces::home_of(&h.url)) {
        std::thread::spawn(move || {
            if let Err(e) = crate::spaces::stop(&home) {
                log::warn!("space {}: {e}", home.display());
            }
        });
    }
    let was_current = global::<Hosts>().with_untracked(|h| h.current.as_deref() == Some(id));
    global::<Hosts>().update(|h| h.list.retain(|x| x.id != id));
    if was_current {
        switch_host(None);
    } else {
        save_hosts();
    }
}

pub(crate) fn settings() -> Settings {
    global::<Settings>().with(Clone::clone)
}

pub(crate) fn edit_settings(change: impl FnOnce(&mut Settings)) {
    global::<Settings>().update(change);
    save_prefs();
}


/// What Sophon remembers between launches.
fn save_prefs() {
    let s = global::<Settings>().with_untracked(Clone::clone);
    let json = serde_json::json!({
        "sendBehavior": match s.send_behavior {
            SendBehavior::Steer => "steer",
            SendBehavior::Interrupt => "interrupt",
            SendBehavior::Queue => "queue",
        },
        "localService": s.local_service.0,
        "mySpaceName": s.my_space_name,
        "push": s.push.0,
        "pushHide": s.push_hide,
        "pushDevice": s.push_device,
        "keepAwake": match s.keep_awake {
            KeepAwake::Off => "off",
            KeepAwake::On => "on",
            KeepAwake::Display => "display",
        },
        "sidebarHidden": global::<SidebarHidden>().with_untracked(|h| h.0),
        "explorerOpen": global::<Explorer>().with_untracked(|e| e.open),
        "sidebarWidth": global::<SidebarWidth>().with_untracked(|w| w.0),
        "explorerWidth": global::<ExplorerWidth>().with_untracked(|w| w.0),
        "terminalHeight": global::<TerminalPanel>().with_untracked(|p| p.height),
        "projectOrder": global::<ProjectOrder>().with_untracked(|o| o.0.clone()),
        "keptRuns": global::<KeptRuns>().with_untracked(|k| k.0.clone()),
        "lastWorkspace": global::<LastOpen>().with_untracked(|l| l.workspace.clone()),
        "lastAgent": global::<LastOpen>().with_untracked(|l| l.agent.clone()),
        "collapsedProjects": global::<Collapsed>().with_untracked(|c| c.0.clone()),
        "favoriteModels": global::<FavoriteModels>().with_untracked(|f| f.0.clone()),
        "language": crate::i18n::lang_untracked().as_str(),
        "appearance": match global::<Theme>().with_untracked(|t| t.0) {
            nsh_core::platform::Appearance::Light => "light",
            nsh_core::platform::Appearance::Dark => "dark",
            _ => "system",
        },
    });
    if let Err(e) = crate::daemon::write_file("settings.json", &json.to_string()) {
        log::warn!("cannot save settings: {e}");
    }
}

/// Restores what `save_prefs` wrote.
pub(crate) fn load_prefs() {
    let Some(text) = crate::daemon::read_file("settings.json") else {
        #[cfg(not(target_arch = "wasm32"))]
        if !crate::ui::kit::MOBILE && service_cli().is_none() {
            global::<Settings>().update(|s| s.local_service = LocalService(false));
        }
        return;
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { return };
    let behavior = match v.get("sendBehavior").and_then(serde_json::Value::as_str) {
        Some("interrupt") => SendBehavior::Interrupt,
        Some("queue") => SendBehavior::Queue,
        _ => SendBehavior::Steer,
    };
    let keep_awake = match v.get("keepAwake").and_then(serde_json::Value::as_str) {
        Some("off") => KeepAwake::Off,
        Some("display") => KeepAwake::Display,
        _ => KeepAwake::On,
    };
    // A Mac without Paseo starts as one that only connects to others.
    let local = v.get("localService").and_then(serde_json::Value::as_bool).unwrap_or_else(|| cfg!(target_arch = "wasm32") || service_cli().is_some());
    let my_space = v.get("mySpaceName").and_then(serde_json::Value::as_str).map(str::to_owned).filter(|n| !n.trim().is_empty());
    global::<Settings>().update(|s| {
        s.send_behavior = behavior;
        s.keep_awake = keep_awake;
        s.local_service = LocalService(local);
        s.my_space_name = my_space;
        s.push = PushWanted(v.get("push").and_then(serde_json::Value::as_bool).unwrap_or(true));
        s.push_hide = v.get("pushHide").and_then(serde_json::Value::as_bool).unwrap_or(false);
        s.push_device = v.get("pushDevice").and_then(serde_json::Value::as_str).unwrap_or_default().to_owned();
    });
    if let Some(h) = v.get("sidebarHidden").and_then(serde_json::Value::as_bool) {
        global::<SidebarHidden>().update(|s| s.0 = h);
    }
    if let Some(o) = v.get("explorerOpen").and_then(serde_json::Value::as_bool) {
        global::<Explorer>().update(|e| e.open = o);
    }
    let strings = |k: &str| -> Vec<String> {
        v.get(k).and_then(serde_json::Value::as_array).map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect()).unwrap_or_default()
    };
    global::<ProjectOrder>().set(ProjectOrder(strings("projectOrder")));
    let string = |k: &str| v.get(k).and_then(serde_json::Value::as_str).map(str::to_owned);
    global::<LastOpen>().set(LastOpen { workspace: string("lastWorkspace"), agent: string("lastAgent"), done: false });
    global::<Collapsed>().set(Collapsed(strings("collapsedProjects")));
    global::<KeptRuns>().set(KeptRuns(strings("keptRuns")));
    global::<FavoriteModels>().set(FavoriteModels(strings("favoriteModels")));
    if let Some(h) = v.get("terminalHeight").and_then(serde_json::Value::as_f64) {
        global::<TerminalPanel>().update(|p| p.height = (h as f32).clamp(120.0, 900.0));
    }
    if let Some(w) = v.get("explorerWidth").and_then(serde_json::Value::as_f64) {
        global::<ExplorerWidth>().set(ExplorerWidth((w as f32).clamp(EXPLORER_MIN, EXPLORER_MAX)));
    }
    if let Some(w) = v.get("sidebarWidth").and_then(serde_json::Value::as_f64) {
        global::<SidebarWidth>().set(SidebarWidth((w as f32).clamp(SIDEBAR_MIN, SIDEBAR_MAX)));
    }
    if let Some(l) = v.get("language").and_then(serde_json::Value::as_str) {
        crate::i18n::set_lang(crate::i18n::Lang::parse(l));
    }
    let appearance = match v.get("appearance").and_then(serde_json::Value::as_str) {
        Some("light") => nsh_core::platform::Appearance::Light,
        Some("dark") => nsh_core::platform::Appearance::Dark,
        _ => nsh_core::platform::Appearance::System,
    };
    global::<Theme>().set(Theme(appearance));
    if appearance != nsh_core::platform::Appearance::System {
        set_color_scheme(appearance);
    }
}

#[derive(Clone, Default, PartialEq)]
struct Theme(nsh_core::platform::Appearance);

/// Light, dark, or the system's appearance.
pub(crate) fn appearance() -> nsh_core::platform::Appearance {
    global::<Theme>().with(|t| t.0)
}

/// Shows the app in `appearance` and remembers it.
pub(crate) fn set_appearance(appearance: nsh_core::platform::Appearance) {
    global::<Theme>().set(Theme(appearance));
    set_color_scheme(appearance);
    save_prefs();
}

/// The service's command line: on the PATH, or inside the desktop app that
/// ships it.
pub(crate) fn service_cli() -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from).unwrap_or_default();
    let mut candidates: Vec<std::path::PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).map(|d| d.join("paseo")).collect())
        .unwrap_or_default();
    candidates.extend([
        "/opt/homebrew/bin/paseo".into(),
        "/usr/local/bin/paseo".into(),
        home.join(".local/bin/paseo"),
        "/Applications/Paseo.app/Contents/Resources/bin/paseo".into(),
        home.join("Applications/Paseo.app/Contents/Resources/bin/paseo"),
    ]);
    candidates.into_iter().find(|p| p.is_file())
}

/// Whether this Mac's service comes with the desktop app (its updates come
/// with the app's) rather than from npm, whose package the service can
/// update itself.
pub(crate) fn service_bundled() -> bool {
    service_cli()
        .and_then(|p| std::fs::canonicalize(p).ok())
        .is_some_and(|p| !p.to_string_lossy().contains("node_modules"))
}

#[derive(Clone, Default, PartialEq)]
struct Starting(bool);

pub(crate) fn service_starting() -> bool {
    global::<Starting>().with(|s| s.0)
}

/// The service's start this run: tried on its own once the connection to
/// this Mac's service fails (the app starts it, as a Mac app starts what it
/// needs), and why it failed, if it did.
#[derive(Clone, Default, PartialEq)]
struct AutoStart {
    tried: bool,
    error: Option<String>,
}

/// Why this Mac's service could not be started, if starting it failed.
pub(crate) fn service_start_error() -> Option<String> {
    global::<AutoStart>().with(|a| a.error.clone())
}

/// This Mac's service is down: start it, once a run (a service stopped on
/// purpose afterwards stays stopped; Start service starts it again).
fn auto_start_service() {
    if crate::ui::kit::MOBILE || cfg!(target_arch = "wasm32") || !on_this_mac() {
        return;
    }
    if global::<AutoStart>().with_untracked(|a| a.tried) {
        return;
    }
    global::<AutoStart>().update(|a| a.tried = true);
    start_service();
}

/// Starts the agent service on this Mac; the connection, which retries on
/// its own, picks it up.
pub(crate) fn start_service() {
    if global::<Starting>().with_untracked(|s| s.0) {
        return;
    }
    let Some(cli) = service_cli() else {
        global::<AutoStart>().update(|a| a.error = Some(t("The service is not installed on this Mac").into()));
        return;
    };
    global::<AutoStart>().update(|a| a.error = None);
    global::<Starting>().set(Starting(true));
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Result<(), String>>();
    let home = current_space_home();
    std::thread::spawn(move || {
        let mut args = vec!["daemon".to_owned(), "start".to_owned()];
        if let Some(home) = home {
            args.extend(["--home".to_owned(), home.to_string_lossy().into_owned()]);
        }
        let out = std::process::Command::new(&cli).args(&args).output();
        let _ = tx.send(match out {
            Ok(o) if o.status.success() => Ok(()),
            Ok(o) => Err(String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or("").to_owned()),
            Err(e) => Err(e.to_string()),
        });
    });
    spawn(async move {
        let result = rx.recv().await;
        global::<Starting>().set(Starting(false));
        match result {
            Some(Err(e)) => global::<AutoStart>().update(|a| a.error = Some(e)),
            // Up: connect now rather than after the wait between attempts
            // (up to 30 s once a few have failed).
            _ => send(Command::Reconnect),
        }
    });
}

/// Sets the UI's language and remembers it.
pub(crate) fn set_language(lang: crate::i18n::Lang) {
    crate::i18n::set_lang(lang);
    save_prefs();
}

pub(crate) fn queue(agent_id: &str) -> Vec<String> {
    global::<Queues>().with(|q| q.0.get(agent_id).cloned().unwrap_or_default())
}

/// Whether `agent_id`'s queue waits (stopped with Escape).
pub(crate) fn queue_held(agent_id: &str) -> bool {
    global::<QueueHeld>().with(|h| h.0.contains(agent_id))
}

/// Escape in the open agent's composer, as Claude Code's: while the agent
/// runs it stops it, and what is queued waits instead of being sent; once
/// more, the queued messages come back into the composer to be edited.
pub(crate) fn escape_composer() {
    let Some(agent) = selected_agent() else { return };
    let queued = global::<Queues>().with_untracked(|q| q.0.get(&agent.id).cloned().unwrap_or_default());
    if agent.is_running() {
        if !queued.is_empty() {
            global::<QueueHeld>().update(|h| {
                h.0.insert(agent.id.clone());
            });
        }
        send(Command::Cancel(agent.id));
        return;
    }
    // Attachments stand as chips in the composer's text, which this would
    // replace: they are sent or removed first.
    if queued.is_empty() || !pending_images().is_empty() || !pending_files().is_empty() {
        return;
    }
    global::<Queues>().update(|q| {
        q.0.remove(&agent.id);
    });
    global::<QueueHeld>().update(|h| {
        h.0.remove(&agent.id);
    });
    let mut text = queued.join("\n");
    let draft = draft(&agent.id);
    if !draft.trim().is_empty() {
        text.push('\n');
        text.push_str(&draft);
    }
    set_draft(&agent.id, &text);
}

pub(crate) fn unqueue(agent_id: &str, index: usize) {
    global::<Queues>().update(|q| {
        if let Some(list) = q.0.get_mut(agent_id)
            && index < list.len()
        {
            list.remove(index);
        }
    });
}

/// The agent's slash commands, asked for once per agent.
pub(crate) fn commands(agent_id: &str) -> Vec<SlashCommand> {
    let known = global::<Commands>().with(|c| c.0.get(agent_id).cloned());
    match known {
        Some(list) => list,
        None => {
            global::<Commands>().update(|c| {
                c.0.insert(agent_id.to_owned(), vec![]);
            });
            send(Command::ListCommands(agent_id.to_owned()));
            vec![]
        }
    }
}

/// Files and folders for an @ mention: asks the daemon when the query
/// changes and gives the last answer meanwhile.
pub(crate) fn suggestions(cwd: &str, query: &str) -> Vec<(String, bool)> {
    let (last, entries) = global::<Suggestions>().with(|s| (s.query.clone(), s.entries.clone()));
    if last != query {
        global::<Suggestions>().update(|s| s.query = query.to_owned());
        send(Command::Suggest { cwd: cwd.to_owned(), query: query.to_owned() });
    }
    entries
}

pub(crate) fn notice() -> Option<(String, Option<NoticeAction>)> {
    global::<Notice>().with(|n| n.0.as_ref().map(|(_, m, a)| (m.clone(), a.clone())))
}

pub(crate) fn run_notice_action() {
    let action = global::<Notice>().with_untracked(|n| n.0.as_ref().and_then(|(_, _, a)| a.clone()));
    dismiss_notice();
    match action {
        Some(NoticeAction::UndoArchive(id)) => send(Command::UndoArchive(id)),
        None => {}
    }
}

/// A folder of the host being browsed (the phone's add project screen).
#[derive(Clone, Default, PartialEq)]
pub(crate) struct Browse {
    pub(crate) cwd: String,
    pub(crate) loaded: bool,
    pub(crate) folders: Vec<String>,
    pub(crate) error: Option<String>,
}

pub(crate) fn browse_state() -> Browse {
    global::<Browse>().with(Clone::clone)
}

/// Lists the folders in `cwd`, an absolute path on the host.
pub(crate) fn browse(cwd: &str) {
    global::<Browse>().set(Browse { cwd: cwd.to_owned(), ..Default::default() });
    send(Command::ListDir { cwd: cwd.to_owned(), path: ".".into() });
}

/// The host's home folder, as its workspaces' paths show it
/// (`/Users/name`, `/home/name`), or `/`.
pub(crate) fn host_home() -> String {
    let paths: Vec<String> = global::<Workspaces>().with(|w| w.0.iter().map(|w| w.project_root.clone()).collect());
    paths
        .iter()
        .find_map(|p| {
            let parts: Vec<&str> = p.split('/').collect();
            (parts.len() > 2 && matches!(parts[1], "Users" | "home")).then(|| format!("/{}/{}", parts[1], parts[2]))
        })
        .unwrap_or_else(|| "/".into())
}

/// Folders on the host matching `query`: asks when the query changes and
/// gives the last answer meanwhile.
pub(crate) fn find_dirs(query: &str) -> Vec<String> {
    let (last, entries) = global::<Suggestions>().with(|s| (s.query.clone(), s.entries.clone()));
    if last != query {
        global::<Suggestions>().update(|s| s.query = query.to_owned());
        send(Command::FindDirs { query: query.to_owned() });
    }
    entries.into_iter().filter(|(_, dir)| *dir).map(|(p, _)| p).collect()
}

pub(crate) fn add_project(path: &str) {
    send(Command::AddProject(path.to_owned()));
}

pub(crate) fn create_project_dir(parent: &str, name: &str) {
    send(Command::CreateProjectDir { parent: parent.to_owned(), name: name.to_owned() });
}

pub(crate) fn clone_project(repo: &str, target: &str) {
    send(Command::CloneProject { repo: repo.to_owned(), target: target.to_owned() });
}

pub(crate) fn rename_project(id: &str, name: &str) {
    send(Command::RenameProject { id: id.to_owned(), name: name.to_owned() });
}

pub(crate) fn remove_project(id: &str) {
    global::<Projects>().update(|p| p.0.retain(|x| x.id != id));
    send(Command::RemoveProject(id.to_owned()));
}

pub(crate) fn mark_unread(id: &str) {
    send(Command::MarkUnread(id.to_owned()));
}

pub(crate) fn form() -> Form {
    global::<Form>().with(Clone::clone)
}

pub(crate) fn draft(agent_id: &str) -> String {
    global::<Drafts>().with(|d| d.0.get(agent_id).cloned().unwrap_or_default())
}

pub(crate) fn set_draft(agent_id: &str, text: &str) {
    global::<Drafts>().update(|d| {
        d.0.insert(agent_id.to_owned(), text.to_owned());
    });
}

// -------------------------------------------------------------- navigation

#[derive(Clone, Default)]
struct HistoryState {
    search: String,
    agents: Vec<Agent>,
    loaded: bool,
}

pub(crate) fn open_history() {
    global::<Nav>().update(|n| n.screen = Screen::History);
    send(Command::WatchDiff(None));
    search_history(&global::<HistoryState>().with_untracked(|h| h.search.clone()));
}

pub(crate) fn search_history(query: &str) {
    global::<HistoryState>().update(|h| h.search = query.to_owned());
    send(Command::History { search: query.to_owned() });
}

pub(crate) fn history() -> (String, Vec<Agent>, bool) {
    global::<HistoryState>().with(|h| (h.search.clone(), h.agents.clone(), h.loaded))
}

/// Opens a session: its workspace and tab, restoring it first when archived.
pub(crate) fn open_session(agent: &Agent) {
    if agent.archived {
        send(Command::Unarchive(agent.id.clone()));
        global::<HistoryState>().update(|h| {
            if let Some(a) = h.agents.iter_mut().find(|a| a.id == agent.id) {
                a.archived = false;
            }
        });
    }
    if let Some(ws) = agent.workspace_id.clone() {
        global::<Nav>().update(|n| n.screen = Screen::Workspace(ws.clone()));
        watch_changes(&ws);
        open_tab(Tab::Agent(agent.id.clone()));
    }
}

#[derive(Clone, Default)]
struct SchedulesState {
    list: Option<Vec<crate::daemon::connection::Schedule>>,
    /// The runs of the schedules opened on the page, by schedule id.
    runs: HashMap<String, Vec<crate::daemon::connection::ScheduleRun>>,
}

pub(crate) fn open_schedules() {
    global::<Nav>().update(|n| n.screen = Screen::Schedules);
    send(Command::WatchDiff(None));
    send(Command::Schedules);
}

pub(crate) fn schedules() -> Option<Vec<crate::daemon::connection::Schedule>> {
    global::<SchedulesState>().with(|s| s.list.clone())
}

/// A schedule's runs, newest first, once asked for (`toggle_schedule_runs`).
pub(crate) fn schedule_runs(id: &str) -> Option<Vec<crate::daemon::connection::ScheduleRun>> {
    global::<SchedulesState>().with(|s| s.runs.get(id).cloned())
}

/// Shows a schedule's runs, or puts them away.
pub(crate) fn toggle_schedule_runs(id: &str) {
    let shown = global::<SchedulesState>().with_untracked(|s| s.runs.contains_key(id));
    if shown {
        global::<SchedulesState>().update(|s| {
            s.runs.remove(id);
        });
    } else {
        global::<SchedulesState>().update(|s| {
            s.runs.insert(id.to_owned(), Vec::new());
        });
        send(Command::ScheduleRuns(id.to_owned()));
    }
}

/// Opens the conversation a run started, and lists its workspace from then
/// on. The service archives a run's workspace when the run ends: that is
/// restored first (the workspace, then its agent), and the conversation
/// opens once the host lists the workspace again. Also how any conversation
/// whose workspace was archived under it opens.
pub(crate) fn open_run_agent(agent_id: &str, workspace_id: Option<&str>) {
    let workspace_id = workspace_id.map(str::to_owned).or_else(|| agent(agent_id).and_then(|a| a.workspace_id));
    let workspace_id = workspace_id.as_deref();
    let listed = agent(agent_id).and_then(|a| a.workspace_id).filter(|ws| workspace(ws).is_some());
    let Some(ws) = listed.clone().or_else(|| workspace_id.map(str::to_owned)) else {
        notify(t("That conversation is no longer listed; look in History").to_owned());
        return;
    };
    global::<KeptRuns>().update(|k| {
        if !k.0.contains(&ws) {
            k.0.push(ws.clone());
            // The last few hundred: runs opened long ago are archived again or gone.
            let extra = k.0.len().saturating_sub(300);
            k.0.drain(..extra);
        }
    });
    save_prefs();
    if listed.is_some() {
        open_workspace(&ws);
        open_tab(Tab::Agent(agent_id.to_owned()));
        return;
    }
    global::<OpenWhenListed>().set(OpenWhenListed(Some((ws.clone(), agent_id.to_owned()))));
    send(Command::UndoArchive(ws));
    send(Command::Unarchive(agent_id.to_owned()));
    notify(t("Restoring the conversation…").to_owned());
}

/// Opens the run's conversation that was waiting for its workspace.
fn open_when_listed() {
    let Some((ws, agent)) = global::<OpenWhenListed>().with_untracked(|o| o.0.clone()) else { return };
    if workspace(&ws).is_some() {
        global::<OpenWhenListed>().set(OpenWhenListed(None));
        open_workspace(&ws);
        open_tab(Tab::Agent(agent));
        global::<RestoredOpens>().update(|r| r.0 += 1);
    }
}

pub(crate) fn schedule_op(id: &str, op: &'static str) {
    send(Command::ScheduleOp { id: id.to_owned(), op });
}

fn schedule_of(name: String, prompt: String, cadence: String, cwd: String, worktree: bool) -> crate::daemon::connection::NewSchedule {
    let f = form();
    let provider = if f.provider.is_empty() { "claude".to_owned() } else { f.provider };
    crate::daemon::connection::NewSchedule {
        name,
        prompt,
        cadence,
        timezone: crate::cadence::local_zone(),
        cwd,
        provider,
        model: f.model,
        worktree,
    }
}

pub(crate) fn create_schedule(name: String, prompt: String, cadence: String, cwd: String, worktree: bool) {
    send(Command::CreateSchedule(schedule_of(name, prompt, cadence, cwd, worktree)));
}

/// Changes a schedule; its agent and model stay as they were.
pub(crate) fn update_schedule(id: &str, name: String, prompt: String, cadence: String, cwd: String, worktree: bool) {
    send(Command::UpdateSchedule { id: id.to_owned(), to: schedule_of(name, prompt, cadence, cwd, worktree) });
}

#[derive(Clone, Default)]
struct DaemonConfig(Option<serde_json::Value>);

pub(crate) fn daemon_config() -> Option<serde_json::Value> {
    global::<DaemonConfig>().with(|c| c.0.clone())
}

/// Sets `pointer` (`/autoArchiveAfterMerge`) to `value`, shown at once and
/// sent as a patch.
pub(crate) fn set_daemon_config(pointer: &str, value: serde_json::Value) {
    let patch = config_patch(pointer, value.clone());
    let keys: Vec<&str> = pointer.trim_start_matches('/').split('/').collect();
    global::<DaemonConfig>().update(|c| {
        if let Some(cfg) = c.0.as_mut() {
            let mut node = cfg;
            for (i, k) in keys.iter().enumerate() {
                if i + 1 == keys.len() {
                    node[*k] = value.clone();
                } else {
                    node = &mut node[*k];
                }
            }
        }
    });
    send(Command::SetConfig(patch));
}

/// The patch that sets only `pointer` to `value`: `/a/b` and 1 make
/// `{"a":{"b":1}}`. The daemon merges it, so every other key stays.
fn config_patch(pointer: &str, value: serde_json::Value) -> serde_json::Value {
    let mut patch = serde_json::json!({});
    let mut node = &mut patch;
    for k in pointer.trim_start_matches('/').split('/') {
        node = &mut node[k];
    }
    *node = value;
    patch
}

/// Where the daemon keeps the model that names chats and branches
/// (`agents.metadataGeneration`): an ordered list it tries, empty for its
/// own choice.
const TEXT_GENERATION: &str = "/metadataGeneration/providers";

/// The value [`TEXT_GENERATION`] takes for `choice` (provider, model):
/// that one model, or none (the daemon's default).
fn text_generation_value(choice: Option<(&str, &str)>) -> serde_json::Value {
    match choice {
        Some((provider, model)) => serde_json::json!([{ "provider": provider, "model": model }]),
        None => serde_json::json!([]),
    }
}

/// The model the daemon writes titles with: (provider, model), or `None`
/// for its default.
pub(crate) fn text_generation_model() -> Option<(String, String)> {
    let config = daemon_config()?;
    let first = config.pointer(&format!("{TEXT_GENERATION}/0"))?;
    let provider = first.get("provider")?.as_str()?.to_owned();
    let model = first.get("model").and_then(|m| m.as_str()).unwrap_or_default().to_owned();
    Some((provider, model))
}

/// Sets the model the daemon writes titles with; `None` gives the choice
/// back to the daemon. A refused model is never sent.
pub(crate) fn set_text_generation_model(choice: Option<(&str, &str)>) {
    if let Some((provider, model)) = choice
        && refuse_model(provider, model)
    {
        return;
    }
    set_daemon_config(TEXT_GENERATION, text_generation_value(choice));
}

// ------------------------------------------------------------------ agents

/// Providers' diagnostics (binary, version, login), by provider id.
/// `wanted`: the Agents page is open, so new providers get theirs too.
#[derive(Clone, Default)]
struct Diagnostics {
    map: HashMap<String, crate::daemon::agents::Diagnostic>,
    wanted: bool,
}

pub(crate) fn diagnostic(id: &str) -> Option<crate::daemon::agents::Diagnostic> {
    global::<Diagnostics>().with(|d| d.map.get(id).cloned())
}

/// Asks every provider for its diagnostic: the Agents page shows them.
pub(crate) fn load_diagnostics() {
    global::<Diagnostics>().update(|d| d.wanted = true);
    for p in global::<Providers>().with_untracked(|p| p.0.clone()) {
        send(Command::ProviderDiagnostic(p.id));
    }
}

/// The host's home folder for agent config folders: from Claude Code's
/// config folder there, this machine's own when the host is this machine,
/// else as the workspaces' paths show it.
pub(crate) fn agent_home() -> Option<String> {
    let from_claude = global::<Diagnostics>().with(|d| {
        d.map
            .values()
            .filter_map(|x| x.login.as_ref()?.config_dir.as_deref())
            .find_map(crate::daemon::agents::home_from_config_dir)
    });
    from_claude.or_else(|| {
        if !on_this_mac() {
            return None;
        }
        std::env::var("HOME").ok().filter(|h| h.starts_with('/'))
    })
    .or_else(|| Some(host_home()).filter(|h| h != "/"))
}

/// Every provider id the host knows, in the list or only in the config.
pub(crate) fn provider_ids() -> Vec<String> {
    let mut ids: Vec<String> = providers().into_iter().map(|p| p.id).collect();
    if let Some(cfg) = daemon_config()
        && let Some(map) = cfg.get("providers").and_then(serde_json::Value::as_object)
    {
        ids.extend(map.keys().cloned());
    }
    ids
}

/// Sends a providers patch, merged into the shown config at once; the
/// daemon answers with the whole config and the list refreshes after.
pub(crate) fn patch_providers(patch: serde_json::Value) {
    global::<DaemonConfig>().update(|c| {
        if let Some(cfg) = c.0.as_mut() {
            if let Some(remove) = patch.get("removeProviders").and_then(serde_json::Value::as_array)
                && let Some(map) = cfg.get_mut("providers").and_then(serde_json::Value::as_object_mut)
            {
                for id in remove.iter().filter_map(serde_json::Value::as_str) {
                    map.remove(id);
                }
            }
            if let Some(add) = patch.get("providers").and_then(serde_json::Value::as_object) {
                for (id, fields) in add {
                    let entry = &mut cfg["providers"][id.as_str()];
                    if !entry.is_object() {
                        *entry = serde_json::json!({});
                    }
                    if let (Some(e), Some(f)) = (entry.as_object_mut(), fields.as_object()) {
                        e.extend(f.clone());
                    }
                }
            }
        }
    });
    send(Command::PatchProviders(patch));
}

thread_local! {
    /// The command patches sent for the apps' CLIs, by host and provider:
    /// each is sent once, so an agent that stays unavailable on the app's
    /// CLI is not patched over and over.
    static BUNDLED_SENT: RefCell<std::collections::HashSet<String>> = RefCell::new(std::collections::HashSet::new());
}

/// Claude Code and Codex not installed for the service, on this Mac, run
/// from the CLI their desktop app carries ([`crate::bundled_cli`]); a
/// command already there follows the app's updates.
fn use_bundled_clis() {
    if !on_this_mac() {
        return;
    }
    let Some(home) = std::env::var_os("HOME").map(std::path::PathBuf::from) else { return };
    let list = global::<Providers>().with_untracked(|p| p.0.clone());
    let config = global::<DaemonConfig>().with_untracked(|c| c.0.clone());
    let host = current_host().map(|h| h.url).unwrap_or_default();
    for id in ["claude", "codex"] {
        let Some(p) = list.iter().find(|p| p.id == id) else { continue };
        let unavailable = p.enabled && p.status == "unavailable";
        if config.is_none() {
            // The config says whether the command is someone's own: ask
            // for it first, and come back with it.
            if unavailable && crate::bundled_cli::find(id).is_some() {
                send(Command::GetConfig);
                return;
            }
            continue;
        }
        let Some(patch) = crate::bundled_cli::patch_for(id, unavailable, config.as_ref(), &home) else { continue };
        if !BUNDLED_SENT.with_borrow_mut(|sent| sent.insert(format!("{host} {patch}"))) {
            continue;
        }
        log::info!("{id}: its command follows the desktop app's CLI");
        if patch["providers"][id]["command"][0].as_str().is_some_and(|c| c.starts_with('/')) {
            notify(tf("{} runs from its desktop app: it is not installed on its own", &[&p.label]));
        }
        patch_providers(patch);
    }
}

/// What the Agents section reads: the host's config and the diagnostics.
pub(crate) fn load_agents() {
    send(Command::GetConfig);
    load_diagnostics();
}

/// A local agent CLI's update: how it was installed, the newest version that
/// way, and an update under way or failed. By the binary the service runs,
/// which instances of one CLI share.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct AgentUpdate {
    pub(crate) install: Option<crate::agent_update::Install>,
    pub(crate) latest: Option<String>,
    pub(crate) updating: bool,
    pub(crate) error: Option<String>,
}

#[derive(Clone, Default)]
struct AgentUpdates(HashMap<String, AgentUpdate>);

/// The update of the CLI at `path`, while the host is this computer.
pub(crate) fn agent_update(path: &str) -> Option<AgentUpdate> {
    if !on_this_mac() {
        return None;
    }
    global::<AgentUpdates>().with(|u| u.0.get(path).cloned())
}

/// Looks up the newest version of the CLI at `path`, once a run: a network
/// or Homebrew call, on a thread of its own.
#[cfg(not(any(target_os = "ios", target_arch = "wasm32")))]
fn check_agent_update(path: String) {
    if !on_this_mac() || global::<AgentUpdates>().with_untracked(|u| u.0.contains_key(&path)) {
        return;
    }
    let real = std::fs::canonicalize(&path).map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|_| path.clone());
    let install = crate::agent_update::install_of(&real);
    global::<AgentUpdates>().update(|u| {
        u.0.insert(path.clone(), AgentUpdate { install: install.clone(), ..Default::default() });
    });
    let Some(install) = install else { return };
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    std::thread::spawn(move || {
        let _ = tx.send(crate::agent_update::latest(&install));
    });
    spawn_detached(async move {
        // Not found is not shown: the row only offers what it knows.
        if let Some(Ok(latest)) = rx.recv().await {
            global::<AgentUpdates>().update(|u| {
                if let Some(e) = u.0.get_mut(&path) {
                    e.latest = Some(latest);
                }
            });
        }
    });
}

#[cfg(any(target_os = "ios", target_arch = "wasm32"))]
fn check_agent_update(_path: String) {}

/// Updates the CLI at `path` the way it was installed, then asks the
/// providers again for their versions. This computer's only.
#[cfg(not(any(target_os = "ios", target_arch = "wasm32")))]
pub(crate) fn update_agent(path: &str) {
    if !on_this_mac() {
        return;
    }
    let mut install = None;
    global::<AgentUpdates>().update(|u| {
        if let Some(e) = u.0.get_mut(path).filter(|e| !e.updating) {
            install = e.install.clone();
            if install.is_some() {
                e.updating = true;
                e.error = None;
            }
        }
    });
    let Some(install) = install else { return };
    let (path, binary) = (path.to_owned(), path.to_owned());
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    std::thread::spawn(move || {
        let _ = tx.send(crate::agent_update::run_update(&install, &binary));
    });
    spawn_detached(async move {
        let result = rx.recv().await.unwrap_or_else(|| Err("stopped".into()));
        global::<AgentUpdates>().update(|u| {
            if let Some(e) = u.0.get_mut(&path) {
                e.updating = false;
                e.error = result.as_ref().err().cloned();
            }
        });
        if result.is_ok() {
            refresh_agents();
        }
    });
}

#[cfg(any(target_os = "ios", target_arch = "wasm32"))]
pub(crate) fn update_agent(_path: &str) {}

/// Asks the providers again, and their diagnostics.
pub(crate) fn refresh_agents() {
    global::<Diagnostics>().update(|d| d.map.clear());
    send(Command::RefreshProviders);
    load_diagnostics();
}

#[derive(Clone, Default)]
struct LabelCatalog(Vec<(String, String)>);

pub(crate) fn labels() -> Vec<(String, String)> {
    global::<LabelCatalog>().with(|l| l.0.clone())
}

pub(crate) fn label_color(name: &str) -> String {
    global::<LabelCatalog>().with(|l| l.0.iter().find(|(n, _)| n == name).map(|(_, c)| c.clone()).unwrap_or_else(|| "violet".into()))
}

pub(crate) fn set_label(workspace_id: &str, name: &str, color: &str, assigned: bool) {
    global::<Workspaces>().update(|w| {
        if let Some(ws) = w.0.iter_mut().find(|w| w.id == workspace_id) {
            ws.labels.retain(|l| l != name);
            if assigned {
                ws.labels.push(name.to_owned());
            }
        }
    });
    send(Command::AssignLabel { workspace: workspace_id.to_owned(), name: name.to_owned(), color: color.to_owned(), assigned });
}

pub(crate) fn update_daemon() {
    send(Command::UpdateDaemon);
    notify(t("Updating the service…").into());
}

pub(crate) fn recover_workspace(id: &str) {
    send(Command::RecoverWorkspace(id.to_owned()));
}

/// A restart is under way: the next connection says it is done.
#[derive(Clone, Copy, Default)]
struct Restarting(bool);

pub(crate) fn restart_daemon() {
    global::<Restarting>().set(Restarting(true));
    send(Command::RestartDaemon);
    notify(t("Restarting the service…").into());
}

pub(crate) fn open_settings() {
    send(Command::GetConfig);
    load_diagnostics();
    global::<Nav>().update(|n| n.screen = Screen::Settings);
    send(Command::WatchDiff(None));
}

pub(crate) fn go_home() {
    global::<Nav>().update(|n| *n = Nav::default());
    send(Command::WatchDiff(None));
}

/// At launch, the workspace (and agent) open last, once the workspaces are
/// known; not on the phone, whose home is its lists.
fn restore_last_open() {
    if crate::ui::kit::MOBILE {
        return;
    }
    let (ws, agent, done) = global::<LastOpen>().with_untracked(|l| (l.workspace.clone(), l.agent.clone(), l.done));
    if done || screen() != Screen::Home {
        return;
    }
    global::<LastOpen>().update(|l| l.done = true);
    let Some(ws) = ws.filter(|id| workspace(id).is_some()) else { return };
    open_workspace(&ws);
    if let Some(agent) = agent.filter(|a| self::agent(a).is_none_or(|a| a.workspace_id.as_deref() == Some(ws.as_str()))) {
        open_tab(Tab::Agent(agent));
    }
}

pub(crate) fn open_workspace(id: &str) {
    global::<LastOpen>().update(|l| {
        l.workspace = Some(id.to_owned());
        l.done = true;
    });
    let tab = workspace_agents(id).into_iter().last().map(|a| Tab::Agent(a.id)).unwrap_or(Tab::Draft);
    global::<Nav>().update(|n| n.screen = Screen::Workspace(id.to_owned()));
    watch_changes(id);
    open_tab(tab);
    if workspace(id).is_some_and(|w| matches!(w.status.as_str(), "attention" | "needs_input")) {
        send(Command::ClearAttention(id.to_owned()));
    }
}

fn watch_changes(workspace_id: &str) {
    let Some(cwd) = workspace(workspace_id).map(|w| w.directory) else { return };
    if global::<FileTree>().with_untracked(|t| t.cwd != cwd) {
        global::<Files>().update(|f| f.0.clear());
    }
    if global::<Git>().with_untracked(|g| g.cwd != cwd) {
        global::<Git>().update(|g| *g = Git { cwd: cwd.clone(), ..Default::default() });
    }
    send(Command::GitStatus(cwd.clone()));
    if global::<Terminals>().with_untracked(|t| t.cwd != cwd) {
        global::<Terminals>().update(|t| *t = Terminals { cwd: cwd.clone(), list: vec![] });
        send(Command::WatchTerminals { cwd: cwd.clone(), workspace_id: workspace_id.to_owned() });
    }
    if global::<Changes>().with_untracked(|c| c.cwd != cwd) {
        global::<Changes>().update(|c| *c = Changes { cwd: cwd.clone(), ..Default::default() });
    }
    send(Command::WatchDiff(Some(cwd)));
}

pub(crate) fn open_tab(tab: Tab) {
    if let Tab::Agent(id) = &tab {
        global::<LastOpen>().update(|l| l.agent = Some(id.clone()));
        save_prefs();
    }
    let agent_id = match &tab {
        Tab::Agent(id) => Some(id.clone()),
        Tab::Draft | Tab::Diff(_) | Tab::File(_) | Tab::Subagent(..) | Tab::CommitDiff(..) => None,
    };
    if let (Tab::CommitDiff(sha, path), Some(cwd)) = (&tab, current_workspace().map(|w| w.directory)) {
        global::<CommitDiffState>().update(|c| c.0 = None);
        send(Command::CommitFileDiff { cwd, sha: sha.clone(), path: path.clone() });
    }
    if let Tab::Subagent(parent, id) = &tab {
        send(Command::SubagentTimeline { parent: parent.clone(), id: id.clone() });
    }
    if matches!(tab, Tab::Diff(_) | Tab::File(_) | Tab::Subagent(..) | Tab::CommitDiff(..)) {
        global::<Nav>().update(|n| n.tab = tab);
        return;
    }
    batch(|| {
        global::<Nav>().update(|n| n.tab = tab.clone());
        global::<Timeline>().update(|t| {
            *t = Timeline { agent_id: agent_id.clone(), ..Default::default() };
            // This device's copy of its last page at once; the host then
            // says what came since.
            if let Some((mut items, older, cursor)) = agent_id.as_deref().and_then(crate::daemon::connection::cached_timeline) {
                crate::sent_pictures::restore(&mut items);
                *t = Timeline { agent_id: agent_id.clone(), items, older, cursor, loaded: true, syncing: true, ..Default::default() };
            }
        });
        if tab == Tab::Draft {
            reset_form(current_workspace().map(|w| w.directory));
        }
    });
    if global::<Timeline>().with_untracked(|t| t.syncing) {
        scroll_timeline_end_after(&[30, 120, 250], false);
    }
    if let Some(id) = agent_id {
        send(Command::Open(id.clone()));
        send(Command::Subagents(id.clone()));
        send(Command::BackgroundWork(id));
    }
}

#[derive(Clone, Default)]
struct SubagentState {
    by_parent: HashMap<String, Vec<crate::daemon::connection::Subagent>>,
    timelines: HashMap<String, Vec<Item>>,
}
#[derive(Clone, Default)]
struct Background(HashMap<String, Vec<crate::daemon::connection::BackgroundItem>>);

/// The agent whose ended sub-agents the user asked to see (the agent's
/// menu): they are listed over the composer only while one runs.
#[derive(Clone, Default)]
struct SubagentsShown(Option<String>);

pub(crate) fn subagents_shown(parent: &str) -> bool {
    global::<SubagentsShown>().with(|s| s.0.as_deref() == Some(parent))
}

pub(crate) fn toggle_subagents(parent: &str) {
    global::<SubagentsShown>().update(|s| s.0 = if s.0.as_deref() == Some(parent) { None } else { Some(parent.to_owned()) });
}

pub(crate) fn subagents(parent: &str) -> Vec<crate::daemon::connection::Subagent> {
    global::<SubagentState>().with(|s| s.by_parent.get(parent).cloned().unwrap_or_default())
}

pub(crate) fn subagent(parent: &str, id: &str) -> Option<crate::daemon::connection::Subagent> {
    subagents(parent).into_iter().find(|s| s.id == id)
}

pub(crate) fn subagent_items(id: &str) -> Option<Vec<Item>> {
    global::<SubagentState>().with(|s| s.timelines.get(id).cloned())
}

pub(crate) fn background(agent: &str) -> Vec<crate::daemon::connection::BackgroundItem> {
    global::<Background>().with(|b| b.0.get(agent).cloned().unwrap_or_default())
}

pub(crate) fn stop_background(agent: &str, item: &str, token: &str) {
    send(Command::StopBackground { agent: agent.to_owned(), item: item.to_owned(), token: token.to_owned() });
}

pub(crate) fn open_new_workspace(project_path: Option<String>) {
    global::<Nav>().update(|n| n.screen = Screen::NewWorkspace);
    send(Command::WatchDiff(None));
    let path = project_path.or_else(|| {
        rows().into_iter().find_map(|r| match r {
            Row::Project(p) if !p.root_path.is_empty() => Some(p.root_path),
            _ => None,
        })
    });
    reset_form(path);
}

// -------------------------------------------------------------------- form

fn default_provider(list: &[Provider], current: &str) -> String {
    if list.iter().any(|p| p.id == current && p.available()) {
        return current.to_owned();
    }
    list.iter()
        .find(|p| p.available() && p.id == "claude")
        .or_else(|| list.iter().find(|p| p.available()))
        .map(|p| p.id.clone())
        .unwrap_or_else(|| "claude".into())
}

/// Fills the provider's defaults (model, its thinking level, mode).
/// The `provider/` prefixes of models a gateway serves from a subscription
/// signed in to another client (Magpie's signed-in agents as providers).
pub(crate) const SUBSCRIPTION_PREFIXES: &[&str] =
    &["claude", "codex", "copilot", "devin", "grok", "cursor", "gemini", "antigravity", "qoder"];

/// Not allowed: a Claude model through a subscription. Claude's own outside
/// Claude Code (`claude/…` through a gateway: Anthropic counts it as another
/// client's use of the subscription), or another client's (`copilot/claude-…`,
/// `cursor/claude-…`: that account answers for it). A Claude model on an API
/// key, through any gateway, stays allowed.
pub(crate) fn forbidden_model(model_id: &str, label: &str) -> bool {
    let Some((prefix, rest)) = model_id.split_once('/') else { return false };
    if !SUBSCRIPTION_PREFIXES.contains(&prefix) {
        return false;
    }
    let claude = |s: &str| {
        let s = s.to_lowercase();
        s.contains("claude") || s.split(|c: char| !c.is_alphanumeric()).any(|w| matches!(w, "opus" | "sonnet" | "haiku" | "fable"))
    };
    prefix == "claude" || claude(rest) || claude(label)
}

/// Whether `provider`'s `model` is refused ([`forbidden_model`]); says so.
fn refuse_model(provider_id: &str, model: &str) -> bool {
    let label = provider(provider_id).and_then(|p| p.model(model).map(|m| m.label.clone())).unwrap_or_default();
    if forbidden_model(model, &label) {
        notify(t("Claude models run only in Claude Code, on your own Claude account: not through another client's subscription").into());
        return true;
    }
    false
}

fn apply_provider_defaults(f: &mut Form) {
    let Some(p) = provider(&f.provider) else { return };
    // A refused default gives way to the first allowed model.
    let model = p
        .default_model()
        .filter(|m| !forbidden_model(&m.id, &m.label))
        .or_else(|| p.models.iter().find(|m| !forbidden_model(&m.id, &m.label)))
        .cloned();
    f.model = model.as_ref().map(|m| m.id.clone());
    f.thinking = model.and_then(|m| m.default_thinking.or_else(|| m.thinking.first().map(|t| t.id.clone())));
    f.mode = p.default_mode.clone().or_else(|| p.modes.first().map(|m| m.id.clone()));
}

fn reset_form(path: Option<String>) {
    let list = providers();
    // A prompt link's text starts this new draft.
    let prompt = take_prompt_link_text().unwrap_or_default();
    global::<Form>().update(|f| {
        let provider = default_provider(&list, &f.provider);
        let keep = f.provider == provider && f.model.is_some();
        let (model, mode, thinking) = (f.model.clone(), f.mode.clone(), f.thinking.clone());
        *f = Form { path: path.clone().unwrap_or_default(), provider, prompt, ..Default::default() };
        if keep {
            (f.model, f.mode, f.thinking) = (model, mode, thinking);
        } else {
            apply_provider_defaults(f);
        }
    });
    refresh_draft_features();
}

pub(crate) fn edit_form(change: impl FnOnce(&mut Form)) {
    global::<Form>().update(change);
}

fn refresh_draft_features() {
    let f = form();
    if f.provider.is_empty() || f.path.is_empty() {
        return;
    }
    send(Command::ProviderFeatures { provider: f.provider, cwd: f.path, model: f.model, mode: f.mode });
}

/// The features the composer shows: the open agent's, or the draft's.
pub(crate) fn features(live: bool) -> Vec<crate::daemon::model::Feature> {
    if live {
        return selected_agent().map(|a| a.features).unwrap_or_default();
    }
    form().features
}

pub(crate) fn set_feature(live: bool, id: &str, value: serde_json::Value) {
    let apply = |list: &mut Vec<crate::daemon::model::Feature>| {
        if let Some(f) = list.iter_mut().find(|f| f.id == id) {
            match (&mut f.value, &value) {
                (Ok(on), serde_json::Value::Bool(b)) => *on = *b,
                (Err((v, _)), serde_json::Value::String(s)) => *v = Some(s.clone()),
                _ => {}
            }
        }
    };
    if live {
        let Some(agent) = selected_id() else { return };
        global::<Agents>().update(|a| {
            if let Some(ag) = a.0.iter_mut().find(|x| x.id == agent) {
                apply(&mut ag.features);
            }
        });
        send(Command::SetFeature { agent, id: id.to_owned(), value });
    } else {
        edit_form(|f| apply(&mut f.features));
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Control {
    Provider(String),
    Model(String),
    Mode(String),
    Thinking(String),
}

/// The controls the composer shows: the open agent's, or the form's.
#[derive(Clone, Debug, PartialEq, Default)]
pub(crate) struct Controls {
    pub(crate) provider: String,
    pub(crate) model: Option<String>,
    pub(crate) mode: Option<String>,
    pub(crate) thinking: Option<String>,
    /// Modes of the live agent (they can differ from the provider's list).
    pub(crate) modes: Vec<crate::daemon::model::Opt>,
    pub(crate) live: bool,
}

pub(crate) fn controls(live: bool) -> Controls {
    if live {
        if let Some(a) = selected_agent() {
            let modes = if a.modes.is_empty() {
                provider(&a.provider).map(|p| p.modes).unwrap_or_default()
            } else {
                a.modes.clone()
            };
            return Controls {
                provider: a.provider,
                model: a.model,
                mode: a.mode_id,
                thinking: a.thinking_id,
                modes,
                live: true,
            };
        }
    }
    let f = form();
    Controls {
        modes: provider(&f.provider).map(|p| p.modes).unwrap_or_default(),
        provider: f.provider,
        model: f.model,
        mode: f.mode,
        thinking: f.thinking,
        live: false,
    }
}

pub(crate) fn set_control(live: bool, change: Control) {
    if let Control::Model(m) = &change {
        let provider_id = if live { selected_agent().map(|a| a.provider).unwrap_or_default() } else { form().provider };
        if refuse_model(&provider_id, m) {
            return;
        }
    }
    let refresh = !live && matches!(change, Control::Provider(_) | Control::Model(_));
    if live {
        let Some(agent_id) = selected_id() else { return };
        // Shown at once; the daemon's agent_update confirms it.
        global::<Agents>().update(|list| {
            if let Some(a) = list.0.iter_mut().find(|a| a.id == agent_id) {
                match &change {
                    Control::Model(m) => a.model = Some(m.clone()),
                    Control::Mode(m) => a.mode_id = Some(m.clone()),
                    Control::Thinking(t) => a.thinking_id = Some(t.clone()),
                    Control::Provider(_) => {}
                }
            }
        });
        match change {
            Control::Model(model) => send(Command::SetModel { agent_id, model }),
            Control::Mode(mode) => send(Command::SetMode { agent_id, mode }),
            Control::Thinking(t) => send(Command::SetThinking { agent_id, thinking: Some(t) }),
            Control::Provider(_) => {}
        }
        return;
    }
    edit_form(|f| match change {
        Control::Provider(p) => {
            f.provider = p;
            apply_provider_defaults(f);
        }
        Control::Model(m) => {
            // The agent's own list knows its levels (a model the service's
            // list lacks too); the service's otherwise.
            let local = local_models(&f.provider).and_then(|l| l.into_iter().find(|x| x.id == m));
            let model = provider(&f.provider).and_then(|p| p.model(&m).cloned());
            f.model = Some(m);
            if let Some(local) = local.filter(|l| !l.thinking.is_empty()) {
                if !local.thinking.iter().any(|(id, _)| Some(id) == f.thinking.as_ref()) {
                    f.thinking = local.default_thinking.or_else(|| local.thinking.first().map(|(id, _)| id.clone()));
                }
            } else if let Some(model) = model {
                if !model.thinking.iter().any(|t| Some(&t.id) == f.thinking.as_ref()) {
                    f.thinking = model.default_thinking.or_else(|| model.thinking.first().map(|t| t.id.clone()));
                }
            }
        }
        Control::Mode(m) => f.mode = Some(m),
        Control::Thinking(t) => f.thinking = Some(t),
    });
    if refresh {
        refresh_draft_features();
    }
}

/// Starts the form's agent: in the open workspace (draft tab), or in a new
/// workspace (new-workspace screen).
pub(crate) fn submit_form() {
    let f = form();
    if f.busy || f.path.trim().is_empty() || f.provider.is_empty() {
        return;
    }
    if f.model.as_deref().is_some_and(|m| refuse_model(&f.provider, m)) {
        return;
    }
    if uploading() {
        notify(t("Wait for the files to finish uploading").into());
        return;
    }
    let workspace_id = match screen() {
        Screen::Workspace(id) => Some(id),
        _ => None,
    };
    edit_form(|f| {
        f.busy = true;
        f.error = None;
    });
    // The first message's pictures, kept by its words: the host makes its id.
    let images = take_images();
    let thumbs: Vec<String> = images.iter().map(|i| format!("data:{};base64,{}", i.mime, i.data)).collect();
    crate::sent_pictures::keep_for_text(&f.prompt, &thumbs);
    send(Command::CreateAgent(CreateAgent {
        workspace_id,
        path: f.path.trim().to_owned(),
        worktree: f.worktree,
        base_branch: f.base_branch.clone(),
        branch_name: Some(f.branch_name.clone()),
        provider: f.provider,
        model: f.model,
        mode: f.mode,
        thinking: f.thinking,
        prompt: f.prompt,
        images,
        attachments: take_files(),
        context: f.context.map(|(c, _)| c),
        features: f
            .features
            .iter()
            .map(|ft| {
                (ft.id.clone(), match &ft.value {
                    Ok(on) => serde_json::Value::Bool(*on),
                    Err((v, _)) => v.clone().map(serde_json::Value::String).unwrap_or(serde_json::Value::Null),
                })
            })
            .collect(),
    }));
}

// ---------------------------------------------------------- agent actions

pub(crate) fn send_message(text: &str) -> bool {
    let text = text.trim();
    let Some(agent) = selected_agent() else { return false };
    if text.is_empty() && pending_images().is_empty() && pending_files().is_empty() {
        return false;
    }
    if uploading() {
        notify(t("Wait for the files to finish uploading").into());
        return false;
    }
    // A message sent sets a waiting queue going again, after it.
    global::<QueueHeld>().update(|h| {
        h.0.remove(&agent.id);
    });
    let behavior = settings().send_behavior;
    // A pending permission must be answered first; queueing would wait forever.
    if agent.is_running() && behavior == SendBehavior::Queue && agent.pending.is_empty() {
        global::<Queues>().update(|q| q.0.entry(agent.id.clone()).or_default().push(text.to_owned()));
        return true;
    }
    deliver(&agent.id, text, agent.is_running().then_some(match behavior {
        SendBehavior::Steer => "steer",
        SendBehavior::Interrupt | SendBehavior::Queue => "interrupt",
    }));
    true
}

fn deliver(agent_id: &str, text: &str, behavior: Option<&'static str>) {
    let mine = image_key() == agent_id;
    let images = if mine { take_images() } else { vec![] };
    let attachments = if mine { take_files() } else { vec![] };
    let mut shown = text.to_owned();
    // A file the text does not name where its chip stood (a message
    // queued before, a draft kept as text) is named after it.
    for a in &attachments {
        if let Some(name) = a.get("fileName").and_then(serde_json::Value::as_str)
            && !text.contains(&format!("[{name}]"))
        {
            shown.push_str(&format!("\n📎 {name}"));
        }
    }
    // Shown at once under the id the host echoes it back with, which then
    // replaces nothing and adds nothing (the echo has the text alone).
    let message_id = uuid::Uuid::new_v4().to_string();
    let thumbs: Vec<String> = images.iter().map(|i| format!("data:{};base64,{}", i.mime, i.data)).collect();
    // On disk too: the host's history has the text alone.
    crate::sent_pictures::keep(&message_id, text, &thumbs);
    let mut shown_here = false;
    global::<Timeline>().update(|t| {
        if t.agent_id.as_deref() == Some(agent_id) {
            t.items.push(Item::User { text: shown.trim().to_owned(), message_id: Some(message_id.clone()), images: thumbs });
            shown_here = true;
        }
    });
    if shown_here {
        scroll_timeline_to_end();
    }
    send(Command::Send { agent_id: agent_id.to_owned(), text: text.to_owned(), behavior, images, attachments, message_id });
}

/// A system notification when an agent starts needing you while you look
/// elsewhere; the press opens it.
fn notify_attention(next: &Agent) {
    let before = agent(&next.id);
    push_attention(before.as_ref(), next);
    let needed = |a: &Agent| a.requires_attention || !a.pending.is_empty();
    let finished = before.as_ref().is_some_and(|b| b.is_running()) && !next.is_running();
    let newly = needed(next) && !before.as_ref().is_some_and(needed);
    if !(newly || finished) || selected_id().as_deref() == Some(next.id.as_str()) && lifecycle_active() {
        return;
    }
    let body = if !next.pending.is_empty() {
        format!("Waiting for your approval: {}", next.pending[0].title)
    } else if finished {
        "Finished its turn".to_owned()
    } else {
        "Needs your attention".to_owned()
    };
    // The computer's push says it to this phone already.
    if cfg!(target_os = "ios") && push_served() {
        return;
    }
    let n = nsh_core::notifications::Notification::new(format!("agent:{}", next.id), next.title.clone(), body);
    let _ = show_notification(n);
}

/// What the phone knows of its push notifications: Apple's token for it
/// (once Apple gave one), and whether the computer in use sends them (it
/// has a gateway).
#[derive(Clone, Default)]
struct PushState {
    token: Option<String>,
    served: bool,
}

/// Whether the computer in use sends this phone push notifications now.
pub(crate) fn push_served() -> bool {
    global::<PushState>().with(|p| p.served && p.token.as_ref().is_some_and(|t| !t.is_empty())) && settings().push.0
}

/// Whether the phone wants push notifications, and whether they hide what
/// they are about: told to the computer at once.
pub(crate) fn set_push(on: bool, hide: bool) {
    edit_settings(|s| {
        s.push = PushWanted(on);
        s.push_hide = hide;
    });
    push_register_now();
}

/// Tells the computer in use what this phone wants of push notifications
/// (`docs/push.md`), once Apple gave the phone a token.
fn push_register_now() {
    let Some(token) = global::<PushState>().with_untracked(|p| p.token.clone()) else { return };
    let mut s = global::<Settings>().with_untracked(Clone::clone);
    if s.push_device.is_empty() {
        s.push_device = uuid::Uuid::new_v4().simple().to_string();
        let id = s.push_device.clone();
        edit_settings(|s| s.push_device = id);
    }
    #[cfg(target_os = "ios")]
    let env = crate::platform::ios_push::environment();
    #[cfg(not(target_os = "ios"))]
    let env = "production";
    send(Command::PushRegister {
        device: s.push_device,
        token: if s.push.0 { token } else { String::new() },
        env: env.to_owned(),
        lang: if crate::i18n::is_zh() { "zh" } else { "en" }.to_owned(),
        hide: s.push_hide,
    });
}

/// The phone's push notifications (`platform::ios_push`): a press opens
/// its agent, Apple's token goes to the computer, and the agent on screen
/// is not notified about over itself.
#[cfg(target_os = "ios")]
pub(crate) fn start_push() {
    use crate::platform::ios_push;
    ios_push::install();
    if let Some(mut presses) = ios_push::presses() {
        spawn_detached(async move {
            while let Some(id) = presses.recv().await {
                if let Some(agent_id) = id.strip_prefix("agent:") {
                    // The list may not be here yet (the press started the
                    // app): kept until the agent is known.
                    global::<PressedAgent>().update(|p| p.0 = Some(agent_id.to_owned()));
                    open_pressed_agent();
                }
            }
        });
    }
    if let Some(mut tokens) = ios_push::tokens() {
        spawn_detached(async move {
            while let Some(token) = tokens.recv().await {
                global::<PushState>().update(|p| p.token = Some(token));
                push_register_now();
            }
        });
    }
    // Apple's token once the user allows notifications (asked on the
    // first launch).
    spawn_detached(async move {
        if request_notification_permission().await == nsh_core::notifications::NotificationPermission::Granted {
            ios_push::register();
        }
    });
    effect(|| {
        let on = selected_id().filter(|_| lifecycle_active() && matches!(screen(), Screen::Workspace(_)));
        ios_push::set_on_screen(on);
    });
}

/// The agent of a pressed notification, until it can be opened.
#[derive(Clone, Default)]
struct PressedAgent(Option<String>);

/// Opens the agent a notification's press named, once it is known here.
fn open_pressed_agent() {
    let Some(id) = global::<PressedAgent>().with_untracked(|p| p.0.clone()) else { return };
    if let Some(a) = agent(&id) {
        global::<PressedAgent>().update(|p| p.0 = None);
        open_session(&a);
    }
}

/// The paired phones that asked for push notifications hear the same
/// (`docs/push.md`): the computer whose service this is tells them, so
/// only it, and not while the agent is on screen here. A sub-agent's turns
/// are its parent's business.
fn push_attention(before: Option<&Agent>, next: &Agent) {
    #[cfg(not(any(target_arch = "wasm32", target_os = "ios")))]
    {
        use crate::daemon::{push, push_gateway};
        if !is_desktop() || !on_this_mac() || next.parent_id.is_some() {
            return;
        }
        let Some(why) = push::reason(before, next) else { return };
        if selected_id().as_deref() == Some(next.id.as_str()) && lifecycle_active() {
            return;
        }
        let Some(record) = push_gateway::record() else { return };
        let space = current_space_home().map(|h| h.to_string_lossy().into_owned());
        for wants in record.wanting(space.as_deref()) {
            push_gateway::send(push::agent_push(&record.id, next, why, &wants));
        }
    }
    #[cfg(any(target_arch = "wasm32", target_os = "ios"))]
    let _ = (before, next);
}

/// The mark each plan limit was last seen at (`push::limit_mark`), by its
/// key: a push says a limit once as it passes 80% and 95%.
#[cfg_attr(any(target_arch = "wasm32", target_os = "ios"), allow(dead_code))]
#[derive(Clone, Default)]
struct LimitMarks(HashMap<String, u32>);

/// The paired phones hear when a plan limit of this computer passes 80%
/// or 95%, with when it runs out at the pace so far.
fn push_limits(list: &[crate::daemon::connection::ProviderUsage]) {
    #[cfg(not(any(target_arch = "wasm32", target_os = "ios")))]
    {
        use crate::daemon::{push, push_gateway};
        use crate::usage::{self, limits};
        if !is_desktop() || !on_this_mac() {
            return;
        }
        let history = untracked(limit_history);
        let now = usage::now();
        let mut news: Vec<(String, u32, Option<String>)> = Vec::new();
        global::<LimitMarks>().update(|marks| {
            for p in list {
                for w in &p.windows {
                    let Some(pct) = w.used else { continue };
                    let key = limit_key(p, w);
                    let mark = push::limit_mark(pct);
                    if push::limit_news(marks.0.insert(key.clone(), mark), mark) {
                        let resets = w.resets_at.as_deref().and_then(usage::parse::rfc3339);
                        let points = history.windows.get(&key).map(Vec::as_slice).unwrap_or_default();
                        let out = limits::forecast(limits::current_window(points, resets), now, pct, resets, limits::window_length(&w.id, &w.label));
                        news.push((format!("{} · {}", p.name, w.label), pct.round() as u32, out.map(|at| usage::time_label(at, &usage::local_offset))));
                    }
                }
            }
        });
        if news.is_empty() {
            return;
        }
        let Some(record) = push_gateway::record() else { return };
        let space = current_space_home().map(|h| h.to_string_lossy().into_owned());
        for wants in record.wanting(space.as_deref()) {
            for (label, percent, ends) in &news {
                push_gateway::send(push::usage_push(&record.id, label, *percent, ends.as_deref(), &wants));
            }
        }
    }
    #[cfg(any(target_arch = "wasm32", target_os = "ios"))]
    let _ = list;
}

/// While a paired phone wants push notifications, the plan limits are read
/// every twenty minutes, so one near its end is said without the usage
/// page open.
#[cfg(not(any(target_arch = "wasm32", target_os = "ios")))]
pub(crate) fn watch_limits_for_push() {
    fn again() {
        set_timeout(std::time::Duration::from_secs(20 * 60), || {
            let wanted = crate::daemon::push_gateway::record().is_some_and(|r| !r.devices.is_empty());
            if wanted && on_this_mac() && matches!(conn(), Conn::Connected { .. }) {
                send(Command::Usage);
            }
            again();
        });
    }
    Owner::detached().run(again);
}

/// An action a paired device pressed on a notification: the approval is
/// answered, the reply sent, as from here.
pub(crate) fn push_act(act: crate::daemon::push::Act) {
    use crate::daemon::push::Act;
    match act {
        Act::Permission { agent: agent_id, permission, allow } => {
            // Answered already, here or on another device: nothing to do.
            if agent(&agent_id).is_some_and(|a| a.pending.iter().any(|p| p.id == permission)) {
                global::<Agents>().update(|a| {
                    if let Some(agent) = a.0.iter_mut().find(|a| a.id == agent_id) {
                        agent.pending.retain(|p| p.id != permission);
                    }
                });
                send(Command::Permit { agent_id, permission_id: permission, allow, action_id: None, updated_input: None });
            }
        }
        Act::Reply { agent: agent_id, text } => {
            if agent(&agent_id).is_some() {
                deliver(&agent_id, text.trim(), None);
            }
        }
    }
}

fn lifecycle_active() -> bool {
    matches!(lifecycle(), nsh_core::element::Lifecycle::Active)
}

/// Opens the agent a pressed notification names.
#[cfg_attr(target_os = "ios", allow(dead_code))]
pub(crate) fn watch_notification_presses() {
    spawn(async move {
        loop {
            let id = next_notification_press().await;
            if let Some(agent_id) = id.strip_prefix("agent:")
                && let Some(a) = agent(agent_id)
            {
                open_session(&a);
            }
        }
    });
}

/// Sends the next queued message once its agent stops running.
fn flush_queue(agent: &Agent) {
    if agent.is_running() || global::<QueueHeld>().with_untracked(|h| h.0.contains(&agent.id)) {
        return;
    }
    let next = global::<Queues>().with_untracked(|q| q.0.get(&agent.id).and_then(|l| l.first().cloned()));
    if let Some(text) = next {
        global::<Queues>().update(|q| {
            if let Some(list) = q.0.get_mut(&agent.id) {
                list.remove(0);
            }
        });
        deliver(&agent.id, &text, None);
    }
}

pub(crate) fn answer_permission(permission_id: &str, allow: bool, action_id: Option<String>) {
    answer_permission_with(permission_id, allow, action_id, None);
}

pub(crate) fn answer_permission_with(
    permission_id: &str,
    allow: bool,
    action_id: Option<String>,
    updated_input: Option<serde_json::Value>,
) {
    let Some(agent_id) = selected_id() else { return };
    global::<Agents>().update(|a| {
        if let Some(agent) = a.0.iter_mut().find(|a| a.id == agent_id) {
            agent.pending.retain(|p| p.id != permission_id);
        }
    });
    send(Command::Permit { agent_id, permission_id: permission_id.to_owned(), allow, action_id, updated_input });
}

pub(crate) fn stop_selected() {
    if let Some(id) = selected_id() {
        send(Command::Cancel(id));
    }
}

pub(crate) fn archive_agent(id: &str) {
    send(Command::Archive(id.to_owned()));
}

#[derive(Clone, Default)]
struct Recent(Option<Vec<crate::daemon::connection::RecentSession>>);

pub(crate) fn recent_sessions() -> Option<Vec<crate::daemon::connection::RecentSession>> {
    global::<Recent>().with(|r| r.0.clone())
}

pub(crate) fn search_sessions(query: &str) {
    send(Command::RecentSessions { query: query.to_owned() });
}

pub(crate) fn import_session(s: &crate::daemon::connection::RecentSession) {
    send(Command::ImportSession { provider: s.provider.clone(), handle: s.handle.clone(), cwd: s.cwd.clone() });
    notify(t("Importing session…").into());
}

/// The sessions being imported together, one after another (each import
/// loads a conversation on the host: all at once would start them all at
/// once).
#[derive(Clone, Default)]
struct Importing {
    left: Vec<crate::daemon::connection::RecentSession>,
    total: usize,
    failed: usize,
}

/// Imports `sessions`, the first now and each next one as the one before
/// it ends. The host puts each in the workspace of its folder, making the
/// workspace (and so its project in the lists) where there is none.
pub(crate) fn import_sessions(sessions: Vec<crate::daemon::connection::RecentSession>) {
    if sessions.is_empty() || importing() {
        return;
    }
    notify(tf("Importing {} sessions…", &[&sessions.len().to_string()]));
    global::<Importing>().set(Importing { total: sessions.len(), left: sessions, failed: 0 });
    import_next();
}

/// Whether sessions are being imported together.
pub(crate) fn importing() -> bool {
    global::<Importing>().with_untracked(|i| i.total > 0)
}

fn import_next() {
    let next = global::<Importing>().with_untracked(|i| i.left.first().cloned());
    match next {
        Some(s) => {
            global::<Importing>().update(|i| {
                i.left.remove(0);
            });
            send(Command::ImportSession { provider: s.provider, handle: s.handle, cwd: s.cwd });
        }
        None => {
            let (total, failed) = global::<Importing>().with_untracked(|i| (i.total, i.failed));
            global::<Importing>().set(Importing::default());
            let done = (total - failed).to_string();
            notify(if failed == 0 { tf("Imported {} sessions", &[&done]) } else { tf("Imported {} sessions; {} could not be imported", &[&done, &failed.to_string()]) });
        }
    }
}

pub(crate) fn run_script(name: &str, start: bool) {
    if let Some(w) = current_workspace() {
        send(Command::Script { workspace: w.id, name: name.to_owned(), start });
    }
}

/// Find in the open conversation (⌘F).
#[derive(Clone, Debug, PartialEq, Default)]
pub(crate) struct Find {
    pub(crate) open: bool,
    pub(crate) query: String,
    pub(crate) current: usize,
}

thread_local! {
    static TIMELINE_LIST: RefCell<Option<ElementRef>> = const { RefCell::new(None) };
}

pub(crate) fn set_timeline_list(list: ElementRef) {
    TIMELINE_LIST.with_borrow_mut(|l| *l = Some(list));
}

/// Brings item `index` into view: its row, or its folded group's.
pub(crate) fn scroll_timeline_to(index: usize) {
    let Some(index) = crate::fold::row_of(&display_rows(), index) else { return };
    TIMELINE_LIST.with_borrow(|l| {
        if let Some(list) = l {
            list.scroll_to_index(index as u32, nsh_core::geometry::Percentage(0.3), px(0.0), true);
        }
    });
}

/// Brings the conversation to its end (a message just sent), once the
/// new row is laid out.
pub(crate) fn scroll_timeline_to_end() {
    scroll_timeline_end_after(&[60], true);
}

/// Jumps to the end at each delay (milliseconds): the rows' first layout
/// uses estimated heights, so a conversation just opened is brought to its
/// end again once its messages have measured themselves.
fn scroll_timeline_end_after(delays: &'static [u64], animated: bool) {
    spawn(async move {
        for d in delays {
            nsh_core::element::timer(std::time::Duration::from_millis(*d)).await;
            TIMELINE_LIST.with_borrow(|l| {
                if let Some(list) = l {
                    list.scroll_to_end(animated);
                }
            });
        }
    });
}

pub(crate) fn find() -> Find {
    global::<Find>().with(Clone::clone)
}

pub(crate) fn toggle_find() {
    global::<Find>().update(|f| f.open = !f.open);
}

/// The rows matching the query.
pub(crate) fn find_matches() -> Vec<usize> {
    let q = global::<Find>().with(|f| f.query.to_lowercase());
    if q.trim().is_empty() {
        return vec![];
    }
    global::<Timeline>().with(|t| {
        t.items
            .iter()
            .enumerate()
            .filter(|(_, i)| match i {
                Item::User { text, .. } | Item::Assistant { text } => text.to_lowercase().contains(&q),
                _ => false,
            })
            .map(|(n, _)| n)
            .collect()
    })
}

pub(crate) fn set_find_query(q: &str) {
    global::<Find>().update(|f| {
        f.query = q.to_owned();
        f.current = 0;
    });
    if let Some(&first) = find_matches().first() {
        scroll_timeline_to(first);
    }
}

pub(crate) fn find_step(forward: bool) {
    let matches = find_matches();
    if matches.is_empty() {
        return;
    }
    let mut cur = global::<Find>().with_untracked(|f| f.current);
    cur = if forward { (cur + 1) % matches.len() } else { (cur + matches.len() - 1) % matches.len() };
    global::<Find>().update(|f| f.current = cur);
    scroll_timeline_to(matches[cur]);
}

/// The user messages of the conversation, for the outline: (row, text).
pub(crate) fn prompts() -> Vec<(usize, String)> {
    global::<Timeline>().with(|t| {
        t.items
            .iter()
            .enumerate()
            .filter_map(|(n, i)| match i {
                Item::User { text, .. } => Some((n, text.lines().next().unwrap_or_default().to_owned())),
                _ => None,
            })
            .collect()
    })
}

/// The conversation's turns, for the rail beside it: each user message's
/// row and first line, and how the reply to it begins.
pub(crate) fn turns() -> Vec<(usize, String, String)> {
    global::<Timeline>().with(|t| {
        let mut out: Vec<(usize, String, String)> = Vec::new();
        for (n, item) in t.items.iter().enumerate() {
            match item {
                Item::User { text, .. } => out.push((n, text.lines().next().unwrap_or_default().to_owned(), String::new())),
                Item::Assistant { text } => {
                    if let Some(last) = out.last_mut().filter(|l| l.2.is_empty()) {
                        // Its start, as plain words: no Markdown marks.
                        let plain: String = text.chars().filter(|c| !matches!(c, '#' | '*' | '`' | '>')).take(220).collect();
                        last.2 = plain.split_whitespace().collect::<Vec<_>>().join(" ");
                    }
                }
                _ => {}
            }
        }
        out
    })
}

pub(crate) fn rewind(message_id: &str, mode: &'static str) {
    if let Some(agent) = selected_id() {
        send(Command::Rewind { agent, message_id: message_id.to_owned(), mode });
    }
}

pub(crate) fn fork_selected() {
    if let Some(agent) = selected_id() {
        global::<Handoff>().update(|h| h.0 = None);
        send(Command::Fork(agent));
    }
}

/// A fork that continues in another agent: the agent forked, and the
/// provider and model the new agent's draft starts on.
#[derive(Clone, Default)]
struct Handoff(Option<(String, String, String)>);

/// Continues the open conversation in another agent: forks it, then opens
/// the new agent's draft on `provider` (and its `model`, unless empty) with
/// the conversation attached. A model of another vendor so runs in that
/// vendor's own agent, signed in as that agent is, rather than inside this
/// one.
pub(crate) fn continue_in(provider: &str, model: &str) {
    let Some(agent) = selected_id() else { return };
    if refuse_model(provider, model) {
        return;
    }
    global::<Handoff>().update(|h| h.0 = Some((agent.clone(), provider.to_owned(), model.to_owned())));
    send(Command::Fork(agent));
}

pub(crate) fn rename_agent(id: &str, title: &str) {
    send(Command::Rename { agent_id: id.to_owned(), title: title.to_owned() });
}

pub(crate) fn archive_workspace(id: &str) {
    if screen() == Screen::Workspace(id.to_owned()) {
        go_home();
    }
    let name = workspace(id).map(|w| w.name).unwrap_or_else(|| "Workspace".into());
    send(Command::ArchiveWorkspace(id.to_owned()));
    notify_with(format!("“{name}” archived"), Some(NoticeAction::UndoArchive(id.to_owned())));
}

pub(crate) fn pin_workspace(id: &str, pinned: bool) {
    global::<Workspaces>().update(|w| {
        if let Some(ws) = w.0.iter_mut().find(|w| w.id == id) {
            ws.pinned = pinned;
        }
    });
    send(Command::PinWorkspace { workspace_id: id.to_owned(), pinned });
}

pub(crate) fn rename_workspace(id: &str, title: &str) {
    send(Command::RenameWorkspace { workspace_id: id.to_owned(), title: title.to_owned() });
}

/// The notice's number: another for each notice shown.
pub(crate) fn notice_id() -> Option<u64> {
    global::<Notice>().with(|n| n.0.as_ref().map(|(id, _, _)| *id))
}

/// Puts notice `id` away, if it is still the one shown.
pub(crate) fn dismiss_notice_numbered(id: u64) {
    global::<Notice>().update(|n| {
        if n.0.as_ref().is_some_and(|(cur, _, _)| *cur == id) {
            n.0 = None;
        }
    });
}

pub(crate) fn dismiss_notice() {
    global::<Notice>().update(|n| n.0 = None);
}

pub(crate) fn notify(message: String) {
    notify_with(message, None);
}

pub(crate) fn notify_with(message: String, action: Option<NoticeAction>) {
    // Never the same twice: a timer left from a notice long gone must not
    // take a later one away.
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    global::<Notice>().update(|n| n.0 = Some((id, message, action)));
    set_timeout(std::time::Duration::from_secs(8), move || {
        global::<Notice>().update(|n| {
            if n.0.as_ref().is_some_and(|(cur, _, _)| *cur == id) {
                n.0 = None;
            }
        });
    });
}

// ------------------------------------------------------------------ events

/// Applies one daemon event. Called inside `batch` by the event pump.
pub(crate) fn apply(event: Event) {
    match event {
        Event::Connecting { url } => global::<ConnState>().update(|c| {
            c.1 = url.clone();
            if !matches!(c.0, Conn::Down(_)) {
                c.0 = Conn::Connecting(url);
            }
        }),
        Event::Connected { hostname, version } => {
            // This computer may send no push notifications: it says when
            // asked, and is asked each time the phone reaches one.
            global::<PushState>().update(|p| p.served = false);
            push_register_now();
            // A saved host with no name of its own takes the computer's.
            if !hostname.trim().is_empty() {
                if let Some(h) = current_host().filter(|h| !h.named()) {
                    rename_host(&h.id, &hostname);
                }
            }
            global::<ConnState>().update(|c| {
                c.0 = Conn::Connected { hostname, version, url: c.1.clone() };
            });
            if global::<Restarting>().with_untracked(|r| r.0) {
                global::<Restarting>().set(Restarting(false));
                notify(t("The service restarted").into());
            }
        }
        Event::Disconnected { reason } => {
            // The answers to an import under way are lost with the session:
            // what is left of it is given up, not to wait on them for ever.
            if importing() {
                global::<Importing>().set(Importing::default());
                notify(t("The import stopped: the connection was lost").to_owned());
            }
            global::<ConnState>().update(|c| c.0 = Conn::Down(reason));
            auto_start_service();
        }
        Event::ProjectsReset(list) => global::<Projects>().update(|p| p.0 = list),
        Event::ProjectUpsert(project) => global::<Projects>().update(|p| {
            p.0.retain(|x| x.id != project.id);
            p.0.push(project);
        }),
        Event::ProjectRemove(id) => global::<Projects>().update(|p| p.0.retain(|x| x.id != id)),
        Event::WorkspacesReset(list) => {
            global::<Workspaces>().update(|w| w.0 = list);
            open_when_listed();
            restore_last_open();
            prompt_links_known(|p| p.workspaces_known = true);
        }
        Event::WorkspaceUpsert(ws) => {
            global::<Workspaces>().update(|w| {
                w.0.retain(|x| x.id != ws.id);
                w.0.push(ws);
            });
            open_when_listed();
        }
        Event::WorkspaceRemove(id) => {
            global::<Workspaces>().update(|w| w.0.retain(|x| x.id != id));
            if screen() == Screen::Workspace(id) {
                go_home();
            }
        }
        Event::Providers(list) => {
            // The agents may have learned new models since: read their lists again.
            load_local_models();
            let first = global::<Providers>().with_untracked(|p| p.0.is_empty());
            // An instance just added gets its diagnostic too.
            let (wanted, known) = global::<Diagnostics>().with_untracked(|d| (d.wanted, d.map.keys().cloned().collect::<Vec<_>>()));
            if wanted {
                for p in list.iter().filter(|p| !known.contains(&p.id)) {
                    send(Command::ProviderDiagnostic(p.id.clone()));
                }
            }
            global::<Providers>().update(|p| p.0 = list);
            use_bundled_clis();
            if first {
                // The first providers fill the form's defaults; what is
                // written in it (a prompt link's text) stays.
                let Form { path, prompt, .. } = form();
                reset_form(Some(path));
                if !prompt.is_empty() && form().prompt.is_empty() {
                    edit_form(|f| f.prompt = prompt);
                }
            }
            prompt_links_known(|p| p.providers_known = true);
        }
        // One of the sessions imported together: the next, without
        // opening each as it comes.
        Event::Created(_) if importing() => import_next(),
        Event::CreateFailed(error) if importing() => {
            log::warn!("import: {error}");
            global::<Importing>().update(|i| i.failed += 1);
            import_next();
        }
        Event::Created(agent) => {
            // "Importing session…" has done its work.
            if global::<Notice>().with_untracked(|n| n.0.as_ref().is_some_and(|(_, m, _)| m == t("Importing session…"))) {
                dismiss_notice();
            }
            edit_form(|f| {
                f.busy = false;
                f.prompt.clear();
            });
            if let Some(ws) = agent.workspace_id.clone() {
                global::<Nav>().update(|n| n.screen = Screen::Workspace(ws.clone()));
                watch_changes(&ws);
                open_tab(Tab::Agent(agent.id));
            } else {
                notify(tf("“{}” is ready", &[&agent.title]));
            }
        }
        Event::CreateFailed(error) => edit_form(|f| {
            f.busy = false;
            f.error = Some(error);
        }),
        Event::AgentsReset(list) => {
            global::<Agents>().update(|a| a.0 = list.into_iter().filter(|a| !a.archived).collect());
            // A pressed notification's agent, now that it is listed.
            open_pressed_agent();
        }
        Event::AgentUpsert(agent) => {
            let archived = agent.archived;
            let id = agent.id.clone();
            flush_queue(&agent);
            notify_attention(&agent);
            global::<Agents>().update(|a| {
                a.0.retain(|x| x.id != agent.id);
                if !agent.archived {
                    a.0.push(agent);
                }
            });
            if archived && tab() == Tab::Agent(id) {
                if let Screen::Workspace(ws) = screen() {
                    open_workspace(&ws);
                }
            }
        }
        Event::AgentRemove(id) => {
            global::<Agents>().update(|a| a.0.retain(|x| x.id != id));
            if tab() == Tab::Agent(id) {
                if let Screen::Workspace(ws) = screen() {
                    open_workspace(&ws);
                }
            }
        }
        Event::TimelineReset { agent_id, mut items, older, cursor } => {
            let mut opened = false;
            global::<Timeline>().update(|t| {
                if t.agent_id.as_deref() == Some(agent_id.as_str()) {
                    opened = !t.loaded;
                    carry_images(&t.items, &mut items);
                    crate::sent_pictures::restore(&mut items);
                    ask_pictures(&items, Some(&agent_id));
                    t.items = items;
                    t.cursor = cursor;
                    t.loaded = true;
                    t.syncing = false;
                    t.older = older;
                }
            });
            if opened {
                scroll_timeline_end_after(&[30, 120, 250], false);
            }
        }
        Event::TimelineSynced { agent_id } => global::<Timeline>().update(|t| {
            if t.agent_id.as_deref() == Some(agent_id.as_str()) {
                t.syncing = false;
                t.loaded = true;
                // What the host said came since the copy has no pictures yet:
                // those kept here first, then the host's Sophon is asked
                // for the rest (the copy's messages too).
                crate::sent_pictures::restore(&mut t.items);
                ask_pictures(&t.items, Some(&agent_id));
            }
        }),
        Event::Pictures { message_id, text, pictures } => {
            // Kept here from now on, and shown where the message is open.
            if message_id.is_empty() {
                crate::sent_pictures::keep_for_text(&text, &pictures);
            } else {
                crate::sent_pictures::keep(&message_id, &text, &pictures);
            }
            global::<Timeline>().update(|t| {
                for item in t.items.iter_mut() {
                    let Item::User { text: body, message_id: id, images } = item else { continue };
                    let same = if message_id.is_empty() { body.trim() == text.trim() } else { id.as_deref() == Some(message_id.as_str()) };
                    if same && images.is_empty() {
                        *images = pictures.clone();
                    }
                }
            });
        }
        Event::TimelineOlder { agent_id, items, older } => global::<Timeline>().update(|t| {
            if t.agent_id.as_deref() == Some(agent_id.as_str()) {
                let mut merged = items;
                crate::sent_pictures::restore(&mut merged);
                ask_pictures(&merged, Some(&agent_id));
                merged.append(&mut t.items);
                t.items = merged;
                t.older = older;
                t.loading_older = false;
            }
        }),
        Event::Commands { agent_id, list } => global::<Commands>().update(|c| {
            c.0.insert(agent_id, list);
        }),
        Event::Suggestions { query, entries } => global::<Suggestions>().update(|s| {
            if s.query == query {
                s.entries = entries;
            }
        }),
        Event::TimelineLive { agent_id, item, cursor } => global::<Timeline>().update(|t| {
            if t.agent_id.as_deref() != Some(agent_id.as_str()) {
                return;
            }
            if let Some((epoch, seq)) = cursor {
                match &t.cursor {
                    Some((e, end)) if *e == epoch && seq <= *end => return,
                    _ => t.cursor = Some((epoch, seq)),
                }
            }
            apply_live(&mut t.items, item);
        }),
        Event::Diff { cwd, files, error, too_large } => global::<Changes>().update(|c| {
            if c.cwd == cwd || c.cwd.is_empty() {
                *c = Changes { cwd, files, error, too_large, loaded: true };
            }
        }),
        Event::Terminals { cwd, list } => global::<Terminals>().update(|t| {
            if t.cwd == cwd {
                t.list = list;
            }
        }),
        Event::TerminalCreated(info) => {
            let id = info.id.clone();
            global::<Terminals>().update(|t| {
                if !t.list.iter().any(|x| x.id == info.id) {
                    t.list.push(info);
                }
            });
            show_terminal(&id);
        }
        Event::TerminalOutput { terminal_id, data, reset } => {
            TERM_OUT.with_borrow_mut(|out| {
                let b = out.entry(terminal_id).or_default();
                if reset {
                    b.chunks.clear();
                    b.reset = true;
                }
                if !data.is_empty() {
                    b.chunks.push(data);
                }
            });
            global::<TermTick>().update(|t| t.0 += 1);
        }
        Event::TerminalExit(id) => global::<Terminals>().update(|t| t.list.retain(|x| x.id != id)),
        Event::Dir { cwd, path, entries, error } => {
            if global::<Browse>().with_untracked(|b| b.cwd == cwd) && path == "." {
                let base = cwd.trim_end_matches('/').to_owned();
                let mut folders: Vec<String> = entries
                    .iter()
                    .filter(|e| e.is_dir && !e.name.starts_with('.'))
                    .map(|e| format!("{base}/{}", e.name))
                    .collect();
                folders.sort_by_key(|f| f.to_lowercase());
                global::<Browse>().update(|b| {
                    b.loaded = true;
                    b.folders = folders;
                    b.error = error;
                });
                return;
            }
            if let Some(e) = error {
                notify(tf("Could not list {}: {}", &[&path, &e]));
            }
            global::<FileTree>().update(|t| {
                if t.cwd == cwd {
                    t.listing.remove(&path);
                    t.dirs.insert(path, entries);
                }
            });
        }
        Event::File { path, file, error, .. } => global::<Files>().update(|files| {
            let entry = files.0.entry(path).or_default();
            entry.loaded = true;
            entry.error = error;
            if let Some(f) = file {
                entry.kind = f.kind;
                entry.disk = f.text.clone().unwrap_or_default();
                entry.text = entry.disk.clone();
                entry.modified_at = f.modified_at;
                entry.revision = f.revision;
            }
        }),
        Event::FileSaved { path, modified_at, revision, error } => {
            if let Some(e) = &error {
                notify(e.clone());
            }
            global::<Files>().update(|files| {
                if let Some(f) = files.0.get_mut(&path) {
                    f.saving = false;
                    if error.is_none() {
                        f.disk = f.text.clone();
                        f.modified_at = modified_at.unwrap_or_default();
                        f.revision = revision;
                    }
                }
            });
        }
        Event::GitStatus { cwd, status } => global::<Git>().update(|g| {
            if g.cwd == cwd {
                g.status = Some(status);
            }
        }),
        Event::PullRequest { cwd, pr } => global::<Git>().update(|g| {
            if g.cwd == cwd {
                g.pr = pr;
            }
        }),
        Event::GitDone { action, error, url, .. } => {
            global::<Git>().update(|g| g.busy = None);
            match error {
                Some(e) => notify(tf("{} failed: {}", &[&action, &e])),
                None => {
                    notify(match url {
                        Some(u) => tf("Pull request created: {}", &[&u]),
                        None => tf("{} done", &[&action]),
                    });
                    if let Some(cwd) = current_workspace().map(|w| w.directory) {
                        send(Command::GitStatus(cwd));
                    }
                }
            }
        }
        Event::History { search, agents } => global::<HistoryState>().update(|h| {
            if h.search == search {
                h.agents = agents;
                h.loaded = true;
            }
        }),
        Event::FileOpDone { dir, error } => {
            if let Some(e) = error {
                notify(e);
            }
            if let Some(cwd) = tree_cwd() {
                send(Command::ListDir { cwd: cwd.clone(), path: dir.clone() });
                send(Command::ListDir { cwd, path: ROOT.into() });
            }
        }
        Event::DraftFeatures { provider, features } => edit_form(|f| {
            if f.provider == provider {
                f.features = features;
            }
        }),
        Event::ProjectConfig { root, result, saved } => global::<ProjectConfig>().update(|c| {
            if c.root != root {
                return;
            }
            match result {
                Ok((config, revision)) => {
                    c.text = Some(serde_json::to_string_pretty(&config).unwrap_or_default());
                    c.revision = revision;
                    c.error = None;
                    if saved {
                        untracked(|| {
                            close_modal();
                            notify(t("Project settings saved").into());
                        });
                    }
                }
                Err(e) => c.error = Some(e),
            }
        }),
        Event::CommitFileDiff { file, error, .. } => global::<CommitDiffState>().update(|c| {
            c.0 = Some(match (file, error) {
                (_, Some(e)) => Err(e),
                (Some(f), None) => Ok(f),
                (None, None) => Err("No text changes in this file".into()),
            })
        }),
        Event::Uploaded { id, result } => uploaded(id, result),
        Event::Downloaded { id, result } => downloaded(id, result),
        Event::PairingOffer { url, relay_enabled, error } => global::<Pairing>().update(|p| {
            // The QR is Sophon's link (LAN + relay); `url` stays the daemon's
            // relay link, for its own app.
            let link = pairing_link(url.as_deref().filter(|_| relay_enabled));
            *p = Pairing { loaded: true, qr_svg: link.as_deref().and_then(qr_svg), sophon_url: link, url, relay_enabled, error };
        }),
        Event::Labels(list) => global::<LabelCatalog>().update(|l| l.0 = list),
        Event::PushInfo { gateway, tls, id } => {
            global::<PushState>().update(|p| p.served = !gateway.is_empty() && !id.is_empty());
            // What a pressed action needs, kept for a launch with no screen.
            #[cfg(target_os = "ios")]
            if let Some((_, key, token)) = current_host().and_then(|h| crate::daemon::relay::PairingLink::parse(&h.url)).and_then(|l| l.lan) {
                crate::platform::ios_push::remember_host(&id, &gateway, tls, &key, &token);
            }
            #[cfg(not(target_os = "ios"))]
            let _ = tls;
        }
        Event::HostUsage(report) => global::<LocalUsage>().update(|u| {
            u.loading = false;
            if let Some(report) = report.as_ref().and_then(crate::usage::Report::from_json) {
                u.report = Some(report);
            }
        }),
        Event::Usage(list) => {
            record_limits(&list);
            push_limits(&list);
            global::<UsageState>().update(|u| u.0 = Some(list));
        }
        Event::DaemonConfig(cfg) => {
            global::<DaemonConfig>().update(|c| c.0 = Some(cfg));
            use_bundled_clis();
        }
        Event::Diagnostic { provider, text } => {
            let d = crate::daemon::agents::parse_diagnostic(&text);
            if let Some(path) = d.path.clone().filter(|_| d.version.is_some()) {
                check_agent_update(path);
            }
            global::<Diagnostics>().update(|x| {
                x.map.insert(provider, d);
            });
        }
        Event::RecentSessions(list) => global::<Recent>().update(|r| r.0 = Some(list)),
        Event::Rewound { agent, error } => match error {
            Some(e) => notify(tf("Rewind failed: {}", &[&e])),
            None => {
                notify(t("Rewound").into());
                if tab() == Tab::Agent(agent.clone()) {
                    open_tab(Tab::Agent(agent));
                }
            }
        },
        Event::Forked { agent, context, error } => {
            let mut handoff = None;
            global::<Handoff>().update(|h| handoff = h.0.take().filter(|(a, _, _)| *a == agent));
            match (error, context) {
                (Some(e), _) => notify(tf("Fork failed: {}", &[&e])),
                (None, Some(c)) => {
                    let title = self::agent(&agent).map(|a| a.title).unwrap_or_else(|| "agent".into());
                    open_tab(Tab::Draft);
                    edit_form(|f| f.context = Some((c, title)));
                    // The provider first: it resets the model to its default.
                    if let Some((_, provider, model)) = handoff {
                        set_control(false, Control::Provider(provider));
                        if !model.is_empty() {
                            set_control(false, Control::Model(model));
                        }
                    }
                }
                (None, None) => notify(t("Nothing to fork yet").into()),
            }
        }
        Event::Subagents { parent, list } => global::<SubagentState>().update(|s| {
            s.by_parent.insert(parent, list);
        }),
        Event::SubagentUpsert(sub) => global::<SubagentState>().update(|s| {
            let list = s.by_parent.entry(sub.parent.clone()).or_default();
            list.retain(|x| x.id != sub.id);
            list.push(sub);
        }),
        Event::SubagentTimeline { id, items, .. } => global::<SubagentState>().update(|s| {
            s.timelines.insert(id, items);
        }),
        Event::BackgroundWork { agent, items } => global::<Background>().update(|b| {
            b.0.insert(agent, items);
        }),
        Event::Commits { commits, error, .. } => global::<CommitsState>().update(|c| c.0 = Some((commits, error))),
        Event::Branches { list, .. } => global::<BranchesState>().update(|b| b.0 = list),
        Event::Schedules(list) => {
            // The runs shown may have a new one: asked again with the list.
            for id in global::<SchedulesState>().with_untracked(|s| s.runs.keys().cloned().collect::<Vec<_>>()) {
                send(Command::ScheduleRuns(id));
            }
            global::<SchedulesState>().update(|s| s.list = Some(list));
        }
        Event::ScheduleRuns { id, mut runs } => {
            runs.sort_by(|a, b| b.started.cmp(&a.started));
            global::<SchedulesState>().update(|s| {
                // Put away while the answer was on its way: stays away.
                if let Some(kept) = s.runs.get_mut(&id) {
                    *kept = runs;
                }
            });
        }
        Event::Notice(message) => notify(crate::i18n::tr(&message)),
    }
}

/// ⌘Q (and ⌘W with nothing left to close) asks twice, as Chrome does: the
/// first press shows a hint, a second within two seconds quits. Holds the
/// hint's text while it shows.
#[derive(Clone, Default)]
struct QuitHint(Option<(u64, &'static str)>);

pub(crate) fn quit_hint() -> Option<&'static str> {
    global::<QuitHint>().with(|h| h.0.map(|(_, m)| m))
}

/// Quits when asked again while the hint shows; else shows it.
pub(crate) fn request_quit(hint: &'static str) {
    if global::<QuitHint>().with_untracked(|h| h.0.is_some()) {
        nsh_core::element::desktop::quit();
        return;
    }
    let id = global::<QuitHint>().with_untracked(|h| h.0.map_or(0, |(n, _)| n)) + 1;
    global::<QuitHint>().set(QuitHint(Some((id, hint))));
    spawn_detached(async move {
        nsh_core::element::timer(std::time::Duration::from_millis(2000)).await;
        global::<QuitHint>().update(|h| {
            if h.0.is_some_and(|(n, _)| n == id) {
                h.0 = None;
            }
        });
    });
}

/// Whether the keyboard is in the terminal panel (⌘W then ends the
/// terminal shown there, not a tab above).
#[derive(Clone, Default)]
struct TerminalFocus(bool);

pub(crate) fn set_terminal_focused(on: bool) {
    global::<TerminalFocus>().set(TerminalFocus(on));
}

/// ⌘W: closes the tab where the keyboard is, and nothing wider: the
/// terminal shown in the panel when it has the focus; else a file, diff,
/// sub-agent or new-chat tab (back to the conversation). A conversation
/// itself, or a screen with no tabs, stays: quitting is ⌘Q's.
pub(crate) fn close_current() {
    if modal().is_some() {
        close_modal();
        return;
    }
    let Screen::Workspace(_) = screen() else { return };
    if terminal_panel_open() && global::<TerminalFocus>().with_untracked(|f| f.0) {
        if let Some(id) = active_terminal() {
            kill_terminal(&id);
        }
        return;
    }
    match tab() {
        Tab::File(path) => close_file(&path),
        Tab::Subagent(parent, _) => open_tab(Tab::Agent(parent)),
        Tab::Diff(_) | Tab::CommitDiff(..) | Tab::Draft => {
            if let Some(w) = current_workspace() {
                open_workspace(&w.id);
            }
        }
        Tab::Agent(_) => {}
    }
}

#[cfg(test)]
mod model_rule_tests {
    use super::forbidden_model;

    #[test]
    fn a_claude_model_through_a_subscription_is_refused() {
        // Claude's subscription outside Claude Code.
        assert!(forbidden_model("claude/claude-sonnet-5", "Claude Sonnet 5 · Claude"));
        // Claude through Copilot's, Cursor's subscription.
        assert!(forbidden_model("copilot/claude-sonnet-5", "Claude Sonnet 5 · Copilot"));
        assert!(forbidden_model("copilot/claude-haiku-5.5", "Claude Haiku 5.5 · Copilot"));
        assert!(forbidden_model("cursor/opus-5", "Opus 5 · Cursor"));
    }

    #[test]
    fn other_models_and_claude_on_an_api_key_stay() {
        assert!(!forbidden_model("claude-opus-5-5", "Opus 5.5"));
        assert!(!forbidden_model("gpt-6-sol", "GPT-6-Sol"));
        assert!(!forbidden_model("copilot/gpt-5.6-luna", "GPT-5.6 Luna · Copilot"));
        assert!(!forbidden_model("copilot/gemini-3.7-flash", "Gemini 3.7 Flash · Copilot"));
        assert!(!forbidden_model("codex/gpt-6-astra", "GPT-6-Astra · Codex"));
        assert!(!forbidden_model("anthropic/claude-sonnet-5", "Claude Sonnet 5 · Anthropic"));
        assert!(!forbidden_model("teable/claude-opus-5-5", "Claude Opus 5.5 · teable"));
    }
}

#[cfg(test)]
mod config_patch_tests {
    use super::{TEXT_GENERATION, config_patch, text_generation_value};
    use serde_json::json;

    #[test]
    fn a_text_generation_model_patches_only_its_key() {
        let patch = config_patch(TEXT_GENERATION, text_generation_value(Some(("codex", "gpt-5.4-mini"))));
        assert_eq!(patch, json!({ "metadataGeneration": { "providers": [{ "provider": "codex", "model": "gpt-5.4-mini" }] } }));
    }

    #[test]
    fn default_clears_the_list() {
        let patch = config_patch(TEXT_GENERATION, text_generation_value(None));
        assert_eq!(patch, json!({ "metadataGeneration": { "providers": [] } }));
    }

    #[test]
    fn other_settings_still_patch_as_before() {
        assert_eq!(config_patch("/autoArchiveAfterMerge", json!(true)), json!({ "autoArchiveAfterMerge": true }));
        assert_eq!(config_patch("/providers/codex/enabled", json!(false)), json!({ "providers": { "codex": { "enabled": false } } }));
    }
}

/// Each agent's models as it keeps them on this computer ([`crate::models`]),
/// by provider id.
#[derive(Clone, Default)]
struct LocalModels(HashMap<String, Vec<crate::models::LocalModel>>);

/// `provider`'s models as the agent itself lists them, while the host is
/// this computer: they include models the service's built-in list does not
/// have yet, in the agent's own order. `None` elsewhere, or for an agent
/// that keeps no list.
pub(crate) fn local_models(provider: &str) -> Option<Vec<crate::models::LocalModel>> {
    if !on_this_mac() {
        return None;
    }
    global::<LocalModels>().with(|m| m.0.get(provider).cloned())
}

/// Reads the agents' own model lists again.
pub(crate) fn load_local_models() {
    let mut map = HashMap::new();
    for provider in ["claude", "codex"] {
        if let Some(list) = crate::models::read_local(provider) {
            map.insert(provider.to_owned(), list);
        }
    }
    global::<LocalModels>().set(LocalModels(map));
}
