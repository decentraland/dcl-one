> 场景预览中你可以看到的内容

# 预览你的场景

一旦你已经 [创建了一个新场景](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/sdk-101/#create-your-first-scene) 或下载了一个 [场景示例](/docs/creator/content-creator-zh/jiao-cheng-yu-shi-li/examples/) 你就可以在本地预览它。

## 在 Creator Hub 中使用场景编辑器

请确保你已 [安装 Creator Hub](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/editor-installation/).

1. 打开你的场景项目。
2. 点击 **播放** 右上角的按钮。这将打开一个新窗口，其中运行着仅包含你的场景的 Decentraland Desktop Explorer。你可以在其中四处移动并与交互式物品互动。

![](/docs/_remote/ca7e6550cdb1bb947ce74b28.webp)

### 选择预览位置

点击旁边的箭头 **播放** 按钮以打开 **播放选项** 菜单。主面板用于选择运行场景的方式：

* **桌面客户端**：在 Decentraland Desktop Explorer 中打开预览。这是默认选项。
* **Web（Bevy）**：使用 Bevy Web 客户端在浏览器中打开预览。等同于 `--web` CLI 标志。
* **显示移动端二维码**：显示一个二维码，可在 [Decentraland 移动应用](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/yi-dong-ke-hu-duan/overview/)中打开你的场景预览。使用与你的计算机连接到同一 Wi-Fi 网络的手机扫描该代码。点击弹出窗口周围的深色区域即可关闭它。有关详细信息，请参阅 [在移动端预览](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/preview-on-mobile/) 。

### 桌面客户端选项

选择 **桌面客户端** 在 **播放选项** 菜单中，然后将鼠标悬停在其上以打开一个子菜单，其中包含仅影响桌面客户端的设置：

* **启用地形景观**：切换场景周围的景观。默认启用；禁用它可降低场景的内存占用。当场景自身的设置已隐藏地形时，此选项会显示为灰色。
* **多实例预览**：允许同时运行多个 Explorer 窗口，因此你可以 [在本地测试多人场景](#test-a-multiplayer-scene-locally)。启用后，每个窗口都会要求你登录，因此你可以在每个窗口中以不同玩家身份连接。仅当你的项目使用 SDK 7.20.4 或更高版本时可见。
* **启用 MCP 服务器**：启动启用了 MCP 自动化服务器的 Explorer，让 AI 代理能够查看和控制运行中的预览。仅当你的项目使用 SDK 7.25.0 或更高版本时可见。请参阅 [使用 AI 进行氛围编程](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/vibe-coding/#let-the-ai-see-your-scene-in-world) 以了解完整工作流程。
* **压缩资源**：使用本地生成的资源包预览场景，与生产环境中经过 [资源包转换](/docs/creator/content-creator-zh/chang-jing-sdk7/you-hua/performance-optimization/#asset-bundle-conversion)后的渲染效果一致。首次运行会转换所有资源，在大型场景中可能需要数分钟。仅当你的平台和项目的 SDK 版本支持时可见。请参阅 [使用优化资源预览](#preview-with-optimized-assets).

场景的控制台输出无需菜单选项。每次运行预览时，Creator Hub 都会为你打开它。请参阅 [Creator Hub 控制台](#the-creator-hub-console).


> [!NOTE]
> **提示：** 你也可以直接在 Decentraland 移动应用中预览场景。请使用 **显示移动端二维码** Creator Hub 中的选项，或运行 `npm run start -- --mobile` 从 CLI 运行。请参阅 [在移动端预览](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/preview-on-mobile/) 。


## 使用 CLI

要预览场景，请在场景的主文件夹中运行以下命令：

```bash
npm run start
```

系统会安装所有缺失的依赖项，然后 CLI 会在你的系统中创建一个本地 Web 服务器，并通过 `decentraland://` 深层链接在 Decentraland Desktop 客户端中启动场景。桌面客户端是默认预览目标。

若要改为在浏览器标签页中预览，请添加 `-- --web` （或 `-- --bevy-web`）以在以下地址的 Bevy Web 客户端中打开场景： `decentraland.org/bevy-web/`.

每当你对场景进行更改时，预览都会自动重新加载和更新，因此无需再次运行命令。


> [!WARNING]
> **📔 注意**：某些场景依赖于与外部服务器通信，以执行自定义逻辑或存储和检索数据。预览此类场景时，你可能还需要在另一个端口本地运行该服务器。请查看场景的 readme，了解如何启动服务器和场景。


### 预览命令的参数

你可以向 `npm run start` 命令添加以下标志以更改其行为：

* `-- --web` （别名 `-- --bevy-web`）在以下地址的 Bevy Web 浏览器客户端中打开预览： `decentraland.org/bevy-web/` ，而不是在 Desktop Explorer 中打开。基于 Chromium 的浏览器（Chrome 142+）要求托管页面具有本地网络访问权限，才能访问你的本地预览服务器——当浏览器请求访问你设备上的应用时，请点击“允许”。
* `-- --mobile` （别名 `-- -m`）在终端中显示一个二维码，可在连接到同一 Wi-Fi 网络的手机上的 [Decentraland 移动应用](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/yi-dong-ke-hu-duan/overview/) 中打开你的场景。请参阅 [在移动端预览](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/preview-on-mobile/).
* `-- --skip-build` 跳过构建，仅以预览模式提供文件。
* `-- --port` （别名 `-- -p`）用于指定运行场景的端口。否则将使用任何可用端口。
* `-- --no-browser` （别名 `-- -b`）用于阻止预览打开新的浏览器标签页。
* `-- -w` 或 `-- --no-watch` 用于不监视文件系统更改，并避免场景代码变更时热重载。
* `-- --ci` 用于在远程 Unix 服务器上运行地块预览器。
* `-- --multi-instance` 允许同时运行多个 Explorer 实例。
* `-- --asset-bundles` 使用优化的资源包预览。Desktop Explorer 会在预览期间自行将场景资源转换为资源包，与场景在生产环境中的渲染方式一致。等同于 **压缩资源** Creator Hub 中的选项。仅适用于桌面客户端（不适用于 Bevy Web 客户端）。请参阅 [使用优化资源预览](#preview-with-optimized-assets).
* `-- --no-client` 禁止所有自动启动行为（桌面 Explorer 深层链接、打开浏览器、移动端二维码）。如果桌面 Explorer 自行连接，文件监视器仍会通知它。当外部工具负责管理 Explorer 进程时，此选项很有用。
* `-- --mcp` 在 Explorer 中启用 MCP 服务器（作为深层链接参数转发）。
* `-- --mcp-port` Explorer 中 MCP 服务器的端口（作为深层链接参数转发）。例如： `npm run start -- --mcp --mcp-port 3001`.


> [!WARNING]
> **📔 注意**：参数需要使用两组连字符添加，例如 `npm run start -- --web3`.


## 使用优化资源预览

当你发布场景时，Decentraland 服务器会将其所有 3D 模型转换为资源包，这是一种加载和渲染时更轻量的优化格式。请参阅 [资源包转换](/docs/creator/content-creator-zh/chang-jing-sdk7/you-hua/performance-optimization/#asset-bundle-conversion)。默认情况下，本地预览会跳过此步骤，而是加载原始模型。

启用优化资源，以便在预览场景时在本地运行相同的转换。这有两个好处：

* 预览加载更快、运行更流畅，尤其是在包含大型 3D 模型的场景中。
* 你看到的模型与玩家发布后看到的完全一致，因此可以在发布前发现任何资源压缩问题。

此选项仅影响你的本地预览。无论是否使用此选项，已发布的场景始终会在服务器上转换。

要启用它：

* **在 Creator Hub 中**：打开 **播放选项** 旁边的 **播放** 按钮，将鼠标悬停在 **桌面客户端**上，然后勾选 **压缩资源**.
* **从 CLI**：运行 `npm run start -- --asset-bundles`.

![](/docs/_remote/b1d24b74a3fe9e049ed203e0.webp)

启用此选项后，Decentraland Explorer 会先在你的计算机上将场景的 3D 模型转换为资源包，再加载它们。对于大多数场景，转换几乎是即时完成的；但如果场景包含非常大的资源，首次预览可能需要更长时间。转换后的模型会被缓存，因此后续预览时仅需转换新的或已修改的资源。


> [!NOTE]
> **💡 提示**：如果资源转换因任何原因失败，预览会回退为加载原始 3D 模型，就像常规预览一样。


## 将场景上传到 Decentraland

当你对场景感到满意后，可以将其上传并发布到 Decentraland。为此，你必须拥有 LAND、Decentraland NAME 或 ETH ENS 名称，或获得拥有者授予的权限。请参阅 [发布](/docs/creator/content-creator-zh/chang-jing-sdk7/fa-bu/publishing/) 以了解操作说明。

## 预览场景大小

预览中显示的场景大小基于场景配置。

请在场景编辑器的场景菜单第二个标签页中编辑此项。

![](/docs/_remote/5bcc0f6dfa70093b896862ae.webp)

使用下拉菜单并点击 **应用布局** 以更改场景尺寸。你还可以点击各个单独的地块，将其从布局中切换为关闭。

![](/docs/_remote/c2905f95052e96703e78829e.webp)

你也可以编辑 *scene.json* 文件，在“parcels”字段中列出多个地块。请参阅 [通过命令行设置地块](/docs/creator/content-creator-zh/chang-jing-sdk7/xiang-mu-lei-xing/scene-metadata/#scene-parcels) 以了解更多详情。


> [!NOTE]
> **💡 提示**：运行预览时，地块坐标不必与场景实际使用的坐标一致，只要它们相邻并排列成相同形状即可。稍后在你 [部署场景](#upload-a-scene-to-decentraland).


## 查看场景控制台

点击右上角的 ![](/docs/_remote/92e8eb247fcf289c2feff8f8.webp) 图标以打开控制台。你可以在这里看到任何错误消息，以及场景通过 `console.log()`.

打印到控制台的任何文本。你也可以按键盘上的 **\`** 键打开它。你还可以按 Shift + **\`** 以将控制台打开得更宽，以便查看更多文本。

## Creator Hub 控制台

Creator Hub 无需离开编辑器即可显示相同的输出，并且默认如此。运行预览后， **控制台** 标签页会显示在 **文件系统** 和 **资源包** 旁边，位于编辑器底部面板中。

若要阻止控制台打开，请取消勾选 **打开调试控制台** ，位于 **播放选项** 中的 **编辑器** 标签页下。

若要将控制台移开，请点击其标题栏中的弹出图标，标签为 **在单独窗口中打开控制台**。控制台会以名为 **控制台**的独立窗口打开，你可以将其移到第二台显示器上，并在编辑时保持可见。

独立窗口有一个 **清空控制台** 按钮，以及一个 **停靠回编辑器** 按钮，可将控制台返回到底部面板。同时，编辑器中的 **控制台** 标签页会显示 **控制台已在单独窗口中打开** ，并提供一个 **停靠回此处** 按钮，其作用相同。


> [!NOTE]
> **💡 提示**：控制台在分离时仍会持续收集输出，因此将其停靠回去不会丢失任何内容。未运行预览时，它会显示 **运行场景以查看调试输出**.


## 在本地测试多人场景

如果你启动场景预览并在两个（或更多）不同的 Explorer 窗口中打开它，每个打开的窗口都会被视为独立玩家，模拟通信服务器会让这些玩家保持同步。

在一个窗口中与场景互动，然后切换到另一个窗口，以查看该互动的效果是否也在其中可见。

使用 Creator Hub，开启 **多实例预览** 在 **桌面客户端** 的子菜单 **播放选项**，然后再次点击 **播放** 按钮，以打开第二个 Decentraland Explorer 窗口。你必须在两个窗口中使用不同的地址连接。场景重新加载时，这些会话将保持打开。

![](/docs/_remote/ca7e6550cdb1bb947ce74b28.webp)

或者，你可以通过在浏览器 URL 中输入以下内容来打开第二个 Decentraland Explorer 窗口：

> `decentraland://realm=http://127.0.0.1:8000&local-scene=true&debug=true&multi-instance=true`

### 高级：使用远程资源包快速迭代


> [!NOTE]
> **💡 提示**：对于大多数场景，使用资源包预览的最简单方法是启用 [优化资源](#preview-with-optimized-assets)，这会在本地转换你的模型，并始终反映其最新版本。以下模式是另一种方案，它会复用已发布在服务器上的资源包。


对于包含许多 3D 模型的大型场景，你可以通过复用 [资源包](/docs/creator/content-creator-zh/chang-jing-sdk7/you-hua/performance-optimization/#asset-bundle-conversion) 来加快场景加载和重新加载速度；这些资源包已发布在 Decentraland 的服务器上，而无需加载未优化的原始 3D 模型。这在仅迭代代码更改时尤其有用。

要启用此模式，请使用以下参数启动 Decentraland Desktop 客户端：

```bash
npm run start -- --realm http://127.0.0.1:8000/ --position 0,0 --local-scene true --debug --skip-version-check true --lsd-use-remote-ab <ab-source>
```

该 `<ab-source>` 参数会因场景已发布的位置而异：

* **在 Genesis City 中**: `--lsd-remote-ab-server Genesis`
* **在 World 中**: `--lsd-remote-ab-world <world-name>.dcl.eth`

例如，要预览已部署到 World 的场景的本地副本：

```bash
npm run start -- --realm http://127.0.0.1:8000/ --position 0,0 --local-scene true --debug --skip-version-check true --lsd-use-remote-ab --lsd-remote-ab-world myworld.dcl.eth
```

在这两种情况下， `--realm http://127.0.0.1:8000/` 会将客户端指向你的本地预览服务器（运行 `npm run start` 以先启动它），并且 `--local-scene true` 会告知客户端从那里加载场景的代码。


> [!WARNING]
> **📔 重要**：使用此模式时，建议 **全部** 美术资源均已发布，并且其资源包已由内容服务器完全处理。如果你添加了任何新资源，它们会作为原始 gltf 文件加载，因此无法享受优化加载，就像你通常运行预览时一样。但如果你在本地修改了已发布的资源，同时保持相同文件名，那么你将看到该资源先前发布的版本。
> 
> 在这种情况下，请先重新部署场景，等待几分钟让资源包生成（请参阅 [资源包转换](/docs/creator/content-creator-zh/chang-jing-sdk7/you-hua/performance-optimization/#asset-bundle-conversion)），然后继续使用此模式进行仅代码迭代。



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/ru-men/preview-scene.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
