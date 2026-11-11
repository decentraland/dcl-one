> 如何使用 Armature 导出表情动作中的粒子

# 表情动作中的粒子

Emotes 2.0 可以有声音和道具，但你知道也可以使用粒子吗？它们会很有趣，还能为其增添额外的视觉亮点！而且它们也能加快动画制作流程。


> [!NOTE]
> **💡 提示**：安装 [Decentraland Tools Blender 插件](https://extensions.blender.org/add-ons/decentraland-tools/)。它包含一些实用功能，可帮助你编辑和导出 3D 模型、可穿戴物品和表情。


但由于 Emotes 2.0 不支持物体动画和粒子系统，我们需要将这些粒子转换为骨架（这是受支持的），以便将它们添加到表情动作中。为此我们将使用 Foundation 内容团队开发的一个 Blender 插件，可通过下面的链接下载。

使用 [Blender 的粒子转骨骼插件](https://github.com/decentraland/docs/blob/main/creator/images/emotes/particle_system_to_bone_animation.py) 开始吧！

![](/docs/_remote/8a0401cd85e912317d2b0e9e.webp) ![](/docs/_remote/86f127e163391ec5c9cc99a0.webp)

为你的表情动作添加粒子的过程分为两个步骤，你可以结合以下视频按照指南进行：

* **将粒子转换为骨架**

[![Video Preview](/docs/_remote/26ae9b1db5276e225497fc37.webp)](https://vz-8a0704eb-552.b-cdn.net/bb51a195-4a80-48d3-badd-0bfe6cc9e3b3/playlist.m3u8)

* **将粒子骨架合并到道具骨架**

[![Video Preview](/docs/_remote/5a12aec98b02dff46c6ea7a5.webp)](https://vz-8a0704eb-552.b-cdn.net/9f512c34-ae83-42fe-a858-165ccea6d0c5/playlist.m3u8)


> [!NOTE]
> **注意！**
> 
> ⚠️ 本文档不会讲解创建粒子系统。目标是帮助你将其应用到你的表情动作中。


## 将粒子转换为骨架

如前所述，Emotes 2.0 不支持物体动画或任何形式的粒子系统。这就是我们需要将粒子转换为骨架的原因。

开始之前，你需要一个可用的粒子系统和该插件。下面展示的粒子系统非常简单，它只是一个示例，用来说明插件的工作方式。请确保你的粒子系统与表情动作动画相匹配，以获得更好的效果。

![](/docs/_remote/221d5be3906962eace9e413c.webp)

*简单的粒子系统。*

接下来，下载并安装该插件。为此，进入 Edit > Preferences > Add-ons，然后点击 Install，在电脑中浏览该文件。

![](/docs/_remote/7494e551d315ebd81e317174.webp)

*点击 Edit > Preferences。*

![](/docs/_remote/455940b9766bf96f12df39e3.webp)

*在右侧选择 Add-ons，然后点击 Install。*

完成后，点击复选框启用它。视口右侧会出现一个名为 Converter 的选项卡。

![](/docs/_remote/4a04786944a55dc6ea9f1b36.webp)

*点击复选框以启用该插件。*

![](/docs/_remote/c89bb19680957280f6fbecbf.webp)

*你会在视口右侧找到 Converter 选项卡。*

接下来，选择对象，进入 Converter 选项卡，按需重命名 Output Collection，设置开始和结束帧，然后点击 Particle System to Armature Converter，如下所示。

![](/docs/_remote/e7e1f6344eb6ec89ec8c469f.webp)

你会注意到，大纲视图中创建了一个集合，其中包含粒子骨架。同时还生成了一个动画片段，通过烘焙每个对象的所有运动得到。

![](/docs/_remote/d4067e87e6fe33342f19de70.webp)

*由插件生成的骨架。*

![](/docs/_remote/b2b2e68b9a2d4d5dc47d2c10.webp)

*由插件生成的动画片段。*

将粒子转换为骨架就到这里！记得用不同的名字保存文件（保留原文件作为备份），下一步我们还会用到它。


> [!NOTE]
> **注意！**
> 
> ⚠️ 粒子应当是添加到表情动作中的最后一个元素。当表情动作准备完成时，你将拥有动画的准确时长，这会让粒子更容易与所执行的动作同步。


## 将粒子骨架合并到道具骨架

Emotes 2.0 每个文件只能有两个骨架和两个动画片段。如果我们直接导入粒子，最终会得到三个骨架和三个动画片段。解决方法是把粒子骨架合并到道具骨架中。之所以合并到道具而不是头像骨架，原因很简单：头像骨架不能通过添加或删除骨骼的方式进行任何编辑。因此，道具骨架就成了唯一的选择。

## 将粒子骨架合并到道具骨架

Emotes 2.0 每个文件只能有两个骨架和两个动画片段。如果我们直接导入粒子，最终会得到三个骨架和三个动画片段。解决方法是把粒子骨架合并到道具骨架中。之所以合并到道具而不是头像骨架，原因很简单：头像骨架不能通过添加或删除骨骼的方式进行任何编辑。因此，道具骨架就成了唯一的选择。

我们首先要做的是导入或附加由插件生成的集合。你也可以先将它导出为 glb 文件，再导入到表情动作文件中。或者你也可以直接附加该集合，这样更简单也更快捷，方法是进入 ***文件*** > ***附加***，在电脑中浏览粒子文件，选择文件夹 ***集合*** 并选择所需的集合。

![](/docs/_remote/9ca230b3c18bf89586ff9347.webp)

*要附加一个集合，请进入 File > Append。*

![](/docs/_remote/1f0417578e1b9a2dcae76f3e.webp)

*浏览粒子文件后，选择 Collection 文件夹。*

![](/docs/_remote/539369b8db786f772b98368b.webp)

*选择由插件生成的集合（你有机会重命名它）。Particles\_Out 只是这个示例中的重命名方式。*

接下来，我们要合并骨架。在对象模式下，先选择粒子骨架，然后按住 Shift 再选择道具骨架，按 Ctrl+J 将它们合并，如下图所示。

![](/docs/_remote/c6adaaf3d6e33639b4ce3f4e.webp)

完成后，粒子可能会看起来乱掉了，但不用担心，这是流程的一部分。要修复它，依次在大纲视图中选择每个粒子对象，进入对象属性（橙色方块图标），在 Relations 下检查其父级是否为 Armature\_Prop。对所有粒子对象都这样做。这表示所有对象都是 Armature\_Prop 的子级。

![](/docs/_remote/bed704b9a320a1befd208977.webp)

*确保 Armature\_Pop 是每个粒子对象的父级。*

对所有粒子对象都完成后，在大纲视图中选择第一个粒子对象，进入修改器（蓝色扳手图标），此时 Object 字段会是空的。由于这里是修改器选项卡，这意味着你需要在这里选择由哪个骨架来驱动该对象。因此，只需点击 Object 并选择 Armature\_Prop，然后对所有粒子对象重复此操作。在你这样做的过程中，你会发现粒子对象会恢复到原来的大小，修复骨架合并时造成的混乱。

![](/docs/_remote/39489288235ee5b1b5748410.webp)

*选择合适的骨架来驱动这些对象。*

现在骨架已经正确合并并设置好了，但还差最后一步才能正常工作。如果你尝试播放动画，会发现粒子没有移动。这是因为它们现在已经成为 Armature\_Prop 的一部分，而它播放的是道具动画。记住粒子原本有自己的动画片段？我们现在要做的是把粒子动画片段中的关键帧复制并粘贴到道具动画片段中。

为此，进入 ***姿态模式*** ，并在大纲视图中展开 ***Armature\_Prop*** 然后 ***姿态***。选择属于粒子的所有骨骼。

![](/docs/_remote/10184a7f4c04c033592867a1.webp)

*选择所有粒子骨骼。*

选中骨骼后，在动作编辑器中，选择包含所有关键帧的粒子原始动画片段。按 A 选择所有关键帧，右键单击并复制关键帧。然后选择道具动画，在第一帧设置一个关键帧，再粘贴来自粒子动画片段的关键帧。

![](/docs/_remote/b16528d9ab25ee97d3d824e0.webp)

*将粒子动画中的帧复制到道具动画中。*

这样就完成了！保存文件，确保删除原始的粒子动画片段，然后像平常一样导出。

如果你需要更多操作信息，请查看 ***导出*** 部分，位于 [为你的表情动作添加道具和声音](/docs/creator/wearables-and-emotes/emotes/props-and-sounds/) 文档。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chuan-dai-wu-yu-biao-qing-dong-zuo/biao-qing-dong-zuo/particles-in-emotes.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
