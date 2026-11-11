> 플레이어가 머물고 다시 방문하고 싶어지는 씬을 만드는 팁과 실천 방법!

# UX 및 UI 가이드

Decentraland 콘텐츠 크리에이터가 자신의 창의성을 최대한 발휘하여 자유롭게 활용할 수 있는 것은 우리에게 매우 중요합니다. 이것은 Decentraland가 탐험할 가치가 있는 보람 있는 공간이 되는 데 필수적입니다. 하지만 Decentraland의 장면은 매우 다양한 사용자들이 방문하게 되며, 우리는 그 모든 사람이 장면을 이용할 수 있어야 한다는 점도 중요하게 생각합니다. 그러므로 우리는 통일된 디자인 기준을 세워야 할 필요를 무시할 수 없습니다. 이 기준은 플레이어가 어떤 장면에 있든 일관되게 직관적이고 즐거운 경험을 하게 된다는 것을 보장하는 가이드가 되어야 합니다.

이 문서에서는 인터페이스와 플레이어 경험을 디자인할 때 최선의 실천이라고 우리가 생각하는 것을 장려하기 위해 Decentraland의 디자인 가치관을 공유합니다. 이 기준들이 메타버스를 구성하는 장면들을 만들고/또는 개선하는 출발점이 되기를 바랍니다.

## 디자인 가치관

Decentraland의 모든 장면은 다음 가치를 염두에 두고 설계되어야 합니다:

* **환영하는**: 플레이어가 따뜻하게 환영받는다고 느낍니다.
* **사용자 친화적**: UI는 쉽고 재미있게 사용할 수 있어야 합니다. 바퀴를 다시 발명하지 않도록 해야 합니다.
* **배우기 쉬운**: 플레이어가 UI를 친숙하고 직관적으로 느낍니다. 패턴은 장면 전체에서 일관되게 사용되어야 합니다.
* **안내 제공**: 장면이 도움의 손길을 제공합니다. 텍스트, 모션, 사운드, 그래픽이 플레이어를 올바른 방향으로 이끌고 힌트를 줍니다.
* **반응형**: 플레이어 입력에 대한 명확한 행동-반응이 있어야 합니다.
* **미니멀한**: 적을수록 많습니다. 플레이어는 중요한 것에 주의를 집중할 수 있습니다.
* **흥미로운**: 장면은 Decentraland를 독특하고 방문할 가치가 있게 만드는 요소들을 활용합니다.
* **목적이 있는**: 플레이어가 다시 돌아올 강한 이유를 갖게 됩니다.
* **즐거운**: 플레이어는 시각, 소리, 감각적 경험을 즐깁니다.

## 사용자 경험

### 어디서 시작할까요?

당신의 장면의 목적은 무엇인가요? 감상하고 탐험할 아름다운 풍경을 제공하는 것인가요? 박물관인가요? 1인용 게임인가요? 경쟁형 게임인가요? 아니면 선형적인 스토리텔링 흐름을 따르나요?

이 목적을 디자인 과정의 모든 단계에서 분명히 인식하는 것이 중요합니다. 이것이 모든 결정을 이끌어야 하며, 각 결정은 방문자의 경험에 큰 영향을 미칠 수 있습니다.

각 단계가 이전 단계에 의존하는 선형 흐름의 장면이라면, 플레이어가 잘못된 순서로 내용을 접하지 않도록 장면을 벽으로 둘러싸고 하나 또는 여러 개의 고정된 प्रवेश 지점을 설정하는 것을 권장합니다. 참고로 प्रवेश 지점은 플레이어를 환영하고 장면에서의 목표가 무엇인지 설명하는 메시지를 표시하기에 매우 좋은 위치입니다.

<figure><img src="/docs/_remote/978e0e21e5bb4482320c0ae1.webp" alt="In-world banner" width="800"><figcaption><p>월드 내 안내</p></figcaption></figure>

장면에서의 플레이어 경험이 선형적일 필요는 없지만, 그 경험을 충분히 즐길 수 있도록 몇 가지 기본 안내를 제공해야 한다면 팝업으로 안내를 보여줄 수 있습니다. 플레이어가 어느 방향에서 들어오든 입장하자마자 화면에 팝업을 표시할 수 있으며, 이를 통해 플레이어가 항상 핵심을 알 수 있게 할 수 있습니다.

<figure><img src="/docs/_remote/bab29f1240d71035b8426a51.webp" alt="Pop-up banner" width="800"><figcaption><p>팝업 안내</p></figcaption></figure>


> [!WARNING]
> **📔 참고**: 팝업이 너무 방해가 되지 않도록 하고, 아무 곳이나 클릭하면 쉽게 닫히게 하세요.


환영 메시지에는 플레이어에게 다음 내용이 전달되어야 합니다:

* 장면 제목
* 플레이어를 환영하기
* 플레이어의 최종 목표와 동기
* 안내/다음 단계
* *(선택 사항)* 컨트롤. 주로 장면이 전역 버튼 이벤트를 사용할 때 유용합니다. 그렇지 않다면, 아이템 위에 마우스를 올렸을 때 각 아이템 자체가 어떻게 사용하는지 보여줍니다.

### 동기 부여

방문자들이 계속해서 당신의 장면에 돌아오게 하고 싶을 가능성이 높습니다. 그렇게 만들 수 있는 방법을 생각해 보세요! 스스로에게 "플레이어가 내 장면에 다시 오고 싶어 하거나 친구에게 추천하고 싶어지게 만드는 것은 무엇일까?"라고 물어보세요.

예를 들어, 도전 과제를 제공해 여러 번 시도해야 할 정도로 어렵지만 달성 가능한 목표를 줄 수 있습니다. 경쟁적인 멀티플레이어 이벤트를 열 수도 있는데, 이는 사교의 흥미로운 기회이기도 합니다. 플레이어가 그곳에서 시간을 보내는 것을 즐긴다고 느끼는 역동적인 공간으로 장면을 만드세요!

### 피드백과 사운드

사운드는 두 가지 중요한 역할을 하므로, 이를 활용할 기회를 놓치지 마세요! 한편으로는 몰입감을 만드는 데 매우 중요합니다. 소리가 동반되지 않으면 현실적으로 느껴지지 않을 많은 요소들이 있습니다. 또 다른 중요한 역할은 플레이어의 행동에 대한 피드백을 제공하는 것입니다. 시각적 단서에만 의존하면 플레이어가 자신의 행동의 효과를 보기 위해 올바른 방향을 바라보고 있다는 보장이 없습니다. 사운드를 사용하면 무언가가 일어났다는 것을 플레이어가 알아차리게 할 수 있습니다. 시각적 단서와 사운드 단서를 함께 사용하면, 소리를 들은 플레이어가 혹시 보지 못했더라도 시각적 단서를 찾기 위해 주변을 살펴보게 됩니다.

일부 플레이어는 소리를 끄고 플레이할 수 있다는 점을 기억하세요. 사운드를 장면 안내나 게임 메커니즘의 핵심 요소로 사용한다면, 같은 방향을 가리키는 시각적 요소도 함께 제공해 보세요. 예를 들어 영화의 자막처럼 UI 하단에 텍스트를 표시해 이러한 소리를 보조할 수 있습니다.

## 사용자 인터페이스

이 섹션은 장면의 디자인을 통해 플레이어의 집중을 가장 중요한 요소로 이끄는 내러티브를 구성하는 데 도움을 주기 위한 것입니다. 여기에 설명된 도구를 잘 활용하면 플레이어의 경험을 훨씬 더 몰입감 있고 성공적으로 만들 수 있습니다.

색상, 아이콘, 모션, 스케일이 올바른 메시지를 전달하는 데 얼마나 중요한지 아무리 강조해도 지나치지 않습니다. 장면의 모든 것은 의미를 전달한다는 점을 기억하세요.

### 레이아웃

기본 Decentraland UI 요소와 겹치는 화면 영역에 UI를 배치할 때는 주의하세요. 데스크톱 클라이언트에서 기본 Decentraland UI(사이드바, 미니맵, 채팅 등)는 화면의 왼쪽 25%만 차지하도록 설계되어 있습니다(이미지에서 빨간색으로 강조된 영역). 나머지 화면 공간은 콘텐츠 크리에이터가 자유롭게 사용할 수 있는 안전 영역입니다.

<figure><img src="/docs/_remote/1f94cb72ac68f183e0aaa73e.webp" alt="Screen regions used by UI" width="800"><figcaption></figcaption></figure>

화면 왼쪽 25% 영역에 UI 요소를 배치하는 데 제한은 없지만, explorer가 디자인에 대해 지속적으로 변경과 개선을 하고 있다는 점을 기억하세요. 오늘은 가려지지 않는 영역이라도 미래 버전에서는 가려질 수 있습니다.


> [!NOTE]
> **📱 모바일 안전 영역** :  [모바일 클라이언트에서](/docs/creator/content-creator-ko/build-for-mobile/mobile-client/overview/) 는 캔버스의 일부 위에 자체 컨트롤을 표시합니다: **왼쪽** (채팅, 검색, 프로필, 조이스틱, 감정 표현), **오른쪽 위** (프로필 및 카메라 컨트롤), 그리고 **오른쪽 아래** (액션 및 상호작용 버튼)입니다. 해당 영역을 손으로 직접 측정하지 마세요. 클라이언트가 보고하는 영역 내부에 렌더링하고, `screenInset: 'interactable'`. 중요한 UI는 화면 중앙 또는 상단 중앙에 두고 [모바일 안전 영역](/docs/creator/content-creator-ko/build-for-mobile/develop/safe-area/). 다음을 사용하세요: [`isMobile()`](/docs/creator/content-creator-ko/build-for-mobile/develop/detect-platform/) 데스크톱과 모바일 제약 조건이 맞지 않을 때 레이아웃을 전환하는 데 사용하세요.



> [!WARNING]
> **📔 참고**: 장면과 스마트 웨어러블의 UI 요소는 항상 기본 Decentraland UI 뒤의 레이어에 표시됩니다.


#### UI를 절대로 왼쪽 위에 고정하지 마세요

가장 흔한 레이아웃 실수는 패널을 왼쪽 상단 모서리에 고정하거나 화면을 덮는 루트의 원점에 그대로 두는 것입니다. 그 모서리와 전체 왼쪽 가장자리는 explorer가 자체 미니맵과 채팅을 그리는 곳이며, 모바일에서는 가상 조이스틱이 그려지는 곳이기도 합니다. 그곳에 배치된 장면 UI도 계속 렌더링되지만, 클라이언트 자체 UI가 그 위에 그려지므로 가려지고 버튼을 클릭할 수 없는 경우가 많습니다. 레이아웃 엔진은 이에 대해 아무 경고도 하지 않습니다.

대신 다음 중 하나를 사용하세요:

* **오른쪽에 앵커를 두거나, 요소를 중앙에 배치하세요.** 점수, 타이머 또는 작은 패널의 경우 `positionType: 'absolute'` 및 `position: { top: 40, right: 40 }`를 설정하거나, `justifyContent: 'center'` 로 루트에서 중앙 정렬하세요.
* **안전한 영역은 explorer가 알려주게 하세요.** 설정 [`screenInset: 'interactable'`](/docs/creator/content-creator-ko/sdk7/2d-ui/onscreen-ui/#screen-inset-area) 렌더러에서. 그러면 UI가 explorer가 장면 UI를 위해 확보한 사각형 안에 배치되며, 이 영역에는 이미 미니맵, 채팅, 왼쪽 컨트롤이 제외되어 있습니다.

```ts
ReactEcsRenderer.setUiRenderer(uiComponent, { screenInset: 'interactable' })
```

모바일에서는 오른쪽 아래 모서리의 액션 버튼이 의도적으로 그 영역 위에 그려지며, 그곳의 탭은 장면이 아니라 클라이언트의 버튼으로 전달됩니다. 상호작용 요소는 오른쪽 아래 모서리에서 멀리 두거나, 어차피 화면을 장악하는 UI라면 열려 있는 동안 터치 컨트롤을 숨기세요:

```ts
import { TouchScreenControls } from '@dcl/sdk/ecs'

// 전체 화면 패널이 열려 있는 동안에는 플레이어가 점프하거나 E를 누를 필요가 없습니다
TouchScreenControls.hideAll()
TouchScreenControls.hideJoystick()

// 패널이 닫히면 다시 복원합니다
TouchScreenControls.showDefault()
TouchScreenControls.showJoystick()
```

`showDefault()` 장면이 시작될 때의 설정인 점프, 포인터, primary를 복원합니다. 사용하세요 `showAll()` 장면에 정말로 8개의 버튼을 모두 화면에 표시해야 할 때만 사용하세요. 둘 다 SDK 7.32.0 이상이 필요합니다. 이전 SDK에서는 8개 버튼이 기본적으로 모두 표시되며 `showAll()` 가 유일한 복원 방법입니다. 이 헬퍼들은 데스크톱에서는 아무 작업도 하지 않으므로 조건 없이 호출해도 됩니다. 참고하세요 [화면 내 컨트롤](/docs/creator/content-creator-ko/sdk7/interactivity/touch-screen-controls/).

항상 그리드를 염두에 두고, UI를 공간적으로 구성할 때 이를 주요 기준으로 사용하세요. HUD가 필요한 장면을 작업 중이라면 먼저 소모품(예: 화폐, 음식, 체력)을 함께 배치하고, 반대편에는 도구나 무기 같은 저장성 아이템을 배치할 수 있습니다. 그리드의 간격과 패딩은 일관되게 유지하도록 하세요. 모든 플레이어 행동에 대한 피드백을 명확하게 만드세요.

### 색상

색상 팔레트를 선택하는 것은 중요합니다. 이는 장면에 정체성을 부여하고 요소 간의 관계와 위계를 나타내기 때문입니다.

색상 팔레트를 구성할 때는 먼저 메인 색상을 선택하고, 필요하면 보조 색상을 선택하세요. 그런 다음 그 메인 색상과의 관계에서 유사색, 보색, 삼색 조합 등 어떤 팔레트를 사용할지 결정하면 됩니다.

<figure><img src="/docs/_remote/06d8337c29d507083f4727ab.webp" alt="Chromatic color palette" width="300"><figcaption><p>유채색 팔레트</p></figcaption></figure>

<figure><img src="/docs/_remote/cf0b4424895884aa68f549b1.webp" alt="Strategies for combining colors properly" width="300"><figcaption><p>색상을 올바르게 조합하는 전략</p></figcaption></figure>


> [!NOTE]
> **💡 팁**: 우리가 추천하는 무료 색상 팔레트 생성기: [Coolors](https://coolors.co), [Adobe Color CC](https://color.adobe.com/), [Colour Lovers](http://www.colourlovers.com), [Color Hunt](http://www.colorhunt.co), [Color by Hailpixel](http://color.hailpixel.com), [Colour Code](http://colourco.de), [Sip](https://sipapp.io/), [Paletton의 Color Scheme Designer](http://paletton.com), [Cohesive Colors](http://javierbyte.github.io/cohesive-colors), [Colr](http://www.colr.org)


메인 색상은 UI 구성 요소 전반에서 가장 자주 사용되어야 합니다. 팔레트에 보조 색상이 없다면, 메인 색상에 검정이나 흰색을 결합하여 요소를 강조할 수 있습니다. 보조 색상은 필수는 아니지만, 장면의 UI 요소를 강조하고 구분하는 데 도움이 됩니다. 보조 색상은 하이라이트, 선택 컨트롤(슬라이더와 스위치), 링크, 헤드라인에 가장 잘 어울립니다.

각 색상은 고유한 표현적 특성을 가지고 있다는 점을 기억하고, 이를 활용해 메시지를 전달하세요. 예를 들어 빨간색은 종종 부정적인 의미와 연결되고, 초록색은 긍정적인 의미와 연결됩니다.

<figure><img src="/docs/_remote/f0a0dc7306efbc4e1bb5c257.webp" alt="Colored switches" width="300"><figcaption><p>색상을 잘못 사용하면 혼란스러울 수 있습니다</p></figcaption></figure>

색상을 전략적으로 사용해 상태 변화를 나타낼 수도 있습니다. 예를 들어 요소의 색상을 바꿔 활성 상태인지 비활성 상태인지를 표시할 수 있습니다.


> [!WARNING]
> **📔 참고**: 상태는 UI 요소의 상태를 전달합니다. 요소의 상태는 어느 정도의 연속성을 유지해야 하지만, 명확한 사용 가능성을 가져야 하며 다른 상태 및 주변 레이아웃과 쉽게 구분되어야 합니다.


<figure><img src="/docs/_remote/006fc8ebd49f1ffb83ca7363.webp" alt="Active inactive button" width="300"><figcaption><p>버튼 상태를 위한 색상 사용</p></figcaption></figure>

경고! 겹쳐서 표시될 색상을 선택할 때는 함께 사용했을 때 가독성이 확보되는지 특히 주의하세요. 기본 원칙은 함께 표시되는 요소의 색상은 항상 서로 충분한 대비를 가져야 한다는 것입니다. 일부 플레이어는 최적이 아닌 조명 조건에서 화면을 보고 있을 수 있어 읽기가 더 어려워질 수 있다는 점을 염두에 두세요.

<figure><img src="/docs/_remote/78f863c16ac2c51e8736e68a.webp" alt="Icon contrast" width="300"><figcaption><p>두 아이콘 모두 구분할 수 있지만, 첫 번째 아이콘이 대비가 더 좋습니다. 읽기가 더 쉬워지고 이해하는 데 드는 노력이 줄어듭니다.</p></figcaption></figure>

### 타이포그래피 위계

제목, 부제목, 본문을 위해 최소한 3단계 크기 체계를 정의해 보세요. 색상과 글꼴 크기의 사용에 주의하세요. 모든 글꼴 크기는 12px 이상이어야 합니다.

이미지 위(또는 월드 위)에 표시되는 텍스트를 배치하려는 경우, 가독성을 반드시 실험해 보아야 합니다. 그런 경우 텍스트와 이미지 사이의 레이어에 색이 있는 불투명 영역을 추가하는 것을 권장합니다. 그렇게 하면 텍스트가 계속 읽히도록 보장할 수 있습니다.

<figure><img src="/docs/_remote/26f47a30aa943bf272e86fd8.webp" alt="Text over images" width="300"><figcaption><p>가독성을 우선하세요</p></figcaption></figure>

### 아이콘

아이콘은 정보를 압축해 이미지로 행동을 식별할 수 있도록 도와줍니다. 텍스트로 항목을 라벨링하는 것보다 훨씬 빠르게 해석할 수 있는 입력을 제공하는 데 매우 강력한 도구입니다. 텍스트와 함께 아이콘을 표시하는 것도 유용한데, 의미를 구분하는 데 도움이 되기 때문입니다.

<figure><img src="/docs/_remote/d3655e8d247e6f40487d2d6a.webp" alt="Icon examples" width="300"><figcaption><p>아이콘을 보편적인 언어로 사용하세요</p></figcaption></figure>

### 모션

필요할 때 피드백을 제공하고 길을 안내하기 위해 모션을 사용하세요. 모션은 플레이어가 주의 집중을 유지하도록 돕고, UI가 바뀌는 동안 연속성을 유지하는 데 도움이 됩니다.

<figure><img src="/docs/_remote/db15a9b89e2052e22e2de076.webp" alt="Adding glow" width="300"><figcaption><p>파티클, 스케일, 페이드되는 여백을 사용해 요소를 돋보이게 하세요</p></figcaption></figure>

#### 깜빡임 효과는 피하세요

번쩍이는 빛, 스트로브, 빠른 밝기 또는 색상 변화는 광과민성 간질이 있는 사람에게 발작을 유발할 수 있습니다. 여기에는 맥동하는 발광 재질, 깜빡이는 조명, 전체 화면 UI 깜빡임, 비디오가 포함됩니다.

* 깜빡임은 **초당 3회 이하로**, [WCAG "Three Flashes or Below Threshold"](https://www.w3.org/WAI/WCAG22/Understanding/three-flashes-or-below-threshold.html) 지침을 따르세요.
* 깜빡이는 영역은 작게 유지하고, 채도가 높은 빨간색이나 화면 전체를 채우는 크고 대비가 큰 깜빡임은 피하세요.
* 클럽 스트로브, 번개, 피해 표시 같은 강렬한 효과에 의존하는 장면이라면, 이를 끌 수 있거나 약하게 할 수 있는 옵션을 장면에 추가하세요.

### 문구와 콘텐츠 구조

UI 텍스트는 인터페이스의 사용성을 높이고 플레이어가 자신의 행동에 더 자신감을 갖게 합니다. 항상 UI 텍스트를 가능한 한 간결하게 만드세요. 플레이어는 읽기 위해 있는 것이 아니라 플레이하기 위해 그곳에 있습니다. 너무 길어 보이는 텍스트는 대부분의 플레이어가 읽지 않습니다.

환영 메시지로 플레이어를 맞이하고 장면의 목적을 알려 주세요. 먼저 장면에서의 목표를 분명히 하고, 그다음 이를 달성하는 데 필요한 행동을 설명하세요. 이후 필요에 따라 정보를 점진적으로 공개하면, 플레이어가 경험 초반부터 압도당하는 느낌을 받지 않습니다. 그래픽 자원과 마찬가지로 UI 기능과 스토리텔링 전반에서 일관된 단어를 사용하도록 하세요.

#### 말하지 말고 보여 주세요

플레이어는 첫 상호작용 전부터 두 가지 패턴에서 이탈합니다:

* **규칙 팝업**: 모든 메커니즘, 점수 규칙, 조작법을 다루는 빽빽한 여러 문단이 있는 소개 패널입니다. 플레이어는 읽지 않고 닫아버립니다.
* **월드 내 규칙서**: 장면 속 표지판이나 게시판에 같은 문단이 적혀 있습니다. 디자인처럼 보이지만, 월드 내 텍스트는 너무 작고 너무 길어서 읽기 어렵고, 플레이어도 그걸 읽으려고 가만히 서 있지 않습니다.

소개 패널은 스크롤 없이 한 화면에 담기게 하세요. 3\~5개의 짧은 줄이면 충분합니다. 이상적으로는 목표를 위한 한 줄, 컨트롤을 위한 한 줄, 그리고 그림 하나면 됩니다. 표지판은 한 가지 내용만, 10단어 이하로, 큰 글꼴 크기로, 하나의 표지판에는 하나의 아이디어만 담아야 합니다.

그다음에는 글로 쓰지 말고 보여 주세요:

* 첫 번째 목표가 분명하게 드러나게 하세요. 플레이어의 첫 시야에 빛나거나, 애니메이션이 있거나, 크기가 큰 오브젝트를 두세요. 맥동하는 윤곽선은 "나를 클릭하세요"를 문장보다 더 잘 가르쳐 줍니다.
* 실제 게임이 시작되기 전에, 연습 구역에서 플레이어가 메커니즘을 한 번 안전하게 시도해 보게 하세요.
* 다음을 사용하세요 `hoverText` 상호작용 가능한 엔터티에 대한 즉시 필요한 안내로 사용하세요. 이것은 정확히 관련 있을 때 표시되며, 미리 읽을 필요가 없습니다. 참고하세요 [호버 피드백](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/register-callback/#hover-feedback).
* 공간 규칙이나 순서 규칙에는 도식도를 사용하세요. "빨간 보석 = 1, 파란 보석 = 5" 같은 그림 하나가 문단을 대체할 수 있습니다.
* 그보다 긴 내용은 한 번에 한 메시지씩, 플레이어가 건너뛸 수 있는 NPC나 대화창을 통해 전달하세요.

규칙에 그보다 더 많은 텍스트가 필요하다면, 메커니즘을 다시 설계하거나 시각적으로 보여 주세요.

### 오브젝트 상호작용

플레이어가 어떤 오브젝트가 상호작용 가능하고 어떤 것은 아닌지 구분할 수 있게 하세요. 엔터티에 버튼 이벤트를 등록하면 SDK는 기본적으로 호버 피드백을 표시합니다. 엔터티의 가장자리 하이라이트와, 사용할 입력을 보여 주는 커서 옆 힌트가 표시되며, 상호작용의 효과를 설명하는 사용자 정의 텍스트도 포함됩니다. 이를 사용하는 것을 강력히 권장합니다. 장면이 더 자연스럽고 사용하기 쉬워질 것입니다. 참고하세요 [호버 피드백](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/register-callback/#hover-feedback).

### 3인칭 카메라

플레이어는 ‘V’ 키를 눌러 언제든지 1인칭과 3인칭 카메라를 전환할 수 있습니다. 상호작용이 두 모드 모두에서 편안하게 작동하도록 장면을 설계하세요. 예를 들어 3인칭에서는 작은 클릭 대상이 맞히기 더 어렵습니다. 장면은 플레이어의 현재 카메라 모드를 읽거나, 특정 영역에서 특정 모드를 강제할 수 있습니다. 참고하세요 [카메라](/docs/creator/content-creator-ko/sdk7/3d/camera/).

3인칭 카메라는 방의 크기에도 영향을 줍니다. 실제 세계의 치수로 지어진 실내는 화면에서 답답하게 느껴집니다. 참고하세요 [실제보다 더 크게 실내를 구성하세요](/docs/creator/content-creator-ko/sdk7/designing-the-experience/design-games/#build-interiors-larger-than-real-life).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/designing-the-experience/ux-ui-guide.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
