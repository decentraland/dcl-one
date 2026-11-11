> UI 엔티티의 위치, 크기, 패딩 및 기타 속성을 설정합니다.

# UI 배치

모든 종류의 UI 콘텐츠에는 다음을 사용하세요 `uiTransform` 컴포넌트로 엔터티의 정렬과 관련된 크기, 위치 및 기타 속성을 설정합니다.

해당 `uiTransform` 컴포넌트는 화면의 2D 공간에서 다음과 매우 비슷하게 작동합니다 `Transform` 컴포넌트는 씬의 3D 공간에서 작동합니다.

***ui.tsx 파일:***

```ts
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
	<UiEntity
		uiTransform={{
			width: '200px',
			height: '100px',
			justifyContent: 'center',
			alignItems: 'center',
		}}
		uiBackground={{ color: Color4.Green() }}
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


## 위치 지정 속성

UI 엔터티의 정렬은 Flexbox 정렬 모델을 기반으로 합니다. 이는 크기가 달라질 수 있는 모달 안에서 중첩된 엔터티를 동적으로 구성하는 데 매우 강력한 모델입니다.


> [!NOTE]
> **💡 팁**: Decentraland의 UI 구현은 다음을 기반으로 합니다 [Yoga](https://yogalayout.com/docs/). 다음을 읽어보세요 [이 문서](https://www.joshwcomeau.com/css/interactive-guide-to-flexbox/) Flexbox에서 사용할 수 있는 속성들을 매우 쉽게 접근할 수 있으면서도 심도 있게 다루는 내용입니다.


### 엔터티 크기

다음을 사용하세요 `width` 및 `height` 엔터티의 크기를 설정하는 데 사용합니다. 다음과 같은 값 유형을 지원합니다:

* `auto`: 크기는 내부 콘텐츠에 맞게 조정됩니다. 길이가 달라질 수 있는 텍스트에 매우 유용합니다. 값을 "auto"로 작성하세요.
* **백분율**: 부모의 측정값에 대한 백분율로 사용합니다. 값은 "%"로 끝나는 문자열로 작성하세요. 예를 들어 `10 %`.
* **픽셀**: 값을 숫자로 작성하세요.
* **화면 너비 또는 높이**: vw(view width)와 vh(view height)를 사용하면 Decentraland가 실행 중인 창 전체 크기의 일부를 나타낼 수 있습니다. 예를 들어 `10vw` 는 창 너비의 10%를 의미하며, `25vh` 는 창 높이의 25%를 의미합니다.

이러한 속성은 다음에 영향을 준다는 점에 유의하세요: **default** 해당 항목의 크기, 즉 flex grow 및 flex shrink 계산이 적용되기 전의 항목 크기입니다. 최종 크기는 부모 엔터티의 크기와 설정된 Flexbox 속성에 따라 다르게 해석될 수 있습니다.


> [!WARNING]
> **📔 참고**: 숫자와 문자열을 모두 지원하는 속성에서는 픽셀 값으로 설정하려면 숫자를 작성하세요. 이 필드를 부모의 측정값에 대한 백분율로 설정하려면 "%"로 끝나는 문자열로 값을 작성하세요. 예를 들어 `10 %`. 문자열을 다음으로 끝내서 픽셀 값을 문자열로도 설정할 수 있습니다: `px`, 예를 들면 `200px`.
> 
> * 값이 백분율로 표현되면 항상 부모 컨테이너를 기준으로 합니다. UI의 최상위 엔터티도 부모를 가집니다. 렌더러는 이를 다음으로 선택된 영역 안에 배치합니다: [`screenInset`](/docs/creator/content-creator-ko/sdk7/2d-ui/onscreen-ui/#screen-inset-area), 기본값은 기기의 안전 영역입니다. 따라서 루트 수준의 `100%` 는 해당 영역의 100%입니다. 화면보다 좁은 휴대폰에서는 그렇고, 데스크톱에서는 둘이 일치합니다. 다음을 전달하세요: `screenInset: 'none'` 전체 화면을 기준으로 한 백분율이 필요하다면.
> * 값이 픽셀로 표현되면 부모의 스케일에는 영향을 받지 않지만, **아니며** 원시 화면 픽셀도 아닙니다. 대신 다음에서 도출된 UI 스케일 팩터가 곱해집니다: [가상 화면](/docs/creator/content-creator-ko/sdk7/2d-ui/onscreen-ui/#screen-virtual-scale), 기본적으로 활성화되어 있습니다. A `width: 200` 는 "가상 화면 크기의 화면에서 200 px"을 의미하며, 다른 어떤 화면에서도 비례해서 커지거나 작아집니다.
> * 값이 다음으로 표현되면 `vh` 또는 `vw`, 이는 전체 창에 대한 백분율이며, 부모의 스케일이나 가상 화면, 또는 `screenInset`.
> 
> 에서 `auto` width/height가 작동하려면 다음 규칙이 적용됩니다:
> 
> * width/height를 “auto”로 사용하는 UiTransform은 다음을 가져야 합니다: `alignSelf`: `“center”`/`“flex-start”`/`“flex-end”` 또는 `positionType: “absolute”`
> * 자식의 UiTransform이 다음을 사용하면 `positionType: “absolute”`, 부모는 해당 크기/위치에 맞춰 조정되지 않습니다
> * 자식의 UiTransform이 어떤 위치 덮어쓰기를 사용하면, 부모는 해당 크기/위치에 맞춰 조정되지 않습니다


더 고급 방식으로 크기를 조정할 수 있는 다른 속성도 있습니다:

* `maxWidth` 및 `maxHeight`: *number* 또는 문자열(height 및 width와 같은 형식). 엔터티가 가질 수 있는 최대 크기입니다.
* `minWidth` 및 `minHeight`: *number* 또는 문자열(height 및 width와 같은 형식). 엔터티가 가질 수 있는 최소 크기입니다. 부모가 엔터티의 최소 크기를 담기에는 너무 작으면, 엔터티는 부모 밖으로 넘쳐나게 됩니다.
* `flexBasis`: 메인 축을 따라 항목의 기본 크기를 축에 독립적으로 지정하는 방식입니다. 자식의 flex basis를 설정하는 것은 부모가 flex direction: row인 컨테이너일 때 그 자식의 width를 설정하는 것과 비슷하거나, 부모가 flex direction: column인 컨테이너일 때 그 자식의 height를 설정하는 것과 비슷합니다.

```ts
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
	<UiEntity
		uiTransform={{
			alignSelf: 'center',
			alignContent: 'center',
			width: '80%',
			height: '30%',
			minWidth: 300,
			maxWidth: 2500,
			margin: { left: '10%', right: '10%' },
		}}
		uiBackground={{ color: Color4.Green() }}
	/>
)
```

### 자식 엔터티 배치하기

기본적으로 자식 엔터티는 부모의 왼쪽 상단 모서리를 기준으로 배치됩니다. 다음과 같은 속성을 사용할 수 있습니다: `justifyContent` 및 `alignItems` 이 동작을 변경하려면.


> [!NOTE]
> **💡 팁**: 다음을 참조하는 모든 속성은 *content* 메인 축을 따라 있는 엔터티를 참조합니다(다음에 의해 결정됨: `flexDirection`). 다음을 참조하는 모든 속성은


* `flexDirection`: Flex direction은 노드의 자식들이 배치되는 방향을 제어합니다. 이는 메인 축이라고도 합니다. 메인 축은 자식들이 배치되는 방향입니다. 교차 축은 메인 축에 수직인 축이며, 줄바꿈된 줄이 배치되는 축이기도 합니다. 값은 다음에서 가져옵니다: `FlexDirectionType` 타입입니다. 다음 옵션을 사용할 수 있습니다:
  * `row` (기본값)
  * `row-reverse`
  * `column`
  * `column-reverse`
* `justifyContent`: 이 속성은 컨테이너의 메인 축 안에서 자식들을 어떻게 정렬할지 설명합니다. 예를 들어, 다음이 설정된 컨테이너 안에서 자식을 수평 가운데 정렬하는 데 이 속성을 사용할 수 있습니다: `flexDirection` row로 설정된 컨테이너 안에서, 또는 다음이 설정된 컨테이너 안에서 수직으로 `flexDirection` column으로 설정된 경우. 이 속성의 값은 다음에서 가져와야 합니다: `JustifyType` 타입입니다. 가능한 값은 다음과 같습니다:
  * `flex-start` (기본값): 컨테이너의 자식들을 컨테이너 메인 축의 시작에 정렬합니다.
  * `flex-end`: 컨테이너의 자식들을 컨테이너 메인 축의 끝에 정렬합니다.
  * `중심`: 컨테이너의 자식들을 컨테이너 메인 축의 가운데에 정렬합니다.
  * `space-between`: 컨테이너의 메인 축을 따라 자식들을 균등하게 배치하고, 남는 공간을 자식들 사이에 분배합니다.
  * `space-around`: 컨테이너의 메인 축을 따라 자식들을 균등하게 배치하고, 남는 공간을 자식들 주위에 분배합니다. space-between과 비교하면 space-around를 사용하면 첫 번째 자식의 시작 부분과 마지막 자식의 끝 부분에도 공간이 분배됩니다.
  * `space-evenly`: 컨테이너의 메인 축을 따라 자식들을 균등하게 배치하고, 남는 공간을 분배하여 자식들 사이와 자식들 및 컨테이너 가장자리 사이의 간격이 모두 같아지도록 합니다.
* `alignItems`: 컨테이너의 교차 축을 따라 자식들을 어떻게 정렬할지 설명합니다. align items는 justify content와 매우 비슷하지만, 메인 축에 적용되는 대신 교차 축에 적용됩니다. 이 속성은 다음의 값을 필요로 합니다: `AlignType` 타입입니다. 다음 옵션을 사용할 수 있습니다:
  * `stretch`: (기본값) 컨테이너의 자식들을 컨테이너 교차 축의 높이에 맞게 늘립니다.
  * `flex-start`: 컨테이너의 자식들을 컨테이너 교차 축의 시작에 정렬합니다.
  * `flex-end`: 컨테이너의 자식들을 컨테이너 교차 축의 끝에 정렬합니다.
  * `중심`: 컨테이너의 자식들을 컨테이너 교차 축의 가운데에 정렬합니다.
  * `baseline`: 컨테이너의 자식들을 공통 기준선에 따라 정렬합니다. 개별 자식은 부모의 기준 기준선으로 설정할 수 있습니다.
* `alignSelf`: align self는 다음과 동일한 옵션과 효과를 가집니다 `alignItems` 하지만 컨테이너 내의 자식들에게 영향을 주는 대신, 이 속성을 단일 자식에 적용하여 부모 안에서의 정렬을 변경할 수 있습니다. align self는 부모가 align items로 설정한 어떤 옵션이든 덮어씁니다. 값은 다음에서 가져옵니다: `AlignType`, 참조: `alignItems` 이러한 옵션의 자세한 내용은 위를 참조하세요.
* `alignContent`: align content는 교차 축을 따라 줄이 어떻게 분배되는지를 정의합니다. 이는 항목이 다음을 사용하여 여러 줄로 줄바꿈될 때만 효과가 있습니다: `flexWrap`. 값은 다음에서 가져옵니다: `AlignType` 타입입니다. 다음 옵션을 사용할 수 있습니다:
  * `flex-start`: (기본값) 줄바꿈된 줄을 컨테이너 교차 축의 시작에 정렬합니다.
  * `flex-end`: 줄바꿈된 줄을 컨테이너 교차 축의 끝에 정렬합니다.
  * `stretch`: 줄바꿈된 줄을 컨테이너 교차 축의 높이에 맞게 늘립니다.
  * `중심`: 줄바꿈된 줄을 컨테이너 교차 축의 가운데에 정렬합니다.
  * `space-between`: 줄바꿈된 줄을 컨테이너 메인 축을 따라 균등하게 배치하고, 남는 공간을 줄들 사이에 분배합니다.
  * `space-around`: 줄바꿈된 줄을 컨테이너 메인 축을 따라 균등하게 배치하고, 남는 공간을 줄들 주위에 분배합니다. space-between과 비교하면 space-around를 사용하면 첫 줄의 시작 부분과 마지막 줄의 끝 부분에도 공간이 분배됩니다.
* `flexGrow`: 컨테이너 내부의 여분 공간을 메인 축을 따라 자식들에게 어떻게 분배할지 설명합니다. 컨테이너는 자식들을 배치한 후, 자식들이 지정한 flex grow 값에 따라 남는 공간을 분배합니다. flex grow는 0 이상의 ցանկացած 부동소수점 값을 허용하며, 0이 기본값입니다. 컨테이너는 자식의 flex grow 값에 가중치를 두어 남는 공간을 자식들 사이에 분배합니다.
* `flexShrink`: 자식들의 총 크기가 메인 축에서 컨테이너 크기를 초과하는 경우, 메인 축을 따라 자식들을 어떻게 줄일지 설명합니다. flex shrink는 flex grow와 매우 비슷하며, 넘치는 크기를 음수의 남는 공간으로 간주하면 같은 방식으로 생각할 수 있습니다. 이 두 속성은 자식들이 필요에 따라 커지고 줄어들 수 있게 해 주므로 함께 잘 작동합니다. flex shrink는 0 이상의 ցանկացած 부동소수점 값을 허용하며, 1이 기본값입니다. 컨테이너는 자식의 flex shrink 값에 가중치를 두어 자식들을 줄입니다.
* `overflow`: 엔터티의 자식 크기가 부모를 초과할 때 어떻게 되는지 결정합니다. 다음의 값을 사용합니다: `OverflowType` 타입으로 해석할 수 있습니다.
  * `hidden`: 넘치는 엔터티는 보이지 않게 됩니다.
  * `visible`: 넘치는 엔터티는 부모의 여백 밖으로 벗어납니다.
  * `scroll`: 해당 영역을 스크롤 가능하게 만들어, 플레이어가 넘치는 콘텐츠를 스크롤할 수 있게 합니다. 다음을 참조하세요: [스크롤 가능한 컨테이너](#scrollable-containers) 를 참조하세요.
* `flexWrap`: flex wrap 속성은 컨테이너에 설정되며, 자식이 메인 축을 따라 컨테이너 크기를 초과할 때 어떻게 되는지 제어합니다. 기본적으로 자식은 필요하면 메인 축을 따라 여러 줄로 감싸집니다. 다음으로 줄바꿈이 비활성화되면 `nowrap`, 자식들은 하나의 줄로 강제됩니다(이로 인해 엔터티가 작아질 수 있습니다). wrap reverse는 wrap과 동일하게 동작하지만 줄의 순서가 반대입니다. 이 속성의 값은 다음에서 가져옵니다: `FlexWrapType` 타입으로 해석할 수 있습니다.
  * `wrap`
  * `nowrap`
  * `wrap-reverse`

### 마진 및 패딩

* `margin`: 이 속성은 노드 바깥쪽의 간격에 영향을 줍니다. margin이 있는 노드는 부모 경계에서 자신을 떨어뜨릴 뿐만 아니라 형제들의 위치도 밀어냅니다. 부모가 자동 크기일 경우 노드의 margin은 부모의 전체 크기에 기여합니다. 엔터티와 부모의 margin 사이의 공간을 설정합니다. 기대되는 값은 다음 속성을 포함하는 객체입니다: `상단`, `left`, `하단`, 그리고 `right`.
* `padding`: 이 속성은 적용된 노드의 크기에 영향을 줍니다. Yoga의 padding은 box-sizing: border-box;가 설정된 것처럼 동작합니다. 즉, 명시적 크기가 설정된 엔터티에서는 padding이 전체 크기에 추가되지 않습니다. 자동 크기 노드의 경우 padding은 노드의 크기를 증가시키며 자식들의 위치도 밀어냅니다. 기대되는 값은 다음 속성을 포함하는 객체입니다: `상단`, `left`, `하단`, 그리고 `right`.

### 위치 미세 조정

Flexbox에서는 엔터티 위치가 주로 부모-자식 관계와 부모 및 자식에 어떤 배치 속성이 설정되었는지에 따라 결정됩니다. 따라서 다음을 설정할 필요가 없는 경우가 많습니다: `위치` 속성 자체입니다. 하지만 이를 조정하거나, Flexbox의 일반 흐름을 완전히 덮어쓰고 절대 위치를 설정하고 싶다면, 관련 속성은 다음과 같습니다:

* `positionType`: 엔터티가 어떻게 배치되는지 정의합니다. 값은 다음에서 가져옵니다: `PositionType` 열거형을 사용합니다.
  * `relative`: (기본값) 기본적으로 엔터티는 상대적으로 배치됩니다. 이는 엔터티가 레이아웃의 일반 흐름에 따라 배치된 다음, 다음 값들을 기준으로 그 위치에서 상대적으로 오프셋된다는 뜻입니다: `상단`, `right`, `하단`, 그리고 `left`. 이 오프셋은 형제나 부모 엔터티의 위치에는 영향을 주지 않습니다.
  * `absolute`: 절대 위치로 배치되면 엔터티는 일반 레이아웃 흐름에 참여하지 않습니다. 대신 형제들과 독립적으로 배치됩니다. 위치는 다음을 기준으로 결정됩니다: `상단`, `right`, `하단`, 그리고 `left` 값을 참조하세요.
* `위치`: 위치 값은 `상단`, `right`, `하단`, 그리고 `left` 다음에 따라 다르게 동작합니다: `positionType`. 상대 엔터티의 경우 지정된 방향으로 엔터티의 위치를 오프셋합니다. 그러나 절대 엔터티의 경우 이 속성들은 엔터티의 변이 부모의 같은 변에서 얼마나 떨어지는지를 지정합니다. 기대되는 값은 다음 속성을 포함하는 객체입니다: `상단`, `left`, `하단`, 그리고 `right`.


> [!WARNING]
> **📔 참고** : 다음에 대한 양수 값은 `상단` 또는 `left` 부모의 같은 가장자리에서 안쪽으로의 거리를 측정합니다. 예: 상단과 왼쪽에서 부모를 기준으로 20픽셀 여백을 남기고 component를 배치하려면 다음과 같이 설정하세요: `위치` 을(를) `{ top: 20, left: 20 }`.


### 가시성

* `display`: 엔터티가 보이는지 아닌지를 결정합니다. 엔터티를 보이지 않게 하려면 다음을 설정하세요: `display` 을(를) `none`.

### Z 인덱스

해당 `zIndex` 의 속성 `UiEntity` 엔터티가 렌더링되는 순서를 결정합니다. 더 높은 `zIndex` 를 가진 엔터티는 더 낮은 `zIndex`. 기본 `zIndex` 는 0입니다.

```ts
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
	<UiEntity
		uiTransform={{
			zIndex: 4
		}}
		uiBackground={{ color: Color4.Green() }}
	/>
)
```


> [!WARNING]
> **📔 참고** : `zIndex` 속성은 직접 형제 요소 간의 순서만 정하며, 레이아웃 트리의 다른 부분 위에 엔터티를 렌더링하는 데 사용할 수 없습니다. html/CSS 용어로, 모든 DCL UI 요소는 새로운 [스태킹 컨텍스트](https://web.dev/learn/css/z-index#stacking_context). 다음으로 추가된 독립적인 UI 모듈의 순서를 정하려면 `addUiRenderer()` 서로 비교하여, 다음을 사용하세요: `zIndex` 의 [렌더러 옵션](/docs/creator/content-creator-ko/sdk7/2d-ui/onscreen-ui/#stacking-order-between-ui-modules) 대신 사용하세요.
> 
> 맵, 채팅 등을 포함한 기본 Decentraland UI는 항상 다른 모든 UI 요소 위에 렌더링됩니다.


## 스크롤 가능한 컨테이너

UI 엔터티의 콘텐츠가 할당된 크기보다 많을 때는 다음을 설정하여 해당 영역을 스크롤 가능하게 만들 수 있습니다: `overflow` 을(를) `scroll` 엔터티의 `uiTransform`. 그러면 플레이어는 드래그하거나 마우스 휠을 사용해 콘텐츠를 스크롤할 수 있습니다.

스크롤 가능한 컨테이너를 만들려면 부모 엔터티는 고정 크기를 가져야 하며(다음을 사용하여 `width` 및 `height`), 자식들은 그 크기를 초과해야 합니다.

```ts
import { UiEntity, Label, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const scrollableMenu = () => (
	<UiEntity
		uiTransform={{
			width: 300,
			height: 400,
			overflow: 'scroll',
			flexDirection: 'column',
		}}
		uiBackground={{ color: Color4.fromHexString('#1a1a1a') }}
	>
		{/* 이 자식들은 부모의 400px 높이를 초과하여, 영역을 스크롤 가능하게 만듭니다 */}
		<Label value="항목 1" fontSize={18} uiTransform={{ width: '100%', height: 80 }} />
		<Label value="항목 2" fontSize={18} uiTransform={{ width: '100%', height: 80 }} />
		<Label value="항목 3" fontSize={18} uiTransform={{ width: '100%', height: 80 }} />
		<Label value="항목 4" fontSize={18} uiTransform={{ width: '100%', height: 80 }} />
		<Label value="항목 5" fontSize={18} uiTransform={{ width: '100%', height: 80 }} />
		<Label value="항목 6" fontSize={18} uiTransform={{ width: '100%', height: 80 }} />
		<Label value="항목 7" fontSize={18} uiTransform={{ width: '100%', height: 80 }} />
	</UiEntity>
)
```

이 기능은 긴 목록, 인벤토리, 채팅 로그, 리더보드 또는 콘텐츠가 화면에 들어가는 것보다 더 커질 수 있는 어떤 패널을 만들 때 유용합니다.

다른 UI 레이아웃 안에 스크롤 가능한 컨테이너를 중첩할 수도 있습니다. 예를 들어, 고정 헤더와 스크롤 가능한 본문이 있는 대화상자 모달:

```ts
import { UiEntity, Label, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const dialogWithScroll = () => (
	<UiEntity
		uiTransform={{
			width: 400,
			height: 500,
			flexDirection: 'column',
		}}
		uiBackground={{ color: Color4.fromHexString('#2a2a2a') }}
	>
		{/* 고정 헤더 */}
		<Label
			value="리더보드"
			fontSize={22}
			uiTransform={{ width: '100%', height: 60 }}
		/>

		{/* 스크롤 가능한 본문 */}
		<UiEntity
			uiTransform={{
				width: '100%',
				flexGrow: 1,
				overflow: 'scroll',
				flexDirection: 'column',
			}}
		>
			<Label value="1. Alice - 9500" fontSize={16} uiTransform={{ width: '100%', height: 50 }} />
			<Label value="2. Bob - 8200" fontSize={16} uiTransform={{ width: '100%', height: 50 }} />
			<Label value="3. Charlie - 7800" fontSize={16} uiTransform={{ width: '100%', height: 50 }} />
			<Label value="4. Diana - 6100" fontSize={16} uiTransform={{ width: '100%', height: 50 }} />
			<Label value="5. Eve - 5500" fontSize={16} uiTransform={{ width: '100%', height: 50 }} />
			<Label value="6. Frank - 4900" fontSize={16} uiTransform={{ width: '100%', height: 50 }} />
			<Label value="7. Grace - 4200" fontSize={16} uiTransform={{ width: '100%', height: 50 }} />
			<Label value="8. Hank - 3800" fontSize={16} uiTransform={{ width: '100%', height: 50 }} />
			<Label value="9. Ivy - 3100" fontSize={16} uiTransform={{ width: '100%', height: 50 }} />
			<Label value="10. Jack - 2700" fontSize={16} uiTransform={{ width: '100%', height: 50 }} />
		</UiEntity>
	</UiEntity>
)
```


> [!NOTE]
> **💡 팁**: 다음을 사용하세요 `flexGrow: 1` 스크롤 가능한 엔터티에 설정하여 부모의 남은 공간을 채우게 하면, 헤더나 푸터 같은 다른 형제 요소의 크기가 바뀌어도 그에 맞게 조정됩니다.


## 반응형 UI 크기

화면 크기가 다른 플레이어는 UI 레이아웃을 다르게 볼 수 있습니다. 픽셀 값은 다음을 기준으로 스케일링됩니다: [가상 화면](/docs/creator/content-creator-ko/sdk7/2d-ui/onscreen-ui/#screen-virtual-scale) 이를 대신 계산해 주므로, 같은 UI가 해상도가 달라도 비율을 유지합니다 — **직접 스케일 팩터를 계산할 필요가 없습니다**, 그렇게 하면 스케일링이 두 번 적용됩니다.


> [!WARNING]
> **📔 참고**: `devicePixelRatio` UI 레이아웃에 관여하지 않습니다. 이는 디스플레이 밀도 힌트일 뿐입니다 — 텍스처의 1x, 2x 또는 3x 버전을 선택하는 데 유용하며, 그 외에는 아무것도 아닙니다. 씬이 이전 SDK 버전에서 크기 설정되었다면, 픽셀 크기의 UI가 고밀도(retina 및 모바일) 화면에서 최대 2\~3배 더 크게 렌더링될 수 있으니, 수동으로 조정한 부분은 다시 확인하세요.



> [!CAUTION]
> **📔 직접 계산한 스케일 팩터를 제거하세요.** 씬이 다음에서 계산한 팩터로 크기를 곱한다면 `UiCanvasInformation` — 일반적으로 `Math.min(width / 1920, height / 1080)` — 그 곱셈은 제거하세요. 이는 이제 SDK가 기본적으로 적용하는 것과 동일한 팩터이므로, 둘 다 유지하면 UI가 화면 크기에 따라 제곱적으로 커집니다. 자신의 팩터만 사용하고 싶다면 가상 화면을 다음으로 비활성화하세요: `setUiRenderer(ui, { virtualWidth: 0, virtualHeight: 0 })`.


`UiCanvasInformation`, 씬의 루트 엔터티에 기본으로 추가되는 것은 스케일링으로 표현할 수 없는 레이아웃 결정 — 좁은 화면에서의 다른 대화상자 배치, 텍스처 해상도 선택 — 에는 여전히 적절한 도구입니다. `devicePixelRatio`, 또는 인셋 영역을 직접 읽는 것입니다. 크기 조정에는 적절한 도구가 아닙니다.

해당 `UiCanvasInformation` 컴포넌트에는 다음 정보가 들어 있습니다:

* `height`: 픽셀 단위의 캔버스 높이
* `width`: 픽셀 단위의 캔버스 너비
* `devicePixelRatio`: 기기의 물리적 픽셀 해상도와 캔버스 픽셀의 비율입니다. 디스플레이 밀도 힌트로 유용하며, 예를 들어 텍스처의 1x, 2x 또는 3x 버전을 선택할 때 사용할 수 있습니다.
* `interactableArea`: `BorderRect` 객체로, 씬 UI 요소에 지정된 영역을 자세히 설명합니다. 이 객체에는 다음 값이 포함됩니다: `상단`, `하단`, `left` 및 `right`, 각각은 탐색기 UI가 차지하는 화면 해당 가장자리의 픽셀 수입니다.
* `screenInsetArea`: `BorderRect` 객체로, 기기 또는 플랫폼 UI가 예약한 화면 인셋 영역(안전 여백)을 자세히 설명합니다. 예를 들어 노치, 상태 표시줄, 홈 인디케이터 또는 모바일의 둥근 모서리입니다. 이 객체에는 다음 값이 포함됩니다: `상단`, `하단`, `left` 및 `right`, 각각은 화면의 해당 가장자리에서 예약된 픽셀 수입니다. 데스크톱에서는 일반적으로 `0` 모든 면에서 그렇습니다.


> [!WARNING]
> **📔 참고** : Decentraland 탐색기마다 이 값은 다를 수 있습니다. 플랫폼의 전역 UI가 다를 수 있고, 사용자가 다른 전역 UI 메뉴를 펼치거나 숨길 때 값이 동적으로 바뀔 수 있기 때문입니다.


```ts
import { engine, UiCanvasInformation } from "@dcl/sdk/ecs"

export function Main() {
  const canvas = UiCanvasInformation.getOrNull(engine.RootEntity)
  if (!canvas) return
  console.log("CANVAS DIMENSIONS: ", canvas.width, canvas.height)
}
```

UI 크기에 관한 몇 가지 다른 모범 사례:

* 어떤 UI 요소의 너비나 높이가 동적이라면 다음도 함께 사용하는 것이 좋습니다: `maxWidth`, `minWidth`, `maxHeight`, 그리고 `minHeight` 해당 값들이 합리적인 범위에 머물도록 하는 매개변수입니다.
* 숫자형 font size는 다른 값과 마찬가지로 스케일되는 가상 픽셀 값입니다. 가상 화면과 관계없이 유지되도록 캔버스를 기준으로 측정한 크기를 원한다면 다음을 전달하세요: `vw`/`vh` 문자열 — 다음을 참조하세요 [반응형 텍스트 크기](/docs/creator/content-creator-ko/sdk7/2d-ui/ui_text/#responsive-text-size)


> [!NOTE]
> **💡 팁**: UI 크기의 작동 예시는 다음을 참조하세요 [`81,-2-ui-screen-inset-area`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/81,-2-ui-screen-inset-area) 테스트 씬을 보세요. 이 씬은 세 가지 모두를 실행합니다 `screenInset` 모드(`'none'`, `'device'`, `'interactable'`)을 세 개의 공존하는 렌더러로 실행하고, 실시간 `screenInsetArea` 및 `interactableArea` 값과 [`76,-10-UiCanvasInformation`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/76,-10-UiCanvasInformation), 이는 다음을 읽습니다 `UiCanvasInformation` 매 프레임마다 UI를 반응형으로 크기 조정합니다.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/2d-ui/ui-positioning.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
