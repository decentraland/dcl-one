> 컴포넌트의 변경 사항을 감지하고 변경될 때마다 함수를 실행합니다

# 변경 사항 구독

코드를 작성하는 깔끔한 방법은 이벤트를 구독하고, 해당 이벤트가 발생할 때마다 함수를 실행하는 것입니다.

여러 [이벤트 리스너](/docs/creator/content-creator-ko/sdk7/interactivity/event-listeners/) 는 SDK의 일부로 미리 정의되어 있지만, 다음도 사용할 수 있습니다: `onChange()` 메서드를 어떤 컴포넌트에서든 사용해 같은 효과를 얻을 수 있습니다. 또한 다음과 같은 어떤 [사용자 정의 컴포넌트](/docs/creator/content-creator-ko/sdk7/architecture/custom-components/) 직접 정의한 것도 추가 작업 없이 사용할 수 있습니다.

예를 들어, 다음 함수는 `AvatarEquippedData` 플레이어 엔티티의 컴포넌트를 확인하고, 플레이어가 착용 중인 웨어러블이나 이모트를 변경하면 함수를 실행합니다. 컴포넌트의 새 값은 함수 인수로 전달됩니다.

```ts
import { AvatarEquippedData } from '@dcl/sdk/ecs'

export function main() {
	AvatarEquippedData.onChange(engine.PlayerEntity, (equipped) => {
		if (!equipped) return
		console.log('New wearables list: ', equipped.wearableUrns)
		console.log('New emotes list : ', equipped.emoteUrns)
	})
}
```

다음 덕분에 `onChange()` 메서드 덕분에 시스템을 만들고 매 프레임마다 새 값을 반복적으로 확인할 필요가 없으며, 이 매우 흔한 사용 사례를 크게 단순화할 수 있습니다.


> [!WARNING]
> **📔 참고**: 사용하지 마세요 `onChange()` System 내부에서 사용하면 게임 루프의 매 프레임마다 함수의 새 복사본이 구독되어 잠재적으로 충돌을 일으킬 수 있습니다.


같은 메서드는 다음과도 즉시 작동합니다: [사용자 정의 컴포넌트](/docs/creator/content-creator-ko/sdk7/architecture/custom-components/). 예를 들어:

```ts
// 컴포넌트 정의
export const MyComponent = engine.defineComponent('myComponent', {
	value1: Schemas.Boolean,
	value2: Schemas.Float,
})

// 사용
export function main() {
	// 엔터티 생성
	const myEntity = engine.addEntity()

	// 컴포넌트 인스턴스 생성
	MyComponent.create(myEntity, {
		value1: true,
		value2: 10,
	})

	// 변경 사항 구독
	MyComponent.onChange(myEntity, (componentData) => {
		if (!componentData) return
		console.log(componentData.value1)
		console.log(componentData.value2)
	})
}
```

이 접근 방식은 다음과도 함께 사용할 수 있습니다: [컴포넌트 쿼리하기](/docs/creator/content-creator-ko/sdk7/architecture/querying-components/)를 사용하면, 씬에서 특정 컴포넌트를 가진 모든 엔티티를 각자의 함수에 일괄 구독할 수 있습니다.

```ts
export function main() {
	for (const [entity] of engine.getEntitiesWith(MyComponent)) {
		MyComponent.onChange(entity, (componentData) => {
			if (!componentData) return
			console.log(componentData.value1)
			console.log(componentData.value2)
		})
	}
}
```

이 접근 방식은 다음에만 구독된다는 점에 유의하세요: `onChange()` 씬 시작 시 존재하는 엔티티에 대해서만 적용됩니다. 예를 들어, 다음의 UI를 통해 생성된 엔티티: [크리에이터 허브](/docs/creator/content-creator-ko/scene-editor/get-started/about-editor/).


> [!NOTE]
> **💡 팁**: 컴포넌트 변경과 반드시 관련되지 않은 이벤트를 처리하고 싶다면, TypeScript 라이브러리 [Mitt](https://www.npmjs.com/package/mitt) 를 씬에 가져오는 것을 권장합니다. 이 라이브러리는 이벤트를 발생시키고 수신하는 간단한 함수를 제공합니다.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/architecture/subscribe-to-changes.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
