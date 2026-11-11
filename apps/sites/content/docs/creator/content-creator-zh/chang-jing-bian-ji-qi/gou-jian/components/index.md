> 了解一个物品的组件如何工作

# 实体和组件

在画布上或实体树中单击某个项目即可将其选中。随后，你会在屏幕右侧的属性面板中看到其组件。不同的项目具有不同的组件，每个组件都会显示特定设置。

![](/docs/_remote/8c1945559d13f38d8a485e7a.webp)

大多数非交互式项目具有以下组件：

* **变换**：设置项目的位置、旋转和缩放。如果该项目是 [实体树](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/scene-editor-essentials/#the-entity-tree)中另一个项目的子项，这些值将相对于父项的值。
* **GLTF**：要加载的 3D 模型。它包含此 3D 模型文件的本地路径，还包含一些用于配置 [碰撞体](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/colliders/#colliders-on-3d-models) 在模型上的属性。

场景中的项目都是 **实体**。场景中的一切都是实体，它们是场景的基本构建块。项目是至少具有位置和可见形状的实体。

## 添加组件

要向任何实体添加组件，请点击 **+** 属性选项卡顶部的加号，并从列表中选择组件。请参阅 [让任何项目变得智能](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/make-any-item-smart/)

![实体上打开的添加组件菜单，其中列出了按 3D 内容和交互分组的组件。](/docs/_remote/e0e52c0c060b63e97e2466dd.webp)

你可以点击实体中任何组件右侧的三点图标，然后选择 **删除组件**.

## 从头创建实体

要创建一个全新的实体，请右键单击根 **场景** 实体树中的实体，或任何其他实体，然后选择 **添加子项**

![](/docs/_remote/a6edc747060b504b764ee35b.webp)

这会创建一个仅包含 **变换** 组件的空实体。新实体是你所单击的父实体的子项。然后，你可以向其添加任何其他所需组件，将其塑造成你想要的任何形式。

## 可用组件

以下组件可通过场景编辑器 UI 添加到任何实体：

* **网格渲染器**：根据基本形状（立方体、平面、圆柱体或球体）为实体提供可见形状。
* **网格碰撞体**：为实体提供不可见的碰撞体几何形状。这可以阻止玩家穿过该项目，和/或使其可被点击。请参阅 [碰撞体](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/colliders/).
* **材质**：定义具有 **网格渲染器** 组件的实体的颜色、纹理和其他属性。请参阅 [材质](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/materials/).

  <div data-gb-custom-block data-tag="hint" data-style="warning" class="hint hint-warning"><p>**📔 注意**：该项目必须具有 **网格渲染器** 组件。它不会影响具有 **GLTF** 可见形状的项目。</p></div>
* **可见性**：定义实体是否不可见。
* **光源**：向实体添加光源。
* **替换材质**：替换具有 **GLTF** 组件的实体的材质。如果 3D 模型具有多个网格，你可以单独替换每个网格的材质。
* **音频源**：从实体所在位置播放声音文件中的声音。请参阅 [声音](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/sounds/).
* **文本形状**：在 3D 空间中显示文本。请参阅 [文本](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/text/).
* **指针事件**：将实体标记为可点击，并显示悬停提示。


> [!WARNING]
> **📔 注意**： **指针事件** 组件仅提供反馈。要在与实体交互时执行操作，请参阅 [让任何项目变得智能](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/make-any-item-smart/)


* **多人游戏**：共享实体发生的任何更改，使场景中的所有玩家也能看到。它可配置为仅共享特定组件上的更改。请参阅 [无服务器多人游戏](/docs/creator/content-creator-zh/chang-jing-sdk7/lian-wang/serverless-multiplayer/#mark-an-entity-as-synced) 了解更多详情。
* **动画控制器**：控制具有 **GLTF** 3D 模型的实体上的动画播放。请参阅 [3D 模型动画](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/3d-model-animations/).
* **补间**：使实体在一段时间内逐渐移动、旋转或缩放。请参阅 [移动实体](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/move-entities/).
* **公告牌**：使实体始终旋转以面向玩家。请参阅 [面向用户](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/entity-positioning/#face-the-player).
* **附加到虚拟形象**：将实体附加到玩家的虚拟形象，使其跟随玩家移动。请参阅 [将实体附加到虚拟形象](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/entity-positioning/#attach-an-entity-to-an-avatar).
* **视频播放器**：在场景中播放视频，可通过实体的 **材质**显示。请参阅 [视频播放](/docs/creator/content-creator-zh/chang-jing-sdk7/mei-ti/video-playing/).
* **NFT 形状**：在相框内显示 NFT 的图像。请参阅 [显示 NFT](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/gou-jian/nfts/).
* **粒子系统**：从实体位置发射粒子，用于烟雾、火焰或闪光等效果。请参阅 [粒子系统](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/particle-system/).
* **虚拟摄像机**：定义场景可切换玩家视角的自定义摄像机角度。请参阅 [摄像机](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/camera/).
* **虚拟形象修改区域**：更改位于以此实体为中心的区域内的玩家所见虚拟形象的行为或外观。实体的缩放决定区域大小。请参阅 [修改区域](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/modifier-areas/).
* **摄像机修改区域**：在以此实体为中心的区域内，强制玩家摄像机使用第一人称或第三人称视角。实体的缩放决定区域大小。请参阅 [修改区域](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/modifier-areas/).
* **脚本**：将自定义代码模块附加到实体，并可从 UI 配置参数。请参阅 [脚本组件](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-yong-dai-ma-kuo-zhan/script-component/).


> [!WARNING]
> **📔 注意**：SDK 中还存在其他目前只能通过代码使用的组件。你也可以创建自己的 [自定义组件](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/custom-components/) ，这些组件不会有 UI 表示，但可通过代码添加和编辑。
> 
> 请参阅 [结合代码使用](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-yong-dai-ma-kuo-zhan/overview/) 以了解如何编辑场景代码。
> 
> 另请注意，一个实体只能拥有每种组件的 **一个** 。无法为实体中已存在的组件分配第二个实例。例如，你不能向同一个实体添加两个 **动作** 组件。


## 值得了解的设置

大多数组件字段与上面链接的 SDK 页面一一对应。有些字段很容易被忽略，或其行为并不像字段名称所暗示的那样直观。

### 音频源

**音调** 会乘以播放速度，并同时改变音调。 `1` 播放原始声音， `2` 以两倍速度播放（升高一个八度）， `0.5` 以半速播放（降低一个八度）。它必须大于 0，默认值为 `1`.

### 视频播放器

* **播放速率**：与音调相同的概念，但用于视频。 `1` 为正常速度。留空以使用默认值。
* **起始位置（秒）**：视频开始时从何处开始播放。留空则从头开始。
* **空间音频**：开启 **空间化** 以使视频声音具有位置感，玩家走远时声音会逐渐减弱。 **空间距离** 随后设置范围：音频在距离内保持全音量，直至 **最小** 米，并在 **最大** 米处渐弱至静音。默认值为 0 和 60。

当 **空间化** 关闭时，音频会在场景中的任何位置以相同音量播放。

### 文本形状

* **大小**：文本框的宽度（**宽**）和高度（**高**），单位为米。对齐和内边距均相对此框进行测量。
* **文本换行**：当文本达到以下设置的宽度时，将文本换为新行 **大小**。关闭后，文本会保持在同一行。

### 材质

* **颜色 Alpha**：主 **颜色**的不透明度，范围为 0 到 1。当 **透明模式** 为 **不透明**.
* **Alpha 测试**：像素变为不可见的截止值。它仅在 **透明模式** 为 **Alpha 测试** 或 **Alpha 测试与混合**时出现，因为只有这些模式会使用它。
* **无**：颜色下拉菜单现在提供一个 **无** 条目以清除颜色，而纹理 **类型** 下拉菜单提供 **无** 以移除纹理。

### 粒子系统

“ **旋转** ”部分控制粒子的朝向和旋转方式：

* **公告牌**：每个粒子始终面向摄像机。默认开启。对于应在 3D 空间中自由翻滚的粒子，请将其关闭。
* **面向移动方向**：每个粒子初始指向其移动方向，如同箭头。
* **初始旋转（度）**：每个粒子出生时沿各轴的旋转角度。
* **随时间旋转（度/秒）**：沿各轴的旋转速度。每轴数值上限为 180。

当 **使用纹理** 开启时，会出现另外两个下拉菜单：

* **环绕模式**：纹理在超出边缘时的行为方式。 **钳制** （默认）会拉伸边缘像素， **重复** 会平铺它， **镜像** 会以交替翻转的方式平铺它。
* **过滤模式**：纹理的采样方式。 **双线性** （默认）较为平滑， **点** 为像素化，适合像素艺术， **三线性** 会在 mipmap 级别之间进行平滑处理。

### 补间

**补间类型** 提供五种模式：

| 模式                           | 功能              |
| ---------------------------- | --------------- |
| **移动项目**, **旋转项目**, **缩放项目** | 在 **持续时间**.     |
| **持续移动**                     | 以恒定速度沿固定方向永久移动。 |
| **持续旋转**                     | 以恒定速度绕固定轴永久旋转。  |

这两种持续模式没有结束值，因此 **开始**, **结束**, **持续时间**, **缓动函数** 和 **循环** 对它们是隐藏的。你会改为获得：

* **持续移动**：一个 **方向** 向量和一个 **速度（米/秒）**。使用长度为 1 的方向向量，使速度恰好以米每秒为单位。负速度会向相反方向移动。
* **持续旋转**：一个 **旋转轴（度）** 和一个 **速度（度/秒）**。只有轴的方向重要，角度大小并不重要。

切换 **补间类型** 会将这些值重置为新模式的合理默认值，因为它们在每种模式中的含义不同。

“ **播放** 复选框（之前称为“自动开始”）决定补间是否在场景加载后立即运行。关闭时，补间会在起始值处暂停创建，只有当你的代码将其设为播放时才会运行。


> [!NOTE]
> **💡 提示**：某些组件属性只能通过代码设置。在检查器中编辑实体不再会丢弃这些属性，因此你可以在场景代码中设置值，然后继续在编辑器中使用该实体而不会丢失它。


## 智能项目

[智能项目](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/smart-items/) 还可以包含控制实体交互性的特殊组件。它们通常是：

* **动作**：列出该项目可执行的所有可能操作。
* **触发器**：确定何时执行动作组件中的操作。
* **状态**：跟踪项目的当前状态。该状态可用于条件逻辑，仅在项目处于特定状态时触发某些操作。
* **计数器**：跟踪一个计数器。计数器可用于条件逻辑，仅在计数器的值等于、高于或低于给定值时触发某些操作。

请参阅 [智能项目高级](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/smart-items-advanced/) 了解更多详情。

## 关于实体和组件

场景中的一切都是实体。场景中的所有项目和智能项目都是实体。

实体的所有特征均由其组件决定。它们定义实体是什么、位于何处、声音如何以及行为如何。例如，一个 **变换** 组件存储实体的坐标、旋转和缩放。一个 **MeshRenderer** 组件为实体提供可见形状（如立方体或球体），而一个 **材质** 组件为实体提供颜色或纹理。

组件上的值可以随时间改变。在场景编辑器中，你可以配置这些组件的初始值。但场景运行后，玩家的操作或时间的流逝可能会改变这些值。

例如，一个移动平台智能项目具有通过其 **变换** 组件设置的初始位置，但在该项目的动作使其移动后，其 **变换** 将拥有不同的值。

请参阅 [实体和组件](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/entities-components/) 以深入了解这一概念及其在 Decentraland 场景中的使用方式。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-bian-ji-qi/gou-jian/components.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
