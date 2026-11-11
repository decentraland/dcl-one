# 제한된 작업

해당 `RestrictedActions` 모듈은 씬이 민감한(따라서 제한된) 기능에 접근할 수 있게 합니다. 이는 다음과 연결됩니다: [`권한`](/docs/contributor/contributor-ko/content/entity-types/scenes/#permissions) 시스템이며, 씬은 이를 사용해 개별 메서드의 사용을 요청합니다.

```ts
const RestrictedActions = require("~system/RestrictedActions");
```


> [!NOTE]
> World Explorer 구현자로서, 런타임에 권한 강제가 없을 수도 있습니다. 우리는 **강력히** 이것을 강력히 권장하지 않습니다. 플레이어를 위험에 빠뜨리기 때문입니다.


제한된 기능의 대부분은 이 모듈에서 제공되지만, 제한된 [전역 함수](/docs/contributor/contributor-ko/scene-runtime/globals/).

이 모듈에는 다음 메서드와 타입이 포함됩니다:

* [`function movePlayerTo`](#moveplayerto)
* [`function teleportTo`](#teleportto)
* [`function triggerEmote`](#triggeremote)
* [`function changeRealm`](#changerealm)
* [`function openExternalUrl`](#openexternalurl)
* [`function openNftDialog`](#opennftdialog)
* [`function openExplorerUi`](#openexplorerui)
* [`function stopEmote`](#stopemote)
* [`function triggerSceneEmote`](#triggersceneemote)
* [`function copyToClipboard`](#copytoclipboard)
* [`function setCommunicationsAdapter`](#setcommunicationsadapter)
* [`인터페이스 Vector3`](#vector3)

### 메서드

아래의 각 메서드는 특정 [권한](/docs/contributor/contributor-ko/content/entity-types/scenes/#permissions) 씬 매니페스트에서 요청할 수 있는 특정 권한입니다.

### `movePlayerTo` <a href="#moveplayerto" id="moveplayerto"></a>

플레이어를 현재 위치를 기준으로 새로운 위치로 이동시키고, 필요하면 카메라 타깃을 다음과 함께 설정합니다: [벡터](#vector3).

다음 권한이 필요합니다: [`ALLOW_TO_MOVE_PLAYER_INSIDE_SCENE`](/docs/contributor/contributor-ko/content/entity-types/scenes/#permissions) 권한.

```ts
interface Request {
  newRelativePosition: Vector3;
  cameraTarget?: Vector3;
}

interface Response {
    // 플레이어가 새 위치로 이동을 마치고, 플레이어가 이동을 중단하지 않았다면 true입니다.
  success: boolean;
}

function movePlayerTo(Request): Promise<Response>;
```

### `teleportTo` <a href="#teleportto" id="teleportto"></a>

플레이어를 parcel로 재배치하며, 필요하면 다른 realm에 배치할 수 있습니다.

미리 승인된 권한을 요구하는 대신, 각 `teleportTo` 호출은 플레이어의 승인을 받아야 합니다.

```ts
interface Request {
  // 착지할 parcel. 생략 시: realm의 기본 스폰
  // (realm과 함께 사용할 때만 의미가 있습니다).
  worldCoordinates?: Vector2;

  // parcel이 속한 realm: 월드 이름(`foo.dcl.eth`) 또는 realm URL.
  // 설정하면 클라이언트가 realm을 변경하고(이미 플레이어가 있는 realm이라도 전체 재연결), 그곳의 parcel에 착지합니다.
  // 생략 시: parcel은 플레이어의 현재 realm에 있습니다.
  realm?: string;
  realm?: string;
}

interface Response {}

function teleportTo(Request): Promise<Response>;
```

### `triggerEmote` <a href="#triggeremote" id="triggeremote"></a>

플레이어의 아바타가 미리 정의된 이름 중 하나를 사용해 이모트 애니메이션을 표시하게 합니다.

다음 권한이 필요합니다: [`ALLOW_TO_TRIGGER_AVATAR_EMOTE`](/docs/contributor/contributor-ko/content/entity-types/scenes/#permissions) 권한.

```ts
interface Request {
  predefinedEmote: string;
}

interface Response {}

function triggerEmote(Request): Promise<Response>;
```

### `openExternalUrl` <a href="#openexternalurl" id="openexternalurl"></a>

적절한 UI(다른 애플리케이션일 수도 있음)를 사용해 웹사이트를 플레이어에게 표시하도록 제안합니다.

다음 권한이 필요합니다: [`OPEN_EXTERNAL_LINK`](/docs/contributor/contributor-ko/content/entity-types/scenes/#permissions) 권한.

```ts
interface Request {
  url: string;
}

interface Response {
  // 플레이어가 링크 열기를 승인했는지 여부.
  success: boolean;
}

function openExternalUrl(Request): Promise<Response>;
```

### `openNftDialog` <a href="#opennftdialog" id="opennftdialog"></a>

적절한 UI를 사용해 플레이어에게 NFT 정보를 표시합니다.

```ts
interface Request {
  // NFT의 URN.
  urn: string;
}

interface Response {
  // NFT를 성공적으로 찾았고 이미지 또는 비디오로 연결되었는지 여부.
  success: boolean;
}

function openNftDialog(Request): Promise<Response>;
```

### `openExplorerUi` <a href="#openexplorerui" id="openexplorerui"></a>

탐색기 패널을 전체 화면 또는 모달로 열고, 열기 판정 결과를 반환합니다.

```ts
interface Request {
  ui: ExplorerUi;

  // 이 요청이 생성하는 이벤트에서 다시 전달되어, 해당 호출과 매칭할 수 있습니다.
  // 설정되지 않으면 상관관계가 없음을 의미하며, 0은 결과에서도 같은 의미로 예약되어 있으므로 ID는 1부터 시작합니다.
  // 동일한 의미로 예약되어 있으므로, ID는 1부터 시작합니다.
  requestId?: number;

  // `EU_ITEM_PURCHASE`에만 해당합니다.
  itemPurchase?: { urn: string };
}

interface Response {
  // 열기 작업의 판정 결과만 전달합니다.
  openResult: OpenExplorerUiResult;
}

function openExplorerUi(Request): Promise<Response>;
```

### `stopEmote` <a href="#stopemote" id="stopemote"></a>

플레이어 아바타가 현재 재생 중인 이모트를 중지합니다.

```ts
interface Request {}

interface Response {
  success: boolean;
}

function stopEmote(Request): Promise<Response>;
```

### `triggerSceneEmote` <a href="#triggersceneemote" id="triggersceneemote"></a>

플레이어의 아바타가 씬의 파일에서 가져온 이모트 애니메이션을 표시하게 합니다.

```ts
interface Request {
  // 씬 내 이모트 파일의 경로.
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

제공된 텍스트를 플레이어의 클립보드에 복사합니다.

```ts
interface Request {
  text: string;
}

interface Response {}

function copyToClipboard(Request): Promise<Response>;
```

### `setCommunicationsAdapter` <a href="#setcommunicationsadapter" id="setcommunicationsadapter"></a>

World Explorer에 다른 통신 어댑터에 연결하도록 요청합니다. 씬은 이를 사용해 private game server에 참여합니다.

```ts
interface Request {
  connectionString: string;
}

interface Response {
  success: boolean;
}

function setCommunicationsAdapter(Request): Promise<Response>;
```

### `changeRealm` (사용되지 않음) <a href="#changerealm" id="changerealm"></a>

기본 URL을 사용해 World Explorer를 다른 콘텐츠 서버로 전환합니다.

다음으로 대체되어 사용되지 않습니다 [`teleportTo`](#teleportto) 다음이 있는 `realm` (그리고 `worldCoordinates` realm의 기본 스폰 위치는 없음). `changeRealm` 변경이 승인되면 완료되며, 새 realm이 활성화되기 전이므로 `teleportTo` 바로 직후에 발행된 요청은 이전 realm을 대상으로 동작합니다.

```ts
interface Request {
  // 새 realm의 URL.
  realm: string;

  // 사용자가 변경을 승인해야 할 때 보여줄 선택적 메시지.
  message?: string;
}

interface Response {
  // realm 변경이 승인되었고 성공했는지 여부.
  success: boolean;
}

function changeRealm(Request): Promise<Response>;
```

### 타입

이 모듈의 메서드에서 사용되는 유일한 추가 타입은 `Vector3`.

### `Vector3` <a href="#vector3" id="vector3"></a>

상대적 또는 절대적인 3D 위치를 나타냅니다.

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
GET https://docs.decentraland.org/contributor/contributor-ko/scene-runtime/runtime-modules/restricted-actions.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
