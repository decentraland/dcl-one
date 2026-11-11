> 了解导入到 Decentraland 的 3D 模型支持哪些网格属性。

# 网格

3D 模型有一个 *网格* 由三角形 *面组成*。这些面彼此相交于 *边* （它们接触的线）和 *顶点* （它们的角相接的点）。


> [!NOTE]
> **💡 提示**：安装 [Decentraland Tools Blender 插件](https://extensions.blender.org/add-ons/decentraland-tools/)。它包含一些实用功能，可帮助你编辑和导出 3D 模型、可穿戴物品和表情。


## 空间限制

你场景中的所有 3D 模型都必须位于其地块范围内。如果你的模型有任何部分在预览运行时超出这些范围，这些超出的部分将被裁掉，不会被渲染。

![](/docs/_remote/9ba86deac82448c396d45bba.webp)

在 Creator Hub 编辑器中，你会看到模型被染成红色，并且其边界框以红色标记。

![](/docs/_remote/ac6b1ca6eff1a48fc7dfe75d.webp)


> [!NOTE]
> **📔 注意**：在较旧的 Decentraland Web 客户端中，出于性能原因，如果 3D 模型的任何部分越界，即使只是边界框越界，整个模型也不会被渲染。自 Decentraland 2.0 桌面客户端起，只有模型中越界的部分会被裁掉。模型中位于场景内的部分会被渲染。


## 包围盒

如果你的模型的所有顶点都整齐地位于场景区域内，但它有很大的边界框，大部分为空并延伸到场景限制之外，那么在 Creator Hub 中整个模型都会被标记为超出场景限制。模型中位于你地块内的部分会被正确渲染，而且你仍然可以发布你的场景。

每个网格都有一个包围其形状范围的边界框。为了让 3D 模型在 Decentraland 中更实用，请确保其边界框不要比必要的范围延伸得更多。

为避免这个问题，你可以整理你的 3D 模型，重置网格的位置和旋转，使边界框不会超出其所包围的网格。

![](/docs/_remote/7c3c5e5108be6fd7e142dbf6.webp)

*在 Blender 中，你可以通过选择对象，按 Ctrl+A，然后应用“所有变换”来做到这一点。*

例如，在模型边缘附近旋转一个子网格时要小心。由于边界框是立方体，即使网格是圆形的，在旋转 45° 之后，它的边界框角落也可能会伸出去。

![](/docs/_remote/15f51a636f43103faeefa105.webp)

我们建议你烘焙模型中每个网格的旋转和缩放，以确保没有多余的边界框超出它们应有的大小。

## 三角形限制

由于 Decentraland 是一个开放世界，充满了不同的场景和对象，并且这些内容会在运行时按需下载（处于同一个 3D 空间中），因此优化我们的网格非常重要，以便在游玩时获得良好的性能。从这个意义上说，关于网格，我们总是需要牢记一些场景限制：

> n 代表一个场景占用的地块数量。

* **三角形：** n x 10000 场景中所有模型的三角形总数。
* **高度：** 330 米，不论地块数量多少。
* **文件大小：** 每个地块 15 MB - 最多 300 MB 上传到内容服务器的文件总大小。包括 3D 模型和音频。不包括未上传的文件，例如 node.js 包。
* **单个文件最大 50 MB** 无论场景有多少地块，场景中的任何类型单个文件都不能超过 50 MB。

更多信息请查看所有场景限制 [这里。](/docs/creator/sdk7/optimizing/scene-limitations/)


> [!WARNING]
> **🔥优化提示🔥：添加多边形数量** 一个很有价值的提示是始终跟踪模型的面数。要在 Blender 中做到这一点，你需要在视口覆盖层面板中打开统计信息。
> 
> <img src="/docs/_remote/11ef35913355fa3c25f5a19a.webp" alt="" data-size="original">


## 大型场景中的网格

创建网格时，我们应该牢记以下 2 个最佳实践：

* **模块化：** 将大型网格拆分为更小的模块化部分。例如，不要为整栋建筑只使用一个网格，而是可以将建筑拆分为分别对应每面墙、每层楼和各个区域的独立网格。这样可以让 Unity 的视锥裁剪更有效地工作，因为只有可见部分需要被考虑渲染。这也会改善内存占用，因为一个网格可以在不同位置被多次复用来表示同一个对象（要实现这一点，你需要在 SDK7 场景定义中引用同一个网格，或者在从 Blender 导出场景时使用实例而不是重复对象）。
* **包围体积**：确保每个对象的包围体积尽可能贴合。包围体积中的多余空间会导致对象在不可见时仍然被渲染。

![](/docs/_remote/85cd1390893917a4f9f84647.webp)

### 什么是视锥裁剪？

视锥裁剪是一种优化技术，它会禁用位于相机可视区域之外的对象渲染器（网格）。请看以下 2 个示例：

![](/docs/_remote/aa272a10f2f7825fafd9b7e8.webp)

*一个迷宫般的室内关卡。这个普通的场景视图显示了所有可见的游戏对象。*

![](/docs/_remote/2581f915e835c33d658e7101.webp)

*常规的视锥裁剪只渲染相机视野内的对象。这是自动进行的，并且始终会发生。*

## 实例化对象与复制对象

在 Blender 中，复制一个对象会创建该对象的一个完全独立的副本，而实例化一个对象则会创建一个与原始对象共享相同数据的副本。

当一个对象被复制时，它会创建一个具有完全独立数据集的新对象，包括其所有几何体、材质和动画。这意味着对原始对象所做的任何更改都不会反映到副本对象上，反之亦然。

另一方面，当一个对象被实例化时，对原始对象所做的任何更改都会反映到它的所有实例上，反之亦然。这是因为所有实例都共享相同的底层数据。

从性能角度看，实例化通常比复制高效得多，尤其是在处理复杂场景或大量对象时更是如此。这是因为实例化比复制更省内存，因为它不会为每个实例创建新数据。此外，实例化还可以让 Blender 通过将所有实例视为单个对象来优化渲染过程，而不是分别渲染每个副本。

总的来说，实例化可以是提升 Blender 以及你的 Decentraland 场景性能和工作流效率的强大工具，尤其是在处理大型、复杂场景时。

![](/docs/_remote/403e1a531d34d0b12ef576b8.webp)

在某些情况下，当复制树木、植物和路灯等对象时，实例化相比复制可以显著提升性能。例如，这个场景有十四个路灯，它们完全相同。我们知道每个路灯有两种不同的纹理，一种用于灯杆，另一种用于灯光。因此在这种情况下，我们会有两次绘制调用，每种纹理一次。

![](/docs/_remote/b4811452757e72b24eb01716.webp)

在 Blender 中，当你处于克隆并在空间中放置元素的阶段时，你有三种选择：复制、实例化或合并对象。

让我们分析一下每种方法的优缺点。

### 复制对象

| 菜单：  | 对象 ‣ 复制对象 |
| ---- | --------- |
| 快捷键： | Shift-D   |

这会创建一个与所选对象在视觉上完全相同的副本，但它们会被当作不同的对象处理。因此，在导出时，每个对象会有 2 次绘制调用，2 \* 14 = 28 次绘制调用，并且磁盘总占用为 320kb。

![](/docs/_remote/4a2e11a5d69b78b82bfaeffb.webp)

### 实例化对象

| 面板：  | 工具栏 ‣ 工具 ‣ 编辑 ‣ 关联复制 |
| ---- | -------------------- |
| 菜单：  | 对象 ‣ 关联复制            |
| 快捷键： | Alt-D                |

这会创建一个新对象，其所有数据都链接到原始对象。如果你在编辑模式下修改其中一个关联对象，所有关联副本都会被修改。变换属性仍然是副本，而不是链接，因此你仍然可以自由地旋转、缩放和移动，而不会影响其他副本。

![](/docs/_remote/2366fb1c01bf3e7b09b2e7f4.webp)

*当我们导出这些模型时，绘制调用次数仍然相同（28 次）。不过请注意，我们现在只有一个网格，这会显著减少磁盘占用（41 kb）。*

![](/docs/_remote/8af76e55acae51d4209ab721.webp)

*为了保持组织清晰，你可以直接在场景中实例化集合，这样你就能从一个文件夹和一个源头管理所有实例。*


> [!NOTE]
> **💡 提示**：同样的逻辑也适用于 SDK 端。十四个实体如果都指向它们的 `GltfContainer` 同一个 *.glb* 文件，行为就像实例一样：文件只会被下载一次、转换一次，而且其网格和纹理只会在内存中保留一份，不管有多少实体使用它。将相同内容拆分成十四个几乎相同的 *.glb* 文件，相当于复制，会让你付出十四次下载和十四份内存副本的代价。


### 合并对象

| 菜单：  | 对象 ‣ 合并 |
| ---- | ------- |
| 快捷键： | Ctrl-J  |

这会将所选对象合并为一个具有单一网格的对象。它在对象模式下工作，将所有内容合并到最后一个被选择的 *active* 对象中，并要求所有选中的对象类型相同。材质会被保留并合并，因此我们的十四个路灯不再是十四个可渲染对象，而会变成一个，引擎会用 2 次绘制调用而不是 28 次来绘制它们——每个材质一次，只需一次。

这三种选项中，只有这一种真正减少了绘制调用，而且有必要明确原因：Decentraland 引擎是在运行时从流式内容构建你的场景的，因此它无法像某些引擎那样，替你对预先烘焙好的内容进行分组或批处理重复对象。如果你需要更少的绘制调用，这个合并必须在 Blender 中完成。

这种减少并不是没有代价的。合并会同时放弃好几个优势：

* **磁盘占用又会增加。** 合并后的网格会存储全部十四个路灯的几何体，因此你失去了实例化带来的节省。
* **视锥裁剪变成了要么全部要么没有。** 只要合并对象的任何部分出现在屏幕上，它就会被完整绘制。街道上分布着十四个路灯时，几乎总会有一个路灯可见，因此最终你会一直绘制全部十四个——这很容易比你节省下来的绘制调用花费更多。
* **这些部分不再能被单独操作。** 你将无法再移动、播放动画、点击或隐藏单个路灯，因为只剩下一个对象了。

谨慎使用合并：仅用于很多小道具、它们总是在同一个位置一起被看到、不会被交互，并且只有在你确认对象数量确实在拖慢性能之后。对其他所有情况，实例化才是更好的默认选择。


> [!WARNING]
> **📔 注意**：将十四个独立对象导出到一个单独的 *.glb* 文件中就是 **不** 合并。引擎仍然会看到十四个对象，并且仍然会发出 28 次绘制调用。只有合并网格才会改变这个数量。


## 网格命名

为你的网格使用有意义的名称。名称应该能提供资产在何处使用或与对象哪一部分相关的上下文。

为资产命名时使用以下组合 `PascalCase` 和 `snake_case`，我们可以称之为 `PascalSnake_case`。基本规则是——新单词或分隔符（`_`）后面的单词以 **大写字母开头。**

* 为你的网格使用有意义的名称 **示例：** 🟢 **优先** 名称 - `剧院`, `有轨电车` 🔴 **避免** 名称 - `未命名.008`, `primitive(1)`, `Cylinder.091`, `_sphere-AB`
* 使用下划线 `_` 作为分隔符，为名称增加更多上下文。把更通用的内容放在前面，把具体信息放在末尾。 **示例：** 🟢 **优先** 名称 - `喷泉雕像`, `剧院主墙`, `电车左轮` 🔴 **避免** 名称 - `北_主墙_剧院`, `电车_左前_轮`

如果有多个同名对象，请在词后添加数字： `FountainStatue01、FountainStatue02 等。`

## 创建模型的工具

有许多插件和外部工具可以在创建资源时帮助你，让流程更快更高效，其中有些是免费的，有些需要购买，举例如下：

### 简化

这是一个广为人知的修改器，可用于在保持模型表面结构的同时减少网格的三角面数量。虽然这是一个优化模型的强大工具，但要注意，一旦模型被简化，其拓扑可能会受到影响，从而导致不良拓扑（因为它会把几何体打散成不可预测的碎片）。另一方面，简化会将网格中的四边形转换为三角形，这会让后续修改变得困难。基于这一点， **简化适合静态模型，但不适合动画模型。**

![](/docs/_remote/5c1b597b93cffd4692e756c8.webp) ![](/docs/_remote/ffc83dd73f6105e78f5d4ed1.webp)

使用简化优化网格的另一种方法是先对模型进行简化，然后再进行适当的重拓扑。这样可以确保结果更可预测，并得到干净的拓扑。

一些可能有用的重拓扑插件包括（其中一些是付费的）：

* **Simplygon：** <https://www.simplygon.com/>
* **Speedretopo：** <https://blendermarket.com/products/speedretopo?ref=2>
* **Retopoflow：** <https://blendermarket.com/products/retopoflow>

### 镜像修改器

Blender 中的镜像修改器是一种通过将对象的一半镜像到任意方向（X、Y、Z）来创建对称模型的工具。当你有对称模型时，它是一个很棒的工具，因为它可以减少 3D 建模和 UV 展开所需的时间（因为你只需要展开模型的一部分，其余部分会共享相同的 UV 坐标），并且还能通过避免在本质上是镜像的部分上浪费纹理分辨率来优化纹理。

![](/docs/_remote/d2acc97f8cfac981e2ef4ed3.webp)

### 批量重命名

一种非常方便、以简单方式更改命名约定的工具，是使用 Blender 工具集中集成的批量重命名。要访问它，请前往 *编辑* 然后 *批量重命名*.

![](/docs/_remote/b586dae5ffc349455527b2fc.webp)

选择你想要更改名称的对象，然后直接把默认对象名替换为新名称。这个工具为不同方法和不同类型的资产提供重命名支持，因此它也非常灵活，可用于更改网格、动画、骨骼等的名称。

![](/docs/_remote/ff8b93deb811edcecb68aeba.webp) ![](/docs/_remote/7934176b00dd96ce2f519b23.webp)

## 几何体最佳实践 [#](/docs/creator/3d-modeling/meshes/#best-practices-for-geometries)

* 请注意你为 3D 模型添加了多少面，因为面越多，渲染要求就越高。请参见 [**场景限制**](/docs/creator/sdk7/optimizing/scene-limitations/) 以了解场景施加的限制。
* 确保没有看不见但会增加三角面数量的隐藏面。
* 对于应该具有圆润边侧的形状，将其设置为 *平滑* 而不是添加额外的面。
* 确保所有面的 *法线* 都朝外而不是朝内。如果你渲染模型时发现有些面似乎不见了，这很可能就是原因。
* 烘焙网格的旋转和缩放，使其边界框不会不必要地向外延伸。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/3d-jian-mo-yu-dong-hua/meshes.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
