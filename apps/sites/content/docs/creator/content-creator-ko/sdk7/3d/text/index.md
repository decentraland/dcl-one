> 씬에 텍스트를 추가하는 방법

# 텍스트

장면에 텍스트를 추가하려면 `TextShape` 컴포넌트입니다. 이 텍스트는 위치에

Decentraland의 텍스트는 모든 *utf8* 문자를 지원합니다. 여기에는 동양 문자와 특수 문자도 포함됩니다.


> [!WARNING]
> **📔 참고**: 이 컴포넌트는 플레이어의 2D HUD UI가 아니라 장면의 3D 공간에 존재하는 월드 내 라벨과 UI에 유용합니다.


기존 엔티티에 라벨로 텍스트를 추가하려면, 다음을 가진 두 번째 엔티티를 만드는 것이 좋습니다: `TextShape` 컴포넌트를 추가하고 다른 엔티티의 자식으로 설정합니다.

## Creator Hub의 Scene Editor를 사용하세요

월드에 텍스트를 배치하는 가장 쉬운 방법은 다음을 추가하는 것입니다: **Text** [스마트 아이템](/docs/creator/content-creator-ko/scene-editor/interactivity/smart-items/) Scene Editor에서 시각적으로 배치하는 것입니다. 그런 다음 Scene Editor의 UI에서 사용 가능한 모든 필드를 설정할 수 있습니다.

![](/docs/_remote/339b81b8af14deb7d4666a01.webp)

## 텍스트 컴포넌트 만들기

다음 예는 다음을 만드는 방법을 보여줍니다: `TextShape` 컴포넌트를 만들고 코드로 엔티티에 추가하는 방법입니다.

```ts
const sign = engine.addEntity()

Transform.create(sign, {
	position: Vector3.create(8, 1, 8),
})

TextShape.create(sign, {
	text: '안녕하세요, 세계',
})
```


> [!WARNING]
> **📔 참고**: 텍스트 컴포넌트가 있는 엔티티가 다른 엔티티의 자식이면 부모의 스케일 영향을 받습니다. 부모가 축을 따라 비균등하게 스케일되면 텍스트도 늘어나거나 압축됩니다.



> [!WARNING]
> **📔 참고**: `TextShape` 컴포넌트는 클릭할 수 없습니다. `PointerEvents` 컴포넌트는 다음을 가진 엔티티에 사용할 때 활성화되지 않습니다: `TextShape` 컴포넌트.



> [!WARNING]
> **📔 참고**: `TextShape` 다음을 통해 가져와야 합니다:
> 
> > `import { TextShape } from "@dcl/sdk/ecs"`
> 
> 다음을 참조하세요. [Imports](/docs/creator/content-creator-ko/sdk7/getting-started/coding-scenes/#imports) 에서 이를 쉽게 처리하는 방법을 확인하세요.


## 텍스트 값 변경

새 텍스트 컴포넌트를 만들 때 표시할 문자열을 할당합니다. 이 문자열은 다음에 저장됩니다: `text` 필드.

컴포넌트가 표시하는 문자열을 바꾸고 싶다면, 언제든지 다음을 변경하여 할 수 있습니다: `text` 필드 [mutable 버전](/docs/creator/content-creator-ko/sdk7/programming-patterns/mutable-data/) 의 컴포넌트.

```ts
const mutableText = TextShape.getMutable(myEntity)

mutableText.text = '새 문자열'
```

## 기본 텍스트 속성

다음 `TextShape` 컴포넌트에는 텍스트 스타일을 지정할 수 있는 여러 속성이 있습니다. 아래는 가장 일반적인 것들입니다:

* `font`: enum의 값 `Font`.
* `fontSrc`: *문자열*. 다음으로의 경로 `.ttf` 씬의 폰트 파일 경로입니다. 사용자 지정 폰트로 텍스트를 렌더링합니다. 참고: [사용자 지정 폰트](#custom-fonts).
* `fontSize`: *숫자*. font 10인 엔티티의 높이는 1미터입니다.
* `textColor`: *Color4* 객체. *Color4* 객체는 다음을 저장합니다: *RBG* 0에서 1 사이의 세 숫자로 된 색상과 *alpha* 투명도를 위한 값입니다. 참고: [색상 유형](/docs/creator/content-creator-ko/sdk7/3d/color-types/) 에서 자세한 내용을 확인하세요.

```ts
TextShape.create(sign, {
	text: '안녕하세요, 세계',
	textColor: { r: 1, g: 0, b: 0, a: 1 },
	fontSize: 5,
	font: Font.F_SANS_SERIF,
})
```

## 폰트

텍스트 쉐이프는 다음 enum의 폰트를 사용할 수 있습니다: `Font`. 이 enum에는 현재 다음 폰트가 포함되어 있습니다:

* `Font.F_SANS_SERIF`
* `Font.F_SERIF`
* `Font.F_MONOSPACE`

기본적으로 다음을 사용합니다 `Font.F_SANS_SERIF`.

```ts
TextShape.create(sign, {
	text: '안녕하세요, 세계',
	textColor: { r: 1, g: 0, b: 0, a: 1 },
	fontSize: 5,
	font: Font.F_SANS_SERIF,
})
```


> [!NOTE]
> **💡 팁**: VS Studio나 다른 IDE를 사용하는 경우, 다음을 입력하세요: `Font.` 그러면 사용 가능한 모든 폰트가 제안 목록으로 표시됩니다.


### 사용자 지정 폰트

자신만의 폰트로 텍스트를 렌더링하려면 TrueType (`.ttf`) 폰트 파일을 씬 폴더에 추가하고 다음을 설정하세요: `fontSrc` 속성을 씬의 루트 폴더를 기준으로 한 경로로 설정합니다(다음과 마찬가지로 `src` 의 `GltfContainer`). 다음을 유지하세요: `font` 또한 설정하세요: 사용자 지정 폰트를 불러올 수 없을 때마다 사용되는 대체 폰트입니다.


> [!WARNING]
> **📔 참고**: `fontSrc` 필요합니다 `@dcl/sdk` 7.31.0 이상이 필요합니다. 이전 버전에는 이 속성이 없습니다.


```ts
TextShape.create(sign, {
	text: '안녕하세요, 세계',
	fontSize: 5,
	fontSrc: 'assets/fonts/Lobster-Regular.ttf',
	font: Font.F_SANS_SERIF,
})
```

동일한 `fontSrc` 속성은 UI에 있습니다 `Label`, `Button`, `Input` 및 `Dropdown` 엔티티와 다음에도 있습니다: `uiText` 의 `UiEntity`. 자세한 내용은 [UI의 사용자 지정 폰트](/docs/creator/content-creator-ko/sdk7/2d-ui/ui_text/#custom-fonts).

사용자 지정 폰트를 사용할 때는 다음 사항을 기억하세요:

* **오직 `.ttf` 파일만 지원됩니다**. 파일은 TrueType 폰트여야 합니다. CFF 윤곽선을 가진 OpenType 폰트(보통 `.otf`), 폰트 컬렉션(`.ttc`) 및 웹 폰트(`.woff`, `.woff2`)는 허용되지 않으며, 텍스트는 대체 폰트로 렌더링됩니다 `font`.
* **폰트는 씬 안의 파일이어야 합니다**. `fontSrc` 외부 URL을 가리킬 수 없으며, 폰트 패밀리 이름(예: Google Fonts의 이름)도 지원되지 않습니다. 다음을 다운로드하세요: `.ttf` 파일을 씬 폴더에 포함하고 경로를 참조하세요. 폰트 라이선스가 재배포를 허용하는지 확인하세요. Google Fonts의 대부분은 SIL Open Font License를 사용하며, 이는 허용합니다.
* **데스크톱 클라이언트에서는 씬의 에셋이 최적화된 후에만 사용자 지정 폰트가 표시됩니다**. 씬을 게시하면 폰트는 3D 모델과 함께 최적화된 에셋 번들로 변환되며, 변환이 완료되면 사용자 지정 폰트가 표시됩니다. 그 전까지 텍스트는 대체 폰트를 사용합니다 `font`. 사용자 지정 폰트가 지원되기 전에 게시된 씬은 다시 게시할 때만 폰트가 변환됩니다. 로컬 미리보기에도 동일하게 적용됩니다. 사용자 지정 폰트는 최적화된 에셋으로 미리볼 때만 표시됩니다( **에셋 압축** Creator Hub의 옵션 또는 `npm run start -- --asset-bundles` 를 CLI에서 사용). 참고: [최적화된 에셋으로 미리보기](/docs/creator/content-creator-ko/sdk7/getting-started/preview-scene/#preview-with-optimized-assets). Bevy 웹 클라이언트는 다음을 직접 로드하므로 `.ttf` 파일을 직접 로드하므로, 로컬 미리보기에서도 사용자 지정 폰트가 바로 표시됩니다.
* **모바일에서는 지원되지 않음**. Decentraland 모바일 앱은 다음을 무시하고 `fontSrc` 대체 폰트로 렌더링합니다 `font`. 여전히 보기 좋은 대체 폰트를 선택하고, 고정된 공간에 텍스트를 맞추기 위해 사용자 지정 폰트에 의존하지 마세요. 참고: [모바일의 누락된 기능](/docs/creator/content-creator-ko/build-for-mobile/mobile-client/missing-features/).
* **폰트 파일은 최대 16MB까지 가능합니다**. 더 큰 파일은 거부되며 대체 폰트가 `font` 사용됩니다. 글리프 윤곽선이 매우 복잡한 폰트도 거부될 수 있습니다.
* **문자**: 라틴 문자(ASCII 및 Latin-1)는 미리 렌더링됩니다. 키릴 문자나 중국어처럼 폰트에 포함된 다른 문자는 장면에 처음 나타날 때 렌더링되며, 이로 인해 잠시 끊김이 생길 수 있습니다. 폰트에 포함되지 않은 문자는 내장 sans-serif 폰트로 그려집니다.
* **파일 하나는 한 개의 페이스입니다**. 하나의 `.ttf` 파일은 하나의 굵기와 스타일만 담습니다. 굵은 제목과 일반 본문을 위해 두 파일을 모두 추가하고 각 텍스트를 자체 파일에 연결하세요. 정적 폰트 파일을 사용하세요. 폰트 패밀리가 variable font로만 제공된다면, 정적 인스턴스를 사용하세요.
* **폰트는 공유됩니다**: 동일한 폰트를 사용하는 모든 텍스트 엔티티는 `fontSrc` 폰트의 단일 사본을 공유하므로, 씬 전체에서 같은 폰트를 재사용해도 한 번 사용하는 것보다 더 많은 비용이 들지 않습니다. 각 폰트는 자체 텍스처를 가지므로 여러 가지보다 소수의 폰트를 선호하세요. 각 폰트 파일은 약 700KB의 최적화된 다운로드를 추가합니다.

데스크톱 클라이언트가 사용자 지정 폰트를 사용할 수 없으면 다음의 로그에 메시지를 남깁니다: `SDK_FONTS` 콘솔의 카테고리에서 파일 이름과 이유를 표시합니다. 예를 들어, `font_src "https://example.com/font.ttf"은 외부 URL일 수 없습니다`, 또는 파일이 씬 콘텐츠의 일부가 아니거나, 씬에 아직 변환된 폰트가 없다는 뜻입니다. 이러한 메시지는 기본적으로 숨겨져 있습니다. 표시하려면 다음을 입력하세요: `/log-matrix enable SDK_FONTS Warning` 및 `/log-matrix enable SDK_FONTS Log` 를 채팅에 입력한 뒤, 다음으로 씬을 다시 불러오세요: `/reload`, 그런 다음 다음으로 로그 폴더를 여세요: `/logs`. 폰트가 변환되지 않았다면, 파일이 TrueType인지 확인한 다음 `.ttf` 16MB 미만인지 확인하고, 다시 게시하세요.

## 텍스트 정렬 및 패딩 속성

다음 `TextShape` 컴포넌트는 크기, 패딩 등을 가진 텍스트 상자를 만듭니다.

* `textAlign`: 다음에서 값을 선택하세요: `TextAlignMode` enum. 가능한 값에는 세로(*top*, *middle*, *bottom*)과 가로(*left*, *center*, *right*) 정렬의 모든 조합이 포함됩니다.
* `width`: *숫자*. 텍스트 상자의 너비입니다.
* `height`: *숫자*. 텍스트 상자의 높이입니다.
* `paddingTop`: *숫자*. 텍스트와 텍스트 상자 윤곽선 사이의 공간입니다.
* `paddingRight`: *숫자*. 텍스트와 텍스트 상자 윤곽선 사이의 공간입니다.
* `paddingBottom`: *숫자*. 텍스트와 텍스트 상자 윤곽선 사이의 공간입니다.
* `paddingLeft`: *숫자*. 텍스트와 텍스트 상자 윤곽선 사이의 공간입니다.


> [!NOTE]
> **💡 팁**: 텍스트가 공간에 떠 있는 형태라면 다음을 추가하는 것이 좋습니다: [`Billboard` 컴포넌트](/docs/creator/content-creator-ko/sdk7/3d/entity-positioning/#face-the-player) 그래야 텍스트가 회전하여 항상 플레이어를 향하고 읽기 쉬워집니다.


## 텍스트 그림자 및 윤곽선 속성

텍스트에는 기본적으로 그림자가 없지만, 다음 값을 설정해 그림자 같은 효과를 줄 수 있습니다.

* `shadowBlur`: *숫자*
* `shadowOffsetX`: *숫자*
* `shadowOffsetY`: *숫자*
* `shadowColor`: *Color3* 객체. *Color3* 객체는 다음을 저장합니다: *RBG* 0에서 1 사이의 세 숫자로 된 색상입니다.

```ts
TextShape.create(sign, {
	text: '그림자가 있는 텍스트',
	shadowColor: { r: 1, g: 0, b: 0 },
	shadowOffsetY: 1,
	shadowOffsetX: -1,
})
```

텍스트의 글자에는 둘레를 감싸는 다른 색상의 윤곽선도 넣을 수 있습니다.

* `outlineWidth`: *숫자*. 텍스트 윤곽선의 두께를 모든 방향으로 0에서 1 사이의 숫자로 나타낸 값입니다. 기본값은 *0*, 이므로 보이지 않습니다.
* `outlineColor`: *Color3* 객체. *Color3* 객체는 다음을 저장합니다: *RBG* 0에서 1 사이의 세 숫자로 된 색상입니다.

## 여러 줄

텍스트를 여러 줄에 걸쳐 표시하려면 다음을 사용하세요: `\n` 를 문자열의 일부로 사용하세요. 다음 예에는 텍스트의 두 개의 개별 줄이 있습니다:

```ts
TextShape.create(sign, {
	text: '이것은 한 줄입니다. \n이것은 다른 줄입니다',
})
```

여러 줄 텍스트와 관련된 다음 속성도 설정할 수 있습니다:

* `lineCount`: *숫자*. 텍스트 상자에 최대 몇 줄을 맞출지 지정합니다. 설정하지 않으면 줄 수에 제한이 없습니다. 다음을 `textWrapping` 속성은 *true* 두 줄 이상 사용하려면 활성화되어야 합니다.
* `lineSpacing`: *숫자*. 각 줄 사이의 간격입니다.

## 텍스트가 어느 방향을 향하는지

A `TextShape` 로컬 **-Z** 쪽에서 올바르게 읽힙니다. 하나를 만들고 회전하지 않으면 엔티티의 -Z 쪽에 서 있는 플레이어가 읽을 수 있습니다.

따라서 다음에 라벨을 붙이려면: **+Z** 모델의 면, 텍스트를 모델의 자식으로 만들고, 다음 방향으로 약간 오프셋을 주어 **+Z** 메시에 파묻히지 않게 한 뒤, Y축 기준으로 180도 회전시키세요:

```ts
// 모델의 +Z 면에 붙은 라벨
import { engine, TextShape, Transform } from '@dcl/sdk/ecs'
import { Quaternion, Vector3 } from '@dcl/sdk/math'

const label = engine.addEntity()

Transform.create(label, {
	parent: myModel,
	// +Z 방향의 작은 오프셋은 텍스트가 모델 표면에서 떨어지게 합니다
	position: Vector3.create(0, 1, 0.01),
	rotation: Quaternion.fromEulerDegrees(0, 180, 0),
})

TextShape.create(label, { text: '앞면' })
```


> [!NOTE]
> **💡 팁**: 텍스트를 플레이어가 어느 곳에 서 있든 읽을 수 있게 유지하려면 다음을 추가하세요: [Billboard](/docs/creator/content-creator-ko/sdk7/3d/entity-positioning/#face-the-player) 직접 회전시키는 대신 사용하세요.


## 텍스트를 열로 배치하기

기본 폰트에서는 공백과 탭이 문자의 너비가 서로 달라 열을 맞춰 정렬하지 못합니다. 공백으로 점수판을 채우면 이름이나 숫자 길이가 바뀌는 즉시 열이 어긋납니다.

표를 만들려면 다음을 하나 사용하세요: `TextShape` 열마다 하나씩. 각 열에 자체 엔티티를 주고 위치를 지정한 다음, 행을 다음에 넣으세요: `text` 필드 `\n`. 그러면 각 행이 모든 열에서 같은 높이에 놓입니다.

```ts
// 두 열 점수판: 이름용 TextShape 하나, 점수용 하나
const board = engine.addEntity()
Transform.create(board, { position: Vector3.create(8, 2, 8) })

const names = engine.addEntity()
Transform.create(names, { parent: board, position: Vector3.create(-1, 0, 0) })
TextShape.create(names, {
	text: 'Ana\nBruno\nChiara',
	textAlign: TextAlignMode.TAM_MIDDLE_LEFT,
	fontSize: 2,
})

const scores = engine.addEntity()
Transform.create(scores, { parent: board, position: Vector3.create(1, 0, 0) })
TextShape.create(scores, {
	text: '120\n85\n9',
	textAlign: TextAlignMode.TAM_MIDDLE_RIGHT,
	fontSize: 2,
})
```

각 열을 열 사이의 간격 쪽으로 정렬하세요: `TAM_MIDDLE_LEFT` 왼쪽 열에는, `TAM_MIDDLE_RIGHT` 오른쪽 열에는 사용하세요. 숫자는 오른쪽 정렬이 가장 잘 읽히므로, 자릿수가 맞춰집니다.

행을 업데이트하려면 다음 전체를 다시 작성하세요: `text` 각 열의 문자열입니다. 각 열의 줄 수를 같게 유지하세요. 그렇지 않으면 행이 더 이상 맞지 않습니다.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/3d/text.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
