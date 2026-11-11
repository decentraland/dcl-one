> 씬에서 플레이어를 위한 UI를 만드는 방법을 알아보세요. 예를 들어 게임 관련 정보를 표시할 때 유용합니다.

# 화면 UI

장면을 위한 UI를 만들 수 있으며, 3D 월드 공간이 아니라 화면의 고정된 2D 공간에 표시됩니다.

UI 요소는 플레이어가 장면의 LAND 구획 안에 서 있을 때만 보입니다. 인접한 장면에는 표시할 자체 UI가 있을 수 있기 때문입니다. UI의 일부는 월드 공간에서 특정 이벤트가 발생할 때 열리도록 트리거할 수도 있습니다. 예를 들어 플레이어가 특정 위치를 클릭하는 경우입니다.

중첩된 구조를 정의하여 UI를 만듭니다 `UiEntity` 객체를 JSX로 정의합니다. UI에 사용되는 구문은 다음과 매우 비슷합니다 [React](https://reactjs.org/) (웹 UI를 만들기 위한 매우 인기 있는 자바스크립트 기반 라이브러리입니다).


> [!WARNING]
> **📔 참고**: UI 구문은 `.tsx` 확장자를 가진 파일에서만 정의할 수 있습니다. `.tsx` 파일은 다음이 지원하는 모든 것을 지원하며 `.ts` 파일이 지원하는 내용에 더해 UI 구문도 지원합니다. 다음을 생성하는 것을 권장합니다 `ui.tsx` 파일을 만들고 그곳에 UI를 정의하세요. UI 렌더 메서드는 다음에서 호출해야 한다는 점을 기억하세요 `index.ts` 를 사용하여 `ReactEcsRenderer.setUiRenderer(yourUiMethodName)`를 호출하세요. 아래 예시를 참고하세요.


정적 요소로만 이루어진 단순한 UI는 HTML과 매우 비슷해 보일 수 있지만, 상태 변화에 반응하는 동적 요소를 추가하면 훨씬 더 강력한 작업을 할 수 있습니다.

기본 Decentraland 탐색기 UI에는 채팅 위젯, 지도, 그리고 다른 요소들이 포함됩니다. 이 UI 요소들은 항상 최상위 레이어에 표시되며, 장면별 UI보다 위에 있습니다. 따라서 장면에 이들과 같은 화면 영역을 차지하는 UI 요소가 있으면 가려집니다.

참조하세요 [UX 가이드라인](/docs/creator/content-creator-ko/sdk7/designing-the-experience/ux-ui-guide/) 에서 UI의 외형과 느낌을 설계하는 방법에 대한 팁을 확인하세요.


> [!NOTE]
> **📱 모바일용 설계**: 기기들은 노치, 상태 표시줄, 홈 인디케이터 및 둥근 모서리를 위해 화면 공간을 예약하며, 클라이언트는 캔버스 일부 위에 자체 컨트롤을 그립니다. 둘 다 직접 측정할 필요는 없습니다. 아래의 [`screenInset`](#screen-inset-area) 를 사용해 UI가 놓일 영역을 선택하세요. 기본값은 기기의 안전 영역이므로, 별도 작업 없이 하드웨어 여백이 피됩니다. 게시 전에 다음을 검토하세요 [모바일 안전 영역](/docs/creator/content-creator-ko/build-for-mobile/develop/safe-area/) 과 [모바일 UI 모범 사례](/docs/creator/content-creator-ko/build-for-mobile/develop/ui-best-practices/).


플레이어가 화면 오른쪽 아래 모서리에 있는 *UI 닫기* 버튼을 클릭하면 모든 UI 요소가 숨겨집니다.

## UI 렌더링하기

장면에 UI를 표시하려면 `ReactEcsRenderer.setUiRenderer()` 함수를 사용하고, 유효한 엔티티 구조를 전달하세요. 이 구조는 `.tsx` 파일에 설명되어 있습니다.

각 엔티티는 HTML과 유사한 노드로 정의되며, 각 컴포넌트에 대한 속성을 가집니다.

***ui.tsx 파일:***

```ts
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
  <UiEntity
    uiTransform={{
      width: 700,
      height: 400,
      margin: { top: '35px', left: '500px' },
    }}
    uiBackground={{ color: Color4.Red() }}
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

엔티티 구조를 정의하고 그것을 렌더링하는 작업도 하나의 동일한 명령으로 할 수 있습니다. 다음에서 `.tsx` 파일에 설명되어 있습니다.

***ui.tsx 파일:***

```tsx
import ReactEcs, { ReactEcsRenderer, UiEntity } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export function setupUI() {
  ReactEcsRenderer.setUiRenderer(() => (
    <UiEntity
      uiTransform={{
        width: 700,
        height: 400,
        margin: { top: '35px', left: '500px' },
      }}
      uiBackground={{ color: Color4.Red() }}
    />
  ), { virtualWidth: 1920, virtualHeight: 1080 })
}
```

***index.ts 파일:***

```ts
import { setupUI } from './ui'

export function main() {
    setupUI()
}
```


> [!WARNING]
> **📔 참고**: 모든 UI 요소는 같은 구조 안에 중첩되어 있어야 하며, 구조의 루트에는 하나의 단일 부모가 있어야 합니다. 다음 호출은 한 번만 할 수 있습니다 `ReactEcsRenderer.setUiRenderer()` 장면에서 한 번만.


## UI 엔티티

UI의 각 요소는 별도의 `UiEntity`로 정의되어야 하며, 이미지, 텍스트, 배경, 보이지 않는 정렬 박스 등 무엇이든 될 수 있습니다. 장면의 3D 공간에서처럼 각 `UiEntity` 는 위치, 색상 등을 부여하는 자체 컴포넌트를 가집니다.

React와 유사한 구문을 사용하면 각 컴포넌트를 `UiEntity`내의 속성으로 지정할 수 있으며, 이렇게 하면 코드가 더 짧고 읽기 쉬워집니다.

에서 사용되는 컴포넌트는 `UiEntity` 일반 엔티티에서 사용되는 컴포넌트와 다릅니다. 일반 엔티티에 UI 컴포넌트를 적용할 수도 없고, UI 엔티티에 일반 컴포넌트를 적용할 수도 없습니다.

다음 컴포넌트는 `UiEntity`:

* `uiTransform`
* `uiBackground`
* `uiText`
* `onMouseDown`
* `onMouseUp`
* `onMouseEnter`
* `onMouseLeave`
* [`uiInputBinding`](/docs/creator/content-creator-ko/sdk7/2d-ui/ui_input_binding/) — 요소가 눌려 있는 동안 입력 동작을 유지하여 사용자 지정 컨트롤을 만들 수 있습니다

HTML 태그와 마찬가지로 컴포넌트를 자체 닫기 형태로 정의하거나 하나를 다른 것 안에 중첩할 수 있습니다.


> [!CAUTION]
> **경고:** 전체 화면 레이아웃 래퍼에는 포인터 핸들러를 두지 마세요. 다음 중 하나라도 추가하면 `onMouse` 핸들러를 추가하거나 `pointerFilter: 'block'`를 `UiEntity` 크기로 `100%` 만든 경우 `100%` 화면의 모든 클릭을 가로채게 되어 다른 UI와 전체 3D 월드를 클릭할 수 없게 됩니다. 대신 필요한 패널이나 버튼에 핸들러를 붙이세요. 자세한 내용은 [포인터 차단](/docs/creator/content-creator-ko/sdk7/2d-ui/ui_button_events/#pointer-blocking).


***ui.tsx 파일:***

```tsx
import ReactEcs, { ReactEcsRenderer, UiEntity } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
  // 부모 엔티티
  <UiEntity
    uiTransform={{
      width: 200,
      height: 200,
      margin: { top: '250px', left: '500px' },
    }}
    uiBackground={{ color: Color4.Blue() }}
  >
    {/* 자체 닫기 자식 엔티티 */}
    <UiEntity
      uiTransform={{
        width: 400,
        height: 400,
        margin: { top: '35px', left: '500px' },
      }}
      uiText={{ value: `Hello world!`, fontSize: 40 }}
    />
    {/* 부모 엔티티에 대한 닫는 구문 */}
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

UI 모듈의 정의에는 부모 수준 엔티티가 하나만 있을 수 있습니다. 원하는 만큼 다른 엔티티를 정의할 수 있지만, 모두 맨 위에 하나의 부모를 가진 구조 안에 들어가야 합니다.

## 화면 가상 스케일

UI에 대한 가상 너비와 높이를 설정하세요. 이렇게 하면 실제 픽셀 기준 화면 크기와 무관하게, 서로 다른 화면 크기에서도 UI가 동일하게 보입니다.

```ts
export function setupUi() {
    ReactEcsRenderer.setUiRenderer(uiComponent, { virtualWidth: 1920, virtualHeight: 1080 })
}
```

가상 너비를 1920, 가상 높이를 1080으로 설정하면 UI는 화면 크기에 맞게 확대/축소됩니다. 화면이 1920x1080이면 UI는 가상 크기와 같은 크기로 표시됩니다. 화면이 더 크거나 작으면 모든 픽셀 값이 가상 크기에 맞게 조정됩니다. 예를 들어 화면이 3840x2160이면 너비가 100픽셀로 정의된 항목은 실제 픽셀 200개에 걸쳐 표시됩니다.

픽셀 값에 곱해지는 UI 스케일 팩터의 실제 계산식은 [`Math.min(realWidth / virtualWidth, realHeight / virtualHeight)`](https://github.com/decentraland/js-sdk-toolchain/blob/main/packages/%40dcl/react-ecs/src/system.ts).

#### 전체 화면 아트가 가상 크기보다 더 큽니다

스케일 팩터는 참조 해상도를 유지합니다 **안에** 화면에 배치하므로, 가상 크기와 모양이 다른 화면에서는 추가 공간도 여전히 UI의 일부입니다. 퍼센트는 스케일되지 않으므로 `width: '100%'`, `height: '100%'` 인 루트 요소는 화면 전체를 덮게 되며, 한 방향으로는 가상 크기보다 더 클 수 있습니다.

기본값인 `1920x1080`이 설정된 16:10 화면에서는 스케일 팩터가 너비에서 결정되므로, 전체 화면 루트의 크기는 **1920x1200** 가상 픽셀 기준이 됩니다. 더 넓은 화면에서는 대신 1920보다 더 넓어집니다.

따라서 화면을 채워야 하는 배경 이미지의 경우:

* 아트는 **1920x1080보다 더 크게** 내보내고 CSS의 `cover` 맞춤처럼 잘리도록 두세요. 정확히 1920x1080 크기의 아트는 더 긴 화면에서 위아래에 여백을 남깁니다.
* 반드시 보이게 해야 하는 것(로고, 제목, 버튼 등)은 가운데 정렬된 1920x1080 영역 안에 두고, 그 밖의 영역은 사라져도 되는 것으로 간주하세요.

### 기본 가상 크기

가상 크기는 선택 사항이지만 두 차원은 함께 갑니다. 둘 다 전달하거나 둘 다 전달하지 않아야 합니다. 하나를 전달하지 않으면 플랫폼 기본값이 적용됩니다:

| 플랫폼      | 기본 가상 크기    |
| -------- | ----------- |
| 모바일      | `1600x720`  |
| 데스크톱 및 웹 | `1920x1080` |

즉, 아무 옵션도 전달하지 않더라도 UI의 픽셀 값은 항상 참조 해상도에 맞춰 조정됩니다. 장면이 `addUiRenderer()` 만 사용하고 `setUiRenderer()`.

를 전혀 호출하지 않더라도 마찬가지입니다.

* **세 가지 특별한 경우:**&#xC2A4;케일링 제외 `0` : 잘못된 크기—

  ```ts
  ReactEcsRenderer.setUiRenderer(uiComponent, { virtualWidth: 0, virtualHeight: 0 })
  ```
* **불완전한 크기**: 두 차원 중 하나만 주는 것 역시 잘못된 값이므로, 위의 경우와 마찬가지로 가상 화면이 비활성화됩니다. 플랫폼 기본값은 **적용되지** 않습니다. 위의 제외와 달리 이 경우는 보고됩니다. 절반만 지정된 크기는 의도적인 선택이 아니라 실수이므로 SDK가 다음을 로그에 남깁니다 `불완전한 가상 화면 크기(…): 두 차원이 모두 필요하므로 가상 화면이 비활성화되고 UI 스케일링이 적용되지 않습니다.` 를 크기당 한 번씩 출력합니다. 두 차원을 모두 전달하거나, 아예 전달하지 마세요.

  ```ts
  // 이렇게 하지 마세요 — 스케일링이 전혀 적용되지 않습니다
  ReactEcsRenderer.setUiRenderer(uiComponent, { virtualWidth: 1920 })
  ```

  메인 렌더러에서는 반드시 다시 확인할 가치가 있습니다. 그곳에서 불완전한 크기는 장면 전체의 전환 스위치이기도 하기 때문입니다. `setUiRenderer()` 을 언급하는 호출이 *어느 쪽이든* 차원을 가지면 그 값이 어떤 `addUiRenderer()`보다 우선하므로, 위 예시는 장면 전체의 가상 화면을 비활성화하고 추가 렌더러에 전달된 유효한 크기를 버리게 됩니다.
* **모바일의 16:9 크기**: 휴대폰 화면은 16:9보다 훨씬 넓으므로 16:9 가상 캔버스는 UI에 레터박스를 만들게 됩니다. 16:9 크기(`1920x1080`, `1280x720`, …)를 전달했고 장면이 모바일에서 실행되면, 그것은 `1600x720`으로 재정의되며 콘솔에 메시지가 한 번 로그됩니다. 모바일에서 16:9가 아닌 크기와 데스크톱 또는 웹에서의 모든 유효한 크기는 그대로 존중됩니다.


> [!NOTE]
> **📔 참고** :  `vw` 및 `vh` 단위는 가상 화면과 독립적입니다: `1vw` 는 캔버스 너비의 1%이고 `1vh` 는 캔버스 높이의 1%로, CSS와 정확히 같습니다. 또한 [`screenInset`](#screen-inset-area)도 무시하므로 기본 `'device'` 안쪽에서 `width: '100vw'` 는 `width: '100%'` 보다 더 넓습니다. 앞의 것은 화면 전체를 가득 채우고, 뒤의 것은 inset 영역을 채웁니다.



> [!NOTE]
> **📱 참고**: 플랫폼 감지는 비동기적으로 해결됩니다. 처음 몇 프레임 동안 SDK는 아직 모바일에서 실행 중이라는 사실을 알지 못하므로, 가상 화면은 `1920x1080` 에서 시작해 `1600x720` 로 전환됩니다. 감지가 완료되면 UI가 로드 시 잠시 다시 스케일링되는 것을 볼 수 있으며, 장면 시작 후 잠시 뒤 콘솔에 16:9 재정의 메시지가 나타납니다. 처음 프레임부터 단일하고 안정적인 참조 해상도를 원한다면 명시적으로 16:9가 아닌 가상 크기를 전달하세요.



> [!WARNING]
> **📔 이 SDK 버전 이전에 작성된 장면에서 달라진 점**
> 
> 세 가지 변경 사항이 기존 장면에 영향을 줍니다. 이들 중 어느 것도 선택 사항이 아니므로, 전혀 건드리지 않은 장면도 SDK 버전을 업데이트하면 외형이 달라집니다:
> 
> 1. **이제 가상 화면이 기본적으로 적용됩니다.** 옵션을 전혀 전달하지 않던 장면은 이전에는 픽셀 값을 원시 캔버스 픽셀로 배치했습니다. 이제는 다음 기준으로 스케일됩니다 `1920x1080` (`1600x720` (모바일에서). 이전 동작으로 되돌리려면 가상 화면을 다음으로 명시적으로 비활성화하세요 `setUiRenderer(ui, { virtualWidth: 0, virtualHeight: 0 })`.
> 2. **`screenInset` 기본값은 `'device'`.** 이제 UI는 기기 안전 영역 안에 배치되므로 모바일에서는 안쪽으로 이동하고, 루트 수준의 `100%` 는 더 이상 전체 화면이 아닙니다. 전체 화면 배경, 디머, 오버레이는 노치와 홈 인디케이터에서 멈춥니다. 이전 동작으로 되돌리려면 `setUiRenderer(ui, { screenInset: 'none' })`를 전달하세요. 참조 [화면 inset 영역](#screen-inset-area).
> 3. **`devicePixelRatio` 는 UI 레이아웃에 관여하지 않습니다.** 픽셀 크기 UI는 이제 고밀도(레티나 및 모바일) 화면에서 이전보다 최대 2\~3배 더 크게 표시됩니다. **이 경우에는 제외 옵션이 없습니다** — 수동으로 조정한 크기를 다시 확인하고, 장면이 자체 계산하던 모든 스케일 팩터를 제거하세요.


## 화면 inset 영역

화면은 완전히 사용할 수 있는 상태가 아닙니다. 휴대폰은 노치, 상태 표시줄, 홈 인디케이터 및 둥근 모서리를 위한 공간을 예약하고, 모든 탐색기는 캔버스 일부 위에 자체 HUD(미니맵, 채팅, …)를 그립니다. 렌더러 옵션의 선택 사항인 `screenInset` 속성은 UI가 배치될 화면 영역을 선택합니다:

| 값                  | UI가 배치되는 영역                                                                                                                              |
| ------------------ | ---------------------------------------------------------------------------------------------------------------------------------------- |
| `'device'` *(기본값)* | 노치, 상태 표시줄 및 둥근 모서리를 제외한 기기 안전 영역입니다. 다음에서 읽습니다 `UiCanvasInformation.screenInsetArea`.                                                   |
| `'interactable'`   | 탐색기가 자체 HUD(미니맵, 채팅, …)가 없는 것으로 보고하는 영역입니다. 다음에서 읽습니다 `UiCanvasInformation.interactableArea`. 무엇을 포함하는지는 각 탐색기에 달려 있습니다. 대상 플랫폼에서 확인하세요. |
| `'none'`           | 전체 화면이며 `0,0` 는 왼쪽 위 모서리에 있습니다.                                                                                                          |

```ts
// UI는 노치, 상태 표시줄 및 둥근 모서리에서 떨어져 있게 유지됩니다 — 이것이 기본값입니다
ReactEcsRenderer.setUiRenderer(uiComponent, { virtualWidth: 1920, virtualHeight: 1080 })

// UI는 탐색기 자체 HUD에서 떨어져 있게 유지됩니다
ReactEcsRenderer.setUiRenderer(uiComponent, { screenInset: 'interactable' })

// UI가 전체 화면을 덮으며, 여백은 직접 처리합니다
ReactEcsRenderer.setUiRenderer(uiComponent, { screenInset: 'none' })
```

데스크톱에서는 기기 inset이 0이므로 `'device'` 그곳에서는 UI가 전체 화면에 배치됩니다. 이는 `'none'`와 동일한 결과입니다. 영역은 매 틱마다 다시 읽히므로, 회전하거나 시스템 바가 나타났다 사라질 때처럼 inset이 바뀌면 UI도 그것을 따라갑니다.


> [!WARNING]
> **📔 UI를 왼쪽 위에 고정하지 마세요.** 그 모서리와 전체 왼쪽 가장자리는 탐색기가 미니맵, 채팅, 그리고 모바일에서는 가상 조이스틱을 그리는 위치입니다. 그곳에 배치된 장면 UI는 렌더링되지만, 클라이언트의 자체 UI가 그 위에 그려지므로 가려지고 버튼을 클릭할 수 없는 경우가 많습니다. UI는 오른쪽에 고정하거나 가운데에 두거나, 또는 `screenInset: 'interactable'` 을 사용해 탐색기가 자체 HUD를 피해서 배치하도록 하세요. 참조 [UI를 절대로 왼쪽 위에 고정하지 마세요](/docs/creator/content-creator-ko/sdk7/designing-the-experience/ux-ui-guide/#never-anchor-your-ui-to-the-top-left).



> [!WARNING]
> **📔 `'interactable'` 는 데스크톱에서 무효가 아닙니다.** 기기 inset과 달리 interactable 영역은 *적용되지* 데스크톱 클라이언트에서 0이 아닙니다. 자체 UI를 위해 화면의 왼쪽 약 25%를 예약하므로 `screenInset: 'interactable'` 그곳에서는 남은 75% 영역에 UI를 배치합니다. 이것이 이 옵션의 목적이지만, 데스크톱 레이아웃도 바뀐다는 뜻이므로 [`isMobile()`](/docs/creator/content-creator-ko/build-for-mobile/develop/detect-platform/) 로 분기하세요. 휴대폰에서만 적용하려는 경우입니다.
> 
> **클라이언트 지원**: `'interactable'` 영역을 보고하는 탐색기가 필요합니다. 데스크톱에서는 지원되며, 모바일에서는 클라이언트 버전 `1.12.1` 부터 지원됩니다. 더 오래된 모바일 클라이언트에서는 값이 0으로 보고되고, UI는 전체 화면을 덮는 것으로 대체됩니다. 같은 `1.12.1` 릴리스에서는 또한 `'device'` 영역을 Android와 iOS 사이에서 정규화하므로, 어느 inset에 의존하든 레이아웃의 기준선으로 삼으세요. `'none'` 는 어디서나 동일하게 동작합니다.



> [!WARNING]
> **📔 참고**: UI를 [`ScreenInsetArea` 또는 `InteractableArea`](/docs/creator/content-creator-ko/build-for-mobile/develop/safe-area/#wrap-part-of-your-ui-instead) 컴포넌트로 감싸면서 렌더러의 해당 `screenInset` 값도 그대로 두지 마세요. inset이 두 번 적용되어 UI가 두 배의 여백만큼 안쪽으로 밀리게 됩니다. 대신 `screenInset`에 의존하거나, 그것을 `'none'` 으로 설정하고 래퍼를 직접 배치하세요.


### 실제 기기의 세 가지 영역

아래 캡처는 같은 휴대폰의 같은 장면을, 오직 `screenInset` 값만 바꿔서 세 번 렌더링한 것입니다. 자홍색 사각형은 UI 자체이며, 윤곽선은 탐색기가 보고하는 영역입니다. 시안은 전체 캔버스, 앰버는 `screenInsetArea`이고, 초록색은 `interactableArea`.

<figure><img src="/docs/_remote/75b3a862ee39f3dbb1722d14.webp" alt="Scene UI covering the whole phone screen, including the notch strip and the areas under the client controls"><figcaption><p><code>screenInset: 'none'</code> — UI가 전체 캔버스를 덮습니다. 모서리는 노치 아래와 클라이언트 자체 컨트롤 뒤에 있습니다.</p></figcaption></figure>

<figure><img src="/docs/_remote/b3f7d16c8434488f8b197eda.webp" alt="Scene UI inset to the device safe area, clear of the notch"><figcaption><p><code>screenInset: 'device'</code> (기본값) — UI가 기기의 안전 영역 안으로 들어오며, 노치와 둥근 모서리를 피합니다. 다만 이 영역의 일부가 아닌 클라이언트 컨트롤과는 여전히 겹칩니다.</p></figcaption></figure>

<figure><img src="/docs/_remote/de3e57715b49b467501da7ac.webp" alt="Scene UI inset to the area the client reports as free of its own HUD"><figcaption><p><code>screenInset: 'interactable'</code> — UI는 클라이언트가 장면 UI를 위해 지정한 사각형 안에 배치됩니다. 여기에서 캡처한 모바일 클라이언트 <code>1.12.1</code>에서는 왼쪽 열(프로필, 채팅, 조이스틱)은 제외되며, 오른쪽 아래의 액션 버튼은 설계상 그 위에 그려집니다.</p></figcaption></figure>


> [!NOTE]
> **💡 팁**: `'interactable'` 는 각 탐색기가 장면 UI를 위해 지정하는 영역을 제공합니다. 이는 의도적으로 "모든 클라이언트 컨트롤이 그 밖에 있다"는 뜻과 같지 않습니다. 세 번째 캡처에서 보듯이 모바일 액션 버튼은 의도적으로 그 영역 위에 그려집니다. 그 아래에 배치한 요소는 여전히 선택할 수 있지만 같은 탭을 두고 경쟁하게 되므로, [혼잡한 모서리](/docs/creator/content-creator-ko/build-for-mobile/develop/safe-area/#where-the-client-controls-live) 를 염두에 두고 대상 플랫폼을 확인하세요. 그 영역에 무엇이 포함되는지는 각 탐색기의 판단이며 버전 간에 달라질 수 있습니다.


각 렌더러는 자체 `screenInset`를 존중하므로, 메인 UI와 `addUiRenderer()` 로 추가된 UI는 동시에 서로 다른 영역을 사용할 수 있습니다. 가상 크기와 달리 이 값은 렌더러 간에 공유되지 않습니다.

## 여러 UI 모듈

장면에 각자 자체 UI를 정의하는 여러 시스템이나 모듈이 있다면, 각 UI 모듈을 `ReactEcsRenderer.addUiRenderer()`로 렌더링할 수 있습니다. 이는 여러 UI 컴포넌트가 있는 복잡한 장면을 다룰 때, 또는 [스마트 아이템](/docs/creator/content-creator-ko/scene-editor/interactivity/smart-items/)의 UI를 정의할 때 특히 유용합니다. 스마트 아이템의 UI는 장면의 나머지 코드와 독립적으로 사용할 수 있어야 합니다.

다음 `ReactEcsRenderer.addUiRenderer()` 함수는 UI의 소유자로 엔티티를 제공해야 합니다. 이것은 아무 엔티티나 될 수 있으며, UI 소유자로만 사용하기 위해 만든 더미 엔티티여도 됩니다.

```ts
export function setupUi() {

    // UI의 소유자가 될 더미 엔티티를 만듭니다
    const dummyEntity = engine.addEntity()

    // UI 모듈을 UI 모듈 배열을 반환하는 함수로 정의합니다
    const uiComponent = () => [
      // UI 모듈을 반환하는 함수,
      // UI 모듈을 반환하는 함수
      // ...
    ]

    // 더미 엔티티를 소유자로 하여 UI 모듈을 렌더링합니다
    ReactEcsRenderer.addUiRenderer(dummyEntity, uiComponent)
}
```

이 스니펫은 장면의 다른 UI 코드와 독립적으로 존재할 수 있습니다. 장면의 나머지 부분에는 `ReactEcsRenderer.setUiRenderer()`가 포함될 수도 있고 전혀 없을 수도 있지만, UI는 여전히 렌더링됩니다.

하나의 `addUiRenderer()` 호출에는 `setUiRenderer()`처럼 가상 너비와 높이를 포함할 수도 있습니다. 그러나 장면에 `setUiRenderer()` 호출도 있어서 가상 너비와 높이를 정의하고 있다면 `addUiRenderer()` 호출의 가상 너비와 높이는 무시됩니다.

```tsx
ReactEcsRenderer.addUiRenderer(dummyEntity, uiComponent, { virtualWidth: 1920, virtualHeight: 1080 })
```

가상 크기는 장면 전체에서 하나의 값이며, 다음과 같이 결정됩니다: `setUiRenderer()` 의 크기가 우선하고, 그렇지 않으면 첫 번째 `addUiRenderer()` 호출이 제공한 값이 우선하며, 어떤 렌더러도 제공하지 않았다면 [플랫폼 기본값](#default-virtual-size) 이 적용됩니다. `screenInset` 만 포함하는 옵션은 제공된 크기로 간주되지 않습니다.

다음 [`screenInset`](#screen-inset-area) 는 반대로 동작합니다. 렌더러별이므로 각 UI 모듈은 서로 다른 화면 영역에 놓일 수 있습니다:

```tsx
// 이 위젯은 메인 UI가 무엇을 사용하든 관계없이 탐색기 HUD와 겹치지 않게 유지됩니다
ReactEcsRenderer.addUiRenderer(dummyEntity, uiComponent, { screenInset: 'interactable' })
```

그 UI는 다음으로 제거할 수 있습니다 `ReactEcsRenderer.removeUiRenderer(dummyEntity)` 또한 UI를 소유한 엔티티가 파괴되면 UI도 제거됩니다. 같은 엔티티에 대해 `ReactEcsRenderer.addUiRenderer()` 가 다시 호출되지만 UiRenderer가 다르면 이전 것은 정리되고 새 것이 이를 대체하며 스택 순서에서의 위치를 유지합니다. 이 호출을 다시 하는 것은 이미 렌더링된 UI 모듈의 옵션을 변경하는 방법이기도 합니다. 예를 들어 `zIndex`.

### UI 모듈 간의 쌓임 순서

기본적으로 UI 모듈은 처음 나타난 순서대로 쌓이며, 나중에 나타난 것이 앞에 옵니다. 같은 프레임에서 처음 나타나는 모듈 — 일반적으로 씬의 첫 프레임 전에 등록된 모든 항목 — 은 main UI와 함께 그려집니다 `setUiRenderer()` 뒤쪽에 배치되고, 그 뒤에 다른 모듈들이 추가된 순서대로 이어집니다. 이를 명시적으로 제어하려면, 하나를 전달하세요 `zIndex` 렌더러 옵션에서: 더 높은 값을 가진 모듈은 `zIndex` 더 낮은 값을 가진 모듈들 앞에 렌더링됩니다. 나타난 순서와 관계없이 적용됩니다. 그대로 둔 모듈은 `0` 기본 순서를 유지합니다. 이 옵션은 `setUiRenderer()`에서도 동일하게 동작하므로, main UI도 다른 모듈보다 앞에 배치할 수 있습니다.

```tsx
// 먼저 등록되지만, 항상 다른 UI 모듈보다 앞에 배치됩니다
ReactEcsRenderer.addUiRenderer(dummyEntity, uiComponent, { zIndex: 10 })

// main UI는 더 낮은 값을 설정하지 않는 모든 모듈보다 뒤에 위치합니다
ReactEcsRenderer.setUiRenderer(mainUi, { zIndex: -10 })
```

이것은 다음과는 다른 설정입니다 `zIndex` 의 `UiEntity`, 이는 오직 [UI 모듈 내부의 형제 요소들](/docs/creator/content-creator-ko/sdk7/2d-ui/ui-positioning/#z-index). 렌더러 옵션은 전체 모듈들을 서로 간에 정렬하고, 각 모듈 내부의 요소들은 자신만의 `zIndex`.

### 하나의 setUiRenderer 문을 공유하기

호출하는 대신 `ReactEcsRenderer.addUiRenderer()` 각 UI 모듈마다 `ReactEcsRenderer.setUiRenderer()` UI 모듈 배열과 함께 한 번 호출할 수 있으며, 이 배열은 서로 다른 파일에 있어도 됩니다.

```ts
const uiComponent = () => [
  // UI 모듈을 반환하는 함수,
  // UI 모듈을 반환하는 함수
  // ...
]

ReactEcsRenderer.setUiRenderer(uiComponent, { virtualWidth: 1920, virtualHeight: 1080 })
```

아래는 더 완전한 예시입니다:

***ui.tsx 파일:***

```ts
export function UIModule1() {
  return (
    <UiEntity
      uiTransform={{
        flexDirection: 'column',
        alignItems: 'center',
        justifyContent: 'space-between',
        positionType: 'absolute',
        position: { right: '3%', bottom: '3%' },
      }}
    >
      <Label value="안녕하세요, 세상!" fontSize={18} textAlign="middle-center" />
    </UiEntity>
  )
}

export function UIModule2() {
  return (
    <UiEntity
      uiTransform={{
        flexDirection: 'column',
        alignItems: 'center',
        justifyContent: 'space-between',
        positionType: 'absolute',
        position: { right: '3%', top: '3%' },
      }}
    >
      <Label
        value="여기에 UI가 조금 더 있습니다!"
        fontSize={18}
        textAlign="middle-center"
      />
    </UiEntity>
  )
}
```

***index.ts 파일:***

```ts
import { ReactEcsRenderer } from '@dcl/sdk/react-ecs'
import { UIModule1, UIModule2 } from './ui'

export function main() {
    ReactEcsRenderer.setUiRenderer(() => [
      UIModule1(),
      UIModule2(),
      // ...
      // 아래 줄은 DCL UI Toolkit 라이브러리를 사용하기 위한 것입니다
      // https://github.com/decentraland-scenes/dcl-ui-toolkit
      ui.render(),
    ], { virtualWidth: 1920, virtualHeight: 1080 })
}
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/2d-ui/onscreen-ui.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
