# Feature checklist

Sophon's feature checklist against the daemon's protocol.
Each line: feature — daemon messages — priority/size. `[x]` done, `[~]` partial, `[ ]` todo.

## Navigation

- Sidebar: header items (New workspace, History, Search, Schedules) → pinned → Project → Workspace rows
  (status dot, title, PR badge, diff stat). **Agents are tabs inside a workspace, not sidebar rows.**
- Workspace screen: tab strip (agent, draft, terminal, changes, files, PR, file, diff, setup) + right
  Explorer (Files / Changes) + composer per agent pane (tracks: Queue, Subagents, Tasks).
- Compact layout: header + sheets.

## P0 — core daily use

- [x] Connect, hello with local credential, reconnect with backoff
- [x] Agent list + live updates — `fetch_agents_request` (sub), `agent_update`
- [x] Timeline tail + live stream — `fetch_agent_timeline_request`, `agent.timeline.set_subscription.request`, `agent_stream`
- [x] Send message, stop — `send_agent_message_request`, `cancel_agent_request`
- [x] Permissions: tool, plan and question cards — `agent_permission_response` (`selectedActionId`, `updatedInput.answers`)
- [x] Project + workspace sync — `project.list.request`, `project.update`, `fetch_workspaces_request` (sub), `workspace_update`
- [x] Sidebar as Project → Workspace (pinned section); agents as workspace tabs + draft tab
- [x] Workspace status dot, clear attention on open, mark unread
- [x] Pin / rename / archive workspace with confirm + undo toast
- [x] New workspace + first agent in one `workspace.create.request` (project picker, local/worktree, base branch, branch name, import session)
- [x] New agent in existing workspace (draft tab) — `create_agent_request`
- [x] Providers snapshot: model / mode / thinking pickers — `get_providers_snapshot_request`, `providers_snapshot_update`
- [x] Live and draft agent controls — model / mode / thinking / provider features
- [x] Composer: steer / interrupt / queue (setting), per-agent drafts
- [x] Slash commands — `list_commands_request`
- [x] @ file mentions — `directory_suggestions_request`
- [x] Timeline rows: compaction notices; tool call detail sheet
- [x] History pagination — `fetch_agent_timeline_request` direction `before`
- [x] Rename / archive agent — `update_agent_request`, `archive_agent_request`
- [x] Add project: existing folder, new folder, clone from GitHub; project rename / remove

## P1

- [x] Diff review: Explorer Changes list + diff tabs (split/unified) + review notes and line-anchored comments (+ beside a line or selection) sent to the agent
- [x] Terminal tabs + PTY — `subscribe_terminals_request`, `create_terminal_request`, `subscribe_terminal_request` (full replay), binary frames, `terminal_input`, `kill_terminal_request`
- [x] File explorer tree (context menu: new file / folder, rename, duplicate, delete) + editor tab with conflict-checked save — `file_explorer_request`, `fs.entry.*`, `fs.file.write.request` (live watch todo)
- [x] Git actions: commit all, push, pull, merge into base, switch branch, stash / pop; ahead/behind
- [x] PR create, status badge with checks, merge / squash — `checkout_pr_*` (timeline panel todo)
- [x] Commits list (Explorer tab) with per-commit file diffs — `checkout.commits.list.request`, `checkout.commits.file_diff.request`
- [x] Sub-agents track + read-only sub-agent tabs, live — `agent.provider_subagents.*` (detach todo)
- [x] Background work track with stop — `agent.background_work.*`
- [~] Attachments: images via picker or clipboard paste (`images`) — file uploads, issue/PR todo
- [x] Rewind (conversation / files / both) and fork into a new agent — `agent.rewind.request`, `agent.fork_context.request`
- [x] Find in chat (⌘F, client-side over loaded history) + outline of prompts
- [x] Workspace scripts / services menu: state, health, start / stop, open URL — `workspace.script.*`
- [x] Import provider session (search + import) — `fetch_recent_provider_sessions_request`, `import_agent_request`
- [x] History screen with search + restore archived — `fetch_agent_history_request`, `refresh_agent_request`
- [x] Context window meter + cost; tasks track; project rename / remove / settings (`paseo.json` editor)

## P2

- [x] Schedules: list, create (presets + cron, local/worktree), pause/resume, run now, delete — `schedule/*`
- [x] Command palette (⌘K) + shortcuts (⌘N ⌘T ⇧⌘T ⌘S ⌘B ⌘E ⌘, ⌘1–9)
- [x] Settings: sending, host, host behavior (daemon config toggles, system prompt), providers on/off
- [x] Host: status, version, update and restart daemon — `daemon.update.request`, `restart_server_request`
- [x] Usage: per-provider plan windows (used %, resets) — `provider.usage.list.request`
- [~] Dictation into the composer with the system speech recognizer (mic button) — daemon realtime voice mode todo
- [x] Multi-host + pairing: host switcher; relay E2EE (Noise NK protected + legacy NaCl box); pairing QR (`sophon://pair`, camera scan via URL scheme); Sophon LAN bridge (Noise NK + token) with relay fallback; direct LAN / Tailscale / wss addresses with password
- [~] Notifications: desktop system notifications on attention / approval / finish (press opens the agent) — workspace recovery and labels done
- [ ] Plugins — JS bundles; likely partial parity only (L)
- Out of scope: Electron browser pane.
