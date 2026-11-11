> 创建具有交互能力的可穿戴物

# 智能可穿戴物

智能穿戴物是一种全局场景。比如 [可携式体验](/docs/creator/content-creator-zh/chang-jing-sdk7/xiang-mu-lei-xing/portable-experiences/)，它们是玩家在元宇宙中移动时随身携带的玩法。例如，在运行一个全局场景时，玩家可以从 Genesis Plaza 的地面上拿起一个雪球，走到另一个场景，然后把雪球扔给另一位也在玩同一游戏的玩家。

智能穿戴物是在玩家穿上某件特定服装后启动的可携式体验。智能穿戴物可以赋予玩家新能力，比如让玩家飞行的喷气背包，或者在整个 Genesis City 上随机放置可收集金币，在世界的其余部分之上增加一层新的内容。


> [!WARNING]
> **📔 注**：智能穿戴物只能使用 SDK 7 创建。
> 
> 该 **Creator Hub** 目前不支持创建智能穿戴物项目。


## 入门

### 使用 CLI

1. 在一个新文件夹中打开命令行并运行

`npx @dcl/sdk-commands init --project smart-wearable`

该命令会为一个新的智能穿戴物创建基础文件和结构。

## 模板中的文件

一个全新的智能穿戴物项目的文件夹与一个 [Decentraland 场景](/docs/creator/content-creator-zh/chang-jing-sdk7/xiang-mu-lei-xing/scene-files/)的文件夹非常相似，但你会注意到以下区别：

* `wearable.json` 包含可携式体验的所有元数据
* 有一个暗色眼镜的占位 3D 模型（glasses.glb）和缩略图（glasses.png）。你应该把它们替换成你正在创建的实际穿戴物
* `scene.json` 包含 `"isPortableExperience": true`。地块与已发布的穿戴物无关，它可以在任何地方工作，但你可以为本地测试设置它们：预览只会在 `scene.json`

## 关于 wearable.json

默认的 `wearable.json` 文件如下：

```json
{
	"data": {
		"replaces": [],
		"hides": [],
		"tags": ["special", "new", "eyebrows"],
		"representations": [
			{
				"bodyShapes": [
					"urn:decentraland:off-chain:base-avatars:BaseMale",
					"urn:decentraland:off-chain:base-avatars:BaseFemale"
				],
				"mainFile": "glasses.glb",
				"contents": ["glasses.glb"],
				"overrideHides": [],
				"overrideReplaces": []
			}
		],
		"category": "eyewear"
	},
	"name": "Smart Wearable Example",
	"description": "Put the glasses to see a new world",
	"rarity": "mythic"
}
```

这些是 `wearable.json`:

* `id`：智能穿戴物的唯一 ID。
* `名称` （必填）：用户在市场中会看到的穿戴物名称
* `描述`：用户在市场中会看到的穿戴物描述。请务必说明该智能穿戴物能做什么，因为市场用户在购买前无法预览其功能。最多可为 **64 个字符** 长。上传后，你可以在 Builder 的物品编辑器中写更长的描述。
* `稀有度`：代币的稀有度供应量。可取值为：
  * unique（1 份）
  * mythic（10 份）
  * exotic（50 份）
  * legendary（100 份）
  * epic（1000 份）
  * rare（5000 份）
  * uncommon（10,000 份）
  * common（100,000 份）


> [!WARNING]
> **📔 注**：如果你是从已有项目分叉而来，并且它包含一个 `id`，在发布你的穿戴物之前请确保该值是唯一的。使用 [uuidgenerator.net](https://www.uuidgenerator.net/) 生成一个新的随机 UUID


* `data` （必填）：包含以下内容。 `替代`, `hides`, `标签`, `representations` 和 `category` 都必须存在。 `替代`, `hides` 和 `标签` 可以是空列表 `[]`，但 `representations` 至少需要一个条目。你也可以在上传智能穿戴物后，从 Builder UI 中调整这些设置。
  * `替代`：除本类别的默认值外，还列出穿戴上此穿戴物时应同时卸下的其他穿戴物类别列表。例如：穿上上身披风时，也要隐藏脚部。
  * `hides`：除本类别的默认值外，还列出穿戴上此穿戴物时应被隐藏（但不卸下）的其他穿戴物类别列表。
  * `标签`：用于让该穿戴物在市场中可搜索的标签。
  * `representations`:
    * `bodyShapes`：可使用此穿戴物的头像形态列表。所有智能穿戴物都必须同时适用于男性和女性体型。
    * `mainFile`：包含该穿戴物 3D 模型的主文件。
    * `contents`：用于渲染该穿戴物 3D 模型的完整文件列表。例如，3D 模型可能包含单独的纹理文件。
    * `overrideHides`：该穿戴物类别默认 *隐藏* 行为的任何例外。
    * `overrideReplaces`：该穿戴物类别默认 *替换* 行为的任何例外。
  * `category`：使用哪种穿戴物类别。可取值为：
    * 'eyebrows'
    * 'eyes'
    * 'facial\_hair'
    * 'hair'
    * 'mouth'
    * 'upper\_body'
    * 'lower\_body'
    * 'feet'
    * 'earring'
    * 'eyewear'
    * 'hat'
    * 'helmet'
    * 'mask'
    * 'tiara'
    * 'top\_head'
    * 'skin'
    * 'hands\_wear'


> [!WARNING]
> **📔 注**：除了上面的字段外，Builder 还接受一个可选的顶层 `映射` 字段，以及可选的 `removesDefaultHiding` 和 `blockVrmExport` 字段，位于 `data`中。智能穿戴物通常不需要它们。Builder 会拒绝一个 `wearable.json` 含有任何其他顶层字段的对象，并报错 *“the wearable config file is invalid”*。旧版本的此页面列出了 `menuBarIcon`, `model`, `bodyShape` 和 `collectionAddress`：如果你的项目仍然包含它们，请将其移除。如果 `描述` 超过 64 个字符，也会出现相同错误。


## 缩略图

你必须在文件夹根目录中包含一个名为 `thumbnail.png` 的图片。该图片会在背包和市场中显示，用于以 2D 形式表示你的穿戴物。推荐的必需图片尺寸为 256x256。

请选择一张能设定玩家预期并恰当代表你的作品的图片。

## 预览

运行智能穿戴物的预览与运行场景的预览很相似：在 Creator Hub 中点击 **运行** ，或者在命令行中运行 `npm run start` 。Creator Hub 不能创建智能穿戴物项目，但可以打开并预览它。

预览会把你的穿戴物代码作为普通场景在 `scene.json`中列出的地块上运行，而不是作为可携式体验。这意味着某些行为会与已发布的穿戴物不同：

* 只在玩家所在场景中生效的代码，例如 [对玩家的物理作用力](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/player-physics/)，在你走出预览地块后就会停止工作。一旦发布，智能穿戴物会在玩家去往任何地方时持续工作。
* 穿戴物的 3D 模型可能不会显示在你的头像上。

要尝试让穿戴物被装备并在任何地方工作，请将其上传到 Builder 并使用 **在 Decentraland 中查看** （见 [发布](#publish)）。无需铸造或发布该合集即可完成。

要测试智能穿戴物在场景上下文中的行为，你还可以使用一个 [工作区](/docs/creator/content-creator-zh/chang-jing-sdk7/xiang-mu-lei-xing/workspaces/)，在运行一个或多个场景预览的同时运行你的穿戴物预览。例如，你可以将智能穿戴物与 [Genesis Plaza](https://github.com/decentraland-scenes/Genesis-Plaza) 场景一起运行，以测试它在繁忙场景、乘电梯等情况下的表现。

## 提示

* 放置实体时，请注意位置是全局的，相对于 Genesis Plaza 的 0,0 坐标。
* 要响应附近玩家：
  * 参见 [获取所有玩家](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/user-data/#fetch-all-players) ，了解如何从周围其他玩家那里获取数据。
  * 请注意，智能穿戴物、周围场景以及其他玩家的加载顺序可能会因情况不同而有所变化。如果玩家在已经穿上智能穿戴物的情况下进入 Decentraland，那么你的穿戴物的全局场景很可能会先于其他玩家加载。另一方面，如果玩家先加载进入某个场景，然后再穿上穿戴物，那么当穿戴物场景开始运行时，其他玩家很可能已经加载完成。
  * 只有当玩家连接到其领域中的一个岛屿后，其他玩家才会加载。获取领域数据并检查‘room’字段。如果‘room’字段为 null，则玩家尚未连接到岛屿，其他玩家也不会加载。你可以每 1 秒定期检查一次，直到‘room’字段出现，然后再初始化你的逻辑。这样只会让你看到其他玩家：如需在穿戴者之间共享数据，参见 [限制](#limitations).
* 要与周围场景交互：
  * 你不能直接向附近场景或其他可携式体验发送任何指令， `messageBus` 当前对每个可携式体验/场景都是沙箱化的。
  * 你可以使用一个中间服务器在可携式体验和场景之间传递信息。
  * 如果你进行射线检测，可以检测到与周围场景中实体碰撞器的命中。这可以告诉你精确的命中位置、法线方向，甚至 3D 模型的实体名称和网格名称。只有当命中使用 SDK7 编写的场景中的实体时才有效。
* 终止一个可携式体验：运行 `kill()` 方法即可让一个可携式体验自行终止。
* 推动玩家移动： [物理](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/player-physics/) 像 `Physics.applyImpulseToPlayer()` 这样的函数可在穿戴者去往的任何地方工作，因为智能穿戴物的场景始终算作玩家所在的场景。不需要权限。场景可以为访客关闭智能穿戴物，见 [功能切换](/docs/creator/content-creator-zh/chang-jing-sdk7/xiang-mu-lei-xing/scene-metadata/#feature-toggles).
* 播放只有穿戴者能听到的声音：使用 [AvatarAttach](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/entity-positioning/#attach-an-entity-to-an-avatar) 将实体附加到玩家，并给它一个 `AudioSource`。其他玩家听不到。
* 在文件需要之前就加载它们：使用 [AssetLoad](/docs/creator/content-creator-zh/chang-jing-sdk7/you-hua/pre-load-resources/) 组件在穿戴物启动时获取声音、模型或表情文件，这样第一次使用时就不用等待下载。

## 发布

要发布你的智能穿戴物：

1. 确保 `wearable.json` 中的信息准确无误。如果你是以其他项目作为起点，请确保 `id` 是一个唯一标识符，没有被其他穿戴物使用。
2. 运行 `npm run pack` 在你的项目文件夹中运行。它会生成一个 `smart-wearable.zip` 文件在你的项目文件夹中。


> [!WARNING]
> **📔 注**： `smart-wearable.zip` 的未压缩内容总和必须少于 3MB，否则 Builder 会将该文件视为过大并拒绝上传。 `npm run pack` 当项目超过 2MB 时会打印大小提示，但它仍然会创建 zip。请检查 zip 中各文件的大小，而不是 zip 本身的大小。见 [限制](#limitations).


3. 打开 Builder，打开 Collections 选项卡，点击 + 上传一个新的穿戴物。
4. 将你压缩后的 `smart-wearable.zip` 文件拖入 Builder，并验证所有信息都正确。

> 注意：如果你的穿戴物为每种体型都有不同的 3D 模型，请按每种体型列出一个 representation：
>
> 1. 在你的项目中，为每个 representation 新建一个文件夹（`male` 和 `female`），并把每个 representation 的 3D 模型放到对应的文件夹中。
> 2. 更新你的 `wearable.json` 文件以包含新的 representations。
> 3. ```lang-json
>    "representations": [{
>      "bodyShapes": ["urn:decentraland:off-chain:base-avatars:BaseMale"],
>      "mainFile": "male/glasses.glb",
>      "contents": ["male/glasses.glb"],
>      "overrideHides": [],
>      "overrideReplaces": []
>    },
>    {
>      "bodyShapes": ["urn:decentraland:off-chain:base-avatars:BaseFemale"],
>      "mainFile": "female/glasses.glb",
>      "contents": ["female/glasses.glb"],
>      "overrideHides": [],
>      "overrideReplaces": []
>    }],
>    ```
> 4. 运行 `npm run pack` 以生成一个新的 smart-wearable.zip 文件。
> 5. 将新的 smart-wearable.zip 文件拖入 Builder。

5. 打开穿戴物编辑器，并确保“hide”和“remove”类别已正确设置，以便在此穿戴物穿上时禁用其他穿戴物类别。
6. 用这个以及可能的其他穿戴物创建一个新合集。
7. 在该合集上，点击 **在 Decentraland 中查看**。这会在 Sepolia 测试网络上打开 Decentraland，你可以从背包中装备该合集的穿戴物，并查看它们在更真实场景中的表现，例如在 Genesis Plaza 周围跑动。见 [在 Decentraland 中测试](/docs/creator/content-creator-zh/chuan-dai-wu-yu-biao-qing-dong-zuo/guan-li-he-ji/uploading-smart-wearables/#testing-in-decentraland).
8. 到这里，你的穿戴物已经准备好发布了。

## 权限

智能穿戴物以受限权限运行。要使用以下任一功能，请在你的 `requiredPermissions` 列表中声明相应权限， `scene.json`否则该调用在运行时会失败：

* `USE_FETCH`：使用 `fetch` 或 `signedFetch`
* `USE_WEBSOCKET`：WebSocket 连接
* `USE_WEB3_API`：钱包交易或消息签名
* `OPEN_EXTERNAL_LINK`：提示玩家打开外部链接

```json
"requiredPermissions": [
    "USE_FETCH"
  ],
```

当智能穿戴物声明了其中任何一项时，玩家在装备它时会看到授权提示，如果他们拒绝，穿戴物就不会运行。只声明你使用的权限。见 [所需权限](/docs/creator/content-creator-zh/chang-jing-sdk7/xiang-mu-lei-xing/scene-metadata/#required-permissions) 了解详情。

## 限制

> 重要：整个智能穿戴物必须控制在 3MB 以内。这包括 3D 模型、缩略图、代码、库、声音文件、额外的 3D 模型、UI 图片等。此限制针对未压缩文件夹。Builder 不会允许你上传更大的穿戴物。要检查大小，请解压生成的 `smart-wearable.zip` 文件并检查文件夹大小，或者运行 `unzip -l smart-wearable.zip` 并读取总大小。

要将文件排除在 zip 之外，请像场景一样将它们列在项目的 `.dclignore` 文件中。


> [!WARNING]
> **📔 注**：如果你在 Creator Hub 中编辑智能穿戴物项目，它可以将 3D 视图的全尺寸截图保存为 `assets/Images/autogenerated-thumbnail.png` 并指向它。该图片本身可能有几 MB 大，并使穿戴物超出限制。智能穿戴物不会使用它，所以添加 `scene.json` 到 `assets/Images/autogenerated-thumbnail.png` 设为 `.dclignore`.


智能穿戴物只会为穿戴该穿戴物的玩家运行可携式体验。其他玩家看不到这些效果。例如，如果可携式体验渲染了一个跟随玩家的宠物，周围的其他玩家就看不到这只宠物。不过，其他玩家会看到作为穿戴物场景一部分运行的头像动画，即使 [自定义头像动画](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/tou-xiang/avatar-animations/#custom-animations) 是作为智能穿戴物文件的一部分上传的。

智能穿戴物没有内置多人功能。每个佩戴该穿戴物的玩家都会运行其代码的独立副本，即使两个穿戴者并肩站立也是如此。 `MessageBus`，使用 `syncEntity()` 以及 [多人服务器](/docs/creator/content-creator-zh/chang-jing-sdk7/lian-wang/authoritative-servers/) 同步实体的功能在智能穿戴物中不起作用。要在穿戴者之间共享数据，比如共享分数，唯一方式是使用你自己的服务器，并让穿戴物通过 `fetch` 或 WebSocket 与其通信，见 [网络连接](/docs/creator/content-creator-zh/chang-jing-sdk7/lian-wang/network-connections/).

自定义动画可以包含道具，而道具可以包含由动画骨骼制作的粒子效果（见 [道具和声音](/docs/creator/content-creator-zh/chuan-dai-wu-yu-biao-qing-dong-zuo/biao-qing-dong-zuo/props-and-sounds/) 和 [表情中的粒子](/docs/creator/content-creator-zh/chuan-dai-wu-yu-biao-qing-dong-zuo/biao-qing-dong-zuo/particles-in-emotes/)）。其他玩家也能看到这些。声音则不同：使用 `triggerSceneEmote()` 播放的自定义动画不会播放音频，即使表情文件自带音频。要播放声音，请使用一个 `AudioSource`，只有穿戴者能听到。

只有玩家穿上时，智能穿戴物才会工作。因此，玩家只能激活有限数量的智能穿戴物，而且根据它们占据身体的部位，有些会彼此不兼容。例如，你不能同时戴两个帽子，或同时戴头盔和头发。

如果某个穿戴物被其他已穿戴物“隐藏”但未被“移除”，那么即使对应穿戴物不可见，智能穿戴物仍然可以处于开启状态。

## 示例

[智能穿戴物示例](https://github.com/decentraland/smart-wearable-sample))


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/xiang-mu-lei-xing/smart-wearables.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
