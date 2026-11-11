> 创建 Decentraland 表情动作的技巧与指南。

# 创建表情动作

本文档将介绍文件规格、Blender 动画基础、如何正确导出表情动作，以及如何将其导入 Builder。


> [!NOTE]
> **💡 提示**：安装 [Decentraland Tools Blender 插件](https://extensions.blender.org/add-ons/decentraland-tools/)。它包含多个实用功能，可帮助你编辑和导出 3D 模型、穿戴物和表情动作。


#### 动画规格表

| 帧率         | 30 fps              |
| ---------- | ------------------- |
| 最大长度       | 10 秒（300 帧）         |
| 每个文件中的动画数量 | 1                   |
| 导出格式       | .glb                |
| 采样率        | 默认 1（如有需要可设为 2 或 3） |
| 最大文件大小     | 3 MB                |
| 动画最大位移     | 1 米（前/后、左/右）        |
| 动画最大高度     | 4 米                 |

你可以在下方找到对动画规格更详细的说明 [**下方**](#the-animation-specifications).

### **资源**

本文档说明 Rig 1.0 的设置、控制和功能。

[Decentraland Blender 骨骼绑定](https://github.com/decentraland/docs/tree/main/creator/images/emotes/Avatar_File.blend)


> [!NOTE]
> 如果你使用 Maya，可以下载这个 [Maya 骨骼绑定](https://github.com/decentraland/docs/tree/main/creator/images/emotes/DCL_Maya_Rig.ma) 和 [选择器](https://github.com/decentraland/docs/tree/main/creator/images/emotes/emoteAvatar.pkr) 由 [SparkleStudios](https://www.sparkles.studio/) ❤️。


## **开始之前**

### **帧率**

在开始之前，先检查帧率很重要。Decentraland 的动画必须使用 30 fps。该骨骼文件默认是 24 fps，因此在开始制作动画前请将其改为 30（错误的帧率会影响动画速度）。该选项可在下方所示的格式中的“输出属性”（打印机图标）里找到：

![开始前请确保帧率设置为 30 fps。](/docs/_remote/53650a16525c3ac92e8eef4c.webp)

开始前请确保帧率设置为 30 fps。

### **姿态模式**

在 Blender 中，骨骼绑定可在三种不同模式下查看：对象模式、编辑模式和姿态模式。动画只能在姿态模式下制作（在该模式下，控制器会显示颜色）。选中骨骼后，你会在右上角的下拉菜单中找到该选项。

![切换到姿态模式。](/docs/_remote/2d50e8d239676da8c48b0736.webp)

切换到姿态模式。

### **动画界面**

在骨骼文件中，除了视口的两个窗口（正视图和侧视图）外，底部还有三个窗口：一个 ***曲线编辑器***, ***摄影表***，以及一个 ***时间轴***.

* ***曲线编辑器***：在此编辑器中，可以编辑所选控制器每个变换属性的动画曲线。这些曲线显示插值是如何计算的，可以通过编辑它们来达到动画想要的效果。在这里以及在摄影表中，都会启用“仅显示所选”工具，这意味着它只会包含与所选控制器相关的通道。你可以通过点击箭头图标来开启或关闭它。 ***仅显示所选*** 工具，这意味着它只会包含与所选控制器相关的通道。你可以通过点击箭头图标来开启或关闭它。
* ***摄影表***：在这里可以编辑关键帧。你也可以在此创建新的动画或浏览已创建的多个动画。请记住，要访问动画，必须选中“动作编辑器”选项。该选项在图标旁边的下拉菜单中。 ***动作编辑器*** 必须被选中。该选项就在 *摄影表* 图标旁的下拉菜单中。
* ***时间轴***：这里可以找到时间轴和播放控制。在这里，“自动插入关键帧”是开启的，这意味着每次操作控制器时都会自动创建一个关键帧。你始终可以通过点击播放控制旁边的圆点来禁用该功能。 ***自动插入关键帧*** 是开启的，这意味着每次操作控制器时都会自动创建一个关键帧。你始终可以通过点击播放控制旁边的圆点来禁用该功能。

有了这个工作区，你就拥有开始制作动画所需的一切！

![](/docs/_remote/4dfc916aa7bf8bcc00b1659f.webp)

这些是底部窗口。上面的那个在 ***曲线编辑器中，*** 中间的那个在 ***摄影表中，*** 而底部的那个是 ***时间轴。*** 上方红色箭头表示 ***仅显示所选*** 工具，而下方箭头表示 ***自动插入关键帧***.


> [!NOTE]
> **💡 提示！**
> 
> 由于 Blender 的自定义程度很高，现在也是设置最适合你的布局的好时机，你可以添加、调整或移除窗口。每位动画师都有自己的偏好，所以请随意按你的需要编辑布局！


## 开始

#### **起始姿势**

在提供的骨骼文件中，已经有一个动作： ***Starting\_Pose***。考虑到所有虚拟形象动作都从待机姿势开始， **我们非常建议你从该姿势开始制作动画，并且在最后一帧再次使用它**。这样可以让从待机到表情动作的过渡更自然，动画也更流畅。


> [!NOTE]
> **💡 提示！**
> 
> 如果你想做循环动画，就不必从起始姿势开始。请自由使用最符合你动画的姿势！\*\*


要让循环顺畅运行，第一帧和最后一帧关键帧必须完全相同。例如，在 30 fps 下做一个 4 秒循环，就需要第 1 帧和第 121 帧使用相同姿势。如果二者有任何不同，玩家每次循环时都会看到明显的跳变。

#### 动画区域 <a href="#ground-reference-and-animation-area" id="ground-reference-and-animation-area"></a>

在提供的虚拟形象文件中，你会找到一个名为 ***Animation\_Area\_Reference***。它由三个对象组成，可直观帮助你设置动画边界： ***Animation\_Area\_Reference***, ***Ground\_Reference*** 和 ***Root\_Animation\_Area***.

* Animation\_Area\_Reference 是一个 4x4 的立方体，即包围盒。虚拟形象网格和道具应在整个动画过程中保持在这个区域内。只要道具始终位于立方体区域内，就可以自由移动。
* Ground\_Reference 是一个平面，用于帮助检查动画过程中是否有穿地情况。它还包含重要信息：较小的圆，其直径为 2 米，定义了虚拟形象根部可移动的区域。这意味着在地面上时，虚拟形象每个方向最多只能移动 1 米：前、后、左和右。
* Root\_Animation\_Area 是一个圆柱体，定义了你可以移动虚拟形象根部/重心（CTRL\_Avatar\_UpperBody 或 Avatar\_Hips）的区域。该圆柱体高 4 米，这意味着只要没有网格超出包围盒，根部就可以一直向上移动。虚拟形象的腿、手臂和其他身体部位可以位于圆柱体外，只要它们从未超出 Animation\_Area\_Reference 即可。

![动画区域参考。](/docs/_remote/8ef56331d4f29f23223dc576.webp)

总结一下，只要根部保持在圆柱体内，虚拟形象就可以向右、向左、向前和向后移动，也就是说四个方向各 1 米。

![虚拟形象移动。](/docs/_remote/7949b480b39476fbab88c60f.webp)

至于高度，只要虚拟形象保持在包围盒内，就可以一直向上移动（最多 4 米）。下面是一些正确的虚拟形象高度示例（整个身体都在包围盒内）以及一些错误示例（网格超出包围盒）。

| ![](/docs/_remote/cbee2350215541a459589276.webp) | ![](/docs/_remote/4774d078d1fff2f442dda8f4.webp) |
| -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

| ![](/docs/_remote/d51139e66aeafc6d74965579.webp) | ![](/docs/_remote/93bbdf9a30346096af762dea.webp) |
| -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

![](/docs/_remote/da31c95fa83a05f64398f245.webp)

下面是一些位于动画区域边界内的表情动作示例。

| ![](/docs/_remote/989fc61d99ce5203d6197a0e.webp) | ![](/docs/_remote/aa48c499a9817ab4725cfe89.webp) |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |


> [!NOTE]
> **💡 注意！**
> 
> 请留意这些边界，因为越界可能会导致游戏问题。


## **创建动画**

该 blend 文件中已有一个可供编辑的动画片段： *StartingPose\_Avatar*。你可以按需要复制并重命名该动画片段。无需从头创建！

在 *浏览动作* 部分，只需点击 ***创建新动作*** 按钮即可复制当前动画。要重命名片段，只需点击文本并输入其他名称。

Blender 4.4 引入了 *槽化动作*，它是 *浏览动作* ……右侧的图标，与之前版本中的该部分对应。如果你在制作不带道具的表情动作，就无需处理它，保持原样即可。如果你是在给虚拟形象做动画，请确保槽化动作是 Avatar\_Animation。

![](/docs/_remote/75912c834d5cbe632654a9f0.webp)

通过复制现有动画或点击 ***取消关联动作*** 然后 ***新建***.

### **浏览和删除动画**

在 Blender 中，同一个文件里可以有多个动画轨道。你可以通过点击“浏览动作”下拉菜单来浏览它们。所有带有 F（Fake User）的动画都会被保存。要删除动画，请按住键盘上的 Shift 并点击 X。之后，该动画旁边会显示一个 0，这表示在你下次关闭 Blender 或重新打开文件时它会被删除。

![](/docs/_remote/cf72bdeab774263920c04bb9.webp)

浏览动画：带 F 的会被保存，带 0 的会被删除。

无需重新加载 Blender 即可删除动画的另一种方法，是将显示模式从视图层改为 Blender 文件。展开“动作”，然后右键单击不需要的动画并选择“删除”。

![](/docs/_remote/3e2be89fb22f3ee63cc39ef1.webp)

你可以在大纲视图的显示模式下，直接从 Blender 文件中删除动画。


> [!NOTE]
> **💡 提示！**
> 
> 不要总是编辑同一个动画轨道。在做重大更改之前，先复制动画。这样如果你后悔删除或修改了什么，还会有一个备份版本。这也是记录目前进度的好方法！


![](/docs/_remote/75912c834d5cbe632654a9f0.webp)

复制动画片段。

### **命名**

**动画名称应以大写字母开头；如果名称不止一个单词，单词之间应使用 \_ 分隔。** 不要使用空格或特殊字符。以下是一些命名示例：

* Snowfall
* Rainbow\_Dance
* Throw\_Money
* Talk\_To\_Hand

### **表情动作覆盖**

当变形骨骼在某个参数上没有设置关键帧时，就会发生表情动作覆盖。没有关键帧时，该骨骼就没有关于它应在何处、旋转和缩放了多少的信息，从而使该通道保持开放。其结果是，如果你在世界中播放一个表情动作，然后在前一个动作仍在播放时触发你自己的动作，位置信息、旋转和缩放信息会被前一个表情动作覆盖，从而导致两个动作叠加。除非这是有意为之，否则会影响你的动画，有时会产生有趣的结果，但有时会把表情动作完全弄乱。下面是一个表情动作覆盖的示例。

![](/docs/_remote/4d3cf50ba093eedb38f43aee.webp)

为避免这种情况，请选择所有包含骨骼的图层（可在 ***对象数据属性*** > ***骨架*** > ***图层***&#x4E2D;找到）。然后，在 ***姿态模式***&#x4E2D;，将时间轴光标停留在动画的第一帧，并让鼠标位于 ***视口显示***&#x4E2D;，按 ***A*** 来全选。在 ***曲线编辑器***&#x4E2D;，双击 ***眼睛*** 图标，显示骨架通道旁的所有通道。选中所有骨骼后，按 ***I*** 来设置关键帧。对最后一帧也执行同样操作。

**请务必选择变形骨骼，这一点尤其重要！** 变形骨骼位于最下面一层，在 ***视口***.

![](/docs/_remote/359c9a9a72aea9590225e2c7.webp) 中显示为绿色骨骼

## **在第一帧和最后一帧为所有骨骼设置关键帧可以防止表情动作覆盖。**

### **动画长度**

动画的最大长度为 **10 秒** 或 **300 帧**。请记得在第一帧和最后一帧为每个控制器的属性设置关键帧。


> [!WARNING]
> ⚠️ 在曲线编辑器中关闭可见性的通道不会被设置关键帧、删除，甚至不会显示在动作编辑器中。除非是有意为之，否则请特别注意可见性。


![](/docs/_remote/91ee3dde0d4a300d0eb09a66.webp)

设置关键帧前先让通道可见！

### **动画数量**

如果是标准表情动作（不带道具），导出的文件只能包含一个动画。对于表情动作 2.0，你可以为虚拟形象保留一个片段、为道具保留一个片段。如果在制作过程中复制了动画，请确保在导出前将它们全部删除，只保留最终版本。目前还不支持需要多个动画才能工作的序列表情动作（动作开始、动作循环和动作结束）。

### **格式**

动画应导出为 .**GLB**。文件中只能包含变形骨架和动画。 **网格、控制器以及任何其他对象都不应导出**。关于如何导出的更多详情可以在 [**下方**](#exporting).

### **采样**

由于约束无法导出，导出动画片段的唯一方式是烘焙它，这意味着所有变形骨骼的位置、旋转和缩放都会在动画的每一帧中设置关键帧。如果片段太长，比如达到 300 帧，那么导出后会有 300 个关键帧；关键帧越多，文件就越大。

采样是优化动画的好方法。采样率决定关键帧在动画中被烘焙的频率。例如，如果采样率设为 2，就表示每 2 帧创建一个关键帧。采样率为 3 时，则每 3 帧烘焙一个关键帧，以此类推。采样率越高，文件越小。

但缺点是，动画会变得越来越不流畅，因为它会丢失一些重要关键帧（这些关键帧在动画中分布并不均匀）。另外还要注意， **采样并不是用动画帧数除以采样率**.

通常， **2 或 3 的采样率** 就足够了。这些数值可以在不影响质量的情况下优化动画。


> [!NOTE]
> **💡 提示！**
> 
> 如果动画帧数可以被采样率整除，那就更好了！这意味着最后一帧也会被烘焙，从而保留动画从结束到开始的过渡。


### **文件大小**

最大文件大小为 **3 MB**。如果导出后文件超过该大小，请检查是否误导出了网格，或者动画是否超过 10 秒。如果仍然超过 3 MB，可以尝试调整采样率，因为更高的数值会提升优化效果。

如果表情动作包含任何额外的 3D 模型，这些模型中的纹理尺寸不能超过 1024 像素。

## **导出**

由于我们只希望导出骨架和动画，请在导出前关闭网格以及骨架以外任何对象的可见性，如下所示：

![](/docs/_remote/3b011135dba6d9de9e62a101.webp)

导出前请关闭网格可见性！

你的文件中也必须只包含你要导出的那个动作。该骨骼文件自带一个 `Starting_Pose` 动作，所以 [将其删除](#browsing-and-deleting-animations) 以及任何其他残留动作，否则导出的 `.glb` 会包含两个片段，而不是 [每个文件一个动画](#number-of-animations) 这是 Decentraland 所要求的。请先保存一份你的 `.blend` 可编辑副本，然后再删除内容。

要导出，请前往 *文件* > *导出* > *glTF2.0（.glb、.gltf）*

![](/docs/_remote/3142125b130d9e9715dddd3d.webp)

在导出设置中，展开 Include，并在 Limit to 中勾选 Visible Objects。然后展开 Data 选项卡，再展开 Armature，并启用 Export Deformation Bones Only。

| ![](/docs/_remote/446d202239ea82992946aa43.webp) | ![](/docs/_remote/eef0447fdcd598e5153a2669.webp) |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

如果你需要对动画进行采样，请展开 Animation 选项卡，展开 Sampling Animations，并选择所需的采样数量。

| ![](/docs/_remote/b5ee46c688eb78bff045e6a1.webp) | ![](/docs/_remote/6813e1cd152caee20fe515fb.webp) |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

导出动画到此结束！

## 参考资料

如果你仍然不确定从哪里开始，或者需要一些参考或灵感，这里有一些动画片段可以帮助你。这些也可以作为很好的学习材料！

[Idle.glb](https://raw.githubusercontent.com/decentraland/documentation-creators/main/images/emotes/idle.glb)

[Jump.glb](https://raw.githubusercontent.com/decentraland/documentation-creators/main/images/emotes/jump.glb)

[Walk.glb](https://raw.githubusercontent.com/decentraland/documentation-creators/main/images/emotes/walk.glb)

[Run.glb](https://raw.githubusercontent.com/decentraland/documentation-creators/main/images/emotes/run.glb)

[Pose\_Jump.glb](https://raw.githubusercontent.com/decentraland/documentation-creators/main/images/emotes/pose_jump.glb)

[Pose\_Spin.glb](https://raw.githubusercontent.com/decentraland/documentation-creators/main/images/emotes/pose_spin.glb)

[Spotlight.glb](https://raw.githubusercontent.com/decentraland/documentation-creators/main/images/emotes/spotlight.glb)

[Fashionista.glb](https://raw.githubusercontent.com/decentraland/documentation-creators/main/images/emotes/fashionista.glb)

[Chic.glb](https://raw.githubusercontent.com/decentraland/documentation-creators/main/images/emotes/chic.glb)

[Flag\_Emote.glb](https://github.com/decentraland/docs/tree/main/creator/images/emotes/Flag_Emote.glb)

[Flag\_Emote.blend](https://github.com/decentraland/docs/tree/main/creator/images/emotes/Flag_Emote_Final.blend)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chuan-dai-wu-yu-biao-qing-dong-zuo/biao-qing-dong-zuo/creating-emotes.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
