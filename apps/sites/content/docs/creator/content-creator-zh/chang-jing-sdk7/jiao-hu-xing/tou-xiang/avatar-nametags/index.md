> 在头像姓名标签上方显示自定义标签，以展示等级、角色或队伍。

# 头像姓名标签

该 `AvatarNametag` 该组件会在头像的常规名牌上方添加一个小牌子，显示场景选择的文本。可用于展示等级、角色、队伍，或场景跟踪玩家的任何其他信息，例如“VIP”、“红队”或“俱乐部所有者”。

<figure><img src="/docs/_remote/bb915fa6311eac2059c6e8e8.webp" alt="An avatar with a Club Owner plate floating above its regular nametag"><figcaption><p>场景提供的、位于玩家常规名牌上方的牌子</p></figcaption></figure>

本页介绍如何添加牌子、设置样式，以及在玩家进出时保持同步。


> [!WARNING]
> **📔 注意**: `AvatarNametag` 需要 `@dcl/sdk` 7.28.0 或更高版本。


## 给玩家添加牌子

唯一必需的字段是 `label`，要显示的文本。

```ts
import { AvatarNametag, engine } from '@dcl/sdk/ecs'

// 在当前玩家名牌上方显示“俱乐部所有者”
AvatarNametag.create(engine.PlayerEntity, {
	label: '俱乐部所有者',
})
```

该牌子默认使用客户端自己的名牌颜色，因此无需额外操作就能与界面其余部分保持一致。


> [!NOTE]
> **💡 提示**：你无需等待头像完成加载后再写入该组件。如果头像仍在加载，牌子会在就绪后立即应用。


## 组件的适用范围

`AvatarNametag` 只会对带有头像的实体生效：

* `engine.PlayerEntity`，即运行场景的玩家。
* 其他玩家的实体，可通过其 [`PlayerIdentityData`](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/user-data/) 组件。
* 一个 [NPC 头像](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/tou-xiang/npc-avatars/)，即任何带有 `AvatarShape` 组件。

在其他任何实体上，该组件都会被忽略。


> [!WARNING]
> **📔 注意**：牌子仅存在于绘制它的客户端上。它绝不会发送给其他玩家。如果你想让所有人看到相同的牌子，每个客户端都必须基于大家都能读取的数据运行相同的逻辑。参见 [多人场景中的牌子](#plates-in-multiplayer-scenes).


## 更改颜色

三个可选字段接受一个 [Color3](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/color-types/):

* `labelColor`：文本颜色。默认使用客户端原生名牌的文字颜色。
* `backgroundColor`：牌子的填充色。默认使用客户端原生名牌的背景色。
* `borderColor`：牌子的轮廓色。默认使用 `backgroundColor`，因此除非你设置它，否则不会显示可见边框。

`Color3` 没有 alpha 通道，而且该组件没有 opacity 字段，因此牌子始终完全不透明。

```ts
import { AvatarNametag, engine } from '@dcl/sdk/ecs'
import { Color3 } from '@dcl/sdk/math'

// 深蓝色牌子上的白色标签，带浅蓝色边框
AvatarNametag.create(engine.PlayerEntity, {
	label: '学生',
	labelColor: Color3.White(),
	backgroundColor: Color3.create(0.1, 0.2, 0.6),
	borderColor: Color3.create(0.78, 0.85, 1),
})
```

若要将某个字段恢复为原生颜色，请在重新写入组件时不包含该字段。请使用 `createOrReplace` 而不是 `getMutable`，因为只有替换整个组件才会清除你省略的字段：

```ts
// 保留标签，移除自定义颜色并恢复原生外观
AvatarNametag.createOrReplace(engine.PlayerEntity, { label: '学生' })
```

## 移除牌子

使用 `deleteFrom` 用它来移除该牌子。头像的常规名牌不受影响：

```ts
AvatarNametag.deleteFrom(engine.PlayerEntity)
```

## 标签行为

在选择标签文本之前，有几点值得了解：

* **标签只占一行。** 它们不会自动换行。
* **较长的标签会被截断** 并以省略号结尾。标签尽量保持一两个词。
* **空标签** (`''`）会绘制一个不含文字的牌子。
* **空格会被保留**，因此只由空格组成的标签会拉宽一个无文字牌子。这是一种为纯色牌子设定尺寸的方法。
* **若要隐藏文字但保留其宽度**，请将 `labelColor` 设置为与……相同的值 `backgroundColor`.


> [!NOTE]
> **💡 提示**：牌子不需要任何文字。例如，若要只显示一个带颜色编码的牌子来标记队伍归属，请将 `label` 设置为一串空格，并选择一个 `backgroundColor`。空格越多，牌子越宽。
> 
> ```ts
> AvatarNametag.createOrReplace(engine.PlayerEntity, {
> 	label: '      ',
> 	backgroundColor: Color3.Red(),
> })
> ```
> 


## 在玩家进入时标记他们

给其他玩家加标记的最简单方法，是在他们进入场景时使用 `onEnterScene` 事件。每次它都会给你一个全新的实体，因此无需查找或缓存：

```ts
import { onEnterScene } from '@dcl/sdk/players'
import { AvatarNametag } from '@dcl/sdk/ecs'

export function main() {
	onEnterScene((player) => {
		AvatarNametag.createOrReplace(player.entity, { label: 'VIP' })
	})
}
```

参见 [获取玩家数据](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/user-data/) 了解更多关于跟踪玩家进出的信息。

## 为场景中的每位玩家加标记

若要一次性在每个人上方显示牌子，请遍历具有 `PlayerIdentityData` 组件。

先将实体收集到数组中 **然后再** 对它们写入。在仍然遍历 `engine.getEntitiesWith()` 可能会在循环过程中使查询失效。

```ts
import { AvatarNametag, engine, Entity, PlayerIdentityData } from '@dcl/sdk/ecs'

const LABELS = ['蓝队', '红队']

function tagAllPlayers() {
	// 1. 先收集
	const players: { entity: Entity; address: string }[] = []
	for (const [entity, identity] of engine.getEntitiesWith(PlayerIdentityData)) {
		players.push({ entity, address: identity.address.toLowerCase() })
	}

	// 按地址排序能让每个客户端得到相同结果，
	// 无论玩家以什么顺序加入
	players.sort((a, b) => a.address.localeCompare(b.address))

	// 2. 然后写入，但仅在标签实际变化时
	players.forEach(({ entity }, index) => {
		const label = LABELS[index % LABELS.length]
		if (AvatarNametag.getOrNull(entity)?.label === label) return
		AvatarNametag.createOrReplace(entity, { label })
	})
}
```

在写入前先与当前值比较，可以避免每次遍历都进行不必要的组件更新。

### 跟踪玩家的加入和离开

玩家实体 ID 是 **在断开连接后并不稳定**。当玩家离开时，他们的实体 ID 会被回收，并可能分配给下一位加入的玩家，因此写到旧实体上的牌子最终会显示在错误的头像上。

以下两条规则可确保正确性：

* 在……之前重新解析实体 **每次** 写入，而不是缓存该实体。
* 玩家离开时移除该组件。

定时重新运行标记函数，可以捕捉后加入的玩家，并且不会持有过期实体。每秒一次就足够频繁：

```ts
let timer = 0

engine.addSystem((dt: number) => {
	timer += dt
	if (timer < 1) return
	timer = 0
	tagAllPlayers()
})
```

## 多人场景中的牌子

牌子仅对每位玩家的客户端本地可见。牌子不会发送给其他玩家，每位玩家客户端上的场景副本都会计算自己的牌子。如果所有玩家都应看到相同的标签，那么分配这些标签的逻辑必须基于大家共享的数据，在每个客户端上产生相同结果。像上面的示例那样按钱包地址对玩家排序，就是一种实现方式。否则，你的场景需要自己同步这些分配，参见 [无服务器多人模式](/docs/creator/content-creator-zh/chang-jing-sdk7/lian-wang/serverless-multiplayer/).

## 与隐藏名牌的交互

一个 [`AvatarModifierArea`](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/tou-xiang/modifier-areas/#hide-nametags) 与 `AMT_HIDE_NAMETAGS` 修饰符会连同常规名牌一起隐藏这些牌子。同样也适用于 `AMT_HIDE_AVATARS`，它会隐藏整个头像。

如果你希望一个头像显示 **仅** 你的牌子，而下面不显示名字，请将 `AvatarShape` 组件的 `名称` 设置为空字符串。参见 [NPC 头像](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/tou-xiang/npc-avatars/#add-a-label-above-the-name).

## 相关页面

* [头像修饰区域](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/tou-xiang/modifier-areas/)：隐藏场景某一区域内的头像和名牌。
* [移动玩家](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/tou-xiang/move-player/) 和 [头像动画](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/tou-xiang/avatar-animations/)：控制玩家的头像。
* [NPC 头像](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/tou-xiang/npc-avatars/)：显示非玩家头像。
* [获取玩家数据](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/user-data/)：读取有关玩家和场景中其他所有人的数据。
* [颜色类型](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/color-types/)：如何构建 `Color3` 值列表。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/tou-xiang/avatar-nametags.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
