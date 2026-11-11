> 场景管理员可以对场景中实时发生的事情进行特殊控制。

# 场景管理员

将某些玩家授予特殊角色 **管理员** 在你的场景中。

在实时活动期间，管理员可以直接在 Decentraland 内部即兴控制场景中发生的事情，而无需预先安排动作或依赖第三方服务。等到人群聚集得足够多时再播放音乐，或者在时机合适时撒下彩纸，或让一艘宇宙飞船出现。



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/efjJN7Jr7Qo" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=efjJN7Jr7Qo>">Watch on YouTube</a></p>



当场景管理员访问你的场景时，他们会在右上角看到一个只有他们自己可见的特殊管理员面板。该面板使用顶部的图标标签在各个部分之间切换（视频、公告、智能物品、权限）。通过这个界面，他们可以播放视频或直播流，发送公告，封禁玩家，或激活任何已配置为可通过此方式激活的智能物品。这些操作会被场景中所有连接到与管理员相同通信岛的其他玩家看到。

![](/docs/_remote/4a423dfb13080da0e3f0dd27.webp)

## 设置管理员

要分配管理员，你需要添加 **管理员工具** 智能物品到你的场景中。

![](/docs/_remote/360f0190e175f35ce4c17c72.webp)


> [!WARNING]
> **📔 注意**: 更新你的场景以使用最新依赖项。
> 
> <img src="/docs/_remote/83f4693cc61cdbab26246a7d.webp" alt="" data-size="original">


在你开发场景并在本地测试时，你始终是管理员。一旦场景发布，任何拥有该场景发布权限的人也会自动成为管理员。其中包括：

* 发布该场景的 LAND 地块或 World NAME 的所有者
* 任何被授予 **操作员权限** 的这些地块或名称上的权限。参见 [授予权限](/docs/player/marketplace/land-manager/#give-permissions).
* 任何租用该土地的用户。参见 [租赁](/docs/player/marketplace/rentals/).

要分配没有发布权限、但可以在场景中进行实时运维的其他管理员：

1. 发布场景，并以管理员身份访问线上版本
2. 打开 **权限** 标签页（通过管理员面板顶部的图标标签）。

   ![](/docs/_remote/ed908a8f1e3f969689af965f.webp)
3. 在“添加管理员”旁边输入你想添加的人的钱包地址 **添加管理员** 并点击 **添加**.

你可以通过点击……查看场景中谁是管理员 **查看管理员列表** 按钮。从此界面你还可以 **移除** 将人员从管理员列表中移除。

![](/docs/_remote/983d682c49d98b1ce3fb7227.webp)


> [!WARNING]
> **📔 注意**: 只有通过以下方式手动添加到列表中的玩家，才能移除其管理员角色： **权限与管理** 标签页。作为场景所有者、操作员或租户的玩家会显示在此列表中，但无法通过此界面将其管理员角色移除。要移除操作员的管理员角色，必须先移除其操作员角色。


只要管理员玩家在场景中，他们就会在右上角看到一个特殊界面。非管理员玩家看不到此界面。

![](/docs/_remote/4a423dfb13080da0e3f0dd27.webp)

### 通过代码检查管理员状态

也可以使用代码判断某个玩家是否是场景管理员。这使得可以为玩家启用（或禁用）其他行为，例如允许与场景中的特定实体进行交互。

```ts
import { isAdmin } from "@dcl/asset-packs/dist/admin";

async function onPlayerSpawn() {
  const isAdminUser = await isAdmin();
  if (isAdminUser) {
    // 显示仅管理员可见的 UI，传送到舞台，显示实体等。
  }
}
```


> [!NOTE]
> **💡 提示**: 有关 async 函数的更多信息，请参见 [异步函数](/docs/creator/content-creator-zh/chang-jing-sdk7/bian-cheng-mo-shi/async-functions/).


## 视频播放

管理员最常执行的操作之一就是播放视频。管理员面板包含一个视频播放器部分，管理员可以在其中控制与视频相关的任何内容。

要启用此功能，你需要添加一个 **视频播放器** 智能物品到你的场景，并将其链接到 Admin Tools 智能物品。

1. 添加一个 **视频播放器** 智能物品到你的场景

   ![](/docs/_remote/bb8266cdd73eaeb440e3fe4d.webp)

   参见 [视频屏幕](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/video-screen/) 了解如何配置 Video Player 智能物品的默认媒体来源及其他设置的更多详情。场景运行后，大多数这些配置都可以被管理员覆盖。

   <div data-gb-custom-block data-tag="hint" data-style="warning" class="hint hint-warning"><p><strong>📔 注意</strong>: 管理员只能管理在 Video Screen 智能物品上播放的视频，不能管理通过 SDK 代码添加的屏幕上的视频。</p><p>你可以添加任意数量的视频屏幕。通常应避免同时播放多个不同视频，因为这会严重影响性能。</p></div>
2. 打开 Admin Tools 智能物品，确保 **视频屏幕** 已勾选该部分的复选框，以显示此部分。然后从下拉列表中选择屏幕，并为其指定一个友好名称，以便显示在管理员界面上。你可以添加任意数量的视频屏幕，每个屏幕都独立控制。

   <img src="/docs/_remote/34ed9d039629babf7b485b20.webp" alt="场景名称" width="300">

完成以上配置后，场景中的管理员用户可以打开管理员面板，并选择视频标签页（通过顶部图标标签）来控制这些视频屏幕。

<img src="/docs/_remote/48468cefefacba6545b47b45.webp" alt="场景名称" width="300">

如果你的场景有多个独立的视频屏幕， **屏幕** 下拉菜单可让你选择要控制的视频屏幕。列表中会显示你在 Admin Tools 智能物品配置中为每个视频屏幕指定的名称。


> [!NOTE]
> **💡 提示**: 若要在多个可作为一个整体控制的屏幕上显示同一视频，请参见 [多个视频屏幕](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/video-screen/#multiple-video-screens).


## 媒体来源

视频部分顶部的分段控件可让你在三种媒体来源之间切换。当前激活的来源会以胶囊标签显示。切换标签页只会显示该来源的设置，直到你点击 **激活**.

* **视频 URL**: 从 URL 播放视频。将视频 URL 粘贴到字段中，然后点击 **激活**。视频将开始在所选屏幕上为所有玩家播放。你也可以停止、暂停、重新开始并调整音量。

  <img src="/docs/_remote/77179ebe47d00636f626ed06.webp" alt="场景名称" width="300">

  <div data-gb-custom-block data-tag="hint" data-style="warning" class="hint hint-warning"><p><strong>📔 注意</strong>: 并非所有视频 URL 都可用。某些网站的视频对其内容有严格政策，会阻止 Decentraland 访问它们。请参见 <a href="/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/video-screen/#streaming-from-other-sources">来自其他来源的串流</a> 以了解在 Decentraland 中可以播放和不能播放的内容。</p></div>
* **DCL Cast**: 使用 Decentraland 的免费串流网页应用，即可轻松与场景中的其他玩家共享你的摄像头或屏幕，无需设置串流软件。演示控制项（上一页、下一页、播放视频、停止）会在演示进行时内联显示。

  <img src="/docs/_remote/0d1340c42e272abcc25bf603.webp" alt="DCL Cast" width="300">
* **直播**: 使用 Decentraland 的免费串流基础设施和 OBS 或 StreamYard 等串流软件播放直播流。

  <img src="/docs/_remote/79d537ec46382a7153d0fb2f.webp" alt="场景名称" width="300">

  参见 [直播串流](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-shi-yun-ying/live-streaming/) 以了解如何设置直播流的更多信息。

每个媒体来源部分都包含一个 **音量滑块** ，可让你将音量设置为 0%（静音）到 100%。

![](/docs/_remote/6fa0aca94ae7997a1988bcad.webp)

## 公告

在 **公告** 在管理员面板的标签页中，管理员可以编写所有场景玩家都能看到的消息。此类消息只能由管理员发送，因此其他玩家会认为它们比自称是管理员的人在聊天中发的消息更可信。

选择管理员面板的公告标签页。写一条消息并点击 **分享**。消息最多可包含 90 个字符。发送后输入框会自动清空，因此你可以立即输入新消息。

![](/docs/_remote/a7a34d2c60e405a1f61bb7f6.webp)

## 封禁玩家

你可以通过选择管理员面板中的 **权限** 标签页，输入你要封禁的玩家名称或钱包地址，然后点击 **封禁** 按钮。

![](/docs/_remote/ed908a8f1e3f969689af965f.webp)


> [!NOTE]
> **💡 提示**: 要获取玩家的钱包地址，请点击其头像打开个人资料，然后点击 **复制到剪贴板** 钱包地址旁边的按钮。


被封禁的玩家将无法加载你的场景或与其中任何内容互动。其他玩家将不会在场景中看到他们，也无法阅读他们的任何聊天消息。


> [!WARNING]
> **📔 注意**: 你的封禁会立即生效并持续有效。玩家一旦被封禁，在解除封禁前都会保持封禁状态。被封禁期间，玩家无法查看或与你的场景互动，无法在其中的 Nearby 频道聊天，场景中的其他玩家也看不到他们。
> 
> 如果玩家离开了你场景的边界，他们就不再受你场景的封禁规则影响，并且会再次看到被封禁的玩家。


点击 **查看封禁列表** 以查看当前被封禁的玩家列表。你也可以在此列表中 **解除封禁** 玩家。

## 触发智能物品

要从场景中的任何智能物品触发动作：

* 向你的场景添加一个智能物品
* 打开……的设置 **管理员工具** Creator Hub 中的智能物品
* 在 **智能物品动作** 部分，从下拉菜单中添加智能物品，为其指定一个自定义名称，并选择默认动作

完成上述配置后，管理员可以通过打开 **智能物品** 管理员面板的标签页并从下拉列表中选择一个项目来触发该动作。然后他们可以从 **操作** 下拉菜单中选择该项目的一个动作，并点击 **执行动作** 来触发它。还有一个按钮可以 **隐藏/显示** 所选项目。

<img src="/docs/_remote/078360766f2efe8ad3431ea0.webp" alt="场景名称" width="300">

你也可以显示或隐藏此列表中的任何智能物品，即使它本身没有用于执行此操作的动作。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-shi-yun-ying/scene-admin.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
