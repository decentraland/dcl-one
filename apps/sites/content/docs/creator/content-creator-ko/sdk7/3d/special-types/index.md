> Vector, Quaternion 등을 포함해 어떤 특수 유형이 있는지 알아봅니다.

# 특수 유형

## Vector3

Decentraland는 *vector3* 데이터를 사용해 경로, 공간의 점, 방향을 나타냅니다. 벡터는 다음의 더 친숙한 대안으로 회전 방향을 정의하는 데도 사용할 수 있습니다. *쿼터니언*. Vector3 객체에는 각 *x*, *y*, 그리고 *부터* 축입니다.

```ts
const myVector: Vector3 = { x: 8, y: 1, z: 8 }
```

해당 `Vector3` 네임스페이스에는 대부분의 벡터 수학 연산을 직접 다루지 않도록 호출할 수 있는 유용한 메서드들이 여러 개 포함되어 있습니다. 작성하면 `Vector3.`를 입력하면 VS Studio가 사용 가능한 모든 함수가 포함된 드롭다운을 표시합니다.

아래는 벡터를 이용한 몇 가지 기본 연산의 구문을 보여주는 몇 줄입니다.

```ts
// 벡터 객체 생성
let myVector = Vector3.create(3, 1, 5)

// 벡터 객체를 생성하는 대체 구문
let myOtherVector: Vector3 = { x: 8, y: 1, z: 8 }

// 값 중 하나 수정
myVector.x = 5

// Vector3 네임스페이스의 함수 호출,
// 이 모든 함수는 매개변수로 Vector3 객체를 전달해야 합니다

let normalizedVector = Vector3.normalize(myVector)

let distance = Vector3.distance(myVector, myOtherVector)

let midPoint = Vector3.lerp(myVector, myOtherVector, 0.5)
```

Vector3 객체는 여러 컴포넌트의 필드에서 자주 필요합니다. 예를 들어, `Transform` 컴포넌트에는 `Vector3` 값이 포함되어 있으며, 이는 *위치* 및 *크기* 엔티티의

을 만들려면 [사용자 정의 컴포넌트](/docs/creator/content-creator-ko/sdk7/architecture/custom-components/) Vector3 값이 필요한 매개변수가 있는 `Schemas.Vector3`.


> [!WARNING]
> **📔 참고**: `Vector3` 는 다음을 통해 가져와야 합니다.
> 
> > `import { Vector3 } from "@dcl/sdk/math"`
> 
> 참조 [가져오기](/docs/creator/content-creator-ko/sdk7/getting-started/coding-scenes/#imports) 에서 이를 쉽게 다루는 방법을 확인하세요.


### 방향 벡터를 작성하기 위한 단축 구문

일반 벡터를 정의하는 데 사용할 수 있는 다음 단축 구문이 있습니다:

* `Vector3.Zero()` 반환값은 *(0, 0, 0)*
* `Vector3.Up()` 반환값은 *(0, 1, 0)*
* `Vector3.Down()` 반환값은 *(0, -1, 0)*
* `Vector3.Left()` 반환값은 *(-1, 0, 0)*
* `Vector3.Right()` 반환값은 *(1, 0, 0)*
* `Vector3.Forward()` 반환값은 *(0, 0, 1)*
* `Vector3.Backward()` 반환값은 *(0, 0, -1)*

## 쿼터니언

쿼터니언은 Transform 컴포넌트의 회전 정보를 저장하는 데 사용됩니다. 쿼터니언은 -1과 1 사이의 네 개 숫자로 구성됩니다: *x*, *y*, *부터*, *w*.

```ts
const myQuaternion: Quaternion = { x: 0, y: 0, z: 0, w: 1 }
```

쿼터니언은 [*오일러* 각](https://en.wikipedia.org/wiki/Euler_angles), 더 일반적인 *x*, *y* 및 *부터* 대부분의 사람들이 익숙한 0부터 360까지의 숫자를 사용하는 표기법과는 다릅니다. 엔진은 모든 회전을 쿼터니언으로 표현하므로, 가능한 경우 오일러로 변환하는 계산을 피하는 것이 좋습니다.

해당 `Quaternion` 네임스페이스에는 많은 수학 연산을 직접 다루지 않도록 호출할 수 있는 유용한 메서드들이 여러 개 포함되어 있습니다. 작성하면 `Quaternion.`를 입력하면 VS Studio가 사용 가능한 모든 함수가 포함된 드롭다운을 표시합니다.

아래는 쿼터니언을 이용한 몇 가지 기본 연산의 구문을 보여주는 몇 줄입니다.

```ts
// 쿼터니언 객체 생성
let myQuaternion = Quaternion.create(0, 0, 0, 1)

// 값 중 하나 수정
myQuaternion.x = 1

// quaternion 네임스페이스의 함수 호출
let midPoint = Quaternion.slerp(myQuaternion1, myQuaternion2, 0.5)

// 한 방향을 바라보는 상태에서 다른 방향으로 회전하는 데 필요한 회전 값을 가져오기,
// 두 방향 Vector3 값을 전달
let rotation = Quaternion.fromToRotation(Vector3.Forward(), Vector3.Up())
```

오일러 각도로 생각하는 것이 훨씬 더 쉽기 때문에, SDK에는 쿼터니언과 오일러 간의 변환을 위한 몇 가지 함수가 포함되어 있습니다.


> [!NOTE]
> **💡 팁**: 이러한 변환을 시스템 내부의 반복 로직의 일부로, 즉 매 틱마다 실행되는 작업으로 돌리지 마세요. 비용이 많이 들 수 있기 때문입니다. 이러한 변환은 새 엔티티의 회전을 설정하는 것과 같은 일회성 작업에 주로 유용합니다.


```ts
// 오일러에서 쿼터니언으로
let myQuaternion = Quaternion.fromEulerDegrees(90, 0, 0)

// 쿼터니언에서 오일러로
let myEuler = Quaternion.toEulerAngles(myQuaternion)
```

쿼터니언 객체는 컴포넌트의 필드에서 자주 필요합니다. 예를 들어, `Transform` 컴포넌트에는 `Quaternion` 엔티티의 회전 값입니다.

을 만들려면 [사용자 정의 컴포넌트](/docs/creator/content-creator-ko/sdk7/architecture/custom-components/) Quaternion 값이 필요한 매개변수가 있는 경우, 이러한 매개변수의 타입을 다음과 같이 설정하세요. `Schemas.Quaternion`.


> [!WARNING]
> **📔 참고**: `Quaternion` 는 다음을 통해 가져와야 합니다.
> 
> > `import { Quaternion } from "@dcl/sdk/math"`
> 
> 참조 [가져오기](/docs/creator/content-creator-ko/sdk7/getting-started/coding-scenes/#imports) 에서 이를 쉽게 다루는 방법을 확인하세요.


## 스칼라

스칼라는 단순히 숫자일 뿐입니다. 그런 이유로, `스칼라` 데이터를 저장하기 위한 객체를 인스턴스화하는 것은 큰 의미가 없습니다. 숫자로도 같은 작업을 할 수 있기 때문입니다. 하지만 `스칼라` 네임스페이스의 함수는 *Vector3* 네임스페이스의 함수와 유사한 여러 유용한 함수를 제공하며, 숫자에 사용할 수 있습니다.

```ts
// Scalar 클래스의 함수 호출
let random = Scalar.randomRange(1, 100)

let midPoint = Scalar.lerp(number1, number2, 0.5)

let clampedValue = Scalar.clamp(myInput, 0, 100)
```


> [!WARNING]
> **📔 참고**: `스칼라` 는 다음을 통해 가져와야 합니다.
> 
> > `import { Scalar } from "@dcl/sdk/math"`
> 
> 참조 [가져오기](/docs/creator/content-creator-ko/sdk7/getting-started/coding-scenes/#imports) 에서 이를 쉽게 다루는 방법을 확인하세요.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/3d/special-types.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
