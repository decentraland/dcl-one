> 了解如何为 Decentraland 中的角色创建骨架。

# 创建骨架

本文档展示了如何在 Blender 中设置一个基础骨架。如果你想更熟悉一些骨架绑定概念，可以查看 [《骨架：基础概念》](/docs/creator/wearables-and-emotes/emotes/avatar-rig/).


> [!NOTE]
> **💡 提示**：安装 [Decentraland Tools Blender 插件](https://extensions.blender.org/add-ons/decentraland-tools/)。它包含一些实用功能，可帮助你编辑和导出 3D 模型、可穿戴物品和表情。


### 添加骨架

首先，你需要将你的 3D 模型导入 Blender；如果你已经在 Blender 里完成了它，就直接打开你的 blend 文件。在对象模式下，按 `Shift+A` 并选择 ***骨架*** 在菜单中，如下所示。然后，在 ***对象数据属性***，在 ***视口显示***，勾选 ***前置***，这样你就能透过网格看到骨骼。网格和骨架应该对齐，所以在添加骨骼之前，请确保模型已正确放置在世界中心。骨架的原点应位于 0,0,0（X、Y、Z）。

![](/docs/_remote/c2e0666e9dda208616b7bea9.webp)

*添加骨架并使其透过网格显示。*

### 编辑骨架

添加骨架对象后，选中它并进入 ***编辑模式***。在这里你可以通过挤出原始骨骼来添加其他骨骼（按 `E`）`Shift+D`）或者直接用 `Shift+A`。骨架应当跟随模型的形状，就像真实骨架一样。骨骼可以缩放、抓取和旋转，并且有两部分：头部和尾部。头部是枢轴点，这意味着旋转和缩放都会从那里开始。这也意味着网格变形会在那里发生，所以要尽可能将骨骼放在网格中心。

![](/docs/_remote/8512ec54c085799325f2d767.webp)

*骨骼结构。*

![](/docs/_remote/edf9b89cb68b376886dd222e.webp)

*骨骼围绕其枢轴点旋转。*

现在无需担心为右侧和左侧都创建骨骼。先专注于只创建一侧。

### 重命名骨骼

需要牢记的一件非常重要的事是，按照 [Blender 的命名规范](https://docs.blender.org/manual/en/latest/animation/armatures/bones/editing/naming.html)正确重命名所有骨骼。这不仅能让一切保持有序，还能让镜像姿态和权重绘制成为可能。你可以直接把它重命名为它所代表的身体部位，比如 Spine\_01；或者更具体一些，命名为 DEF\_spine.001（DEF 代表变形骨骼，所以你就知道这根骨骼属于变形层级）。

### 骨骼朝向

在继续之前，你应该检查骨骼朝向。你可以通过进入 ***对象数据属性*** 并在 ***视口显示***，勾选 ***坐标轴***。这样会显示骨骼的轴向，以便你检查它们是否会朝正确的方向旋转。

轴应与骨骼希望旋转的方向对齐。如果感觉有些不对，选中骨骼，并在 ***项变换*** 视口右侧的菜单中调整 ***滚转*** ，直到朝向合适为止。

![](/docs/_remote/e985c26fabd10eaadd09c1d3.webp)

*修正骨骼滚转。*

![](/docs/_remote/9737a17ebb3d4b75ab40ecbe.webp)

*开启轴显示，这样你就能看到骨骼旋转的方向。*

修正骨骼滚转对手指尤其重要，这样所有关节都会朝相同的正确方向弯曲。不过你不必为手指中的每根骨骼都这样做，只要先把其中一根调好即可。然后，先选中你想修正的骨骼，再最后选中那根滚转正确的骨骼，进入 ***骨架 > 骨骼滚转 > 重新计算滚转 > 活动骨骼*** ，它们就会拥有相同的滚转。或者你也可以直接按 `Shift+N` > ***活动骨骼***。这可用于许多拥有长骨骼链的情况，例如触手、尾巴、长马尾或机械臂。

![](/docs/_remote/a781e753f3bb59cd494a2002.webp)

*按 `Shift+N` 以重新计算骨骼滚转。*

### 镜像骨架

你不必为两侧都创建骨骼。先做一侧即可，等你修正好所有命名和骨骼朝向后，选中你想镜像的所有骨骼，右键单击并选择 ***对称化***。这样所有骨骼都会正确镜像，并保持正确的朝向。请记住，为了让此工具生效，骨骼需要按照 Blender 的命名规范命名。如果名称中有小写或大写的 “L”、“R”、“left” 或 “right”，Blender 就会正确处理对应项。

正确命名规范的一些示例：

* Hand\_L –> Hand\_R
* Hand.L –>Hand.R
* HandLeft –> HandRight

**以上示例都有效，但只选择一种并在整个骨架中保持一致。**

![](/docs/_remote/de49fec9cde1c419a8de093f.webp)

*使用 Symmetrize 选项来镜像你的骨骼。*


> [!NOTE]
> **⚠️ 注意！** 只有在骨骼已正确重命名时，Symmetrize 才能正常工作。如果镜像不正确，你可能需要仔细检查骨骼名称。


### 蒙皮

蒙皮是将网格绑定到骨架的过程。为此，在 ***对象模式***&#x4E2D;，先选中网格，再选中骨架，然后按 `CTRL+P` > ***骨架变形 > 使用自动权重。*** 然后，进入 ***姿态模式*** 并测试不同姿态，以检查网格变形。大多数情况下都需要一些调整，如下所示。

![](/docs/_remote/6b45f045f5eacbbaf6f518ce.webp)

*在蒙皮过程中，通过按 `CTRL+P` ，将网格设为骨架的子级，然后在姿态模式下测试。*

如果你觉得某根骨骼放错了位置，或者枢轴点不准确，你始终可以回到骨架的 ***编辑模式*** 并进行调整。这完全不会影响蒙皮，而且你随时可以使用 ***对称化*** 来镜像这些更改。

要修复奇怪的变形，请进入 ***对象模式***，选中网格并进入 ***权重绘制***。在 ***对象数据属性*** 中，你会找到 ***顶点组***.

![](/docs/_remote/a2dbfb8ab2995b61fc35c031.webp)

*在权重绘制中编辑骨骼的影响。*

在开始编辑之前，如果想让权重绘制在绘制时自动镜像，请开启 ***镜像顶点组*** 并选择 ***X*** 在 ***镜像*** 在 ***对称性*** 下（在 Blender 3.4.0 中）要编辑影响，请选择你想要的顶点组，并在 ***工具*** 菜单中，在 ***混合***，在……之间切换 ***添加*** 和 ***减去*** 根据需要。你还可以更改画笔的大小、权重和强度。

![](/docs/_remote/3a6194f2b0653962efb783c3.webp)

*将混合方式改为添加或移除影响。确保对称性设置正确。*

![](/docs/_remote/7787147101d32c50c282e5c6.webp)

使用 ***模糊*** 工具，位于屏幕左侧，用于平滑权重绘制并使其更均匀。

![](/docs/_remote/94e217fd4c7d693055d4cc02.webp)

*模糊工具。*

### 设置 IK

当你希望某个东西保持在原位时，IK 非常重要。它最典型的用途是在腿部，但也可以用于各种情况，例如下面这些示例：

![](/docs/_remote/ae34643f4671eb5037f61506.webp)

*IK 在机械臂中的应用。*

![](/docs/_remote/af572bd577d62ebeb0784242.webp)

*IK 在电缆中的应用。*

由于它会改变骨骼的层级结构，最好将其作为单独的设置。首先要做的是复制将受 IK 影响的骨骼链。以腿为例。选中这些骨骼，按 `Shift+D` 来复制它们，并通过按 `M` 并为其选择另一个槽位。

记得重命名复制出来的骨骼，在名称中加入 IK，这样你就知道它们属于 IK 设置。由于它们不应当变形网格，请选中它们全部，并在 ***骨骼属性***&#x4E2D;，按住 `Alt` 并取消勾选 ***变形***.

![](/docs/_remote/3b93713349f793f06d7f6bcb.webp)

*复制骨骼并将其移动到另一层。*

![](/docs/_remote/fb47b25da4bfe5bf1f79c139.webp)

*选中所有 IK 骨骼，并在按住 `Alt`时取消勾选变形。*

接下来，你需要创建一根驱动 IK 链的骨骼。选中脚部骨骼的头部并沿 Y 轴挤出它。然后，按 `Alt+P` 取消父级关系，因为 IK 骨骼不能是链的一部分，也不能与其他骨骼相连。你需要让脚部成为 IK 的子级，所以先选中脚部骨骼，再最后选中 IK 骨骼，然后按 `CTRL+P` > ***保持偏移***.

![](/docs/_remote/2538a5aac86c9d53322d27e3.webp)

*创建 IK 骨骼。*

在 ***姿态模式***，点击小腿骨，然后按 `CTRL+Shift+C` 并选择 ***反向运动学***。看起来会很乱，但别担心，等你更改几个设置后就会修复。选中小腿骨后，进入 ***骨骼约束属性*** 如下所示。

![](/docs/_remote/25208beb5685cd2437384baa.webp)

*在骨骼约束属性中，你可以编辑 IK 设置。*

对于 ***目标***，选择 ***骨架***。完成后，会出现一个名为 ***骨骼*** 的选项位于目标下方。为此，请选择你创建的 IK 骨骼。在 ***链长*** 中，你将设置应受 IK 影响的骨骼数量。由于我们在做腿部，所以希望它影响小腿和大腿，因此将其设为 ***2***.

### 极向目标

极向目标可让你控制骨骼弯曲的方向。以下是将极向目标（或极向向量）正确添加到 IK 链中的步骤：

* 在 ***编辑模式***，复制大腿骨并将其放置在任意位置。
* 选中膝关节（或大腿骨的尾部），按 `Shift+S` > ***光标到所选内容。***
* 选中复制出来的大腿骨，按 `Shift+S` > ***所选内容到光标***.
* 在 ***变换方向***，将其改为 ***法线*** 并在 ***变换枢轴点*** 将其改为 ***活动元素***.
* 在 X 轴上旋转该骨骼 -90（或 90，取决于你设置的朝向），让它指向前方，然后沿 Y 轴移动它，直到它位于腿前方的合适位置。你也可以稍微缩小它一点。

![](/docs/_remote/9eb9391b9e9f16ea7b21d708.webp)

*创建极向目标。*

回到 ***姿态模式***，再次选中小腿骨并进入 ***骨骼约束属性***。对于 ***极向目标*** 选择 ***骨架*** ，对于 ***骨骼***，选择你刚创建的极向目标。如果你显示变形骨骼，就会看到极向目标让 IK 略微旋转了。

![](/docs/_remote/a7dfd226eea1cbdac586bafa.webp)

*由于极向目标，IK 链和变形骨骼不再对齐。*

![](/docs/_remote/4c1a015b3348c877c0b7412e.webp)

*通过更改极向角即可修复旋转。*

这可以通过更改 ***极向角***&#x8F7B;松修复。通常 -90° 就能解决问题，但你也可以手动调整，以确保它们完全对齐。

最后，通过同时选中 IK 骨骼和极向目标，再选中根骨骼，然后按 `CTRL+P` > ***保持偏移***.

把大腿和小腿移动到另一层，因为动画时你不会用到它们，它们只是 IK 设置的一部分。

### 将变形骨骼绑定到非变形骨骼

IK 链已经设置好了，但它应当驱动变形骨骼，而现在还没有实现，不过你可以使用约束来修复这一点。在 ***姿态模式***，先选中 IK 链中的一根骨骼，再最后选中对应的变形骨骼，然后按 `CTRL+Shift+C` 并选择 ***复制变换***。对所有骨骼都这样做，示例中包括大腿、小腿、脚和脚趾。变形骨骼会呈绿色，这表示它们带有约束。如果你点击 ***骨骼约束属性***，就可以看到正在使用的是哪种约束，以及由哪根骨骼驱动它。

![](/docs/_remote/6b79927002f5fe0563c8468a.webp)

*绿色骨骼带有约束。你可以在骨骼约束属性中检查它们。*


> [!NOTE]
> **💡 提示**：你不必再为另一侧重新设置 IK。在 *编辑模式*中，只需删除没有 IK 的那一侧的所有骨骼，然后选中你想镜像的所有变形骨骼和 IK 链，右键单击并选择 *对称化*。它不仅会镜像骨骼，还会镜像所有约束！


![](/docs/_remote/657c90b8853090a842847bce.webp)

*也可以使用 Symmetrize 选项来镜像约束！*

### 非变形骨架和控制器\*\*

不要直接给变形骨骼做动画，这是个好习惯，因为这可能会破坏骨架，而且在导出时，给变形骨架添加约束会影响层级结构和骨骼行为。

解决办法是创建一个非变形骨架，让它通过约束驱动变形骨架，并且可以安全地进行动画，而不会有破坏任何内容的风险。你还可以自定义它们的形状，让骨骼功能更容易辨认。它们将成为你骨架的控制器。

这个过程与 IK 设置时几乎相同。使用 `Shift+D` 复制所有骨骼（除了 IK 设置）并通过按 `M` 并为其选择另一个槽位。通过添加 **Control\_** 或 **CTRL\_** 作为后缀来重命名它们，这样你就知道这些骨骼属于控制层级。按 `一个` 以全选它们，并在 ***骨骼属性***&#x4E2D;，按住 `Alt` 并取消勾选 ***变形***.

现在你需要添加约束来把这两个骨架绑定在一起。为了让这个过程更简单，你可以通过点击 ***对象数据属性*** 并在 ***视口显示***&#x4E0B;，将 ***显示方式*** 从 ***八面体*** 设置为 ***B-Bone***。然后，按 `CTRL+Alt+S` 把骨骼稍微放大一点，让它们比原始骨骼更大。

![](/docs/_remote/99d8f893af1a418a6af220a9.webp)

*更改骨骼形状并将它们放大，这样更容易区分这两个骨架。*

对于约束，先选中一个控制骨骼，再最后选中对应的变形骨骼，然后按 `CTRL+Shift+C` 并选择 ***复制变换***。对所有控制骨骼都这样做。

你可以把控制骨骼分成不同的组，并为它们分配颜色。进入 ***姿态模式*** > ***对象数据属性*** > ***骨骼组***。点击 ***+*** 图标以添加新组，按需重命名，并为其选择一种颜色。然后，选中你希望属于该组的骨骼，并点击 ***分配***。你可以根据需要创建任意多个组，以保持一切井井有条。你还可以将不同的骨骼组移动到不同的层。

![](/docs/_remote/d70aff06bf4143e4711b47e3.webp)

*骨骼组将帮助你让骨架更加有序且更直观。*

改进骨架的另一种方法是自定义骨骼的形状。为此，请进入 ***对象模式***，按 `Shift+A` 添加一个网格，例如圆形。为了让一切更有条理，为你的形状创建一个集合，并把刚创建的圆移到那里。回到姿态模式，并在 ***对象数据属性*** > ***视口显示*** 勾选 ***形状***。选中你想要的骨骼，然后在 ***骨骼属性***&#x4E2D;，点击 ***视口显示*** > ***自定义形状***。在 ***自定义对象***，在大纲视图中选择那个圆。

![](/docs/_remote/a712323c80495b4deba01b97.webp)

*自定义骨骼形状。*

如果形状感觉不对，你始终可以在 ***编辑模式***&#x4E2D;编辑它，如下所示。确保形状与骨骼具有相同的朝向。

![](/docs/_remote/6ea2f3c66b5674c66248f2bc.webp)

*在编辑模式下编辑形状。*

根据不同控制器的功能创建不同形状，让你的骨架更直观。

### 弹簧骨骼

除了标准的变形骨架外，可穿戴物还可以包含 **弹簧骨骼** —— 这些额外骨骼的变换由物理模拟而非动画驱动。弹簧骨骼能让头发、耳环、披风和其他配饰随着角色移动和重力产生动态运动。

要向骨架中添加弹簧骨骼：

1. 在 **编辑模式**，从相应的父骨骼挤出新骨骼（例如， `Avatar_Head` 用于头发）。
2. 为每根骨骼命名，并在名称中包含 `springbone` （不区分大小写），例如： `SpringBone_hair_left`, `springbone_hair_left_01`, `Hair_SpringBone_left_tip` 只要它们包含 `springbone`.
3. 构建 **线性链** ，至少 **2 根骨骼** （一个根部和一个末端）。单独的一根弹簧骨骼不会产生任何运动——模拟需要一条链，其中根骨骼负责驱动物理效果，而末端骨骼定义终点。链中的每根骨骼最多只能有一个弹簧骨骼子级。
4. 像往常一样，将网格的皮肤权重绑定到弹簧骨骼上。
5. 导出为 `.glb` —— 弹簧骨骼将包含在骨架中。

物理参数（刚度、重力、阻力）在上传后的 Builder 中配置，而不是在 Blender 中配置。完整指南请见 [弹簧骨骼](/docs/creator/wearables-and-emotes/wearables/spring-bones/).

就是这样！有了这些信息，你已经可以制作一个基础骨架了！


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/3d-jian-mo-yu-dong-hua/create-a-rig.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
