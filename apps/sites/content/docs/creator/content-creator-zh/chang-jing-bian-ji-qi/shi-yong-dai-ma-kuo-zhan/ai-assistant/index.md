> 在 Creator Hub 中与 AI 助手聊天，构建并修改你的场景。

# AI 助手

Creator Hub 可以运行一个 AI 助手，它能看到你打开的场景并为你编辑。你可以让它放置物体、连接行为、编写脚本，或者运行预览并检查结果。


> [!WARNING]
> **📔 注意**：该助手运行的是你安装并登录的编码 CLI，对你的机器拥有完全访问权限，和它在你的终端中的权限一样大。只有在你信任它将要执行的操作时，才启用它。


## 它的设置所在位置

助手默认开启。它的设置有单独的选项卡：

1. 点击左上角的 Creator Hub 标志，然后选择 **设置**.
2. 前往 **AI** 选项卡中。

该选项卡包含 **启用 AI 助手** 开关， **改为计入 API 密钥** 选项， **连接** 编码 CLI 列表，以及 **暴露 AI 助手 MCP 服务器** 设置。

当助手开启时，编辑器顶部栏会出现一个 **AI 助手** 按钮（闪光图标），位于 **代码** 按钮左侧。点击它即可在场景旁边打开聊天面板。

![编辑器顶部栏，左侧是场景名称，右侧是闪光 AI 助手图标、代码、预览和发布按钮。](/docs/_remote/97b2496489d20fd6596ad9b5.webp)

![初次打开时的 AI 助手面板：顶部有“新聊天”下拉菜单和显示为 Claude 的 CLI 下拉菜单，中间有欢迎消息、“关于 AI 使用”卡片、一个标明所选实体的 Selection 标签，以及底部的“询问助手”输入框。](/docs/_remote/dceaca65cd87dab45134934a.webp)

## 连接一个编码 CLI

该助手不会使用自己的 API 密钥。它运行的是你安装并登录的编码 CLI，因此费用计入该工具自己的订阅。

支持四种：Claude、Codex、Cursor 和 Gemini。你需要通过各自厂商的安装程序，在终端中分别安装它们。

### 安装 CLI

运行适用于你的操作系统的命令：

| CLI    | macOS 和 Linux                                           | Windows                                                                                      |
| ------ | ------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| Claude | `curl -fsSL https://claude.ai/install.sh \| bash`       | `winget install Anthropic.ClaudeCode`                                                        |
| Codex  | `curl -fsSL https://chatgpt.com/codex/install.sh \| sh` | `powershell -ExecutionPolicy ByPass -c "irm https://chatgpt.com/codex/install.ps1 \| iex"`   |
| Cursor | `curl https://cursor.com/install -fsS \| bash`          | `powershell -ExecutionPolicy ByPass -c "irm 'https://cursor.com/install?win32=true' \| iex"` |
| Gemini | `npm install -g @google/gemini-cli`                     | `npm install -g @google/gemini-cli`                                                          |

你不必手动复制这些内容。Creator Hub 会在两个位置显示适用于你的操作系统的准确命令，并附上该厂商安装指南的链接：

* 聊天面板中的设置卡片，在你第一次打开且尚未连接 CLI 时会出现。
* 设置中的 **AI** 选项卡下 **连接**，位于 **通过终端** 部分的每个 CLI 下方。那里的每条命令旁都有一个 **复制** 按钮。

厂商指南：

* [Claude Code 安装指南](https://code.claude.com/docs/en/terminal-guide)
* [Codex CLI 安装指南](https://developers.openai.com/codex/cli)
* [Cursor CLI 安装指南](https://cursor.com/docs/cli/installation)
* [Gemini CLI 安装指南](https://www.geminicli.com/docs/get-started/installation)

安装后，点击 **重新检查** 聊天面板设置卡片上的此按钮，Creator Hub 就会识别新的 CLI，无需重启应用。 **AI** 选项卡在每次打开时都会自动再次检查。


> [!WARNING]
> **📔 注意**：Claude Desktop 和 Claude Code 不是同一个东西。该助手需要的是 Claude Code，也就是 Anthropic 的终端工具；登录桌面应用并不会让你登录到它。如果 Creator Hub 找到了 Claude Desktop 但没有找到 Claude Code，它会明确提示并向你显示安装命令。
> 
> Gemini CLI 需要付费的 Gemini API 密钥，或者 Gemini Code Assist Standard 或 Enterprise 许可证。


### 登录

| CLI    | 登录                   |
| ------ | -------------------- |
| Claude | `claude auth login`  |
| Codex  | `codex login`        |
| Cursor | `cursor-agent login` |
| Gemini | 运行 `gemini` 并按照提示操作  |

在终端中运行登录命令。它会在浏览器中完成。

Codex 有个快捷方式：如果它还未安装，Creator Hub 可以替你安装并登录。点击 **使用 Codex 登录** 聊天面板设置卡片上的此按钮，或者点击 **登录** 旁边的 Codex **连接** 在 **AI** 选项卡中。Creator Hub 会执行登录并打开你的浏览器，然后同一位置会出现一个 **退出登录** 链接。如果你是用上面的命令自行安装了 Codex，那就不会有按钮：请像其他工具一样从终端登录。

一个绿色圆点和 **已连接** 显示在某个 CLI 旁边，表示它已准备好使用。


> [!NOTE]
> **💡 提示**：如果你登录了不止一个 CLI，聊天面板中的下拉菜单可让你在它们之间切换。


### 当代理要求你再次登录时

即使 CLI 仍显示已连接，已保存的登录状态也可能过期。当某个 CLI 在回复消息时返回“未登录”，Creator Hub 会显示 **需要登录** 状态，并用琥珀色圆点提示，而不是把原始命令行直接传给聊天。

聊天面板和 **通过终端** 选项卡中的 **AI** 部分会显示要依次运行的两个命令，最终在浏览器中完成：

| CLI    | 先退出登录，再重新登录                                    |
| ------ | ---------------------------------------------- |
| Claude | `claude auth logout`，然后 `claude auth login`    |
| Codex  | `codex logout`，然后 `codex login`                |
| Cursor | `cursor-agent logout`，然后 `cursor-agent login`  |
| Gemini | 运行 `gemini`，然后 `/auth logout`，然后 `/auth login` |

如果 Creator Hub 替你安装了 Codex，请改用 **退出登录** 然后使用 **登录** 在 **AI** 选项卡。

登录后，再次发送你的消息。代理会始终保持启用，所以你无需从头重新设置。

### 改为计入 API 密钥

如果你宁愿按 token 付费而不是使用 CLI 订阅，请在设置的 **改为计入 API 密钥** 在 **AI** 选项卡中启用。它会从你的环境中读取提供商的 API 密钥： `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `CURSOR_API_KEY` 或 `GEMINI_API_KEY`。默认是关闭的。

这也是绕过无法完成的登录的一种方法。当某个 CLI 处于 **需要登录** 状态时，Creator Hub 会显示一个 **或者使用 API 密钥** 提示，并附上匹配的变量名。

## 它能做什么

助手会在你当前打开的场景上工作。它可以：

* **编辑场景**：创建和删除实体，更改组件，放置 [智能物体](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/smart-items/)，附加 [脚本](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-yong-dai-ma-kuo-zhan/script-component/)，以及读取和写入 `scene.json`.
* **编写代码** 于你场景的文件中。
* **运行预览** 并进行操作：截图、四处走动、点击元素、读取日志并检查性能。
* **读取你的选择**，因此你可以选中一个实体并说“把这个弄高一点”。

更改会在发生时保存，并进入正常的撤销历史。若要一次性撤回整个对话，请在 **撤销 AI 更改** 助手回复中使用该按钮。

![在请求重命名实体并添加 Mesh Renderer 之后，停靠在编辑器右侧的 AI 助手面板：回复列出了它运行的工具，描述了结果，并在下方显示“撤销 AI 更改”按钮。](/docs/_remote/6a907fb6c14f96a55c037af0.webp)

当助手工作时，它的更改会实时显示在实体树和画布中。

![完整的 Scene Editor，右侧停靠着 AI 助手面板，实体树中可见已重命名的 Stage 实体，助手回复末尾显示“撤销 AI 更改”。](/docs/_remote/1a8e6565c0eaf9d6cba56d1b.webp)

## 与聊天协作

* **新聊天** 开始一段新的对话。
* **聊天历史** 列出更早的对话及其运行时间。对话会按项目保存，因此重新打开场景时会恢复这些聊天记录。
* **在单独窗口中打开** 会把助手弹出到自己的窗口中，这在第二台显示器上很方便。 **停靠回编辑器** 将其返回。
* 面板可以拖得更宽或更窄。

一个 **关于 AI 使用** 卡片会提醒你，助手会消耗你已连接账户中的 token。请留意该账户的使用情况。

助手一次只处理一条消息。在回复进行中时， **思考中……** 指示会一直显示，直到该轮结束，包括在较长的工具调用期间。发送第二条消息不会取消第一条：请等待回复，或者按 **停止** 自行结束这一轮。

有些回复会先向你提问，然后才继续。如果你让某个问题长时间得不到回应，它会以“此问题不再有效。”结束，你可以再次提问。

### 将文件附加到提示中

有时，向助手展示某个内容比描述它更容易。你可以通过三种方式将文件附加到任何消息中：

* 点击消息框左侧的回形针图标，标注为 **附加文件**，然后选择一个或多个文件。
* 从文件管理器将文件拖到消息框上。拖动时会出现 **松开以附加文件** 提示。
* 直接从剪贴板粘贴图像，例如你刚截取的屏幕截图。

每个已附加文件都会作为一个标签显示在消息框上方，并带有表示其类型的图标：图像、3D 模型（`.glb`, `.gltf`）、音频（`.mp3`, `.wav`, `.ogg`, `.m4a`, `.flac`），或其他任何内容的通用文件。点击标签上的 **移除** 叉号即可在发送前将其删除。单条消息最多可附加 8 个文件，而且你甚至可以在没有任何文本的情况下发送附件。

你拖入或从磁盘中选取的文件会通过其真实路径交给助手，因此不会复制文件，也没有大小限制。助手可以直接查看图像，并且在编写场景代码时可以通过路径引用模型或音频文件。粘贴的图像在磁盘上没有对应文件，因此 Creator Hub 会先将它们保存为临时文件；这些文件每个上限为 8 MB。

如果你的 Claude CLI 版本过旧，面板会显示警告。运行 `claude update` 在终端中修复它。

### 在提示中指向一个资源

附加文件会发送其内容。提及资源更轻量：它只是告诉助手你指的是项目中的哪个资源，因此你可以说“用这个”而不必输入路径。

打开助手后，双击 **本地资源** 选项卡中的任意资源。它会作为一个标签显示在消息框上方，内容为 **资源：&#x20;*****文件名***，或 **内置动画：&#x20;*****名称*** ，用于某个内置头像动画。

* 你可以提及任意多个资源。每个资源都会有自己的标签和自己的 **移除** 叉号。
* 提及会与 **Selection** 标签并排显示，因此你可以同时指向场景中的一个实体和一个资源：“把这个动画放到那个 NPC 身上”。
* 提及只对你的下一条消息生效。发送后它们就会清除。


> [!NOTE]
> **💡 提示**：选中 Scene 根节点等同于什么都没选，因此它永远不会发送给助手。想让助手对某个实体操作时，请选中一个真实实体。


## Decentraland 技能已为你安装

当助手运行时，Creator Hub 会下载官方的 [Decentraland SDK Skills](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/vibe-coding/) 并将它们链接到你的项目中，这样助手就会遵循经过验证的 SDK7 模式，而不是靠猜测。

它们会链接到 `.claude/skills` 和 `.agents/skills` ，位于你的项目文件夹内，并且两者都会被添加到 `.gitignore` 中，因此它们永远不会进入你的仓库。

## 助手留在你场景文件夹中的文件

助手可以通过本地 MCP 服务器读取和编辑你当前打开的场景。每个 CLI 对该服务器的告知方式不同，因此根据你使用的工具不同，你可能会在项目中看到新文件：

| CLI    | 场景文件夹中会出现什么                                           |
| ------ | ----------------------------------------------------- |
| Cursor | `.cursor/mcp.json`，以及 `.cursor/rules/creator-hub.mdc` |
| Gemini | `.gemini/settings.json`                               |
| Claude | 不会有。配置会放在项目外的临时文件中。                                   |
| Codex  | 不会有。配置会通过命令行传递。                                       |

如果你已经有这些配置文件中的某一个，Creator Hub 会将其条目合并进去，而不是覆盖你自己的服务器。


> [!WARNING]
> **📔 注意**: `.cursor/mcp.json` 是唯一包含访问令牌本身的文件，因为 Cursor 的配置格式无法从环境中读取它。Creator Hub 会在写入前把它添加到你的 `.gitignore` 中，所以不要提交它，也不要删除那一行。Gemini 文件则通过 `$CREATOR_HUB_MCP_TOKEN` 环境变量来引用令牌，因此它不包含机密内容，也不会被忽略。


## 改用你自己的 AI 工具

如果你已经在 Claude Code、Cursor、Codex 或其他支持 MCP 的工具中工作，你可以让它指向当前打开的场景，而不是使用内置面板。MCP（模型上下文协议）是一种 AI 工具调用另一个程序的标准方式。

1. 从 Creator Hub 标志打开主菜单，选择 **设置**，然后在 **AI** 选项卡中展开 **暴露 AI 助手 MCP 服务器**.
2. 点击 **复制配置**.
3. 并将其粘贴到你工具的 MCP 配置中。

复制的配置看起来像这样，其中已填入真实的端口和令牌：

```json
{
  "mcpServers": {
    "creator-hub": {
      "type": "http",
      "url": "http://127.0.0.1:<PORT>/mcp",
      "headers": { "Authorization": "Bearer <TOKEN>" }
    }
  }
}
```

对于 Claude Code，你也可以在场景文件夹中的终端里添加它：

```bash
claude mcp add --transport http --scope local creator-hub http://127.0.0.1:<PORT>/mcp \
  --header "Authorization: Bearer <TOKEN>"
```


> [!WARNING]
> **📔 注意**：每次 Creator Hub 启动时，端口和令牌都会不同。如果你的工具突然出现 `401` 错误，请重新复制配置。服务器只监听你自己的机器，并且只会作用于你当前打开的场景。


## 相关页面

* [与 AI 一起进行 Vibe Coding](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/vibe-coding/)：关于 SDK Skills 以及如何更好地编写提示的完整指南。
* [与代码结合](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-yong-dai-ma-kuo-zhan/overview/)：在代码编辑器中打开你的场景。
* [使用 Script Component](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-yong-dai-ma-kuo-zhan/script-component/)：从文件中将行为附加到实体。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-yong-dai-ma-kuo-zhan/ai-assistant.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
