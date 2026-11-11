> 엔티티와 관련된 특정 데이터를 처리할 사용자 지정 컴포넌트를 만듭니다

# 사용자 지정 컴포넌트

엔티티에 대한 데이터는 해당 엔티티의 [구성 요소](/docs/creator/content-creator-ko/sdk7/architecture/entities-components/). Decentraland SDK는 엔티티의 위치, 모양, 재질 등 서로 다른 측면을 관리하는 여러 기본 컴포넌트를 제공합니다. 엔진은 이들 안의 정보를 해석하는 방법을 알고 있으며, 값이 변경되는 즉시 그에 따라 엔티티의 렌더링 방식을 변경합니다.

씬의 로직상 SDK의 기본 컴포넌트가 처리하지 않는 엔티티의 정보를 저장해야 한다면, 씬에 커스텀 타입의 컴포넌트를 만들 수 있습니다. 그러면 [시스템](/docs/creator/content-creator-ko/sdk7/architecture/systems/) 이러한 컴포넌트의 변경 사항을 확인하고 그에 맞게 반응하는

## 컴포넌트 정의하기

새 컴포넌트를 정의하려면 `engine.defineComponent`. 각 컴포넌트에는 다음이 필요합니다:

* 하나의 **componentName**: SDK가 내부적으로 이 컴포넌트 타입을 식별하기 위해 사용하는 고유한 문자열 식별자입니다. 고유하기만 하다면 어떤 문자열이든 사용할 수 있습니다.
* 하나는 **schema**: 컴포넌트가 보유할 데이터 구조를 정의하는 클래스입니다.
* **기본값** *(선택 사항)*: 제공되지 않았을 때 컴포넌트의 복사본을 초기화하는 데 사용할 기본값을 포함하는 객체입니다.

```ts
export const WheelSpinComponent = engine.defineComponent('wheelSpinComponent', {
	spinning: Schemas.Boolean,
	speed: Schemas.Float,
})
```


> [!WARNING]
> **📔 참고**: 커스텀 컴포넌트는 항상 `main()` 함수 바깥, 별도 파일에 작성해야 합니다. 이들은 `main()` 가 실행되기 전에 작성되어야 합니다. 이를 위한 권장 위치는 `/components` 폴더 내부의 `/src`이며, 각 컴포넌트는 별도 파일에 두는 것입니다. 이렇게 하면 나중에 프로젝트에서 이들을 재사용하기가 더 쉬워집니다.


커스텀 컴포넌트를 정의한 뒤에는, 씬의 엔티티를 참조하는 이 컴포넌트의 인스턴스를 만들 수 있습니다. 컴포넌트의 인스턴스를 만들 때는 컴포넌트의 스키마에 있는 각 필드에 값을 제공합니다. 값은 각 필드에 선언된 타입을 따라야 합니다.

```ts
// 엔터티 생성
const wheel1 = engine.addEntity()
const wheel2 = engine.addEntity()

// 컴포넌트 인스턴스 생성
WheelSpinComponent.create(wheel1, {
	spinning: true,
	speed: 10,
})

WheelSpinComponent.create(wheel2, {
	spinning: false,
	speed: 0,
})
```

컴포넌트가 추가된 각 엔티티는 해당 엔티티에 대한 구체적인 데이터를 보유하는 컴포넌트의 새 복사본을 인스턴스화합니다.

커스텀 컴포넌트는 다른 컴포넌트에서 사용할 수 있는 다른 일반적인 기능들도 수행할 수 있습니다:

```ts
// 엔티티에서 컴포넌트의 읽기 전용 인스턴스 가져오기
const readOnlyInstance = MyCustomComponent.get(myEntity)

// 엔티티에서 컴포넌트의 변경 가능한 인스턴스 가져오기
const mutableInstance = MyCustomComponent.getMutable(myEntity)

// 엔티티의 컴포넌트 인스턴스 삭제
MyCustomComponent.deleteFrom(myEntity)
```

## componentName에 대하여

각 컴포넌트에는 내부적으로 구분하기 위한 고유한 컴포넌트 이름 또는 식별자가 있어야 합니다. 코드의 다른 곳에서 이 내부 식별자를 사용할 필요는 없습니다. 좋은 방법은 컴포넌트에 부여한 이름을 사용하되 첫 글자는 소문자로 시작하는 것입니다. 하지만 실제로 중요한 것은 이 식별자가 프로젝트 내에서 고유해야 한다는 점입니다.

라이브러리의 일부로 공유될 컴포넌트를 만들 때는, 라이브러리의 컴포넌트 이름이 사용되는 프로젝트의 다른 컴포넌트 이름이나 해당 프로젝트에서 함께 사용되는 다른 라이브러리의 컴포넌트 이름과 겹치지 않도록 주의해야 합니다. 겹칠 위험을 피하려면, 권장되는 모범 사례는 라이브러리 이름을 `componentName` 문자열의 일부로 포함하는 것입니다. 다음 공식을 따를 수 있습니다: `${packageName}::${componentName}`. 예를 들어`MyUtilities` 라는 라이브러리를 만들고 그 안에 `MoveEntity` 컴포넌트가 포함되어 있다면, 해당 컴포넌트의 `componentName` 를 다음으로 설정합니다 `MyUtilities::moveEntity`.

## 플래그로서의 컴포넌트

데이터를 저장하는 데 사용하지 않고, 엔티티를 구분하기 위해 단순히 엔티티에 플래그를 붙이는 컴포넌트를 추가하고 싶을 수 있습니다. 이렇게 하려면 스키마를 빈 객체로 두면 됩니다.

이는 특히 [컴포넌트 쿼리](/docs/creator/content-creator-ko/sdk7/architecture/querying-components/)를 사용할 때 유용합니다. 간단한 플래그 컴포넌트는 엔티티를 다른 엔티티와 구별하는 데 사용할 수 있으며, 시스템이 필요 이상으로 많은 엔티티를 반복 처리하지 않도록 해줍니다.

```ts
export const IsEnemyFlag = engine.defineComponent('isEnemyFlag', {})
```

그런 다음 이 컴포넌트를 가진 모든 엔티티를 순회하는 시스템을 만들 수 있습니다.

```ts
export function handleEnemies() {
	for (const [entity] of engine.getEntitiesWith(IsEnemyFlag)) {
		// 각 엔티티에 대해 무언가 수행
	}
}

engine.addSystem(handleEnemies)
```

## 컴포넌트 스키마

스키마는 컴포넌트 내부 데이터의 구조를 설명합니다. 컴포넌트는 원하는 만큼 많은 필드를 저장할 수 있으며, 각 필드는 스키마 구조에 포함되어야 합니다. 스키마는 필요한 만큼 여러 단계의 중첩 항목을 포함할 수 있습니다.

스키마의 모든 필드에는 타입 선언이 포함되어야 합니다. SDK에서 제공하는 특수 스키마 타입만 사용할 수 있습니다. 예를 들어, 다음과 같은 타입을 사용합니다 `Schemas.Boolean` 대신 `불리언`. 다음을 작성하세요 `Schemas.` 그러면 IDE에 사용 가능한 모든 옵션이 표시됩니다.

```ts
export const WheelSpinComponent = engine.defineComponent('WheelSpinComponent', {
	spinning: Schemas.Boolean,
	speed: Schemas.Float,
})
```

위 예제는 스키마가 두 개의 값을 보유하는 컴포넌트를 정의하는데, `spinning` 불리언과 `speed` 부동소수점 숫자입니다.

컴포넌트를 정의하는 동안 스키마를 인라인으로 만들 수도 있고, 더 가독성 있게 하려면 스키마를 먼저 만든 다음 이를 참조할 수도 있습니다.

```ts
// 옵션 1: 인라인 정의
export const WheelSpinComponent = engine.defineComponent('WheelSpinComponent', {
	spinning: Schemas.Boolean,
	speed: Schemas.Float,
})

// 옵션 2: 스키마와 컴포넌트를 별도로 정의

//// 스키마
const mySchema = {
	spinning: Schemas.Boolean,
	speed: Schemas.Float,
}

//// 컴포넌트
export const WheelSpinComponent = engine.defineComponent(
	'WheelSpinComponent',
	mySchema
)
```


> [!NOTE]
> **💡 팁**: 컴포넌트 인스턴스를 만들 때, VS Studio 자동 완성 옵션은 *Ctrl + Space*.


### 기본 스키마 타입

스키마의 필드 안에서 사용할 수 있는 기본 타입은 다음과 같습니다:

* `Schemas.Boolean`
* `Schemas.Byte`
* `Schemas.Double`
* `Schemas.Float`
* `Schemas.Int`
* `Schemas.Int64`
* `Schemas.Number`
* `Schemas.Short`
* `Schemas.String`
* `Schemas.Entity`

다음과 같은 복합 타입도 존재합니다. 각각은 숫자 값을 가진 여러 단계의 중첩 속성을 포함합니다.

* `Schemas.Vector3`
* `Schemas.Quaternion`
* `Schemas.Color3`
* `Schemas.Color4`


> [!NOTE]
> **💡 팁**: 참조 [기하 도형 유형](/docs/creator/content-creator-ko/sdk7/3d/special-types/) 및 [색상 타입](/docs/creator/content-creator-ko/sdk7/3d/color-types/) 이러한 데이터 타입이 어떻게 유용한지에 대한 자세한 내용은


예를 들어, 엔티티의 점진적인 이동을 추적하기 위해 이런 컴포넌트에서 이러한 스키마 타입을 사용할 수 있습니다. 이 컴포넌트는 초기 위치와 최종 위치를 Vector3 값으로 저장하고, 속도와 완료된 경로의 비율을 float 숫자로 저장합니다. 자세한 구현은 [엔티티 이동](/docs/creator/content-creator-ko/sdk7/3d/move-entities/#move-between-two-points) 이 예제의 전체 구현을 참고하세요.

```ts
const MoveTransportData = {
	start: Schemas.Vector3,
	end: Schemas.Vector3,
	fraction: Schemas.Float,
	speed: Schemas.Float,
}

export const LerpTransformComponent = engine.defineComponent(
	'LerpTransformComponent',
	MoveTransportData
)
```

### 배열 타입

필드의 타입을 배열로 설정하려면 `Schemas.Array()`를 사용합니다. 배열 안의 요소 타입을 속성으로 전달하세요.

```ts
const MySchema = {
	numberList: Schemas.Array(Schemas.Int),
}
```

배열 필드를 `MyComponent.get()`로 다시 읽어오면 배열은 읽기 전용입니다. `.push()`처럼 제자리에서 변경하는 메서드는 사용할 수 없습니다. `MyComponent.getMutable()` 를 사용해야 합니다.

### 선택적 필드

다음을 사용하세요 `Schemas.Optional()` 를 사용해 필드가 값 또는 `undefined`.

```ts
const MySchema = {
	playerId: Schemas.Optional(Schemas.String),
	score: Schemas.Optional(Schemas.Int),
}
```

오직 `undefined` 는 "설정되지 않음"으로 간주됩니다. 다음과 같은 falsy 값은 `false`, `0`, 그리고 `''` 그대로 저장되고 정확히 동일하게 읽혀집니다.

### 중첩 스키마 타입

필드의 타입을 객체로 설정하려면 `Schemas.Map()`를 사용합니다. 이 객체의 내용을 속성으로 전달하세요. 이 중첩 객체는 본질적으로 스키마 자체이며, 부모 스키마 안에 중첩되어 있습니다.

```ts
const MySchema = {
	simpleField: Schemas.Boolean,
	myComplexField: Schemas.Map({
		nestedField1: Schemas.Boolean,
		nestedField2: Schemas.Boolean,
	}),
}
```

또는 더 읽기 쉽고 재사용 가능하게 하려면, 중첩 스키마를 별도로 정의한 다음 부모 스키마를 정의할 때 이를 참조하는 방식으로 같은 결과를 얻을 수 있습니다.

```ts
const MyNestedSchema = Schemas.Map({
	nestedField1: Schemas.Boolean,
	nestedField2: Schemas.Boolean,
})

const MySchema = {
	simpleField: Schemas.Boolean,
	myComplexField: MyNestedSchema,
}
```

### 열거형 타입

스키마의 필드 타입을 열거형으로 설정할 수 있습니다. 열거형은 유한한 수의 옵션 중에서 쉽게 선택할 수 있게 해주며, 각 값에 사람이 읽을 수 있는 값을 제공합니다.

필드의 타입을 열거형으로 설정하려면 먼저 열거형을 정의해야 합니다. 그런 다음 `Schemas.EnumNumber` 또는 `Schemas.EnumString`를 사용해 참조할 수 있습니다. 이는 열거형의 타입에 따라 다릅니다. 이 함수들은 두 개의 매개변수를 받습니다. 하나는 참조할 열거형이고, 다른 하나는 이 필드에 사용할 기본값입니다.

```ts
//// 문자열 enum

// enum 정의
enum Color {
	Red = 'red',
	Green = 'green',
	Pink = 'pink',
}

// 이 enum을 필드에서 사용하는 컴포넌트 정의
const ColorComponent = engine.defineComponent('Color', {
	color: Schemas.EnumString<Color>(Color, Color.Red),
})

// 엔티티에 컴포넌트 사용
ColorComponent.create(engine.addEntity(), { color: Color.Green })

//// 숫자 enum

// enum 정의
enum CurveType {
	LINEAR,
	EASEIN,
	EASEOUT,
}

// 이 enum을 필드에서 사용하는 컴포넌트 정의
const CurveComponent = engine.defineComponent('curveComponent', {
	curve: Schemas.EnumNumber<CurveType>(CurveType, CurveType.LINEAR),
})

// 엔티티에 컴포넌트 사용
CurveComponent.create(engine.addEntity(), { curve: CurveType.EASEIN })
```

### 상호 교환 가능한 타입

스키마의 필드 타입을 다음과 같은 방식으로 설정할 수 있습니다 `oneOf` 패턴을 따르며, 서로 다른 타입을 허용할 수 있습니다.

```ts
const MySchema = {
	myField: Schemas.OneOf({ type1: Schemas.Vector3, type2: Schemas.Quaternion }),
}

export const MyComponent = engine.defineComponent('MyComponent', MySchema)
```

컴포넌트 인스턴스를 만들 때는 선택한 타입을 `$case`로 지정해야 합니다. 예를 들면:

```ts
MyComponent.create(myEntity, {
	myField: {
		$case: 'type1',
		value: Vector3.create(1, 1, 1),
	},
})
```

필드를 비워 두는 것도 유효합니다. 설정되지 않은 `OneOf` 필드에는 아무것도 없으며 `$case` 빈 객체로 읽혀집니다, `{}`.

### 단일 타입의 컴포넌트

컴포넌트가 여러 필드를 가진 객체를 보유할 필요는 없습니다. 단일 값을 보유하는 것을 정의하려면 `engine.defineComponentFromSchema()` 를 사용하고 타입을 직접 전달합니다:

```ts
// 엔티티당 숫자 하나를 보유하는 컴포넌트
export const Score = engine.defineComponentFromSchema('my-scene::Score', Schemas.Int)

// 엔티티당 숫자 목록을 보유하는 컴포넌트
export const History = engine.defineComponentFromSchema(
	'my-scene::History',
	Schemas.Array(Schemas.Int)
)
```

이들은 저장된 값이 falsy일 때도 다른 컴포넌트와 동일하게 동작합니다. A `Score` 의 `0` 는 존재하며 다음 값을 보유하는 컴포넌트입니다 `0`, 누락된 컴포넌트가 아닙니다.

## 기본값

새 복사본을 만들 때마다 각 값을 명시적으로 설정할 필요가 없도록 컴포넌트에 기본값을 두는 것이 좋습니다.

해당 `engine.defineComponent()` 함수는 세 번째 인수를 받으며, 이를 통해 기본값으로 사용할 객체를 전달할 수 있습니다. 이 객체에는 스키마의 값 전체 또는 일부가 포함될 수 있습니다. 기본값이나 컴포넌트 복사본을 초기화할 때 제공한 값으로 덮이지 않은 필드는 `0`, `false`타입에 따라 0에 가까운 값, 예를 들면

```ts
// 정의

//// 스키마
const mySchema = {
	spinning: Schemas.Boolean,
	speed: Schemas.Float,
}

//// 기본값
const myDefaultValues = {
	spinning: true,
	speed: 1,
}

//// 컴포넌트
export const WheelSpinComponent = engine.defineComponent(
	'WheelSpinComponent',
	mySchema,
	myDefaultValues
)

// 사용
export function main() {
	//// 엔티티 생성
	const wheel = engine.addEntity()
	const wheel2 = engine.addEntity()

	//// 기본값을 사용해 컴포넌트 초기화
	WheelSpinComponent.create(wheel)

	//// 하나의 사용자 지정 값으로 컴포넌트 초기화, 나머지는 기본값 사용
	WheelSpinComponent.create(wheel2, { speed: 5 })
}
```

위 예제는 `WheelSpinComponent` 컴포넌트와 함께 사용할 스키마 및 기본값 집합을 모두 생성합니다. 그런 다음 이 컴포넌트의 복사본을 어떤 값도 지정하지 않고 초기화하면 기본값으로 설정된 값이 사용됩니다.

## 변경 사항 구독

흔한 사용 사례는 특정 컴포넌트의 데이터가 변경된 경우에만 함수를 실행하는 것입니다. 다음을 사용하세요 [OnChange](/docs/creator/content-creator-ko/sdk7/architecture/subscribe-to-changes/) 함수를 사용하면 시스템을 정의하거나 이전 값과 새 값을 명시적으로 비교할 필요가 없습니다.

```ts
export function main() {
	// 엔티티 생성 등

	WheelSpinComponent.onChange(myEntity, (componentData) => {
		if (!componentData) return
		console.log(componentData.speed)
		console.log(componentData.spinning)
	})
}
```

## 컴포넌트를 사용하기 위한 시스템 구축

컴포넌트를 정의하고 씬의 엔티티에 추가했다면, [시스템](/docs/creator/content-creator-ko/sdk7/architecture/systems/) 을 만들어 이 컴포넌트에 저장된 데이터를 활용해 로직을 수행할 수 있습니다.

```ts
// 컴포넌트 정의
export const WheelSpinComponent = engine.defineComponent('WheelSpinComponent', {
	spinning: Schemas.Boolean,
	speed: Schemas.Float,
})

// 사용
export function main() {
	// 엔터티 생성
	const wheel1 = engine.addEntity()
	const wheel2 = engine.addEntity()

	// 컴포넌트 인스턴스 생성
	WheelSpinComponent.create(wheel1, {
		spinning: true,
		speed: 10,
	})

	WheelSpinComponent.create(wheel2, {
		spinning: false,
		speed: 0,
	})
}

// 이 엔티티들을 순회하는 시스템 정의
export function spinSystem(dt: number) {
	// WheelSpinComponent가 있는 모든 엔티티를 순회
	for (const [entity, wheelSpin] of engine.getEntitiesWith(
		WheelSpinComponent
	)) {
		// spinning == true일 때만 처리
		if (wheelSpin.spinning) {
			// 변경 가능한 Transform 컴포넌트를 가져옴
			const transform = Transform.getMutable(entity)

			// 회전 값을 그에 맞게 업데이트
			transform.rotation = Quaternion.multiply(
				transform.rotation,
				Quaternion.fromAngleAxis(dt * wheelSpin.speed, Vector3.Up())
			)
		}
	}
}

// 엔진에 시스템 추가
engine.addSystem(spinSystem)
```

위 예제는 커스텀 `wheelSpinComponent`를 포함하는 모든 엔티티를 순회하고, 게임 루프의 각 틱마다 그들을 조금씩 회전시키는 시스템을 정의합니다. 이 회전량은 각 엔티티의 컴포넌트 인스턴스에 저장된 `speed` 값에 비례합니다. 이 예제는 [컴포넌트 쿼리](/docs/creator/content-creator-ko/sdk7/architecture/querying-components/) 를 사용해 관련된 엔티티만 가져옵니다.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/architecture/custom-components.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
