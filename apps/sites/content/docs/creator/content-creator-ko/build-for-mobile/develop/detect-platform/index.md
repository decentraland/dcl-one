> 씬이 모바일, 데스크톱 또는 웹에서 실행 중인지 감지합니다.

# 플랫폼 감지

다음을 사용하세요 `isMobile()` 모바일에 맞게 UI, 조작, 게임플레이를 조정하는 함수입니다. 씬 로직을 완전히 분기하지 않고도 모든 클라이언트에서 훌륭한 경험을 제공하는 권장 방법입니다.

## 사용 가능한 함수

```ts
import { getPlatform, isMobile, isDesktop, isWeb } from '@dcl/sdk/platform'
```

* **`getPlatform()`** — 현재 플랫폼을 다음으로 반환합니다 `'mobile' | 'desktop' | 'web' | null`. 반환합니다 `null` 탐색기가 플랫폼을 씬으로 다시 보고할 때까지입니다(이 작업은 씬이 시작된 직후에 일어납니다).
* **`isMobile()`** — 반환합니다 `true` 플레이어가 모바일 클라이언트에 있는 경우.
* **`isDesktop()`** — 반환합니다 `true` 플레이어가 데스크톱 클라이언트에 있는 경우.
* **`isWeb()`** — 반환합니다 `true` 플레이어가 웹 클라이언트에 있는 경우.


> [!WARNING]
> **📔 참고**: 이러한 함수는 씬이 시작될 때 비동기적으로 채워지는 값을 읽으며, 실행 시점에 이미 사용할 수 있다고 보장되는 것은 없습니다 `main()` 실행됩니다. 너무 일찍 호출하면 여전히 반환할 수 있습니다 `null` / `false`. 안전하게 하려면, 플랫폼에 의존하는 설정은 `getPlatform()` 다음이 아닌 값을 반환할 때까지 `null`, 예를 들어 시스템 내부에서 확인하는 방식으로:
> 
> ```ts
> import { engine } from '@dcl/sdk/ecs'
> import { getPlatform } from '@dcl/sdk/platform'
> 
> function platformCheckSystem() {
>   if (getPlatform() === null) return
>   engine.removeSystem(platformCheckSystem)
>   setupUI()
> }
> 
> engine.addSystem(platformCheckSystem)
> ```
> 


## 플랫폼별로 UI 분기하기

모바일 플레이어와 데스크톱 플레이어를 위해 서로 다른 UI를 설정하는 것이 일반적인 패턴입니다:

```ts
import { isMobile, isDesktop } from '@dcl/sdk/platform'

function setupUI() {
  if (isMobile()) {
    // 터치에 맞춘 더 큰 버튼, 더 단순한 레이아웃
    createMobileUI()
  } else if (isDesktop()) {
    // 키보드와 마우스에 맞춘 더 조밀한 레이아웃
    createDesktopUI()
  }
}
```

다음에도 같은 패턴을 사용할 수 있습니다:

* 각 입력 방식에 맞춘 화면 안내를 표시하거나 숨깁니다.
* 모바일에서는 작은 클릭 요소를 더 큰 터치 대상으로 대체합니다.
* 모바일에서 쉽게 사용할 수 없는 입력 바인딩을 비활성화합니다(참조 [모바일에서의 입력](/docs/creator/content-creator-ko/build-for-mobile/develop/input-on-mobile/)).

## 원시 플랫폼 값을 확인하기

단일 switch에서 여러 플랫폼을 처리해야 한다면, 다음을 사용하세요 `getPlatform()`:

```ts
import { getPlatform } from '@dcl/sdk/platform'

const platform = getPlatform()

switch (platform) {
  case 'mobile':
    // 모바일 전용 동작
    break
  case 'desktop':
    // 데스크톱 전용 동작
    break
  case 'web':
    // 웹 전용 동작
    break
  case null:
    // 플랫폼이 아직 알려지지 않음
    break
}
```

## 관련

* [모바일 안전 영역](/docs/creator/content-creator-ko/build-for-mobile/develop/safe-area/)
* [모바일 UI 모범 사례](/docs/creator/content-creator-ko/build-for-mobile/develop/ui-best-practices/)
* [화면 UI](/docs/creator/content-creator-ko/sdk7/2d-ui/onscreen-ui/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/build-for-mobile/develop/detect-platform.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
