> 씬에서 3D 모델에 애니메이션을 적용하는 방법

# 3D 모델 애니메이션

의 3D 모델 *.glTF* 및 *.glb* .glTF 형식에는 원하는 만큼 많은 애니메이션을 포함할 수 있습니다. 애니메이션은 일련의 *키프레임* 을 시간에 따라 배치하며, 그러면 메시는 한 포즈에서 다른 포즈로 블렌딩되어 연속적인 움직임을 시뮬레이션합니다.

대부분의 3D 모델 애니메이션은 [*골격 애니메이션*](https://en.wikipedia.org/wiki/Skeletal_animation). 이러한 애니메이션은 모델의 복잡한 기하 구조를 "막대 인형"으로 단순화하여, 메시에 있는 모든 정점을 가장 가까운 *뼈* 을 *스켈레톤*. 모델러는 스켈레톤을 다양한 포즈로 조정하고, 메시는 이러한 움직임을 따라 늘어나고 구부러집니다.

대안으로, *정점 애니메이션* 은 스켈레톤 없이 모델을 애니메이션합니다. 이러한 애니메이션은 모델의 각 정점 위치를 직접 지정합니다. Decentraland도 이러한 애니메이션을 지원합니다.

참조 [애니메이션](https://github.com/decentraland/docs/blob/main/creator/3d-modeling/animations.md) 3D 모델용 애니메이션을 만드는 방법에 대한 자세한 내용은 다음을 읽어 보세요. [Shape 구성 요소](/docs/creator/content-creator-ko/sdk7/3d/shape-components/) 를 읽고 3D 모델을 씬으로 가져오는 방법을 확인하세요.


> [!NOTE]
> **💡 팁**: 애니메이션은 일반적으로 엔티티의 위치를 바꾸는 것보다는 제자리에서 무언가를 움직이는 데 더 적합합니다. 예를 들어 캐릭터의 발을 제자리에서 움직이도록 애니메이션을 설정할 수 있지만, 엔티티의 위치를 바꾸려면 Transform 구성 요소를 사용하는 것이 가장 좋습니다. 다음을 참조하세요. [엔티티 배치](/docs/creator/content-creator-ko/sdk7/3d/move-entities/) 자세한 내용은.


## 3D 모델에 애니메이션이 있는지 확인하기

모든 *glTF* 파일에 애니메이션이 포함되어 있는 것은 아닙니다. 사용할 수 있는 애니메이션이 있는지 확인하려면 다음을 수행할 수 있습니다:

* 다음을 사용하는 경우 [VS Code](https://code.visualstudio.com/)(권장), 다음을 설치하세요. *GLTF Tools* 확장을 설치한 다음 그곳에서 glTF 파일의 내용을 확인하세요.
* 다음을 엽니다 [Babylon Sandbox](https://sandbox.babylonjs.com/) 사이트로 이동한 뒤 glTF 파일(및 모든 *.jpg* 또는 *.bin* 종속 파일)을 브라우저로 끌어다 놓으세요.
* 다음을 엽니다 *.glTF* 파일을 텍스트 편집기로 열고 아래로 스크롤하여 다음을 찾으세요. *"animations":*.


> [!NOTE]
> **💡 팁**: *골격*  애니메이션에서 애니메이션 이름은 종종 아마추어 이름, 밑줄, 애니메이션 이름으로 구성됩니다. 예를 들어 `myArmature_animation1`.


## 자동 재생

3D 모델에 애니메이션이 포함되어 있으면 기본 동작은 그중 첫 번째 애니메이션이 항상 반복 재생되는 것입니다.

이 동작을 피하려면 다음을 추가하세요. `애니메이터` 구성 요소를 모델이 있는 엔티티에 추가한 다음 애니메이션 재생을 명시적으로 처리하세요. 만약 `애니메이터` 구성 요소가 엔티티에 있으면 모든 애니메이션의 기본값은 `playing: false` 상태가 되며, 수동으로 재생해야 합니다.


> [!NOTE]
> **💡 팁**: [씬 편집기](/docs/creator/content-creator-ko/scene-editor/get-started/about-editor/), 다음을 추가할 수 있습니다. **애니메이터** 구성 요소를 시각적으로 추가할 수 있습니다. 다음을 참조하세요. [컴포넌트 추가](/docs/creator/content-creator-ko/scene-editor/build/components/#add-components). 또한 코딩 없이 다음을 통해 애니메이션을 제어할 수도 있습니다. **동작**, 참조: [어떤 항목이든 스마트 항목으로 만들기](/docs/creator/content-creator-ko/scene-editor/interactivity/make-any-item-smart/).


## 애니메이션을 명시적으로 처리

하나의 `애니메이터` 구성 요소는 엔티티의 모든 애니메이션에 접근하는 데 사용되며, 엔티티에 애니메이션을 재생하거나 중지하라고 명시적으로 지시하는 데 사용할 수 있습니다.  `애니메이터` 구성 요소에는 배열이 포함되어 있으며 `상태`, 이 목록에는 3D 모델이 수행할 수 있는 각 애니메이션마다 하나의 객체가 포함되어야 합니다. 하나의 `애니메이터` 에는 필요한 만큼 많은 상태를 포함할 수 있습니다.

```ts
// 엔티티 생성
const shark = engine.addEntity()

// 여기에 3D 모델 추가
GltfContainer.create(shark, {
	src: 'models/shark.glb',
})

Animator.create(shark, {
	states: [
		{
			clip: 'swim',
			playing: true,
			loop: true,
		},
	],
})
```

각 `state` 객체는 애니메이션이 현재 재생 중인지 추적합니다.


> [!WARNING]
> **📔 참고**: `애니메이터` 컴포넌트는 다음을 통해 import해야 합니다
> 
> > `import { Animator } from "@dcl/sdk/ecs"`
> 
> 참조 [가져오기](/docs/creator/content-creator-ko/sdk7/getting-started/coding-scenes/#imports) 에서 이를 쉽게 다루는 방법을 확인하세요.


## 애니메이션 가져오기

다음에서 클립을 가져오세요. `애니메이터` 의 이름으로 다음을 사용하여 `.Animator.getClip()` 함수. 이 함수는 애니메이션 상태 객체의 변경 가능한 버전을 반환합니다.

```ts
const swimAnim = Animator.getClip(sharkEntity, 'swim')
```

`Animator.getClip` 에는 다음 매개변수가 필요합니다:

* `엔티티`: 다음의 엔티티 `애니메이터` 구성 요소를 조회하려는 경우입니다.
* `clipName`: 가져오려는 클립의 이름 문자열입니다.

`Animator.getClip` 는 애니메이션 상태의 변경 가능한 버전을 가져오므로, 이 함수가 반환한 값의 값을 자유롭게 수정할 수 있습니다.

```ts
const swimAnim = Animator.getClip(sharkEntity, 'swim')
swimAnim.loop = false
```


> [!WARNING]
> **📔 참고**: `Animator.getClip()` 를 사용해 목록에 없는 클립을 가져오려고 하면 `애니메이터` 구성 요소에 포함되어 있지 않으면 오류가 발생합니다. 다음을 사용하세요. `Animator.getClipOrNull()` 대신 다음을 받고 싶다면 `null` 오류가 아니라 응답을 받으세요.


## 애니메이션 재생

해당 `.playing` 필드는 애니메이션이 현재 재생 중인지 여부를 결정합니다. 하나의 3D 모델에서 여러 애니메이션이 동시에 재생될 수 있다는 점에 유의하세요.

다음을 사용하세요 `Animator.playSingleAnimation()`  함수는  `AnimationState`  객체에서 사용합니다.

```ts
Animator.playSingleAnimation(sharkEntity, 'swim')
```

엔티티가 다른 애니메이션을 재생 중이었다면, `Animator.playSingleAnimation` 이들을 중지합니다.

`Animator.playSingleAnimation` 에는 다음 매개변수가 필요합니다:

* `엔티티`: 다음의 엔티티 `애니메이터` 영향을 주려는 구성 요소입니다.
* `clipName`: 재생하려는 클립의 이름 문자열입니다.
* `resetCursor`: *(선택 사항)* 만약 *true*, 애니메이션이 이전에 일시 정지되었더라도 처음부터 재생합니다. 만약 *false*, 일시 정지된 지점부터 계속 재생합니다. 기본값: *true*.

```ts
Animator.playSingleAnimation(sharkEntity, 'swim', false)
```

다음 표는  `Animator.playSingleAnimation()` 이 어떻게 동작하는지, 다음의 다양한 값에 따라 요약합니다. `resetCursor`  속성:

|                | `resetCursor` = *false* | `resetCursor` = *true* (기본값) |
| -------------- | ----------------------- | ---------------------------- |
| **현재 재생 중**    | 영향이 없습니다.               | 처음부터 재생합니다.                  |
| **일시 정지됨**     | 마지막으로 재생한 프레임부터 재개합니다.  | 처음부터 재생합니다.                  |
| **완료됨(반복 없음)** | 처음부터 재생합니다.             | 처음부터 재생합니다.                  |

## 반복 애니메이션

기본적으로 애니메이션은 무한히 반복되는 루프 방식으로 재생됩니다.

다음 값을 설정하여 이 설정을 변경하세요. `loop` 의 `state`  객체에서 사용합니다.

```ts
Animator.create(shark, {
	states: [
		{
			clip: 'bite',
			playing: true,
			loop: false,
		},
	],
})
```

만약 `loop` 가 *false*, 애니메이션은 한 번만 재생된 뒤 멈추고 마지막 프레임의 자세에 머뭅니다.

## 애니메이션 중지

엔티티가 재생 중인 모든 애니메이션을 중지하려면 다음을 사용하세요. `Animator.stopAllAnimations()`.

```ts
Animator.stopAllAnimations(shark)
```

`Animator.stopAllAnimations` 에는 다음 매개변수가 필요합니다:

* `엔티티`: 다음의 엔티티 `애니메이터` 영향을 주려는 구성 요소입니다.
* `resetCursor`: *(선택 사항)* 만약 *true*, 애니메이션의 첫 번째 프레임 자세로 돌아갑니다. 만약 *false*, 현재 자세에서 일시 정지 상태를 유지합니다. 기본값: *true*.


> [!WARNING]
> **📔 참고**: 다음과 함께 애니메이션을 재생할 때 `Animator.playSingleAnimation`, 이 함수가 내부적으로 다른 모든 애니메이션의 중지를 처리합니다. 이 경우 다른 애니메이션을 명시적으로 중지할 필요가 없습니다.


반복되지 않는 애니메이션 재생이 끝나면 기본적으로 3D 모델은 마지막 자세를 유지합니다.  `shouldReset`  속성은 중지된 애니메이션이 다시 재생될 때 어떤 일이 일어나는지 제어합니다. 만약 *true*, 애니메이션은 초기 상태(첫 프레임, 또는 음수 `speed` 속도로 재생할 때는 마지막 프레임)로 복원됩니다. 만약 *false* (기본값)라면, 이전 위치에서 재개됩니다.

```ts
Animator.create(shark, {
	states: [
		{
			clip: 'bite',
			playing: true,
			shouldReset: true,
			loop: true,
		},
	],
})
```

다음을 사용할 수도 있습니다. `Animator.stopAllAnimations()` 언제든지 애니메이션의 첫 번째 프레임으로 자세를 명시적으로 되돌릴 수 있습니다.


> [!WARNING]
> **📔 참고**: 자세를 재설정하는 것은 급격한 변화입니다. 모델이 다른 자세로 부드럽게 전환되게 하려면 다른 애니메이션을 재생하고 둘 사이를 점진적으로 전환하면서 `가중치`  속성을 조정하세요. 다음을 참조하세요. [애니메이션 가중치](#animation-weight).


## 애니메이션이 끝났는지 감지하기

반복되지 않는 애니메이션 재생이 끝나면 엔진은 해당 애니메이션 상태의 `playing`  속성을 다시 *false*. 씬의 코드는 이 값을 읽어 애니메이션이 언제 끝났는지 알 수 있으며, 예를 들어 바로 뒤에 다른 애니메이션을 연결할 수 있습니다.

```ts
let wasPlaying = false

engine.addSystem(() => {
	const animator = Animator.get(shark)
	const biteState = animator.states.find((state) => state.clip === 'bite')
	const isPlaying = biteState?.playing ?? false

	if (wasPlaying && !isPlaying) {
		console.log('bite animation finished')
		// 다음 애니메이션 연결
		Animator.playSingleAnimation(shark, 'swim')
	}

	wasPlaying = isPlaying
})
```


> [!WARNING]
> **📔 참고**: 매 프레임 애니메이션 상태를 폴링할 때는 항상 다음을 통해 읽으세요. `Animator.get()` (읽기 전용). 다음을 사용하지 마세요. `Animator.getClip()` 또는 `Animator.getMutable()` 를 폴링에 사용하지 마세요. 이는 구성 요소의 변경 가능한 버전을 반환하여 매 프레임마다 변경됨으로 표시하고 불필요한 동기화 작업을 유발합니다.
> 
> 해당 `playing`  속성은 애니메이션이 스스로 끝날 때만 엔진에 의해 변경됩니다. 반복 애니메이션은 중지될 때까지 재생되므로 스스로는 이 속성을 변경하지 않으며, 다음이 설정된 애니메이션은 `speed` 0으로 설정되면 절대 끝나지 않습니다.



> [!WARNING]
> **📔 참고**: 이 기능은 데스크톱 클라이언트에서만 지원됩니다.


## 여러 애니메이션 처리

3D 모델에 여러 애니메이션이 포함되어 있다면 하나의 `애니메이터` 구성 요소로 모두 처리할 수 있습니다.

```ts
// 엔티티 생성
const shark = engine.addEntity()

// 여기에 3D 모델 추가
GltfContainer.create(shark, {
	src: 'models/shark.glb'
})

Animator.create(shark, {
	states:[{
			clip: "swim",
			playing: true,
			loop: true
		}, {
			clip: "bite",
			playing: true,
			loop: true
		}
	]
})
```

위 예에서는 두 애니메이션이 각각 별도의 `state` 객체로 처리된 다음 둘 다 동일한 `애니메이터` 컴포넌트도 부여해야 합니다.

애니메이션의 각 뼈는 한 번에 하나의 애니메이션만 영향을 줄 수 있습니다. 단, 이러한 애니메이션에 다음이 있는 경우는 예외입니다. `가중치` 합산 값이 1 이하가 되는

한 애니메이션이 캐릭터의 다리에만 영향을 주고 다른 애니메이션이 캐릭터의 머리에만 영향을 준다면 문제 없이 동시에 재생할 수 있습니다. 하지만 둘 다 캐릭터의 다리에 영향을 준다면 한 번에 하나만 재생하거나 더 낮은 `가중치` 값을 참조하세요.

위 예에서 `bite` 애니메이션이 상어의 입에만 영향을 주고 `swim` 애니메이션이 상어 척추의 뼈에만 영향을 준다면, 두 애니메이션은 동시에 재생할 수 있습니다.


> [!WARNING]
> **📔 참고**: `Animator.playSingleAnimation()` 는 엔티티가 현재 재생 중인 다른 모든 애니메이션을 중지합니다. 여러 애니메이션을 동시에 재생하려면 다음의 `playing` 속성을 애니메이션 상태에서 수동으로 수정하세요.


## 애니메이션 속도

다음을 변경하여 애니메이션 재생 속도를 변경하세요. `speed` 속성입니다. speed 값의 기본값은 1입니다.

```ts
Animator.create(shark, {
	states: [
		{
			clip: 'swim',
			playing: true,
			loop: true,
			speed: 2,
		},
	],
})
```

속도를 1보다 낮게 설정하면 더 느리게 재생됩니다. 예를 들어 0.5로 설정하면 절반 속도로 재생됩니다. 1보다 높게 설정하면 더 빠르게 재생됩니다. 예를 들어 2로 설정하면 두 배 속도로 재생됩니다.

```ts
const swimAnim = Animator.getClip(sharkEntity, 'swim')

swimAnim.speed = 0.5
```

## 애니메이션 가중치

해당 `가중치` 속성은 하나의 모델이 여러 애니메이션을 동시에 수행할 수 있게 하며, 애니메이션에 포함된 모든 움직임의 가중 평균을 계산합니다.  `가중치` 의 값은 해당 애니메이션이 평균에서 얼마나 중요한지 결정합니다.

기본적으로 `가중치` 는 다음과 같습니다. *1*.  `가중치` 의 값은 다음보다 높을 수 없습니다. *1*.

```ts
Animator.create(shark, {
	states: [
		{
			clip: 'swim',
			playing: true,
			loop: true,
			weight: 0.2,
		},
	],
})
```

해당 `가중치` 엔티티의 활성 애니메이션들의 값은 항상 합계가 1이 되어야 합니다. 1보다 작으면 가중 평균은 계산의 나머지 부분에서 아마추어의 기본 위치를 사용합니다.

예를 들어 위 코드 예제에서는 다음을 재생하고 있습니다. *swim* 애니메이션은 다음의 값만 가지며 `가중치` 의 *0.2*. 이 수영 동작은 상당히 미묘할 것입니다. 애니메이션이 정의한 강도의 20%만 적용됩니다. 계산의 나머지 80%는 아마추어의 기본 자세에서 값을 가져옵니다.

해당 `가중치` 속성은 흥미로운 방식으로 사용할 수 있습니다. 예를 들어 다음을 `가중치` 속성 *swim* 상어가 얼마나 빠르게 헤엄치는지에 비례해 설정할 수 있으므로, 빠른 수영과 느린 수영을 위한 여러 애니메이션을 만들 필요가 없습니다.

또한 다음을 변경하여 `가중치` 애니메이션을 시작하고 멈출 때 값을 점진적으로 바꾸어 더 자연스러운 전환을 만들고 기본 포즈에서 애니메이션의 첫 포즈로의 급격한 점프를 피할 수 있습니다.


> [!WARNING]
> **📔 참고**: 더해진 `가중치` 3D 모델의 뼈에 작용하는 모든 애니메이션의 값은 1을 초과할 수 없습니다. 여러 애니메이션이 동시에 같은 뼈에 영향을 준다면, 가중치의 합이 1보다 작도록 설정해야 합니다.


```ts
const swimAnim = Animator.getClip(sharkEntity, 'swim')

swimAnim.weight = 0.5
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/3d/3d-model-animations.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
