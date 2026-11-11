> 在你的场景中播放视频

# 视频屏幕

要在场景中的屏幕上播放预录制或流式视频，请使用视频播放器 [智能物品](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/smart-items/).

![](/docs/_remote/bb8266cdd73eaeb440e3fe4d.webp)

## 常规设置

这些设置适用于所有场景，无论你是在播放视频还是进行流式传输。

![](/docs/_remote/6418389e00c134e192913d0e.webp)

你可以配置视频音频的音量。请注意，流的音频不是空间化的，在整个场景中听到的音量都是一致的。

该 **默认媒体源** 下拉菜单可让你在两种不同类型的源之间进行选择：

* **视频 URL**：从 URL 或本地视频文件获取视频或流
* **直播流**：使用 Decentraland 的免费流媒体基础设施来显示流。要使用此功能，你还必须包含一个 [管理工具](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-shi-yun-ying/scene-admin/) 场景中的智能物品。

## 播放视频

你可以通过以下任一方式播放预录制视频：

* **本地文件**：将视频文件作为场景的一部分上传，然后将 *URL* 字段指向该文件的路径。
* **从 URL 流式传输**：指向网页上的直播流或预录制流，例如来自 Bunny 等提供商的流。参见 [视频流式传输](#streaming-from-other-sources)

Video Player 智能物品何时播放视频的时机可能取决于不同因素：

* **自动**：场景加载后视频就会立即开始播放。为此，请将默认媒体源下拉菜单设置为 **视频 URL** 并将 URL 直接粘贴到 **视频路径或 .m3u8 URL** 字段中。

  ![](/docs/_remote/6418389e00c134e192913d0e.webp)
* **由管理员触发**：一个 [场景管理员](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-shi-yun-ying/scene-admin/) 当前在场景中的管理员可以使用管理界面粘贴视频 URL，并为当前在场景中的所有玩家播放该视频。
* **基于玩家操作**：定义一种类型为 **播放视频**。这样你就可以通过与其他智能物品交互来触发视频播放，例如走进一个房间或按下一个按钮。参见 [智能物品 - 高级](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/smart-items-advanced/).

  ![](/docs/_remote/87a1916dbb31dbea1502aad6.webp)

在所有情况下，你都要将视频配置为循环播放或仅播放一次。


> [!WARNING]
> **📔 注意**：如果场景中同时播放的视频过多，部分视频会被引擎暂停。优先级由与玩家的距离、摄像机方向和屏幕大小决定。可同时播放的视频最大数量取决于玩家的画质设置。
> 
> * 低：1
> * 中：5
> * 高：10
> 
> 我们还建议在玩家靠近时或执行相应操作时再开始播放视频。如果在场景远在地平线处加载时就开始播放视频，会在玩家访问相邻场景时不必要地影响性能。


## 多个视频屏幕

你可以同时在多个屏幕上播放同一个视频。为此，你必须编辑视频播放器智能物品的高级属性。


> [!WARNING]
> **📔 注意**：避免同时播放多个不同的视频，因为这会严重影响性能。
> 
> 如果你只是将相同的 URL 粘贴到两个视频播放器中，引擎不会知道它们是同一个视频，并会分别播放。请按照以下步骤配置第二个视频播放器播放与第一个相同的视频。


1. 向场景中添加两个视频播放器智能物品，每个屏幕一个。
2. 按如下所述正常配置第一个： [播放视频](#play-videos) 部分。
3. 在第二个视频播放器上，移除 **视频播放器** 组件的实体时，组件不会被激活。

![](/docs/_remote/a77f00900280e771283a1861.webp)


> [!WARNING]
> **📔 注意**：这一步很重要，否则即使第二个视频播放器不可见，引擎也会对其进行处理。


4. 仍在第二个视频播放器上，打开 **材质** 组件，展开 **纹理** 部分，然后选择 **视频源实体** 下拉菜单，将其指向第一个视频播放器。

![](/docs/_remote/ac9747acfdb69510f8b3a247.webp)

只要你将每个视频播放器都配置为指向同一个视频播放器，就可以对任意数量的视频播放器执行相同操作。

当进行 [直播流](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-shi-yun-ying/live-streaming/)时，两个屏幕也会显示相同的流。


> [!NOTE]
> **💡 提示**：上述步骤也可与一个具有以下组件的物品一起重复： **替换材质** 组件，将任何 3D 模型变成视频屏幕。配置 **纹理** 组件中的 **替换材质** 部分，使其指向视频播放器实体。


## 关于视频文件

支持以下文件格式：

* *.mp4*
* *.ogg*
* *.webm*

请注意，视频文件会增加场景的总大小，这会让进入你场景的玩家下载场景所需的时间更长。视频大小也可能让你超出 [场景限制](/docs/creator/content-creator-zh/chang-jing-sdk7/you-hua/scene-limitations/)，因为你每个地块最多可使用 15 MB。我们建议尽可能压缩视频，以减少这个问题。

## 直播流

如需端到端直播流，请参见 [直播流](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-shi-yun-ying/live-streaming/).

### 来自其他来源的流媒体

你也可以使用其他流媒体基础设施来串流视频。为此，只需将 Video Player 智能物品配置为使用 **视频 URL** 媒体源，并将流 URL 粘贴到 **视频路径或 .m3u8 URL** 字段中。

流媒体来源必须是一个 *https*  URL（*http* URL 不受支持），并且来源应具备 [CORS 策略（跨源资源共享）](https://en.wikipedia.org/wiki/Cross-origin_resource_sharing) 允许外部访问。这意味着你不能从 YouTube 或类似网站流式传输视频，因为这些网站只允许在其品牌化的 HTML 小组件中显示其内容。参见 [关于外部流媒体](/docs/creator/content-creator-zh/chang-jing-sdk7/mei-ti/video-playing/#about-external-streaming) 了解可选方案和提示。

视频流式传输有多种选项。最简单的选择是使用托管服务提供商，例如 [Vimeo](https://vimeo.com/), [Bunny](https://bunny.net), [Livepeer Studio](https://livepeer.studio/) 或 [Serraform](https://serraform.gitbook.io/streaming-docs/guides/decentraland-playback) 在那里，你向提供商支付费用，由其管理所有流媒体基础设施。

阅读 [设置 OBS 以成功进行直播](/docs/creator/content-creator-zh/chang-jing-sdk7/mei-ti/video-playing/#setting-up-obs-for-successful-streaming) 以获取有关如何最佳地将内容流式传输到 Decentraland 的提示。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/video-screen.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
