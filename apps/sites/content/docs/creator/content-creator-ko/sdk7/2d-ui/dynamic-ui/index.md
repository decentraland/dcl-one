> 데이터 변경에 반응하는 동적 UI를 만드는 방법을 알아봅니다.

# 동적 UI

동적 요소를 포함하는 UI를 정의할 수 있으며, 이는 매 틱마다 업데이트됩니다. 이 데이터에 해당하는 변수를 업데이트하기만 하면 되고, UI는 새로운 값에 맞춰 자동으로 반응합니다.

이것은 타이머, 플레이어의 점수 등과 같은 요소를 포함하는 데 매우 유용합니다. 하지만 이를 한 단계 더 발전시켜 상태를 기반으로 전체 UI 구조를 정의할 수도 있습니다.

## 참조 변수

uiEntity의 컴포넌트 중 어떤 속성에서든 변수를 간단히 참조할 수 있습니다. 변수가 값이 바뀌면 UI도 그에 맞게 조정됩니다.

아래 예제는 변수를 정의합니다 `playerCurrentPosition` 그리고 이를 문자열의 일부로 `uiText` 컴포넌트에서 참조합니다. 그러면 시스템이 플레이어의 현재 위치를 사용해 매 틱마다 이 변수의 값을 업데이트합니다. 변수의 값이 바뀌면 UI도 그에 맞게 업데이트되며, UI를 명시적으로 수정할 필요가 전혀 없습니다.

***ui.tsx 파일:***

```tsx
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { playerCurrentPosition } from './index'
import { Color4 } from '@dcl/sdk/math'

// UI를 그리기
export const uiMenu = () => (
  <UiEntity
    uiTransform={{
			width: '100%',
			height: '100px',
			justifyContent: 'center',
			alignItems: 'center',
    }}
    uiText={{ value: `플레이어: `+  playerCurrentPosition, fontSize: 40 }}
    uiBackground={{ color: Color4.create(0.5, 0.8, 0.1, 0.6) }}
  />
)
```

***index.ts 파일:***

```ts
import { ReactEcsRenderer } from '@dcl/sdk/react-ecs'
import { engine, Transform } from '@dcl/sdk/ecs'
import { uiMenu } from './ui'

export function main() {
    ReactEcsRenderer.setUiRenderer(uiMenu, { virtualWidth: 1920, virtualHeight: 1080 })
}

// 변수 정의
export let playerCurrentPosition: string = ""

// 변수를 업데이트하는 시스템
engine.addSystem(() => {
  const playerPosition = Transform.getOrNull(engine.PlayerEntity)
  if (!playerPosition) return
  const { x, y, z } = playerPosition.position
  playerCurrentPosition =  `{x: ${x.toFixed(2)}, y: ${y.toFixed(2)}, z: ${z.toFixed(2)} }`
})
```

위 예제에서는 변수도 문자열의 일부로 포함할 수 있으며, 변수를 `${ }`.

```ts
uiText={{
	value: `플레이어: ${playerCurrentPosition}`,
	fontSize: 40
}}
```

## UI 내부에서 함수 호출하기

JSX 정의 내부에서 함수도 호출하여, UI의 속성에 사용할 값을 반환할 수 있습니다. 이 JSX 정의 내부에서 호출된 함수는 게임 루프의 매 틱마다 반복적으로 호출됩니다.

아래 예제에서는 `uiText` 컴포넌트가 `getPlayerPosition()` 함수를 호출하여 표시할 문자열의 일부를 정의합니다.

이 예제는 앞선 섹션의 예제와 비슷하지만, UI 정의 내부에서 함수를 호출함으로써 별도의 변수를 선언하고 그 변수를 변경하는 시스템을 정의할 필요가 없어집니다. 참고로 `getPlayerPosition()` 은 게임 루프의 매 틱마다 호출되며, 별도의 시스템을 명시적으로 선언할 필요가 없습니다.

***ui.tsx 파일:***

```tsx
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'
import { engine, Transform } from "@dcl/sdk/ecs";

export const uiMenu = () => (
  <UiEntity
    uiTransform={{
      width: '100%',
      height: '100px',
      justifyContent: 'center',
      alignItems: 'center',
    }}
    uiText={{ value: `플레이어: `+  getPlayerPosition(), fontSize: 40 }}
    uiBackground={{ color: Color4.create(0.5, 0.8, 0.1, 0.6) }}
  />
)

function getPlayerPosition(){
  const playerPosition = Transform.getOrNull(engine.PlayerEntity)
  if (!playerPosition) return '로딩 중...'
  const { x, y, z } = playerPosition.position
  return `{x: ${x.toFixed(2)}, y: ${y.toFixed(2)}, z: ${z.toFixed(2)} }`
}
```

***index.ts 파일:***

```ts
import { ReactEcsRenderer } from '@dcl/sdk/react-ecs'
import { uiMenu } from './ui'

export function main() {
    ReactEcsRenderer.setUiRenderer(uiMenu, { virtualWidth: 1920, virtualHeight: 1080 })
}
```

## UI 켜기/끄기 전환하기

UI를 켜고 끄는 가장 쉬운 방법은 `display` 엔티티의 `uiTransform`. `display` 속성은 UI 엔티티와 그 모든 자식을 보이지 않게 만듭니다 `none`.

다음 예제는 변수를 사용하여 `display` UI의 한 부분에 있는 필드를 설정합니다. 이 변수의 값은 다른 UI 요소를 클릭하여 전환할 수 있습니다.

***ui.tsx 파일:***

```tsx
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

// 현재 메뉴 표시 상태를 반영하는 변수
var isMenuVisible: boolean = false

// 메뉴 상태를 전환하는 함수
function toggleMenuVisibility() {
  isMenuVisible = !isMenuVisible
}

export const uiMenu = () => (
   // 상위 요소
   <UiEntity>
      {/* 메뉴 */}
      <UiEntity
       uiTransform={{
          width: '80%',
          height: '100px',
          alignContent: 'center',
          justifyContent: 'center',
          display: isMenuVisible ? 'flex': 'none'
        }}
         uiText={{
          value: "메뉴",
          fontSize: 30
        }}
        uiBackground={{ color: Color4.Green() }}
      />
      {/* 버튼 */}
      <UiEntity
        uiTransform={{
          width: 100,
          height: 30,
          margin: { top: '35px', left: '500px' }
        }}
        uiText={{
          value: "메뉴 전환",
          fontSize: 40
        }}
        uiBackground={{ color: Color4.Red() }}
        onMouseDown={toggleMenuVisibility}
      />
   </UiEntity>
)
```

***index.ts 파일:***

```ts
import { ReactEcsRenderer } from '@dcl/sdk/react-ecs'
import { uiMenu } from './ui'

export function main() {
    ReactEcsRenderer.setUiRenderer(uiMenu, { virtualWidth: 1920, virtualHeight: 1080 })
}
```

## 동적 UI 엔티티

위의 섹션에 있는 예제들은 엔티티의 단일 속성을 동적으로 변경하는 방법을 보여 주지만, 동적으로 변하는 데이터를 기반으로 크기가 조정될 수 있는 엔티티 구조 전체를 정의할 수도 있습니다. 이런 패턴은 React 같은 라이브러리를 사용할 때 웹 개발에서 흔히 볼 수 있으며, 매우 강력합니다. 이를 통해 매우 유연하고 확장 가능한 UI 애플리케이션을 정의할 수 있습니다.

다음 예제는 장면에서 다음을 가진 모든 엔티티의 ID를 나열합니다 `MeshRenderer` 및 `Transform`. 그리고 이를 생성합니다 `uiText` 각각에 대해. 장면의 콘텐츠가 변경되면 UI 엔티티 목록도 매 틱마다 함께 적응합니다.

***ui.tsx 파일:***

```tsx
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'
import { engine } from '@dcl/sdk/ecs'
import { MeshRenderer } from '@dcl/sdk/ecs'
import { Transform } from '@dcl/sdk/ecs'

export const uiMenu = () => (
  <UiEntity
    uiTransform={{
      width: '100%',
      height: '300px',
      justifyContent: 'center',
      alignItems: 'center',
    }}
    uiBackground={{ color: Color4.create(0.5, 0.8, 0.1, 0.6) }}
  >
    <UiEntity>
      {generateText()}
    </UiEntity>
  </UiEntity>
)


function generateText(){
  return Array.from(engine.getEntitiesWith(
    MeshRenderer,
    Transform
  )).map(([entity]) => <TextComponent value={entity.toString()} key={entity} /> )
}


function TextComponent(props: { value: string; key: string | number }) {
  return <UiEntity
    key={props.key}
    uiTransform={{ width: 80, height: 20 }}
    uiText={{ value: props.value, textAlign: 'middle-center', fontSize: 12 }}
    uiBackground={{ color: { r: 1, g: 0.176, b: 0.333, a: 1 } }}
  />
}
```

***index.ts 파일:***

```ts
import { ReactEcsRenderer } from '@dcl/sdk/react-ecs'
import { uiMenu } from './ui'

export function main() {
    ReactEcsRenderer.setUiRenderer(uiMenu, { virtualWidth: 1920, virtualHeight: 1080 })
}
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/2d-ui/dynamic-ui.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
