> 关于头像骨架的基础知识。

# 头像骨架

绑定系统是一个虚拟骨架，允许模型移动。它由一组层级结构中的单独骨骼组成，很像现实中的骨架，并且在父/子关系下工作。本文将介绍一些基础绑定概念，例如骨骼位置、骨骼朝向、变形骨骼和非变形骨骼、IK 与 FK 之间的区别及其用途。头像绑定的结构、自定义属性以及动画设置，可在 [绑定功能](/docs/creator/content-creator-zh/chuan-dai-wu-yu-biao-qing-dong-zuo/biao-qing-dong-zuo/rig-features/).


> [!NOTE]
> **💡 提示**：安装 [Decentraland Tools Blender 插件](https://extensions.blender.org/add-ons/decentraland-tools/)。它包含多个实用功能，可帮助你编辑和导出 3D 模型、穿戴物和表情动作。


## **基础**

#### 骨骼位置或枢轴点

尽管绑定系统并不是人体骨架的精确复制，但在放置数字骨骼时，遵循现实中骨骼的位置是一种良好做法。这样做是为了保证可信且流畅的变形。骨骼位置很重要，因为它决定了枢轴点（运动从何处开始）。骨骼放置不当会导致网格变形不良。下图展示了骨骼位置的差异。

![head\_pivot\_rig\_1.0.gif](/docs/_remote/19b528a3a24a1ffcdeaf8d5f.webp)

![head\_pivot\_rig\_2.0.gif](/docs/_remote/95fc62ca479c3f614187167d.webp)

#### 骨骼朝向

骨骼的朝向将决定它们沿哪个方向旋转，例如正 X、负 Z 等。只要整个绑定系统保持一致，它们可以以许多不同方式设置。例如，如果选择 X 轴用于脊柱向前弯曲的动作，那么腿部也使用同样的设置就很合理。朝向也很重要，因为它会影响镜像行为。这意味着通常右侧的朝向是左侧朝向的镜像版本，反之亦然。

![](/docs/_remote/4d39fc26ef0dd267cbd54630.webp)

*打开坐标轴以显示骨骼朝向。*

![](/docs/_remote/8278804deb8abeeac7cba8ee.webp)

*坐标轴表示骨骼旋转的方向。*

#### 变形骨骼和非变形骨骼

变形骨骼是会让网格发生变形的骨骼，它们负责决定模型如何移动。这是基础骨架，并且它 **绝对不应被编辑**。以任何方式修改它都可能破坏绑定系统，并且导出时动画将无法工作。因此，这个基础绑定被移动到了它自己的图层（最底部的最后一层）。这些骨骼就是与网格一起导出的骨架。

非变形骨骼是不会让网格变形的骨骼，但它们在一个稳健的绑定系统中仍然是必要的，并用于 [IK、FK](#whats-fk-and-ik-in-a-rig) 以及其他自定义属性的设置。非变形骨骼的例子包括控制器、IK 和 FK 骨骼、脚部设置骨骼。这些不应被导出，它们仅用于动画目的。

![](/docs/_remote/4587f7ead5c1973d96247db4.webp)

*变形骨骼。*

![](/docs/_remote/8c4d5b63d6775310aef4709b.webp)

*非变形骨骼。*


> [!WARNING]
> ⚠️ **注意！** **不要对基础骨架进行任何编辑！**


![](/docs/_remote/7856fc2ef29a54224be410db.webp)

*基础骨架。*

#### 控制器

按照良好实践，绑定系统不应通过操作变形骨骼来制作动画，因为这可能会导致绑定系统损坏。相反， **动画应通过操作控制器来完成。**

控制器本质上就是非变形骨骼，这意味着它们不会影响网格，使其可以完全安全地被操作和制作动画。它们的作用是通过约束和驱动器控制基础骨架，而不是直接接触它。它们通常具有不同的形状和颜色，作为其功能和用途的视觉提示，使动画师更容易区分骨骼。其中一些还会拥有除位置、旋转和缩放之外的更多变换，因为可以为它们添加自定义属性，例如 IK/FK 切换。

另外需要注意的是，不能在与其最初创建时不同的软件中使用这套控制器设置。每个软件都有自己的逻辑，而且约束无法导出。

![](/docs/_remote/66b97ad8bec99d92c8bba7a2.webp) \_控制器及其不同的形状和颜色。\_


> [!WARNING]
> ⚠️ **警告**：骨骼绑定必须在创建它的同一软件中进行动画制作。例如，不能在 Maya 中使用 Blender 的设置，反之亦然。


#### 骨骼绑定中的 FK 和 IK 是什么？

一个绑定系统可以使用两种不同的设置来影响其运动方式：FK 和 IK。

#### FK - 前向动力学

在前向动力学（FK）中，层级中的父级会带动其下方所有子骨骼移动。以手臂为例：当肩膀旋转时，手臂的其余部分也会随之旋转；当上臂旋转时，前臂也会跟随其动作。在 FK 中制作动画时，每根骨骼都必须单独旋转。这种设置能对动作提供很强的控制，非常适合弧线运动，而弧线运动对流畅动画至关重要。

*层级中的运动方向。*

![](/docs/_remote/c8dc3dc16dcc58581a238041.webp)

*在 FK 中，每根骨骼都必须单独旋转。*

#### IK - 反向动力学

在反向动力学（IK）中，层级中的子级可以影响其父级的运动。在这种情况下，仍以手臂为例，当手在周围移动时，手臂的其余部分会跟随该动作。这也意味着，无论肩膀如何移动，手都会保持其位置不变。在这种设置中，极向量/极目标将控制骨骼弯曲的方向。腿部通常使用 IK，这对于动画时让脚保持在地面上方至关重要。

![](/docs/_remote/2137414a0b2253ea260c8f89.webp)

*层级中的运动方向。*

![](/docs/_remote/9de7bae6abe4d41fe6467d0e.webp)

*在 IK 中，手部会带动整条手臂移动，同时还能保持其位置。极目标控制肘部弯曲的方向。*


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chuan-dai-wu-yu-biao-qing-dong-zuo/biao-qing-dong-zuo/avatar-rig.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
