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
- **Antigravity**：Google 的 Antigravity CLI（`agy acp`，取代个人账号已停用的 Gemini CLI），走 Agent Client Protocol，启动和授权方式照 T3 Code 的做法：
  - 要求 CLI 版本至少 1.5.1、低于 2。
  - 每个提供商有自己的目录（`GEMINI_HOME`，默认 `~/.sophon-antigravity`），用 `--gemini-dir` 交给 CLI。
  - 登录凭据以文件形式存在这个目录里（`AGY_ACP_FORCE_FILE_STORAGE=1`），不进所有目录共用的钥匙串。
  - 服务环境里的 `GEMINI_API_KEY`、`GOOGLE_*` 等变量不会传给它。
  - CLI 要打开浏览器时只记一个标记，不打开，也不显示链接。没登录的会话会提示去“设置 › Agents”登录；登录用 CLI 自己的 `agy acp --login`。
  - 服务不读凭据，也不直接调 Google 的接口。
  - 模型和思考强度取自会话的 `configOptions`。
  - 读取、搜索直接放行，其余按所选模式决定是否弹审批卡。
  - 回合中追加的消息会并入当前回合。
  - 停止时等 CLI 结束回合，15 秒内没结束就结束进程。

### 账号与接口

Agent 用的账号和接口由 CLI 自己决定，也就是它的登录状态和配置文件，与在终端里直接运行 `claude` 或 `codex` 一致。

"设置 › 编程 Agent"可以添加提供方：另一个账号（Claude Code、Codex、Antigravity），或一个 API 接口。每个账号是一个独立的提供方，有自己的配置目录和登录，Sophon 不会在账号之间自动切换。

Codex 的登录状态由服务向 `codex app-server` 询问（`account/read`），诊断信息里的 `Auth` 会写明是哪个 ChatGPT 账号和套餐。Codex 账号可以在 Sophon 里直接登录，用的是 Codex 自己的登录流程（`account/login/start`，和 T3 Code 一样）：
- 在运行服务的这台 Mac 上，登录页在浏览器里打开，Codex 在本机等待回调；
- 在别的设备上，改用验证码登录：在任意设备上打开登录页，输入显示的验证码。

服务端对应三个请求：`provider.login.start.request` 开始登录，返回登录页地址或验证码；`provider.login.wait.request` 等到登录结束，返回是否成功；`provider.login.cancel.request` 取消登录。同一个提供方同时只进行一次登录，等待超过 15 分钟自动结束。服务不读取凭据，凭据由 Codex 写进该账号的 `CODEX_HOME`。Claude Code 和 Antigravity 仍然用各自 CLI 的登录命令，在这台 Mac 上可以一键在终端里运行。

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
| 项目与工作区 | 注册已有目录、创建本地工作区或 Git worktree、创建 Agent、重命名工作区、置顶、归档与恢复、清除提醒、找回丢失的工作区目录、Agent 归档与历史搜索 |
| 终端 | 真实 PTY、输入、调节行列数、断线后重放最近 256 KiB、结束终端 |
| 文件 | 目录列表、文本预览、冲突检查、保存、新建、改名、复制、删除，单文件最多 64 MiB 的二进制传输 |
| Git | 状态、分支列表和切换、订阅工作区 diff、提交、推拉、合并到基准分支、stash、提交列表与文件 diff。单个文件超过 2 万行改动或 1 MiB 补丁、整个工作区超过 8 MiB 时，超出的文件只列出增删行数 |
| GitHub | 通过宿主机登录的 `gh` 查看、创建和合并 PR |
| 自动化 | 带时区的 cron 和固定间隔，暂停、恢复、立即执行与运行记录；也可以由集成的触发器或 `sophon trigger` 触发；每次运行开一个对话，或只执行一条命令，见[自动化](#自动化) |
| Codex 额度 | 使用宿主机的 `codex app-server` 查询用量窗口、列出额度重置券，按用户选择和请求幂等键兑换 |

宿主功能以 Sophon 的实际工作流为准。[客户端协议清单](parity.md) 用于查询协议覆盖情况。服务随应用发布，设置里的“重启”和“更新”都是让它重新启动。提供商诊断给出 CLI 的路径、版本、配置目录和默认模型。其他提供商、提供商功能开关、工作区脚本和 Claude 额度重置按实际需求排期。

会话集成测试使用说同一协议的模拟 CLI。另有两项测试驱动宿主机上真实的 Claude Code 和 Codex，会消耗账号额度，默认不运行：

```sh
cargo test -p sophon-service --test session live_ -- --ignored --test-threads 1
```

客户端发送消息时沿用 Paseo 的 `messageId`，宿主借鉴 T3 Code 的命令回执，返回消息 ID 和这条消息在时间线里的序号 `seq`。在同一会话中，即使连接断开或宿主重启，用同一 ID 重发也会得到原回执，模型只收到一次调用。客户端应在第一次发送前生成 ID，并在重试时复用它。

## 自动化

客户端里的「自动化」（原来的定时任务）由两部分组成：什么时候开始，和开始后做什么。

**做什么**有两种：

- **开对话**：用任务的提示词启动一个 Agent，适合需要判断的事。
- **运行命令**（`command`）：在项目目录里用 `sh -c` 执行一条命令，不启动 Agent，不消耗模型。输出的末尾 4000 个字符记在运行记录里，退出码非零记为失败，最长运行 10 分钟。命令拿到的环境变量和回话脚本一样（`SOPHON_EVENT_ID`、`SOPHON_VAR_<NAME>` 等），事件的内容只通过环境变量传入，不会拼进命令里。

`dailyLimit` 限制一条自动化每天（按这台电脑的日期）最多开始多少次运行，不管是到点、探测到事件还是 `sophon trigger`：到点的那次直接跳过，探测到的事件留到第二天探测时还在的话再处理，`sophon trigger` 被拒绝。手动点「立即运行」不受限制。

没有 `cadence` 的自动化不会自己运行，只等 `sophon trigger` 或手动运行。

### 事件触发

自动化除了到点运行，还可以在有事发生时运行。服务不关心事件从哪来、回话发到哪去，这两件事各由一个脚本负责，脚本在任务的项目目录里用 `sh -c` 执行，30 秒超时。

**探测脚本**（`probe`）。到了任务的时间，服务运行探测脚本而不是直接开对话。脚本每输出一行 JSON 就是一个事件：

```json
{"id":"om_abc","vars":{"url":"https://github.com/acme/app/pull/12","sender":"张三"}}
```

- 同一个 `id` 只触发一次，服务为每个任务记住最近 500 个，所以脚本可以不存状态，每次输出最近一段时间的全部结果。
- `vars` 填进提示词里的 `{{url}}`、`{{sender}}`，`{{id}}` 是事件的 id。
- 任务启用后的第一次探测只记下已有的事件，不触发；暂停后恢复也一样。点“立即运行”会马上探测并触发没见过的事件。
- 同一个任务同时最多跑 2 个事件（`concurrency`），超出的留给下一次探测。
- 输出里有不是 JSON 的行、缺 `id`、退出码非零或超时都算失败，原因显示在任务上；连续失败 5 次自动暂停。

**命令触发**。任何程序都可以让一个任务运行一次，任务按 id 或名字指定：

```sh
sophon trigger "PR 值守" --id om_abc --var url=https://github.com/acme/app/pull/12
```

**回话脚本**（`reply`）。事件触发的对话里，Agent 的 `PATH` 上有 `sophon`，环境里有这次运行的令牌，它可以对触发它的那个事件回话：

```sh
sophon reply "已评论"
sophon react DONE
```

服务把这些交给回话脚本，并在运行的几个节点自己调用它。脚本从环境变量读取：

| 变量 | 内容 |
|---|---|
| `SOPHON_ACTION` | `ack`（已接手）、`reply`、`react`、`done`（结束，`SOPHON_TEXT` 是 Agent 最后一条回复）、`failed`（失败，`SOPHON_TEXT` 是原因） |
| `SOPHON_TEXT` | 回复的文字或表情名 |
| `SOPHON_EVENT_ID`、`SOPHON_EVENT` | 事件的 id，和整个事件的 JSON |
| `SOPHON_VAR_<NAME>` | 事件的每个 `vars`，名字转成大写 |

令牌只对应它自己那次运行，Agent 拿不到回话脚本用的凭据。没有回话脚本时，回话只记在运行记录里。`sophon` 是 `<服务目录>/bin/sophon`，指向服务自己的程序；提供商配置里的 `env` 如果改了 `PATH`，要把这个目录带上。Codex 的沙箱禁止联网时，`sophon` 连不上服务。

### 集成与插件

自动化之外有两样可以安装和分享的东西，各管一件事：

| | 是什么 | 清单文件 |
|---|---|---|
| **集成** | 连上一个平台：授权、能调用的命令、能盯着的事（触发器）。不关心拿来做什么 | `.sophon-plugin/integration.json` |
| **插件** | 别人做好的自动化，打包起来分享：提示词、用哪个集成的触发器、怎么回话、有哪些选项。可以自带集成、技能和 MCP 服务 | `.sophon-plugin/plugin.json` |
| **自动化** | 用户自己的、正在跑的任务。记着启动它的触发器；用插件建的还记着是插件的哪一条 | 存在服务的状态里 |

一个文件夹的 `.sophon-plugin/` 里放哪份清单，它就是哪一种。两种的 id 共用一个名字空间。

清单的位置和内容的组织方式跟 Claude Code、Codex 的插件一致：清单在一个点开头的文件夹里（它们是 `.claude-plugin/`、`.codex-plugin/`），只写元数据和自己的设置；内容靠文件夹约定，放在那里就算，不用在清单里登记。技能（`skills/`）和 MCP 服务（`.mcp.json`）直接用它们的格式，所以同一个文件夹可以同时是三家的插件。Sophon 多出来的是它们没有的两样：触发器（归集成）和自动化（归插件）。

两种清单都可以写 `interface`（字段名沿用 Codex）：`brandColor` 是 `#rrggbb` 的品牌色，`logo` 是文件夹里一张图片的路径（png、jpg、webp 或 gif，不超过 256 KB）。卡片上有图片就显示图片，否则在品牌色上显示名字的首字。清单没写 `name`、`description` 时用 `displayName`、`shortDescription`。

**集成的清单**：

```json
{
  "id": "github",
  "name": "GitHub",
  "description": "…",
  "params": [
    {"name": "token", "label": "访问令牌", "secret": true}
  ],
  "setup": "./watch login",
  "status": "./watch status",
  "use": "./watch use",
  "triggers": [
    {
      "id": "review-requested",
      "label": "有人请求我 review",
      "description": "GitHub 上有人把你设为 PR 的 reviewer 时",
      "probe": "./watch probe",
      "everyMs": 120000,
      "params": [
        {"name": "repos", "type": "multi", "label": "只处理这些仓库", "optionsFrom": "./watch repos", "optional": true}
      ]
    }
  ]
}
```

- `triggers` 是集成盯着的事，每项有 `id`、给人看的 `label` 和背后的 `probe`（就是上面「事件触发」里的探测脚本，可带 `everyMs`）。客户端新建自动化时按名字列出来供选择，用户不用看脚本。触发器还可以带 `reply`：自动化自己没有回话方式时，由它回给引发事件的人。
- `use` 是集成提供给别人的命令。插件的脚本、自动化里的 Agent 用 `sophon use <id> <参数…>` 调用，由服务以这个集成的身份运行，凭据不外传。
- `setup` 是装好后要运行一次的命令，比如登录；在客户端的「集成」页里点按钮时由服务运行，没有终端，所以它要自己打开浏览器并等到完成。`setupLabel` 是按钮上的字。`status` 是一条命令，用一行字说明现在是谁的账号，还没设置好时失败。

**插件的清单**（`.sophon-plugin/plugin.json`）只有元数据和它自己的设置：

```json
{
  "id": "github-pr-watch",
  "name": "GitHub PR 值守",
  "description": "…",
  "version": "0.6.0",
  "requires": ["github", "feishu"],
  "use": "./notify use",
  "interface": {"brandColor": "#1f883d"},
  "params": [
    {"name": "people", "label": "GitHub 账号对应的飞书账号", "optional": true}
  ]
}
```

内容按文件夹放：

```
github-pr-watch/
  .sophon-plugin/plugin.json
  automations/review/automation.json   每条自动化一个文件夹，文件夹名就是它的 id
  automations/review/prompt.md
  skills/pair-feishu-account/SKILL.md  技能
  .mcp.json                            MCP 服务（可选）
  integrations/<id>/                   自带的集成（可选）
  notify                               脚本
```

**一条自动化**（`automations/<id>/automation.json`）：

```json
{
  "category": "review",
  "trigger": "github/review-requested",
  "reply": "./notify reply",
  "modeId": "auto",
  "concurrency": 2,
  "params": [
    {"name": "approve", "var": "rule", "type": "switch", "label": "没有阻塞问题时直接 approve", "value": false,
     "prompt": {"yes": "没有阻塞问题就 approve…", "no": "只评论，不要 approve…"}}
  ]
}
```

- `automations/` 下每个带 `automation.json` 的文件夹是一条自动化，可以有几条，按文件夹名排序。提示词是同一文件夹里的 `prompt.md`；也可以用 `prompt` 指定那个文件夹里的另一个文件，或直接写提示词本身。
- `trigger` 写成 `集成 id/触发器 id`，不写就是按时间运行。`reply` 是回话脚本，和插件的其他命令一样在插件自己的文件夹里执行，所以脚本放在自动化文件夹里时写 `automations/<id>/脚本名`。`modeId`、`concurrency` 是建议值。`category` 是 `review` 时归在「代码审查」下，其余归在「维护」下。`name`、`description` 不写就用插件的。
- 插件文件夹里的 `skills/<名字>/SKILL.md` 是它带的技能，写法和 Claude Code 的技能一样（文件开头写 `name` 和 `description`）。`.mcp.json` 是它带的 MCP 服务，也是 Claude Code 的写法（`{"mcpServers": {…}}`）。用这个插件建的自动化启动 Claude Code 时，服务把两样一起交给它（`claude --plugin-dir`）：插件文件夹本身已经有 `.claude-plugin/plugin.json` 时原样交过去，否则在 `plugin-data/<id>/claude-plugin` 里现做一个，`.mcp.json` 里的 `${CLAUDE_PLUGIN_ROOT}` 换成插件自己的文件夹。别的 Agent 没有这个入口：技能在提示词后面列出文件位置让它用到时去读，MCP 服务目前只交给 Claude Code。
- `requires` 列出它依赖的集成（触发器所属的集成会自动算进去）。客户端在插件卡片上显示每个依赖有没有安装、设置好没有。
- 插件文件夹里的 `integrations/<id>/` 如果有 `.sophon-plugin/integration.json`，就是它自带的集成：插件在它就在，插件关掉它也关掉，随插件一起卸载。本机已经单独装了同 id 的集成时，用单独装的那个。
- 插件没有登录这一步（`setup`、`status`、`triggers` 都是集成的），写了会报错并说明该放在哪。

**参数**。两种清单都可以要参数，写在哪里决定它归谁：

- 清单顶层的 `params` 是集成或插件自己的设置，填一次，在各自的页面里填。只传这几个值时可以用一行文字：`sophon plugin export <id>` 输出一行以 `sophon-integration:` 开头的文字，里面有 id 和每个参数的值，**包括密钥**；对方 `sophon plugin import <那一行>`，或在「集成」页的添加框里粘贴。整套交给别人用下面的文件。
- 触发器的 `params` 和插件自动化的 `params` 由每条自动化各自保存（分别在自动化的 `triggerOptions` 和 `options` 里），客户端在自动化编辑器的触发器下面显示它们。
- 每项可以有 `type`：`text`（默认）、`switch`（开是 `yes`，关是 `no`）、`multi`（多选，值用逗号连接）。多选的选项写在 `options` 里，或由 `optionsFrom` 指定的命令输出，每行一个，可以是文字或 `{"value","label","hint"}`。
- 脚本通过 `SOPHON_PARAM_<NAME>` 拿到参数。探测脚本拿到的是集成自己的参数加这条自动化给触发器填的；回话脚本拿到的是插件自己的参数加这条自动化给插件填的。两边互相看不到。
- 插件自动化的参数可以用在提示词里：`{{名字}}` 会换成它的值。写了 `"prompt": {"yes": "…", "no": "…"}` 时换成这个值对应的那段话；写了 `var` 时提示词里用 `{{var 的值}}` 引用。事件带的同名变量优先。
- 脚本另外拿到 `SOPHON_PLUGIN_DIR`（自己的文件夹）、`SOPHON_PLUGIN_DATA`（自己存东西的地方，`<服务目录>/plugin-data/<id>`，更新不会动它）和 `SOPHON_PROJECT_DIR`（任务的项目目录）。

连接、填参数、授权在客户端侧边栏的 **集成** 页里完成，插件在 **插件** 页；新建自动化在 **自动化** 页，可以从插件带来的模板开始，也可以自己选一个集成的触发器、写自己的提示词。Sophon 的终端里也有同样的命令，两种都用 `sophon plugin`（`sophon integration` 是同一个命令）：

```sh
sophon plugin add <git 地址或本机文件夹>   # git 地址克隆一份；本机文件夹是链接，改了立即生效
sophon plugin setup <id> [参数]            # 运行集成的 setup
sophon plugin set <id> name=value          # 填它自己的参数
sophon plugin update <id>                  # git 安装的拉取更新
sophon plugin list                         # 每行：id、是集成还是插件、名字、说明
sophon plugin remove <id>                  # 还有自动化在用时拒绝
sophon plugin pack <id> [文件]             # 存成一个 .sophon 文件；--no-secrets 不带密钥，--no-settings 不带设置
sophon plugin add <文件.sophon>            # 从这样的文件添加：先列出里面有什么，再安装
```

**存成文件交给别人**。Sophon 没有可以安装插件的服务器，也没有官方应用可以授权，所以配好的东西靠一个文件传：卡片上的「存为文件用于分享」（或 `sophon plugin pack`）生成一个 `.sophon` 文件，对方在「集成」或「插件」页点「选择文件…」（或 `sophon plugin add`）。文件是一个 gzip 压缩的 tar：

- `sophon.json`：里面有什么。这个文件是关于哪个包的、它依赖的每个包、每个包的设置、一条配好的自动化（可选）。
- `packages/<id>/…`：每个不是随 Sophon 自带的包的整个文件夹（不含 `.git`）。自带的包只记名字，不打包。插件自带的集成在插件的文件夹里，跟着一起走。

设置是每个包在这台电脑上填过的值，可以全带、不带密钥、或都不带。个人的登录（每个包在 `plugin-data/<id>` 里存的东西）永远不在文件里，对方导入后自己授权一次。自动化带的是名字、提示词、触发器和各项选项、运行时间；在哪个项目里跑、用哪个模型是对方电脑上的事，不在文件里。

导入分两步，因为包会在那台电脑上运行脚本：先读（`plugin/unpack`），服务把文件解到 `plugins/.handed-<token>/` 这个不会运行任何东西的地方，回一份清单：会安装哪些包、哪些会替换已有的、会填哪些设置（只有名字，没有值）、带不带自动化；客户端把这份清单给人看过，确认后才安装（`plugin/install`）：包放到 `plugins/<id>`（同 id 的被替换，它存的东西留着）、填好设置、打开开关。带了自动化时客户端打开一张填好的新建表单，选好项目点创建才真正建出来。读了没装的，一小时后清掉。

文件是别人给的，所以里面的每个路径都要检查：只能是 `sophon.json` 或 `packages/<合法 id>/` 下的普通文件，不能有 `..`、绝对路径、链接、`.git`；解出来的总大小（64 MB）和文件数（4000）有上限，文件本身不超过 8 MB。不符合的整个拒绝，不留下任何东西。文件里的密钥是明文，只发给信任的人。

仓库的 `plugins/` 目录下是随 Sophon 自带的：集成 GitHub、飞书，插件 GitHub PR 值守。打包时放到服务程序旁边的 `plugins` 文件夹，不用安装也不能卸载；开发时用 `SOPHON_BUNDLED_PLUGINS` 指向这个目录。装一个同 id 的包会替换自带的那个。

自动化只记下集成、触发器、插件和它是插件的哪一条的 id，脚本每次运行时从它们的文件夹里读，所以更新后下一次探测就用新的脚本。提示词在创建时从插件抄一份，之后是这条自动化自己的，可以随意改。安装集成或插件就是允许它的脚本在这台电脑上运行，只安装信任的来源。

早先的版本里只有「插件」一种，一条自动化记的是 `plugin` 和 `pluginTrigger`。服务启动时会把这样的自动化改成新的写法：原来的那个包现在是集成，它的触发器照旧启动这条自动化；如果有插件的自动化用的正是这个触发器，就认它作来源，原来填的选项分到各自该在的地方。旧写法读不出来，列表里会说明该怎么改：清单还放在文件夹顶层的（`sophon-integration.json`、`sophon-plugin.json`）、插件清单里带 `triggers`、`automation` 或 `setup` 的、自动化还列在清单的 `automations` 里的。
