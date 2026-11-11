> 显示并控制 NPC 头像

# NPC 头像

将头像作为场景中的实体显示。


> [!NOTE]
> **💡 提示**: 试试 [NPC Toolkit 库](https://github.com/decentraland-scenes/dcl-npc-toolkit) ，以便更轻松地处理 NPC，尤其是在需要通过对话树进行交互时。


## 创建头像

以下代码片段创建一个带有随机穿戴物和身体形状、名称为“NPC”的头像。

```ts
const myAvatar = engine.addEntity()
AvatarShape.create(myAvatar)

Transform.create(myAvatar, {
	position: Vector3.create(4, 0.25, 5),
})
```

在传递数据以生成 `AvatarShape`时，以下字段为必填：

* `id`：Avatar 的内部标识符（必填）。最后 4 个字符的 `id` 会以淡化文本显示在 NPC 名称后面，就像玩家的钱包后缀一样。一个 `id` 少于 4 个字符则显示 `0000`。给每个 NPC 一个 `id` ，其结尾与名称搭配起来更自然，例如 `'guide-0001'`.

以下可选字段也可用：

* `名称`：显示在头像头顶上的名称。默认值："NPC"。将其设为空字符串可完全不显示名称，这在你只想要一个 [Avatar 名牌](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/tou-xiang/avatar-nametags/) 牌子放在 NPC 上方时很有用。
* `bodyShape`：用于定义要使用哪种身体形状的字符串。有效选项为 'urn:decentraland:off-chain:base-avatars:BaseMale' 和 'urn:decentraland:off-chain:base-avatars:BaseFemale'。
* `wearables`：包含头像当前穿戴物 URN 列表的数组。如果穿戴物冲突（例如有两个帽子），列表中的最后一个会替换另一个。
* `emotes`：包含头像能够播放的 NFT 表情 URN 列表的数组
* `eyeColor`: *Color3* 用于眼睛颜色（任意颜色均可）
* `skinColor`: *Color3* 用于皮肤颜色（任意颜色均可）
* `hairColor`: *Color3* 用于头发颜色（任意颜色均可）
* `talking`：如果 *true*，则会在名称旁显示一组绿色条形，就像玩家在世界中使用语音聊天时一样。
* <div data-gb-custom-block data-tag="hint" data-style="info" class="hint hint-info"><p><strong>💡 提示</strong>：参见 <a href="/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/color-types/">颜色类型</a> 有关如何设置颜色的更多详情。</p></div>


> [!WARNING]
> **📔 注**： `AvatarShape`组件必须通过以下方式导入
> 
> > `import { AvatarShape } from "@dcl/sdk/ecs"`
> 
> 参见 [导入](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/coding-scenes/#imports) ，了解如何轻松处理这些内容。



> [!WARNING]
> **📔 注**：URN 字段必须遵循与 [NFTShapes](/docs/creator/content-creator-zh/chang-jing-sdk7/mei-ti/display-a-certified-nft/): `urn:decentraland:<CHAIN>:<CONTRACT_STANDARD>:<CONTRACT_ADDRESS>:<TOKEN_ID>`


## 移动 NPC

要移动 NPC，请更改 `位置` 它的 `Transform`。头像不会瞬间跳到新位置：它会从当前位置走过去，速度约为每秒 4 米，当目标距离达到 1.5 米或更远时会加快移动。移动时，它会转身面向行进方向。该 `rotation` 你在其中设置的 `Transform` 仅在到达后才会生效。

```ts
// NPC 会从当前位置走到新位置
Transform.getMutable(myAvatar).position = Vector3.create(10, 0.25, 5)
```

若要自行控制移动，请使用一个 [Tween](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/move-entities/)。对 position 使用 Tween 会取代这种自动行走；对 rotation 使用 Tween 则会阻止 NPC 转身面向其行进方向。一个实体一次只能有一个 Tween，因此不能同时对同一实体的位置和旋转做 Tween。

## 动画

头像静止时会播放默认待机动画。

要在头像上播放动画，请将 `expressionTriggerId` 字符串设置为你想播放的动画名称。

```ts
const myAvatar = engine.addEntity()
AvatarShape.create(myAvatar, {
	id: 'dancer-0001',
	emotes: [],
	wearables: [],
	expressionTriggerId: 'robot',
})

Transform.create(myAvatar, {
	position: Vector3.create(4, 0.25, 5),
})
```

该 `expressionTriggerId` 字段支持所有 [默认动画](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/tou-xiang/avatar-animations/#default-animations)，以及自定义动画 [来自场景文件的](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/tou-xiang/avatar-animations/#custom-animations)，甚至包括已发布到市场中的表情 URN。

要使用自定义动画，请提供 `_emote.glb` 文件在项目根目录中的路径，例如 `'assets/animations/Cheer_emote.glb'`。与 [`triggerSceneEmote()`](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/tou-xiang/avatar-animations/#custom-animations)一样，路径错误则不会播放任何内容。


> [!WARNING]
> **📔 注**: `AvatarShape` 没有 `蒙版` 字段，因此一个 `expressionTriggerId` 动画总是作用于整个身体。 [仅为上半身制作动画](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/tou-xiang/avatar-animations/#animate-only-the-upper-body) 仅适用于玩家自己的头像，不适用于 NPC。


### 在 NPC 播放表情时移动它

修改头像的 `Transform.position`，或者对其运行 [Tween](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/move-entities/) ，都会取消当前播放的表情。头像会回到行走循环，因为 Decentraland 会根据头像自上一帧以来移动了多远来判断要从哪种移动动画进行混合。

要在 NPC 播放表情时移动它，如果移动范围较小，你可能希望将移动直接烘焙到动画本身中，例如在 `_emote.glb` 文件内加入 root 或 hips 运动。

### 循环动画

在 `AvatarShape` 上播放的动画只会执行一次；如果你希望头像持续循环播放动画，应创建一个系统，每隔几秒让它再次播放该动画。

唯一的例外是会循环的默认表情， `sittingChair1`, `sittingChair2`, `sittingGround1` 和 `sittingGround2`，它们会在一个 `AvatarShape` 上保持姿势，无需重新播放。来自文件的自定义动画在一个 `AvatarShape`上绝不会循环，即使该片段本身被标记为循环，因此要持续保持自定义姿势，需要使用下面的重播模式。

使用 `expressionTriggerTimestamp` 来重播同一个表情。该字段的值是一个 [Lamport 时间戳](https://en.wikipedia.org/wiki/Lamport_timestamp)，这意味着它不是时间值，而是一个在每次重复表情时加 1 的索引。

因此第一次播放表情时，你设置 `expressionTriggerTimestamp` 设为 *0*。若要再次播放该表情，必须将该值更新为 1。引擎就是这样判断这是一条新指令，而不是它已经执行过的指令。

以下代码片段创建了一个每 2 秒运行一次相同表情的系统：

```ts
const myAvatar = engine.addEntity()
AvatarShape.create(myAvatar, {
	id: 'fan-0001',
	emotes: [],
	wearables: [],
	expressionTriggerId: 'clap',
    expressionTriggerTimestamp: 0
})

Transform.create(myAvatar, {
	position: Vector3.create(4, 0.25, 5),
})

let clapTimer = 0
let emoteDuration = 2  // 2 seconds

// system
engine.addSystem((dt: number) => {
    clapTimer += dt
      
    if (clapTimer >= emoteDuration) {
        // 触发表情 clap
        AvatarShape.getMutable(myAvatar).expressionTriggerTimestamp += 1
        
        clapTimer = 0 // 重置计时器
    }
})
```


> [!NOTE]
> **💡 提示**：你必须知道表情的持续时间，并将其设为系统的持续时间。如果你创建了一个让头像保持同一姿势不动的表情，建议将表情持续时间设得比系统更长。这样可以确保在结束并重置动画时不会出现瑕疵。


## 在名称上方添加标签

使用 `AvatarNametag` 组件，可在 NPC 名称上方显示带有自定义文本的牌子，例如“Shopkeeper”或“Boss”之类的角色。将其附加到拥有 `AvatarShape`:

```ts
AvatarNametag.create(myAvatar, { label: 'Shopkeeper' })
```

如果 `AvatarShape` 拥有空的 `名称`，则只显示牌子，下方不会有空的名牌框。这对于只用头衔来标记 NPC 很方便。


> [!NOTE]
> **💡 提示**：对于没有文字的彩色牌子，请将 `label` 设置为空格字符串，并选择一个 `backgroundColor`。空格会被保留，因此空格越多，牌子越宽。
> 
> ```ts
> AvatarNametag.create(myAvatar, {
> 	label: '      ',
> 	backgroundColor: Color3.Red(),
> })
> ```
> 


参见 [Avatar 名牌](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/tou-xiang/avatar-nametags/) 以查看包含颜色在内的完整字段列表。

## 从玩家复制穿戴物

以下代码片段会更改 NPC 头像的穿戴物和其他特征，使其与玩家当前穿戴的内容一致。这可以在场景中用作人体模型，展示某件特定穿戴物或表情，并与玩家当前的装扮搭配。

```ts
import { getPlayer } from '@dcl/sdk/src/players'


export function swapAvatar(avatar: Entity) {

  let userData = getPlayer()
  console.log(userData)

  if (!userData || !userData.wearables) return

  const mutableAvatar = AvatarShape.getMutable(avatar)

  mutableAvatar.wearables = userData.wearables
  mutableAvatar.bodyShape = userData.avatar?.bodyShapeUrn
  mutableAvatar.eyeColor = userData.avatar?.eyesColor
  mutableAvatar.skinColor = userData.avatar?.skinColor
  mutableAvatar.hairColor = userData.avatar?.hairColor
  
}
```

## 仅显示穿戴物

使用 `showOnlyWearables` 字段用于仅显示头像中列出的穿戴物。头像其余身体部分将不可见。

```ts
const myAvatar = engine.addEntity()
AvatarShape.create(myAvatar, {
	id: 'mannequin-0001',
	emotes: [],
	wearables: [
    'urn:decentraland:matic:collections-v2:0x90e5cb2d673699be8f28d339c818a0b60144c494:0'
  ],
	showOnlyWearables: true,
})

Transform.create(myAvatar, {
	position: Vector3.create(4, 0.25, 5),
})
```

这对于展示穿戴物很有用，例如在商店中。


> [!NOTE]
> **💡 提示**：如果某个穿戴物比较小，可以尝试将 `scale` 其 `Transform` 的值设置得更大一些。


## 将实体附加到 NPC

你可以使用 `AvatarAttach` 功能将一个实体固定到 NPC 头像的某根骨骼上，例如让 NPC 手中拿着一个物体。该实体会在头像动画播放时随之移动。

要使用此功能，请使用 `id` 在 `AvatarShape` 属性为该头像分配任意 id，然后在 `AvatarAttach`上引用该 id。id 可以是你想要的任意字符串。

```ts
// 创建 NPC，并带有 ID
const myAvatar = engine.addEntity()
Transform.create(myAvatar, {
  position: Vector3.create(8, 0.25, 8),
})
AvatarShape.create(myAvatar, {
  id: "my-avatar-id", 
  wearables: [],
  emotes: []
})

// 创建要附加到 NPC 的对象
const attachedEntity = engine.addEntity()
Transform.create(attachedEntity, {
    position: Vector3.create(4, 2, 4),
    scale: Vector3.create(0.15,0.15,0.15)
})
MeshRenderer.setBox(attachedEntity)
Material.setBasicMaterial(attachedEntity, { diffuseColor: Color4.Blue() })
AvatarAttach.create(attachedEntity, {
    avatarId: "my-avatar-id",
    anchorPointId: AvatarAnchorPointType.AAPT_LEFT_HAND
})
```

了解更多关于 `AvatarAttach` 组件 [这里](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/entity-positioning/#attach-an-entity-to-an-avatar).

## 替换玩家的头像

你无法更改玩家头像当前穿戴的物品，但可以改为用一个可完全自定义的 NPC 头像替换玩家头像。

要做到这一点，请使用一个 [Avatar Modifier Area](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/tou-xiang/modifier-areas/#hide-avatars) ，其中使用 `AMT_HIDE_AVATARS` 修饰器来隐藏玩家自己的头像，并在玩家位置显示一个 `AvatarShape` 实体。


> [!WARNING]
> **📔 注**：若要让玩家完全控制该头像，你应监听按钮事件来检测他们何时按下按钮，然后在 NPC 头像上触发相应动画。参见 [按钮事件](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/system-based-events/) 了解更多详情。
> 
> 这样做时控制的流畅性可能并不完美，因此你可能只想在非常特定的情况下使用它。



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/tou-xiang/npc-avatars.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
