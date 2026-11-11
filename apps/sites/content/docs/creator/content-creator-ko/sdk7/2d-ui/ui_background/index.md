> UI 엔티티의 배경과 테두리를 설정합니다.

# UI 배경

다음 속성들은 UI 엔티티의 배경과 테두리를 설정하는 데 사용됩니다.

## 배경

하나는 `uiBackground` 컴포넌트는 엔티티의 영역에 색상이나 텍스처를 적용합니다. 이는 엔티티의 `uiTransform`.

다음 필드를 구성할 수 있으며, 모두 선택 사항입니다:

* `색상` : 엔티티에 사용할 색상으로,  [Color4](/docs/creator/content-creator-ko/sdk7/3d/color-types/) 값.


> [!NOTE]
> **💡 팁** : 엔티티를 반투명하게 만들려면 `Color4` 의 4번째 값을 1보다 작게 설정하세요.


* `텍스처` : 엔티티에 표시할 텍스처입니다. 텍스처에 관한 다양한 매개변수를 담은 객체를 사용합니다. 동일한 속성은 [3D 엔티티의 머티리얼에서의 텍스처와 동일하게 사용할 수 있습니다.](/docs/creator/content-creator-ko/sdk7/3d/materials/#using-textures).

  * `src` : 텍스처로 사용할 이미지 파일의 경로입니다. (string)

  * `filterMode`: *(선택 사항)* 텍스처의 픽셀이 렌더링될 때 어떻게 늘어나거나 압축되는지를 결정합니다. . 참조: [텍스처 스케일링](/docs/creator/content-creator-ko/sdk7/3d/materials/#texture-scaling). (FilterMode = 'point' | 'bi-linear' | 'tri-linear')

  * `wrapMode`: *(선택 사항)* 텍스처가 엔티티에 어떻게 타일링되는지를 결정합니다. 이는 `TextureWrapMode` 열거형의 값을 사용합니다. \[Texture Wrapping]\(([문서 보기](https://github.com/decentraland/docs/tree/main/creator/sdk7/README.md)). (WrapMode = 'repeat' | 'clamp' | 'mirror')

  > 팁: 두 속성 모두를 `텍스처` 및 `색상` 하나의 `uiBackground` 컴포넌트에 결합하여 색조가 입혀진 텍스처를 만들 수 있습니다.
* `textureMode` : 텍스처를 적용한 엔티티의 크기에 맞게 텍스처를 어떻게 조정할지 선택합니다. (TextureMode = 'nine-slices' | 'center' | 'stretch')다음 값을 지원하는 enum입니다:
  * `중심` : 텍스처는 늘어나지 않으며, 엔티티 중앙에 배치되고 엔티티 크기에 따라 일부가 잘릴 수 있습니다.
  * `stretch` : 텍스처가 엔티티의 전체 표면에 맞도록 늘어납니다.
  * `nine-slices` : 텍스처의 일부가 엔티티의 전체 표면에 맞게 늘어나고, 여백은 늘어나지 않습니다. 참조: [나인-슬라이스 텍스처](#nine-slice-textures).
* `avatarTexture` : 아바타 ID를 기반으로 아바타 프로필 썸네일을 표시합니다. \[Avatar Portraits]\(([문서 보기](https://github.com/decentraland/docs/tree/main/creator/sdk7/README.md)).
* `textureSlices` : 나인-슬라이스 텍스처 모드를 사용할 때 사용할 여백을 결정합니다. 참조: [나인-슬라이스 텍스처](#nine-slice-textures). 이미지의 전체 너비 또는 높이의 비율로 1보다 작은 숫자를 설정하세요.

단순 색상:

***ui.tsx 파일:***

```tsx
import { ReactEcs, UiEntity } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
  <UiEntity
    uiTransform={{
      width: 700,
      height: 400
    }}
    uiBackground={{
		color: Color4.create(0.5, 0.8, 0.1, 0.6)
	}}
  />
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
> **📔 참고**: 이 페이지의 다음 스니펫들은 모두 다음이 있다고 가정합니다: `.ts` 위와 유사한 것이 실행 중이며, `ReactEcsRenderer.setUiRenderer()` 함수입니다.


반복 텍스처 패턴:

```ts
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'

export const uiMenu = () => (
  <UiEntity
    uiTransform={{
      width: 700,
      height: 400
    }}
    uiBackground={{
		textureMode: 'center',
		texture: {
			src: "images/brick-wall-texture.png",
			wrapMode: 'repeat'
		}
	}}
  />
)
```

## 테두리

몇 가지 속성은 UI 엔티티 주변에 테두리를 설정하는 데 사용됩니다. 이러한 속성은 `uiTransform` 컴포넌트에 존재합니다. 각각은 테두리의 모든 면에 대해 단일 값을 설정하거나, 면마다 다른 값을 설정할 수 있게 합니다.

* `borderColor` : 엔티티에 사용할 색상으로,  [Color4](/docs/creator/content-creator-ko/sdk7/3d/color-types/) 값.
* `borderWidth` : 테두리의 너비로, 픽셀 단위의 숫자입니다. 퍼센트 값도 지원하며, 예를 들어 `borderWidth: '2%'` 는 테두리 너비를 엔티티 너비의 2%로 설정합니다.
* `borderRadius` : 이 속성을 사용하면 엔티티의 모서리에 둥근 테두리를 줄 수 있습니다. 모서리의 반지름을 픽셀 단위로 설정합니다.

```ts
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
  <UiEntity
    uiTransform={{
      width: 700,
      height: 400,
      borderColor: Color4.Red(),
      borderWidth: 4,
      borderRadius: 10
    }}
  />
)
```

`borderWidth`, `borderColor` 및 `borderRadius` 또한 엔티티의 각 면마다 다른 값으로 설정할 수 있습니다.

```ts
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
  <UiEntity
    uiTransform={{
      width: 700,
      height: 400,
      borderColor: { top: Color4.White(), left: Color4.Red(), right: Color4.Blue(), bottom: Color4.Gray() },
      borderRadius: { topLeft: 20, topRight: 20, bottomLeft: 20, bottomRight:0 },
      borderWidth: { top: 3, left: 2, right: 3, bottom: 4 }
    }}
  />
)
```

## 투명도

다음을 사용하세요 `opacity` 의 `uiTransform` 속성은 `UiEntity` 엔티티와 그 모든 자식에 투명도를 추가합니다. opacity 속성은 0에서 1 사이의 값이며, 0은 완전히 투명하고 1은 완전히 불투명합니다.

```ts
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
  <UiEntity
    uiTransform={{
      width: 700,
      height: 400,
      opacity: 0.7
    }}
    uiBackground={{ color: Color4.Green() }}
  >
    <UiEntity
        uiTransform={{
          width: 100,
          height: 30,
        }}
        uiText={{
          value: "이 텍스트도 투명합니다",
          fontSize: 40
        }}
      />
   </UiEntity>
)
```

opacity 값은 UiEntity의 모든 자식에 영향을 주며, 배경색, 텍스트 색상, 배경 이미지에 투명도를 적용합니다. 부모와 자식 모두 opacity 값을 가지면, 자식의 최종 투명도는 자신의 값과 부모 값의 곱입니다.

```ts
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
  <UiEntity
    uiTransform={{
      width: 700,
      height: 400,
      opacity: 0.7
    }}
    uiBackground={{ color: Color4.Green() }}
  >
    <UiEntity
      uiTransform={{
        width: 100,
        height: 30,
        opacity: 0.7
      }}
      uiText={{
        value: "이 텍스트는 훨씬 더 투명합니다",
        fontSize: 40
      }}
    />
  </UiEntity>
)
```

## 나인-슬라이스 텍스처

다음을 사용할 수 있습니다: [9-슬라이스 스케일링](https://en.wikipedia.org/wiki/9-slice_scaling) 를 사용하여 모서리와 여백이 고르지 않게 늘어나지 않도록 하세요.

이 널리 쓰이는 기법은 이미지를 9개의 세그먼트로 나누고, 여백과 모서리의 비율을 보존하기 위해 서로 다른 방식으로 늘립니다. 예를 들어, 어떤 크기에도 쉽게 맞는 둥근 모서리 배경을 정의할 때 사용할 수 있습니다. 다음 이미지를 참고하세요 (출처: [Wikipedia](https://en.wikipedia.org/wiki/9-slice_scaling#/media/File:Traditional_scaling_vs_9-slice_scaling.svg)):

![](/docs/_remote/e03c4d135bed8010ce6253a3.webp)

이 이미지에서는 원본 텍스처(왼쪽 위)와 이를 전통적인 방식으로 스케일링한 결과(오른쪽 위)를 볼 수 있습니다. 모서리가 어떻게 변형되는지 주목하세요. 그 아래에는 텍스처가 9개 슬라이스로 분할된 모습(왼쪽 아래)과, 9-슬라이스 방법에 따라 이미지를 늘린 결과(오른쪽 아래)가 보입니다.

위 이미지를 참고하여 각 세그먼트가 어떻게 영향을 받는지 살펴보겠습니다.

* 세그먼트 5는 x축과 y축 모두에서 완전히 늘어나는 유일한 부분입니다.
* 세그먼트 1, 3, 7, 9(모서리)는 전혀 늘어나지 않습니다.
* 세그먼트 2와 8은 가로로만 늘어납니다
* 세그먼트 4와 6은 세로로만 늘어납니다.

엔티티에 나인-슬라이스 늘리기를 사용하려면 `textureMode` 을(를) `'nine-slices'`로 설정하세요. 또한 선택적으로 각 면의 여백 너비를 `textureSlices`.

```ts
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'

export const uiMenu = () => (
  <UiEntity
    uiTransform={{ width: 700, height: 400 }}
    uiBackground={{
      textureMode: 'nine-slices',
      texture: {
        src: 'images/rounded_alpha_square.png'
      },
      textureSlices: {
        top: 0.2,
        bottom: 0.2,
        left: 0.2,
        right: 0.2
      }
	}}
  />
)
```

## 텍스처 UV

다음을 사용하세요 `uvs` 의 `uiBackground` 컴포넌트 속성을 사용하여 텍스처의 특정 영역을 표시할 수 있습니다. 이는 스프라이트 시트에서 개별 스프라이트를 선택하거나, 이미지를 회전시키는 데 유용합니다.

해당 `uvs` 필드는 8개의 숫자 배열을 사용하며, 텍스처 영역의 네 모서리에 대한 4쌍의 UV 좌표를 나타냅니다. 순서는 다음과 같습니다: **왼쪽 아래**, **왼쪽 위**, **오른쪽 위**, **오른쪽 아래**. 각 값은 0에서 1 사이이며, `(0, 0)` 는 이미지의 왼쪽 아래 모서리이고 `(1, 1)` 는 오른쪽 위입니다.


> [!NOTE]
> **💡 팁** : 사용자 지정 `uvs`, 다음을 설정하세요 `textureMode` 을(를) `'stretch'` 를 사용할 때 선택된 영역이 엔티티의 영역을 채우도록 합니다.


### 스프라이트 시트의 스프라이트

더 큰 이미지의 일부를 표시하려면(예: 여러 장의 카드가 들어 있는 스프라이트 시트에서 한 장의 카드), `uvs` 을 해당 영역의 좌표로 설정하세요.

```ts
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'

// 텍스처의 왼쪽 절반 표시 (예: 2열 시트의 첫 번째 카드)
export const uiMenu = () => (
  <UiEntity
    uiTransform={{ width: 200, height: 300 }}
    uiBackground={{
      textureMode: 'stretch',
      texture: { src: 'images/card-atlas.png' },
      uvs: [
        // 왼쪽 아래, 왼쪽 위, 오른쪽 위, 오른쪽 아래
        0, 0,
        0, 1,
        0.5, 1,
        0.5, 0
      ]
    }}
  />
)
```

프레임이 격자로 배치된 스프라이트 시트의 경우(예: 4열 2행 시트), 원하는 프레임의 열과 행을 기준으로 UV를 계산하세요:

```ts
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'

// 격자 스프라이트 시트에서 단일 프레임 선택
function getFrameUVs(col: number, row: number, totalCols: number, totalRows: number): number[] {
  const stepU = 1 / totalCols
  const stepV = 1 / totalRows
  const left = col * stepU
  const right = (col + 1) * stepU
  const top = 1 - row * stepV
  const bottom = 1 - (row + 1) * stepV
  return [
    left, bottom,
    left, top,
    right, top,
    right, bottom
  ]
}

// 4x2 스프라이트 시트의 2열 0행 표시
export const uiMenu = () => (
  <UiEntity
    uiTransform={{ width: 128, height: 128 }}
    uiBackground={{
      textureMode: 'stretch',
      texture: { src: 'images/spritesheet.png' },
      uvs: getFrameUVs(2, 0, 4, 2)
    }}
  />
)
```

### UV로 이미지를 회전시키기

UV 좌표에 2D 회전을 적용하여 텍스처를 회전시킬 수 있습니다. 이는 변환 기반 회전 없이 스피너나 로딩 표시기를 만들 때 유용합니다.

```ts
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { engine } from '@dcl/sdk/ecs'

// 중심을 기준으로 2D 점 회전
function rotate2D(angle: number, x: number, y: number, cx: number, cy: number): number[] {
  const cos = Math.cos(angle)
  const sin = Math.sin(angle)
  return [
    cos * (x - cx) - sin * (y - cy) + cx,
    sin * (x - cx) + cos * (y - cy) + cy
  ]
}

// 회전된 UV 좌표 생성
function rotateUVs(angle: number): number[] {
  const uv00 = rotate2D(angle, 0, 0, 0.5, 0.5)
  const uv01 = rotate2D(angle, 0, 1, 0.5, 0.5)
  const uv11 = rotate2D(angle, 1, 1, 0.5, 0.5)
  const uv10 = rotate2D(angle, 1, 0, 0.5, 0.5)
  return [uv00[0], uv00[1], uv01[0], uv01[1], uv11[0], uv11[1], uv10[0], uv10[1]]
}

let spinnerAngle = 0

// 프레임마다 각도를 업데이트하는 시스템
engine.addSystem((dt: number) => {
  spinnerAngle += dt * 5
})

export const uiMenu = () => (
  <UiEntity
    uiTransform={{ width: 128, height: 128 }}
    uiBackground={{
      textureMode: 'stretch',
      texture: { src: 'images/spinner.png' },
      uvs: rotateUVs(spinnerAngle)
    }}
  />
)
```

해당 `rotateUVs` 함수는 네 개의 UV 모서리를 중심점을 기준으로 `(0.5, 0.5)` 주어진 각도(라디안)만큼 회전시킵니다. 이는 React와 유사한 렌더러에 의해 매 프레임 호출되므로 스피너가 부드럽게 업데이트됩니다.


> [!NOTE]
> **💡 팁**: 스프라이트, 애니메이션 스프라이트 시트, 스피너 등 전체 동작 예제를 [UI 애니메이션](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/ui-animations) 예제 씬에서 확인하세요.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/2d-ui/ui_background.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
