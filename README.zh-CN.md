<p align="center">
  <img src="assets/app-icon/icon-macos-1024.png" width="128" alt="Sophon 图标">
</p>

<h1 align="center">Sophon · 智子</h1>

<p align="center">
  一个大脑，所有设备：编程智能体的原生客户端，支持 macOS 和 iOS。
</p>

<p align="center">
  <a href="https://github.com/caoxing9/sophon/actions/workflows/ci.yml"><img src="https://github.com/caoxing9/sophon/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://github.com/caoxing9/sophon/releases/latest"><img src="https://img.shields.io/github/v/release/caoxing9/sophon" alt="Release"></a>
  <a href="README.md">English</a>
</p>

---

Sophon 通过自带的 Rust 服务运行电脑上的 Claude Code 和 Codex。你可以
在 Mac 或手机上启动智能体、跟进进度、审查改动、回答它们的提问，所有设备连的都是同一个守护进程。
Sophon 用 Rust 和 [NSH](https://github.com/longbridge/nsh) 编写，在每个平台上都是真正的原生应用，不是网页套壳。

> **名字的由来。** 在《三体》里，智子是一个被展开成超级计算机的质子：一粒无处不在的智慧，跨越任何距离实时同步。
> Sophon 为一个人做同样的事：同一批智能体、同一份上下文，出现在你的每一台设备上。

## 功能

- **项目与工作区**：本地文件夹或 git worktree，新建分支；置顶、重命名、归档
- **智能体即标签页**：一个工作区多个智能体；切换模型、模式和思考深度；斜杠命令、`@` 引用文件、图片附件；排队、插话或打断
- **实时时间线**：流式输出、工具调用、计划、权限与提问卡片、子智能体、后台任务，支持回退和分叉
- **代码审查**：改动列表、并排/统一 diff、逐行评论发回给智能体；提交记录；提交、推送、拉取、合并、贮藏；创建和合并 PR 并查看检查状态
- **终端与文件**：像 VS Code 一样的底部终端面板（⌃` 或 ⌘J）、文件树和编辑器；工作区里的任意文件都能下载到当前设备：Mac 上弹出保存面板，iPhone 上存进「文件」App 里的 Sophon 文件夹，局域网和中继都能用
- **自动化、历史与用量**：到点或有事发生时开对话或运行命令，可安装别人分享的插件；已归档的智能体、各服务商的用量窗口
- **随处可用**：扫码配对 iPhone；局域网直连，或在任何地方通过中继连接，端到端加密（Noise）
- **键盘优先**：命令面板（⌘K）和快捷键，在输入框里打字时也能用；智能体需要你时发送桌面通知
- **自动更新**：有新版本时侧边栏会出现更新按钮

宿主服务的支持范围见[服务指南](docs/native-service.md)，客户端功能见[协议清单](docs/parity.md)。

## 安装

macOS 应用包自带 `sophon-service`，按需启动。源码开发时先运行 `cargo build -p sophon-service`，在存放代码的电脑上安装并登录要使用的 Agent CLI。智能体使用 CLI 自己的登录和配置，和在终端里直接运行 `claude`、`codex` 一致。

### 会话存在哪里？

项目、工作区和对话时间线保存在运行 `sophon-service` 的 Mac 上。iPhone 显示当前所选宿主的会话。切换宿主后，侧边栏会显示另一份列表；切回原宿主即可继续查看原来的对话。局域网直连和中继只是到达同一宿主的两条路径。

| 宿主 | Mac 上的数据目录 |
|---|---|
| 使用 Paseo 的旧版 | `~/.paseo/` |
| Rust 服务 | 默认 `~/.sophon/service/` |
| 单独安装的本地测试版 | 可用 `SOPHON_SERVICE_HOME` 指定独立目录，例如 `~/.sophon-beta/service/` |

**从 Paseo 迁移。** 先保留旧目录，再将项目、工作区和会话 ID 导入空的目标服务。提供商的本地会话仍在时，可以接着聊。迁移前的逐条消息和定时任务仍需通过旧宿主查看。[导入步骤](docs/native-service.md#导入旧数据)

### macOS（14 Sonoma 及以上，Apple 芯片）

**Homebrew**

```sh
brew tap caoxing9/sophon https://github.com/caoxing9/sophon
brew install --cask sophon
```

**一行命令**

```sh
curl -fsSL https://raw.githubusercontent.com/caoxing9/sophon/main/install.sh | sh
```

默认装到 `/Applications`（没有写权限时装到 `~/Applications`）。可选环境变量：`SOPHON_VERSION=0.1.0` 指定版本，
`SOPHON_DIR=…` 指定目录，`SOPHON_NO_OPEN=1` 装完不自动打开。

**手动安装**：在 [Releases](https://github.com/caoxing9/sophon/releases/latest) 下载
`Sophon-<版本>-macos-aarch64.dmg`。

> Sophon 目前只做了 ad hoc 签名，还没有经过 Apple 公证。如果 macOS 拒绝打开，运行一次下面的命令即可（Homebrew 和安装脚本会自动处理）：
>
> ```sh
> xattr -dr com.apple.quarantine /Applications/Sophon.app
> ```

**更新**：Sophon 启动后不久以及之后每 6 小时检查一次 GitHub Releases。有新版本时，侧边栏底部会出现
*Update to x.y.z* 按钮，点一下即可下载、校验、替换并重启。Homebrew cask 安装的始终是最新版本，之后由应用自己更新（想从 Homebrew 更新可以用 `brew upgrade --cask --greedy sophon`）。

### iOS

要从源码装到自己的 iPhone 上，在 Xcode 登录 Apple ID（*Settings → Accounts*），连上 iPhone，打开
*开发者模式*（*设置 → 隐私与安全性*），准备好构建工具（见下文），然后运行

```sh
xcrun devicectl list devices
SOPHON_DEVICE="<目标 iPhone 的 UDID>" scripts/ios-device.sh
```

它会生成开发证书和描述文件，构建、签名并安装 Sophon。第一次需要在 iPhone 上信任你的 Apple ID
（*设置 → 通用 → VPN与设备管理*）。免费签名 7 天有效，到期后再运行一次即可。

**连接 Mac。** 在要连接的 Mac 应用中打开 *设置 → 配对设备*，用 iPhone 扫描二维码。正式版与 Beta 并排运行时，请扫对应版本的码。之后轻点手机首页的主机名称，即可从底部列表切换已配对的电脑。

同一局域网可以直连。要从其他网络访问，请先在对应 Mac 应用的设置里填入自托管 `sophon-relay` 地址，再扫码，让配对链接带上中继信息。已有的 v2 中继可以继续使用。

## 从源码构建

需要：装有 Xcode 的 macOS、[Rust](https://rustup.rs)（stable）和 Git。

Sophon 依赖 `Cargo.toml` 中固定的 NSH 提交，`nsh` 命令行也必须来自同一个提交（已发布的 `nsh` 可能比这个提交旧）。
下面的脚本会构建它并输出可执行文件所在的目录：

```sh
export PATH="$(scripts/install-nsh.sh):$PATH"
```

然后：

```sh
nsh run                          # 在本机运行；修改 Rust 代码会自动重新构建并重启
nsh run --target ios             # 在 iOS 模拟器里运行
nsh check                        # cargo check 并对 App.test.tsx 做类型检查
cargo test --locked              # Rust 测试
nsh test App.test.tsx            # 无界面 UI 测试
nsh package --target macos       # 打包为 .nsh/out/Sophon.app
nsh package --target ios-simulator --icon assets/app-icon/icon-ios-1024.png
```

也可以用 `cargo run` 直接在本机桌面后端（macOS 上是 AppKit）启动。

### 目录结构

| 路径 | 内容 |
|---|---|
| `crates/sophon-service/` | 宿主服务（守护进程）：运行 Claude Code 和 Codex，保存会话，操作文件、Git 和终端 |
| `relay/` | 中继服务：在手机和电脑之间转发端到端加密的连接，可自托管 |
| `src/daemon/` | 客户端一侧与守护进程的连接：WebSocket 协议、中继端到端加密、供手机连接的局域网桥 |
| `src/store.rs` | 用 signal 管理的 UI 状态 |
| `src/ui/kit/` | 设计系统：控件、图标、菜单、设计变量 |
| `src/ui/shared/` | 两端公用的组件：对话时间线、输入框、智能体页面、项目列表、弹窗、设置分区 |
| `src/ui/desktop/` | 桌面端独有：侧边栏、标签页、文件浏览、Git、终端、新建工作区、历史、自动化、插件、快捷键 |
| `src/ui/mobile/` | 手机端独有：配对引导、可滑动的三栏（工作区 / 智能体 / 改动）、精简设置 |
| `src/update.rs` | 从 GitHub Releases 自动更新 |
| `App.test.tsx` | 根组件的无界面 UI 测试，由 `nsh test` 运行 |
| `assets/app-icon/` | 应用图标：SVG 源文件，`build.sh` 重新生成所有尺寸 |
| `nsh.json` | 应用名、标识、版本、图标、URL scheme、更新源 |
| `Casks/`、`install.sh`、`scripts/` | Homebrew cask、安装脚本、版本与发布辅助脚本 |

## 发布

```sh
scripts/set-version.sh 0.2.0
git commit -am "Release 0.2.0"
git tag v0.2.0
git push origin main v0.2.0
```

[发布工作流](.github/workflows/release.yml) 会构建 macOS 应用（Apple 芯片和 Intel）和 iOS 模拟器应用，发布 GitHub
Release 和 `latest.json`（已安装的应用从这里获取更新）。Homebrew cask 不再随版本改写，它按每个版本都带的固定文件名下载最新版本。每个版本包含：

| 文件 | 用途 |
|---|---|
| `Sophon-<v>-macos-aarch64.dmg` | 手动安装 |
| `Sophon-<v>-macos-aarch64.tar.gz` | 安装脚本和应用内更新 |
| `Sophon-macos-aarch64.tar.gz` | 同一个安装包，供 Homebrew cask 下载 |
| `Sophon-<v>-ios-simulator.zip` | iOS 模拟器 |
| `latest.json` | 更新源 |

配置了 App Store 签名密钥（`IOS_CERT_P12`、`IOS_CERT_PASSWORD`、`IOS_PROFILE`、`ASC_APP_ID`、`ASC_ISSUER_ID`、
`ASC_KEY_ID`、`ASC_KEY_P8`）后，还会构建签名的 `Sophon.ipa` 并上传到 TestFlight。

[CI](.github/workflows/ci.yml) 会在每次推送和 PR 时运行：检查、Rust 测试和 UI 测试，并打包 macOS 和 iOS 模拟器版本
（可在运行记录的 Artifacts 中下载）。

## 致谢

- [NSH](https://github.com/longbridge/nsh) 是 Sophon 所用的原生 UI 框架。
- 名字取自刘慈欣的《三体》。Sophon 是爱好者项目，与原著版权方和 NSH 均无关联。
