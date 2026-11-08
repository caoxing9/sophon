//! The app's state, one signal per concern so a streaming delta re-renders
//! the conversation and nothing else. Screens read through these functions
//! and change state only here: by applying daemon events or sending commands.
//!
//! Navigation follows Paseo: the sidebar lists projects and their
//! workspaces; a workspace shows its agents as tabs, plus a draft tab that
//! starts a new agent there. "New workspace" is its own screen.

use crate::i18n::{t, tf};
use std::{cell::RefCell, collections::HashMap};

use nsh_core::prelude::*;
use tokio::sync::mpsc::UnboundedSender;

use crate::daemon::{
    Agent, Command, Event, Item, Project, Provider, Workspace,
    connection::{CreateAgent, ImageInput, SlashCommand},
    model::apply_live,
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
    Workspace(String),
}

#[derive(Clone, Debug, PartialEq, Default)]
pub(crate) enum Tab {
    #[default]
    Draft,
    Agent(String),
    /// The workspace's uncommitted changes: one file, or all (`None`).
    Diff(Option<String>),
    Terminal(String),
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
    send(Command::Usage);
    open_modal(Modal::Usage);
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
    send(Command::Send { agent_id: agent.id.clone(), text, behavior, images: vec![], attachments: vec![] });
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
    let Some(cwd) = tree_cwd() else { return };
    let known = global::<Files>().with_untracked(|f| f.0.contains_key(path));
    if !known {
        global::<Files>().update(|f| {
            f.0.insert(path.to_owned(), OpenFile::default());
        });
        send(Command::ReadFile { cwd, path: path.to_owned() });
    }
    open_tab(Tab::File(path.to_owned()));
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
    }
}

pub(crate) fn kill_terminal(id: &str) {
    global::<Terminals>().update(|t| t.list.retain(|x| x.id != id));
    send(Command::KillTerminal(id.to_owned()));
    if tab() == Tab::Terminal(id.to_owned())
        && let Some(w) = current_workspace()
    {
        open_workspace(&w.id);
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
    tabs.extend(terminals().into_iter().map(|t| Tab::Terminal(t.id)));
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
}
/// Messages waiting for the running turn to end, per agent.
#[derive(Clone, Default)]
struct Queues(HashMap<String, Vec<String>>);
#[derive(Clone, Default)]
struct Commands(HashMap<String, Vec<SlashCommand>>);
#[derive(Clone, Default)]
struct Suggestions {
    query: String,
    entries: Vec<(String, bool)>,
}

/// What Return does while the agent is running (Paseo's "Sending" setting).
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
    ArchiveWorkspace { id: String, name: String },
    Commit,
    Palette,
    NewSchedule,
    AddProject,
    RenameProject { id: String, name: String },
    SwitchBranch,
    ToolDetail { name: String, detail: String },
    ImportSession,
    RestartDaemon,
    Usage,
    AddHost,
    PairDevice,
    Labels { workspace: String },
    ProjectSettings { root: String, name: String },
    /// Names a new file or folder in `dir`, or renames `path`.
    FileName { op: &'static str, path: String, name: String },
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
    global::<ModalState>().update(|s| s.0 = None);
}

#[derive(Clone, Debug, PartialEq, Default)]
pub(crate) struct Settings {
    pub(crate) send_behavior: SendBehavior,
}
/// What a notice's button does.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum NoticeAction {
    UndoArchive(String),
}

#[derive(Clone, Default)]
struct Notice(Option<(u64, String, Option<NoticeAction>)>);
/// Unsent composer text per agent (Paseo keeps drafts per agent).
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

/// Every workspace, those being archived left out.
pub(crate) fn workspaces() -> Vec<Workspace> {
    global::<Workspaces>().with(|w| w.0.iter().filter(|w| !w.archiving).cloned().collect())
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
    let workspaces: Vec<Workspace> =
        global::<Workspaces>().with(|w| w.0.iter().filter(|w| !w.archiving).cloned().collect());

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

pub(crate) fn item_keys() -> Vec<String> {
    global::<Timeline>().with(|t| {
        let agent = t.agent_id.as_deref().unwrap_or_default();
        (0..t.items.len()).map(|i| format!("{agent}:{i}")).collect()
    })
}

pub(crate) fn timeline_loaded() -> bool {
    global::<Timeline>().with(|t| t.loaded)
}

pub(crate) fn has_older() -> bool {
    global::<Timeline>().with(|t| t.older.is_some())
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

/// The largest file the host takes (Paseo's limit).
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

#[cfg(not(target_arch = "wasm32"))]
type BridgeInfo = crate::daemon::bridge::BridgeInfo;
/// A page runs no bridge.
#[cfg(target_arch = "wasm32")]
#[derive(Clone)]
struct BridgeInfo {
    addr: String,
    key: [u8; 32],
    token: String,
}

thread_local! {
    static BRIDGE: RefCell<Option<BridgeInfo>> = const { RefCell::new(None) };
}

/// Starts the LAN bridge (desktops), so paired phones on this network skip
/// the relay.
pub(crate) fn start_bridge() {
    #[cfg(target_arch = "wasm32")]
    let info: Option<BridgeInfo> = None;
    #[cfg(not(target_arch = "wasm32"))]
    let info = crate::daemon::bridge::start();
    if info.is_none() {
        log::warn!("the LAN bridge could not start; phones pair through the relay only");
    }
    BRIDGE.with_borrow_mut(|b| *b = info);
}

/// The link a phone pairs with: the LAN bridge and the daemon's relay.
pub(crate) fn pairing_link(relay_link: Option<&str>) -> Option<String> {
    let lan = BRIDGE.with_borrow(|b| b.clone());
    if lan.is_none() && relay_link.is_none() {
        return None;
    }
    Some(crate::daemon::relay::PairingLink::format(
        lan.as_ref().map(|b| (b.addr.as_str(), &b.key, b.token.as_str())),
        relay_link,
    ))
}

pub(crate) fn bridge_addr() -> Option<String> {
    BRIDGE.with_borrow(|b| b.as_ref().map(|b| b.addr.clone()))
}

/// Pairs from a scanned `sophon://pair…` link (the system camera opens it).
pub(crate) fn watch_pairing_links() {
    spawn(async move {
        loop {
            let url = next_url_scheme_open().await;
            if crate::daemon::relay::PairingLink::parse(&url).is_some() {
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
    if let Some(host) = current_host() {
        send(Command::SwitchHost(Some(crate::daemon::connection::RemoteHost { url: host.url, password: host.password })));
    }
}

/// Connects to `id` (`None`: this machine), starting fresh.
pub(crate) fn switch_host(id: Option<String>) {
    global::<Hosts>().update(|h| h.current = id.clone());
    save_hosts();
    batch(|| {
        global::<Projects>().update(|p| p.0.clear());
        global::<Workspaces>().update(|w| w.0.clear());
        global::<Agents>().update(|a| a.0.clear());
        global::<Providers>().update(|p| p.0.clear());
        global::<Nav>().update(|n| *n = Nav::default());
    });
    let remote = current_host().map(|h| crate::daemon::connection::RemoteHost { url: h.url, password: h.password });
    send(Command::SwitchHost(remote));
}

pub(crate) fn add_host(name: &str, url: &str, password: &str) {
    let id = format!("host_{}", uuid::Uuid::new_v4().simple());
    let profile = HostProfile {
        id: id.clone(),
        name: if name.trim().is_empty() { url.trim().to_owned() } else { name.trim().to_owned() },
        url: url.trim().to_owned(),
        password: Some(password.to_owned()).filter(|p| !p.is_empty()),
    };
    global::<Hosts>().update(|h| h.list.push(profile));
    switch_host(Some(id));
}

pub(crate) fn remove_host(id: &str) {
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
        "sidebarHidden": global::<SidebarHidden>().with_untracked(|h| h.0),
        "explorerOpen": global::<Explorer>().with_untracked(|e| e.open),
        "sidebarWidth": global::<SidebarWidth>().with_untracked(|w| w.0),
        "projectOrder": global::<ProjectOrder>().with_untracked(|o| o.0.clone()),
        "lastWorkspace": global::<LastOpen>().with_untracked(|l| l.workspace.clone()),
        "lastAgent": global::<LastOpen>().with_untracked(|l| l.agent.clone()),
        "collapsedProjects": global::<Collapsed>().with_untracked(|c| c.0.clone()),
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
    let Some(text) = crate::daemon::read_file("settings.json") else { return };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { return };
    let behavior = match v.get("sendBehavior").and_then(serde_json::Value::as_str) {
        Some("interrupt") => SendBehavior::Interrupt,
        Some("queue") => SendBehavior::Queue,
        _ => SendBehavior::Steer,
    };
    global::<Settings>().update(|s| s.send_behavior = behavior);
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
fn service_cli() -> Option<std::path::PathBuf> {
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

/// Whether this Mac's service comes with the Paseo app (its updates come
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

/// Starts the agent service on this Mac; the connection, which retries on
/// its own, picks it up.
pub(crate) fn start_service() {
    if global::<Starting>().with_untracked(|s| s.0) {
        return;
    }
    let Some(cli) = service_cli() else {
        notify(crate::i18n::tf("Couldn't start the service: {}", &["paseo CLI not found"]));
        return;
    };
    global::<Starting>().set(Starting(true));
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Result<(), String>>();
    std::thread::spawn(move || {
        let out = std::process::Command::new(&cli).args(["daemon", "start"]).output();
        let _ = tx.send(match out {
            Ok(o) if o.status.success() => Ok(()),
            Ok(o) => Err(String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or("").to_owned()),
            Err(e) => Err(e.to_string()),
        });
    });
    spawn(async move {
        let result = rx.recv().await;
        global::<Starting>().set(Starting(false));
        if let Some(Err(e)) = result {
            notify(crate::i18n::tf("Couldn't start the service: {}", &[&e]));
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
struct SchedulesState(Option<Vec<crate::daemon::connection::Schedule>>);

pub(crate) fn open_schedules() {
    global::<Nav>().update(|n| n.screen = Screen::Schedules);
    send(Command::WatchDiff(None));
    send(Command::Schedules);
}

pub(crate) fn schedules() -> Option<Vec<crate::daemon::connection::Schedule>> {
    global::<SchedulesState>().with(|s| s.0.clone())
}

pub(crate) fn schedule_op(id: &str, op: &'static str) {
    send(Command::ScheduleOp { id: id.to_owned(), op });
}

pub(crate) fn create_schedule(name: String, prompt: String, cadence: String, cwd: String, worktree: bool) {
    let f = form();
    let provider = if f.provider.is_empty() { "claude".to_owned() } else { f.provider };
    send(Command::CreateSchedule(crate::daemon::connection::NewSchedule {
        name,
        prompt,
        cadence,
        cwd,
        provider,
        model: f.model,
        worktree,
    }));
}

#[derive(Clone, Default)]
struct DaemonConfig(Option<serde_json::Value>);

pub(crate) fn daemon_config() -> Option<serde_json::Value> {
    global::<DaemonConfig>().with(|c| c.0.clone())
}

/// Sets `pointer` (`/autoArchiveAfterMerge`) to `value`, shown at once and
/// sent as a patch.
pub(crate) fn set_daemon_config(pointer: &str, value: serde_json::Value) {
    let mut patch = serde_json::json!({});
    let keys: Vec<&str> = pointer.trim_start_matches('/').split('/').collect();
    {
        let mut node = &mut patch;
        for (i, k) in keys.iter().enumerate() {
            if i + 1 == keys.len() {
                node[*k] = value.clone();
            } else {
                node = &mut node[*k];
            }
        }
    }
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
        Tab::Draft | Tab::Diff(_) | Tab::Terminal(_) | Tab::File(_) | Tab::Subagent(..) | Tab::CommitDiff(..) => None,
    };
    if let (Tab::CommitDiff(sha, path), Some(cwd)) = (&tab, current_workspace().map(|w| w.directory)) {
        global::<CommitDiffState>().update(|c| c.0 = None);
        send(Command::CommitFileDiff { cwd, sha: sha.clone(), path: path.clone() });
    }
    if let Tab::Subagent(parent, id) = &tab {
        send(Command::SubagentTimeline { parent: parent.clone(), id: id.clone() });
    }
    if matches!(tab, Tab::Diff(_) | Tab::Terminal(_) | Tab::File(_) | Tab::Subagent(..) | Tab::CommitDiff(..)) {
        global::<Nav>().update(|n| n.tab = tab);
        return;
    }
    batch(|| {
        global::<Nav>().update(|n| n.tab = tab.clone());
        global::<Timeline>().update(|t| *t = Timeline { agent_id: agent_id.clone(), ..Default::default() });
        if tab == Tab::Draft {
            reset_form(current_workspace().map(|w| w.directory));
        }
    });
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
fn apply_provider_defaults(f: &mut Form) {
    let Some(p) = provider(&f.provider) else { return };
    let model = p.default_model().cloned();
    f.model = model.as_ref().map(|m| m.id.clone());
    f.thinking = model.and_then(|m| m.default_thinking.or_else(|| m.thinking.first().map(|t| t.id.clone())));
    f.mode = p.default_mode.clone().or_else(|| p.modes.first().map(|m| m.id.clone()));
}

fn reset_form(path: Option<String>) {
    let list = providers();
    global::<Form>().update(|f| {
        let provider = default_provider(&list, &f.provider);
        let keep = f.provider == provider && f.model.is_some();
        let (model, mode, thinking) = (f.model.clone(), f.mode.clone(), f.thinking.clone());
        *f = Form { path: path.clone().unwrap_or_default(), provider, ..Default::default() };
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
            let model = provider(&f.provider).and_then(|p| p.model(&m).cloned());
            f.model = Some(m);
            if let Some(model) = model {
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
        images: take_images(),
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
    if !images.is_empty() {
        shown.push_str(&format!("\n\n[{} image(s)]", images.len()));
    }
    for a in &attachments {
        if let Some(name) = a.get("fileName").and_then(serde_json::Value::as_str) {
            shown.push_str(&format!("\n📎 {name}"));
        }
    }
    let mut shown_here = false;
    global::<Timeline>().update(|t| {
        if t.agent_id.as_deref() == Some(agent_id) {
            t.items.push(Item::User { text: shown.trim().to_owned(), message_id: None });
            shown_here = true;
        }
    });
    if shown_here {
        scroll_timeline_to_end();
    }
    send(Command::Send { agent_id: agent_id.to_owned(), text: text.to_owned(), behavior, images, attachments });
}

/// A system notification when an agent starts needing you while you look
/// elsewhere (Paseo's attention notifications); the press opens it.
fn notify_attention(next: &Agent) {
    let before = agent(&next.id);
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
    let n = nsh_core::notifications::Notification::new(format!("agent:{}", next.id), next.title.clone(), body);
    let _ = show_notification(n);
}

fn lifecycle_active() -> bool {
    matches!(lifecycle(), nsh_core::element::Lifecycle::Active)
}

/// Opens the agent a pressed notification names.
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
    if agent.is_running() {
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

pub(crate) fn scroll_timeline_to(index: usize) {
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

pub(crate) fn rewind(message_id: &str, mode: &'static str) {
    if let Some(agent) = selected_id() {
        send(Command::Rewind { agent, message_id: message_id.to_owned(), mode });
    }
}

pub(crate) fn fork_selected() {
    if let Some(agent) = selected_id() {
        send(Command::Fork(agent));
    }
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

pub(crate) fn dismiss_notice() {
    global::<Notice>().update(|n| n.0 = None);
}

pub(crate) fn notify(message: String) {
    notify_with(message, None);
}

pub(crate) fn notify_with(message: String, action: Option<NoticeAction>) {
    let id = global::<Notice>().with_untracked(|n| n.0.as_ref().map_or(0, |(id, _, _)| id + 1));
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
            global::<ConnState>().update(|c| {
                c.0 = Conn::Connected { hostname, version, url: c.1.clone() };
            });
            if global::<Restarting>().with_untracked(|r| r.0) {
                global::<Restarting>().set(Restarting(false));
                notify(t("The service restarted").into());
            }
        }
        Event::Disconnected { reason } => global::<ConnState>().update(|c| c.0 = Conn::Down(reason)),
        Event::ProjectsReset(list) => global::<Projects>().update(|p| p.0 = list),
        Event::ProjectUpsert(project) => global::<Projects>().update(|p| {
            p.0.retain(|x| x.id != project.id);
            p.0.push(project);
        }),
        Event::ProjectRemove(id) => global::<Projects>().update(|p| p.0.retain(|x| x.id != id)),
        Event::WorkspacesReset(list) => {
            global::<Workspaces>().update(|w| w.0 = list);
            restore_last_open();
        }
        Event::WorkspaceUpsert(ws) => global::<Workspaces>().update(|w| {
            w.0.retain(|x| x.id != ws.id);
            w.0.push(ws);
        }),
        Event::WorkspaceRemove(id) => {
            global::<Workspaces>().update(|w| w.0.retain(|x| x.id != id));
            if screen() == Screen::Workspace(id) {
                go_home();
            }
        }
        Event::Providers(list) => {
            let first = global::<Providers>().with_untracked(|p| p.0.is_empty());
            global::<Providers>().update(|p| p.0 = list);
            if first {
                let path = form().path;
                reset_form(Some(path));
            }
        }
        Event::Created(agent) => {
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
            global::<Agents>().update(|a| a.0 = list.into_iter().filter(|a| !a.archived).collect())
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
        Event::TimelineReset { agent_id, items, older } => {
            let mut opened = false;
            global::<Timeline>().update(|t| {
                if t.agent_id.as_deref() == Some(agent_id.as_str()) {
                    opened = !t.loaded;
                    t.items = items;
                    t.loaded = true;
                    t.older = older;
                }
            });
            if opened {
                scroll_timeline_end_after(&[30, 120, 250], false);
            }
        }
        Event::TimelineOlder { agent_id, items, older } => global::<Timeline>().update(|t| {
            if t.agent_id.as_deref() == Some(agent_id.as_str()) {
                let mut merged = items;
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
        Event::TimelineLive { agent_id, item } => global::<Timeline>().update(|t| {
            if t.agent_id.as_deref() == Some(agent_id.as_str()) {
                apply_live(&mut t.items, item);
            }
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
            open_tab(Tab::Terminal(id));
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
        Event::PairingOffer { url, relay_enabled, error } => global::<Pairing>().update(|p| {
            // The QR is Sophon's link (LAN + relay); `url` stays Paseo's for
            // its own app.
            let link = pairing_link(url.as_deref().filter(|_| relay_enabled));
            *p = Pairing { loaded: true, qr_svg: link.as_deref().and_then(qr_svg), sophon_url: link, url, relay_enabled, error };
        }),
        Event::Labels(list) => global::<LabelCatalog>().update(|l| l.0 = list),
        Event::Usage(list) => global::<UsageState>().update(|u| u.0 = Some(list)),
        Event::DaemonConfig(cfg) => global::<DaemonConfig>().update(|c| c.0 = Some(cfg)),
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
        Event::Forked { agent, context, error } => match (error, context) {
            (Some(e), _) => notify(tf("Fork failed: {}", &[&e])),
            (None, Some(c)) => {
                let title = self::agent(&agent).map(|a| a.title).unwrap_or_else(|| "agent".into());
                open_tab(Tab::Draft);
                edit_form(|f| f.context = Some((c, title)));
            }
            (None, None) => notify(t("Nothing to fork yet").into()),
        },
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
        Event::Schedules(list) => global::<SchedulesState>().update(|s| s.0 = Some(list)),
        Event::Notice(message) => notify(crate::i18n::tr(&message)),
    }
}
