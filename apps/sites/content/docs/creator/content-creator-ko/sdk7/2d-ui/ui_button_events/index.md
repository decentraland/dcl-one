> UI 엔티티의 버튼 이벤트를 처리합니다.

# UI 버튼 이벤트

UI에 버튼을 만들려면 다음을 생성하세요 `버튼` 다음 속성을 가진 UI 요소:

* `값`: 버튼에 표시할 텍스트가 들어 있는 문자열.
* `onMouseDown`: 사용자가 엔터티 위에서 포인터 버튼을 누를 때마다 실행되는 콜백 함수입니다.
* `uiTransform`: UI 요소의 위치 지정 속성입니다.

다음 예제는 클릭 가능한 UI 버튼을 만드는 방법을 보여줍니다.

***ui.tsx 파일:***

```tsx
import { Button } from '@dcl/sdk/react-ecs'

export const uiMenu = () => (
	<Button
		value="클릭하세요"
		uiTransform={{ width: 100, height: 100 }}
		onMouseDown={() => {
			console.log('UI를 클릭했습니다')
		}}
	/>
)
```

***index.ts 파일:***

```ts
import { ReactEcsRenderer } from '@dcl/sdk/react-ecs'
import { uiMenu } from './ui'

export function main() {
    ReactEcsRenderer.setUiRenderer(uiMenu)
}
```


> [!WARNING]
> **📔 참고**: 이 페이지의 다음 스니펫들은 모두 다음이 있다고 가정합니다: `.ts` 위와 유사한 것이 실행 중이며, `ReactEcsRenderer.setUiRenderer()` 함수입니다.


클릭 시 실행되는 함수를 UI 정의 밖에 작성하고 이름으로 참조할 수도 있습니다. 이렇게 하면 UI 코드의 가독성이 높아지고, 여러 개의 클릭 가능한 UI 엔터티가 같은 함수를 호출해야 할 때도 유용합니다.

```tsx
import { Button } from '@dcl/sdk/react-ecs'

function handleClick() {
	// onClick 시 무언가 수행
	console.log('UI를 클릭했습니다')
}
export const uiMenu = () => (
	<Button
		value="클릭하세요"
		uiTransform={{ width: 100 }}
		onMouseDown={handleClick}
	/>
)
```

다음 필드는 다음에 추가할 수 있습니다: `버튼` UI 요소:

* `onMouseDown`: 사용자가 엔터티 위에서 포인터 버튼을 누를 때마다 실행되는 콜백 함수입니다.
* `onMouseUp`: 포인터가 엔터티를 가리키는 동안 포인터 버튼을 뗄 때마다 실행되는 콜백 함수입니다.
* `onMouseEnter`: 포인터가 버튼 위로 호버를 시작할 때마다 실행되는 콜백 함수입니다.
* `onMouseLeave`: 포인터가 버튼 위에서 호버를 멈출 때마다 실행되는 콜백 함수입니다.
* `색상`: 버튼 텍스트의 색상입니다.
* `글꼴`: 버튼 텍스트의 글꼴입니다.
* `textAlign`: 버튼 내부 텍스트의 정렬
* `uiTransform`: UI 요소의 위치 지정 속성입니다.
* `uiBackground`: UI 요소의 색상 또는 텍스처를 설정합니다.
* `variant`: 이 속성을 사용해 버튼 스타일을 기본값 중 하나로 설정합니다. `primary` 및 `secondary` 를 사용할 수 있습니다.
* `disabled`: 버튼을 비활성화하는 불리언 값입니다.  `disabled` 가 *true*, `onMouseDown` 및 `onMouseUp` 동작은 더 이상 호출되지 않고, 버튼은 포인터 상호작용을 더 이상 알리지 않습니다. 버튼은 또한 "회색으로 비활성화된" 상태로 그려집니다. 텍스트와 배경 모두 자신의 절반 `알파` 값으로 렌더링됩니다. 이는 표시상의 변경일 뿐입니다.  `Color4` 씬이 전달하는 값은 절대 수정되지 않으므로, 여러 요소에서 공유 팔레트 객체를 안전하게 재사용할 수 있습니다.

## 버튼 스타일링

variant를 다음으로 설정하면 `primary` 또는 `secondary` 버튼의 기본 스타일 옵션을 활용할 수 있습니다. `primary` 버튼을 흰색 텍스트의 빨간색으로 만들고, `secondary` 버튼을 빨간색 텍스트의 흰색으로 만듭니다.

```tsx
import { UiEntity, Button, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
	<UiEntity
		uiTransform={{
			width: 500,
			height: 230,
			margin: '16px 0 8px 270px',
			padding: 4,
			alignSelf: 'center',
		}}
		uiBackground={{ color: Color4.Gray() }}
	>
		<Button
			value="클릭하세요"
			variant="primary"
			uiTransform={{ width: 80, height: 20, margin: 4 }}
			onMouseDown={() => {
				console.log('UI를 클릭했습니다')
			}}
		/>
		<Button
			value="클릭하세요"
			variant="secondary"
			uiTransform={{ width: 80, height: 20, margin: 4 }}
			onMouseDown={() => {
				console.log('UI를 클릭했습니다')
			}}
		/>
	</UiEntity>
)
```

background의 모든 속성을 자유롭게 사용할 수도 있습니다. variant를 설정한 다음 그 속성 중 일부를 재정의할 수도 있습니다. 이 예제는 `primary` variant를 사용하지만 색상을 초록색으로 재정의합니다:

```tsx
import { Button } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
	<Button
		value="내 버튼!"
		variant="primary"
		uiTransform={{ width: 100, height: 100 }}
		onMouseDown={() => {
			console.log('내 버튼을 클릭했습니다!')
		}}
		uiBackground={{
			color: Color4.Green(),
		}}
	/>
)
```

## 토글 가능한 버튼

흔한 사용 사례는 버튼을 스위치처럼 두 상태 사이에서 전환되게 만드는 것입니다. 아래 예제는 버튼을 누를 때마다 두 색상 사이를 전환합니다:

```tsx
import { Button } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

let buttonEnabled = false

export const uiMenu = () => (
	<Button
		value="내 버튼"
		variant="primary"
		uiTransform={{ width: 100, height: 100 }}
		onMouseDown={() => {
			console.log('내 버튼을 클릭했습니다!')
			buttonEnabled = !buttonEnabled
			if (buttonEnabled) {
				// 무언가 수행
			} else {
				// 다른 작업 수행
			}
		}}
		uiBackground={{
			color: buttonEnabled ? Color4.Green() : Color4.Red(),
		}}
	/>
)
```

참고로 위 예제에서 색상은 다음에 따라 달라집니다: `buttonEnabled` 변수. 이 변수의 값이 바뀔 때마다 즉시 배경색에 반영됩니다.

## 호버 피드백

또 다른 흔한 사용 사례는 버튼 위에 호버할 때 어떤 시각적 힌트를 표시하여 이것이 상호작용 가능하다는 점을 분명히 하거나, 이 버튼이 무엇을 하는지 설명하는 호버 힌트를 표시하는 것입니다. 다음을 사용하세요: `onMouseEnter` 및 `onMouseLeave` 플레이어의 커서가 버튼 위에 있을 때를 감지하고 그에 맞게 반응하는 콜백입니다.

```tsx
import { Button } from '@dcl/sdk/react-ecs'

let buttonEnabled = false

export const uiMenu = () => (
	<Button
		value="내 버튼"
		uiTransform={{ width: 100, height: 100 }}
		onMouseDown={() => {
			// 버튼 함수
		}}
		onMouseEnter={() => {
			// 힌트 표시
		}}
		onMouseLeave={() => {
			// 힌트 숨김
		}}
	/>
)
```

## 다른 요소를 클릭 가능하게 만들기

UI의 어떤 요소든 다음을 추가하여 클릭 가능하게 만들 수 있습니다: `onMouseDown` 속성을 추가하면 버튼과 동일하게 작동합니다. 다음 예제는 `onMouseDown` 배경 이미지와 텍스트에 속성을 추가합니다.

```tsx
import { UiEntity, Label, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'
import { engine, Transform } from '@dcl/sdk/ecs'

export const uiMenu = () => (
	<UiEntity
		onMouseDown={() => {
			console.log('배경을 클릭했습니다!')
		}}
		uiTransform={{
			width: 400,
			height: 230,
		}}
		uiBackground={{ color: Color4.create(0.5, 0.8, 0.1, 0.6) }}
	>
		<Label
			onMouseDown={() => {
				console.log('레이블을 클릭했습니다!')
			}}
			value={`플레이어: ${getPlayerPosition()}`}
			fontSize={18}
			uiTransform={{ width: '100%', height: 30 }}
		/>
	</UiEntity>
)

function getPlayerPosition() {
	const playerPosition = Transform.getOrNull(engine.PlayerEntity)
	if (!playerPosition) return '알 수 없음'
	const { x, y, z } = playerPosition.position
	return `{x: ${x.toFixed(2)}, y: ${y.toFixed(2)}, z: ${z.toFixed(2)} }`
}
```

## 포인터 차단

모든 UI 엔터티는 기본적으로 포인터 차단이 아니므로, 플레이어의 클릭은 이를 통과해 뒤에 있는 3D 월드 공간의 오브젝트와 상호작용하게 됩니다. 엔터티에 `onMouseDown` 콜백이 있으면 포인터 차단 상태가 되어 플레이어의 클릭이 해당 UI 엔터티 뒤의 대상에 영향을 주지 않습니다.

다음의 값을 변경하여 이 기본 동작을 바꿀 수 있습니다: `pointerFilter` 속성을 `uiTransform` 모든 UI 엔터티의 컴포넌트에서. 예를 들어, `onMouseDown` 포인터 차단 상태로 설정하려면

다음 값이 지원됩니다: `pointerFilter` :

* `block`: UI 요소가 포인터 차단 상태이므로, 플레이어는 이 UI 요소 뒤의 어떤 것도 클릭할 수 없습니다.
* `none`: UI 요소가 포인터 차단이 아닙니다. 이 요소는 클릭할 수 없으며, 뒤에 있는 것은 무엇이든 클릭할 수 있습니다.

아래는 다음이 없는 간단한 UI입니다: `onMouseDown`, 하지만 다음을 설정하여 기본 동작인 비포인터 차단 상태를 무시합니다: `pointerFilter` 을(를) `block`.

```tsx
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

// UI를 그리기
export const uiMenu = () => (
	<UiEntity
		uiTransform={{
			width: '100%',
			height: '100px',
			pointerFilter: `block`,
		}}
		uiText={{ value: `이 요소는 포인터 차단 상태입니다`, fontSize: 40 }}
		uiBackground={{ color: Color4.create(0.5, 0.8, 0.1, 0.6) }}
	/>
)
```

### 차단은 보이는 픽셀이 아니라 레이아웃 박스를 따릅니다

차단 요소는 다음 전체에 걸친 클릭을 가로챕니다: **전체 직사각형**에 무엇이 그려져 있든 상관없습니다. 완전히 투명한 배경도 차이가 없습니다.


> [!CAUTION]
> **경고:** 포인터 핸들러나 `pointerFilter: 'block'` 전체 화면 래퍼에 넣지 마세요. 크기가 `100%` 로 `100%`.
> 
> 그 직사각형은 화면 전체이므로, 레이아웃 루트에 잘못 들어간 하나의 `onMouseDown` 포인터 핸들러가 다른 모든 UI 요소와 3D 세계의 모든 것을 클릭 불가능하게 만듭니다. 보이는 패널은 화면의 작은 부분만 덮기 때문에 UI는 여전히 완벽해 보여서 이를 알아채기 매우 어렵습니다.


핸들러는 필요한 가장 작은 요소에만 연결하세요: 패널, 버튼, 행. 레이아웃 래퍼에는 핸들러를 두지 마세요.

전체 화면 차단 요소가 적절한 경우는 두 가지이며, 둘 다 의도적인 경우입니다:

* 하나는 **모달 배경**, 클릭을 흡수하기 위한 것이며 모달이 열려 있을 때만 렌더링합니다.
* 하나는 **드래그 해제 캐처**, 드래그가 진행되는 동안에만 존재합니다. 다음을 참조하세요: [드래그 상호작용](#drag-interactions) 아래.

씬의 어느 곳에서든 클릭이 작동하지 않게 되면, 가장 먼저 이 부분을 확인하세요.

## 드래그 상호작용

UI 포인터 핸들러(`onMouseDown`, `onMouseUp`, `onMouseEnter`, `onMouseLeave`)는 매개변수를 받지 않습니다. 이는 단순한 `() => void` 콜백으로 실행되며, 위치나 좌표 데이터는 없습니다. 다음은 존재하지 않습니다: `onMouseDrag` 또는 `onMouseMove` UI 시스템의 핸들러.

드래그 기반 UI(슬라이더, 스크럽 바, 드래그 핸들)를 만들려면 다음을 사용하세요: `PrimaryPointerInfo.screenDelta` 다음에서 `@dcl/sdk/ecs`. 이를 통해 마지막 프레임 이후의 마우스 이동량을 픽셀 단위로 얻을 수 있으며, 커서가 무엇 위에 있든 상관없이 매 프레임 업데이트됩니다.

패턴은 다음과 같이 동작합니다:

1. `onMouseDown` 드래그 대상에서 드래그를 시작하고 초기 값을 기록합니다.
2. 시스템이 다음을 읽습니다: `screenDelta` 를 매 프레임 읽고, 드래그가 활성화되어 있는 동안 그 값을 누적합니다.
3. 다음을 가진 전체 화면의 보이지 않는 오버레이가 `pointerFilter: 'block'` 마우스 버튼 해제를 감지하므로, 좁은 대상 밖에서 놓아도 드래그가 끝납니다.

```ts
import { engine, PrimaryPointerInfo, UiCanvasInformation } from '@dcl/sdk/ecs'

let dragging = false
let sliderValue = 0.5

// 이 시스템을 매 프레임 호출하여 드래그 이동을 누적합니다
engine.addSystem(() => {
	if (!dragging) return

	const delta = PrimaryPointerInfo.getOrNull(engine.RootEntity)?.screenDelta
	if (!delta || delta.x === 0) return

	// 화면 픽셀을 0–1 범위로 변환
	// UI 스케일 팩터로 나누어 드래그 속도가 커서와 일치하도록 함
	const canvas = UiCanvasInformation.getOrNull(engine.RootEntity)
	const scale = canvas ? Math.min(canvas.width / 1920, canvas.height / 1080) : 1
	const trackWidth = 200 // slider track의 가상 픽셀 너비

	sliderValue = Math.max(0, Math.min(1, sliderValue + delta.x / scale / trackWidth))
})
```


> [!WARNING]
> **참고:** 모바일에서는 `screenDelta` 항상 0을 반환합니다(자유롭게 움직이는 커서가 없기 때문입니다). 모바일과 호환되는 슬라이더를 만들려면 스테퍼 버튼(`-` / `+`)을 드래그 트랙 옆에 추가하세요. 다음을 사용하세요: [`isMobile()`](/docs/creator/content-creator-ko/build-for-mobile/develop/detect-platform/) 다음에서 `@dcl/sdk/platform` 을 사용해 UI 분기를 나누세요.



> [!NOTE]
> **팁:** 항상 다음을 나누세요: `screenDelta` UI 스케일 팩터로 나누지 않으면, 해상도가 가상 크기와 다른 화면에서 드래그가 과하게 가거나 부족하게 됩니다.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/2d-ui/ui_button_events.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
