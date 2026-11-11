> Scene Editor 是一个简单的可视化工具，可让你创建并发布 Decentraland 场景。

# 编辑场景

Creator Hub 包含一个强大的场景编辑器，它将简单的无代码界面与编写代码以进一步自定义场景的能力相结合。

![创作者中心](/docs/_remote/9100f2b541affb1c6fecfe89.webp)

参见 [Creator Hub 安装](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/editor-installation/) 开始使用。



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/tK5-fyBVnK0" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=tK5-fyBVnK0>">Watch on YouTube</a></p>



## 创建场景

要创建新场景，打开 Creator Hub，然后点击 **创建** 按钮旁边的箭头。从下拉菜单中选择 **新场景**.

![“创建”按钮的下拉菜单已打开，包含“新场景”和“导入场景”选项。](/docs/_remote/727cb68c3c3feea0e6221431.webp)

然后系统会要求你为场景命名，并选择保存位置。如果你想从带有一些初始内容的项目开始，而不是空场景，点击 **模板** 在 **场景** 选项卡中并选择一个。

参见 [管理场景](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/manage-scenes/) 以了解更多详情。

场景打开后，你会在左侧看到实体树，中间是画布，右侧是所选项目的属性，底部是资源包。

![场景编辑器打开了一个小场景：左侧实体树列出了 Player、Camera、Scene 和几个项目，画布显示一块草地，上面有一个烧烤架和一个鸟喷泉，右侧属性面板显示所选 Bird Fountain 的 Transform 和 GLTF 组件。](/docs/_remote/e758b583939953f86e7202c8.webp)

参见 [场景编辑器基础](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/scene-editor-essentials/) ，可逐一了解各个部分。

## 移动浏览

要在场景编辑器中熟悉各个部分：

* 使用 **W** 和 **S** 来靠近或远离。你也可以使用鼠标滚轮，或者 **+** 和 **-** 按键
* 使用 **一个** 和 **D** 来横向移动。
* 使用 **Q** 和 **E** 来上下移动。
* 使用 **鼠标左键** 来点击并选择项目，以及移动它们。
* 使用 **鼠标右键** 并拖动来旋转摄像机。


> [!NOTE]
> **💡 提示**：你也可以通过按住 **Alt** 在 Windows 上，或 **Option 键** 在 Mac 上拖动来旋转摄像机。这在使用触控板而不是鼠标时尤其方便。


* 按 **空格键** 将摄像机重置回默认位置

## 添加项目

使用底部菜单中的主题资源包分类来浏览，找到可放置到场景中的不同项目。

![](/docs/_remote/c5121b56db02ac73b992d99e.webp)

要放置项目，请从资源包菜单中点击并拖动它到画布中场景的某个位置。

![](/docs/_remote/b20b9f38d71f91b4c4270633.webp)

点击并拖动所选项目，可以让它在场景地面上自由移动。参见 [场景编辑器基础](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/scene-editor-essentials/#position-items) 以了解更多详情。


> [!NOTE]
> **💡 提示**：有些项目是 **智能项目**，这些自带内置交互行为。参见 [智能项目](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/smart-items/) 以了解更多详情。


![](/docs/_remote/4401edd6b0ae269ede4c9467.webp)

要让 3D 视图占满窗口的全部高度，请点击其标签栏最右端的折叠按钮，将资源面板折叠起来。面板会缩小到只剩标签，再次点击同一按钮即可恢复到你之前的高度。折叠时点击任意标签也会将其重新打开。

### 头像动画

该 **头像动画** 包中保存的是玩家头像的动画，而不是可放置的项目：坐姿、钓鱼、搬箱子、跳舞等等。它们被归类到相同的 **操作**, **坐姿** 和 **表情** 组中，与其他内容一致。

将鼠标悬停在卡片上即可查看动画播放效果。将其拖到 3D 视图中，会把其文件复制到场景的本地资源中。不会添加实体，因为动画不是场景中静止存在的东西：你需要从代码中播放它。参见 [头像动画](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/tou-xiang/avatar-animations/).

带有 **内置** 徽章的卡片工作方式不同。这些动画已经随 Decentraland 一起提供，因此每个场景都可以直接播放，无需添加文件。你不能将它们拖入场景。相反，请右键单击卡片并选择 **复制名称** 以获取代码中要使用的名称。

在 **本地资源** 选项卡中， **头像动画** 筛选器会收集项目中的动画文件，以及一个只读的 **内置动画** 文件夹，列出那些不需要文件的动画。

## 预览

要测试你的场景并像玩家一样体验它，点击右上角的 **播放** 按钮。这将打开一个新的窗口，里面运行着 Decentraland Desktop Explorer，仅加载你的场景。在那里你可以在场景中移动并与交互式项目互动。


> [!WARNING]
> **📔 注意**：如果你的电脑上没有安装它，请下载 **Decentraland 启动器** 从 [Decentraland.org](https://decentraland.org).


![](/docs/_remote/ca7e6550cdb1bb947ce74b28.webp)

点击 **播放** 按钮旁边的箭头以打开 **播放选项** 菜单，你可以选择是在桌面客户端、浏览器还是手机上预览。参见 [预览你的场景](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/preview-scene/) 以查看所有可用选项的完整列表。

## 场景渲染器

场景编辑器使用 **Babylon** 作为编辑画布的默认渲染器。你可以切换到 **Bevy（实验性）**.

要更改渲染器：

1. 点击左上角的 Creator Hub 标志并选择 **设置**.
2. 前往 **实验性** 选项卡。
3. 开启 **启用 Bevy 场景渲染器**.

![“应用偏好设置”对话框在“实验性”选项卡中，显示“启用 Bevy 场景渲染器”开关。](/docs/_remote/ef0ed302a5aa995c466b03ac.webp)

Bevy 渲染器是编辑画布的另一种引擎。它影响的是你在编辑时场景的外观，而不是发布后玩家看到的样子。

使用 Bevy 渲染器时，工具栏会增加一个摄像机模式下拉菜单、用于在画布中运行场景的播放和停止按钮，以及一个 **交互** 切换（手柄图标），还有一个 **静音场景音频** 切换（扬声器图标）。

![Bevy 渲染器下的编辑器工具栏：撤销和重做、gizmo 工具、显示为 Free 的摄像机下拉菜单、播放和停止按钮、Interact 手柄切换、Preferences 齿轮和 Inspector 按钮。](/docs/_remote/851c27272919db1fa6fcf53e.webp)

使用 **交互** 切换，可直接在编辑器画布中测试正在运行的场景，而无需打开单独的预览窗口。这让你无需停止场景就能快速检查交互。在交互时，智能项目会在画布中直接显示悬停提示，例如要按下的按键。

点击标有 **静音场景音频**的扬声器按钮，以便在你工作时将场景静音。它会静音场景中的每个 [音频源](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/sounds/) 和音频流，无论是在编辑时还是场景在画布中运行时都会静音，而且即使场景重新加载也会保持静音。再次点击它，此时标为 **取消静音场景音频**，即可恢复声音。该按钮只会在 Bevy 渲染器下显示，因为 Babylon 渲染器不会播放场景音频。

Bevy 编辑器还会加载 [自定义项目](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/custom-items/)，为损坏或缺失的资源显示占位标记，并在你的场景 SDK 依赖过期时提供带修复操作的友好错误消息。工具栏中的摄像机切换显示为 **自由** 或 **玩家**，具体取决于你当前所处的模式。


> [!WARNING]
> **📔 注意**：场景统计选项卡在 Bevy 下不可用。请在桌面或移动客户端中运行场景预览，并在那里查看统计信息。



> [!WARNING]
> **注意：** Bevy 渲染器属于实验性功能，可能并不支持默认 Babylon 渲染器所支持的所有编辑功能。


## 场景设置

点击 **铅笔图标** 位于屏幕右上角。它会打开一系列可编辑的场景级属性，包括名称、缩略图、场景大小等。

![](/docs/_remote/a64bc0ae30e4d852f97ccd78.webp)

参见 [场景设置](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/pei-zhi/scene-settings/) 以了解更多详情。

## 发布你的场景

一旦你对场景满意，按下 **发布** 按钮。

![](/docs/_remote/75db93f7a2fdd17bec3b4907.webp)

参见 [发布场景](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/fa-bu/publish-scene/) 以了解更多详情。

## 另请参见

* 参见 [场景编辑器基础](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/scene-editor-essentials/) 以了解有关场景编辑器界面的更多细节。
* 参见 [智能项目](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/smart-items/) 以了解如何为你的场景添加简单的交互。
* 参见 [与代码结合使用](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-yong-dai-ma-kuo-zhan/overview/) 以了解如何编辑你的场景代码。
* 参见 [发布场景](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/fa-bu/publish-scene/) 以了解如何将你的场景发布到 Decentraland。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/about-editor.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
