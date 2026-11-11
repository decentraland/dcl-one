# 受限操作

该 `RestrictedActions` 模块允许场景访问敏感（因此受限）的功能。它与 [`权限`](/docs/contributor/contributor-zh/nei-rong/shi-ti-lei-xing/scenes/#permissions) 系统相连，场景使用该系统来请求使用各个方法。

```ts
const RestrictedActions = require("~system/RestrictedActions");
```


> [!NOTE]
> 作为 World Explorer 的实现者，你的运行时可能完全不执行任何权限校验。我们 **强烈** 建议不要这样做，因为这会让玩家处于危险之中。


大多数受限功能都由此模块提供，但也有受限的 [全局函数](/docs/contributor/contributor-zh/chang-jing-yun-xing-shi/globals/).

该模块包含以下方法和类型：

* [`函数 movePlayerTo`](#moveplayerto)
* [`函数 teleportTo`](#teleportto)
* [`函数 triggerEmote`](#triggeremote)
* [`函数 changeRealm`](#changerealm)
* [`函数 openExternalUrl`](#openexternalurl)
* [`函数 openNftDialog`](#opennftdialog)
* [`函数 openExplorerUi`](#openexplorerui)
* [`函数 stopEmote`](#stopemote)
* [`函数 triggerSceneEmote`](#triggersceneemote)
* [`函数 copyToClipboard`](#copytoclipboard)
* [`函数 setCommunicationsAdapter`](#setcommunicationsadapter)
* [`接口 Vector3`](#vector3)

### 方法

下面的每个方法都关联着一个特定的 [权限](/docs/contributor/contributor-zh/nei-rong/shi-ti-lei-xing/scenes/#permissions) ，可在场景清单中请求。

### `movePlayerTo` <a href="#moveplayerto" id="moveplayerto"></a>

将玩家移动到相对于当前位置的新位置，并可选地使用 [向量](#vector3).

需要 [`ALLOW_TO_MOVE_PLAYER_INSIDE_SCENE`](/docs/contributor/contributor-zh/nei-rong/shi-ti-lei-xing/scenes/#permissions) 权限。

```ts
interface Request {
  newRelativePosition: Vector3;
  cameraTarget?: Vector3;
}

interface Response {
    // 一旦玩家移动到新位置，并且玩家没有中断移动，此值将为 true。
  success: boolean;
}

function movePlayerTo(Request): Promise<Response>;
```

### `teleportTo` <a href="#teleportto" id="teleportto"></a>

将玩家重新定位到一个地块，可选地位于另一个 realm 中。

不同于需要预先批准的权限，对 `teleportTo` 的每次调用都必须由玩家批准。

```ts
interface Request {
  // 要落地的地块。省略：该 realm 的默认出生点
  //（仅与 `realm` 一起使用时有意义）。
  worldCoordinates?: Vector2;

  // 该地块所属的 realm：一个世界名称（`foo.dcl.eth`）或一个 realm URL。
  // 设置后，客户端会切换 realm（一次完整重连，即使是切换到玩家
  // 已经所在的 realm），然后降落到那里的地块上。
  // 省略：该地块位于玩家当前的 realm 中。
  realm?: string;
}

interface Response {}

function teleportTo(Request): Promise<Response>;
```

### `triggerEmote` <a href="#triggeremote" id="triggeremote"></a>

使用预定义名称之一，让玩家的头像播放一个表情动作动画。

需要 [`ALLOW_TO_TRIGGER_AVATAR_EMOTE`](/docs/contributor/contributor-zh/nei-rong/shi-ti-lei-xing/scenes/#permissions) 权限。

```ts
interface Request {
  predefinedEmote: string;
}

interface Response {}

function triggerEmote(Request): Promise<Response>;
```

### `openExternalUrl` <a href="#openexternalurl" id="openexternalurl"></a>

使用合适的界面（也可能是另一个应用）向玩家显示一个网站。

需要 [`OPEN_EXTERNAL_LINK`](/docs/contributor/contributor-zh/nei-rong/shi-ti-lei-xing/scenes/#permissions) 权限。

```ts
interface Request {
  url: string;
}

interface Response {
  // 玩家是否授权打开链接。
  success: boolean;
}

function openExternalUrl(Request): Promise<Response>;
```

### `openNftDialog` <a href="#opennftdialog" id="opennftdialog"></a>

使用合适的界面向玩家显示有关 NFT 的信息。

```ts
interface Request {
  // NFT 的 URN。
  urn: string;
}

interface Response {
  // NFT 是否已成功定位并指向一张图片或视频。
  success: boolean;
}

function openNftDialog(Request): Promise<Response>;
```

### `openExplorerUi` <a href="#openexplorerui" id="openexplorerui"></a>

打开一个 explorer 面板，全屏或模态，并返回打开结果。

```ts
interface Request {
  ui: ExplorerUi;

  // 会回传到此请求产生的事件中，以便它们可以与
  // 其调用匹配。未设置表示不进行关联，0 保留用于结果上的相同
  // 含义，因此 id 从 1 开始。
  requestId?: number;

  // 仅用于 `EU_ITEM_PURCHASE`。
  itemPurchase?: { urn: string };
}

interface Response {
  // 仅携带打开动作的结果。
  openResult: OpenExplorerUiResult;
}

function openExplorerUi(Request): Promise<Response>;
```

### `stopEmote` <a href="#stopemote" id="stopemote"></a>

停止玩家头像当前正在播放的表情动作。

```ts
interface Request {}

interface Response {
  success: boolean;
}

function stopEmote(Request): Promise<Response>;
```

### `triggerSceneEmote` <a href="#triggersceneemote" id="triggersceneemote"></a>

让玩家的头像播放场景中文件里的表情动作动画。

```ts
interface Request {
  // 场景内表情动作文件的路径。
  src: string;

  loop?: boolean;
  mask?: AvatarMask;
}

interface Response {
  success: boolean;
}

function triggerSceneEmote(Request): Promise<Response>;
```

### `copyToClipboard` <a href="#copytoclipboard" id="copytoclipboard"></a>

将提供的文本复制到玩家的剪贴板中。

```ts
interface Request {
  text: string;
}

interface Response {}

function copyToClipboard(Request): Promise<Response>;
```

### `setCommunicationsAdapter` <a href="#setcommunicationsadapter" id="setcommunicationsadapter"></a>

要求 World Explorer 连接到另一个通信适配器，场景使用它来加入私有游戏服务器。

```ts
interface Request {
  connectionString: string;
}

interface Response {
  success: boolean;
}

function setCommunicationsAdapter(Request): Promise<Response>;
```

### `changeRealm` （已弃用） <a href="#changerealm" id="changerealm"></a>

使用其基础 URL 将 World Explorer 切换到另一个内容服务器。

已弃用，改用 [`teleportTo`](#teleportto) 并配合 `realm` （且不使用 `worldCoordinates` 作为该 realm 的默认出生点）。 `changeRealm` 在更改被接受后就会解析，而不是等到新 realm 变为可用之后，因此紧接着执行的 `teleportTo` 会在旧 realm 上运行。

```ts
interface Request {
  // 新 realm 的 URL。
  realm: string;

  // 当用户需要批准更改时显示给他们的一条可选消息。
  message?: string;
}

interface Response {
  // 该 realm 更改是否已获授权并成功。
  success: boolean;
}

function changeRealm(Request): Promise<Response>;
```

### 类型

此模块中方法使用的唯一附加类型是 `Vector3`.

### `Vector3` <a href="#vector3" id="vector3"></a>

保存一个相对或绝对的 3D 位置。

```ts
interface Vector3 {
  x: number;
  y: number;
  z: number;
}
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/chang-jing-yun-xing-shi/yun-xing-shi-mo-kuai/restricted-actions.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
