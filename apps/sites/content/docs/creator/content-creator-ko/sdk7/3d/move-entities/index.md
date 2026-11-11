> 점진적인 변경을 통해 엔티티를 시간에 따라 서서히 이동, 회전, 크기 조정하는 방법.

# 엔티티 이동

장면의 엔티티를 일정 시간에 걸쳐 이동, 회전 또는 크기 조정하려면 `Tween` 컴포넌트를 사용합니다. 엔진은 원하는 변환을 부드럽게 수행하며, 지정된 시간이 끝날 때까지 매 프레임마다 업데이트를 표시합니다. 또한 `Transform` 영향을 받는 엔티티의 컴포넌트 값도 장면 코드에서 근접성 검사가 필요할 경우 실시간으로 업데이트됩니다.


> [!NOTE]
> **💡 팁**: [Creator Hub의 씬 에디터에](/docs/creator/content-creator-ko/scene-editor/get-started/about-editor/)에서는 노코드 방식으로 엔티티를 **액션**, 다음을 참조하세요. [모든 아이템을 스마트하게 만들기](/docs/creator/content-creator-ko/scene-editor/interactivity/make-any-item-smart/).


Tween 컴포넌트에는 다음 함수가 있습니다:

* `setMove`: 두 지점 사이로 이동
* `setRotate`: 두 방향 사이에서 회전
* `setScale`: 두 크기 사이로 크기 조정
* `setMoveRotateScale`: 세 매개변수 모두에 동시에 전환 적용
* `setMoveContinuous`: 같은 방향으로 계속 이동
* `setRotateContinuous`: 같은 방향으로 계속 회전
* `setTextureMove`: 두 위치 사이에서 재질의 텍스처 오프셋 조정
* `setTextureMoveContinuous`: 같은 방향으로 재질의 텍스처를 계속 오프셋 조정

## 두 지점 사이로 이동

엔티티를 두 지점 사이로 이동하려면 `Tween` 컴포넌트를 다음과 함께 생성합니다 `setMove` 함수를 사용하세요.

```ts
const myEntity = engine.addEntity()
Transform.create(myEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(myEntity)

Tween.setMove(myEntity, 
	Vector3.create(1, 1, 1), 
	Vector3.create(8, 1, 8), 
	2000
)
```

이동 트윈은 다음 정보를 받습니다:

* `entity`: 이동할 엔티티
* `시작`: 시작 위치를 나타내는 Vector3
* `종료`: 종료 위치를 나타내는 Vector3
* `지속 시간`: 두 위치 사이를 이동하는 데 걸리는 밀리초 수

다음 선택적 매개변수도 사용할 수 있습니다:

* `easingFunction`: 사용할 이징 함수를 지정합니다. 다음을 참조하세요: [비선형 트윈](#non-linear-tweens)

## 두 방향 사이에서 회전

엔티티를 두 지점 사이에서 회전시키려면 `Tween` 컴포넌트를 다음과 함께 생성합니다 `setRotate` 함수를 사용하세요.

```ts
const myEntity = engine.addEntity()
Transform.create(myEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(myEntity)

Tween.setRotate(myEntity, 
	Quaternion.fromEulerDegrees(0, 0, 0), 
	Quaternion.fromEulerDegrees(0, 170, 0), 
	700
)
```

회전 트윈은 다음 정보를 받습니다:

* `시작`: 시작 회전을 나타내는 Quaternion
* `종료`: 종료 회전을 나타내는 Quaternion
* `지속 시간`: 두 위치 사이를 이동하는 데 걸리는 밀리초 수

다음 선택적 매개변수도 사용할 수 있습니다:

* `easingFunction`: 사용할 이징 함수를 지정합니다. 다음을 참조하세요: [비선형 트윈](#non-linear-tweens)

### 피벗 포인트를 기준으로 회전

엔티티를 회전할 때 회전은 항상 엔티티의 중심 좌표를 기준으로 합니다. 다른 좌표 집합을 피벗 포인트로 사용해 엔티티를 회전하려면, 피벗 포인트를 위치로 갖는 두 번째(보이지 않는) 엔티티를 만들고, 회전시키려는 엔티티의 부모로 설정하세요.

부모 엔티티를 회전하면 그 자식들은 모두 부모의 위치를 피벗 포인트로 사용해 회전합니다. 다음을 참고하세요: `위치` 자식 엔티티의 위치는 부모 엔티티의 위치를 기준으로 합니다.

```ts
const pivotEntity = engine.addEntity()
Transform.create(pivotEntity, {
	position: Vector3.create(4, 1, 4),
})

const childEntity = engine.addEntity()
Transform.create(childEntity, {
	position: Vector3.create(1, 0, 0),
	parent: pivotEntity,
})
MeshRenderer.setBox(childEntity)

Tween.setRotate(pivotEntity, 
	Quaternion.fromEulerDegrees(0, 0, 0), 
	Quaternion.fromEulerDegrees(0, 170, 0), 
	700
)
```

이 예제에서는 시스템이 `pivotEntity` 엔티티를 회전하고 있으며, 이는 `childEntity` 엔티티의 부모입니다.

## 두 크기 사이로 크기 조정

엔티티의 크기를 두 크기 사이에서 변경하려면 `Tween` 컴포넌트를 다음과 함께 생성합니다 `setScale` 함수를 사용하세요.

```ts
const myEntity = engine.addEntity()
Transform.create(myEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(myEntity)

Tween.setScale(myEntity, 
	Vector3.create(1, 1, 1), 
	Vector3.create(4, 4, 4), 
	2000
)	

```

크기 조정 트윈은 다음 정보를 받습니다:

* `시작`: 시작 크기를 나타내는 Vector3
* `종료`: 종료 크기를 나타내는 Vector3
* `지속 시간`: 두 위치 사이를 이동하는 데 걸리는 밀리초 수

다음 선택적 매개변수도 사용할 수 있습니다:

* `easingFunction`: 사용할 이징 함수를 지정합니다. 다음을 참조하세요: [비선형 트윈](#non-linear-tweens)

## 현재 위치에서 이동

트윈이 활성화되어 있는 동안 엔진은 `Transform` 컴포넌트 값을 실시간으로 업데이트합니다. 즉, 엔티티의 현재 위치를 해당 `Transform` 컴포넌트에서 언제든지 읽어 새 트윈의 `시작` 로 사용할 수 있습니다. 그러면 엔티티는 어디에 있든 현재 위치에서 대상 지점으로 이동하며, 어떻게 그곳에 도달했는지는 상관하지 않습니다.

```ts
pointerEventsSystem.onPointerDown(
	{
		entity: myEntity,
		opts: { button: InputAction.IA_POINTER, hoverText: '이동' },
	},
	() => {
		Tween.setMove(myEntity, 
			Transform.get(myEntity).position, 
			Vector3.create(8, 1, 8), 
			2000,
			EasingFunction.EF_EASEOUTQUAD
		)
	}
)
```


> [!WARNING]
> **📔 참고**: 항상 명시적인 `시작` 값을 전달하세요. 이 `시작` 매개변수가 설정되지 않으면 *(0, 0, 0)*&#xB85C; 간주되므로, 엔티티는 이동하기 전에 장면 원점으로 순간이동합니다. 마찬가지로, 엔티티의 현재 위치와 일치하지 않는 하드코딩된 `시작` 값을 전달하면, 엔티티는 이동하기 전에 그 위치로 눈에 띄게 순간이동합니다. 이동을 엔티티의 현재 위치에서 시작하고 싶다면, 이 섹션의 예제처럼 엔티티의 `Transform` 컴포넌트에서 현재 위치를 읽어오세요.


### 이동 중에 대상 변경

호출하기 `setMove` (또는 다른 트윈 함수)를 이미 실행 중인 트윈이 있는 엔티티에 호출하면, 실행 중인 트윈이 새 트윈으로 대체됩니다.  `시작` 는 엔티티의 실시간 위치에서 읽히므로, 이동 중에도 엔티티의 경로를 자연스럽게 다시 설정합니다. 즉, 그 순간의 위치에서 방향을 바꾸며, 갑작스러운 점프는 없습니다.

아래 예제는 플레이어가 클릭한 패드로 엔티티를 이동시킵니다. 엔티티가 아직 이동 중일 때 플레이어가 다른 패드를 클릭하면, 엔티티는 새 대상 쪽으로 부드럽게 방향을 바꿉니다.

```ts
function moveTo(target: Vector3) {
	Tween.setMove(shipEntity, 
		Transform.get(shipEntity).position, 
		target, 
		2000,
		EasingFunction.EF_EASEOUTQUAD
	)
}

pointerEventsSystem.onPointerDown(
	{
		entity: padA,
		opts: { button: InputAction.IA_POINTER, hoverText: '여기로 이동' },
	},
	() => moveTo(Vector3.create(2, 1, 2))
)

pointerEventsSystem.onPointerDown(
	{
		entity: padB,
		opts: { button: InputAction.IA_POINTER, hoverText: '여기로 이동' },
	},
	() => moveTo(Vector3.create(14, 1, 14))
)
```

엔티티가 목적지에 도달했을 때 로직을 실행하려면 [트윈 완료 시](#on-tween-finished).


> [!WARNING]
> **📔 참고**: 클릭이나 웨이포인트 도달 같은 가끔 발생하는 이벤트에만 이렇게 대상을 다시 지정하세요. 대상이 계속 바뀌는 경우, 예를 들어 플레이어를 따라가는 엔티티라면 매 프레임마다 새 트윈을 만들지 마세요. 엔티티가 눈에 띄게 끊깁니다. 다음을 참조하세요: [이동하는 대상 따라가기](#follow-a-moving-target).


## 비선형 트윈

트윈은 서로 다른 **이징 함수** 를 따를 수 있으며, 이는 시간에 따른 변화율에 영향을 줍니다.  **linear** 함수는 변화 속도가 시작부터 끝까지 일정하다는 뜻입니다. 시작 및/또는 끝이 느리게 시작하는지, 그리고 얼마나 느린지에 따라 곡선 모양이 다르게 나타나는 다양한 옵션이 있습니다.  **easeinexpo** 곡선은 시작은 느리고 끝은 빠르며, 지수적으로 속도가 증가합니다. 반대로  **easeoutexpo** 곡선은 시작은 빠르고 끝은 느립니다.


> [!NOTE]
> **💡 팁**: 다양한 이동 곡선을 실험해 보세요. 차이는 종종 미묘하지만, 우리는 무게, 마찰, 심지어 성격과 같은 정보를 사물이 움직이는 방식에서 무의식적으로 해석합니다.


```ts
Tween.setScale(myEntity, 
	Vector3.create(1, 1, 1), 
	Vector3.create(4, 4, 4), 
	2000,
	EasingFunction.EF_EASEOUTBOUNCE
)

```

선택적 `easingFunction` 매개변수는 `EasingFunction` 열거형에서 값을 가져오며, 다음 옵션을 제공합니다:

* `EF_EASEBACK`
* `EF_EASEBOUNCE`
* `EF_EASECIRC`
* `EF_EASECUBIC`
* `EF_EASEELASTIC`
* `EF_EASEEXPO`
* `EF_EASEINBACK`
* `EF_EASEINBOUNCE`
* `EF_EASEINCIRC`
* `EF_EASEINCUBIC`
* `EF_EASEINELASTIC`
* `EF_EASEINEXPO`
* `EF_EASEINQUAD`
* `EF_EASEINQUART`
* `EF_EASEINQUINT`
* `EF_EASEINSINE`
* `EF_EASEOUTBACK`
* `EF_EASEOUTBOUNCE`
* `EF_EASEOUTCIRC`
* `EF_EASEOUTCUBIC`
* `EF_EASEOUTELASTIC`
* `EF_EASEOUTEXPO`
* `EF_EASEOUTQUAD`
* `EF_EASEOUTQUART`
* `EF_EASEOUTQUINT`
* `EF_EASEOUTSINE`
* `EF_EASEQUAD`
* `EF_EASEQUART`
* `EF_EASEQUINT`
* `EF_EASESINE`
* `EF_LINEAR`

## 일정한 회전

엔티티를 계속 회전시키려면 `Tween` 컴포넌트를 다음과 함께 생성합니다 `setRotateContinuous` 함수를 사용하세요.

```ts
Tween.setRotateContinuous(myEntity, 
	Quaternion.fromEulerDegrees(0, -1, 0), 
	45
)
```

연속 회전 트윈은 다음 정보를 받습니다:

* `entity`: 회전할 엔티티
* `방향`: 회전 축을 결정하는 Quaternion입니다. 이 회전의 축만 중요하며, 각도의 크기는 무시됩니다. 예를 들어 `Quaternion.fromEulerDegrees(0, -1, 0)` 및 `Quaternion.fromEulerDegrees(0, -90, 0)` 는 같은 방식으로 동작하며, 둘 다 *y* 축을 음의 방향으로 회전시킵니다.
* `속도`: 엔티티가 초당 몇 도 회전할지 지정합니다. 음수 값은 반대 방향으로 회전합니다.

다음 선택적 매개변수도 사용할 수 있습니다:

* `지속 시간`: 회전을 유지할 밀리초 수입니다. 이 시간이 지나면 회전이 멈춥니다.

## 일정한 이동

엔티티를 같은 방향으로 계속 이동시키려면 `Tween` 컴포넌트를 다음과 함께 생성합니다 `setMoveContinuous` 함수를 사용하세요.

```ts
Tween.setMoveContinuous(myEntity, 
	Vector3.create(0, 0, 1), 
	0.7
)
```

연속 이동 트윈은 다음 정보를 받습니다:

* `entity`: 이동할 엔티티
* `방향`: 이동 방향을 나타내는 Vector3
* `속도`: 엔티티가 초당 이동할 미터 수

다음 선택적 매개변수도 사용할 수 있습니다:

* `지속 시간`: 이동을 유지할 밀리초 수입니다. 이 시간이 지나면 이동이 멈춥니다.

연속 이동 트윈은 다음 정보를 받습니다:

### 이동하는 대상 따라가기

`setMoveContinuous` 목적지가 계속 바뀌는 경우, 예를 들어 플레이어를 따라가는 엔티티일 때에도 올바른 도구입니다. 대신 매 프레임마다 플레이어의 최신 위치를 향하는 새로운 `setMove` 트윈을 만드는 것이 유혹적일 수 있지만, 그렇게 하면 눈에 띄는 끊김이 발생합니다.

그 이유는 `Transform` 컴포넌트에서 읽는 위치가 엔진이 마지막으로 장면에 보고한 위치이기 때문에, 엔티티의 실제 위치보다 몇 프레임 뒤처져 있기 때문입니다. 그 값을 새 트윈의 `시작` 로 전달하면 엔진은 즉시 적용하고, 엔티티는 몇 프레임 전의 위치로 되돌아갑니다. 이러한 보정 중 하나는 눈에 띄지 않지만, 초당 여러 번 반복되면 그렇지 않습니다.

`setMoveContinuous` 시작점을 대신 방향과 속도를 제공하기 때문에 이 문제를 완전히 피할 수 있습니다. 엔진은 엔티티의 실제 위치를 계속 사용하므로, 방향을 바꾸기 위해 트윈을 교체해도 엔티티가 절대 점프하지 않습니다.

```ts
const CHASE_SPEED = 3
const STOP_DISTANCE = 1

engine.addSystem(() => {
	const playerPosition = Transform.get(engine.PlayerEntity).position
	const myPosition = Transform.get(myEntity).position

	if (Vector3.distance(myPosition, playerPosition) <= STOP_DISTANCE) {
		// 충분히 가까움: 연속 트윈에는 목적지가 없으므로 직접 중지합니다
		if (Tween.has(myEntity)) {
			Tween.deleteFrom(myEntity)
		}
		return
	}

	const direction = Vector3.subtract(playerPosition, myPosition)
	direction.y = 0 // 플레이어가 점프해도 지면에 머무르기

	Tween.setMoveContinuous(myEntity, Vector3.normalize(direction), CHASE_SPEED)
})
```


> [!WARNING]
> **📔 참고**: 연속 트윈은 스스로 끝나지 않으므로, 엔티티는 `Tween` 컴포넌트를 `Tween.deleteFrom()`으로 제거하거나, 선택적 `지속 시간` 이 소진될 때까지 계속 움직입니다. 또한 매 프레임마다 다시 조준하면 매 프레임 엔진에 업데이트가 전송된다는 점에 유의하세요. 이를 피하려면 방향이 충분히 바뀌었을 때만 `setMoveContinuous` 를 다시 호출하세요.


## 트윈 시퀀스

엔티티가 여러 트윈을 순서대로 재생하게 하려면 `TweenSequence` 컴포넌트를 사용합니다. 이 컴포넌트에는 두 필드가 필요합니다:

* `sequence`: 여러 트윈 정의를 담은 배열로, 순차적으로 실행됩니다. 배열은 비어 있을 수 있으며, 이 경우 현재 트윈만 재생됩니다.
* `loop` *(선택 사항)*: 제공되지 않으면 시퀀스는 한 번만 재생됩니다. 필드가 있으면 값은 다음의 값이어야 합니다: `TweenLoop` 열거형입니다. 허용되는 값은 다음과 같습니다:
  * `TL_RESTART`: 시퀀스가 끝나면 다시 시작합니다. 마지막 상태가 첫 상태와 일치하지 않으면 엔티티가 즉시 한 상태에서 다른 상태로 점프합니다.
  * `TL_YOYO`: 시퀀스가 끝나면 뒤로 진행하며, 시작 지점에 다시 도달할 때까지 모든 트윈을 역순으로 실행합니다. 그런 다음 다시 시작합니다.

### 앞뒤로 이동

플랫폼이 두 위치 사이를 계속 앞뒤로 이동하게 하려면 `sequence` 배열을 비워 두고 `loop` 로 설정하여 `TweenLoop.TL_YOYO`

```ts
const myEntity = engine.addEntity()
Transform.create(myEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(myEntity)

Tween.setMove(myEntity, 
	Vector3.create(1, 1, 1), 
	Vector3.create(8, 1, 8), 
	2000
)

TweenSequence.create(myEntity, { sequence: [], loop: TweenLoop.TL_YOYO })
```

엔티티는 시작점과 끝점 사이를 앞뒤로 이동하며, 양방향 모두 같은 지속 시간과 같은 이징 함수를 사용합니다.

### 경로 따라가기

여러 지점이 있는 더 복잡한 경로를 엔티티가 따라가게 하려면 `sequence` 의 트윈 정의 목록을 제공하세요 `TweenSequence` 컴포넌트.

```ts
const myEntity = engine.addEntity()
Transform.create(myEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(myEntity)

Tween.setMove(myEntity, 
	Vector3.create(6.5, 7, 4), 
	Vector3.create(6.5, 7, 12), 
	4000
)

TweenSequence.create(myEntity, {
	sequence: [
		{
			duration: 2000,
			easingFunction: EasingFunction.EF_LINEAR,
			mode: Tween.Mode.Move({
				start: Vector3.create(6.5, 7, 12),
				end: Vector3.create(6.5, 10.5, 12),
			}),
		},
		{
			duration: 3000,
			easingFunction: EasingFunction.EF_LINEAR,
			mode: Tween.Mode.Move({
				start: Vector3.create(6.5, 10.5, 12),
				end: Vector3.create(6.5, 10.5, 4),
			}),
		},
		{
			duration: 3000,
			easingFunction: EasingFunction.EF_LINEAR,
			mode: Tween.Mode.Move({
				start: Vector3.create(6.5, 10.5, 4),
				end: Vector3.create(6.5, 7, 4),
			}),
		},
	],
	loop: TweenLoop.TL_RESTART,
})
```

TweenSequence 안에서 트윈을 정의할 때는 더 자세한 형식의 `Tween.Mode.Move` 또는 `Tween.Mode.Rotate` 또는 `Tween.Mode.Scale` 을 사용해야 합니다. 이 더 자세한 형식에서는 다음을 지정해야 합니다:

* `지속 시간`: 두 위치 사이를 이동하는 데 걸리는 밀리초 수
* `easingFunction`: 사용할 이징 함수를 지정합니다. 다음을 참조하세요: [비선형 트윈](#non-linear-tweens). 이 형식에서는 값이 필수입니다.
* `mode`: 트윈의 모드로, 다음 중 하나일 수 있습니다: `Tween.Mode.Move`, `Tween.Mode.Rotate` 또는 `Tween.Mode.Scale`.

그리고 `mode` 필드 안에서 다음을 지정해야 합니다:

* `시작`: 트윈의 시작 값
* `종료`: 트윈의 종료 값

## 트윈 완료 시

다음을 사용하세요 `tweenSystem.tweenCompleted` 를 사용해 트윈이 끝났는지 감지합니다. 이는 트윈이 종료될 때 동작을 수행하는 데 유용할 수 있습니다. 예를 들어 엘리베이터 문을 여는 경우입니다.

```ts
engine.addSystem(() => {
	const tweenCompleted = tweenSystem.tweenCompleted(myEntity)
	if (tweenCompleted) {
		// 사운드 재생
	}
})
```

## 동시 트윈

엔티티를 초기 상태와 종료 상태 사이에서 이동, 회전, 크기 조정하려면 `Tween` 컴포넌트를 다음과 함께 생성합니다 `setMoveRotateScale` 함수를 사용합니다. 이 함수는 이 세 매개변수의 어떤 조합에도 사용할 수 있습니다.

```ts
const myEntity = engine.addEntity()
Transform.create(myEntity, {
	position: Vector3.create(14, 1, 2),
})
MeshRenderer.setBox(myEntity)

Tween.setMoveRotateScale(myEntity, {
	position: { start: Vector3.create(14, 1, 2), end: Vector3.create(14, 3, 2) },
	rotation: { start: Quaternion.fromEulerDegrees(0, 0, 0), end: Quaternion.fromEulerDegrees(0, 180, 90) },
	scale: { start: Vector3.One(), end: Vector3.create(2, 0.5, 2) },
	duration: 2000
})
```

이동 트윈은 다음 정보를 받습니다:

* `entity`: 이동할 엔티티
* `params`: 다음 매개변수를 가진 객체
  * `위치`: *(선택 사항)* 하나의 `시작` 및 `종료` 값은 다음 두 가지 형식으로 모두 제공됩니다: `Vector3`.
  * `회전`: *(선택 사항)* 하나의 `시작` 및 `종료` 값은 다음 두 가지 형식으로 모두 제공됩니다: `Quaternion`.
  * `크기 조정`: *(선택 사항)* 하나의 `시작` 및 `종료` 값은 다음 두 가지 형식으로 모두 제공됩니다: `Vector3`. 다음 중 적어도 하나는 `위치`, `회전` 또는 `크기 조정` 제공되어야 합니다.
  * `지속 시간`: *(필수)* 두 값 집합 사이를 전환하는 데 걸리는 밀리초 수
  * `easingFunction`: *(선택 사항)* 어떤 이징 함수를 사용할지 지정합니다. 다음을 참조하세요: [비선형 트윈](#non-linear-tweens)

엔티티는 하나의 `Tween` 컴포넌트만 가질 수 있으며, 각 트윈 컴포넌트는 한 번에 하나의 변환만 수행할 수 있습니다.  `setMoveRotateScale` 트윈 타입을 통해 엔티티를 옆으로 이동시키면서 동시에 회전시킬 수 있지만, 두 동작은 같은 타임라인을 따릅니다. 서로 독립적인 전환이 필요하다면, 대안으로 부모-자식 엔티티를 사용할 수 있습니다. 예를 들어, 보이지 않는 부모 엔티티가 옆으로 이동하고 보이는 자식이 회전하게 할 수 있습니다.

다음 스니펫에서는 부모 엔티티가 회전하고 자식은 크기가 커집니다.

```ts
const parentEntity = engine.addEntity()
Transform.create(parentEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(parentEntity)
Tween.setRotate(parentEntity, 
	Quaternion.fromEulerDegrees(0, 0, 0), 
	Quaternion.fromEulerDegrees(0, 170, 0), 
	5000
)

const childEntity = engine.addEntity()
Transform.create(childEntity, {
	position: Vector3.create(0, 0, 0),
	parent: parentEntity,
})
MeshRenderer.setBox(childEntity)
Tween.setScale(childEntity, 
	Vector3.create(1, 1, 1), 
	Vector3.create(4, 4, 4), 
	2000
)
```

## 트윈 일시 중지

트윈을 일시 중지하려면 `playing` 속성을 false로 변경합니다. 다시 시작하려면 true로 되돌리세요.

```ts
pointerEventsSystem.onPointerDown(
	{
		entity: button,
		opts: { button: InputAction.IA_POINTER, hoverText: '일시 중지' },
	},
	() => {
		let tweenData = Tween.getMutable(myEntity)
		tweenData.playing = !tweenData.playing
	}
)
```

계속할 필요가 없는 트윈을 끝내려면 `Tween` 엔티티에서 해당 컴포넌트를 삭제하세요. 엔티티가 또한 다음을 사용 중이었다면 `TweenSequence` 컴포넌트도 함께 삭제하세요.

```ts
pointerEventsSystem.onPointerDown(
	{
		entity: button,
		opts: { button: InputAction.IA_POINTER, hoverText: '일시 중지' },
	},
	() => {
		if( Tween.has(myEntity)){
			Tween.deleteFrom(myEntity)
		}
		if( TweenSequence.has(myEntity)){
			TweenSequence.deleteFrom(myEntity)
		}
	}
)
```

## 시스템 기반 트윈

Tween 컴포넌트를 사용하고 엔진이 변환을 처리하도록 하는 대신, 이 전환을 프레임별로 점진적으로, 즉 다음을 통해 수행하는 것을 선호할 수 있습니다. [시스템](/docs/creator/content-creator-ko/sdk7/architecture/systems/) 장면 안에서. 함수가 실행될 때마다 엔티티를 조금씩 이동시키는 방식입니다.

한편으로는, 매 프레임마다 움직임을 다시 계산할 수 있어 더 많은 제어권을 얻습니다. 다른 한편으로는 코드가 더 복잡해지고, 성능이 낮은 머신의 플레이어는 각 증분이 보이는 지연처럼 느껴지는 트윈을 경험할 수 있습니다.

### 시스템을 통해 이동

엔티티를 이동시키는 가장 쉬운 방법은 다음 값을 점진적으로 수정하는 것입니다. *위치* 에 저장된 값 `Transform` 컴포넌트.

```ts
function SimpleMove() {
	let transform = Transform.getMutable(myEntity)
	transform.position = Vector3.add(
		transform.position,
		Vector3.scale(Vector3.Forward(), 0.05)
	)
}

engine.addSystem(SimpleMove)

const myEntity = engine.addEntity()
Transform.create(myEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(myEntity)
```

이 예제에서는 게임 루프의 틱마다 엔티티를 0.05미터씩 이동시키고 있습니다.

`Vector3.Forward()` 는 앞으로 향하고 길이가 1미터인 벡터를 반환합니다. 이 예제에서는 이 벡터의 길이를 다음을 사용해 1/20로 줄입니다: `Vector3.scale()`. 장면이 초당 30프레임이라면, 엔티티는 초당 1.5미터의 속도로 움직이고 있습니다.

![](/docs/_remote/90ee9b2fdf4c6fb2a573ce79.webp)

### 시스템을 통해 회전

엔티티를 회전시키는 가장 쉬운 방법은 Transform 컴포넌트의 값을 조금씩 점진적으로 변경하고, 이를 시스템 함수의 일부로 실행하는 것입니다.

```ts
function SimpleRotate() {
	let transform = Transform.getMutable(myEntity)
	transform.rotation = Quaternion.multiply(
		transform.rotation,
		Quaternion.fromAngleAxis(1, Vector3.Up())
	)
}

engine.addSystem(SimpleRotate)

const myEntity = engine.addEntity()
Transform.create(myEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(myEntity)
```

현재 회전에 각 증분을 결합하기 위해 다음을 사용하고 있다는 점에 유의하세요: `Quaternion.multiply`. 쿼터니언 수학에서는 두 회전을 더하는 것이 아니라 곱해서 결합합니다. 한 쿼터니언을 다른 쿼터니언과 곱해 얻는 결과 회전은, 먼저 하나의 회전을 수행한 다음 다른 회전을 수행했을 때와 동일한 최종 회전입니다.

이 예제에서는 게임 루프의 각 틱마다 엔티티를 위쪽 방향으로 1도씩 회전시키고 있습니다.


> [!NOTE]
> **💡 팁**: 엔티티가 항상 플레이어를 향하도록 회전하게 만들려면 다음을 추가할 수 있습니다: [`빌보드` 컴포넌트](/docs/creator/content-creator-ko/sdk7/3d/entity-positioning/#face-the-player).


![](/docs/_remote/13d2f2aa20f3bb7131ff9274.webp)

### 피벗 포인트를 중심으로 시스템을 통해 회전

엔티티를 회전할 때 회전은 항상 엔티티의 중심 좌표를 기준으로 합니다. 다른 좌표 집합을 피벗 포인트로 사용해 엔티티를 회전하려면, 피벗 포인트를 위치로 갖는 두 번째(보이지 않는) 엔티티를 만들고, 회전시키려는 엔티티의 부모로 설정하세요.

부모 엔티티를 회전하면 그 자식들은 모두 부모의 위치를 피벗 포인트로 사용해 회전합니다. 다음을 참고하세요: `위치` 자식 엔티티의 위치는 부모 엔티티의 위치를 기준으로 합니다.

```ts
function SimpleRotate() {
	let transform = Transform.getMutable(pivotEntity)
	transform.rotation = Quaternion.multiply(
		transform.rotation,
		Quaternion.fromAngleAxis(1, Vector3.Up())
	)
}

engine.addSystem(SimpleRotate)

const pivotEntity = engine.addEntity()
Transform.create(pivotEntity, {
	position: Vector3.create(4, 1, 4),
})

const childEntity = engine.addEntity()
Transform.create(childEntity, {
	position: Vector3.create(1, 0, 0),
	parent: pivotEntity,
})
MeshRenderer.setBox(childEntity)
```

이 예제에서는 시스템이 `pivotEntity` 엔티티를 회전하고 있으며, 이는 `childEntity` 엔티티의 부모입니다.

![](/docs/_remote/3c145695aa57e42efe942e39.webp)

### 지연 시간에 맞춰 이동 조정

플레이어가 당신의 장면을 방문했을 때 프레임 속도를 따라가기 어려워한다고 가정해 봅시다. 그러면 모든 프레임의 시간이 동일하지는 않지만 각 프레임이 엔티티를 같은 양만큼 이동시키므로, 움직임이 끊겨 보일 수 있습니다.

다음을 사용하면 이러한 불균일한 타이밍을 보정할 수 있습니다: `dt` 매개변수로 이동의 스케일을 조정하세요.

```ts
function SimpleMove(dt: number) {
	let transform = Transform.getMutable(myEntity)
	transform.position = Vector3.add(
		transform.position,
		Vector3.scale(Vector3.Forward(), dt)
	)
}

engine.addSystem(SimpleMove)

const myEntity = engine.addEntity()
Transform.create(myEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(myEntity)
```

위 예제는 프레임 속도가 떨어져도 위의 이동 예제와 대체로 같은 속도를 유지합니다. 초당 30프레임으로 실행할 때, 다음 값은 `dt` 1/30입니다.

회전량에 다음을 곱해 같은 방식으로 회전을 부드럽게 만들 수도 있습니다: `dt`.

### 시스템을 통해 두 점 사이 이동

엔티티가 두 점 사이를 부드럽게 이동하도록 하려면 다음을 사용하세요: *lerp* (선형 보간) 알고리즘. 이 알고리즘은 정말 유용하기 때문에 게임 개발에서 매우 잘 알려져 있습니다.

다음 `lerp()` 함수는 세 개의 매개변수를 받습니다:

* 원점 위치의 벡터
* 목표 위치의 벡터
* 양: 변환을 얼마나 수행할지를 나타내는 0에서 1 사이의 값입니다.

```ts
const originVector = Vector3.Zero()
const targetVector = Vector3.Forward()

let newPos = Vector3.lerp(originVector, targetVector, 0.6)
```

선형 보간 알고리즘은 두 벡터 사이 경로에서 제공된 양에 맞는 중간 지점을 찾습니다.

예를 들어, 원점 벡터가 *(0, 0, 0)* 이고 목표 벡터가 *(10, 0, 10)*:

* 0의 값을 사용하면 다음이 반환됩니다: *(0, 0, 0)*
* 0.3의 값을 사용하면 다음이 반환됩니다: *(3, 0, 3)*
* 1의 값을 사용하면 다음이 반환됩니다: *(10, 0, 10)*

이를 구현하려면 `lerp()` 장면 안에서, 다음을 만드는 것을 권장합니다: [커스텀 컴포넌트](/docs/creator/content-creator-ko/sdk7/architecture/custom-components/) 필요한 정보를 저장하기 위해서입니다. 또한 각 프레임에서 점진적 이동을 구현하는 시스템도 정의해야 합니다.

```ts
// 커스텀 컴포넌트 정의
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

// 시스템 정의
function LerpMove(dt: number) {
	let transform = Transform.getMutable(myEntity)
	let lerp = LerpTransformComponent.getMutable(myEntity)
	if (lerp.fraction < 1) {
		lerp.fraction += dt * lerp.speed
		transform.position = Vector3.lerp(lerp.start, lerp.end, lerp.fraction)
	}
}

engine.addSystem(LerpMove)

// 엔티티 생성
const myEntity = engine.addEntity()

Transform.create(myEntity, {
	position: Vector3.create(4, 1, 4),
})

MeshRenderer.setBox(myEntity)

LerpTransformComponent.create(myEntity, {
	start: Vector3.create(4, 1, 4),
	end: Vector3.create(8, 1, 8),
	fraction: 0,
	speed: 1,
})
```

![](/docs/_remote/751a298f237c2c997d52fdec.webp)

### 시스템을 통해 움직이는 대상을 부드럽게 따라가기

플레이어를 따라다니는 펫처럼 계속 움직이는 목표를 향해 엔티티를 부드럽게 다가가게 하려면, 매 프레임 남은 거리의 일부씩 이동시키세요. 다음처럼 프레임마다 고정된 비율을 사용하지 마세요: `Vector3.lerp(current, target, 0.1)`: 그러면 결과가 프레임 속도에 따라 달라져서, 빠른 컴퓨터에서는 느린 컴퓨터보다 엔티티가 더 빨리 따라잡게 됩니다. 다음에서 그 비율을 계산하세요: `dt` 대신:

```ts
const follower = engine.addEntity()
Transform.create(follower, { position: Vector3.create(8, 1, 8) })
MeshRenderer.setBox(follower)

const RATE = 5 // 값이 높을수록 더 빨리 따라잡습니다

function FollowSystem(dt: number) {
	const transform = Transform.getMutable(follower)
	const target = Transform.get(engine.PlayerEntity).position
	// 어떤 프레임 속도에서도 동일한 부드러움
	const t = 1 - Math.exp(-RATE * dt)
	transform.position = Vector3.lerp(transform.position, target, t)
}

engine.addSystem(FollowSystem)
```

같은 공식은 회전에도 다음과 함께 사용할 수 있습니다: `Quaternion.slerp()`.

### 시스템을 통해 두 각도 사이 회전

두 각도 사이를 부드럽게 회전하려면 다음을 사용하세요: *slerp* (*구면* (선형 보간) 알고리즘. 이 알고리즘은 다음과 매우 유사합니다: *lerp*, 하지만 쿼터니언 회전을 처리합니다.

다음 `slerp()` 함수는 세 개의 매개변수를 받습니다:

* 다음 [쿼터니언](https://en.wikipedia.org/wiki/Quaternion) 원점 회전의 각도
* 다음 [쿼터니언](https://en.wikipedia.org/wiki/Quaternion) 목표 회전의 각도
* 양: 변환을 얼마나 수행할지를 나타내는 0에서 1 사이의 값입니다.


> [!NOTE]
> **💡 팁**: 회전 값은 다음과 같이 전달할 수 있습니다: [오일러](https://en.wikipedia.org/wiki/Euler_angles) 각도(0에서 360까지)를 다음을 사용해 `Quaternion.fromEulerDegrees()`.


```ts
const originRotation = Quaternion.fromEulerDegrees(0, 90, 0)
const targetRotation = Quaternion.fromEulerDegrees(0, 0, 0)

let newRotation = Quaternion.slerp(originRotation, targetRotation, 0.6)
```

장면에서 이를 구현하려면, 다음에 들어가는 데이터를 저장하는 것을 권장합니다: `Slerp()` 함수의 데이터를 다음에 저장하세요: [커스텀 컴포넌트](/docs/creator/content-creator-ko/sdk7/architecture/custom-components/). 또한 각 프레임에서 점진적 회전을 구현하는 시스템도 정의해야 합니다.

```ts
// 커스텀 컴포넌트 정의
const RotateSlerpData = {
	start: Schemas.Quaternion,
	end: Schemas.Quaternion,
	fraction: Schemas.Float,
	speed: Schemas.Float,
}

export const SlerpData = engine.defineComponent('SlerpData', RotateSlerpData)

// 시스템 정의
function SlerpRotate(dt: number) {
	let transform = Transform.getMutable(myEntity)
	let slerpData = SlerpData.getMutable(myEntity)
	if (slerpData.fraction < 1) {
		slerpData.fraction += dt * slerpData.speed
		transform.rotation = Quaternion.slerp(
			slerpData.start,
			slerpData.end,
			slerpData.fraction
		)
	}
}

engine.addSystem(SlerpRotate)

// 엔티티 생성
const myEntity = engine.addEntity()

Transform.create(myEntity, {
	position: Vector3.create(4, 1, 4),
})

MeshRenderer.setBox(myEntity)

SlerpData.create(myEntity, {
	start: Quaternion.fromEulerDegrees(0, 0, 0),
	end: Quaternion.fromEulerDegrees(0, 180, 0),
	fraction: 0,
	speed: 0.3,
})
```


> [!WARNING]
> **📔 참고**: 대신 회전을 오일러 각 값으로 표현하고 `Vector3` 값으로 표현한 뒤 다음을 사용할 수도 있습니다: `Lerp()` 함수를 사용할 수 있지만, 그러면 다음에서 변환이 필요하다는 뜻이 됩니다: `Vector3` 로 설정하여 `Quaternion` 매 프레임마다. 회전 값은 내부적으로 `Transform` 컴포넌트에서 쿼터니언으로 저장되므로, 장면이 쿼터니언으로 작업하는 편이 더 효율적입니다.


![](/docs/_remote/e4559d5d1a49351c4ded356f.webp)

더 단순하지만 덜 효율적인 접근 방식은 다음을 활용합니다: `Quaternion.rotateTowards` 함수이며, 어떤 커스텀 컴포넌트도 사용하지 않습니다.

```ts
function SimpleRotate(dt: number) {
	let transform = Transform.getMutable(myEntity)
	transform.rotation = Quaternion.rotateTowards(
		transform.rotation,
		Quaternion.fromEulerDegrees(90, 0, 0),
		dt * 10
	)
	if (
		Quaternion.angle(transform.rotation, Quaternion.fromEulerDegrees(90, 0, 0)) < 0.01
	) {
		console.log('done')
		engine.removeSystem(SimpleRotate)
	}
}

engine.addSystem(SimpleRotate)

const myEntity = engine.addEntity()
Transform.create(myEntity, {
	position: Vector3.create(4, 1, 4),
	rotation: Quaternion.fromEulerDegrees(0, 0, 90),
})

MeshRenderer.setBox(myEntity)
```

위 예제에서 `Quaternion.rotateTowards` 세 개의 인수를 받습니다: 초기 회전, 원하는 최종 회전, 그리고 프레임당 최대 증분입니다. 이 경우 최대 증분이 `dt * 10` 도이므로, 회전은 약 9초에 걸쳐 진행됩니다.

시스템은 회전이 완료되었는지도 확인하고, 완료되면 엔진에서 해당 시스템을 제거한다는 점에 유의하세요. 그렇지 않으면 회전이 끝난 뒤에도 시스템이 매 프레임 계속 계산을 수행하게 됩니다.

### 시스템을 통해 두 크기 사이의 스케일 변경

엔티티의 크기를 부드럽게, 그리고 비율을 바꾸지 않고 변경하려면 다음을 사용하세요: *lerp* (선형 보간) 알고리즘을 사용하는 `Scalar` 객체.

그렇지 않고 축별로 서로 다른 비율로 변경하려면 다음을 사용하세요: `Vector3` 원점 스케일과 목표 스케일을 나타내고, 그다음 다음을 사용합니다: *lerp* 의 함수 `Vector3`.

다음 `lerp()` 의 함수 `Scalar` 객체는 세 개의 매개변수를 받습니다:

* 원점 스케일의 숫자
* 목표 스케일의 숫자
* 양: 스케일 조정을 얼마나 수행할지를 나타내는 0에서 1 사이의 값입니다.

```ts
const originScale = 1
const targetScale = 10

let newScale = Scalar.lerp(originScale, targetScale, 0.6)
```

장면에서 이 lerp를 구현하려면 필요한 정보를 저장할 커스텀 컴포넌트를 만드는 것을 권장합니다. 또한 각 프레임에서 점진적 스케일 조정을 구현하는 시스템도 정의해야 합니다.

```ts
// 커스텀 컴포넌트 정의
const ScaleTransportData = {
	start: Schemas.Number,
	end: Schemas.Number,
	fraction: Schemas.Float,
	speed: Schemas.Float,
}

export const ScaleTransformComponent = engine.defineComponent(
	'ScaleTransformComponent',
	ScaleTransportData
)

// 시스템 정의
function LerpMove(dt: number) {
	let transform = Transform.getMutable(myEntity)
	let lerp = ScaleTransformComponent.getMutable(myEntity)
	if (lerp.fraction < 1) {
		lerp.fraction += dt * lerp.speed
		const newScale = Scalar.lerp(lerp.start, lerp.end, lerp.fraction)
		transform.scale = Vector3.create(newScale, newScale, newScale)
	}
}

engine.addSystem(LerpMove)

// 엔티티 생성
const myEntity = engine.addEntity()

Transform.create(myEntity, {
	position: { x: 4, y: 1, z: 4 },
})

MeshRenderer.setBox(myEntity)

ScaleTransformComponent.create(myEntity, {
	start: 1,
	end: 2,
	fraction: 0,
	speed: 1,
})

Vector3.create(1, 1, 1)
```

![](/docs/_remote/3b128506c9be0c97b7264b06.webp)

### 시스템을 통해 두 점 사이를 불규칙한 속도로 이동

lerp 메서드를 사용하는 동안 이동 속도를 비선형으로 만들 수 있습니다. 이전 예제에서는 매 프레임 lerp 양을 일정한 값만큼 증가시켰지만, 수학 함수를 사용해 수치를 지수적으로 증가시키거나 다른 방식으로 변화시켜 다른 이동 속도를 만들 수도 있습니다.

사인 함수처럼 반복적인 결과를 내는 함수를 사용해 왔다 갔다 하는 움직임을 표현할 수도 있습니다.

이러한 비선형 전환은 장면에 활력을 불어넣는 경우가 많습니다. 곡선을 따라 가속하거나 점차 감속하는 움직임은 객체나 캐릭터의 성격을 많은 것을 말해줄 수 있습니다. 탄력 있는 효과를 더하는 수학 함수까지 활용할 수 있습니다.

```ts
// 커스텀 컴포넌트 정의
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

// 시스템 정의
function LerpMove(dt: number) {
	let transform = Transform.getMutable(myEntity)
	let lerp = LerpTransformComponent.getMutable(myEntity)
	if (lerp.fraction < 1) {
		lerp.fraction += dt * lerp.speed
		const interpolatedValue = interpolate(lerp.fraction)
		transform.position = Vector3.lerp(lerp.start, lerp.end, interpolatedValue)
	}
}

// lerp 비율을 지수 곡선에 매핑
function interpolate(t: number) {
	return t * t
}

engine.addSystem(LerpMove)

// 엔티티 생성
const myEntity = engine.addEntity()

Transform.create(myEntity, {
	position: { x: 4, y: 1, z: 4 },
})

MeshRenderer.setBox(myEntity)

LerpTransformComponent.create(myEntity, {
	start: Vector3.create(4, 1, 4),
	end: Vector3.create(8, 1, 8),
	fraction: 0,
	speed: 1,
})
```

위 예제는 앞서 보여준 선형 lerp 예제와 같지만, `fraction` 필드가 매 틱마다 비선형 값에 매핑됩니다. 이 비선형 값은 다음을 계산하는 데 사용됩니다: `lerp` 함수이며, 그 결과 지수 곡선을 따르는 움직임이 만들어집니다.

위에서 보인 것과 같은 방식으로, 선형 전환을 곡선에 매핑해 회전이나 스케일 전환에도 적용할 수 있습니다.

![](/docs/_remote/95fb7814c57340842f047b62.webp)

### 시스템을 통해 경로 따라가기

엔티티가 벡터 배열을 순환하면서 각 점 사이에서 lerp 이동을 수행해 더 복잡한 경로를 따라가게 할 수 있습니다.

```ts
// 커스텀 컴포넌트 정의
const PathTransportData = {
	path: Schemas.Array(Schemas.Vector3),
	start: Schemas.Vector3,
	end: Schemas.Vector3,
	fraction: Schemas.Float,
	speed: Schemas.Float,
	pathTargetIndex: Schemas.Int,
}

export const LerpTransformComponent = engine.defineComponent(
	'LerpTransformComponent',
	PathTransportData
)

// 시스템 정의
function PathMove(dt: number) {
	let transform = Transform.getMutable(myEntity)
	let lerp = LerpTransformComponent.getMutable(myEntity)
	if (lerp.fraction < 1) {
		lerp.fraction += dt * lerp.speed
		transform.position = Vector3.lerp(lerp.start, lerp.end, lerp.fraction)
	} else {
		lerp.pathTargetIndex += 1
		if (lerp.pathTargetIndex >= lerp.path.length) {
			lerp.pathTargetIndex = 0
		}
		lerp.start = lerp.end
		lerp.end = lerp.path[lerp.pathTargetIndex]
		lerp.fraction = 0
	}
}

engine.addSystem(PathMove)

// 엔티티 생성
const myEntity = engine.addEntity()

Transform.create(myEntity, {
	position: Vector3.create(1, 1, 1),
})

MeshRenderer.setBox(myEntity)

const point1 = Vector3.create(1, 1, 1)
const point2 = Vector3.create(8, 1, 3)
const point3 = Vector3.create(8, 4, 7)
const point4 = Vector3.create(1, 1, 7)

const myPath = [point1, point2, point3, point4]

LerpTransformComponent.create(myEntity, {
	path: myPath,
	start: Vector3.create(4, 1, 4),
	end: Vector3.create(8, 1, 8),
	fraction: 0,
	speed: 1,
	pathTargetIndex: 1,
})
```

위 예제는 네 개의 3D 벡터로 구성된 3D 경로를 정의합니다. `PathTransportData` 커스텀 컴포넌트는 다음의 커스텀 컴포넌트에서 사용된 것과 동일한 데이터를 보유합니다: *lerp* 위 예제와 같지만 다음을 추가합니다: `path` 배열, 즉 경로의 모든 점과 `pathTargetIndex` 필드로 현재 경로의 어느 구간이 사용 중인지 추적합니다.

이 시스템은 다음의 시스템과 매우 유사합니다: *lerp* 예제이지만, lerp 동작이 완료되면 다음을 설정합니다: `목표` 및 `원점` 필드를 새 값으로 바꿉니다. 경로의 끝에 도달하면 경로의 첫 번째 값으로 돌아갑니다.

![](/docs/_remote/98be0ad1836095ed41c8b3e5.webp)

## 텍스처 트윈

텍스처가 부드럽게 미끄러지듯 이동하게 하려면 다음을 사용하세요: `Tween` 컴포넌트를 다음과 함께 생성합니다 `setTextureMove` 함수를 사용하세요.

```ts
Tween.setTextureMove(myEntity, 
	Vector2.create(0, 0), 
	Vector2.create(1, 0), 
	2000
)
```

텍스처 트윈은 다음 정보를 사용합니다:

* `entity`: 텍스처를 이동시킬 엔티티
* `시작`: 시작 위치를 위한 Vector2
* `종료`: 종료 위치를 위한 Vector2
* `지속 시간`: 두 위치 사이를 이동하는 데 걸리는 밀리초 수

다음 선택적 매개변수도 사용할 수 있습니다:

* `movementType`: (선택 사항) 이동이 offset 또는 tiling 필드에서 이루어질지 정의합니다. 기본값은 offset입니다.
* `easingFunction`: 사용할 이징 함수를 지정합니다. 다음을 참조하세요: [비선형 트윈](#non-linear-tweens).

## 상수 텍스처 이동

텍스처가 계속해서 미끄러지듯 이동하게 하려면 다음을 사용하세요: `Tween` 컴포넌트를 다음과 함께 생성합니다 `setTextureMoveContinuous` 함수를 사용하세요.

```ts
Tween.setTextureMoveContinuous(myEntity, 
	Vector2.create(0, 1), 
	0.7
)
```

연속 텍스처 트윈은 다음 정보를 사용합니다:

* `entity`: 텍스처를 이동시킬 엔티티
* `방향`: 이동을 위한 Vector2
* `속도`: 엔티티가 초당 몇 단위 이동할지

다음 선택적 매개변수도 사용할 수 있습니다:

* `movementType`: 이동이 offset 또는 tiling 필드에서 이루어질지 정의합니다. 기본값은 offset입니다.
* `지속 시간`: 이동을 유지할 밀리초 수입니다. 이 시간이 지나면 이동이 멈춥니다.

텍스처 트윈에 대해 더 자세히 읽어보려면 [Texture Tweens](/docs/creator/content-creator-ko/sdk7/3d/materials/#texture-tweens) 섹션을 참고하세요.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/3d/move-entities.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
