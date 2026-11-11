> 使用 Video Screen 和 Admin Tools 将实时视频流传输到场景中。

# 直播

使用 **视频屏幕** 智能物品以及 **管理员工具** 用于将实时视频流传输到你的场景中的智能物品。

Decentraland 提供多种将实时视频流传输到你的场景的方法：

* **DCL Cast** *（简单模式）*：使用 Decentraland 的免费流式传输网页应用，轻松与场景中的其他玩家共享你的摄像头或屏幕，无需设置流媒体软件。此模式延迟最低，也是最容易设置的。
* **直播** *（高级模式）*：使用类似 [OBS](https://obsproject.com/) 的流媒体软件，通过 Decentraland 的流媒体基础设施进行直播。此模式让你能更好地控制直播，例如屏幕布局和音频来源。
* **视频 URL** *（高级模式）*：通过将 URL 粘贴到 **视频 URL** 字段中，指向你自己的流媒体基础设施。

<img src="/docs/_remote/9fe1dc304a4f62e468f108b3.webp" alt="流式传输方式" width="400">

流式传输可在 Worlds 和 Genesis City 中使用，且场景端没有观众数量限制。

## 配置场景

以下步骤对 DCL Cast 和直播方式都通用：

1. 添加一个 **视频屏幕** 智能物品到你的场景中。

   ![](/docs/_remote/bb8266cdd73eaeb440e3fe4d.webp)
2. 添加一个 **管理员工具** 智能物品并启用 **视频屏幕** 部分。从下拉菜单中选择每个屏幕，并为管理界面设置一个易于识别的名称。

   ![](/docs/_remote/34ed9d039629babf7b485b20.webp)
3. 发布你的场景（World 或 Genesis City），并以具有管理员权限的用户身份进入。

   ![](/docs/_remote/522b72b559e9bf32de19bcab.webp)

场景发布后，你可以以具有管理员权限的用户身份进入，并配置流媒体设置。


> [!NOTE]
> **💡 提示**：如果你添加多个视频屏幕来显示同一视频，请将除一个之外所有屏幕的来源都配置为指向同一个视频播放器，详见 [多个视频屏幕](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/video-screen/#multiple-video-screens) 了解更多详情。


## DCL Cast（简单）

### 共享应用访问权限

以管理员用户身份进入你已发布的场景，并打开管理面板。选择 **视频** 选项卡，然后选择 **DCL Cast** 功能。

<img src="/docs/_remote/0d1340c42e272abcc25bf603.webp" alt="DCL Cast" width="400">

你会看到两个可复制并与他人分享的链接。

* **发言者**：此链接供发言者用于将他们的视频投射到场景中。

  <div data-gb-custom-block data-tag="hint" data-style="danger" class="hint hint-danger"><p><strong>❗警告</strong>：请将流媒体链接视为机密，只与你信任的人分享。如有需要，可在演讲者之间重置该链接。</p><p>完成直播后，关闭 DCL Cast 浏览器标签页以释放频道。</p></div>
* **观众**：此链接供观众通过浏览器或手机观看视频。这对当前不在 Decentraland 中的玩家很有用。

点击 **复制链接** 按钮将链接复制到剪贴板。

准备开始直播时，点击 **激活** 按钮，使场景中的观众可见该直播。

<img src="/docs/_remote/6fa0aca94ae7997a1988bcad.webp" alt="激活直播" width="150">

如果因任何原因需要重置房间，请点击 **重置房间** 按钮生成一个新的房间。当前正在直播的任何人都将断开连接。

<img src="/docs/_remote/5daf35dba544baf2c6504619.webp" alt="重置房间" width="150">

### 使用 DCL Cast 应用

当有人将发言者链接粘贴到浏览器中时，他们会看到如下界面：

<img src="/docs/_remote/8b74b9e4f571aec33f619bc5.webp" alt="DCL Cast 应用" width="400">

浏览器会请求共享你的摄像头和麦克风的权限。你还可以配置用于直播的不同输入设备。


> [!NOTE]
> **📔 提示**：请使用 Google Chrome 或基于 Chrome 引擎的浏览器。这些浏览器支持直接从浏览器标签页轻松共享视频和音频。


用户可以输入一个名称（不必与其 Decentraland 用户名一致），然后点击 **立即加入** 按钮开始直播。

开始直播后，该应用与常见的视频会议应用类似，提供静音/取消静音、共享摄像头和屏幕以及聊天界面等按钮。

聊天为只读，会接收 Decentraland 场景中玩家发送的所有消息。即使你从其他设备直播，这也非常适合与观众保持联系。

<img src="/docs/_remote/66b24803959f06d37231b10c.webp" alt="DCL Cast 应用" width="700">

在 **参与者** 选项卡中，你可以看到三个列表：

* **发言者**：当前正在向场景直播的人。
* **观众**：当前正在通过浏览器观看直播的人。
* **场景内参与者**：当前在场景内观看直播的玩家。

<img src="/docs/_remote/1ff69ac3627fa7cbbad58fca.webp" alt="参与者选项卡" width="200">

如果 DCL Cast 会话中有多个发言者，场景内的玩家会听到所有发言者的声音，且画面会自动切换为当前正在发声的人，以始终显示正在说话的人。

若要覆盖此默认行为：

* 点击 **发言者** 场景管理面板上的按钮

<img src="/docs/_remote/2e5e3dd8ae5ce493e7c7cc13.webp" alt="参与者选项卡" width="200">

* 选择一位发言者，并选择要展示的来源（该发言者的摄像头或屏幕）

<img src="/docs/_remote/7e4c6bd675830ab1e698bcfc.webp" alt="参与者选项卡" width="200">

这将强制始终在屏幕上显示该来源，无论其他发言者是否在说话。


> [!NOTE]
> **📔 提示**：如果你也在场景内观看直播，可能会觉得在 DCL Cast 应用和 Decentraland 场景中重复播放的音频产生回声令人不适。你可以通过切换屏幕左下角的扬声器图标，轻松静音 DCL Cast 应用中的所有音频
> 
> <img src="/docs/_remote/f1c102f46a10285862b60d5a.webp" alt="Participants tab" data-size="original">
> 
> 否则，你可以在 Decentraland 设置中将音频静音。


### 共享演示文稿

你也可以共享幻灯片演示的内容，作为另一种图像来源。

* 在场景内，点击 **共享演示文稿** 在 DCL Cast 选项卡中的场景内按钮。然后在 DCL Cast 应用中，点击旁边的下拉菜单 **共享屏幕** 按钮，并选择 **共享演示文稿**.

<img src="/docs/_remote/021196a0a453b918bd70cad9.webp" alt="参与者选项卡" width="200">

* 粘贴 Google 幻灯片链接、Drive 中托管的 .pdf 链接或类似来源的链接，或者上传 .pdf 文件。

现在，演示文稿将成为一个可选择在屏幕上显示的来源，同时仍可听到所有发言者的声音。


> [!WARNING]
> **📔 注意**：演示文件必须小于 100 MB。Google 幻灯片演示文稿必须设置为 *公开*.
> 
> 在一次 DCL Cast 会话中，同一时间只能有一个活动演示文稿。


然后你可以切换幻灯片，甚至通过点击在 DCL Cast 应用和场景内 Admin Tools 界面中都存在的按钮来播放或暂停这些幻灯片中嵌入的任何视频。

## 直播（高级）

要在你的场景中使用直播功能，你需要安装一款能够输出到 RTMP 端点的流媒体软件（例如 [OBS](https://obsproject.com/), [XSplit](https://www.xsplit.com/), [StreamYard](https://streamyard.com/)).


> [!WARNING]
> **警告**：单人模式世界不支持直播。如果你的世界已在世界设置中启用 **单人模式** （或 `fixedAdapter` 设置为 `"offline:offline"` 在 `scene.json`），请在设置直播之前将其禁用。直播功能依赖通信层，而单人模式下该通信层是禁用的。


### 获取直播凭据

1. 在场景中打开管理界面（右上角图标）。

   ![](/docs/_remote/3a78785f1b44cd50b0963098.webp)
2. 在 **视频** 选项卡中，切换到 **直播** 并点击 **获取直播密钥**.

   ![](/docs/_remote/548a347a2432d40b40d20705.webp)
3. 复制 **RTMP 服务器** 和 **直播密钥** 到你的流媒体软件中。

   ![OBS configuration](/docs/_remote/683eec84be8a03a78e4ed799.webp)


> [!CAUTION]
> **❗警告**：同一时间只能有一人向场景直播。完成直播后，点击 **停止直播** 在你的软件中释放该频道。


### 开始并控制直播

1. 从你的软件开始直播。
2. 在管理界面中，点击 **激活** 即可在场景中显示直播。

   <img src="/docs/_remote/6fa0aca94ae7997a1988bcad.webp" alt="激活直播" width="100">

### 直播密钥

直播密钥按场景生成，有效期为 4 天（96 小时）。单次直播会话可连续运行最长 4 小时。

![](/docs/_remote/8fdba6b9787070f6aacf6827.webp)

* 点击 **重置直播密钥** 以吊销当前密钥并发放新密钥。正在进行的直播将停止。
* 每个场景都有自己的直播地址和密钥。管理员可以将密钥分享给外部主播。
* 每个场景同一时间只能有一个直播处于活动状态；启动新的直播将覆盖当前直播。


> [!CAUTION]
> **❗警告**：请将直播密钥视为机密。如有需要，可在不同演讲者之间重置密钥。


## 通过 URL 直播（高级）

你也可以通过将视频屏幕配置为使用以下选项来进行直播 **视频 URL** 并粘贴直播 URL。

你应该可以粘贴指向大多数主流视频网站的视频 URL。请注意这些平台的服务条款。

若要从本地机器上的视频文件进行直播，最简单的方式是将该视频上传到公开的 Google Drive，然后粘贴链接。

* URL 必须为 `https`。参见 [关于外部流式传输](/docs/creator/content-creator-zh/chang-jing-sdk7/mei-ti/video-playing/#about-external-streaming).
* 推荐的提供商包括 [Vimeo](https://vimeo.com/), [Bunny](https://bunny.net), [Livepeer Studio](https://livepeer.studio/) 和 [Serraform](https://serraform.gitbook.io/streaming-docs/guides/decentraland-playback).
* 编码器设置提示： [设置 OBS 以成功直播](/docs/creator/content-creator-zh/chang-jing-sdk7/mei-ti/video-playing/#setting-up-obs-for-successful-streaming).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-shi-yun-ying/live-streaming.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
