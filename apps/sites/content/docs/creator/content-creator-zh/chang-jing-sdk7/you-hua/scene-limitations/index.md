> 我可以在场景中放多少东西？

# 场景限制

为了提升元宇宙中的性能，我们制定了一套每个场景都必须遵守的限制。这些限制是按地块计算的。因此，场景占用的地块越多，这些限制就越高。

在使用 [Creator Hub](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/editor-installation/)时，你可以查看场景中 3D 模型所使用资源的统计信息，以及你场景的限制。

![](/docs/_remote/ab87e37107598ee524831639.webp)

你可以展开此菜单查看详情。

![](/docs/_remote/8d4619b1778683c8d72d3c61.webp)


> [!NOTE]
> **💡 提示**：如需查看按地块数划分的所有具体数值参考表，请参见：
> 
> [参考表](https://docs.google.com/spreadsheets/d/1BTm0C20PqdQDAN7vOQ6FpnkVncPecJt-EwTSNHzrsmg/edit#gid=0)


## 场景限制规则

以下是场景允许同时渲染的元素最大数量：

> *n* 表示场景占用的地块数量。

* **三角形：** `n x 10000` 场景中所有模型的三角形总数。
* **实体：** `n x 200` 场景中的实体数量。
* **网格：** `n x 300` 场景中的网格数量。
* **材质：** `log2(n+1) x 20` 场景中的材质数量。包括作为模型一部分导入的材质。
* **纹理：** `log2(n+1) x 10` 场景中的纹理数量。包括作为模型一部分导入的纹理。
* **高度：** `330 米` 场景的最大高度，不论其占用多少地块。

  > 警告：放置在 200 米以上的实体可能会出现多人同步问题。请将玩家会交互的内容或需要在玩家之间同步的内容保持在该高度以下。

  > 重要：只有当前在场景中被渲染的实体才会计入这些限制。如果你的场景在不同 3D 模型之间切换，重要的是任一时刻被渲染的模型，而不是总和。玩家头像以及玩家从场景外带入的任何物品也不计入这些限制的计算。
* **总文件大小：** 在 Genesis City 中 -`每个地块 15 MB - 最大 300 MB`。对于 Worlds，请参见 [世界大小](#world-size)。上传到内容服务器的文件总大小。包括 3D 模型和音频。不包括未上传的文件，例如 node.js 包。在确认部署之前，你可以查看即将发布的完整文件列表及其大小。
* **文件数量：** `每个地块 200 个文件` 已上传文件的总数。包括 3D 模型和音频。不包括未上传的文件，例如 node.js 包。
* **单个文件最大大小** `每个文件 50 MB` 场景中的任何单个文件都不能超过 50 MB。较小的场景还会受到进一步限制，因为文件总大小不得超过其总文件大小限制（例如，单地块场景的总大小上限为 15 MB）。


> [!WARNING]
> **📔 注**：文件大小和文件数量限制会在部署时强制执行，超过限制会阻止部署。其他限制（三角形、实体、网格、材质、纹理、高度）属于软限制：Creator Hub 会将其报告为警告，但超出后通常只会导致性能下降，而不会直接失败。请将它们视为强烈建议。



> [!NOTE]
> **💡 提示**：并非场景项目文件夹中的所有文件都会计入文件大小限制，只有上传到服务器的文件才会计入。 *node\_modules* 文件夹中的所有内容都很大，这些都是不会被上传的依赖，因此不计入限制。 `/src` 文件夹中的任何文件也同样不计入，因为源代码不会被上传。
> 
> 你可以在项目中的 `.dclignore` 文件里列出任何你希望排除上传的其他文件或文件夹。请注意 `.dclignore` 也适用于本地预览，所以不要列出场景实际会加载的任何内容。参见 [dclignore 文件](/docs/creator/content-creator-zh/chang-jing-sdk7/xiang-mu-lei-xing/scene-files/#the-dclignore-file).


## 优化

参见 [性能优化](/docs/creator/content-creator-zh/chang-jing-sdk7/you-hua/performance-optimization/) ，了解如何让你的场景保持在这些限制之下，并让玩家获得更流畅的运行体验。


> [!NOTE]
> **📱 移动端**：本页上的硬性限制适用于所有客户端，但移动设备的余量比桌面端更少。针对 [移动应用](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/yi-dong-ke-hu-duan/overview/)开发时，请尽量让内容明显低于这些限制，并优先减少三角形、材质和纹理尺寸。


## 场景边界

在运行预览时，任何位于地块边界之外的内容在渲染时都会以红色高亮。如果有任何内容超出这些边界，那么玩家访问你的场景时，该部分内容将不会被渲染。

如果大型物体的一部分超出边界，那么这部分会被裁切掉。

单地块场景的尺寸为 16 米 x 16 米。如果场景包含多个地块，尺寸会根据地块的排列方式而变化。

可以将实体放置在地下，以隐藏它们，或者只让其中一部分露出。场景不能有低于默认地面高度的隧道，玩家也不能移动到 `y = 0` 高度以下。

## 着色器限制

Decentraland 中使用的 3D 模型必须使用受支持的着色器和材质。请参见 [3D 模型材质](/docs/creator/content-creator-zh/3d-jian-mo-yu-dong-hua/materials/) 查看受支持着色器列表。

## 网格压缩


> [!WARNING]
> **📱 在** [**移动客户端**](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/yi-dong-ke-hu-duan/overview/) **上使用 Draco 压缩的网格需要进行资源优化。** 使用 [Draco](https://google.github.io/draco/) 网格压缩的 glTF/GLB 模型，只有在经过资源优化流程处理后，才会在 Decentraland 移动应用中加载。如果你的 Draco 压缩模型在移动端无法加载，请确认它们已被优化——或者导出模型时 **不要使用** Draco 压缩，以完全跳过这一步。其他形式的 glTF 优化（量化、纹理压缩、网格简化）不受影响。


## 照明

场景的照明条件不能从默认设置改为所有玩家统一生效，不过每个玩家都可以在 Explorer UI 中自由更改自己的天空盒设置。

## 纹理尺寸限制

纹理尺寸必须使用与以下数值相匹配的宽度和高度（像素）：

```
1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024
```

> 这个序列由 2 的幂组成： `f(x) = 2 ^ x` 。1024 是我们允许的纹理尺寸最大值，超过该尺寸的纹理会在资源包转换过程中被缩小。这是其他许多渲染引擎常见的要求，这是由于图形处理器的内部优化所致。

宽度和高度不需要相同，但都必须属于这个序列。

**纹理推荐尺寸为 1024x1024**，我们发现这在国内网络传输以及提供合理的加载/质量体验方面是最佳尺寸。

其他有效尺寸示例：

```
32x32
64x32
512x256
512x512
1024x1024
```


> [!WARNING]
> **📔 注**：虽然任意尺寸的纹理有时也能工作，但它们经常会出现渲染错误，并且更不稳定。我们强烈建议你的所有纹理都符合这些尺寸。


## 世界大小

Decentraland [Worlds](/docs/creator/content-creator-zh/chang-jing-sdk7/fa-bu/publishing-options/#decentraland-worlds) 有不同的限制，因为它们作为单独场景加载。

* 发布到 Decentraland NAME 的 Worlds 会从一个存储预算中分配空间，该预算由同一钱包拥有的所有 Worlds 共享。你拥有的每个 NAME 可获得 `100 MB`，每个 LAND 地块可额外获得 `100 MB`，而每持有 2,000 MANA 又可再获得 `100 MB`。你可以按自己的方式在各个 Worlds 之间分配这部分预算。
* 发布到 ENS 域名的 Worlds 的上限为 `36 MB` ，且无法扩展。

你可以在 Creator Hub 的 **管理** 部分，或在 **Worlds** 选项卡中的 [Builder](https://decentraland.org/builder/worlds)里查看已使用和剩余的预算。如果你的预算超出上限（例如出售或转移资产后），你有 48 小时来释放空间，否则你的 Worlds 将无法访问。

参见 [世界大小限制](/docs/creator/content-creator-zh/chang-jing-sdk7/xiang-mu-lei-xing/kinds-of-project/#size-limits) 了解更多详情。

上传到内容服务器的文件总大小。包括 3D 模型和音频。不包括未上传的文件，例如 node.js 包。在确认部署之前，你可以查看即将发布的完整文件列表及其大小。

Worlds 中的其他所有限制都是按地块计算的，包括三角形、材质等。由于为世界添加更多地块是免费的，因此你最多可以向场景添加 300x300 个地块（总计 90,000 个），并获得与该地块数相对应的限制。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/you-hua/scene-limitations.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
