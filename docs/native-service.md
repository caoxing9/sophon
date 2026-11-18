# Rust 宿主服务

桌面端按需启动 `sophon-service`。它在本机回环地址上监听，通过每次启动生成的凭证验证连接。局域网和自托管中继沿用 Sophon 的加密桥接。设置里留空中继地址表示只使用局域网。

## 开发

```sh
cargo build --locked -p sophon-service
target/debug/sophon-service daemon start
target/debug/sophon-service daemon stop
```

`daemon run` 可以在前台运行。默认服务目录为 `~/.sophon/service`，其中 `state.json` 保存项目、工作区和 Agent，`timelines/<agentId>.jsonl` 按 Agent 保存时间线。`--home DIR` 或 `SOPHON_SERVICE_HOME` 可以指定独立目录。开发版应用会查找同目录的服务程序，也可以通过 `SOPHON_SERVICE_DIR` 指定程序所在目录。

## Agent 会话

每个 Agent 是一个比进程活得久的会话。服务按需启动宿主机自己的 CLI，用 CLI 保存的会话 ID 续聊；进程空闲 15 分钟后退出，下一条消息到来时再拉起。

- **Claude Code**：`claude -p` 的双向 stream-json。回复和思考按增量推送，工具调用带输入、输出和 diff；需要授权时，CLI 的 `can_use_tool` 请求变成客户端的审批卡（工具、计划、提问三种）。模式切换、换模型和中断都通过控制请求作用于正在运行的进程，回合进行中发来的消息会并入当前回合。
- **Codex**：`codex app-server` 的 JSON-RPC。一个线程对应一个会话，一条消息对应一个回合；命令执行、文件修改和提问的审批同样变成审批卡。默认模式沿用 `~/.codex/config.toml` 里的审批与沙箱设置，`read-only` 和 `full-access` 模式会覆盖它。

### 账号与接口

Agent 用的账号和接口由 CLI 自己决定，也就是它的登录状态和配置文件，与在终端里直接运行 `claude` 或 `codex` 一致。

服务和桌面应用启动时都会清掉自身环境里的 `ANTHROPIC_*`、`CLAUDE_CODE_*`、`CLAUDE_AGENT_*`、`CODEX_*`，以及 `CLAUDECODE`、`CLAUDE_EFFORT`、`CLAUDE_PID`。这些变量通常来自拉起服务的那个 Agent 会话（比如在某个 Agent 的终端里启动了应用），留着会让新 Agent 走那个会话的网关和令牌。`CLAUDE_CONFIG_DIR` 和 `CODEX_HOME` 会保留。

要让某个提供商走别的接口，在服务配置里给它写 `env`，例如 `{"providers":{"gateway":{"extends":"claude","env":{"ANTHROPIC_BASE_URL":"…","ANTHROPIC_AUTH_TOKEN":"…"}}}}`。在无界面主机上用 API key 时也是这样配置。

## 从旧服务升级

服务第一次在 `~/.sophon/service` 启动时（还没有 `state.json`），会自动把旧服务在 `~/.paseo` 里保存的内容带过来，之后不再重复：

- 项目、工作区、会话（ID、标题、模型、思考强度、权限模式、归档状态和提供商会话 ID），旧服务内部用的会话除外。
- 定时任务和各自的运行记录。到期时间已经过去的任务从现在起重新排期，不会一启动就补跑；发往已有会话的任务会以暂停状态带过来，因为本服务每次运行都新建会话。
- 中继地址，以及基于 Claude Code 或 Codex 的提供商配置。
- 其他空间：`~/.paseo-spaces/<名字>` 对应到 `~/.sophon/spaces/<名字>`，并保留各自的监听端口。

旧目录只读不改，退回旧版本时数据仍在。如果旧服务还在运行，会先停掉它，避免它的定时任务和带过来的重复执行。

**聊天记录**不是复制过来的：新旧服务都不保存它，保存它的是 CLI 自己的会话文件。带过来的会话在第一次打开时，按它的会话 ID 读取对应的那一个文件（Claude Code 的 `projects/*/<会话ID>.jsonl`，Codex 的 `sessions/**/rollout-*-<线程ID>.jsonl`）。只读取 Sophon 自己记录在案的会话，不会扫描或列出电脑上其他的 Claude Code、Codex 会话。CLI 的会话文件如果已被清理，对应会话仍可继续，只是看不到之前的消息。

其他位置的服务目录（`--home` 指向别处）不会自动带入，可以手动导入一次，目标目录必须为空：

```sh
target/debug/sophon-service daemon stop
target/debug/sophon-service import "$HOME/.paseo" --home DIR
target/debug/sophon-service daemon start
```

## 当前覆盖

| 功能 | 原生服务 |
| --- | --- |
| 会话 | Claude Code 和 Codex 的流式回复、工具调用、审批卡（含“始终允许”）、权限模式、换模型与思考强度、用量、回合中追加消息、中断、重启后续聊、时间线分页 |
| 回退与分叉 | 回到某条消息之前：Claude Code 可回退文件、对话或两者；Codex 只能回退对话。把一段对话作为上下文带进新会话 |
| 后台任务 | 列出并停止 Claude Code 在后台运行的命令和子 Agent |
| 导入会话 | 列出 CLI 里尚未加入 Sophon 的会话，按需逐个导入 |
| 子 Agent | Agent 派出的子 Agent 列表、状态，以及各自独立的时间线 |
| 斜杠命令 | Claude Code 在该目录下可用的命令、技能和插件命令；Codex 不提供列表 |
| 项目与工作区 | 注册已有目录、从 GitHub 克隆、创建本地工作区或 Git worktree、创建 Agent、重命名工作区、置顶、归档与恢复、清除提醒、找回丢失的工作区目录、Agent 归档与历史搜索 |
| 终端 | 真实 PTY、输入、调节行列数、断线后重放最近 256 KiB、结束终端 |
| 文件 | 目录列表、文本预览、冲突检查、保存、新建、改名、复制、删除，单文件最多 64 MiB 的二进制传输 |
| Git | 状态、分支列表和切换、订阅工作区 diff、提交、推拉、合并到基准分支、stash、提交列表与文件 diff。单个文件超过 2 万行改动或 1 MiB 补丁、整个工作区超过 8 MiB 时，超出的文件只列出增删行数 |
| GitHub | 通过宿主机登录的 `gh` 查看、创建和合并 PR |
| 定时任务 | 带时区的 cron 和固定间隔，暂停、恢复、立即执行与运行记录 |
| Codex 额度 | 使用宿主机的 `codex app-server` 查询用量窗口、列出额度重置券，按用户选择和请求幂等键兑换 |

宿主功能以 Sophon 的实际工作流为准。[客户端协议清单](parity.md) 用于查询协议覆盖情况。服务随应用发布，设置里的“重启”和“更新”都是让它重新启动。提供商诊断给出 CLI 的路径、版本、配置目录和默认模型。其他提供商、提供商功能开关、工作区脚本和 Claude 额度重置按实际需求排期。

会话集成测试使用说同一协议的模拟 CLI。另有两项测试驱动宿主机上真实的 Claude Code 和 Codex，会消耗账号额度，默认不运行：

```sh
cargo test -p sophon-service --test session live_ -- --ignored --test-threads 1
```

客户端发送消息时沿用 Paseo 的 `messageId`，宿主借鉴 T3 Code 的命令回执，返回消息 ID 和这条消息在时间线里的序号 `seq`。在同一会话中，即使连接断开或宿主重启，用同一 ID 重发也会得到原回执，模型只收到一次调用。客户端应在第一次发送前生成 ID，并在重试时复用它。
