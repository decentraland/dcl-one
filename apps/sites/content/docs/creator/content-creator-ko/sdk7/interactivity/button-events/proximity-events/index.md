> 플레이어가 엔티티의 근접 범위에 들어오거나 벗어날 때를 감지합니다.

# 근접 이벤트

근접 상호작용을 사용하면 플레이어가 엔터티에 가까이 다가가 대략 그쪽을 바라볼 때, 커서를 엔터티에 정확히 겨누지 않아도 엔터티가 버튼 이벤트에 반응할 수 있습니다.

기존 포인터 이벤트는 레이 캐스팅을 사용합니다. 즉, 플레이어는 이벤트를 트리거하기 전에 커서를 엔터티의 콜라이더에 겨눠야 합니다. 반면 근접 이벤트는 플레이어의 커서가 어디를 향하고 있는지와 상관없이 아바타 주변의 지정된 영역 안에 있는 엔터티를 확인합니다.

근접 상호작용에 대해 고려되는 상호작용 영역은 구의 넓은 삼각형 조각으로, 플레이어의 위치에서 앞쪽으로 투영됩니다. 포인터 이벤트와 달리 카메라가 향하는 방향과는 별개로 아바타가 바라보는 방향이 중요합니다.

더 구체적인 사용자 지정 로직을 설정하려면 원시 데이터를 다루고 다음을 사용할 수 있습니다. [고급](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/advanced-button-events/) 방식.

엔터티가 상호작용 가능하려면 [콜라이더](/docs/creator/content-creator-ko/sdk7/3d/colliders/)가 있어야 합니다. 자세한 내용은 [장애물](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/click-events/#obstacles) 을(를) 참조하세요.

## 근접 콜백 등록

다음 `pointerEventsSystem` 은 포인터 이벤트 콜백과 동일한 패턴을 따르는 근접 이벤트용 헬퍼 함수를 포함합니다. [포인터 이벤트 콜백](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/register-callback/).

### 근접 버튼 누르기

다음을 사용하세요 `pointerEventsSystem.onProximityDown` 플레이어가 범위 안에 있을 때 버튼 누름을 감지합니다.  `onPointerDown`과 달리, 플레이어가 커서를 엔터티에 겨눌 필요가 없습니다.

```ts
pointerEventsSystem.onProximityDown(
    {
        entity: myEntity,
        opts: {
            button: InputAction.IA_PRIMARY,
            hoverText: 'E를 누르세요',
            maxDistance: 5,
        },
    },
    function () {
        console.log('플레이어가 엔터티 근처에서 버튼을 눌렀습니다')
    }
)
```

다음을 사용하세요 `pointerEventsSystem.onProximityUp` 플레이어가 범위 안에 있을 때 버튼을 놓는 시점을 감지합니다.

```ts
pointerEventsSystem.onProximityUp(
    {
        entity: myEntity,
        opts: {
            button: InputAction.IA_PRIMARY,
            hoverText: 'E를 놓으세요',
            maxDistance: 5,
        },
    },
    function () {
        console.log('플레이어가 엔터티 근처에서 버튼을 놓았습니다')
    }
)
```


> [!WARNING]
> **📔 참고**: 하나의 `onProximityDown` 과 하나의 `onProximityUp` 만 엔터티당 등록할 수 있습니다. 한 번 추가되면 제거될 때까지 계속 수신합니다. 시스템 루프 안에서 이들을 호출하지 마세요. 그러면 동작이 계속 덮어써집니다.


## 근접 이벤트 유형

다음 두 값이 `PointerEventType` 열거형에 추가되었습니다:

* `PET_PROXIMITY_ENTER`: 플레이어가 엔터티의 근접 범위 안으로 들어올 때 트리거됩니다.
* `PET_PROXIMITY_LEAVE`: 플레이어가 엔터티의 근접 범위 밖으로 나갈 때 트리거됩니다.

다음과 함께 사용할 수 있습니다 `PointerEvents` 컴포넌트와 `inputSystem` 을 [시스템 기반 접근 방식](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/system-based-events/#proximity-events)에 사용할 수 있습니다. 이 접근 방식은 또한 `InteractionType.PROXIMITY` 필드를 사용한 근접 버튼 누르기도 지원합니다.

## 우선순위

여러 엔터티가 범위 안에 있고 동일한 입력에 반응할 수 있는 경우, 하나만 활성화되며 기본적으로 가장 가까운 엔터티가 선택됩니다. 어느 엔터티가 우선되는지 제어하려면 `우선순위` 필드를 사용하세요. 우선순위 값이 더 큰 엔터티가 먼저 반응합니다. **더 큰 숫자** 를 우선순위 값으로 가진 엔터티가 먼저 반응합니다.

플레이어가 근접 상호작용이 있는 엔터티의 범위 안에 있으면서, 커서가 포인터 상호작용이 있는 엔터티를 가리키고 있는 경우에는 포인터 상호작용이 있는 엔터티가 항상 우선됩니다. 다시 말하지만, 하나의 엔터티만 활성화됩니다.

```ts
pointerEventsSystem.onProximityDown(
    {
        entity: doorEntity,
        opts: {
            button: InputAction.IA_PRIMARY,
            hoverText: '문 열기',
            maxDistance: 5,
            priority: 2,
        },
    },
    function () {
        console.log('문이 활성화되었습니다')
    }
)

pointerEventsSystem.onProximityDown(
    {
        entity: floorEntity,
        opts: {
            button: InputAction.IA_PRIMARY,
            hoverText: '여기를 밟으세요',
            maxDistance: 5,
            priority: 1,
        },
    },
    function () {
        console.log('바닥이 활성화되었습니다')
    }
)
```

위 예시에서는 두 엔터티가 모두 범위 안에 있더라도 우선순위가 더 높은 문 엔터티가 반응합니다.

다음 `우선순위` 필드는 다음의 속성에서도 설정할 수 있습니다 `PointerEvents` 컴포넌트.

## 옵션

근접 헬퍼 함수는 포인터 대응 함수와 동일한 옵션을 허용합니다:

* `button`: 어떤 버튼을 감지할지 지정합니다. 다음을 참조하세요 [포인터 버튼](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/click-events/#pointer-buttons) 에서 지원되는 옵션을 확인하세요.
* `maxDistance`: 플레이어의 **아바타** 에서 엔터티까지의 최대 거리(미터)입니다. 기본값은 10입니다.
* `hoverText`: 플레이어가 엔터티 근처에 있을 때 UI에 표시할 텍스트입니다.
* `showHighlight`: true이면 플레이어가 범위 안에 있을 때 엔터티에 가장자리 하이라이트를 표시합니다. *true* 가 기본값입니다.
* `showFeedback`: true이면 엔터티 중심 주변에 호버 피드백을 표시합니다. *true* 가 기본값입니다.
* `우선순위`: 여러 엔터티가 근처에 있을 때의 충돌을 해결합니다. 값이 클수록 우선합니다. 여러 엔터티의 우선순위 값이 같으면 가장 가까운 엔터티가 선택됩니다.

`maxPlayerDistance` 는 다음의 사용 중단된 별칭입니다 `maxDistance`. 둘 다 아바타로부터의 거리를 측정하므로 새 씬에서는 `maxDistance` 을 사용하세요.

### 범위 안으로 간주되는 기준

근접 이벤트는 다음에서부터 측정합니다 **플레이어 캡슐의 중심**, 즉 대략 가슴 높이에서 엔터티 콜라이더의 가장 가까운 지점까지입니다. 이는 커서 클릭이 측정하는 지점보다 약 1미터 위이므로, 같은 위치라도 두 경로에서 약간 다른 거리로 읽힐 수 있습니다.

거리만으로는 충분하지 않습니다. 근접 이벤트를 발동하려면 플레이어는 또한 다음을 만족해야 합니다:

* 엔터티를 **바라보고** 있어야 하며, 앞쪽 약 120도 원뿔 안에 있어야 합니다.
* 다음의 **명확한 시야** 를 가지고 있어야 하며, 그 사이에 아무것도 없어야 합니다.


> [!WARNING]
> **📔 참고**: 근접 이벤트는 다음을 무시합니다 `maxCameraDistance`. 카메라 위치는 아무런 영향을 주지 않습니다. 아바타의 거리, 바라보는 방향, 시야만 고려됩니다. 다음을 참조하세요 [거리 제한](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/register-callback/#distance-limits) 에서 커서 이벤트에 대한 카메라 제한이 어떻게 작동하는지 확인하세요.


## 콜백 제거

근접 콜백을 제거하려면 해당하는 제거 함수를 사용하세요:

```ts
pointerEventsSystem.removeOnProximityDown(myEntity)
pointerEventsSystem.removeOnProximityUp(myEntity)
pointerEventsSystem.removeOnProximityEnter(myEntity)
pointerEventsSystem.removeOnProximityLeave(myEntity)
```

제거되면 엔터티는 더 이상 해당 근접 이벤트에 반응하지 않습니다.

## 근접 진입 및 이탈

다음을 사용하세요 `pointerEventsSystem.onProximityEnter` 는 플레이어가 엔터티의 근접 범위 안으로 들어올 때 콜백을 실행하고, `pointerEventsSystem.onProximityLeave` 는 나갈 때 실행합니다. 예를 들어 아이템과 상호작용할 수 있을 때 소리나 애니메이션 같은 추가 피드백 힌트를 플레이어에게 표시하는 데 사용할 수 있습니다.

```ts
pointerEventsSystem.onProximityEnter(
    {
        entity: myEntity,
        opts: {
            button: InputAction.IA_POINTER,
            hoverText: '가까이',
            maxDistance: 5,
        },
    },
    function () {
        console.log('플레이어가 근접 범위 안으로 들어왔습니다')
    }
)

pointerEventsSystem.onProximityLeave(
    {
        entity: myEntity,
        opts: {
            button: InputAction.IA_POINTER,
            hoverText: '가까이',
            maxDistance: 5,
        },
    },
    function () {
        console.log('플레이어가 근접 범위 밖으로 나갔습니다')
    }
)
```

## 예시: 근접 문

다음 예시는 플레이어가 가까이에 있을 때 버튼을 누르면 문을 열거나 닫으며, 문을 겨눌 필요가 없습니다.

```ts
const doorPivot = engine.addEntity()
Transform.create(doorPivot, { position: Vector3.create(3, 0, 4) })

const door = engine.addEntity()
GltfContainer.create(door, { src: 'assets/door.glb' })
Transform.create(door, { position: Vector3.create(-1, 0, 0), parent: doorPivot })

let isDoorOpen = false
const closedRot = Quaternion.fromEulerDegrees(0, 0, 0)
const openRot = Quaternion.fromEulerDegrees(0, 90, 0)

pointerEventsSystem.onProximityDown(
    {
        entity: door,
        opts: {
            button: InputAction.IA_PRIMARY,
            hoverText: '열기 / 닫기',
            maxDistance: 5,
            priority: 1,
        },
    },
    function () {
        if (isDoorOpen) {
            Tween.setRotate(doorPivot, openRot, closedRot, 700)
            isDoorOpen = false
        } else {
            Tween.setRotate(doorPivot, closedRot, openRot, 700)
            isDoorOpen = true
        }
    }
)
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/interactivity/button-events/proximity-events.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
