> 导入你自己的 3D 模型、图片、声音等，用于你的场景。

# 导入自定义资源

你可以将自己的 3D 模型导入场景编辑器。你可以从互联网上大量免费的或付费的资源中挑选模型，也可以创建自己的自定义模型。你还可以导入其他资源，如图片、声音文件和视频。



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/UepXpH-k0EI" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=UepXpH-k0EI>">Watch on YouTube</a></p>



## 导入资源

要将 3D 模型、图片、声音文件或视频从本地磁盘导入到你的场景中：

1. 将文件直接拖到底部面板上。你也可以点击 **+ 导入资源** 按钮，然后在底部面板左上角从本地驱动器中选择。

![](/docs/_remote/7a7e1bf757e28411bf4bd633.webp)

2. 检查模型缩略图并点击 **导入**。当导入多个资源时，使用箭头按钮逐个切换查看每个资源。

![](/docs/_remote/114b427e0cc71cef23245e2a.webp)

现在你可以在 **本地资源** 选项卡中找到你的资源。资源会按类型分类到文件夹中：3D 模型位于 *assets/Models* 文件夹中，图片位于 *assets/Images*，声音文件位于 *assets/Audio*，视频位于 *assets/Video*.

。内置免费资源包中的项目存放在 *assets/asset-packs/*&#x4E0B;，而在编辑器中创建的自定义项目存放在 *assets/custom/*&#x4E0B;。较旧的场景可能会将用户导入的内容放在 *assets/scene/* 中。这些路径都可以在你的场景代码中使用，只需引用资源所在的文件夹即可。

* 对于 3D 模型，请将 `.glb` 或 `.gltf` 文件拖到画布上，将它们添加为场景中的物体。
* 你也可以将图片、声音或视频文件拖到画布上。编辑器会创建一个已经指向你的文件的现成物体：

  * 一个 `.png`, `.jpg` 或 `.jpeg` 文件会变成一个 **图像** 物体。
  * 一个 `.mp3`, `.ogg` 或 `.wav` 文件会变成一个环境音效物体。
  * 一个 `.mp4` 文件会变成一个 **视频屏幕** 物体。

  新物体会以文件名命名，并且会带有与资源包中对应物体相同的组件和设置，因此你可以马上在检查器中对其进行调整。
* 任何资源也都可以拖到场景中已有物体的字段上。例如，你可以拖一个 `.mp3` 文件到 *路径* 某个 *音频源* 组件，则抛出错误。


> [!NOTE]
> **💡 提示**：你也可以将文件直接从 Finder、Explorer 或其他任何地方粘贴到项目的 `资源` 文件夹中。复制完成后不久，它们就会单独出现在 **本地资源** 选项卡中。如果它们没有出现，请按 **刷新** 按钮，位于 **导入资源** 按钮。
> 
> <img src="/docs/_remote/bef94fa811ae48e5d4c837f0.webp" alt="" data-size="original">


### 支持的格式

#### 音频

以下音频格式受支持：

* *.mp3*
* *.wav*
* *.ogg*

#### 图像

以下图像格式受支持：

* *.png*
* *.jpg*
* *.jpeg*

#### 视频

以下视频格式受支持：

* *.mp4*

#### 3D 模型

以下 3D 模型格式受支持：

* *.glTF*
* *.glb*

两者都可以包含外部纹理图像文件，或者外部二进制（*.bin*）文件。

你可以使用各种不同的编辑器和工具将其他格式转换为这些格式。请参见 [3D 建模](/docs/creator/content-creator-zh/3d-jian-mo-yu-dong-hua/3d-models/) 获取推荐和提示。

模型中的所有材质都需要是以下之一： *基础材质* 或 *PBR*，并且所有纹理都必须是 2 的幂尺寸（例如：256、512）。请参见 [场景限制](/docs/creator/content-creator-zh/chang-jing-sdk7/you-hua/scene-limitations/) 了解详情。

任何类型的导入文件要能在场景中使用，其大小都必须小于 50 MB。不支持更大的文件。

**3D 模型的免费资源库**

与其自己制作 3D 模型，你也可以从多个免费或付费资源库下载，或者使用 AI 工具生成。请参见 [有用资源](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/useful-resources/) 获取推荐的资源库和生成式 AI 工具列表。


> [!WARNING]
> **📔 注意**：请注意你下载内容所附带的许可限制。


请注意，在其中一些网站上，你可以选择模型的下载格式。始终选择 *.glTF* 或 *.glb* 格式（如果可用）。如果不可用，你必须先将其转换为 *.glTF* 或 *.glb* 才能在场景中使用。为此，我们建议将它们导入 Blender，并使用可用的某个 *.glTF* 导出插件导出它们。

### 碰撞体

你可能会发现，在运行预览时，玩家可以穿过你导入的 3D 模型。这很可能是因为这些模型缺少一个 *碰撞网格* 用于定义碰撞几何体的。请参见 [碰撞器](/docs/creator/content-creator-zh/3d-jian-mo-yu-dong-hua/colliders/) 获取更多详情和说明。


> [!NOTE]
> **💡 提示**：与其编辑模型来添加一个 *碰撞网格*，一种更简单的替代方案是添加一个 *隐形墙* 形状大致相同的智能物体来代替它。


### 动画

如果导入的模型包含动画，则会循环播放打包在模型中的第一个动画。

请注意，你无法控制动画何时开始或停止，也无法在多个动画中控制播放哪一个。

如果场景中有多个玩家，他们看到的动画可能彼此不同步。

要更改此行为，你可以包含一个 **Animator** 组件遍历所有玩家。参见 [让任何物品变智能](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/make-any-item-smart/) 用于无代码工具，使你的物体具有交互性。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-bian-ji-qi/gou-jian/import-items.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
