# 碰撞体

了解如何为导入到 Decentraland 的 3D 模型添加碰撞体。

要让 3D 模型与场景中的用户发生碰撞，你必须为模型添加碰撞体。如果没有碰撞体，玩家就可以像那里什么都没有一样穿过模型。要让对象响应玩家的指针事件、阻挡指针事件，或被射线检测命中或阻挡射线检测，也需要碰撞体。出于性能原因，碰撞体通常会单独建模为更简单的不可见几何体，其顶点比可见几何体更少。


> [!NOTE]
> **💡 提示**: 安装 [Decentraland Tools Blender 插件](https://extensions.blender.org/add-ons/decentraland-tools/). 它包含多个实用功能，可帮助你编辑和导出 3D 模型、可穿戴物品和表情动作。


模型可以使用一个不可见几何体作为碰撞体，或者（自 SDK7 起）你也可以将可见几何体指定为碰撞体。参见 [**3D 模型上的碰撞体**](/docs/creator/sdk7/3d-essentials/colliders/#colliders-on-3d-models) . 你还可以为不同几何体分配不同的碰撞层，以分别控制玩家物理、指针事件和其他自定义层，参见 [**3D 模型上的碰撞体**](/docs/creator/sdk7/3d-essentials/colliders/#collision-layers) .

碰撞体不会影响模型与实体之间的交互，它们始终可以相互重叠。碰撞体只会影响模型与玩家头像的交互。

要让 Decentraland 场景将一个对象识别为碰撞体，只需要以特定方式命名它。对象名称末尾必须包含后缀“\_collider”。

例如，要为路灯柱创建一个碰撞体，你可以创建一个包围其柱体的简单盒子对象。场景中的玩家看不到这个盒子，但它会阻挡他们的路径。

![](/docs/_remote/872cedc3afd092432696701c.webp)

在这种情况下，我们可以将这个盒子命名为“LampPost*collider”，并将树和盒子一起导出为一个单一的 \_\_gltf 模型。\*\_\_collider*\* 标签会提醒 Decentraland 世界引擎该盒子对象属于碰撞体集合，从而使碰撞体网格不可见。

每当玩家在你的场景中查看路灯柱模型时，他们会看到灯的复杂模型。然而，当他们走进去时，碰到的会是盒子，而不是树。

## 玩家空间限制

当你设计供玩家行走的模型时，需要牢记以下参考尺寸：

* 玩家可以不跳跃地走上最高约 40 厘米的台阶。45 厘米或更高的台阶需要跳跃。对于走起来舒适的楼梯，每级高度应保持在 25 厘米或更低，深度至少 30 厘米。
* 玩家可以不跳跃地走上最高 46 度的斜坡。更陡的表面会让头像滑落。为了便于行走，坡道角度应保持在 30 度或更小。楼梯上方的坡道碰撞体是个例外：它会沿着楼梯的斜坡走，约为台阶高度 25 厘米、深度 30 厘米时的 40 度（参见 [为楼梯添加碰撞体](#add-a-collider-to-a-staircase)).
* 玩家可以跳到 1.8 米高
* 头像高度约为 1.9 米，但要考虑他们头上可能戴着帽子或其他可穿戴物品。
* 头像的碰撞体是一个宽 0.6 米的胶囊体（半径 0.3 米）。低于约 1.85 米的横梁或天花板会阻挡头像。玩家行走的任何地方都至少留出 2 米的净高。在室内，要为摄像机留出更多空间，参见 [把室内空间建得比现实更大](/docs/creator/content-creator-zh/chang-jing-sdk7/ti-yan-she-ji/design-games/#build-interiors-larger-than-real-life).

每当玩家在你的场景中查看树模型时，他们会看到树的复杂模型。然而，当他们走进你的树时，碰到的会是盒子，而不是树。

### 为楼梯添加碰撞体

楼梯是碰撞体对象非常常见的使用场景。为了让玩家能够爬楼梯，必须有一个对应的 \_collider 对象，玩家可以踩上去。

我们建议为楼梯碰撞体使用坡道对象，这样在上下楼梯时体验会好得多。当他们爬上楼梯时，会表现为平滑的上升或下降，而不是需要他们在每一级台阶上“跳”一下。

使用坡道对象也可以避免创建不必要的几何体，为其他更复杂的模型节省空间。请记住，在计算 [**场景限制**](/docs/creator/sdk7/optimizing/scene-limitations/)

1. 创建一个与原始楼梯大小和比例相似的坡道形状新对象。

![](/docs/_remote/8162fc04d84389328f79ddb3.webp)

2. 将坡道对象命名为类似 S*air\_collider*. 它必须以 \_*collider*.
3. 将坡道对象叠放到楼梯上，使它们占据同一空间。

![](/docs/_remote/fab0dd4c3439215495a72aa2.webp)

4. 将这两个对象一起导出为一个单一的 *glTF* 模型。

现在，当玩家在你的场景中查看楼梯时，他们会看到更精细的楼梯模型，但当他们攀爬时，碰到的会是坡道。

## 优化碰撞体

* 优先使用 **Box**, **Sphere** 和 **Capsule** 碰撞网格，因为在加载和运行时性能会更好。
* 可以考虑用多个原始碰撞网格来近似复杂碰撞网格（**Box**/**Sphere**/**Capsule**)

![](/docs/_remote/195d7d652ad646f33e815b49.webp)

*例如：环形物体的碰撞体可以由多个 Capsule 或 Box 碰撞体组成*

* 将碰撞网格放在其相关对象的层级中。例如，从层级结构的角度看，Spaceship\_collider 可以作为 Spaceship 父对象下的子对象。

### 凸形与凹形碰撞体

如果只能使用任意网格碰撞体，那么请将碰撞网格设为凸形（而不是凹形）。这可以避免奇怪的物理情况，例如卡顿、卡在碰撞体内，或穿透碰撞体。注意，通常可以用多个凸网格来模拟凹网格。

![](/docs/_remote/f40bda0817152df8a3cd5005.webp)

## 碰撞命名

**使用有意义的名称。名称应能体现该资源的使用场景，或它所对应的对象部分。**

在资源命名中，我们混合使用 `PascalCase` 和 `snake_case`，这可以称为 `PascalSnake_case`。基本规则如下——新单词或分隔符（`_`）后面的单词以 **大写字母开头。**

Collision Mesh 是在 Explorer 中创建碰撞体时用作参考的网格

* 它遵循与上面的 Meshes 相同的原则，但在末尾带有 `_collider` 后缀 **示例：** 🟢 **推荐** 的名称 - `Tram_Wheel_LeftFront_collider` 🔴 **避免** 的名称 - `Primitive.001_Collider`, `collider (1)`, `Cylinder.007_collider`,


> [!WARNING]
> **🔥优化提示🔥**
> 
> 如果某个碰撞体可以近似为 Box、Sphere 或 Capsule 形状，那么请额外使用这三种变体之一的后缀：\_box、\_sphere 或 \_capsule。使用这三种形状而不是任意网格进行碰撞检测，会提升场景性能和加载时间 💪🚀
> 
> 示例：🟢 Teater\_MainWall\_North\_collider\_box，Fountain\_Statue\_collider\_sphere


## 碰撞体最佳实践

* **创建碰撞体时，始终尽量使用最少的三角形。** 避免复制复杂对象来用作碰撞体。简单的碰撞体既能保证良好的用户体验，也能让你的场景保持在三角形限制之内。
* **碰撞体对象不应包含任何材质**，因为场景中的玩家永远看不到它。碰撞体对玩家是不可见的。
* **所有碰撞体对象名称都必须以 \_ 结尾*****collider*****.** 例如，T*ree\_collider*.
* **如果你将 \_plane**\_\*\* 用作碰撞体，它只会在一个方向上阻挡。\*\*如果你希望碰撞体从两侧都能阻挡，例如用于墙壁，就需要创建两个法线朝相反方向的平面。
* 复制碰撞体对象时，请注意它们的名称。有些程序会在文件名末尾追加一个 \_*1* 以避免重复，例如 *tree\_collider\_1*。像这样命名的对象会被 Decentraland World Engine 解释为普通对象，而不是碰撞体。
* 如果你添加一个不可见的 `MeshCollider` 组件到实体上，它会近似 3D 模型的形状。
* 如果你配置 `GLTFContainer` 组件使用可见几何体作为碰撞体。参见 [**3D 模型上的碰撞体**](/docs/creator/sdk7/3d-essentials/colliders/#colliders-on-3d-models) .
* 如果你正在创建一个包含许多网格形状、且每个都需要碰撞体的 3D 模型，有一些实用的插件可以提供帮助。对于 Blender，你可以使用 Blender 中的批量重命名选项，自动添加一个 `_collider` 后缀到组中所有网格名称上。关于此工具的更多信息请参见“**使用 Blender 重命名资源”部分**


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/3d-jian-mo-yu-dong-hua/colliders.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
