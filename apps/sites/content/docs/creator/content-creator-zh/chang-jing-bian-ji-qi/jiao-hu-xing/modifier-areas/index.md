> 在 Scene Editor 中使用修饰区域来改变头像行为，或在你场景的某个区域内强制使用特定摄像机模式。

# 修饰区域

修饰器区域是不可见的区域，它们会改变玩家体验场景某一部分的方式。共有两种类型：

* **头像修饰区域**: 会改变区域内玩家看到的头像的行为或外观，例如隐藏头像或禁用 Passport 弹窗。
* **摄像机修改器区域**: 会在玩家进入区域时强制其摄像机切换为第一人称或第三人称。

玩家走出后，这两种类型都会恢复正常行为。

## 添加修饰器区域

你可以通过两种方式添加修饰器区域：

**从资源目录：** 打开资源目录并搜索 **头像修饰区域** 或 **摄像机修改器区域** 在 **Utils** 类别下。将其拖入场景中。该项目会显示为一个半透明的占位立方体，只在编辑器中可见，在运行中的场景里不可见。

**从组件菜单：** 选择任意已有实体，点击 **+** 按钮，然后在属性面板顶部选择 **头像修饰区域** 或 **摄像机修改器区域** ，从列表中选择。参见 [添加组件](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/gou-jian/components/#add-components) 以了解更多详情。

![“添加组件”菜单在一个名为 Stage area 的实体上打开，列出了 3D Content 和 Interaction 组下的组件，包括 Avatar Modifier Area 和 Camera Modifier Area。](/docs/_remote/e0e52c0c060b63e97e2466dd.webp)

## 调整区域大小

修饰器区域的大小由实体的 **Transform scale**控制。如果将实体缩放为 4 x 3 x 4，修饰器区域就会变成一个以实体位置为中心、尺寸为 4 x 3 x 4 米的盒子。使用缩放操纵器，或在 Transform 组件中输入精确数值来设置所需大小。


> [!WARNING]
> **注意：** 编辑器面板中没有单独的“Area”字段。编辑器会自动将内部 area 属性与实体的缩放保持同步，因此只需调整实体大小即可。


## 头像修饰区域

Avatar Modifier Area 面板有两个设置：

![Avatar Modifier Area 组件面板，Modifiers 下拉菜单中已选中 Hide Avatars，下面有一个 Exclude Player IDs 区域和一个 Add Player ID 按钮。](/docs/_remote/d49ca42bf27a1995edcc536d.webp)

### 修饰项

一个可多选的下拉菜单，包含以下选项：

* **隐藏头像**: 区域内的头像会变得不可见。区域内的玩家彼此看不见，但区域外的玩家仍能正常看到他们。
* **禁用 Passport**: 点击区域内玩家的头像将不再打开其个人资料弹窗。

你可以在同一区域中选择一个或两个修饰项。添加该组件时， **隐藏头像** 会默认选中。点击 **x** 修饰项上的

### 排除的玩家 ID

一个不受修饰项影响的钱包地址列表。地址在此列表中的玩家即使身处区域内也会正常表现。

点击 **添加玩家 ID** 以添加一条记录，然后粘贴钱包地址。要删除一条记录，请点击其旁边的三点菜单并选择 **移除玩家 ID**.

这在活动中很有用，你可能想在舞台上隐藏观众头像，同时保持表演者可见。


> [!WARNING]
> **注意：** 请确保钱包地址使用小写。修饰器区域采用严格的字符串匹配。


### 使用场景

* **隐私区域**: 在后台或更衣区隐藏头像。
* **整洁舞台**: 在舞台上隐藏观众头像，这样只有表演者（列在 Exclude Player IDs 中）可见。
* **游戏竞技场**: 在战斗区域禁用 Passport，避免玩家在游戏过程中误打开个人资料。

## 摄像机修改器区域

Camera Modifier Area 面板只有一个设置：

![Camera Modifier Area 组件面板，其中 Camera Mode 下拉菜单设置为 First Person。](/docs/_remote/199bb09636ac6f76d722633e.webp)

### Camera Mode

一个包含两个选项的下拉菜单：

* **第一人称**: 玩家在区域内时会被强制切换为第一人称视角。
* **第三人称**: 玩家在区域内时会被强制切换为第三人称视角。

默认值为 **第一人称**。当玩家离开该区域时，会恢复其之前的摄像机模式。


> [!NOTE]
> **提示：** 这里不提供电影式摄像机模式。若要在第一/第三人称切换之外控制摄像机，请使用 [Virtual Camera](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/camera/#using-virtual-cameras) 并结合代码。


### 使用场景

* **狭窄走廊或解谜房间**: 强制第一人称，便于精确点击小物体。
* **风景观景点**: 强制第三人称，让玩家能在背景前看到自己的头像。
* **教程区域**: 确保所有玩家在引导体验中使用相同的摄像机模式。

## 组合修饰器区域

你可以将两种类型的修饰器区域放在同一个实体上，或让独立的修饰器区域在同一空间中重叠。每个区域都会独立生效。


> [!WARNING]
> **注意：** 请尽量保持场景中的修饰器区域数量较少。数量过多可能影响性能。对于大多数场景来说，少量几个就足够了。


## 通过代码使用修饰器区域

Scene Editor 中的组件与 SDK7 的 `AvatarModifierArea` 和 `CameraModeArea` 组件直接对应。有关可用修饰项、运行时行为以及动态更改排除列表等高级选项的完整详情，请参见：

* [头像修饰器区域](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/tou-xiang/modifier-areas/) （SDK7）
* [摄像机模式区域](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/camera/#1st-and-3rd-person-camera-modes) （SDK7）


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/modifier-areas.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
