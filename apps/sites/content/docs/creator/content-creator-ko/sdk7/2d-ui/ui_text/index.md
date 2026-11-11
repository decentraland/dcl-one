> UI 엔티티에 텍스트를 씁니다.

# UI 텍스트

UI에 텍스트를 추가하려면 다음을 만들어 보세요 `Label` 엔티티의 부모입니다.

A `Label` 엔터티에는 다음과 같이 구성할 수 있는 필드가 있습니다:

* `값`: 표시할 문자열
* `fontSize`: 텍스트의 크기, 숫자 형식입니다.

  > 참고: 다음 `fontSize` 는 해당 엔터티나 부모 엔터티의 크기에 영향을 받지 않습니다.
* `색상`: 텍스트의 색상, 다음과 같이 [Color4](/docs/creator/content-creator-ko/sdk7/3d/color-types/).
* `font`: 사용할 글꼴입니다. 지원되는 값은 다음과 같습니다:
  * `'serif'`
  * `'sans-serif'` *(기본값)*
  * `'monospace'`
* `fontSrc`: 다음으로의 경로 `.ttf` 씬의 글꼴 파일로, 텍스트를 사용자 지정 글꼴로 렌더링합니다. `font` 대체 글꼴로 사용됩니다. 다음을 참조하세요 [사용자 지정 폰트](#custom-fonts).
* `textAlign`: 텍스트가 부모와 어떻게 정렬될지 지정합니다. 다음에서 값을 가져옵니다: `TextAlignType` type. TextAlignType = 'top-left' | 'top-center' | 'top-right' | 'middle-left' | 'middle-center' | 'middle-right' | 'bottom-left' | 'bottom-center' | 'bottom-right';
* `textWrap`: 텍스트가 최대 너비를 초과하지 않도록 줄바꿈을 사용할지 설정합니다. 기본적으로 켜져 있으며(`'wrap'`), 비활성화하려면 다음 값을 전달하세요: `'nowrap'`.


> [!WARNING]
> **📔 참고**: `fontSize` 는 해당 엔터티나 부모 엔터티의 크기에 영향을 받지 않습니다.


A `Label` 엔터티에는 다른 유형의 UI 엔터티에서 볼 수 있는 다른 일반 컴포넌트도 포함될 수 있습니다. 예: `uiTransform` 그리고 `uiBackground`.

***ui.tsx 파일:***

```ts
import { UiEntity, Label, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
	<UiEntity uiTransform={{ width: 'auto', height: 'auto' }}>
		<Label
			value="This is a label"
			color={Color4.Red()}
			fontSize={29}
			font="sans-serif"
			textAlign="top-left"
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


> [!WARNING]
> **📔 참고**: 이 페이지의 다음 모든 코드 조각은 다음이 있다고 가정합니다: `.ts` 위와 유사한 것이 실행 중인 상태입니다: `ReactEcsRenderer.setUiRenderer()` 함수를 사용하세요.


텍스트 한 줄이 할당된 너비나 컨테이너의 최대 너비에 맞기에는 너무 길면, 텍스트는 다음 줄로 이어집니다. 다음 값을 변경하면 이를 비활성화할 수 있습니다: `textWrap` 속성을 `'nowrap'`.

또한 다음을 명시적으로 추가하여 줄바꿈을 강제할 수 있습니다: `\n` 문자열에.

```ts
import { UiEntity, Label, ReactEcs } from '@dcl/sdk/react-ecs'

export const uiMenu = () => (
	<UiEntity uiTransform={{ width: 700, height: 400 }}>
		<Label 
			value="Hello World!\nThis other bit is quite long. It probably won't fit in a single line, so it will include a line break somewhere.\nFourth line"
			textWrap= {`nowrap`} 
		/>
	</UiEntity>
)
```

명시적인 `height` 또는 `width` 가 설정되어 있지 않으면 `uiTransform` 컨테이너의 ...에 설정되어 있지 않으면, 컨테이너는 값 `자동`을 사용하며, 이는 모든 텍스트에 맞게 조정됩니다. 다음을 설정할 수 있습니다: `최대 너비` 그리고 `최대 높이` 로 특정 한도를 초과하지 않도록 할 수 있습니다. 또한 다음을 사용할 수 있습니다: `최소 너비` 그리고 `최소 높이` 텍스트가 더 짧더라도 컨테이너가 너무 작아지지 않도록 할 수 있습니다.


> [!WARNING]
> **📔 참고**: 텍스트에 대해 해당 자동 맞춤에 의존하지 마세요. 얼마나 많은 공간을 `Label` 크기를 지정하지 않았을 때 차지하는지는 탐색기마다 다릅니다. Bevy 기반 탐색기는 렌더링된 텍스트를 측정해 그에 맞게 배치합니다. Unity 탐색기는 거의 높이를 주지 않지만 글자는 계속 그리므로, 쌓인 레이블이 서로 겹치고 텍스트 크기에 맞춰진 부모는 아무것도 없는 것처럼 접힙니다.
> 
> 모든 `Label` 에 명시적인 `width` **그리고** `height` 에서 `uiTransform`를 지정하세요, 그리고 레이블을 쌓는 모든 컨테이너에는 명시적인 높이를 지정하세요. 줄바꿈된 텍스트의 경우, 줄 수에 맞춰 높이를 지정하세요: 다음에서 두 줄이면 `fontSize: 20` 에는 대략 `height: 60`.
> 
> 이는 탐색기마다 다르므로, 한 클라이언트에서 올바르게 보이는 미리보기가 다른 클라이언트에서도 레이아웃이 올바르다는 것을 의미하지는 않습니다.


## 사용자 지정 폰트

렌더링하려면 `Label` 사용자 지정 글꼴로 렌더링하려면 TrueType(`.ttf`) 폰트 파일을 씬 폴더에 추가하고 다음을 설정하세요: `fontSrc` 속성을 씬의 루트 폴더를 기준으로 한 경로에 추가하세요. 다음을 유지하세요: `font` 또한 설정하세요: 사용자 지정 폰트를 불러올 수 없을 때마다 사용되는 대체 폰트입니다.


> [!WARNING]
> **📔 참고**: `fontSrc` 필요합니다 `@dcl/sdk` 7.31.0 이상이 필요합니다. 이전 버전에는 이 속성이 없습니다.


```tsx
<Label
	value="Welcome!"
	fontSize={40}
	uiTransform={{ width: 300, height: 60 }}
	fontSrc="assets/fonts/Lobster-Regular.ttf"
	font="sans-serif"
/>
```

다음의 `Button` 엔터티, 그리고 `uiText` 의 트윈 정의 목록을 제공하세요 `UiEntity` 및 [`Input` 그리고 `Dropdown`](/docs/creator/content-creator-ko/sdk7/2d-ui/ui_special_types/) 엔터티는 다음도 허용합니다: `fontSrc` 도 마찬가지이며, [`TextShape`](/docs/creator/content-creator-ko/sdk7/3d/text/#custom-fonts) 3D 공간의 텍스트용 컴포넌트도 마찬가지입니다.

사용자 지정 폰트를 사용할 때는 다음 사항을 기억하세요:

* **오직 `.ttf` 파일만 지원됩니다**. CFF 윤곽을 사용하는 OpenType 글꼴(보통 `.otf`), 글꼴 모음과 웹 글꼴은 지원되지 않으며, 텍스트는 대체 글꼴로 렌더링됩니다 `font`.
* **폰트는 씬 안의 파일이어야 합니다**. `fontSrc` 외부 URL을 가리킬 수 없으며, 폰트 패밀리 이름(예: Google Fonts의 이름)도 지원되지 않습니다. 다음을 다운로드하세요: `.ttf` 파일을 씬 폴더에 포함하세요. 라이선스가 재배포를 허용하는지 확인하세요.
* **데스크톱 클라이언트에서는 씬의 에셋이 최적화된 후에만 사용자 지정 폰트가 표시됩니다**. 씬을 게시하면 폰트는 3D 모델과 함께 최적화된 에셋 번들로 변환되며, 변환이 완료되면 사용자 지정 폰트가 표시됩니다. 그 전까지 텍스트는 대체 폰트를 사용합니다 `font`. 로컬 미리보기도 마찬가지입니다. 사용자 지정 글꼴은 최적화된 에셋으로 미리보기할 때만 표시됩니다( **에셋 압축** Creator Hub의 옵션 또는 `npm run start -- --asset-bundles` 를 CLI에서 사용). 참고: [최적화된 에셋으로 미리보기](/docs/creator/content-creator-ko/sdk7/getting-started/preview-scene/#preview-with-optimized-assets). Bevy 웹 클라이언트는 다음을 직접 로드하므로 `.ttf` 파일을 직접 로드하므로, 로컬 미리보기에서도 사용자 지정 폰트가 바로 표시됩니다.
* **모바일에서는 지원되지 않음**. Decentraland 모바일 앱은 다음을 무시하고 `fontSrc` 대체 폰트로 렌더링합니다 `font`. 여전히 보기 좋은 대체 폰트를 선택하고, 고정된 공간에 텍스트를 맞추기 위해 사용자 지정 폰트에 의존하지 마세요. 참고: [모바일의 누락된 기능](/docs/creator/content-creator-ko/build-for-mobile/mobile-client/missing-features/).
* **파일 하나는 한 개의 페이스입니다**. 굵은 글꼴과 일반 글꼴을 모두 추가하고 `.ttf` 파일을 추가한 뒤 각 텍스트를 해당 파일로 지정하세요.
* **Creator Hub UI 디자이너는 다음을 `fontSrc`**. 코드에서 요소에 직접 수동으로 추가하세요. UI 디자이너는 사용자가 편집한 속성만 변경하므로, `fontSrc` 직접 작성한 것은 그대로 유지됩니다. 캔버스에는 사용자 지정 글꼴이 표시되지 않습니다.
* **글꼴에 포함되지 않은 문자는** 내장 sans-serif 글꼴로 그려집니다. 글꼴에 포함된 비라틴 문자는 처음 표시될 때 렌더링되며, 이로 인해 잠깐 끊길 수 있습니다.

최대 파일 크기와 같은 전체 제한 사항은 다음을 참조하세요 [사용자 지정 폰트](/docs/creator/content-creator-ko/sdk7/3d/text/#custom-fonts) 텍스트 페이지의

## UI 텍스트에 이모지를 사용하지 마세요

모든 `Label` 또는 `Button` `값`, `uiText.value`, `Input` `자리 표시자`, 그리고 `Dropdown` 옵션.

SDK에는 이모지 글리프가 포함되어 있지 않습니다. 이모지가 표시될지는 각 탐색기에 번들된 글꼴에 따라 달라지며, Unity 탐색기에는 없으므로 문자가 빈 상자로 보이거나 완전히 사라집니다. 화살표나 박스 그리기 문자 같은 다른 장식용 유니코드에도 같은 주의가 필요합니다.

그림형 레이블이 필요하면 대신 아트를 포함하세요: 작은 `UiEntity` 텍스트 옆에 이미지를 배치하고, 다음을 사용하세요: `uiBackground` 를 사용한 `텍스처`. 다음을 참조하세요 [UI 배경](/docs/creator/content-creator-ko/sdk7/2d-ui/ui_background/).

```ts
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
	<UiEntity
		uiTransform={{
			minWidth: 100,
			maxWidth: 300,
			height: 'auto',
			alignSelf: 'center',
			padding: 10,
		}}
		uiBackground={{
			color: Color4.Red(),
		}}
		uiText={{
			value: 'Hello world!',
			fontSize: 18,
		}}
	/>
)
```

## 반응형 텍스트 크기

다음을 사용하세요 `scaleFontSize()` 함수는 플레이어 화면 크기에 맞게 조정되는 글꼴 값을 제공합니다. 다음을 설정할 때 `fontSize` 텍스트 UI 엔터티의 속성에는 단일 숫자 대신 이 함수를 전달하세요.

```ts
import { scaleFontSize } from '@dcl/sdk/react-ecs'
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
  <UiEntity
    uiTransform={{
      width: 'auto',
      height: 'auto',
      alignSelf: 'center',
      padding: 10,
    }}
    uiText={{
      value: 'Hello world!',
      fontSize: scaleFontSize(15)
    }}
  />
)
```

다음의 `scaleFontSize()` 함수에는 두 개의 매개변수가 필요합니다:

* `fontSize`: 사용할 기본 글꼴 크기입니다.
* `scaleUnit` *(선택 사항)*: 스케일링 계수입니다. 텍스트를 화면 너비 또는 높이에 따라 조정할지, 그리고 얼마나 조정할지 결정합니다. 기본값: `0.39` (숫자이므로 다음을 기준으로 해석됩니다: *width*). 값은 다음과 같을 수 있습니다:
  * *숫자*: 단순 숫자이며, 이 경우 다음을 기준으로 해석됩니다: *width*
  * *다음으로 끝나는 문자열 **vw***: 숫자를 뷰 너비에 상대적으로 만듭니다. 예: `"0.8vw"`
  * *다음으로 끝나는 문자열 **vh***: 숫자를 뷰 높이에 상대적으로 만듭니다. 예: `"0.8vh"`


> [!NOTE]
> **💡 팁**: 이 함수는 CSS의 `calc()` 함수를 사용하세요.


의 값은 `scaleUnit` 창 너비 또는 높이의 백분율입니다. 따라서 `scaleUnit` 의 `"100vw"` 은 화면 너비의 100%이고, 값 `"0.5vw"` 은 화면 너비의 0.5%입니다.

다음 공식은 `scaleFontSize()` 다음을 따릅니다: 화면 너비 또는 높이에 `scaleUnit`을 곱한 뒤, 거기에 다음을 더합니다 `fontSize` 첫 번째 매개변수로 전달된 값

```ts
final font = fontSize + (screen width * scaleUnit / 100)
```

예를 들어, 아래 코드 조각은 `scaleUnit` 값 0.8을 사용합니다. 화면 너비가 *1280px*이면, 텍스트 크기는 **25.24**이 되며, 다음 방정식을 따릅니다 `15 + (1280 * 0.8 / 100)`.


> [!WARNING]
> **📔 참고**: `scaleFontSize()` 는 다음을 반환합니다 **숫자**가상 *픽셀 — 여전히 다음 값에 곱해집니다* UI 배율 [로 그려지기 전에. 위 예에서 기본](/docs/creator/content-creator-ko/sdk7/2d-ui/onscreen-ui/#screen-virtual-scale) 가상 화면과 너비 1280px의 캔버스에서는 배율이 `1920x1080` 이므로 텍스트는 약 16.8px로 렌더링됩니다. 가상 화면을 무시하고 캔버스를 기준으로 직접 측정되는 글꼴 크기를 원하면 `1280 / 1920 = 0.667`은 `vw`/`vh` **문자열** 을 바로 `fontSize` 대신 전달하세요 — 예: `fontSize: '1.8vh'`.


```ts
import { scaleFontSize } from '@dcl/sdk/react-ecs'
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'

export const uiMenu = () => (
  <UiEntity
    uiTransform={{
      width: 'auto',
      height: 'auto',
      alignSelf: 'center',
      padding: 10,
    }}
    uiText={{
      value: 'Hello world!',
      fontSize: scaleFontSize(15, 0.8)
    }}
  />
)
```


> [!NOTE]
> **💡 팁**: 테스트할 다양한 화면 크기가 없다면 Web Explorer를 사용하고 미리보기를 실행한 창 크기를 조정해 보세요. 창을 바꿀 때마다 텍스트가 즉시 조정됩니다.


의 대안으로 `scaleFontSize()`, 다음을 전달하세요 `vw`/`vh` 문자열을 직접 다음으로 `fontSize` — 예: `fontSize: '1.8vh'` — 이는 캔버스를 기준으로 텍스트 크기를 정하고 UI 배율은 완전히 무시합니다. 하지 마세요 **아니라** 글꼴 크기에 직접 계산한 계수를 곱하지 마세요 `UiCanvasInformation`: SDK가 이미 하나를 적용합니다. 다음을 참조하세요 [반응형 UI 크기](/docs/creator/content-creator-ko/sdk7/2d-ui/ui-positioning/#responsive-ui-size).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/2d-ui/ui_text.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
