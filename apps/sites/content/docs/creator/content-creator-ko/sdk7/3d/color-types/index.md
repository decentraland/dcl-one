> 색상 값을 정의하는 방법

# 색상 유형

색상 값은 다음과 같은 다양한 컴포넌트의 속성으로 전달할 수 있습니다. [머티리얼](/docs/creator/content-creator-ko/sdk7/3d/materials/), [텍스트](/docs/creator/content-creator-ko/sdk7/3d/text/) 또는 다음의 다른 속성에 [UI 엔터티](/docs/creator/content-creator-ko/sdk7/2d-ui/onscreen-ui/). 색상 값은 다음 유형 중 하나일 수 있습니다 `Color3` 또는 `Color4`.

`Color3` 는 다음 세 가지 속성을 포함합니다. *빨간색*, *녹색*, 그리고 *파란색*. `Color4` 에는 동일한 세 가지와 함께 `알파`가 포함되며, 투명도 값을 나타냅니다.

## 색상 설정

다음을 사용하여 색상을 만들 수 있습니다 `Color3.create()` 또는 `Color4.create()` 함수입니다.

```ts
// 빨간색
const red3 = Color3.create(1, 0, 0)
const red4 = Color4.create(1, 0, 0, 1)
```

또한 다음의 일부인 미리 정의된 특정 색상을 만들 수 있습니다 `Color3` 및 `Color4` 네임스페이스입니다.

```ts
const red = Color3.Red()
const blue = Color3.Blue()
const black = Color3.Black()
```


> [!NOTE]
> **💡 팁**: 작성 `Color3.` 또는 `Color4.` 그리고 Visual Studio는 지능형 드롭다운에서 가능한 모든 값을 제안해 줄 것입니다.


그렇지 않다면 다음 함수를 사용하여 임의의 색상을 선택할 수 있습니다:

```ts
const randomColor = Color3.Random()
```

색상을 16진수로 표현하려면 다음을 사용하세요 `Color3.fromHexString()`.

```ts
const red = Color3.fromHexString('FD350A')
const blue = Color3.fromHexString('0A0CFD')
```

다음을 위한 숫자 값을 포함하는 모든 객체 값은 `r`, `g`, 그리고 `b` 로 해석할 수 있습니다 `Color3` 값으로. 마찬가지로, 해당 속성들과 함께 `a` 값을 포함하는 모든 객체는 `Color4` 값으로 해석할 수 있습니다. 이를 통해 다음 구문도 사용할 수 있습니다:

```ts
// 빨간색
const red3 = { r: 1, g: 0, b: 0 }
const red4 = { r: 1, g: 0, b: 0, a: 1 }
```

다음 예제는 색상 속성을 다음의 일부로 사용합니다 `TextShape` 컴포넌트에서 텍스트 색상을 설정합니다.

```ts
const myEntity = engine.addEntity()

Transform.create(myEntity, {
  position: Vector3.create(4, 1, 4),
})

TextShape.create(myEntity, {
  text: '이 텍스트는 RED입니다',
  textColor: Color4.create(1, 0, 0, 1),
})
```


> [!WARNING]
> **📔 참고**: `Color3` 및 `Color4` 는 다음을 통해 가져와야 합니다.
> 
> > `import { Color3, Color4 } from "@dcl/sdk/math"`
> 
> 참조 [가져오기](/docs/creator/content-creator-ko/sdk7/getting-started/coding-scenes/#imports) 에서 이를 쉽게 다루는 방법을 확인하세요.


## 투명도

다음을 사용하세요 *알파* 속성 `Color4` 를 사용해 색상을 반투명하게 만듭니다.

alpha가 1이면 색상은 완전히 불투명하고, 0이면 완전히 보이지 않습니다. 그 사이의 값은 부분적으로 투명한 톤이 됩니다.

```ts
// 완전히 불투명
const opaque = Color4.create(1, 1, 1, 1)

// 중간
const half = Color4.create(1, 1, 1, 0.5)

// 거의 보이지 않음
const almostInvisible = Color4.create(1, 1, 1, 0.1)
```

## Lerp

다음을 사용하세요 `Color3.lerp()` 또는 `Color4.lerp()` 함수는 두 다른 색상 사이 어딘가에 있는 색상을 정의하는 데 사용됩니다. 이 함수들은 다음과 유사하게 작동합니다 `Vector3.lerp()` 함수입니다.

둘 다 `Color3.lerp()` 또는 `Color4.lerp()` 다음 인수를 받습니다:

* `left`: 기준으로 사용할 첫 번째 색상
* `right`: 기준으로 사용할 두 번째 색상
* `amount`: 혼합에 사용할 양을 정의하는 0에서 1 사이의 숫자입니다. 0에 가까울수록 더 *right* 색상에 가깝습니다. *left* 색상입니다.

```ts
const orange = Color3.lerp(Color3.Red(), Color3.Yellow(), 0.3)
```

시스템을 사용하여 `amount` 매개변수를 점진적으로 변경해 부드러운 전환을 만들 수 있습니다.

```ts
// 시스템
var pulseState: number = 0
const color1 = Color4.Red()
const color2 = Color4.Yellow()

export function PulseSystem(dt: number) {
  pulseState += dt
  // 0과 1 사이를 진동
  const amount = (Math.sin(pulseState) + 1) / 2
  const entitiesWithMaterial = engine.getEntitiesWith(Material)

  // 그룹의 엔터티를 반복합니다
  for (const [entity] of entitiesWithMaterial) {
    const material = Material.getMutable(entity)
    if (material.material?.$case === 'pbr') {
      material.material.pbr.albedoColor = Color4.lerp(color1, color2, amount)
    }
  }
}

engine.addSystem(PulseSystem)
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/3d/color-types.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
