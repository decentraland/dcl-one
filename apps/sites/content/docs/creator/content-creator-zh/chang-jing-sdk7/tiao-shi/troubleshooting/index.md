> 常见问题的修复方法

# 故障排除

## 使用 AI 助手进行调试

在深入查看下面的问题之前，不妨把问题交给 AI 助手，比如 Cursor 的聊天、GitHub Copilot 或 Claude Code。把控制台中的错误消息粘贴给它，或者描述哪些地方没有按预期工作，它通常能在你场景的代码中找到问题并为你修复。

为了获得更好的效果，请确保 AI 已安装 Decentraland SDK skills。这些 skills 会教它 SDK 的模式和约束，这样它就不会基于通用或过时的信息去猜测。请在你的场景项目中运行以下命令进行安装：

```bash
npx skills add decentraland/sdk-skills
```

参见 [Vibe Coding with AI](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/vibe-coding/) 有关设置和提示 AI 助手的更多详情。

你也可以让 AI 调试场景 *在运行时*。Decentraland 桌面客户端可以暴露一个 MCP 服务器，让代理自行截图、读取场景的控制台输出、让玩家移动，并点击对象——因此不必由你来复现 bug 并粘贴错误，代理会自己复现、查看发生了什么，并反复迭代直到修复。使用 `npm run start -- --mcp` 并将你的代理连接到它。

参见 [让 AI 在世界中查看你的场景](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/vibe-coding/#let-the-ai-see-your-scene-in-world) 有关完整设置，并安装 `unity-explorer-mcp` 技能，这样代理就知道工作流程：

```bash
npx skills add decentraland/sdk-skills --skill unity-explorer-mcp
```

## 运行预览时出现的问题

#### 问题：无法运行任何场景预览，错误消息显示提到了 **权限被拒绝** 或 **EACCES**

你的操作系统不允许你修改想要运行项目的文件夹权限。在运行场景时，需要安装一些依赖，但被禁止了。你需要配置该文件夹的权限，允许你的 Windows/Mac/Linux 用户账户在其中编辑文件。

有用的资源：

* [docs.npmjs](https://docs.npmjs.com/resolving-eacces-permissions-errors-when-installing-packages-globally)
* [letscodepare](https://letscodepare.com/blog/npm-resolving-eacces-permissions-denied)

#### 问题：无法运行某个特定场景预览，错误显示 **错误：构建项目时出错**

如果你运行的是别人与你分享的场景，请确保该场景在共享时没有包含一个 `node_modules` 或 `bin` 文件夹，或者一个 `package-lock.json` 文件。这些文件包含依赖项，所使用的版本与你的操作系统和机器相关，它们应在第一次运行场景时生成。请手动删除这些文件夹和文件，然后运行 `npm run start` 再次。

#### 问题：运行 `npm run start` 时没有错误消息，但没有打开浏览器窗口，输出中也没有用于打开预览的 URL

请确保你的 Node 版本是最新的，必须是 20 或更高。

#### 问题：运行 `npm run start` 会打开一个浏览器标签页，但加载界面一直无法完成加载，或者我看到一个写着“critical error”的红色错误横幅。

* 请确保你的项目中安装了最新版本的 Decentraland SDK。运行：

  `npm i @dcl/sdk@latest`

#### 问题：场景运行时，在控制台中我看到 `循环依赖` 警告。

这些指的是你场景中相互引用的文件。这不一定是问题，但从编写软件的角度来说并不推荐，因为它可能导致难以调试的竞态条件和其他问题。尽管有这些警告，你的场景很可能仍然可以正常工作。

理想情况下，你场景中的代码加载应遵循清晰的顺序。存在循环依赖的代码可能会遇到“先有鸡还是先有蛋”的问题，即编译器不知道先启动哪个。通常这可以不出问题地解决，但最好避免。

要修复这些依赖，你通常必须通过调用函数或对象构造函数，在函数参数中传入已经实例化的实体/对象引用；而不是在函数中硬编码这些实体/对象的引用，因为它们可能已经实例化，也可能尚未实例化。

## 部署时出现的问题

#### 问题：你没有权限部署到这些地块

* 请确保 `scene.json` 你的场景文件正确列出了你想要部署到的坐标。
* 请确保 Metamask 已正确设置，以便使用正确的钱包来签署交易。这个钱包可以是持有 LAND 代币的钱包，也可以是由所有者授予了操作员权限的钱包。

#### 问题：运行 `npm run deploy` 失败

* 检查你场景的出生点，出生点的三个 x、y、z 坐标必须都是数字或都是范围。要么全部三个都是数字，要么全部三个都是范围。不支持某些是范围而另一些是数字。

  例如，不支持这种写法：

  `"position": {"x": [1,4], "y": 0, "z": [1,4]}`

  支持这种写法：

  `"position": {"x": [1,4], "y": [0,0], "z": [1,4]}`
* 分配给你的默认 catalyst 服务器可能已宕机或存在问题。你可以强制将 `npm run deploy` 命令改为部署到特定的 catalyst 服务器。要在 Decentraland Editor 中部署到特定服务器：

  1. 打开你的场景并点击 **发布**
  2. 选择选项 **发布到其他服务器** （位于底部）。
  3. 在下拉菜单中选择 **自定义服务器**
  4. 输入服务器地址，例如 `peer-ec1.decentraland.org`
  5. 点击 **发布到自定义服务器**
  6. 像正常部署一样批准该交易。

  通过 CLI 执行此操作：

  `npm run deploy -- --target-content <server-name>`

  例如：

  `npm run deploy -- --target-content peer-ec1.decentraland.org`

  参见 [catalyst-monitor](https://decentraland.github.io/catalyst-monitor/) 用于检查 catalyst 网络中所有服务器的状态。你也可以从每个卡片顶部复制其地址。
* 检查你场景的 `package.json`. 一个常见问题是存在一个 `bundleDependencies` 并且还有一个 `bundledDependencies` （多了一个 d）的部分。这有时可能是因为在同一个项目上不同时期运行了不同的 Node 版本，或者是因为在安装了不同 Node 版本的人之间共享了该项目。删除 `bundleDependencies`，它与较旧的 Node 版本有关。

还要确保你的 Node 版本是最新的，至少是 20 版。

#### 问题：运行 `npm run deploy` 或 `npm run build` 报告类型错误

你的场景可能存在 TypeScript 报告的类型错误，例如指出某个变量可能是类型 `任何` 或者 `undefined` 或 `null` 不允许。运行 `npm run deploy`时，也会运行 `npm run build`，它在这些检查上比 `npm run start`.

与 JavaScript 不同，TypeScript 会强制对所有变量进行严格类型检查。即使你的场景编写方式表明，例如某个值永远不会是 `undefined`，TypeScript 也需要知道在那种情况下会发生什么，或者你需要明确说明该值只能是例如字符串。

作为替代，你可以运行 `npm run deploy -- --skip-build` 以跳过运行 `npm run build`，并阻止这些检查执行。

#### 问题：我已部署场景，但进入 Decentraland 时看不到更改

* 请记住，如果场景包含较大的资源，这可能需要几分钟。
* 在新版本的 3D 模型转换为 asset bundle 的过程中，玩家会被刻意提供你场景最后一个完全正常工作的版本。这通常只需要几秒钟，但对于非常大的场景或者服务器繁忙时可能会更久。你可以 [检查转换状态](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/fa-bu/publish-scene/#check-the-conversion-status) ，直接查看你场景的状态。
* 仅仅重新加载场景并不足以获取新版本：重新加载只会重启场景代码，但不会拉取新发布的版本。转换完成后，请完全退出并重新启动 Decentraland，然后通过跳转链接或 `/goto` 聊天命令。

#### 问题：有些玩家看到的是我场景的新版本，而其他人仍然看到旧版本

你场景 3D 模型的转换在每个平台（Windows 和 Mac）上完成的时间不同，而且每个玩家的客户端也会保留本地缓存。要检查所有平台是否都已完成转换， [检查转换状态](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/fa-bu/publish-scene/#check-the-conversion-status) 并比较 `windows` 和 `mac` 下的值 `assetBundles`。一旦两者都 `完成`，请受影响的玩家完全退出并重新启动 Decentraland。

#### 问题：发布卡在转换阶段，Jump In 按钮始终不出现

你的场景要么排在其他正在转换的场景后面，要么转换失败了。

* 打开 `https://asset-bundle-registry.decentraland.org/queues/status` 并查找你场景的实体 ID。你可以在 `entityId` 字段中，当你 [检查转换状态](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/fa-bu/publish-scene/#check-the-conversion-status) 使用你场景的坐标时。如果你的 ID 已列在队列中，说明场景已排队，你只需等待。
* 如果它不在队列中， [检查转换状态](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/fa-bu/publish-scene/#check-the-conversion-status) 你场景的 `失败`，请再次发布场景。如果第二次仍失败， [报告问题](/docs/creator/content-creator-zh/chang-jing-sdk7/tiao-shi/report-bug/) 并附上实体 ID。

#### 问题：部署后，一些 3D 模型缺失、发黑或没有纹理

* 如果你刚刚发布，模型转换为 asset bundle 的工作可能还没有完成。请输入 `/detectabs` 到聊天窗口中：被标红的模型尚未转换。 [检查转换状态](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/fa-bu/publish-scene/#check-the-conversion-status) 并等待它完成。
* 确保所有 3D 模型都在场景边界内，连同它们的边界框也一样。如果在运行预览时，模型的任何部分超出这些限制，这些超出的部分都会被裁掉，并且不会渲染，无论是在预览中还是在已发布的场景中都是如此。
* 如果问题出在纹理上，请记住，在转换过程中，3D 模型中的纹理最大尺寸会被限制为 1024x1024 像素。

#### 问题：场景近看没问题，但从远处看时却破损或缺失

用于从远处渲染场景的低细节层级（LOD）资源版本，会在发布的最后阶段生成，可能还没有完成。这不会阻止你从近处测试场景。 [检查转换状态](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/fa-bu/publish-scene/#check-the-conversion-status) 并查看 `lods`下的值，或者干脆等待，稍后再检查。

#### 问题：部署后，我的 3D 模型看起来不一样了

* 如果纹理看起来不同，请记住，3D 模型中的纹理最大尺寸会被限制为 1024x1024 像素。这样做是为了确保 Decentraland 能让每个人都流畅运行。
* 如果模型看起来不同，可能是模型转换为 asset bundle 时出现了问题。阅读更多关于 asset bundle 压缩的信息 [此处](/docs/creator/content-creator-zh/chang-jing-sdk7/you-hua/performance-optimization/#asset-bundle-conversion).

  为验证这一点，请尝试使用 URL 参数运行场景 `&DISABLE_ASSET_BUNDLES`。如果在使用这个标志时模型看起来正常，那么问题一定与模型转换中的 bug 有关。在这种情况下， [报告问题](/docs/creator/content-creator-zh/chang-jing-sdk7/tiao-shi/report-bug/) 并附上你部署的实体 ID。

  请注意，将模型生成压缩后的 asset-bundle 版本需要服务器花一点时间（通常几分钟，场景非常大或服务器繁忙时会更久）。你可以通过在聊天窗口中输入以下命令，查看模型是否正以压缩 asset bundle 的形式加载 `/detectabs`。压缩模型会被标为绿色，未压缩的会被标为红色。

  你还可以在发布前通过启用 [预览中的优化资源](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/preview-scene/#preview-with-optimized-assets)在本地复现这个转换。这样你就能在不部署场景的情况下捕获并调试这些问题。

#### 问题：我的场景完全消失了

* 如果场景位于 World：你的 Worlds 存储配额可能已超出，例如在出售或转移 NAMEs、LAND 或 MANA 之后。你有 48 小时来释放空间或增加配额，否则你的 Worlds 将无法访问。请在 **管理** Creator Hub 的选项卡中，或在 **Worlds** 选项卡中的 [Builder](https://decentraland.org/builder/worlds)，然后重新发布。参见 [Worlds 大小限制](/docs/creator/content-creator-zh/chang-jing-sdk7/xiang-mu-lei-xing/kinds-of-project/#size-limits).
* 如果场景位于 LAND：拥有部署权限的人可能已在你的地块上发布了一个新场景，从而抹去了之前的内容。参见 [场景覆盖](/docs/creator/content-creator-zh/chang-jing-sdk7/fa-bu/publishing/#scene-overwriting).
* 在极少数情况下，违反 Decentraland 内容政策的内容可能会被内容服务器列入拒绝名单。如果你认为这是误判，请通过 [Decentraland Discord](https://decentraland.org/discord).

#### 问题：我的场景在正式环境中的 FPS 很差，尽管在预览中运行流畅。

你的场景性能可能会受到遵循不良实践的邻近场景影响，因为它们也会并行运行。你可以通过打开设置并将视距设置为最小值来验证是否如此，这样只会加载当前场景周围 1 个地块。

你还可以通过使用参数运行场景来进一步降低视距 `&LOS=0`，这样就不会加载任何周围场景。

如果你刚刚部署了场景，那么在服务器将场景中的 3D 模型转换为压缩 asset bundle 之后，加载场景时的负担也可能会减轻。你可以通过在聊天窗口中输入以下命令，查看模型是否正以压缩 asset bundle 的形式加载 `/detectabs`。压缩模型会被标为绿色，未压缩的会被标为红色。

### 报告错误

如果你遇到的问题不是你的场景本身，而是 Decentraland SDK 本身的问题，请参见 [报告错误](/docs/creator/content-creator-zh/chang-jing-sdk7/tiao-shi/report-bug/).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/tiao-shi/troubleshooting.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
