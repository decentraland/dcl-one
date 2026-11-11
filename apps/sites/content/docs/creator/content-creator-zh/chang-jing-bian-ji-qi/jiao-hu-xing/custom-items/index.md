> 创建你自己的自定义物品，以便在任何场景中重复使用。

# 自定义物品

定义一个自定义项目，以便在你的任何场景中轻松复用，或与其他场景创作者分享。自定义项目可以由单个实体组成，也可以包含任意多个实体。自定义项目可以是现有智能项目的变体，也可以完全自定义，拥有自己定制的模型和功能。



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/7cGLu8P7dso" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=7cGLu8P7dso>">Watch on YouTube</a></p>



## 如何定义自定义项目

右键单击 [实体树](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/scene-editor-essentials/#the-entity-tree)中的某个实体，或选择多个实体后再右键单击它们。选择 **创建自定义项目**.

![](/docs/_remote/52a9b492b9f5042f38574e21.webp)

随后，在屏幕下方区域会提示你为新的自定义项目命名。

![](/docs/_remote/eb32f501e5b7c6a70d610e46.webp)

该项目现在会列在 **自定义项目** 选项卡中，位于屏幕底部。只有在你的工作区中至少存在一个自定义项目时，此选项卡才会显示。

![](/docs/_remote/a66e7b1f2ae4e2d83577aeb6.webp)

定义自定义项目时，你可以选择同一层级上的多个实体，但如果实体不共享共同祖先，则不能选择来自不同父级的实体。任何嵌套实体都会自动作为自定义项目的一部分包含进来，无需手动选择。

你场景中的原始实体不会受到定义自定义项目这一操作的影响。


> [!WARNING]
> **📔 注意**：定义自定义项目时，你会对所选实体上每个组件的状态进行快照（根实体的位置、旋转和缩放除外）。这包括 **动作**, **触发器**, **多人模式**, **脚本**, **可见性**，以及任何其他组件。
> 
> 任何智能项目的动作和触发器都会自我引用其自身副本。例如，如果你定义了一个包含电梯和按钮的自定义项目，那么每个电梯副本上的按钮都会控制它们所属的那一份电梯副本，而不是原始电梯副本。


### 使用自定义项目

只需将项目从 **自定义项目** 选项卡拖入你的场景即可。

添加后，你可以自由修改自定义项目的任何属性；你所做的更改只会影响 *该副本* 中的自定义项目。你也可以编辑或删除定义该自定义项目时所依据的原始项目，这不会影响现有或未来的副本。

请注意，自定义项目在实体树中会显示不同的图标。在右侧项目属性菜单顶部，你还会看到它们是基于哪个自定义项目创建的。

要删除自定义项目定义，请右键单击 **自定义项目** 菜单中的该项目并选择 **删除**。此操作不会影响场景中该项目的任何现有副本，已脱离原项目的自定义项目在场景中会保持不变。删除自定义项目定义只会将其从“自定义项目”选项卡中移除。

要重命名自定义项目定义，请右键单击 **自定义项目** 选项卡中的该自定义项目定义并选择 **重命名**.

你无法修改已创建的自定义项目定义，必须创建一个新的定义并删除原始定义。

### 动态生成自定义项目

每个自定义项目都会以复合体形式存储，这是一种描述实体和组件树的文件。这意味着你可以在场景运行时生成一个新的自定义项目副本，而不是手动放置每个副本。

你可以通过两种方式生成自定义项目：

* **无需代码**：使用场景编辑器中的 **生成实体** 动作。参见 [关于生成实体](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/smart-items-advanced/#about-spawning-entities).
* **使用代码**：使用 `Composite.instance()` 在 SDK 中。参见 [复合体](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/composites/).

### 分享自定义项目

你可以将自定义项目分享给其他创作者，让他们在自己的场景中使用。

自定义项目会分别存储在本地机器上的独立文件夹中。点击 <img src="/docs/_remote/ad0c0dd6dced1bedf2b986b9.webp" alt="Code" data-size="line"> 自定义项目选项卡右上角的文件夹图标即可打开该文件夹。

你也可以在你的机器上手动找到此文件夹：

* 在 Windows 中： *User/AppData/Roaming/creator-hub/Custom Items*
* 在 Mac 中： *Users/username/Library/Application Support/creator-hub/Custom Items*


> [!WARNING]
> **📔 注意**： *Library* 文件夹在 Mac 中默认是隐藏的。访问它最简单的方法是打开“前往 > 前往文件夹”，然后输入 *application support/creator-hub*


要与他人分享，只需打开“自定义项目”文件夹并复制该项目的完整文件夹。你可能需要将该文件夹压缩成 zip 以便更容易传输。此文件夹包含使用你的自定义项目所需的一切内容。

使用你自定义项目的人随后必须将该项目文件夹解压到他们自己机器上的“自定义项目”文件夹中。他们可能需要点击刷新按钮 <img src="/docs/_remote/29e0f054f9a96c7fb0c004cd.webp" alt="Refresh" data-size="line"> ，该项目才会显示在他们的 **自定义项目** 选项卡中。

你的自定义项目使用的任何 **资源** 也会存储在该自定义项目的文件夹中。这包括该项目引用的任何 3D 模型、图片、脚本、声音和视频。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/custom-items.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
