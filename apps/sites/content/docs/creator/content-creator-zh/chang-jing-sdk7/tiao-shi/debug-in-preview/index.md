> 如何在本地预览运行时调试你的场景

# 在预览中调试

运行预览会提供一些有用的调试信息和工具，帮助你了解场景是如何渲染的。

如果场景无法编译，你只会看到空地，什么都不会渲染在上面。如果发生这种情况，你可以在几个地方查找错误消息，以帮助你了解哪里出了问题：

1. 检查你的代码编辑器，确保它没有标记任何语法或逻辑错误。
2. 检查预览窗口中的控制台，查看是否有任何错误消息。通过点击 ![](/docs/_remote/92e8eb247fcf289c2feff8f8.webp) 右上角的图标。
3. 如果你是通过 Creator Hub 运行场景，请检查带有场景控制台输出的窗口。如果你是通过 CLI 运行场景，请检查你运行 `npm run start`


> [!NOTE]
> **💡 提示**：在你将场景发布到生产环境之前，你可以先将它发布到 [Decentraland World](/docs/creator/content-creator-zh/chang-jing-sdk7/fa-bu/publishing-options/#decentraland-worlds) ，以便在一个更接近生产环境的上下文中进行测试。


## 使用控制台

通过点击以下内容打开控制台： ![](/docs/_remote/92e8eb247fcf289c2feff8f8.webp) 右上角的图标。

你也可以通过按下键盘上的反引号键来切换它： **`**。这个键在大多数英文键盘上位于 1 键左侧。或者按 Shift + **\`** 以打开更宽的控制台视图，方便你查看更多文本。

将消息输出到控制台（使用 `console.log()`）。然后你可以通过打开场景控制台来查看这些消息的生成过程。

控制台还会显示错误消息，这些消息会以红色标记。

## 调试面板

通过点击以下内容打开调试控制台： ![](/docs/_remote/3a2d6a1437dc36383f2065db.webp) 右上角的图标。

这会在右下角打开一个面板，显示渲染器引擎的信息，并会随着变化实时更新。

![](/docs/_remote/eace962a130964afb6774cff.webp)

### 当前场景

该 **当前场景** 选项卡包含你当前站立所在场景的信息。这包括：

* 场景中的地块数量
* 你被允许建造的最大高度（每个场景 330 米）

你还可以检查 **显示场景边界** 选项，以查看场景周围的红色墙壁，这些墙壁标记了场景的边界。

### 内存

该 **内存** 部分显示了 Decentraland 使用的内存。请始终尽量将此值保持在 **内存预算阈值**以下。如果这个值增长过多，可能会导致访问你场景的玩家应用崩溃。请记住，在生产环境中，场景可能会与周围其他场景和玩家一起加载，这些都可能使内存消耗高于你在预览中看到的数值。

### 性能

该 **性能** 选项卡包含几个与 *FPS* （每秒帧数）相关的字段。随着场景复杂度增加，这个值通常会降低，请确保始终将此值保持在 25 FPS 以上，以保证玩家获得良好的体验。FPS 会因你运行它的机器而异。

这里最重要的值是 **平均 FPS**，它会将最近 1000 帧的 FPS 四舍五入显示。

![](/docs/_remote/5ba9631495e357c71fac976e.webp)


> [!WARNING]
> **📔 注意**：渲染器的 FPS 不同于场景每秒的游戏循环更新次数。场景逻辑上限为 30 FPS，渲染器的 FPS 可以达到 60 或更高。


卡顿指的是帧率的瞬时停顿，这可能对整体 FPS 数值影响不大，但玩家会非常明显地察觉到。通常在加载大型资源或类似情况时会发生。

### Web 请求

该 **WEB 请求** 部分允许你打开 Chrome DevTools 的 Network 选项卡，它会显示场景发出的网络请求数量及其内容。这样可以帮助你了解场景是否向服务器发出了过多请求，因为这会影响场景性能。

如果你的场景会与第三方服务器交互，查看进出数据通常会很有帮助。点击 **打开 Chrome DevTools** 以打开一个新的 Chrome 窗口，并自动打开 Network 选项卡。


> [!WARNING]
> **📔 注意**：要使用此功能，你必须先安装 [创作者中心](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/editor-installation/)，因为所需的依赖项已随该安装包捆绑。
> 
> 使用此功能无需打开 Creator Hub，也无需通过 Creator Hub 运行场景预览；如果你使用 CLI 运行场景预览，也可以使用此功能。


或者，你也可以在本机上运行一些免费的工具，它们可以报告进出 Decentraland 应用的所有数据。例如 [Charles](https://www.charlesproxy.com/) 或 [Wireshark](https://www.wireshark.org/).

## 快速重新加载

每当你对场景进行更改时，预览都会自动重新加载并更新，因此无需手动打开和关闭预览窗口。

如果你需要重新加载你当前所在的场景，请点击 **重新加载场景** 按钮，位于预览窗口的左上角。

![](/docs/_remote/a2ec4266edb8ce7bb450b24d.webp)

你也可以通过在聊天中输入以下内容并按回车来重新加载场景：

`/reload`

## 报告错误

如果你遇到的问题不是你的场景本身，而是 Decentraland SDK 的通用问题，请参阅 [报告错误](/docs/creator/content-creator-zh/chang-jing-sdk7/tiao-shi/report-bug/).

## 依赖版本

请确保你始终使用场景中所有依赖项的最新版本，因为你遇到的任何问题可能已经在较新的版本中修复。Creator Hub 会在你的 Decentraland 场景有可安装更新时通知你。

![](/docs/_remote/a002cdf10ca3a37130f2959d.webp)

你也可以通过命令行手动更新依赖项。要在本地运行 Decentraland 场景，依赖两个主要库： `@dcl/sdk` 和 `@dcl/js-runtime`，它安装在每个项目文件夹中。请确保这两个库都已更新到最新。你可以运行以下命令，手动将这两个库更新到最新稳定版：

```
npm i @dcl/js-runtime@latest
npm i @dcl/sdk@latest
```

如果你的场景使用了智能物品，它也可能正在使用 `@dcl/asset-packs` 库，你可以通过以下方式更新：

```
npm i @dcl/asset-packs@latest
```

如果你正在使用任何 [utils 库](https://studios.decentraland.org/resources?sdk_version=SDK7&resource_type=Library) ，请确保它们也已更新，因为这些库的旧版本可能与更新版本的 `@dcl/sdk`.

## 光照条件

世界内的时间段对 3D 模型的外观有很大影响。光源颜色会发生细微变化：夜晚带有蓝色调，而日出和日落时带有红色调。光线方向也会在天空中移动，从而投射出不同方向的阴影。

通过将游戏时钟切换到不同的值，检查你的场景在一天中的各个时段是否都表现良好。点击 **天空盒时间** 图标，在屏幕左侧面板上设置你喜欢的任意时间。如果此滑块呈灰色，请确保 **自动** 选项已禁用。

<figure><img src="/docs/_remote/be169c2886b8a8e49dfe5d5d.webp" alt="Dynamic skybox" width="300"><figcaption><p>世界内说明</p></figcaption></figure>

你的 3D 模型材质可能看起来与创建它时所用的建模工具中的效果不同。这是正常的，因为所有 3D 渲染引擎在处理光照和材质方面都有细微差异。

你也可以将场景设置为固定时间，从而使光照不再变化。详情请参见 [天空盒控制](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/skybox-control/) 。

## 多人测试

如果你启动场景预览并在两个（或更多）不同的浏览器窗口中打开它，每个打开的窗口都会被视为一个独立玩家，一个模拟通信服务器会让这些玩家保持同步。

在一个窗口中与你的场景交互，然后切换到另一个窗口，看看该交互的效果是否也会在那里可见。

使用 Creator Hub，在 **多实例预览** 中启用 **桌面客户端** 子菜单下的 **播放选项**，然后第二次点击 **播放** 按钮，以打开第二个 Decentraland 浏览器窗口。你必须在两个窗口中使用不同地址进行连接。随着场景重新加载，相同会话将保持打开状态。你可能需要在第二个窗口中先退出再重新登录，以便选择不同账户。

![](/docs/_remote/ca7e6550cdb1bb947ce74b28.webp)

两个玩家都能看到对方，并且可以彼此交互。如果场景包含任何多人逻辑，它将按预期工作，并在两名玩家之间同步。

或者，你也可以通过在浏览器 URL 中输入以下内容来打开第二个 Decentraland 浏览器窗口：

> `decentraland://realm=http://127.0.0.1:8000&local-scene=true&debug=true&multi-instance=true`

## 使用以太坊测试网络

在测试场景时，为了避免转移真实的 MANA 或其他货币，你可以使用 Ethereum Sepolia 测试网络，改为转移假的测试网 MANA。

要使用测试网络，你必须将 Metamask Chrome 扩展设置为使用 *Sepolia 测试网络* 而不是 *主网*。你还必须在 Sepolia 区块链上拥有 MANA，你可以从 Decentraland 免费获取。


> [!NOTE]
> **💡 提示**：要执行将 Sepolia MANA 转入你的钱包的交易，你需要支付 Sepolia Ether 的 gas 费用。


在此模式下查看场景时，你接受的任何交易都只会发生在测试网络中，不会影响你真实钱包中的 MANA 余额。

要使用测试网络预览你的场景，请将以下 URL 粘贴到浏览器标签页中。这将会在 Decentraland 桌面客户端中打开该场景：

> decentraland://realm=<http://127.0.0.1:8000\&local-scene=true\&debug=true\&dclenv=zone\&position=0,0>


> [!NOTE]
> **💡 提示**：将 position 参数更改为你场景的坐标，以便直接加载到你的场景中。你在此模式下查看场景时接受的任何交易都只会发生在测试网络中，不会影响你真实钱包中的 MANA 余额。


如果你需要在 Polygon 测试网中测试交易，并且需要在该测试网上拥有 MANA，那么在从 Sepolia 获得 MANA 后，你需要将 MANA 跨链到该网络。要将 Sepolia MANA 桥接到 Polygon 测试网，请访问你在 Sepolia 中的 Decentraland 账户页面，并在 Ethereum MANA 一侧点击“swap”。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/tiao-shi/debug-in-preview.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
