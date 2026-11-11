> 学习如何创建可嵌入到导入 Decentraland 的 3D 模型中的动画。

# 动画

动画是赋予事物生命的艺术。而让你的场景更生动的最好方式，就是给你的 3D 模型添加一些动画。


> [!NOTE]
> **💡 提示**：安装 [Decentraland Tools Blender 插件](https://extensions.blender.org/add-ons/decentraland-tools/)。它包含多个实用功能，可帮助你编辑和导出 3D 模型、穿戴物和表情动作。


有几种方法可以实现：通过 **对象动画**，通过一个 **骨架（骨骼动画）**，或者使用 **形态键（变形目标动画）**.

**对象动画** 最适合简单模型，例如弹跳的球、旋转的地球仪或漂浮的椅子，而且它不需要骨架。重要的是要提到，对象动画不同于 **顶点动画**。在对象动画中，模型作为整体进行动画处理，而在顶点动画中，对象的每个顶点都可以分别进行动画处理。

**形态键（也称为形变目标或混合形状）** 是一种顶点动画形式，并且 **受 Decentraland 引擎支持**。它们尤其适用于面部表情、口型同步，或任何仅靠骨骼难以实现的变形。嵌入 glTF/GLB 文件中的形态键动画会像骨骼动画或对象动画一样在世界中播放。

如果你的模型更复杂，例如人物、生物或机械，那么你就需要一个 **骨架**。骨架不过是一个数字骨骼系统，它会移动并变形网格。将网格绑定到骨骼的过程称为蒙皮，在这个过程中，你定义每个骨骼会影响哪些顶点组，以及这种影响的强度是多少，从而确保模型以尽可能好的方式变形。

不过，无论采用哪种方法，3D 模型的所有动画都必须嵌入到它的 *glTF* 文件中，因为你不能引用位于单独文件中的动画。

## 创建动画

下面介绍如何使用 Blender 创建动画。

### 对象动画

* 确保你已打开 ***非线性动作编辑器*** > ***动作编辑器*** 选项卡，并点击 ***新建*** 以添加一个动画片段。
* 将其重命名为你喜欢的名称，并确保切换 ***Fake User*** （盾牌图标），这样你的动画就会被保存。
* 选择对象并按下 ***I***。然后移动对象、旋转或缩放它，并在时间轴中的另一帧按下 ***I*** 再次按下，以使用当前的变换设置另一个关键帧。
* 动画的最后一帧应与第一帧相同，所以只需复制第一帧并将其粘贴到最后一帧。

![](/docs/_remote/da0a0df92981981477fc8dbd.webp)

*创建动画片段并添加关键帧。*

### 骨骼动画

要制作骨骼动画，你需要一个骨架。如果你想自己制作，请参见 [创建骨架](https://www.notion.so/686e6f59a1604585b059f990a36b2d55) ，了解如何设置统一的骨架。

当绑定完成且蒙皮变形效果良好时，你就可以开始动画了！这个过程和对象动画非常相似，但不是在 ***对象模式***&#x4E2D;进行，而是会在 ***姿态模式***.

* 前往 ***姿态模式*** 并确保你已打开 ***非线性动作编辑器*** > ***动作编辑器*** 选项卡，并点击 ***新建*** 以添加一个动画片段。
* 将其重命名为你喜欢的名称，并确保切换 ***Fake User*** （盾牌图标），这样你的动画就会被保存。
* 将鼠标放在 ***视口***&#x4E0A;，按 ***一个*** 以全选，然后按 ***I*** 为整个骨架设置关键帧。
* 移动到时间轴中的另一帧，并按你想要的方式调整骨骼以获得所需的姿势。根据你设置骨架的方式，你可以更改骨骼的位置、旋转和缩放。
* 当结果满意时，再为整个骨架设置另一个关键帧。如果动画需要循环，请记得让第一帧和最后一帧保持相同。

![](/docs/_remote/aa4ecd75ce5fa54f9c60066e.webp)

如果你刚接触动画流程，可以看看这个为初学者准备的、包含一些实用技巧的教程：

[![Video Preview](/docs/_remote/629c65271f6a14386ffd42f7.webp)](https://youtu.be/-iWslh4uQIk)

## 蒙皮动画与变换动画的比较

**在性能方面，变换动画通常比蒙皮网格动画更好，因为它在显示动画所需的计算资源开销更少。**

蒙皮网格动画是指使用带有骨骼骨架的网格，并对其顶点进行权重分配。骨架本身会被动画驱动，从而带动网格动画。这个过程计算量可能相当大，尤其是在需要动画的顶点数量很多时。

另一方面，变换动画是直接对对象的变换属性（如位置、旋转和缩放）进行动画处理。与蒙皮网格动画相比，这可以用更少的计算资源来完成，因为通常需要动画的变换属性比网格中的顶点更少。

此外，变换动画通常可以预先计算并以更紧凑的格式存储，例如关键帧数据，这样可以快速访问和播放，而无需在实时中进行大量计算。

总体而言，虽然蒙皮网格动画可以产生更细致、更逼真的动画，但在对性能要求较高的应用中，例如电子游戏，通常更偏好变换动画。

## 创建并导出多个动画片段

你可以为模型添加任意多个动画片段。你可以在 [这里的这一节](https://www.notion.so/5e962e5d54a24bcb9b906748007eb4cc).

在下面的视频中，你将学习如何在一个 GLB 文件中从 Blender 导出多个动画。

[![Video Preview](/docs/_remote/eb23ea78b0ffa9a0778e60d6.webp)](https://www.youtube.com/watch?v=YxAB4bujO_w&ab_channel=Decentraland)

## 将多个动画导出为一个动画

与骨骼动画不同，在骨骼动画中你可以绑定多个对象并将它们的动画合并到一个动画片段里，而对象动画则不能这样做。动画会分别导出，并且一次只能播放一个对象。解决方法是把多个动画合并为一个，这样你就会得到一个包含所有对象同时运动的动画片段。

为此，请将其下推到 ***非线性动画轨道*** ，如下所示，把每个对象的动画都下推到轨道中。

![](/docs/_remote/03413441d7a33b6e5298b18f.webp)

*将每个对象的动画下推到 NLA 轨道。*

当所有动画都列在 NLA 编辑器中后，选中所有对象，前往 **文件 > 导出 > glTF2.0**。展开 Include 并勾选 Selected Objects。展开 Animation，再次展开 Animation，并取消勾选 **按 NLA 轨道分组**。你可以在 Merged Animation Name 中重命名动画片段，然后只需按下 **导出 glTF 2.0**.

![](/docs/_remote/9ba538ff6074491ba9311c8c.webp)

*即可按设置将多个动画导出为一个。*

请记住，这只在每个对象各自只有一个动画时有效。如果对象有多个动画片段，最好分别导出。


> [!NOTE]
> **💡 提示**：你也可以不自己创建动画，而是下载通用动画并将其应用到你的模型上。例如，对于具有类人特征的 3D 角色，你可以从 [Mixamo](https://www.mixamo.com/#/).


## 采样

有时，由于动画数量过多或动画+模型的组合，动画文件最终可能会变得过大。

采样是优化动画的好方法。采样率决定关键帧在动画中被烘焙的频率。例如，如果采样率设为 2，就表示每两帧创建一个关键帧。采样率为 3 时，则每三帧烘焙一个关键帧，以此类推。采样率越高，文件越轻。

不过，缺点是动画会开始变得越来越不流畅，因为它会丢失一些重要关键帧（这些关键帧在动画中分布得并不均匀）。还要注意的是 **采样并不是将动画帧数除以采样率**.

通常， **2 或 3 的采样率** 就足够了。这些数值可以在不影响质量的情况下优化动画。

你可以在 ***采样率*** 在导出设置中的 ***动画***.

![](/docs/_remote/942221d28977584ca4c825da.webp)


> [!NOTE]
> **💡 提示**：如果动画的帧数能被采样率整除，那就很好！这意味着最后一帧会被烘焙，从而保留动画从结束到开始的过渡。


## 实现动画

本文档介绍如何将动画添加到 3D 模型中。请参见 [**处理动画**](/docs/creator/sdk7/3d-essentials/3d-model-animations/) ，了解如何在场景中激活动画并进行处理。

## 动画最佳实践

* 尽可能保持骨架简单，只为你打算添加动画的模型部分创建骨骼。骨骼会影响性能，因此请务必只添加真正必要的内容。
* 如果动画会在你的场景中循环播放，请确保最终姿势与起始姿势完全一致，以获得更好的过渡效果。
* 不要让角色处于没有动画的状态，即使他们实际上什么也没做。为角色静止时创建一个“待机”动画。待机动画可以包含一些细微动作，比如呼吸，也许还有偶尔的目光移动。
* 不要让手、手指、头或颈部之类的骨骼处于未摆姿势的状态。细节在优秀动画中非常重要，僵硬的动作只会让它更不真实。
* 除非极其必要，否则不要使用过多关键帧。你添加的关键帧越多，插值出错的可能性就越大，动画也会越重。
* 选择所有骨骼（包括变形骨骼），并在动画的第一帧和最后一帧设置关键帧。这样可以避免某些骨骼没有信息，导致一个动画无意中影响另一个动画。
* 导出时，确保文件中只有一个骨架。导入动画时，也会连同骨架一起导入。所有动画必须由同一个基础骨架执行，所以请删除任何不需要的内容。
* 始终重命名你的骨骼和动画。保持一切井然有序！
* 导出 *glTF* 模型时，确认你正在导出所有对象和动画。有些导出器默认只导出 ***当前选中的*** 内容。
* 导出模型后，请在 [Babylon Sandbox](https://sandbox.babylonjs.com/) 中检查所有动画是否正常工作，以及名称是否符合预期。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/3d-jian-mo-yu-dong-hua/animations.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
