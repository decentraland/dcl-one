> 이름이나 태그로 코드에서 아이템을 참조합니다.

# 코드에서 아이템 참조

Creator Hub의 드래그 앤 드롭 인터페이스를 통해 추가된 항목을 코드에서 참조할 수 있습니다. 이는 해당 항목에 정교한 동작을 추가하거나 코드의 다른 부분에서 참조하는 데 유용합니다.

## 이름으로 가져오기

Creator Hub를 사용하여 항목을 캔버스로 끌어다 놓아 엔티티를 추가할 때, 각 엔티티에는 고유한 이름이 있습니다. 다음을 사용하세요 `engine.getEntityOrNullByName()` 함수를 사용해 코드에서 이러한 엔티티 중 하나를 참조할 수 있습니다.

다음을 사용하세요 `EntityNames` 열거형을 사용하면 Creator Hub를 통해 추가한 엔티티의 이름에 쉽게 접근할 수 있으며, 또는 Scene Editor의 씬 엔티티 트리 뷰에 표시된 그대로 이름을 문자열로 작성할 수도 있습니다.

```ts
import { EntityNames } from '../assets/scene/entity-names'

function main() {

	// EntityNames 열거형을 사용
	const door1 = engine.getEntityOrNullByName(EntityNames.Door_1)

	// 이름을 문자열로 작성
	const door2 = engine.getEntityOrNullByName('Door 2')

	// 두 문이 씬에 모두 존재하는지 확인
	if (door1 && door2) {
		// 
	}

}
```

![](/docs/_remote/6c4b9156f30308b7eb4c3fc9.webp)

해당 `EntityNames` 열거형에는 Creator Hub에서 추가한 엔티티의 전체 목록이 들어 있으며, 변경 사항을 적용하는 즉시 바로 업데이트됩니다. 가져온다면 `EntityNames.` 을 코드에 가져오면, IDE에서 사용 가능한 모든 엔티티 이름이 포함된 드롭다운을 보여줍니다.

![](/docs/_remote/c76350566b1b5677df83aaca.webp)

다음을 사용할 수도 있습니다 `engine.getEntityByName<EntityNames>()` 함수에 `<EntityNames>` 를 [TypeScript 제네릭](https://www.typescriptlang.org/docs/handbook/2/generics.html)으로 전달해, 해당 이름의 엔티티가 씬에 실제로 존재하는지 검증할 수 있습니다. 참조된 엔티티의 이름이 Creator Hub에서 변경되면 이 메서드는 오류로 경고합니다. 이 함수의 출력은 `null`일 수 없으므로, 엔티티가 존재하는지 확인하지 않아도 됩니다.

```ts
import { EntityNames } from '../assets/scene/entity-names'

function main() {

	const door1 = engine.getEntityByName<EntityNames>(EntityNames.Door_1)

	// null 여부를 확인할 필요 없음
	console.log(Transform.get(door1).position.x)

}
```


> [!WARNING]
> **📔 참고**: 반드시 다음만 사용하세요 `engine.getEntityOrNullByName()` 및 `engine.getEntityByName()` 다음 내부에서만 `main()` 함수, 그리고 다음 이후에 실행되는 함수에서 `main()`또는 시스템 안에서 사용하세요. 이러한 컨텍스트 밖에서 사용하면 Scene Editor에서 생성된 엔티티가 아직 인스턴스화되지 않았을 수 있습니다.


위의 어떤 방법으로든 엔티티에 대한 참조를 가져오면, 컴포넌트를 추가하거나 제거하고, 기존 컴포넌트의 값을 수정하거나, 심지어 엔진에서 엔티티를 제거하는 등 원하는 작업을 자유롭게 수행할 수 있습니다.

```ts
import { EntityNames } from '../assets/scene/entity-names'

function main() {
	// 엔티티 가져오기
	const door = engine.getEntityOrNullByName(EntityNames.Door_3)
	// 엔티티가 존재하는지 확인
	if (door) {
		// 포인터 이벤트 콜백 추가
		pointerEventsSystem.onPointerDown(
			{
				entity: door,
				opts: { button: InputAction.IA_PRIMARY, hoverText: '열기' },
			},
			function () {
				// 문 열기
			}
		)
	}
}
```

Scene Editor를 통해 추가된 모든 엔티티에는 `이름` 컴포넌트가 있으며, 다음처럼 모두 순회할 수도 있습니다:

```ts
function main() {
	for (const [entity, name] of engine.getEntitiesWith(Name)) {
		console.log({ entity, name })
	}
}
```

## 태그로 가져오기

엔티티를 태그로도 가져올 수 있습니다. 태그는 엔티티를 함께 그룹화하는 방법이며, 같은 목적이나 동작을 가진 엔티티를 식별하는 데 유용합니다.

다음을 통해 엔티티에 태그를 추가할 수 있습니다 **태그** 섹션에서 항목 속성 패널 상단에 있는 **태그 그룹 1** 에서 **태그 그룹 4**과 같은 일반 태그를 선택하거나, 더 구체적인 이름으로 직접 만들 수 있습니다.

![](/docs/_remote/feb4023234754c8ebd2b7590.webp)


> [!NOTE]
> **💡 팁**: 하나의 엔티티에는 여러 태그를 할당할 수 있습니다.
> 
> <img src="/docs/_remote/22e3f2ff154ade672d73545c.webp" alt="" data-size="original">


그다음 다음을 사용해 특정 태그를 가진 모든 엔티티를 가져올 수 있습니다 `engine.getEntitiesByTag()` 함수. 이는 같은 목적이나 동작을 가진 엔티티 그룹을 순회하고 싶을 때 이상적입니다.

```ts
import { engine } from '@dcl/sdk/ecs'

export function main() {
	const taggedEntities = engine.getEntitiesByTag('myTag')
  
	for (const entity of taggedEntities) {
      // 각 엔티티로 무언가 수행
    }
}
```

코드에서 엔티티의 태그를 추가하거나 제거할 수도 있습니다. 이는 논리에 따라 태그를 변경하고 싶거나, 특정 태그를 가진 엔티티를 동적으로 생성하고 싶을 때 유용합니다.

```ts
import { Tags } from '@dcl/sdk/ecs'

Tags.remove(entity, tagName);
Tags.add(entity, tagName);
```

## 항목의 모든 자식 가져오기

특정 항목에 대한 참조를 얻으면, 화면 왼쪽의 엔티티 트리에서 해당 항목의 자식으로 그룹화된 모든 항목을 가져올 수 있습니다. 아래 스크립트는 부모 엔티티를 가져온 다음 Transform 컴포넌트가 있는 각 자식을 순회합니다. 그런 다음 각 항목을 순회하면서 원하는 사용자 지정 로직을 적용할 수 있습니다.

```ts
import { engine, Entity, Transform, Name, getEntitiesWithParent } from '@dcl/sdk/ecs'
import { EntityNames } from '../assets/scene/entity-names'

function main() {
	// 부모 엔티티 가져오기
	const parent = engine.getEntityByName<EntityNames>(EntityNames.ParentEntity)

	// 자식 전체 목록 가져오기
	const children = getEntitiesWithParent(engine, parent)
	for (const child of children) {
   		// 각 자식 엔티티 처리
	}
}
```

## 스마트 아이템 트리거

스마트 아이템의 **트리거 이벤트**를 감지하고, 이에 사용자 지정 코드로 응답할 수 있습니다. 예를 들어 버튼 스마트 아이템을 배치하고, 버튼이 클릭될 때 사용자 지정 코드를 실행할 수 있습니다.

다음을 사용하세요 `getTriggerEvents` 를 사용해 특정 스마트 아이템의 트리거 이벤트를 처리할 수 있는 객체를 가져온 다음, 반환된 객체의 `.on()` 함수를 사용해 콜백 함수를 구독합니다. 이 콜백 함수는 트리거 이벤트가 발생할 때마다 실행됩니다.

예를 들어, 씬에 다음과 같은 일반적인 버튼이 있다면 **클릭 시** 이벤트가 있을 때, 버튼이 활성화될 때마다 아래 코드를 작성해 사용자 지정 코드를 실행할 수 있습니다.

![](/docs/_remote/ed2a86256c21aaf6084b2d47.webp)

```ts
import { engine } from '@dcl/sdk/ecs'
import { getTriggerEvents, getActionEvents } from '@dcl/asset-packs/dist/events'
import { TriggerType } from '@dcl/asset-packs'
import { EntityNames } from '../assets/scene/entity-names'

function main() {
	const restart = engine.getEntityOrNullByName(EntityNames.Restart_Button)
	if (restart) {
		const restart_event = getTriggerEvents(restart)
		restart_event.on(TriggerType.ON_CLICK, () => {
			// restartGame()
		})
	}
}
```

다른 유형의 트리거 이벤트에도 마찬가지로 구독할 수 있습니다. 예를 들어 **ON\_PLAYER\_ENTERS\_AREA**, **ON\_SPAWN**, **ON\_TWEEN\_END**, 등.

## 스마트 아이템 액션

스마트 아이템의 **동작**활성화를 감지하고, 이에 사용자 지정 코드로 응답할 수 있습니다. 예를 들어 문 스마트 아이템을 배치하고, **Open** 작업이 호출될 때마다 사용자 지정 코드를 실행할 수 있습니다.

다음을 사용하세요 `getActionEvents` 를 사용해 특정 스마트 아이템의 작업을 처리할 객체를 가져옵니다. 그런 다음 반환된 객체의 `.on()` 함수를 사용해 콜백 함수를 구독할 수 있습니다. 이 콜백 함수는 작업이 다른 스마트 아이템에 의해 활성화되었는지, 아니면 여러분 자신의 사용자 지정 코드에 의해 활성화되었는지와 상관없이, 작업이 발생할 때마다 실행됩니다.

예를 들어, 씬에 다음 기본 **Open** 작업이 있는 문이 있다면, 문이 열릴 때마다 아래 코드를 작성해 사용자 지정 코드를 실행할 수 있습니다.

![](/docs/_remote/d8d14bd55320cbd4d130d66f.webp)

```ts
import { engine } from '@dcl/sdk/ecs'
import { getTriggerEvents, getActionEvents } from '@dcl/asset-packs/dist/events'
import { TriggerType } from '@dcl/asset-packs'
import { EntityNames } from '../assets/scene/entity-names'

function main() {
	const door = engine.getEntityOrNullByName(EntityNames.Wooden_Door)
	if (door) {
		// 작업 감지
		const actions = getActionEvents(door)
		actions.on('Open', () => {
			console.log('문이 열렸습니다!!')
			// 사용자 지정 코드
		})

		// 트리거 감지
		const triggers = getTriggerEvents(door)
		triggers.on(TriggerType.ON_CLICK, () => {
			console.log('문이 클릭되었습니다!!')
			// 사용자 지정 코드
		})
	}
}
```

코드에서 작업 이벤트를 방출할 수도 있습니다. 이렇게 하면 스마트 아이템의 Action 컴포넌트 내부에 이미 정의된 작업을 활용할 수 있습니다. 아래 코드 조각은 버튼 스마트 아이템이 트리거될 때마다 문 스마트 아이템의 "Open" 작업을 호출합니다.

```ts
import { engine } from '@dcl/sdk/ecs'
import { getTriggerEvents, getActionEvents } from '@dcl/asset-packs/dist/events'
import { TriggerType } from '@dcl/asset-packs'
import { EntityNames } from '../assets/scene/entity-names'

function main() {
	const button = engine.getEntityOrNullByName(EntityNames.Red_Button)
	const door = engine.getEntityOrNullByName(EntityNames.Wooden_Door)
	if (button && door) {
		// 작업과 트리거 참조
		const buttonTriggers = getTriggerEvents(button)
		const doorActions = getActionEvents(door)

		// 버튼의 트리거 감지
		buttonTriggers.on(TriggerType.ON_INPUT_ACTION, () => {
			// 문 열기
			doorActions.emit('Open', {})
		})
	}
}
```


> [!NOTE]
> **💡 팁**: 매우 복잡한 작업을 하려는 것이 아니라면, 코드를 작성하는 대신 원하는 동작을 처리하는 사용자 지정 스마트 아이템을 만들 수도 있습니다. 다음을 참조하세요 [어떤 항목이든 스마트 항목으로 만들기](/docs/creator/content-creator-ko/scene-editor/interactivity/make-any-item-smart/).


## 기타 스마트 아이템 컴포넌트

스마트 아이템에는 다음과 같은 asset-packs 라이브러리의 특수 컴포넌트가 포함될 수 있습니다 `States` 또는 `Counter`. 이러한 컴포넌트는 Decentraland SDK의 일부는 아니지만, 라이브러리의 `getComponents()` 함수를 통해 가져올 수 있습니다. 그런 다음 씬 코드에서 이러한 컴포넌트의 값을 읽거나 쓸 수 있어, 스마트 아이템 동작과 코드 간의 통합을 한층 더 긴밀하게 만들 수 있습니다.

아래 예제는 체스트 스마트 아이템의 State 컴포넌트 값을 읽어 로그에 출력하며, 체스트의 작업이 트리거될 때마다 실행됩니다.

```ts

import { engine } from '@dcl/sdk/ecs'
import { getComponents } from '@dcl/asset-packs'
import { getTriggerEvents } from '@dcl/asset-packs/dist/events'
import { TriggerType } from '@dcl/asset-packs'
import { EntityNames } from '../assets/scene/entity-names'


export function main() {

    const chest = engine.getEntityByName<EntityNames>(EntityNames.chest)
 
    if (chest) {

        const chestTriggers = getTriggerEvents(chest)

        chestTriggers.on(TriggerType.ON_INPUT_ACTION, () => {
            const { States } = getComponents(engine)
            let state = States.getMutableOrNull(chest)?.currentValue
            console.log("체스트의 새 상태", state)
        })
    }
}
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/scene-editor/extend-with-code/reference-items.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
