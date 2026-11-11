> 씬이 실행되는 컨텍스트와 씬 자체에서 데이터를 가져옵니다.

# 런타임 데이터

## Decentraland 시간 가져오기

Decentraland는 낮/밤 주기를 따르며, 완료되는 데 2시간이 걸리므로 하루에 총 12번의 완전한 주기가 있습니다. 플레이어는 설정을 변경하여 특정한 고정 시간대를 경험할 수도 있습니다. 예를 들어 항상 오후 10시의 밤하늘로 Decentraland를 볼 수 있습니다. 이러한 이유로 Decentraland의 시간은 플레이어마다 다를 수 있습니다.

다음을 사용하세요 `getWorldTime()` 플레이어가 Decentraland 안에서 경험하는 시간을 가져옵니다.

```ts
import { getWorldTime } from '~system/Runtime'

executeTask(async () => {
  let time = await getWorldTime({})
  console.log(time.seconds)
})
```


> [!NOTE]
> **💡 팁**: `getWorldTime()` 함수는 비동기입니다. 다음을 참조하세요. [비동기 함수](/docs/creator/content-creator-ko/sdk7/programming-patterns/async-functions/) 잘 모른다면.


`getWorldTime()` 다음이 포함된 객체를 반환합니다. `seconds` 속성이 있습니다. 이 속성은 하루의 시작 이후 경과한 시간(Decentraland 시간 기준)을 초 단위로 나타냅니다. 전체 주기가 24시간이라고 가정하면, 초 값을 60으로 나누어 분을 얻고, 다시 60으로 나누어 하루가 시작된 이후 경과한 시간을 시 단위로 구합니다. 예를 들어, `seconds` 값이 *36000*이면, 이는 *오전 10시*.

Decentraland 시간에서 해는 항상 6:15에 뜨고 19:50에 집니다.

이 정보를 사용해 장면을 그에 맞게 바꿀 수 있습니다. 예를 들어 낮에는 새 소리를 재생하고 어두울 때는 귀뚜라미 소리를 재생하거나, 어두울 때 가로등의 발광 재질을 켜는 식입니다.

```ts
import { getWorldTime } from '~system/Runtime'

executeTask(async () => {
  let time = await getWorldTime({})
  console.log(time.seconds)
  if (time.seconds < 6.25 * 60 * 60 || time.seconds > 19.85 * 60 * 60) {
    // 밤 시간
    console.log('귀뚜라미 소리를 재생합니다')
  } else {
    // 낮 시간
    console.log('새 소리를 재생합니다')
  }
})
```

## 릴름 데이터 가져오기

Decentraland의 플레이어들은 여러 개의 별도 *영역에 존재하며*릴름에 존재합니다. 서로 다른 릴름의 플레이어들은 같은 parcel 위에 서 있어도 서로를 볼 수 없고, 상호작용하거나 대화할 수 없습니다. 이렇게 플레이어를 나누면 Decentraland가 제한에 걸리지 않고 무제한의 플레이어를 처리할 수 있습니다. 또한 서로 가까운 지역의 플레이어를 묶어, 상호작용하는 플레이어들 사이의 핑 시간이 허용 가능한 수준이 되도록 합니다.

씬이 다음으로 데이터를 전송한다면 [서드파티 서버](/docs/creator/content-creator-ko/sdk7/networking/third-party-servers/) 실시간으로 플레이어 간 변경 사항을 동기화하기 위해서는, 변경 사항이 같은 릴름에 있는 플레이어들 사이에서만 동기화되는 것이 중요한 경우가 많습니다. 하나의 릴름에 속한 모든 변경 사항은 다른 릴름의 변경 사항과 분리해서 처리해야 합니다. 그렇지 않으면 아무도 변경하지 않았는데도 플레이어들은 이상하게 무언가가 바뀌는 것을 보게 됩니다.

```ts
import { getRealm } from '~system/Runtime'

executeTask(async () => {
  const { realmInfo } = await getRealm({})
  if (!realmInfo) return
  console.log(`현재 릴름: `, realmInfo.realmName)
})
```


> [!NOTE]
> **💡 팁**: `getRealm()` 함수는 비동기입니다. 다음을 참조하세요. [비동기 함수](/docs/creator/content-creator-ko/sdk7/programming-patterns/async-functions/) 잘 모른다면.


Decentraland는 플레이어 위치, 채팅, MessageBus 메시지, 스마트 아이템 상태 변경을 포함한 플레이어 간 통신을 분산형 통신 서버 네트워크를 통해 처리합니다. 이러한 서버 각각은 **Realm**이라고 불리며, 각 서버는 여러 개의 별도 **룸** (라고도 함 **섬에 존재합니다**),을 지원할 수 있으며, 각 룸은 Decentraland 맵에서 서로 가까운 서로 다른 플레이어 집단을 묶습니다.

해당 `getRealm()` 함수는 다음 정보를 반환합니다:

* `baseUrl`: *(string)* 릴름 서버의 도메인
* `realmName`: *(string)* 릴름 서버의 이름
* `networkId`: *(number)* 이더리움 네트워크
* `commsAdapter`: *(string)* 쿼리 매개변수(자격 증명)를 모두 제거한 통신 어댑터
* `isPreview`: *(boolean)* 씬이 Decentraland에 게시된 것이 아니라 로컬 미리보기로 실행 중이면 true입니다.
* `isConnectedSceneRoom`: *(boolean)* 사용자가 씬 룸에 연결되어 있으면 true입니다.


> [!WARNING]
> **📔 참고**: `layer` 속성은 사용 중단되었으므로 피해야 합니다.


플레이어가 맵을 이동하면서, 이제 가장 가까운 플레이어들과 함께 묶이기 위해 다른 룸으로 이동할 수 있습니다. 룸은 관리 가능한 사람 수에 맞추기 위해 경계를 동적으로 조정하기도 하므로, 플레이어가 가만히 서 있어도 다른 플레이어들이 세계에 들어오고 나가면 그 플레이어는 다른 룸에 있게 될 수 있습니다. 같은 `room` 서버에 있는 플레이어들은 통신하며, 서로 보기에는 너무 멀리 떨어져 있어도 MessageBus를 통해 메시지를 주고받습니다. 같은 서버에 있지만 서로 다른 룸에 있는 플레이어들은 현재는 통신하지 않지만, 맵을 돌아다니며 룸이 바뀌면 통신하게 될 수 있습니다.

플레이어의 릴름 또는 룸과 관련된 변경 사항에 반응하려면 다음을 사용하세요. `onChange` 다음의 함수에서 `RealmInfo` 컴포넌트로, 엔진이 다음에 추가합니다. `engine.RootEntity`. 이 컴포넌트는 다음에서 반환되는 것과 동일한 필드를 포함합니다. `getRealm()`.

```ts
import { engine, RealmInfo } from '@dcl/sdk/ecs'

export function main() {
	RealmInfo.onChange(engine.RootEntity, (realmInfo) => {
		if (!realmInfo) return
		console.log('릴름이 변경되었습니다: ', realmInfo.realmName)
	})
}
```


> [!WARNING]
> **📔 참고**: 씬이 처음 로드될 때는 아직 플레이어에게 할당된 룸이 없을 수 있습니다. Explorer가 결국 플레이어에게 룸을 할당하지만, 이는 씬이 로드된 후 몇 초 뒤에 일어날 수도 있습니다.


## 플레이어 플랫폼 가져오기

플레이어는 공식 데스크톱 앱, 사용 중단된 웹 및 데스크톱 버전, 그리고 [대체 실험용 클라이언트](https://github.com/decentraland/protocol-squad) 를 포함한 다양한 플랫폼을 통해 Decentraland에 접속할 수 있습니다.

다음을 사용하세요 `getExplorerInformation()` 현재 플레이어가 어떤 플랫폼에서 Decentraland를 실행 중인지 확인합니다.

```ts
import { getExplorerInformation } from '~system/Runtime';

executeTask(async () => {
  let data = await getExplorerInformation({})
  console.log("플랫폼: ", data.platform, " 에이전트: ", data.agent)
})
```

공식 Decentraland 데스크톱 앱을 사용할 때 이 함수는 다음 데이터를 반환해야 합니다:

```
{
    agent: unity-explorer,
    platform: desktop
}
```

공식 Decentraland 데스크톱 앱을 사용하는 플레이어는 브라우저보다 훨씬 더 부드러운 경험을 할 가능성이 높습니다. 브라우저 탭이 사용할 수 있는 기계의 처리 성능에 브라우저가 성능 제한을 두기 때문입니다. 또한 카메라 제어, 동적 조명, 플레이어 이동 고정, UI 개선 등 많은 기능이 빠져 있습니다.

## 플레이어 언어 가져오기

다음을 사용하세요 `getPlayerLanguage()` 플레이어 클라이언트가 어떤 언어로 설정되어 있는지 확인합니다. 다음을 반환합니다: [BCP 47](https://en.wikipedia.org/wiki/IETF_language_tag) 태그를, 예를 들면 `'en'`, `'es'` 또는 `'pt-BR'`. 씬의 텍스트 중 어떤 번역을 표시할지 선택하는 데 사용하세요.

```ts
import { getPlayerLanguage, onPlayerLanguageChanged } from '@dcl/sdk/platform'

console.log('언어: ', getPlayerLanguage())

// 세션 중간에 플레이어가 언어를 바꾸는 것에 반응
onPlayerLanguageChanged.add(({ language }) => {
  console.log('언어가 다음으로 변경됨: ', language)
})
```

`onPlayerLanguageChanged` 플레이어가 씬에 있는 동안 클라이언트 언어를 바꿀 때마다 발생합니다. 오래된 클라이언트는 이를 전송하지 않습니다.


> [!WARNING]
> **📔 참고**: `getPlayerLanguage()` 반환값은 `'en'` Explorer가 씬에 언어를 다시 보고할 때까지이며, 이는 씬이 시작된 직후에 발생합니다. 또한 `'en'` 언어를 보고하지 않는 클라이언트에서는 true를 반환합니다. 두 함수 모두 `@dcl/sdk` 버전 7.30.0 이상이 필요합니다.


## EngineInfo 컴포넌트

해당 `EngineInfo`컴포넌트는 씬의 수명 주기에 관한 데이터를 추적하며, 이는 이벤트가 씬 초기화와 비교해 언제 발생하는지 파악하는 데 때때로 유용할 수 있습니다.

이 컴포넌트는 `engine.RootEntity`.

```ts
engine.addSystem((deltaTime) => {
  const engineInfo = EngineInfo.getOrNull(engine.RootEntity)
  if (!engineInfo) return

  console.log(
    '--------------' +
      '\nframeNumber: ' +
      engineInfo.frameNumber +
      '\ntickNumber: ' +
      engineInfo.tickNumber +
      '\ntotalRuntime: ' +
      engineInfo.totalRuntime +
      '\nsceneHidden: ' +
      engineInfo.sceneHidden +
      '\n--------------'
  )
})
```

다음을 사용할 수 있습니다: `sceneHidden` 씬이 보이지 않을 때 리소스를 많이 사용하는 작업을 일시 중지하려면:

```ts
engine.addSystem((deltaTime) => {
  const engineInfo = EngineInfo.getOrNull(engine.RootEntity)
  if (engineInfo?.sceneHidden) return

  // 일반적인 게임플레이 로직은 씬이 보일 때만 실행됩니다
})
```

해당 `EngineInfo` 컴포넌트는 다음 데이터를 보관합니다:

* `frameNumber`: 엔진의 프레임 카운터.
* `totalRuntime`: 이 씬의 총 실행 시간(초)입니다.
* `tickNumber`: 다음에 따른 씬의 틱 카운터 [ADR-148](https://adr.decentraland.org/adr/ADR-148).
* `sceneHidden`: `true` 씬이 Explorer의 전체 화면 UI 뒤에 숨겨져 있을 때입니다. 여기에는 로딩 화면뿐만 아니라 맵, 백팩, 설정 메뉴도 포함됩니다. 플레이어가 씬을 볼 수 없을 때 오디오를 음소거하고 무거운 작업을 일시 중지하는 데 사용하세요.


> [!WARNING]
> **📔 참고**: `EngineInfo` 컴포넌트는 다음을 통해 import해야 합니다
> 
> > `import { EngineInfo } from "@dcl/sdk/ecs"`
> 
> 참조 [가져오기](/docs/creator/content-creator-ko/sdk7/getting-started/coding-scenes/#imports) 에서 이를 쉽게 다루는 방법을 확인하세요.


### 로딩 화면이 사라지는 것에 반응하기

해당 `scene_hidden` 필드는 플레이어가 실제로 씬을 볼 수 있는지, 아니면 Explorer의 전체 화면 UI에 가려져 있는지를 알려줍니다. 로딩 화면이 표시되어 있는 동안에는 `sceneHidden` 가 `true`. 로딩 화면이 사라지고 플레이어가 세계를 처음 보게 되는 순간, 그것은 `false`.

그것은 `true` 나중에 맵, 백팩 또는 설정 메뉴 같은 전체 화면 Explorer UI가 씬을 가릴 때 다시 그렇게 됩니다. 따라서 음소거할 좋은 신호가 되며 [오디오](/docs/creator/content-creator-ko/sdk7/3d/sounds/) 비용이 큰 시스템을 일시 중지한 뒤, 다음으로 돌아오면 다시 시작하세요 `false`.

이것은 그 첫 공개가 언제 일어나는지 씬이 알 수 있는 유일한 방법입니다. 로딩 화면 뒤에서 재생되어 플레이어가 놓치게 될 모든 것, 예를 들어 인트로 시네마틱, स्वागत sounds, 보는 것이 중요해야만 잘 읽히는 트윈, 오프닝 UI, 또는 플레이어가 실제로 도착한 뒤에만 집계되어야 하는 분석 이벤트를 보류하는 데 사용하세요.

```ts
import { engine, EngineInfo } from '@dcl/sdk/ecs'

function onSceneRevealed() {
  // 이제 플레이어가 씬을 보고 있으니 여기서 인트로를 시작하세요
  console.log('로딩 화면이 방금 사라졌습니다')
}

engine.addSystem(function waitForSceneRevealed() {
  const engineInfo = EngineInfo.getOrNull(engine.RootEntity)
  if (!engineInfo || engineInfo.sceneHidden) return

  // 한 번만 실행
  engine.removeSystem(waitForSceneRevealed)
  onSceneRevealed()
})
```


> [!WARNING]
> **📔 참고**: 씬은 정상적으로 계속 실행되는 동안 `sceneHidden` 가 `true`, 단지 표시되지 않을 뿐입니다. 이 필드를 사용해 씬의 로직을 일시 중지하지 말고, 플레이어가 보게 될 순간을 맞추는 데 사용하세요.
> 
> `scene_hidden` 최신 버전의 `@dcl/sdk` 및 최근 버전의 Decentraland Explorer가 필요합니다. 오래된 클라이언트에서는 이 필드가 기본값인 `false`에 머무르므로, 이를 기다리는 씬도 계속 실행되지만 — 로딩 화면의 페이드아웃과는 동기화되지 않습니다.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/interactivity/runtime-data.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
