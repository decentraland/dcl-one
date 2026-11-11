> 在玩家与你的场景互动时获取其数据。

# 用户数据

## 玩家位置和旋转

使用 `PlayerEntity` 以及 `CameraEntity` 要了解玩家的位置和旋转，可检查其 `Transform` 组件。

```ts
function getPlayerPosition() {
	if (!Transform.has(engine.PlayerEntity)) return
	if (!Transform.has(engine.CameraEntity)) return

	//玩家位置
	const playerPos = Transform.get(engine.PlayerEntity).position

	//玩家旋转
	const playerRot = Transform.get(engine.PlayerEntity).rotation

	//摄像机位置
	const CameraPos = Transform.get(engine.CameraEntity).position

	//摄像机旋转
	const CameraRot = Transform.get(engine.CameraEntity).rotation

	console.log('玩家位置: ', playerPos)
	console.log('玩家旋转: ', playerRot)
	console.log('摄像机位置: ', CameraPos)
	console.log('摄像机旋转: ', CameraRot)
}

engine.addSystem(getPlayerPosition)
```

* **PlayerEntity 位置**：头像的位置，胸口高度。大约在地面以上 0.88 米。
* **PlayerEntity 旋转**：头像面向的方向，以四元数表示。
* **CameraEntity 位置**:
  * 第一人称：与头像位置相同，但位于眼睛高度。大约在地面以上 1.75 米。
  * 第三人称：可能会因摄像机移动而变化。
* **PlayerEntity 旋转**:
  * 第一人称：与头像面向的方向相似，以四元数表示。与玩家的旋转可能会有轻微不同的取整。
  * 第三人称：可能会因摄像机移动而变化。


> [!WARNING]
> **📔 注意**：避免在 `engine.PlayerEntity` 或 `engine.CameraEntity` 在初始场景加载时引用它，因为如果实体尚未初始化，可能会导致错误。为避免此问题，请在 `main()` 函数内使用这些内容，或在被 `main()`间接调用的函数中使用。你也可以将该行为封装在一个异步 [`executeTask` 块中。](/docs/creator/content-creator-zh/chang-jing-sdk7/bian-cheng-mo-shi/async-functions/#the-executetask-function).
> 
> 另一种选择是在系统中引用这些实体。那里它们总是可用，因为系统的第一次执行会在场景已正确初始化后触发。


## 获取所有玩家

场景中的所有玩家都有一个 `Transform` 组件。该组件在头像中为只读。要获取所有玩家的位置， [遍历所有具有](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/user-data/) 一个 `PlayerIdentityData` 组件。

```ts
import { PlayerIdentityData } from '@dcl/sdk/ecs'

for (const [entity, data, transform] of engine.getEntitiesWith(
	PlayerIdentityData,
	Transform
)) {
	console.log('玩家数据: ', { entity, data, transform })
}
```

上面的代码会遍历所有具有一个 `Transform` 和一个 `PlayerIdentityData` 组件的实体，并记录它们的数据。你可以使用同样的方法来获取所有玩家的任何可用数据。

参见 [事件监听器](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/event-listeners/#player-enters-or-leaves-scene) 了解如何在新玩家加入场景时检测并响应。

## 获取玩家数据

使用 `getPlayer()` 用于获取当前玩家或场景中任何其他玩家的数据。

```ts
import { getPlayer } from '@dcl/sdk/src/players'

export function main() {
	createCube(5, 1, 5)

	let myPlayer = getPlayer()

	if (myPlayer) {
		console.log('名称: ', myPlayer.name)
		console.log('用户 ID: ', myPlayer.userId)
	}
}
```

`getPlayer()` 返回如下内容：

* `name`: *(字符串)* 玩家用户名，其他人在世界中看到的名称
* `userId`: *(字符串)* 标识该玩家的字符串。对于使用钱包连接的玩家，这是钱包地址的小写形式。对于访客账户，这是本地生成的标识符。
* `isGuest`: *(布尔值)* 表示玩家是否拥有公钥。 *真* 如果玩家是没有公钥的访客账户，则为真。
* `位置`: *(Vector3)* 头像在场景中的位置。
* `avatar`：一个嵌套对象，包含关于玩家基础头像和外观的数据。
* `穿戴物品`：玩家当前穿戴的每个穿戴物品的标识符数组。例如 `urn:decentraland:off-chain:base-avatars:green_hoodie`。所有穿戴物品都有类似的标识符，即使它们是 NFT。
* `表情动作`：玩家当前在快捷轮盘中装备的每个表情动作的标识符数组。
* `实体`：对玩家实体的引用。可方便传递给其他函数，或为其添加自定义组件。

该 `avatar` 对象包含以下嵌套信息：

* `bodyShapeUrn`：头像整体体型的标识符。可以是 `urn:decentraland:off-chain:base-avatars:BaseFemale` 表示女性，或 `urn:decentraland:off-chain:base-avatars:BaseMale` 表示男性。
* `skinColor`：玩家的皮肤颜色，类型为 `Color3`
* `eyesColor`：玩家的眼睛颜色，类型为 `Color3`
* `hairColor`：玩家的头发颜色，类型为 `Color3`
* `name`：玩家的名称。


> [!WARNING]
> **📔 注意**：根据加载时间，玩家数据可能不会在场景的第一帧可用。你应该先确认数据已返回，否则几毫秒后再重试。


```ts
import { getPlayer } from '@dcl/sdk/src/players'

export function main() {
	createCube(5, 1, 5)

	let myPlayer = getPlayer()

	if (myPlayer) {
		是否访客
		console.log('名称: ', myPlayer.name)
		console.log('用户 ID: ', myPlayer.userId)
		console.log('头像形状: ', myPlayer.position)
		console.log('头像形状: ', myPlayer.avatar?.bodyShapeUrn)
		console.log('头像眼睛颜色: ', myPlayer.avatar?.eyesColor)
		console.log('头像头发颜色: ', myPlayer.avatar?.hairColor)
		console.log('穿戴物品: ', myPlayer.wearables)
		console.log('可用表情动作: ', myPlayer.emotes)
	}
}
```

要获取场景中某个特定玩家（不同于当前玩家）的数据，请运行 `getPlayer()` 使用一个包含 `userId` 属性。

```ts
import { getPlayer } from '@dcl/sdk/src/players'

for (const [entity, data, transform] of engine.getEntitiesWith(
	PlayerIdentityData,
	Transform
)) {
	let player = getPlayer({ userId: data.address })
	console.log('玩家: ', player?.name)
}
```

上面的代码片段会遍历所有具有一个 `PlayerIdentityData` 组件的实体，也就是场景中的所有头像实体。然后它会对该实体运行 `getPlayer()` ，针对该实体。

`getPlayer()` 只能从当前位于同一场景中的玩家获取数据；他们不一定必须在可视范围内，但应连接到同一个通信岛。要在预览中尝试这一点，请打开第二个标签页并使用不同账户登录，然后让两个玩家都站在场景中。


> [!WARNING]
> **📔 注意**：用户 ID 必须始终小写。如果复制钱包地址，请确保所有字符都为小写。


## 来自任意玩家的数据

要获取任意玩家的信息，请进行一次 [REST API 调用](/docs/creator/content-creator-zh/chang-jing-sdk7/lian-wang/network-connections/#call-a-rest-api) 到内容服务器。

该信息通过以下 URL 暴露，只需将玩家的用户 ID 追加到该 URL 参数后面。

`https://peer.decentraland.org/lambdas/profile/<player user id>`


> [!NOTE]
> **💡 提示**：在浏览器中试试这个 URL，看看响应的结构。


该 API 可提供以下信息：

* `displayName`: *(字符串)* 玩家用户名，其他人在世界中看到的名称
* `userId`: *(字符串)* 标识玩家的 UUID 字符串。如果玩家有公钥，则此字段的值与公钥相同。
* `hasConnectedWeb3`: *(布尔值)* 表示玩家是否拥有公钥。 *真* 如果玩家拥有，则为真。
* `publicKey`: *(字符串)* 玩家以太坊钱包的公钥。如果玩家作为访客登录，且未关联钱包，则此字段将为 `null`.
* `avatar`：一个嵌套对象，包含关于玩家外观的数据。
* `版本`: *(数字)* 一个版本号，每当玩家更改任何设置时都会加 1。若遇到冲突数据，可用它来判断哪个版本更新。


> [!WARNING]
> **📔 注意**：对于与该玩家的任何以太坊交易，始终使用 `publicKey` 字段，而不是 `userId`，以避免处理不存在的钱包。


该 `avatar` 对象包含以下嵌套信息：

* `穿戴物品`: `WearableId[]` 玩家当前穿戴的每个穿戴物品标识符数组。例如 `urn:decentraland:off-chain:base-avatars:green_hoodie`。所有穿戴物品都有类似的标识符，即使它们是 NFT。
* `bodyShape`：头像整体体型的标识符。可以是 `urn:decentraland:off-chain:base-avatars:BaseFemale` 表示女性，或 `urn:decentraland:off-chain:base-avatars:BaseMale` 表示男性。
* `皮肤`：一个包含 `颜色` 字段，以 `{ r, g, b, a }` 0 到 1 之间的数字表示玩家的肤色。
* `头发`：一个包含 `颜色` 字段，以 `{ r, g, b, a }` 0 到 1 之间的数字表示玩家的肤色。
* `眼睛`：一个包含 `颜色` 字段，以 `{ r, g, b, a }` 0 到 1 之间的数字表示玩家的肤色。
* `快照`：一个嵌套对象，包含不同分辨率下玩家 .jpg 图像的 base64 表示。
  * `face256`: *字符串* 玩家面部的 256x256 像素图像。
  * `身体`: *字符串* 玩家笔直站立时的全分辨率图像，尺寸为 512x1024 像素。


> [!CAUTION]
> **❗警告** 头像快照将在未来弃用，并且不再作为头像数据的一部分返回。推荐的做法是使用 `AvatarTexture` 代替，参见 [头像](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/materials/#avatar-portraits).


不同于 `getPlayer()`，此选项不仅限于当前在同一场景中的玩家，甚至不限于同一服务器。使用这种方式，你可以获取过去曾登录过服务器的任意玩家的数据。

如果你知道要查询的玩家连接的是哪个服务器，你可以将请求发送到该特定服务器，从而获得更实时的数据。例如，如果玩家更换衣服，这些信息会立即出现在玩家所在的服务器中，但很可能需要几分钟才能传播到 `peer.decentraland.org` 服务器。

`https://<player server>/lambdas/profile/<player user id>`


> [!NOTE]
> **💡 提示**：你可以通过调用 `getRealm()` 并读取 `realmInfo.baseUrl` 响应中的 realmInfo.baseUrl 字段来获取当前玩家所在的服务器。


这个示例结合了 `myProfile.userId` 和 `getRealm()` ，直接从玩家所在的服务器获取玩家数据：

```ts
import { getRealm } from '~system/Runtime'
import { myProfile } from '@dcl/sdk/network'

async function fetchPlayerData() {
	const { realmInfo } = await getRealm({})
	if (!realmInfo) return

	const url = `${realmInfo.baseUrl}/lambdas/profile/${myProfile.userId}`
	console.log('使用的 URL: ', url)

	try {
		const response = await fetch(url)
		const json = await response.json()

		console.log('完整响应: ', json)
		console.log('玩家穿戴的是:', json.avatars[0].avatar.wearables)
	} catch {
		console.log('获取玩家数据时发生错误')
	}
}

fetchPlayerData()
```

## 玩家数据组件

不要使用 `getPlayer()`，你可以直接从一系列存储在每个玩家实体上的组件中读取数据。存在以下组件：

* `PlayerIdentityData`：存储玩家地址以及一个 `isGuest` 用于标记访客账户的属性。
* `AvatarBase`：存储基础头像的数据，包括：
  * `name`：玩家的名称。
  * `bodyShapeUrn`：对应于男性或女性体型的 ID。
  * `skinColor`：玩家的皮肤颜色，类型为 `Color3`
  * `eyesColor`：玩家的眼睛颜色，类型为 `Color3`
  * `hairColor`：玩家的头发颜色，类型为 `Color3`
  * `talking`: *true* 当玩家正在进行语音聊天时， *false* 当玩家静音时， *undefined* 当客户端报告其没有语音活动时。参见 [检测玩家何时说话](#detect-when-a-player-is-speaking).
* `AvatarEquippedData`：已装备的穿戴物品和表情动作列表。
  * `wearableUrns`：玩家当前装备的可穿戴物品列表。
  * `emoteUrns`：玩家当前在快捷轮中装备的表情列表。
* `AvatarEmoteCommand`：关于玩家当前正在播放的表情动作的信息。包括：
  * `emoteUrn`：玩家自进入场景以来最后播放的表情动作的 URN
  * `loop`：如果该表情动作正在循环播放，则为真
  * `timestamp`：触发表情动作的时间

```ts
for (const [entity, data, base, attach, transform] of engine.getEntitiesWith(
	PlayerIdentityData,
	AvatarBase,
	AvatarEquippedData,
	Transform
)) {
	console.log('玩家数据: ', { entity, data, transform, base, attach })
}
```


> [!WARNING]
> **📔 注意**：所有这些组件均为只读。你不能在场景中更改它们的值。


## 检测玩家何时说话

阅读 `talking` 在 `AvatarBase` 组件上查看当前是否有人正在语音聊天。它适用于运行场景的玩家以及附近的其他每个玩家。

```ts
import { AvatarBase, engine } from '@dcl/sdk/ecs'

//每次本地玩家开始或停止说话时记录日志
AvatarBase.onChange(engine.PlayerEntity, (base) => {
	console.log('正在说话: ', base?.talking)
})
```

`talking` 有三种状态，而不是两种：

| 值           | 含义                             |
| ----------- | ------------------------------ |
| `true`      | 玩家此刻正在说话。                      |
| `false`     | 玩家没有在说话。                       |
| `undefined` | 客户端没有报告该玩家的语音活动，例如因为附近语音聊天已关闭。 |

第三种状态很重要： `undefined` 表示“没有数据”，而不是“静默”。请显式检查它，而不是把任何非`true` 值都当作静默。

```ts
import { AvatarBase, engine, PlayerIdentityData } from '@dcl/sdk/ecs'

function whoIsSpeaking() {
	for (const [player, base] of engine.getEntitiesWith(AvatarBase)) {
		if (base.talking === undefined) continue // 该玩家没有语音数据
		if (base.talking) console.log(`${base.name} 正在说话`)
	}
}

engine.addSystem(whoIsSpeaking)
```

要在正在说话的玩家头顶放置标记，请使用 [`AvatarAttach`](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/entity-positioning/#attach-to-other-players) ，并使用来自 `PlayerIdentityData`。不要把你的实体设为玩家实体的子实体：这样行不通，标记最终会出现在场景原点。


> [!WARNING]
> **📔 注意**: `AvatarBase.talking` 会报告真实的语音聊天活动，而且你不能写入它。它不同于 `AvatarShape.talking`，这是你的场景设置的一个标志，用于在 [NPC 头像](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/tou-xiang/npc-avatars/) 旁边绘制绿色语音条。


## 获取可携式体验

可携式体验本质上是不受土地地块限制的场景。玩家可以将它们带到在 Decentraland 中的任何地方，在世界之上增加一个新的内容层。智能穿戴物品就是可携式体验的例子。你可能想知道玩家是否穿戴了其中之一，因为智能穿戴物品可能让玩家拥有在竞技游戏中可被视为作弊的能力。例如，在平台游戏中，穿戴喷气背包的玩家相较于其他人有着非常不公平的优势。

作为场景创建者，你可能希望限制佩戴可携式体验的玩家在你的场景中能做什么。使用 `getPortableExperiencesLoaded()` 来检查玩家当前是否激活了任何可携式体验。

```ts
import { getPortableExperiencesLoaded } from '~system/PortableExperiences'

executeTask(async () => {
	let portableExperiences = await getPortableExperiencesLoaded({})
	console.log(portableExperiences.loaded)
})
```

`getPortableExperiencesLoaded()` 返回一个对象数组，其中每个对象都包含一个 `id` 属性。对于穿戴物品来说，id 是该穿戴物品的 URN。

## 获取玩家穿戴物品的详细信息

该 `getPlayer()` 函数只返回穿戴物品 ID 列表，而不包含每个穿戴物品的信息。也许你想检查特定类别的穿戴物品（例如：帽子），或特定稀有度的穿戴物品（例如：神话），那就需要获取关于玩家穿戴物品的更详细信息。

发起一次 [REST API 调用](/docs/creator/content-creator-zh/chang-jing-sdk7/lian-wang/network-connections/#call-a-rest-api) 对以下 URL 的请求，以获取当前可用的所有穿戴物品的完整更新列表，并附带每个物品的详细信息。

`${playerRealm.realmInfo.baseUrl}/lambdas/collections/wearables-by-owner/${userData.userId}?includeDefinitions`


> [!WARNING]
> **📔 注意**：要构造此 URL，你必须获取 realm（可能使用 with `getRealm()`）以及玩家的 ID（可能使用 `getPlayer()`)


这个功能可以与获取玩家信息结合使用，例如仅允许穿戴了万圣节系列中任意穿戴物品，或任何属于 *传奇* 稀有度的玩家进入某个地点。


> [!NOTE]
> **💡 提示**：在浏览器中试试这个 URL，看看响应的结构。


```ts
import { getPlayer } from '@dcl/sdk/src/players'
import { getRealm } from '~system/Runtime'

async function fetchWearablesData() {
	try {
		let userData = getPlayer()
		const realm = await getRealm({})
		if (!userData || !realm.realmInfo) return

		const url =
			`${realm.realmInfo?.baseUrl}/lambdas/collections/wearables-by-owner/${userData.userId}?includeDefinitions`.toString()
		console.log('使用的 URL: ', url)

		let response = await fetch(url)
		let json = await response.json()

		console.log('完整响应: ', json)
	} catch {
		console.log('获取穿戴物品数据时发生错误')
	}
}

executeTask(fetchWearablesData)
```


> [!NOTE]
> **💡 提示**：你还可以从 [以下 API 获取关于特定穿戴物品的更多信息](https://decentraland.github.io/catalyst-api-specs/#tag/Lambdas/operation/searchWearables).


## 检查玩家的摄像机模式

玩家在探索 Decentraland 时，可以使用第一人称或第三人称摄像机。通过检查 `CameraMode` 的组件值，来查看玩家正在使用哪一种。 `engine.CameraEntity` 实体上。

```ts
function checkCameraMode() {
	if (!CameraMode.has(engine.CameraEntity)) return

	let cameraEntity = CameraMode.get(engine.CameraEntity)

	if (cameraEntity.mode == CameraType.CT_THIRD_PERSON) {
		console.log('玩家正在使用第三人称摄像机')
	} else {
		console.log('玩家正在使用第一人称摄像机')
	}
}

engine.addSystem(checkCameraMode)
```


> [!WARNING]
> **📔 注意**：摄像机信息仅对当前运行场景的玩家可用。你不能查询其他玩家的摄像机数据。


摄像机模式使用来自 `CameraType` 枚举的值。可能的取值如下：

* `CameraType.CT_FIRST_PERSON`
* `CameraType.CT_THIRD_PERSON`

该 `CameraMode` 的组件值，来查看玩家正在使用哪一种。 `engine.CameraEntity` 为只读，你不能通过它强制玩家更改摄像机模式。


> [!NOTE]
> **💡 提示**：要更改玩家的摄像机模式，请使用一个 [摄像机修正区域](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/camera/#1st-and-3rd-person-camera-modes).


了解摄像机模式对微调场景机制非常有用，可更好地适配这种模式下更舒适的操作。例如，在第三人称下，小目标更难点击。


> [!WARNING]
> **📔 注意**：避免在 `engine.CameraEntity` 在初始场景加载时引用它，因为如果实体尚未初始化，可能会导致错误。为避免此问题，请在 `main()` 函数内使用这些内容，或在被 `main()`间接调用的函数中使用。你也可以将该行为封装在一个异步 [`executeTask` 块中。](/docs/creator/content-creator-zh/chang-jing-sdk7/bian-cheng-mo-shi/async-functions/#the-executetask-function).
> 
> 另一种选择是在系统中引用该实体。它总是可用，因为系统的第一次执行会在场景已正确初始化后触发。


## 检查玩家是否锁定了光标

玩家可以在两种光标模式之间切换： *锁定光标* 模式，用于控制相机；或 *解锁光标* 模式，用于在 UI 上自由移动光标。

玩家通过点击 *鼠标右键* 或按下 *Esc* 键来解锁光标，并通过点击屏幕任意位置重新锁定光标。

检查 `PointerLock` 场景的 [摄像机实体的](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/entities-components/#reserved-entities) 组件，以了解当前光标模式是什么。

```ts
export function main() {
	const isLocked = PointerLock.get(engine.CameraEntity).isPointerLocked
	console.log(isLocked)
}
```

参见 [事件监听器](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/event-listeners/#player-locks-or-unlocks-cursor) 以查看如何轻松响应光标状态变化。

你也可以通过写入 `isPointerLocked` 的字段来强制设置玩家的光标状态。 `PointerLock` 组件赋予 `engine.CameraEntity`。请参见 [锁定或解锁光标](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/click-events/#lock-or-unlock-the-cursor) 示例。


> [!WARNING]
> **📔 注意**：避免在 `engine.CameraEntity` 在初始场景加载时引用它，因为如果实体尚未初始化，可能会导致错误。为避免此问题，请在 `main()` 函数内使用这些内容，或在被 `main()`间接调用的函数中使用。你也可以将该行为封装在一个异步 [`executeTask` 块中。](/docs/creator/content-creator-zh/chang-jing-sdk7/bian-cheng-mo-shi/async-functions/#the-executetask-function).
> 
> 另一种选择是在系统中引用该实体。它总是可用，因为系统的第一次执行会在场景已正确初始化后触发。


## 检查玩家的光标位置

使用 `PrimaryPointerInfo` 组件来查询玩家的摄像机模式，位于 `engine.RootEntity` 以获取玩家的光标位置。这可用于拖放交互、滑动手势等机制。

```ts
import { PrimaryPointerInfo } from '@dcl/sdk/ecs'

function CursorSystem() {
	const pointerInfo = PrimaryPointerInfo.get(engine.RootEntity)
	console.log(pointerInfo)
}

engine.addSystem(CursorSystem)
```


> [!WARNING]
> **📔 注意**：避免在 `engine.RootEntity` 在初始场景加载时引用它，因为如果实体尚未初始化，可能会导致错误。为避免此问题，请始终在系统内引用该实体。它总是可用，因为系统的第一次执行会在场景已正确初始化后触发。


该 `PrimaryPointerInfo` 组件会返回一个包含以下属性的对象：

* `screenCoordinates`: *(Vector2)* 光标在场景中的位置，以像素表示。原点是屏幕左上角，且 `y` 向下增长——与 `UiTransform` 和 `UiCanvasInformation`相同的坐标空间。当光标被锁定时，这会报告屏幕中心。
* `screenDelta`: *(Vector2)* 光标自上一帧以来的位置变化，以像素表示。详见 [鼠标移动](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/mouse-movement/) 以获取详细信息和示例。
* `worldRayDirection`: *(Vector3)* 一个向量，表示从摄像机到光标的射线方向。原点是摄像机位置。可用它来计算光标在世界中的位置。
* `pointerType`：0 表示 `无`，1 表示 `鼠标`


> [!NOTE]
> **💡 提示**：与其他属性不同， `screenDelta` 即使光标处于 [锁定状态，它仍会持续报告鼠标移动。](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/click-events/#lock-or-unlock-the-cursor)。这使其非常适合拖拽手势和自定义摄像机控制等实时交互，参见 [鼠标移动](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/mouse-movement/).



> [!NOTE]
> **提示：** 要响应 UI 元素上的简单悬停事件，你可能会发现使用 `onMouseEnter` 和 `onMouseLeave` 事件，请参阅 [UI 按钮事件](/docs/creator/content-creator-zh/chang-jing-sdk7/2d-ui/ui_button_events/#hover-feedback).


该 `PrimaryPointerInfo` 组件是只读的，你无法强制玩家更改光标位置。

以下示例展示如何在 UI 元素上显示光标位置。

***ui.tsx 文件：***

```tsx
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'
import {cursorXpos, cursorYpos} from './index'

export const uiMenu = () => (
  <UiEntity
    uiTransform={{
			width: '100%',
			height: '100px',
			justifyContent: 'center',
			alignItems: 'center',
    }}
    uiText={{ value: `光标位置：`+  cursorXpos + `,` + cursorYpos, fontSize: 40 }}
    uiBackground={{ color: Color4.create(0.5, 0.8, 0.1, 0.6) }}
  />
)
```

***index.ts 文件：***

```ts
import { engine, PrimaryPointerInfo } from '@dcl/sdk/ecs'
import { ReactEcsRenderer } from '@dcl/sdk/react-ecs'
import { uiMenu } from './ui'

export function main() {
  ReactEcsRenderer.setUiRenderer(uiMenu)
}

export let cursorXpos: number | undefined = undefined
export let cursorYpos: number | undefined = undefined

function CursorSystem() {
  const pointerInfo = PrimaryPointerInfo.get(engine.RootEntity)
  console.log(pointerInfo)

  cursorXpos = pointerInfo.screenCoordinates?.x
  cursorYpos = pointerInfo.screenCoordinates?.y
}

engine.addSystem(CursorSystem)
```

你可以使用 `worldRayDirection` 用于设置 `direction` 射线投射的字段，以判断某个实体是否位于光标的视线中。请参阅 [射线投射](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/raycasting/) 了解更多详情。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/user-data.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
