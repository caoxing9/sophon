# 随 Sophon 自带的集成和插件

这里的每个文件夹是一个集成或一个插件，看它的 `.sophon-plugin/` 里放的是哪份清单：

- `.sophon-plugin/integration.json`：**集成**，连上一个平台。管授权、能调用的命令和能盯着的事（触发器），不关心拿来做什么。
- `.sophon-plugin/plugin.json`：**插件**，别人做好的自动化。清单只写名字、依赖和它自己的设置；内容按文件夹放：`automations/<id>/`（`automation.json` 加 `prompt.md`）、`skills/<名字>/SKILL.md`、`.mcp.json`。

清单放在 `.sophon-plugin/` 下，和 Claude Code 的 `.claude-plugin/`、Codex 的 `.codex-plugin/` 并排，技能和 `.mcp.json` 也是它们的格式，所以同一个文件夹可以同时是三家的插件。

打包时它们被放到服务程序旁边，服务启动后直接可用，不需要安装。

| 文件夹 | 种类 | 是什么 |
|---|---|---|
| `github` | 集成 | 用本机 `gh` 的登录。提供触发器「有人请求我 review」 |
| `feishu` | 集成 | 用你本人的飞书身份登录一次，供自动化按名字找同事、给同事发消息 |
| `github-pr-watch` | 插件 | 「GitHub PR 值守」：有人请求你 review 时替你 review，结论提交在 PR 上，并在飞书通知作者。依赖上面两个集成 |

清单的写法见 [服务指南](../docs/native-service.md#集成与插件)。在「集成」页里安装一个同 id 的包会替换这里自带的那个。

要把配好的东西交给同事，不用替换自带的包：在卡片上点「存为文件用于分享」得到一个 `.sophon` 文件，里面是设置（可以不带密钥）和一条配好的自动化；自带的包只记名字，不重复打包。对方在同一页点「选择文件…」导入。

各自的测试：`python3 plugins/feishu/test_feishu.py`、`sh plugins/github-pr-watch/test_notify.sh`。
