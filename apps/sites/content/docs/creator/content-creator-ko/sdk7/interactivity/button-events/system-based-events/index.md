> 씬에서 사용자 클릭을 처리하는 방법을 알아봅니다.

# 시스템 기반 이벤트

장면에 모두 같은 로직으로 활성화되는 비슷한 엔터티가 여러 개 있다면, 그 모두를 순회하는 하나의 시스템을 작성해서 그 동작을 한 번만 설명할 수 있습니다. 이것은 또한 가장 성능이 좋고 더 [데이터 지향적](/docs/creator/content-creator-ko/sdk7/architecture/data-oriented-programming/) 접근 방식입니다.

시스템을 사용해 감지할 수도 있습니다 [전역 입력 이벤트](#global-input-events), 그래서 플레이어의 커서가 어디를 조준하고 있는지와 상관없이 키가 눌릴 때마다 장면이 반응합니다.

단일 엔터티를 클릭하거나 버튼을 눌러 활성화하는 것만 원한다면, 가장 쉬운 방법은 [콜백 등록](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/register-callback/) 접근 방식입니다.

더 구체적인 사용자 지정 로직을 설정하려면, 원시 데이터를 다루고 [고급](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/advanced-button-events/) 접근 방식입니다.

엔터티가 상호작용 가능하려면 반드시 [콜라이더](/docs/creator/content-creator-ko/sdk7/3d/colliders/). 다음을 참조하세요. [장애물](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/click-events/#obstacles) 이 있어야 합니다. 이는 근접 클릭 이벤트에도 적용됩니다.

## 시스템 사용

입력에서 도우미 함수 중 하나를 실행해 버튼 이벤트를 확인하세요 `inputSystem` 네임스페이스를 매 틱마다 [시스템](/docs/creator/content-creator-ko/sdk7/architecture/systems/).

예를 들어, 다음 시스템은 `inputSystem.isTriggered()` 함수를 사용해 포인터가 클릭되었는지 확인합니다. 매 틱마다 버튼이 눌렸는지 검사합니다. 만약 `inputSystem.isTriggered()` 가 *true*이면 시스템은 이에 대한 응답으로 일부 사용자 지정 로직을 실행합니다.

```ts
engine.addSystem(() => {
	if (inputSystem.isTriggered(InputAction.IA_POINTER, PointerEventType.PET_DOWN)) {
		// 버튼 누름에 대한 로직
	}
})
```

다음 도우미 함수는 `inputSystem` 네임스페이스에서 사용할 수 있으며, 마찬가지로 매 틱마다 호출할 수 있습니다:

* `inputSystem.isTriggered`: 마지막 틱 이후 입력 동작이 발생했으면 true를 반환합니다.
* `inputSystem.isPressed`: 현재 입력이 눌려 있는 상태이면 true를 반환합니다. 버튼이 다시 올라갈 때까지 매 틱마다 true를 반환합니다.
* `inputSystem.getInputCommand`: 입력 동작에 대한 데이터가 담긴 객체를 반환합니다.

각각에 대한 자세한 내용은 아래 섹션을 참조하세요.

엔터티에서 버튼 이벤트를 처리할 때는 항상 플레이어에게 피드백을 제공해, 엔터티와 상호작용할 수 있다는 것을 플레이어가 알 수 있게 하세요. 만약 `PointerEvents` 컴포넌트를 엔터티에 추가하면, 플레이어는 커서를 그 엔터티 위에 올렸을 때 힌트를 보게 됩니다. 다음을 참조하세요 [피드백 표시](#show-feedback) 를 통해 상호작용 가능한 엔터티에 호버 힌트를 추가하는 방법을 알아보세요.

### 전역 입력 이벤트

사용하세요 `inputSystem.isTriggered` SDK가 추적하는 입력 중 어느 것이든 버튼이 눌리고 떼어지는 이벤트를 감지합니다.

```ts
engine.addSystem(() => {
	if (
		inputSystem.isTriggered(InputAction.IA_POINTER, PointerEventType.PET_DOWN)
	) {
		// 버튼 누름에 대한 로직
	}
})
```

위의 예시는 포인터가 어디에 있든, 혹은 그 경로에 어떤 엔터티가 있든 상관없이 버튼이 눌렸는지 확인합니다.

`inputSystem.isTriggered()` 가 *true* 오직 지정된 버튼이 현재 게임 루프의 틱에서 눌렸을 때만 *false*.


> [!WARNING]
> **📔 참고**: 포인터 이벤트가 감지되려면 플레이어가 장면의 경계 안에 서 있어야 합니다. 또한 플레이어의 커서가 잠겨 있어야 하며, 자유 커서 상태에서 버튼을 눌러도 감지되지 않습니다.
> 
> 상호작용하려는 모든 엔터티에는 반드시 둘 다 있어야 합니다 [`PointerEvents` 컴포넌트를 추가하는 것이 좋습니다](#show-feedback) 와 하나의 [콜라이더](/docs/creator/content-creator-ko/sdk7/3d/colliders/).


다음 `inputSystem.isTriggered` 함수는 다음 필수 인수를 받습니다:

* `InputAction`: 수신할 입력은 `InputAction` enum의 값으로 지정합니다. 다음을 참조하세요 [포인터 버튼](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/click-events/#pointer-buttons) 에서 지원되는 옵션을 확인하세요.
* `PointerEventType`: 수신할 이벤트 유형은 `PointerEventType` enum의 값으로 지정합니다. 다음을 참조하세요 [포인터 이벤트 유형](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/click-events/#types-of-pointer-events) 에서 지원되는 옵션을 확인하세요.

### 엔터티 활성화

특정 엔터티를 가리키는 동안 버튼 이벤트를 감지하려면, `inputSystem.isTriggered` 에 세 번째 선택적 인수를 전달하여 어떤 엔터티를 기준으로 확인할지 지정하세요.

```ts
// 엔터티에 PointerEvents 컴포넌트 부여
PointerEvents.create(myEntity, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_PRIMARY,
				showFeedback: false,
			},
		},
	],
})

// 시스템 생성
engine.addSystem(() => {
	if (
		inputSystem.isTriggered(
			InputAction.IA_POINTER,
			PointerEventType.PET_DOWN,
			myEntity
		)
	) {
		// myEntity에서 버튼 누름에 대한 로직
	}
})
```

위의 예시는 하나의 하드코딩된 엔터티가 포인터 버튼(왼쪽 마우스 버튼)으로 눌렸는지 매 틱마다 확인합니다.

다음 `inputSystem.isTriggered` 함수는 다음 인수를 받습니다:

* `InputAction`: 수신할 입력은 `InputAction` enum의 값으로 지정합니다. 다음을 참조하세요 [포인터 버튼](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/click-events/#pointer-buttons) 에서 지원되는 옵션을 확인하세요.
* `PointerEventType`: 수신할 이벤트 유형은 `PointerEventType` enum의 값으로 지정합니다. 다음을 참조하세요 [포인터 이벤트 유형](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/click-events/#types-of-pointer-events) 에서 지원되는 옵션을 확인하세요.
* `Entity` *(선택 사항)*: 어떤 엔터티에서 이 이벤트를 확인할지 지정합니다. 값을 제공하지 않으면 플레이어의 커서가 어디를 가리키고 있었는지와 관계없이 버튼의 전역 누름을 확인합니다.

이 예시에서는 또한 `PointerEvents` 컴포넌트를 상호작용하려는 엔터티에 추가하고 있다는 점에 유의하세요. 이 단계는 필수이며, 이 컴포넌트가 없으면 엔터티는 `inputSystem`의 어떤 함수로도 감지할 수 없습니다. 다음을 참조하세요 [피드백 표시](#show-feedback) 에 대한 자세한 내용은 `PointerEvents` 컴포넌트가


> [!WARNING]
> **📔 참고**: 그리고 [콜라이더](/docs/creator/content-creator-ko/sdk7/3d/colliders/). 다음을 참조하세요. [장애물](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/click-events/#obstacles) 을 참고하세요.


플레이어가 같은 방식으로 상호작용할 수 있는 엔터티가 여러 개 있다면, `inputSystem.getInputCommand`를 사용하는 것을 고려하세요. 이 명령은 엔터티 ID를 포함한 전역 클릭 명령에 대한 정보를 반환하며, 이를 사용해 모두를 처리할 수 있는 하나의 함수를 실행할 수 있습니다.

```ts
engine.addSystem(() => {
	const result = inputSystem.getInputCommand(
		InputAction.IA_POINTER,
		PointerEventType.PET_DOWN
	)
	if (result) {
		if (result.hit?.entityId === myEntity) {
			// 클릭 처리
		}
	}
})
```


> [!WARNING]
> **📔 참고**상호작용하려는 모든 엔터티에는 반드시 둘 다 있어야 합니다 [`PointerEvents` 컴포넌트를 추가하는 것이 좋습니다](#show-feedback) 와 하나의 [콜라이더](/docs/creator/content-creator-ko/sdk7/3d/colliders/).


다음을 참조하세요. [입력 동작 데이터](#data-from-input-action) 에 대한 자세한 내용은 여기에서 확인하세요. 이 메서드는 또한 포인터 이벤트의 히트에 대한 더 자세한 데이터도 제공합니다.

### 입력 버튼 올림

마찬가지로 `PointerEventType.PET_UP`.

```ts
engine.addSystem(() => {
	if (
		inputSystem.isTriggered(
			InputAction.IA_POINTER,
			PointerEventType.PET_UP,
			myEntity
		)
	) {
		// myEntity를 가리키는 동안 버튼 떼기에 대한 로직
	}
})
```


> [!WARNING]
> **📔 참고**: 특정 엔터티를 기준으로 포인터 버튼 올림 이벤트를 확인할 때, 버튼을 눌렀을 때 커서가 어디를 가리키고 있었는지는 고려하지 않습니다. 버튼을 뗄 때 커서가 어디를 가리키고 있는지만 고려합니다.
> 
> 또한 엔터티에는 반드시 [`PointerEvents` 컴포넌트를 추가하는 것이 좋습니다](#show-feedback) 와 하나의 [콜라이더](/docs/creator/content-creator-ko/sdk7/3d/colliders/).


### 눌린 버튼 확인

현재 버튼이 눌려 있는지 `inputSystem.isPressed()` 를 사용해 시스템 내에서 확인하세요.

```ts
engine.addSystem(() => {
	if (inputSystem.isPressed(InputAction.IA_POINTER)) {
		// 버튼이 계속 눌려 있는 것에 대한 로직
	}
})
```

`inputSystem.isPressed()` 가 *true* 버튼이 현재 계속 눌려 있는지, 버튼을 언제 눌렀는지와 상관없이, 그리고 플레이어의 커서가 어디를 가리키고 있는지와 상관없이 반환합니다. *false*.

다음 `inputSystem.isPressed` 함수는 단일 인수를 받습니다:

* `InputAction`: 수신할 입력은 `InputAction` enum의 값으로 지정합니다. 다음을 참조하세요 [포인터 버튼](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/click-events/#pointer-buttons) 에서 지원되는 옵션을 확인하세요.

### 여러 엔터티 처리

장면에 포인터 이벤트에 동일하게 영향을 받는 엔터티가 여러 개 있다면, 모두를 순회하는 시스템을 작성하는 것이 좋습니다.

```ts
engine.addSystem(() => {
	const activatedEntites = engine.getEntitiesWith(PointerEvents)
	for (const [entity] of activatedEntites) {
		if (
			inputSystem.isTriggered(
				InputAction.IA_POINTER,
				PointerEventType.PET_DOWN,
				entity
			)
		) {
			// 엔터티와의 상호작용에 대한 로직
		}
	}
})
```

이 예시는 [컴포넌트 쿼리](/docs/creator/content-creator-ko/sdk7/architecture/querying-components/) 를 사용해 `PointerEvents` 컴포넌트를 가진 모든 엔터티를 순회합니다. 그런 다음 이 엔터티 각각을 `inputSystem.isTriggered`로 확인하며, 하나씩 순회합니다. 이들 엔터티 중 어떤 것에서든 입력 동작이 감지되면, 사용자 지정 로직을 수행합니다.

순회하는 대신 *모든* 컴포넌트를 가진 엔터티들을 한 시스템에서 `PointerEvents` 로 순회하는 대신, 서로 다른 방식으로 동작해야 하는 엔터티를 처리하기 위해 별도의 시스템을 작성할 수 있습니다. 권장되는 방법은 서로 다른 유형의 엔터티를 특정 [사용자 지정 컴포넌트](/docs/creator/content-creator-ko/sdk7/architecture/custom-components/)로 표시하고, 별도의 시스템에서 순회하는 것입니다.

```ts
engine.addSystem(() => {
	const activatedDoors = engine.getEntitiesWith(IsDoor, PointerEvents)
	for (const [entity] of activatedDoors) {
		if (
			inputSystem.isTriggered(
				InputAction.IA_POINTER,
				PointerEventType.PET_DOWN,
				entity
			)
		) {
			openDoor(entity)
		}
	}
})

engine.addSystem(() => {
	const pickedUpGems = engine.getEntitiesWith(IsGem, PointerEvents)
	for (const [entity] of pickedUpGems) {
		if (
			inputSystem.isTriggered(
				InputAction.IA_POINTER,
				PointerEventType.PET_DOWN,
				entity
			)
		) {
			pickUp(entity)
		}
	}
})
```

이 예시에는 사용자 지정 컴포넌트인 `IsDoor` 를 가진 모든 엔터티를 순회하는 하나의 시스템과, 사용자 지정 컴포넌트인 `isGem`를 가진 모든 엔터티를 순회하는 또 다른 시스템이 있습니다. 두 시스템 모두 일치하는 각 엔터티가 포인터 버튼으로 활성화되었는지 확인합니다.

장면 코드를 이렇게 구성하는 방식은 매우 [데이터 지향적](/docs/creator/content-creator-ko/sdk7/architecture/data-oriented-programming/) 하며, 메모리 자원을 매우 효율적으로 사용하게 됩니다.

\## 엔터티의 여러 버튼

다음과 같은 도우미 함수를 사용하면 `pointerEventsSystem.onPointerDown`각 엔터티는 하나의 이벤트 유형만 가질 수 있습니다. 두 개의 서로 다른 버튼 이벤트를 등록할 수는 없습니다. 시스템 기반 이벤트를 사용할 때는 이런 제한이 없습니다. 예를 들어, 플레이어가 E와 F를 모두 눌러 상호작용할 수 있는 엔터티가 있으며, 각각 서로 다른 기능을 트리거합니다.

```ts
    engine.addSystem(() => {
        if (inputSystem.isTriggered(InputAction.IA_PRIMARY, PointerEventType.PET_DOWN, myEntity)) {
            // E에 반응
        }
        if (inputSystem.isTriggered(InputAction.IA_SECONDARY, PointerEventType.PET_DOWN, myEntity)) {
            // F에 반응
        }
    })
```

## 피드백 표시

엔터티를 가리킬 때 UI 힌트를 표시하려면 엔터티의 `PointerEvents` 컴포넌트가

```ts
// 엔티티 생성
const myEntity = engine.addEntity()

// 엔터티에 PointerEvents 컴포넌트 부여
PointerEvents.create(myEntity, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_POINTER,
			},
		},
	],
})
```

플레이어의 커서가 이 엔터티의 콜라이더를 가리킬 때마다 UI는 엔터티와 상호작용할 수 있음을 나타내는 호버 힌트를 표시합니다. 구성할 수 있는 항목에 대한 자세한 내용은 아래 섹션을 참조하세요.


> [!WARNING]
> **📔 참고**: `PointerEvents` 컴포넌트는 호버 피드백 표시만 처리합니다. 사용자 지정 로직으로 버튼 이벤트 자체를 처리하려면 다음을 참조하세요 [시스템 사용하기](#using-a-system).


다음 `PointerEvents` 컴포넌트에는 최소 하나의 포인터 이벤트 정의가 필요합니다. 각 포인터 이벤트 정의는 다음으로 구성할 수 있습니다:

* `eventType`: 수신할 이벤트 유형은 `PointerEventType` enum의 값으로 지정합니다. 다음을 참조하세요 [포인터 이벤트 유형](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/click-events/#types-of-pointer-events) 에서 지원되는 옵션을 확인하세요.
* `eventInfo`: 다음 필드를 포함할 수 있는 객체:
  * `button` (*필수*): 수신할 입력은 `InputAction` enum의 값으로 지정합니다. 다음을 참조하세요 [포인터 버튼](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/click-events/#pointer-buttons) 에서 지원되는 옵션을 확인하세요.
  * `hoverText` *(선택 사항)*: 호버 피드백 힌트에 표시할 문자열입니다. 기본값은 "Interact"입니다.
  * `showFeedback` *(선택 사항)*: false이면 이 엔터티의 호버 힌트와 가장자리 강조 표시가 모두 숨겨집니다. *true* 기본값으로.
  * `showHighlight` *(선택 사항)*: true이면 플레이어가 엔터티 위에 커서를 올렸을 때 가장자리 강조 표시가 보입니다. *true* 이 값은 `showFeedback` 가 *true*.
  * `maxDistance` *(선택 사항)*: 플레이어의 **아바타** 가 엔터티로부터 얼마나 떨어져 있어도 여전히 상호작용할 수 있는지, 미터 단위로 나타냅니다. 기본값은 *10미터*. 다음을 참조하세요. [최대 거리](#max-distance).
  * `maxCameraDistance` *(선택 사항)*: 카메라가 엔터티로부터 얼마나 떨어져 있어도 여전히 상호작용할 수 있는지, 미터 단위로 나타냅니다. 기본값은 설정되어 있지 않습니다. 다음을 참조하세요 **활성 카메라** 가 엔터티로부터 얼마나 떨어져 있어도 여전히 상호작용할 수 있는지, 미터 단위로 나타냅니다. 기본값은 설정되어 있지 않습니다. 다음을 참조하세요 [최대 거리](#max-distance).
  * `priority` *(선택 사항)*: 여러 엔터티가 겹칠 때 어떤 엔터티가 우선되는지 지정합니다. 값이 높을수록 우선합니다. *0* 기본값으로.

하나의 `PointerEvents` 컴포넌트는 여러 포인터 이벤트 정의를 담을 수 있으며, 버튼마다 서로 다른 이벤트를 감지할 수 있습니다. 각 엔터티는 오직 하나의 *하나* `PointerEvents` 컴포넌트만 가질 수 있지만, 이 컴포넌트의 `pointerEvents` 배열에는 응답할 각 이벤트마다 하나씩 여러 객체를 포함할 수 있습니다.

```ts
PointerEvents.create(NPCEntity, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_POINTER,
				hoverText: '인사하기',
			},
		},
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_PRIMARY,
				hoverText: '조르기',
			},
		},
		{
			eventType: PointerEventType.PET_UP,
			eventInfo: {
				button: InputAction.IA_PRIMARY,
				hoverText: '조르기 중단',
			},
		},
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_SECONDARY,
				hoverText: '보석 주기',
			},
		},
	],
})
```

플레이어는 커서 주변에 방사형으로 표시되는 여러 레이블, 즉 각 포인터 이벤트마다 하나씩 보게 됩니다.

아래 예시는 `PointerEvents` 를 사용해 호버 힌트를 표시하는 것과, 실제로 플레이어의 동작을 사용자 지정 로직으로 처리하는 시스템을 함께 결합합니다.

```ts
// 엔티티 생성
const myEntity = engine.addEntity()

// 엔터티에 호버 피드백 제공
PointerEvents.create(myEntity, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_POINTER,
				hoverText: '열기',
			},
		},
	],
})

// 엔터티의 클릭 이벤트 처리
engine.addSystem(() => {
	if (
		inputSystem.isTriggered(
			InputAction.IA_POINTER,
			PointerEventType.PET_DOWN,
			myEntity
		)
	) {
		// 입력 동작에 대한 사용자 지정 로직
	}
})
```

### 호버 피드백

플레이어가 아이템 위에 커서를 올리면 `PointerEvents` 컴포넌트가 있으면, 플레이어는 다음을 보게 됩니다:

* 엔터티의 가장자리 강조 표시
* 커서 근처에 버튼 아이콘과 "Interact"라는 문자열이 있는 호버 힌트.

이 요소들은 켜거나 끄고 맞춤 설정할 수 있습니다.

UI의 호버 피드백은 `button` 필드에서 선택한 입력에 따라 다른 아이콘을 표시합니다. PC에서는 `E` 에 대해 `InputAction.IA_PRIMARY`, `F` 에 대해 `InputAction.IA_SECONDARY`, 그리고 `InputAction.IA_POINTER`.

에 대해 마우스 아이콘을 표시합니다. `hoverText` 값을 변경해 문자열을 바꾸세요. 이 문자열은 짧게 유지하여 읽기 쉽고 화면을 너무 방해하지 않도록 하세요.

```ts
// 엔티티 생성
const chest = engine.addEntity()

// 엔터티에 PointerEvents 컴포넌트 부여
PointerEvents.create(chest, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_POINTER,
				hoverText: '열기',
			},
		},
	],
})
```

위의 예시에서 포인터 이벤트에는 `hoverText`에 대한 값이 포함됩니다. 이 필드는 플레이어가 엔터티를 가리키는 동안 UI에 표시할 문자열을 정의합니다. 기본적으로 이 문자열은 *상호작용*.


> [!NOTE]
> **💡 팁**: `hoverText` 이라고 표시합니다. `열기`, `활성화`, `집기`, `선택하세요`를 뜻합니다. 이 문자열들은 플레이어의 집중을 너무 많이 빼앗지 않도록 최대한 짧아야 합니다.


엔터티에 여러 포인터 이벤트가 있으면, 각각의 호버 힌트는 커서 주변에 방사형으로 표시됩니다.

다음 `hoverText` 의 `.UP` 포인터 이벤트는 플레이어가 이미 해당 키를 누른 상태로 엔터티를 가리키고 있을 때만 표시됩니다.

엔터티에 `DOWN` 포인터 이벤트와 `UP` 포인터 이벤트가 둘 다 있으면, `DOWN` 동작에 대한 힌트는 버튼이 눌려 있지 않을 때 표시됩니다. 힌트는 버튼이 눌려 있고 계속 눌린 상태일 때만 `UP` 이벤트의 힌트로 전환됩니다.

```ts
// 엔티티 생성
const entity = engine.addEntity()

// 엔터티에 PointerEvents 컴포넌트 부여
PointerEvents.create(entity, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_POINTER,
				hoverText: '드래그',
			},
		},
		{
			eventType: PointerEventType.PET_UP,
			eventInfo: {
				button: InputAction.IA_POINTER,
				hoverText: '놓기',
			},
		},
	],
})
```

호버 힌트를 숨기되 가장자리 강조 표시는 남기려면, `hoverText` 의 값을 ""로 설정하세요.

```ts
// 엔티티 생성
const chest = engine.addEntity()

// 엔터티에 PointerEvents 컴포넌트 부여
PointerEvents.create(chest, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_POINTER,
				hoverText: '',
			},
		},
	],
})
```

가장자리 강조 표시는 숨기되 호버 힌트는 남기려면 `showHighlight` 을 *false*.

```ts
// 엔티티 생성
const chest = engine.addEntity()

// 엔터티에 PointerEvents 컴포넌트 부여
PointerEvents.create(chest, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_POINTER,
				hoverText: '문 열기',
				showHighlight: false,
			},
		},
	],
})
```

호버 힌트와 가장자리 강조 표시를 모두 숨기려면 `showFeedback` 을 *false*를 설정하세요. 이렇게 하면 커서는 어떤 아이콘, 텍스트, 가장자리 강조도 표시하지 않습니다. 또는 단순히 `PointerEvents` 컴포넌트를 엔터티에서 제거해도 됩니다.

```ts
// 엔티티 생성
const chest = engine.addEntity()

// 엔터티에 PointerEvents 컴포넌트 부여
PointerEvents.create(chest, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_POINTER,
				showFeedback: false,
			},
		},
	],
})
```

### 최대 거리

일부 엔터티는 의도적으로 가까운 거리에서만 상호작용 가능하도록 할 수 있습니다. 플레이어가 엔터티에서 너무 멀리 있으면 호버 힌트가 커서 옆에 표시되지 않습니다.

기본적으로 엔터티는 플레이어의 **아바타** 가 *10미터* 의 범위 안에 있을 때만 클릭할 수 있습니다. 이 범위는 `maxDistance` 속성을 설정해 변경할 수 있습니다.

```ts
// 엔티티 생성
const myEntity = engine.addEntity()

// 엔터티에 PointerEvents 컴포넌트 부여
PointerEvents.create(myEntity, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_POINTER,
				maxDistance: 6,
			},
		},
	],
})

// 포인터 이벤트를 처리할 시스템 구현
engine.addSystem(() => {
	// 포인터 이벤트 데이터 가져오기
	const cmd = inputSystem.getInputCommand(
		InputAction.IA_POINTER,
		PointerEventType.PET_DOWN,
		myEntity
	)

	// 클릭이 충분히 가까웠는지 확인
	if (cmd && cmd.hit && cmd.hit.length < 6) {
		// 무언가 수행
	}
})
```

위의 예시는 호버 힌트의 최대 거리를 *6미터*로 설정합니다. 입력 동작을 처리하는 로직도 같은 규칙을 따르도록 하세요. 다음을 참조하세요 [입력 동작 데이터](#data-from-input-action) 에서 입력 동작의 거리를 얻는 방법을 확인하세요.

`maxDistance` 는 플레이어의 **아바타**에서 측정되므로, 1인칭과 3인칭에서 동일하게 동작합니다.

상호작용을 제한하기 위해 **활성 카메라** 가 얼마나 떨어져 있는지를 기준으로 하려면 대신 `maxCameraDistance`를 사용하세요. 둘 다 설정하면 둘 중 하나만 통과해도 충분합니다. 전체 규칙은 다음을 참조하세요 [거리 제한](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/register-callback/#distance-limits) 을 참조하세요.


> [!WARNING]
> **📔 참고**: `maxPlayerDistance` 는 더 이상 사용되지 않는 `maxDistance`의 별칭입니다. 둘 다 아바타로부터의 거리를 측정하므로, `maxDistance` 를 새 장면에서 사용하세요.


## 고급 사용자 지정 힌트

다음 `PointerEvents` 컴포넌트는 플레이어의 커서가 엔터티 위에 호버를 시작하면 UI 힌트를 쉽게 추가합니다. 힌트가 다른 Decentraland 장면에서 플레이어가 익숙한 방식과 일관되게 동작하는 것은 일반적으로 좋은 일입니다. 그러나 경우에 따라 어떤 것이 상호작용 가능하다는 것을 사용자 지정 방식으로 알리고 싶을 수 있습니다. 예를 들어, 플레이어가 엔터티 위에 호버를 시작할 때 미묘한 소리를 재생할 수 있습니다. 호버 중일 때 엔터티 주위에 빛나는 강조 표시를 보여주고, 더 이상 호버하지 않을 때 숨길 수도 있습니다. 특정 게임플레이 메커닉에 사용할 수도 있습니다.

다음을 사용하세요 `inputSystem.isTriggered()` 함수와 함께 `PointerEventType.PET_HOVER_ENTER` 및 `PointerEventType.PET_HOVER_LEAVE` 이벤트를 사용해, 플레이어의 커서가 엔터티의 콜라이더를 가리키기 시작할 때와 커서가 더 이상 가리키지 않을 때마다 사용자 지정 동작을 수행하세요.

아래 예시는 엔터티의 크기를 *1.5* 로 키우며, 커서가 그 콜라이더를 가리키기 시작하면 크기를 다시 *1* 로 되돌립니다.

```ts
engine.addSystem(() => {
	const meshEntities = engine.getEntitiesWith(MeshRenderer)
	for (const [entity] of meshEntities) {
		if (
			inputSystem.isTriggered(
				InputAction.IA_POINTER,
				PointerEventType.PET_HOVER_ENTER,
				entity
			)
		) {
			Transform.getMutable(entity).scale = Vector3.create(1.5, 1.5, 1.5)
		}

		if (
			inputSystem.isTriggered(
				InputAction.IA_POINTER,
				PointerEventType.PET_HOVER_LEAVE,
				entity
			)
		) {
			Transform.getMutable(entity).scale = Vector3.create(1, 1, 1)
		}
	}
})
```


> [!WARNING]
> **📔 참고**상호작용하려는 모든 엔터티에는 반드시 둘 다 있어야 합니다 [`PointerEvents` 컴포넌트를 추가하는 것이 좋습니다](#show-feedback) 와 하나의 [콜라이더](/docs/creator/content-creator-ko/sdk7/3d/colliders/).


## 근접 이벤트

커서가 어디를 조준하고 있는지와 상관없이 `PET_DOWN` 를 사용하는 엔터티에서 근접 버튼 누름을 확인하세요 `InteractionType.PROXIMITY`. 포인터 이벤트 정의에서 `interactionType` 필드를 `eventInfo`옆에 설정해 이벤트를 근접 기반으로 표시하세요.

다음을 참조하세요. [**근접 이벤트**](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/proximity-events/) 에서 사용할 수 있는 도우미 함수 등을 포함한 자세한 내용은 여기에서 확인하세요.

```ts
PointerEvents.create(myEntity, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_PRIMARY,
				hoverText: 'E를 누르세요',
				maxDistance: 5
			},
			interactionType: InteractionType.PROXIMITY,
		},
	],
})

engine.addSystem(() => {
	if (inputSystem.isTriggered(InputAction.IA_PRIMARY, PointerEventType.PET_DOWN, myEntity)) {
		console.log('플레이어가 엔티티 근처에서 버튼을 눌렀습니다')
	}
})
```

다음과 같은 헬퍼를 사용하면 `pointerEventsSystem.onProximityDown`, 엔티티는 동시에 하나의 커서 핸들러와 하나의 근접 핸들러를 가질 수 있으며, 두 콜백이 모두 실행됩니다. 하지만 같은 핸들러에 대해 서로 다른 두 버튼을 등록할 수는 없습니다. 시스템 기반 이벤트에는 이런 제한이 없습니다. 동작당 항목 하나를 정의하면 됩니다. 예를 들어, 커서를 가리키고 E를 눌러서 상호작용할 수도 있고, 커서를 그 위에 두지 않은 채 근처로 걸어가서 E를 눌러서도 상호작용할 수 있는 엔티티가 있습니다.

```ts
	PointerEvents.create(myEntity, {
        pointerEvents: [
            {
                eventType: PointerEventType.PET_DOWN,
                eventInfo: {
                    button: InputAction.IA_POINTER,
                    hoverText: '클릭',
                    maxDistance: 15,
                },
                interactionType: InteractionType.CURSOR,
            },
            {
                eventType: PointerEventType.PET_DOWN,
                eventInfo: {
                    button: InputAction.IA_PRIMARY,
                    hoverText: 'E를 누르세요',
                    maxDistance: 15,
                },
                interactionType: InteractionType.PROXIMITY,
            },
        ],
    })
    
    engine.addSystem(() => {
        if (inputSystem.isTriggered(InputAction.IA_POINTER, PointerEventType.PET_DOWN, myEntity)) {
            console.log('플레이어가 엔티티를 클릭했습니다')
        }
        if (inputSystem.isTriggered(InputAction.IA_PRIMARY, PointerEventType.PET_DOWN, myEntity)) {
            console.log('플레이어가 엔티티 근처에서 버튼을 눌렀습니다')
        }
    })
```

다음도 사용할 수 있습니다 `inputSystem.isTriggered()` 와 함께 `PET_PROXIMITY_ENTER` 및 `PET_PROXIMITY_LEAVE` 이벤트 유형을 사용하면 플레이어가 엔티티의 근접 범위에 들어오거나 벗어날 때 반응할 수 있습니다. 이는 근접 활성화된 엔티티에 추가 피드백을 표시하는 데 유용합니다. 예를 들어 소리나 애니메이션을 재생할 수 있습니다.

이것이 작동하려면, 엔티티에 `PointerEvents` 해당하는 근접 이벤트 유형이 정의된 컴포넌트가 있어야 합니다.

```ts
PointerEvents.create(myEntity, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_PROXIMITY_ENTER,
			eventInfo: {
				button: InputAction.IA_POINTER,
				maxDistance: 5,
			},
		},
		{
			eventType: PointerEventType.PET_PROXIMITY_LEAVE,
			eventInfo: {
				button: InputAction.IA_POINTER,
				maxDistance: 5,
				showFeedback: false,
			},
		},
	],
})

engine.addSystem(() => {
	if (inputSystem.isTriggered(InputAction.IA_POINTER, PointerEventType.PET_PROXIMITY_ENTER, myEntity)) {
		console.log('플레이어가 근접 범위에 들어왔습니다')
	}
	if (inputSystem.isTriggered(InputAction.IA_POINTER, PointerEventType.PET_PROXIMITY_LEAVE, myEntity)) {
		console.log('플레이어가 근접 범위에서 벗어났습니다')
	}
})
```

## 입력 동작 데이터

입력 액션에서 데이터를 가져옵니다. 예를 들어 눌린 버튼, 맞은 엔티티, 레이의 방향과 길이 등이 있습니다. ([문서를 참조하세요](https://github.com/decentraland/docs/tree/main/README.md))를 참조하여 사용 가능한 모든 데이터에 대한 설명을 확인하세요.

이 데이터를 가져오려면 `inputSystem.getInputCommand`를 사용하세요. 이 함수는 입력 이벤트에 대한 데이터가 포함된 전체 데이터 구조를 반환합니다.

```ts
engine.addSystem(() => {
	const cmd = inputSystem.getInputCommand(
		InputAction.IA_POINTER,
		PointerEventType.PET_DOWN,
		myEntity
	)
	if (cmd) {
		console.log(cmd.hit?.entityId)
	}
})
```

쿼리와 일치하는 입력 액션이 없다면, `inputSystem.getInputCommand` 가 *null*. 이 논리를 처리하도록 하세요.

### 최대 클릭 거리

엔티티가 가까운 거리에서만 클릭되도록 최대 거리를 강제하려면, `hit.length` 이벤트 데이터의 속성을 가져오세요.

```ts
// 포인터 이벤트를 처리할 시스템 구현
engine.addSystem(() => {
	// 포인터 이벤트 데이터 가져오기
	const cmd = inputSystem.getInputCommand(
		InputAction.IA_POINTER,
		PointerEventType.PET_DOWN,
		myEntity
	)

	// 클릭이 충분히 가까웠는지 확인
	if (cmd && cmd.hit && cmd.hit.length < 6) {
		// 무언가 수행
	}
})
```


> [!WARNING]
> **📔 참고**: 멀리 있는 이벤트를 무시한다면, 반드시 `maxDistance` 매개변수를 `PointerEvents` 컴포넌트에 설정하여 일관되게 동작하도록 하세요.


### 모델 내부의 서로 다른 메쉬

종종, *.glTF* 3D 모델은 각각 고유한 내부 이름을 가진 여러 메쉬로 구성됩니다. 모든 버튼 이벤트에는 어떤 특정 메쉬가 클릭되었는지에 대한 정보가 포함되므로, 이 정보를 사용해 각 경우마다 서로 다른 클릭 동작을 트리거할 수 있습니다.

모델 내부의 메쉬 이름을 확인하려면, 다음과 같은 편집 도구로 3D 모델을 열어야 합니다, [Blender](https://www.blender.org/) 예를 들어.

![](/docs/_remote/5f167d2485c1df5d331a16b6.webp)


> [!NOTE]
> **💡 팁**: 클릭된 메쉬의 이름은 로그로 출력하고 콘솔에서 읽어 확인할 수도 있습니다.


다음의 `meshName` 속성을 `hit` 객체의 일부로 접근합니다. 이 객체는 클릭 이벤트에서 반환됩니다.

아래 예제에서는 `firePlace`라는 메쉬를 포함한 집 모델이 있습니다. 해당 메쉬가 클릭될 때에만 벽난로를 켜고자 합니다.

```ts
engine.addSystem(() => {
	const cmd = inputSystem.getInputCommand(
		InputAction.IA_POINTER,
		PointerEventType.PET_DOWN,
		myEntity
	)
	if (cmd && cmd.hit?.meshName === 'firePlace') {
		// 불을 켬
	}
})
```


> [!WARNING]
> **📔 참고**: 이는 포인터 기반 이벤트에만 적용되며, 근접 기반 이벤트에는 적용되지 않습니다. 그 경우 모델의 모든 메쉬가 한 번에 플레이어의 근접 범위 안에 있을 수 있기 때문입니다.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/interactivity/button-events/system-based-events.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
