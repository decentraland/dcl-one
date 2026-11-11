> 在场景中的某个区域内隐藏头像、隐藏姓名标签，或禁用玩家护照

# 头像修饰区域

化身在穿越场景时，在整个 Decentraland 中的行为和外观都保持一致。不过，你可以添加一个 `AvatarModifierArea` 到场景中的某个区域，以影响玩家化身进入该区域时的行为。


> [!CAUTION]
> **❗警告**\
> 请将 `AvatarModifierArea` 在场景中使用的数量控制在几个以内。如果使用过多，可能会对性能造成显著影响。


### 放置 Avatar 修饰区域

添加一个带有 `AvatarModifierArea` 组件的实体，并使用一个 `Transform` 组件。

```ts
const entity = engine.addEntity()

AvatarModifierArea.create(entity, {
	area: Vector3.create(4, 3, 4),
	modifiers: [AvatarModifierType.AMT_HIDE_AVATARS],
	excludeIds: []
})

Transform.create(entity, {
	position: Vector3.create(8, 0, 8),
})
```

在创建一个 `AvatarModifierArea` 组件。你必须提供以下内容：

* `area`：修饰区域的大小
* `modifiers`：列出要在该区域中实现的修饰器的数组。此属性使用来自 `AvatarModifierType` 枚举中的一个值。

支持的修饰器有：

* `AvatarModifierType.AMT_HIDE_AVATARS`
* `AvatarModifierType.AMT_DISABLE_PASSPORTS`
* `AvatarModifierType.AMT_HIDE_NAMETAGS`

某个 `AvatarModifierArea` 的所有效果只会在其区域范围内生效。玩家走出该区域后会恢复正常。

一个 `AvatarModifierArea` 只会影响位于该区域内的玩家。进入该区域不会影响对区域外其他玩家的感知。

某个 `AvatarModifierArea` 的效果会针对每个玩家在本地计算。你可以有一个 `AvatarModifierArea` 仅对某些玩家出现在场景中，而对其他玩家不存在的区域。例如，你可以制作一个“马可波罗”游戏，在这个游戏里，场景中只有一名玩家拥有一个能隐藏所有其他玩家的修饰区域。其他在本地版本场景中没有这个修饰区域的玩家，则可以正常看到彼此。

如果该区域会隐藏化身，那么在本地版本场景中没有该区域的玩家将正常看到所有化身，即使那些把自己体验为被隐藏的玩家也是如此。拥有该区域的玩家在进入时，会体验到自己和所有其他化身都受该区域影响。


> [!WARNING]
> **📔 注意**：Avatar 修饰区域会受到 *位置* 和 *旋转* 其宿主实体的 Transform 组件的影响，但不会受到 *缩放*.



> [!WARNING]
> **📔 注意**： `AvatarModifierArea`组件必须通过以下方式导入
> 
> > `import { AvatarModifierArea } from "@dcl/sdk/ecs"`
> 
> 参见 [导入](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/coding-scenes/#imports) 以了解如何轻松处理这些内容。


### 隐藏化身

当玩家走进一个 `AvatarModifierArea` 并具有 `AvatarModifierType.AMT_HIDE_AVATARS` 修饰器时，玩家的化身将停止渲染。这既适用于第三人称视角下的玩家，也适用于其他玩家走进该区域时。

```ts
const entity = engine.addEntity()

AvatarModifierArea.create(entity, {
	area: Vector3.create(4, 3, 4),
	modifiers: [AvatarModifierType.AMT_HIDE_AVATARS],
	excludeIds: []
})

Transform.create(entity, {
	position: Vector3.create(8, 0, 8),
})
```

这允许你用你可能想在场景中展示的任意自定义化身替换默认的 Decentraland 化身，参见 [替换玩家的化身](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/tou-xiang/npc-avatars/#replace-the-players-avatar)。请注意，如果你想看到带有自定义化身的其他玩家，你需要自己处理玩家位置的同步。

### 禁用个人资料弹窗

当玩家走进一个 `AvatarModifierArea` 并具有 `AvatarModifierType.AMT_DISABLE_PASSPORTS` 修饰器后，点击他们将不再打开显示玩家简介、库存等信息的个人资料界面。

```ts
const entity = engine.addEntity()

AvatarModifierArea.create(entity, {
	area: Vector3.create(4, 3, 4),
	modifiers: [AvatarModifierType.AMT_DISABLE_PASSPORTS],
	excludeIds: []
})

Transform.create(entity, {
	position: Vector3.create(8, 0, 8),
})
```

这在某些游戏中尤其有用，因为误打开此界面可能会打断游戏节奏，例如多人射击游戏。

### 隐藏姓名标签

当玩家走进一个 `AvatarModifierArea` 并具有 `AvatarModifierType.AMT_HIDE_NAMETAGS` 修饰器后，玩家的姓名标签会被隐藏，而化身本身仍然可见。

```ts
const entity = engine.addEntity()

AvatarModifierArea.create(entity, {
	area: Vector3.create(4, 3, 4),
	modifiers: [AvatarModifierType.AMT_HIDE_NAMETAGS],
	excludeIds: []
})

Transform.create(entity, {
	position: Vector3.create(8, 0, 8),
})
```

这适用于舞台、演示或脚本化场景，你希望在不隐藏化身本身的情况下抑制玩家姓名标签。例如，在表演期间，你可能希望获得更简洁的视觉体验，让化身可见，而悬浮的名字不会分散观众注意力。

`AMT_HIDE_NAMETAGS` 也会隐藏通过 [`AvatarNametag`](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/tou-xiang/avatar-nametags/) 组件。


> [!NOTE]
> **💡 提示**: `AMT_HIDE_AVATARS` 已经会连同化身一起隐藏姓名标签，因此你不需要添加 `AMT_HIDE_NAMETAGS` ，在使用 `AMT_HIDE_AVATARS`时无需使用 `AMT_HIDE_NAMETAGS` 。只有在你想隐藏姓名标签但保留化身可见时才使用。


你也可以将 `AMT_HIDE_NAMETAGS` 与其他修饰器一起使用，例如 `AMT_DISABLE_PASSPORTS` ，应用于同一区域：

```ts
const entity = engine.addEntity()

AvatarModifierArea.create(entity, {
	area: Vector3.create(4, 3, 4),
	modifiers: [AvatarModifierType.AMT_HIDE_NAMETAGS, AvatarModifierType.AMT_DISABLE_PASSPORTS],
	excludeIds: []
})

Transform.create(entity, {
	position: Vector3.create(8, 0, 8),
})
```


> [!NOTE]
> **💡 提示**：只有当玩家的头部或躯干位于该区域内时，姓名标签才会被隐藏。如果区域太矮，玩家通过二段跳越过它时，姓名标签会短暂重新出现。请将区域设置得足够高，以覆盖预期的移动范围。


### 排除化身

你可以通过将玩家 ID 添加到该修饰区域的一个数组中，来排除一组玩家不受其影响，该数组位于 `excludeIds` 属性中。

此示例会隐藏某个区域内的所有化身，但特定 ID 的玩家除外。比如在直播活动中，你可以用它只显示舞台上的活动主持人，并隐藏任何跳上舞台的其他玩家。

```ts
const entity = engine.addEntity()

AvatarModifierArea.create(entity, {
	area: Vector3.create(4, 3, 4),
	modifiers: [AvatarModifierType.AMT_HIDE_AVATARS],
	excludeIds: ['0xx1...', '0xx2...'],
})

Transform.create(entity, {
	position: Vector3.create(8, 0, 8),
})
```


> [!WARNING]
> **📔 注意**：请确保玩家 ID 全部使用小写字母。必要时使用 `.toLowerCase()` 。


修饰区域会在每个玩家的实例中本地运行。被排除的 ID 列表可以因玩家而异。在下面的示例中，每个玩家都会将自己的 ID 从一个隐藏化身的修饰器中排除，这样他们就只会看到自己的化身，而看不到其他人的。

```ts
import { getPlayer } from '@dcl/sdk/src/players'

export function main() {
	let userData = getPlayer()
	if (!userData) return

	const entity = engine.addEntity()

	AvatarModifierArea.create(entity, {
		area: Vector3.create(16, 5, 16),
		modifiers: [AvatarModifierType.AMT_HIDE_AVATARS],
		excludeIds: [userData.userId],
	})

	Transform.create(entity, {
		position: Vector3.create(8, 0, 8),
	})
}
```


> [!CAUTION]
> **❗警告**\
> 如果排除 ID 列表会定期变化（例如根据玩家进入或离开某个区域），请确保列表保持有序。对数组执行 `.sort()` ，这样列表在每次传递时都保持相同顺序。这样只会计算列表中的变化。否则，这可能会对场景性能造成显著影响。
> 
> ```ts
> AvatarModifierArea.create(entity, {
> 	area: Vector3.create(16, 5, 16),
> 	modifiers: [AvatarModifierType.AMT_HIDE_AVATARS],
> 	excludeIds: myAvatarList.sort(),
> })
> ```
> 


### 重叠区域与运行时变化

每个化身对每种效果都只有一个开/关标志，而不是记录其所处区域的数量。每个区域都会写入同一个标志，最后一次写入生效。这会导致两个容易让人出错的结果：

* **离开一个区域会让化身重新显示，即使它仍然位于另一个区域内。** 走出一个隐藏化身的小区域后，化身会再次可见，尽管一个覆盖整个场景的隐藏区域仍然覆盖着那个位置。
* **一个其 `excludeIds` 列表中包含某个玩家的区域，会在该玩家进入时为其写入“可见”**，针对该区域本身列出的每种效果。这样会覆盖他们同时站立其中的“隐藏所有人”区域。

建议每个化身同一时间只使用一个区域。如果区域确实重叠，请确保它们在每种效果上保持一致。

**就地移除修饰器不会撤销其效果。** 当你更改某个区域的字段时，客户端会重新应用当前列出的修饰器。不会恢复你从列表中移除的那个效果，因此清空 `modifiers`，或者将 `AMT_HIDE_AVATARS` 替换为 `AMT_HIDE_NAMETAGS`，都会让化身保持隐藏。就地添加修饰器是有效的。

要将某个区域切换为另一组修饰器，请删除该实体并在同一帧中创建替代实体：

```ts
import { AvatarModifierArea, AvatarModifierType, engine, Entity, Transform } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

// 替换该区域，而不是就地编辑其修饰器
function setModifiers(oldArea: Entity, modifiers: AvatarModifierType[]): Entity {
	engine.removeEntity(oldArea) // 这也会让其中的每个化身重新显示

	const area = engine.addEntity()
	Transform.create(area, { position: Vector3.create(8, 1, 8) })
	AvatarModifierArea.create(area, {
		area: Vector3.create(16, 5, 16),
		modifiers,
		excludeIds: [],
	})
	return area
}
```

### 调试修饰区域

仅根据代码，很难准确知道你的修饰区域覆盖了场景的哪些部分。可视化反馈能极大帮助确认它们是否放置得当。

要验证某个 `AvatarModifierArea` 或 `CameraModeArea`的位置，请给持有它的实体添加一个 `MeshRenderer` 组件，并使用一个 `box` 形状，并将缩放设置为与 `area` 修饰区域相同的大小。


> [!WARNING]
> **📔 注意**：修饰区域不受 `缩放` transform 的属性影响，它们的大小基于其 `area` 属性。


```ts
const entity = engine.addEntity()
const areaSize = Vector3.create(8, 3, 8)

AvatarModifierArea.create(entity, {
	area: areaSize,
	modifiers: [AvatarModifierType.AMT_HIDE_AVATARS],
  	excludeIds: []
})

Transform.create(entity, {
	position: Vector3.create(8, 0, 8),
	scale: areaSize,
})

MeshRenderer.setBox(entity)
Material.setPbrMaterial(entity, {
	albedoColor: Color4.create(0.5, 0.5, 0.5, 0.5),
})
```

要激活修饰区域的效果，玩家的头部或躯干必须进入该区域。如果只有玩家的脚在区域内，则不会生效。请确保玩家不能轻易通过跳跃绕过该区域。


> [!WARNING]
> **📔 注意**：整个区域应完全位于你的场景边界之内。



> [!NOTE]
> **💡 提示**：如需可运行示例，请查看 [`9,99-modifier-areas`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/9,99-modifier-areas) 测试场景，它结合了 `AvatarModifierArea` 与运行时变更的 `excludeIds`，以及 [`10,99-avatar-modifier-hide-nametags`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/10,99-avatar-modifier-hide-nametags)，其中单独展示了 `AMT_HIDE_NAMETAGS` 修饰器。



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/tou-xiang/modifier-areas.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
