> 使用生成区域以可视化方式设置生成位置和生成摄像机位置。

# 生成区域

创作者可以设置 **出生位置** 和 **出生相机目标** 在创作者中心中通过编辑一个 **出生区域** 实体。

这为创作者提供了一种可视化方式来设置出生区域以及头像出生时朝向的方向，就像场景中的其他对象一样。

出生区域定义了玩家直接进入你的场景时出现的位置，无论是通过输入坐标还是传送。你的场景中可能有一些物体会阻挡玩家移动，如果他们刚好出生在这些物体上方，比如树木或楼梯；或者你的场景可能有起伏较高的地形。如果玩家出生在某个无法移动的地方，体验会很差。因此，你可以选择在临时指定的位置设置一个或多个出生点。


> [!WARNING]
> **📔 注意**: 所有出生点都必须位于构成场景的地块内。你不能将玩家生成在这些地块空间之外。


<img src="/docs/_remote/04f3a7c93b45dca9a38de100.webp" alt="" width="200">

出生区域由两个协同工作的不同实体组成：

* **出生** 实体：可在……中找到 **玩家** 实体。定义玩家的实际出生点。

<img src="/docs/_remote/da6fb5ebe275854d0b596645.webp" alt="" width="200">

* **相机目标** 实体：可在……中找到 **出生区域** 实体。将其移动到你希望玩家出生时面向的方向。

<img src="/docs/_remote/13b486809fcb8431fd6ecb7c.webp" alt="" width="200">

## 更改出生区域

**出生区域** 及其 **相机目标** 可像场景中的其他对象一样直接在场景编辑器中移动，或者手动修改其值（就像其他实体中的 Transform 一样）。

### 参数

选中出生区域后，创作者可以在 Components 部分查看并设置以下值：

<img src="/docs/_remote/81d5fae5f7131c97a7c0709a.webp" alt="" width="600">

* **位置**: 头像的出生位置。这些是场景中的坐标，相对于场景原点，类似于你在 Transform 组件中会使用的坐标。
* **出生相机目标**: 相机（以及头像）在出生进入场景后将朝向的方向。可利用这一点更好地控制玩家进入你的场景时的第一印象。
* **随机区域**: 以米为单位为出生区域添加随机性，可通过以下项启用或禁用： **不要随机化** 切换。启用后，场景编辑器中会显示一个区域，表示玩家可以随机出生的范围。随机偏移同时应用于 X 轴和 Z 轴，并可防止所有玩家出生时彼此重叠，这在拥挤的场景中看起来尤其糟糕。

<img src="/docs/_remote/87b06caf636a947bd765acb5.webp" alt="" width="600">

## 多个出生区域

<img src="/docs/_remote/80f2cf42b9eb7797e76dccd8.webp" alt="" width="600">

创作者可以拥有多个 **出生区域** 已定义。要创建新的出生区域，请按以下步骤操作：

1. 选择任意出生区域， **spawn1** 它是默认项。
2. 所有出生区域都会显示在右侧面板中。你可以通过点击以下项来添加新的出生区域： **+ 添加新的出生区域**&#x20;

   <img src="/docs/_remote/2d3066e023e44c9ba5f38990.webp" alt="图标" width="200">

   .

<img src="/docs/_remote/24547b03ea234f33b90a044e.webp" alt="" width="600">

3. 启用 **主出生点**&#x20;

   <img src="/docs/_remote/745070068b65895eedbc404a.webp" alt="图标" width="100">

   &#x20;中所需出生区域的选项。如果有多个出生区域被切换为 **主出生点**，玩家将随机出现在其中一个。
4. 预览场景。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-bian-ji-qi/gou-jian/spawn-areas.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
