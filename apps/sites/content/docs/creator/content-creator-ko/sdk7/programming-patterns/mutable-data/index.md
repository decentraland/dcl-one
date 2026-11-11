> 컴포넌트에서 읽기 전용 및 변경 가능한 데이터를 처리하는 방법을 알아봅니다

# 변경 가능한 데이터

어떤 ...에서 데이터를 참조할 때 [컴포넌트](/docs/creator/content-creator-ko/sdk7/architecture/entities-components/), 가변 버전 또는 읽기 전용(불변) 버전을 가져올 수 있습니다.

가능하면 항상 데이터의 읽기 전용 버전을 다루어야 합니다. 이 방법은 동일한 데이터를 항상 가변 버전으로 다루는 것과 비교했을 때, 씬의 성능을 매우 크게 향상시킬 수 있습니다.

해당 `.get()` 컴포넌트의 함수는 읽기 전용(불변) 버전의 컴포넌트를 반환합니다. 값은 읽을 수만 있고, 그 어떤 속성도 변경할 수 없습니다.

해당 `.getMutable()` 함수는 값을 변경할 수 있는 컴포넌트 버전을 반환합니다. 컴포넌트를 변경할 계획이 있을 때만 가변 버전을 사용하고, 그렇지 않다면 항상 `get()`.

```ts
// 읽기 전용(불변) 버전 가져오기
const immutableTransform = Transform.get(myEntity)

// 다음은 작동하지 않습니다:
// immutableTransform.position.y = 2

const mutableTransform = Transform.getMutable(myEntity)

// 다음 줄은 엔티티의 위치를 실제로 변경합니다
mutableTransform.position.y = 2
```

좋은 방법은 읽기 전용 컴포넌트를 순회하며 값을 확인한 다음, 변경이 필요할 때만 개별 컴포넌트의 가변 버전을 가져오는 것입니다.

```ts
// 하드코딩된 최대 높이
const MAX_HEIGHT = 10

// 시스템 정의
function HeightLimitSystem(dt: number) {
	// Transform 컴포넌트를 가진 모든 엔티티를 순회
	for (const [entity] of engine.getEntitiesWith(Transform)) {
		// 읽기 전용 값 가져오기
		const currentHeight = Transform.get(entity).position.y

		// 값 비교
		if (currentHeight > MAX_HEIGHT) {
			// 변경을 위해 가변 버전 가져오기
			const mutableTransform = Transform.getMutable(entity)

			// 변환값 변경
			mutableTransform.position.y = MAX_HEIGHT
		}
	}
}

// 시스템을 엔진에 추가
engine.addSystem(HeightLimitSystem)
```

위의 예시에서 시스템은 엔티티의 `Transform` 컴포넌트의 읽기 전용 값을 확인합니다. 매 틱마다 위치의 *y* 가 하드코딩된 최대 높이보다 높은지 확인합니다. Transform의 높이가 이 한도를 넘는 경우에만 가변 버전의 Transform을 가져옵니다. 이 과정은 씬에 추가 작업처럼 보일 수 있지만, 게임 루프의 모든 틱마다 값을 확인하고 변경은 가끔만 하는 씬에서는 엄청난 성능 향상을 가져옵니다.

이 방식은 [데이터 지향 프로그래밍](/docs/creator/content-creator-ko/sdk7/architecture/data-oriented-programming/)의 원칙을 따릅니다. 또한 이것이 가져오는 개선 효과가 매우 크기 때문에 게임 업계에서 점차 업계 표준 관행으로 채택되고 있습니다.


> [!WARNING]
> **📔 참고**: SDK의 이전 버전(6.x 또는 그 이전)에서는 컴포넌트가 항상 가변으로 취급되었습니다. 그 방식은 배우기에는 조금 더 직관적일 수 있지만, 실행 효율은 훨씬 떨어졌습니다.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/programming-patterns/mutable-data.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
