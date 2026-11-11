> 如何使用 Scene Editor

# Scene Editor 基础

场景编辑器的 UI 分为几个不同部分，各有不同用途。

![](/docs/_remote/ee9a6732ea7a70cc625e7341.webp)

* **画布**：可直接操作物体并查看场景的样子。
* **实体树**：包含场景中所有物体及其层级的列表。
* **属性**：显示当前所选物体的详细信息。
* **资源**：显示可用资源。

## 移动浏览

要在场景编辑器中熟悉各个部分：

* 使用 **W** 和 **S** 来靠近或远离。你也可以使用鼠标滚轮，或者 **+** 和 **-** 按键
* 使用 **一个** 和 **D** 来横向移动。
* 使用 **Q** 和 **E** 来上下移动。
* 点击 **鼠标右键** 并拖动来旋转摄像机。
* 按 **空格键** 将摄像机重置回默认位置
* 使用 **鼠标左键** 来点击并选择项目，以及移动它们。

## 在移动端预览

该 **播放选项** 菜单旁边的 **播放** 按钮有一个 **显示移动端二维码** 选项。用与其处于同一 Wi‑Fi 网络的手机扫描二维码，即可在 [Decentraland 移动应用](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/yi-dong-ke-hu-duan/overview/)中打开你的场景。这是为移动端玩家验证 UI、控制和性能的推荐方式。请参阅 [在移动端预览](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/preview-on-mobile/) 完整指南。

## 设置地面

场景的地面可以使用各种不同的纹理。你可以在物品菜单中的不同主题资源包里找到它们。

类型为 **地面** 的物品上带有油漆桶图标。将其中一个拖入场景后，它会用该物品的副本覆盖整个场景地面。

![](/docs/_remote/e120bb99677fe9cb301458fb.webp)

你也可以通过按住 **Shift** 并将地面拖到场景中，来单独添加一个该物品的副本。

![](/docs/_remote/df027a655d783fcc8276005c.webp)

地面物品集合显示在 [实体树](#the-entity-tree) 中的一个文件夹里。它们每一个都已锁定，以防止被意外选中。 [取消锁定](#lock-or-hide-items) 这些物品即可移动或编辑它们。

## 添加项目

使用底部菜单中的主题资源包分类来浏览，找到可放置到场景中的不同项目。

![](/docs/_remote/c5121b56db02ac73b992d99e.webp)

你也可以使用搜索框。请注意，当你位于资源包内时，搜索只会在该资源包内查找。

要放置项目，请从资源包菜单中点击并拖动它到画布中场景的某个位置。

![](/docs/_remote/b20b9f38d71f91b4c4270633.webp)


> [!NOTE]
> **💡 提示**：只要你添加、移动或编辑场景中任何物品的属性，更改都会自动保存。


要复制物品，先选中它，然后按 **Ctrl + C** 然后 **Ctrl + V**。你也可以在 [实体树](#the-entity-tree) 上右键单击并选择 **复制**选项。新物品将与原物体完全重叠。

要从场景中删除物品，先选中它，然后按 *删除* 键。

参见 [导入物品](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/gou-jian/import-items/) ，用于从磁盘添加你自己的自定义 3D 模型。


> [!WARNING]
> **📔 注意**：一旦你把 3D 模型拖入场景，它就会下载到你的项目文件夹中，即使你删除它也仍会保留在那里。这些未使用的模型会增加场景大小。
> 
> 打开 **本地资源** 选项卡以删除任何未使用的模型。


## 放置物品



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/cNl02PFPdcQ" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=cNl02PFPdcQ>">Watch on YouTube</a></p>



单击 **鼠标左键** 并拖动所选物品，即可在场景中地面高度自由移动它。

你也可以使用顶部菜单中的工具：

![](/docs/_remote/c6834cebdebce2e0fb6608ba.webp)

* **移动工具**：单击并拖动每个箭头即可一次沿一个轴移动物品。使用此工具，你还可以将物体放置在地面以上。
* **旋转工具**：单击并拖动物体周围的每个圆环，即可一次绕一个轴旋转物体。
* **缩放工具**：单击操纵器中心并向内或向外拖动即可放大。此工具还允许你沿单个轴拉伸物体以改变其比例；为此，请单击操纵器的某个轴并拖动。

![](/docs/_remote/11bce750b05bad2b59a8c3b8.webp)

若想在移动、旋转或缩放物品时获得更高精度，请按住 *Shift* 键并进行调整。这将避免吸附到网格。

要更改移动粒度和其他设置，请单击工具右侧的向下箭头。可用设置如下：

* **吸附**：打开或关闭网格。关闭时， **Shift** 的行为会反转：默认情况下你不会跟随网格，只有按住 **Shift**.
  * **位置**：当 **吸附** 开启时，移动步进在米单位下的大小。
  * **旋转**：当 **吸附** 开启时，移动步进在米单位下的大小。
  * **缩放**开启时，旋转步进在角度单位下的大小。 **吸附** 开启时，移动步进在米单位下的大小。
* **与世界对齐**：一个单独的复选框，用于控制操纵器所使用的轴。勾选后，移动和旋转工具的轴始终与世界坐标对齐，不会随物体的朝向变化。未勾选时，它们会与物体的朝向对齐。

若想同时选中多个物品，请在选择时按住 *控制* 键。之后你可以一次性移动、旋转、缩放、复制或删除它们。

### 输入精确数值

当拖动不够精确时，可以直接输入数值：

1. 按下一个工具键： **M** 或 **G** 用于移动， **R** 用于旋转， **X** 用于缩放。
2. 按 **X**, **Y** 或 **Z** 选择一个轴。缩放不需要轴：在没有轴的情况下，它会按相同比例缩放所有轴。
3. 输入数字。小数请使用 **.** ，正负切换请使用 **-** 。
4. 按 **按** 应用，或按 **Esc** 取消。 **退格键** 会擦除你输入的内容。

画布底部的一个提示条会在你输入时显示当前输入，告诉你还缺少什么，并确认已应用的内容。

一些示例：

* **R** **X** **1** **5** **按** 将所选内容绕 X 轴旋转 15 度。
* **X** **2** **按** 将所选内容变成原来的两倍大小。
* **M** **Y** **-** **3** **按** 向下移动 3 米。

有几点值得了解：

* 该数值始终相对于当前所选内容的位置，而不是绝对坐标。
* 输入的值会忽略吸附网格。
* 选中多个物品时，它们会以共享中心为轴同时移动、旋转或缩放，并作为一个撤销步骤。
* 按下 **M**, **G** 或 **R** 第二次仍然会将其操纵器关闭。 **X** 不会，因为在输入时它表示 X 轴。


> [!NOTE]
> **💡 提示**：画布右下角的键盘图标可打开完整快捷键列表。


## 智能项目

智能物品是带有内置交互行为的特殊物品。请参阅 [智能项目](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/smart-items/) 以了解更多详情。

![](/docs/_remote/4401edd6b0ae269ede4c9467.webp)

## 实体树

在左侧边栏，你会看到一个包含场景中所有实体的树状结构。这包括你添加的所有物品，以及一些默认实体。


> [!NOTE]
> **💡 提示**：场景中的一切都是实体，它们是场景的基本构件。物品是至少具有位置和可见形状的实体。


你也可以不从场景的 3D 视图中点击物品来选中它，而是从树状视图中选择它。对某个实体单击鼠标右键可显示更多选项：你可以重命名、删除或复制，也可以创建子实体、将其转换为自定义物品，或者为该实体添加组件。

![在名为 Bird Fountain 的实体上打开了右键菜单的实体树，菜单中列出了重命名、添加子项、复制、删除、创建自定义物品和添加组件。](/docs/_remote/42bfc23b5b7dbda3a5127c7a.webp)

### 搜索实体树

实体树顶部的搜索框可让你按名称查找实体。输入实体名称的一部分，树就会过滤，仅显示匹配的实体及其父级层级，这样你就能看到每个匹配项在树中的位置。搜索不区分大小写。匹配项的父实体会自动展开，以便每个结果都可见。

按 **Escape** 或单击清除图标即可移除筛选并返回完整树。如果某个实体已被选中，清除搜索后会滚动回该选中的实体，这样你不会丢失位置。

![实体树按搜索词 fount 过滤：只剩下 Camera、Scene 和 Bird Fountain 实体可见，并且 Scene 父级已展开以显示匹配项。](/docs/_remote/462138368861e2d4fd3aba3d.webp)

### 实体层级

实体遵循可拥有任意多层级的层次结构。通过在树中将一个物品拖到另一个物品上，可以在两个实体之间建立父子关系。子实体会继承父实体的位置，因此当父实体移动时，其所有子实体也会随之移动。这在构建场景时很实用，例如你可以将杯子和盘子设为桌子的子实体，然后移动桌子而无需重新调整其他内容。它在与场景交互时也很重要，因为物品可以一起移动。

![](/docs/_remote/2ba2f0d9e5656796ad572f00.webp)

你也可以最小化或展开某个实体的子项来保持视图简洁，此操作不会影响场景。

### 特殊实体

场景中包含几个你可以在实体树中看到的特殊实体。

* **场景**：这指的是根实体，你在场景中添加的一切都是该实体的子项。你可以展开它以查看 [场景设置](#scene-settings).
* **玩家**：玩家头像。你可以向该实体添加特殊组件，从而改变游戏机制。你也可以将其他实体拖到头像下作为子项。如果某个实体是头像的子项，它的位置将固定在玩家身上。比如你可以用它在玩家头顶添加一个悬浮标记，让它跟随玩家移动。
* **摄像机**：玩家的摄像机。你可以将其他实体拖到摄像机下作为子项。如果某个实体是摄像机的子项，它的位置将固定在屏幕上。比如你可以用它在射击游戏中显示一把枪，即使玩家向上或向下看，它也始终可见。

### 锁定或隐藏物品

有时锁定一个物品会很方便，这样可以防止你意外选中并移动它。这对背景物品尤其有用，比如地面或建筑。要锁定一个物品，请在左侧实体树中找到它，悬停其上，然后选择锁定图标。你也可以通过同一个图标切换此行为的开关。

你也可能想隐藏某个在放置其他物体时会遮挡视线的物品。这对在处理室内时隐藏屋顶或建筑尤其有用。被隐藏的物品只会在场景编辑器的画布窗口中隐藏，不会对进入场景的玩家隐藏。要隐藏一个物品，请在左侧实体树中找到它，悬停其上，然后选择眼睛图标。你也可以通过同一个图标切换此行为的开关。

![](/docs/_remote/1360d2763c98c1ac75eca0d4.webp)

## 属性面板

通过在画布或实体树中单击来选中一个物品。随后你会在屏幕右侧的属性面板中看到其组件。不同物品有不同组件，每个组件都会显示特定设置。

![](/docs/_remote/8c1945559d13f38d8a485e7a.webp)

大多数非交互式物品具有以下组件：

* **Transform**：设置物品的位置、旋转和缩放。
* **GLTF**：要加载哪个 3D 模型。

[智能项目](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/smart-items/) 可以包含其他组件。

参见 [组件](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/gou-jian/components/) 了解更多。

## 场景限制

Decentraland 场景需要遵循某些限制，才能让多个场景可以并排运行。材料、纹理、三角形等的最大数量与场景中的地块数成正比。请参阅 [场景限制](/docs/creator/content-creator-zh/chang-jing-sdk7/you-hua/scene-limitations/) 以了解更多详情。

如果你场景中的内容超出这些限制，场景编辑器会在左下角通知你。

![](/docs/_remote/ab87e37107598ee524831639.webp)

你可以展开此菜单查看详细信息。

![](/docs/_remote/8d4619b1778683c8d72d3c61.webp)


> [!NOTE]
> **💡 提示**：如果你正在构建 Decentraland World，可以随时增加 [场景大小](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/pei-zhi/scene-settings/#layout) 来提高你的限制。


Decentraland 场景中的内容还必须避免溢出到相邻地块。如果场景中模型的任何部分超出边界，当你打开场景预览时，会看到这些部分被裁切。场景编辑器会将整个模型标记为红色，但你真正需要关注的只是超出场景限制的那部分模型。

![](/docs/_remote/8dcb8f74f41494a20d11ba4d.webp)


> [!NOTE]
> **💡 提示**：如果你想显示的模型放不下，可能需要增大场景大小。请参阅 [场景大小](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/pei-zhi/scene-settings/#layout) 以扩大你的场景。


即使 3D 模型的整个几何体都能放进场景中，如果模型的包围盒超出区域，它也可能被标记为红色。若是这种情况，你可以忽略这些警告，因为整个模型仍会被正确显示。了解更多关于 [包围盒](/docs/creator/content-creator-zh/3d-jian-mo-yu-dong-hua/meshes/#bounding-boxes).

## 整理资源

通过移除不再使用的资源，让你的项目保持整洁。打开 **本地资源** 选项卡，然后点击刷子图标。

![](/docs/_remote/98ff3e388967f6b98ae40ab1.webp)

会打开一个对话框，列出场景中的所有资源，并高亮显示那些未被任何物品引用的资源。

![](/docs/_remote/a20d113518b454ef45385873.webp)

使用复选框选择要删除的资源，然后点击 **删除所选项** 以永久移除它们。

从场景中删除物品不会删除其文件。导入的模型、纹理或声音会保留在你的项目中，直到你清理它们，因此请定期检查未使用的资源。


> [!WARNING]
> **重要：** 如果你的场景包含引用资源的代码，某些正在使用的资源可能会被显示为未使用。此对话框只能检测 Creator Hub UI 中由组件引用的资源。点击 **删除所选项**之后，文件将从项目文件夹中删除，此操作是永久性的，无法撤销。


## 场景设置

点击 **铅笔图标** 位于屏幕右上角。这里会打开一系列场景级属性供你编辑。

![](/docs/_remote/a64bc0ae30e4d852f97ccd78.webp)

在这里你可以配置多项属性，包括标题和缩略图、场景大小、场景类别和年龄分级、玩家出生点以及功能开关。

参见 [场景设置](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/pei-zhi/scene-settings/).

## 另请参见

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
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/scene-editor-essentials.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
