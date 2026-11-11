> 공통 컴포넌트를 가진 엔티티 목록을 가져오는 방법을 알아보고, 이를 더 쉽게 확인하거나 업데이트하는 방법을 배웁니다.

# 컴포넌트 조회

할 수 있습니다 [컴포넌트를 쿼리할 수 있습니다](/docs/creator/content-creator-ko/sdk7/architecture/querying-components/) 다음 메서드를 사용하여 `engine.getEntitiesWith(...components)` 특정 컴포넌트를 가진 씬의 모든 엔티티를 추적할 수 있습니다.

[시스템](/docs/creator/content-creator-ko/sdk7/architecture/systems/) 보통 이러한 쿼리에 있는 엔티티를 순회하며 각 엔티티에 동일한 작업을 수행합니다. 미리 정의된 유효 엔티티 그룹을 두는 것은 자원을 절약하는 좋은 방법이며, 특히 게임 루프의 매 틱마다 실행되는 함수에 유용합니다. 매 틱마다 시스템이 필요한 엔티티를 찾기 위해 씬의 모든 엔티티를 하나씩 순회해야 한다면 매우 비효율적일 것입니다.

다음과 같은 방법으로 쿼리의 엔티티에 접근할 수 있습니다.

```ts
for (const [entity] of engine.getEntitiesWith(Transform)) {
	//...
}
```

## 필수 컴포넌트

쿼리를 만들 때, 그룹에 추가되는 모든 엔티티에 어떤 컴포넌트가 있어야 하는지 지정합니다. 원하는 만큼 컴포넌트를 나열할 수 있으며, 쿼리는 다음을 가진 엔티티만 반환합니다. **모든** 나열된 컴포넌트 중 하나 이상.

```ts
for (const [entity] of engine.getEntitiesWith(
	Transform,
	Physics,
	NextPosition
)) {
	//...
}
```


> [!NOTE]
> **💡 팁** : 쿼리가 처리할 필요가 없는 엔티티를 반환한다면, 사용자 정의 컴포넌트를 만들어 [플래그](/docs/creator/content-creator-ko/sdk7/architecture/custom-components/#components-as-flags)로 사용하는 것을 고려하세요. 이 컴포넌트에는 어떤 속성도 필요하지 않지만, 나중에 다르게 처리하고 싶은 특정 엔티티 하위 그룹을 표시하는 데 사용할 수 있습니다.


## 시스템에서 쿼리 사용하기

```ts
// 시스템 정의
function PhysicsSystem(dt: number) {
	// Transform과 Physics 컴포넌트를 모두 포함하는 엔티티를 쿼리합니다
	for (const [entity] of engine.getEntitiesWith(Transform, Physics)) {
		const transform = Transform.getMutable(entity)
		const vel = Physics.get(entity).velocity
		transform.position.x += vel.x
		transform.position.y += vel.y
		transform.position.z += vel.z
	}
}

// 시스템을 엔진에 추가
engine.addSystem(PhysicsSystem)
```

위 예시에서 `PhysicsSystem` 함수는 게임 루프의 매 틱마다 실행되는 쿼리의 엔티티를 순회합니다.

* 씬에 여러 개의 *공* 엔티티가 있고 각각에 `변환` 와 하나의 `Physics` 컴포넌트가 있다면, 이들은 처리되고 각 틱마다 위치가 업데이트됩니다.
* 씬에 다른 엔티티도 있다면, 예를 들어 *림* 와 하나의 *scoreBoard* 와 같이 `변환` 만 있고 `Physics` 컴포넌트는 없는 엔티티가 있더라도, `PhysicsSystem`.

## 엔티티와 컴포넌트 다루기

다음 `getEntitiesWith` 함수는 엔티티 집합에 대한 참조를 포함하는 컬렉션을 반환하며, 선택적으로 나열된 컴포넌트에 대한 참조도 포함할 수 있습니다.

가장 단순한 문법을 사용하면, 해당 엔티티에 대한 참조 목록만 가져옵니다.

```ts
for (const [entity] of engine.getEntitiesWith(myComponent, myOtherComponent)) {
	//...
}
```

이 엔티티 목록을 순회하는 동안, 다음을 사용하여 해당 컴포넌트의 읽기 전용 또는 변경 가능한 버전을 가져올 수 있습니다. `.get` 또는 `getMutable`.

```ts
for (const [entity] of engine.getEntitiesWith(Transform)) {
	// 읽기 전용 버전 가져오기
	const transformReadOnly = Transform.get(entity)

	// 변경 가능한 버전 가져오기
	const transformMutable = Transform.getMutable(entity)
}
```

선택적으로, 쿼리가 반환한 컬렉션의 일부로 나열된 각 컴포넌트에 대한 참조도 직접 가져올 수 있습니다. 이를 위해서는 가져오고 싶은 각 컴포넌트에 대해 하나씩 여러 참조를 함께 선언하면 됩니다. 이러한 참조를 추가하는 것은 선택 사항이며, *모든* 쿼리의 컴포넌트에 대한 참조도 선언할 필요가 없습니다.

```ts
// 엔티티와 첫 번째로 나열된 컴포넌트에 대한 참조를 반환합니다
for (const [entity, component1] of engine.getEntitiesWith(
	MyCustomComponent1,
	MyCustomComponent2
)) {
	// 엔티티 목록을 순회
}

// 엔티티와 처음 두 개의 나열된 컴포넌트에 대한 참조를 반환합니다
for (const [entity, component1, component2] of engine.getEntitiesWith(
	MyCustomComponent1,
	MyCustomComponent2
)) {
	// 엔티티 목록을 순회
}
```


> [!WARNING]
> **📔 참고** : 이러한 참조는 읽기 전용입니다. 해당 컴포넌트의 변경 가능한 버전을 가져오려면 `.getMutable` 함수를 사용하여 엔티티를 참조해야 합니다.


그런 다음 결과 컬렉션을 순회하면서 이러한 참조를 사용할 수 있으며, 각 항목에서 엔티티와 해당 컴포넌트 참조에 접근할 수 있습니다.

```ts
for (const [entity, transformReadOnly] of engine.getEntitiesWith(Transform)) {
	console.log('entity id: ', entity)
	console.log('has position : ', transformReadOnly.position)
}
```

## 변경 사항 구독하기

흔한 사용 사례는 특정 컴포넌트의 데이터가 변경되었을 때만 함수를 실행하는 것입니다. 다음을 사용하세요. [OnChange](/docs/creator/content-creator-ko/sdk7/architecture/subscribe-to-changes/) 함수를 사용하면 시스템을 정의해야 하거나 이전 값과 새 값을 명시적으로 비교해야 하는 일을 피할 수 있습니다.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/architecture/querying-components.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
