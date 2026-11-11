> 씬에서 엔티티의 위치, 회전, 크기를 설정하는 방법

# 엔티티 위치 설정

다음을 설정할 수 있습니다 *위치*, *회전* 및 *크기* 다음을 사용하여 모든 엔티티의 `Transform` 컴포넌트를 사용하여 설정할 수 있습니다. 이는 3D 공간의 어떤 엔티티에도 사용할 수 있으며, 엔티티가 렌더링되는 위치에 영향을 줍니다. 여기에는 기본 도형(큐브, 구, 평면 등), 3D 텍스트 도형, NFT 도형, 3D 모델(`GltfContainer`).

## Creator Hub의 Scene Editor를 사용하세요

Scene Editor를 통해 씬에 항목을 추가하면, 여기에는 암묵적으로 다음이 포함됩니다: **Transform** 컴포넌트. 그런 다음 엔티티의 위치, 회전 또는 크기를 변경하여 암묵적으로 Transform 컴포넌트의 값을 바꿉니다. 더 높은 정밀도가 필요하면 Scene Editor의 UI를 사용해 수치를 직접 입력할 수도 있습니다.

## 코드 핵심

![](/docs/_remote/d0b3835ca11dd6cc73d8d261.webp)

```ts
// 새 엔티티 생성
const ball = engine.addEntity()

// 이 엔티티에 모양을 부여해 보이게 합니다
MeshRenderer.setSphere(ball)

// 이 엔티티에 Transform 컴포넌트를 부여합니다
Transform.create(ball, {
	position: Vector3.create(5, 1, 5),
	scale: Vector3.create(1, 1, 1),
	rotation: Quaternion.Identity(),
})
```

씬에서 엔티티를 일정 시간에 걸쳐 이동, 회전 또는 크기 조정하려면 이 컴포넌트의 값을 프레임마다 조금씩 변경하세요. 다음을 참조하세요: [엔티티 이동](/docs/creator/content-creator-ko/sdk7/3d/move-entities/) 에서 자세한 내용과 모범 사례를 확인하세요.

생략한 필드는 모두 기본값을 사용합니다: 위치 `0, 0, 0`, 회전 없음, 크기는 `1, 1, 1`. 값을 다음으로 설정하기보다는 필드를 생략하세요: `undefined`. 명시적으로 `undefined`, 예를 들어 설정되지 않은 변수에서 온 값은 기본값을 대체하며, 씬은 이를 호출한 줄이 아니라 다음 엔진 업데이트에서 오류를 발생시킵니다: `Transform.create()`.

```ts
// ❌ savedRotation이 undefined이면, 씬은 나중에 오류를 발생시킵니다
Transform.create(ball, { position: Vector3.create(5, 1, 5), rotation: savedRotation })

// ✅ 실제 값으로 대체
Transform.create(ball, { position: Vector3.create(5, 1, 5), rotation: savedRotation ?? Quaternion.Identity() })
```


> [!WARNING]
> **📔 참고**: `Vector3` 및 `Quaternion` 는 다음을 통해 가져와야 합니다:
> 
> > `import { Vector3, Quaternion } from "@dcl/sdk/math"`
> 
> 다음을 참조하세요 [가져오기](/docs/creator/content-creator-ko/sdk7/getting-started/coding-scenes/#imports) 를 쉽게 다루는 방법을 확인하세요.


## 위치

`위치` 은 *3D 벡터*, 이는 엔티티 중심의 위치를 세 축 모두에 설정합니다, *x*, *y*, 그리고 *z*. 다음을 참조하세요: [기하 도형 유형](/docs/creator/content-creator-ko/sdk7/3d/special-types/) 에서 자세한 내용을 확인하세요.

```ts
// 새 엔티티 생성
const ball = engine.addEntity()

// 미리 정의된 위치로 Transform 생성
Transform.create(ball, {
	  position: Vector3.create(5, 1, 5)
})

// Transform의 변경 가능한 버전을 가져옵니다
const mutableTransform = Transform.getMutable(ball)

// 객체로 위치를 설정
mutableTransform.position = { x: 5, y: 1, z: 5 }

// 객체로 위치를 설정(대체 구문)
mutableTransform.position = Vector3.create(2, 1, 4)

// 각 축을 개별적으로 설정
mutableTransform.position.x = 3
mutableTransform.position.y = 1
mutableTransform.position.z = 3
```

위치를 설정할 때는 다음 사항을 염두에 두세요:

* 위치 벡터의 숫자는 다음을 나타냅니다: *미터* (엔티티가 크기가 조정된 엔티티의 자식이 아닌 경우).
* 단일 파셀로 구성된 씬의 크기는 16m x 16m입니다. 씬의 중심(지면 수준)은 `x:8, y:0, z:8`. 씬이 여러 파셀로 구성되어 있으면 중심은 배치에 따라 달라집니다.
* `x:0, y:0, z:0` 는 *남서쪽* 을 가리키며, 씬의 기본 파셀의 지면 수준 모서리입니다.

  > 팁: 씬 미리보기를 볼 때, 씬의 (0,0,0) 지점에 각 축 라벨이 표시된 나침반이 참고용으로 나타납니다.

  > 참고: 다음을 편집하여 씬의 기본 파셀을 변경할 수 있습니다: `base` 속성: *scene.json*.
* 방향을 더 쉽게 파악하려면 자신의 *왼손* 을 사용하세요:
  * 검지(앞을 가리키는 손가락)가 *z* 축
  * 중지(옆을 가리키는 손가락)가 *x* 축
  * 엄지(위로 가리키는 손가락)가 *y* 축입니다.
* 엔티티가 다른 엔티티의 자식이면, `x:0, y:0, z:0` 이는 씬 어디에 있든 부모 엔티티의 중심을 가리킵니다.
* 씬의 모든 엔티티는 항상 자신이 차지하는 파셀의 경계 안에 위치해야 합니다. 엔티티가 이 경계를 벗어나면 오류가 발생합니다.

  > 팁: 미리보기 모드에서 씬을 볼 때 경계를 벗어난 엔티티는 다음 색으로 강조 표시됩니다: *빨간색*.
* 씬의 높이에도 제한이 있습니다. 차지하는 파셀 수와 관계없이 모든 씬의 최대 높이는 330미터입니다. 200미터를 초과해 배치된 엔티티는 멀티플레이 동기화 문제가 발생할 수 있습니다. 다음을 참조하세요: [씬 제한 사항](/docs/creator/content-creator-ko/sdk7/optimizing/scene-limitations/) 에서 자세한 내용을 확인하세요.

## 회전

`회전` 은 다음으로 저장됩니다: [*쿼터니언*](https://en.wikipedia.org/wiki/Quaternion), 즉 4개의 숫자로 이루어진 체계입니다, *x*, *y*, *z* 및 *w*. 이 숫자들은 각각 -1에서 1 사이의 값을 가집니다. 다음을 참조하세요: [기하 도형 유형](/docs/creator/content-creator-ko/sdk7/3d/special-types/) 에서 자세한 내용을 확인하세요.

```ts
// 새 엔티티 생성
const cube = engine.addEntity()

// 미리 정의된 회전 0으로 Transform 생성
Transform.create(cube, {
	  rotation: Quaternion.Identity()
})

// Transform의 변경 가능한 버전을 가져옵니다
const mutableTransform = Transform.getMutable(cube)

// 오일러 각을 사용해 객체로 회전 설정
mutableTransform.rotation = Quaternion.fromEulerDegrees(0, 90, 0)

// 객체로 회전 설정
mutableTransform.rotation = { x: 0.1, y: 0.5, z: 0.5, w: 0 }

// 각 축을 개별적으로 설정
mutableTransform.rotation.x = 0
mutableTransform.rotation.y = 1
mutableTransform.rotation.z = 0.3
mutableTransform.rotation.w = 0
```

회전 필드는 다음을 사용해 설정할 수도 있습니다: [*오일러* 각도](https://en.wikipedia.org/wiki/Euler_angles), 더 일반적인 *x*, *y* 및 *z* 대부분의 사람이 익숙한 0에서 360까지의 숫자를 사용하는 표기법입니다. 오일러 각을 사용하려면 다음 표기법 중 하나를 사용하세요:

```ts
// 오일러 각으로 미리 정의된 회전값을 가진 Transform 생성
Transform.create(cube, {
	  rotation: Quaternion.fromEulerDegrees(0, 90, 0)
})

// Transform의 변경 가능한 버전을 가져옵니다
const mutableTransform = Transform.getMutable(cube)

// 오일러 각을 사용해 객체로 회전 설정
mutableTransform.rotation = Quaternion.fromEulerDegrees(0, 90, 0)
```

다음을 사용할 때 *3D 벡터* 오일러 각을 나타내는 데 *x*, *y* 및 *z* 해당 축의 회전을 도(degree) 단위로 나타냅니다. 한 바퀴는 360도가 필요합니다.

엔티티의 회전을 가져오면 기본적으로 쿼터니언이 반환됩니다. 오일러 각으로 표현된 회전을 얻으려면 다음을 사용하세요: `Quaternion.toEulerAngles()`:

```ts
// Transform의 읽기 전용 버전을 가져옵니다
const transform = Transform.get(cube)

// 오일러 각으로 표현된 회전을 가져옵니다
const eulerAngle = Quaternion.toEulerAngles(transform.rotation)
```

## 엔티티의 전역 위치와 회전 가져오기

다음 `getWorldPosition` 및 `getWorldRotation` 함수는 엔티티의 전역 위치와 회전을 반환합니다. 즉, 부모 계층을 무시하고 플레이어가 아이템을 보게 되는 인식된 위치 또는 회전을 반환합니다.

* `getWorldPosition(engine, entity: Entity): Vector3Type`: 이 함수는 엔티티의 월드 위치를 반환하며, 엔티티 자신에게 부모 엔티티가 있다면 그 모든 위치를 고려합니다. 반환값은 `{x: 0, y: 0, z: 0}` 엔티티에 Transform이 없으면 이 값을 반환합니다.

```ts
const worldPos = getWorldPosition(engine, childEntity)
console.log(`World position: ${worldPos.x}, ${worldPos.y}, ${worldPos.z}`)
```

* `getWorldRotation(engine, entity: Entity): QuaternionType`: 이 함수는 엔티티의 월드 회전을 반환하며, 엔티티 자신에게 부모 엔티티가 있다면 그 모든 회전을 고려합니다. 반환값은 `Quaternion` 형식이며, 항등 쿼터니언을 반환합니다 `{x: 0, y: 0, z: 0, w: 1}` 엔티티에 Transform이 없으면 이 값을 반환합니다.

```ts
const worldRot = getWorldRotation(engine, childEntity)
console.log(`World rotation: ${worldRot.x}, ${worldRot.y}, ${worldRot.z}, ${worldRot.w}`)
```


> [!NOTE]
> **참고:** 전역 위치와 전역 회전은 Genesis City가 아니라 씬 내부 좌표를 기준으로 합니다.


## 플레이어를 향하게 하기

다음을 추가하세요 *빌보드* 컴포넌트를 엔티티에 추가하면 항상 플레이어의 카메라를 향하도록 회전합니다.

빌보드는 90년대 3D 게임에서 흔히 사용되던 기법으로, 대부분의 엔티티가 항상 플레이어를 향하는 2D 평면이었습니다. 같은 아이디어를 3D 모델을 회전시키는 데도 사용할 수 있습니다.


> [!WARNING]
> **📔 참고**: 빌보드는 **카메라**, 아바타가 아닙니다. 3인칭에서는 카메라가 아바타 뒤 위에 있으므로 둘은 다른 위치에 있고, 카메라를 향하도록 한 빌보드는 아바타와 눈에 띄게 어긋납니다. 대신 아바타를 향하게 하려면 다음을 설정하세요: `targetEntity` 에서 `engine.PlayerEntity`.


```ts
// 새 엔티티 생성
const cube = engine.addEntity()

// 엔티티에 보이는 모양을 부여
MeshRenderer.setBox(cube)

// 미리 정의된 위치로 Transform 생성
Transform.create(cube, {
	  position: Vector3.create(5, 1, 5)
})

// 엔티티에 Billboard 컴포넌트를 부여
Billboard.create(cube, {})
```

다음 매개변수로 빌보드의 동작을 구성할 수 있습니다:

* `billboardMode`: 다음의 값을 사용합니다: `BillboardMode` 를 사용해 어떤 회전 축이 자동으로 플레이어를 향할지 설정합니다:
  * `BillboardMode.BM_ALL`: 엔티티는 모든 회전 축에서 플레이어를 향해 회전합니다. 플레이어가 엔티티보다 높이 있으면 엔티티는 위를 향하게 됩니다.
  * `BillboardMode.BM_NONE`: 엔티티는 전혀 회전하지 않습니다.
  * `BillboardMode.BM_X`: 엔티티는 자신의 *x* 회전 축에서만 회전합니다.
  * `BillboardMode.BM_Y`: 엔티티는 자신의 *y* 회전 축에서만 회전합니다. 위아래가 아니라 좌우로만 회전합니다. 플레이어가 엔티티 위나 아래에 있어도 지면에 수직을 유지합니다.
  * `BillboardMode.BM_Z`: 엔티티는 자신의 *z* 회전 축에서만 회전합니다.
* `targetEntity`: (선택 사항) 플레이어의 카메라 대신 빌보드가 향하도록 회전할 엔티티입니다. 참조된 엔티티가 존재하지 않으면(예: 아직 생성되지 않았거나 제거된 경우), 빌보드는 회전을 멈추고 엔티티가 다시 존재할 때까지 마지막 방향을 유지합니다. 이를 다음으로 설정하면 `engine.CameraEntity` 설정하지 않은 것과 같습니다.

```ts
// 평면 빌보드
const perpendicularPlane = engine.addEntity()

Transform.create(perpendicularPlane, {
	position: Vector3.create(8, 1, 8),
})

MeshRenderer.setPlane(perpendicularPlane)

Billboard.create(perpendicularPlane, {
	billboardMode: BillboardMode.BM_Y,
})

// 텍스트 레이블
const textLabel = engine.addEntity()

Transform.create(textLabel, {
	position: Vector3.create(6, 1, 6),
})

TextShape.create(textLabel, {
	text: 'This text is always readable',
})

Billboard.create(textLabel)

// 플레이어 대신 다른 엔티티를 향하는 표지판
const sphere = engine.addEntity()

Transform.create(sphere, {
	position: Vector3.create(4, 2, 4),
})

MeshRenderer.setSphere(sphere)

const sign = engine.addEntity()

Transform.create(sign, {
	position: Vector3.create(8, 1, 8),
})

TextShape.create(sign, {
	text: 'Watching the sphere',
})

Billboard.create(sign, {
	targetEntity: sphere,
})
```


> [!NOTE]
> **💡 팁**: 빌보드는 다음에 추가하기에 매우 유용합니다: *텍스트* 엔티티에 추가하면 항상 읽기 쉽게 만들어 주기 때문입니다.


설정 `targetEntity` 에서 `engine.PlayerEntity` 를 카메라 대신 아바타를 따르도록 설정하세요. 이름표, 말풍선, 또는 플레이어의 시점이 아니라 아바타에 속하는 다른 어떤 것에도 이렇게 하면 됩니다:

```ts
// 카메라가 아니라 아바타를 향하도록 회전하는 라벨
Billboard.create(label, {
	targetEntity: engine.PlayerEntity,
	billboardMode: BillboardMode.BM_Y,
})
```

아바타의 위치는 발에 있으므로, 머리 위의 라벨은 아래로 기울어 아바타를 향하게 됩니다. `BM_Y` 는 라벨을 똑바로 세운 채 좌우로만 회전시킵니다.

### 빌보드가 향하는 방향

빌보드는 자신의 로컬 **-Z** 축을 향하는 방향으로 가리킵니다. 이는 다른 엔티티를 빌보드의 자식으로 둘 때 중요합니다:

* 로컬 **+Z** 오프셋에 있는 자식은 **뒤에** 배치되며, 빌보드에 가려집니다.
* 로컬 **-Z** 오프셋에 있는 자식은 **앞에**, 시청자에게 더 가깝게 배치됩니다.

따라서 빌보드 라벨 뒤에 프레임이나 배경을 두려면 다음 축으로 오프셋하세요: **+Z**. 앞에 강조 표시를 두려면 다음 축으로 오프셋하세요: **-Z**.

다음 `회전` 엔티티의 `Transform` 컴포넌트 값은 빌보드가 플레이어를 따라다녀도 변하지 않습니다.

엔티티에 다음 두 가지가 모두 있으면 `빌보드` 컴포넌트와 `Transform` 다음 값을 가진 컴포넌트가 있으면 `회전` 값이 있다면, 플레이어는 엔티티가 빌보드처럼 회전하는 것을 보게 됩니다. 빌보드가 모든 축에 영향을 주지 않는 경우, 나머지 축은 다음에 따라 회전합니다: `Transform` 컴포넌트.


> [!WARNING]
> **📔 참고**: 동시에 여러 플레이어가 있으면 각 플레이어는 자신을 향하는 빌보드 모드의 엔티티를 보게 됩니다. 빌보드 회전은 각 플레이어에 대해 로컬로 계산되며 다른 사람이 보는 것에는 영향을 주지 않습니다. 이는 다음이 있는 빌보드에는 적용되지 않습니다: `targetEntity`: 대상의 위치가 씬의 일부이므로, 모든 플레이어는 해당 빌보드를 같은 방향으로 보게 됩니다.


## 좌표 세트를 향하기

엔티티 A가 엔티티 B를 바라보게 하려면:

```
1) 엔티티 B의 위치에서 엔티티 A의 위치를 빼서 둘 사이의 거리를 나타내는 벡터를 구합니다.
2) 그 벡터를 정규화하여 길이가 1이 되도록 하되 방향은 유지합니다.
3) `Quaternion.lookRotation`을 사용해 해당 방향으로의 회전을 나타내는 Quaternion 회전을 구합니다.
4) 해당 Quaternion을 엔티티 A의 회전으로 설정합니다
```

```ts
export function turn(entity: Entity, target: Vector3.ReadonlyVector3) {
	const transform = Transform.getMutable(entity)
	const difference = Vector3.subtract(target, transform.position)
	const normalizedDifference = Vector3.normalize(difference)
	transform.rotation = Quaternion.lookRotation(normalizedDifference)
}
```

## 크기

`크기` 도 역시 *3D 벡터*, 다음으로 저장됩니다: `Vector3` 객체이며, 다음 축의 크기 계수를 포함합니다: *x*, *y* 및 *z* 축. 엔티티의 모양은 기본 도형이든 3D 모델이든 그에 맞게 크기가 조정됩니다.

기본 크기는 1이므로, 엔티티를 늘리려면 1보다 큰 값을, 줄이려면 1보다 작은 값을 지정하세요.

```ts
// 새 엔티티 생성
const ball = engine.addEntity()

// 미리 정의된 위치로 Transform 생성
Transform.create(ball, {
	  scale: Vector3.create(5, 5, 5)
})

// Transform의 변경 가능한 버전을 가져옵니다
const mutableTransform = Transform.getMutable(ball)

// Vector3로 크기 설정

mutableTransform.scale = Vector3.create(2, 2, 2)

// 객체로 위치를 설정
mutableTransform.scale = { x: 5, y: 1, z: 5 }

// 각 축을 개별적으로 설정
mutableTransform.scale.x = 3
mutableTransform.scale.y = 3
mutableTransform.scale.z = 2
```

## 부모로부터 변형 상속

엔티티가 다른 엔티티 안에 중첩되면 자식 엔티티는 부모의 컴포넌트를 상속합니다. 즉, 부모 엔티티의 위치, 크기 또는 회전이 바뀌면 자식도 영향을 받습니다. 자식 엔티티의 위치, 회전, 크기 값은 부모의 값을 덮어쓰지 않고, 대신 서로 합쳐집니다.

엔티티를 다른 엔티티의 부모로 지정하려면 다음을 설정합니다: `parent` 자식 엔티티의 `Transform` 컴포넌트.

부모 엔티티의 크기가 조정되면 자식들의 모든 위치 값도 함께 조정됩니다.

```ts
// 엔티티 생성
const parentEntity = engine.addEntity()
const childEntity = engine.addEntity()

// 부모에 대한 Transform 생성
Transform.create(parentEntity, {
	position: Vector3.create(3, 1, 1),
	scale: Vector3.create(0.5, 0.5, 0.5),
})

// 자식에 대한 Transform을 만들고 자식으로 지정
Transform.create(childEntity, {
	position: Vector3.create(0, 1, 0),
	parent: parentEntity,
})
```

이 예에서 자식 엔티티는 부모가 그 크기를 가지고 있으므로 0.5로 축소됩니다. 자식 엔티티의 위치도 부모를 기준으로 하게 됩니다. 부모의 위치에 자식의 위치를 더해야 합니다. 이 경우 부모가 절반 크기로 축소되었으므로 자식의 변형도 비례해서 축소됩니다. 절대적으로는 자식이 다음 위치에 배치됩니다: `{ x: 3, y: 1.5, z: 1 }`. 부모가 다음을 가지고 있었다면 `회전`, 이는 자식이 이동하는 축을 바꾸므로 자식의 최종 위치에도 영향을 줍니다.

자식 엔티티에 다음이 없으면 `위치` Transform에 설정된 값이 없으면 기본값은 `0,0,0`, 부모와 같은 위치에 배치됩니다.

모양 컴포넌트가 없는 보이지 않는 엔티티를 부모로 사용해 다른 엔티티 묶음을 감쌀 수 있습니다. 이 엔티티는 렌더링된 씬에는 보이지 않지만, 자식들을 그룹화하고 모두에 Transform을 적용하는 데 사용할 수 있습니다.


> [!WARNING]
> **📔 참고**: 계층 구조에서 엔티티가 결국 자기 자신의 조상이 되는 루프를 만들지 마세요. SDK는 이러한 루프를 감지하고 관련 엔티티 이름과 함께 경고를 기록합니다. 부모-자식 관계를 수정하기 전까지 영향을 받은 엔티티는 올바르게 배치되지 않습니다.


## 엔티티를 아바타에 부착

엔티티를 플레이어에 부착하는 방법은 세 가지입니다:

* 다음의 자식으로 만드세요 **Avatar Entity**
* 다음의 자식으로 만드세요 **Camera Entity**
* 다음을 사용하세요 **AvatarAttach 컴포넌트**

엔티티를 아바타에 부착하는 가장 간단한 방법은 부모를 다음으로 설정하는 것입니다: [예약된 엔티티](/docs/creator/content-creator-ko/sdk7/architecture/entities-components/#reserved-entities) `engine.PlayerEntity`. 그러면 엔티티는 플레이어의 위치와 함께 이동합니다.

```ts
let childEntity = engine.addEntity()

MeshRenderer.setCylinder(childEntity)

Transform.create(childEntity, {
	scale: Vector3.create(0.2, 0.2, 0.2),
	position: Vector3.create(0, 0.4, 0),
	parent: engine.PlayerEntity,
})
```

엔티티를 다음으로 설정할 수도 있습니다: [예약된 엔티티](/docs/creator/content-creator-ko/sdk7/architecture/entities-components/#reserved-entities) `engine.CameraEntity`. 1인칭에서 카메라 엔티티를 사용하면 부착된 엔티티가 카메라의 움직임을 따라갑니다. 이는 무언가를 항상 화면에 두는 데 이상적입니다. 예를 들어 카메라가 위를 보더라도 총의 3D 모델을 항상 보이게 유지하는 데 적합합니다.

```ts
let childEntity = engine.addEntity()

MeshRenderer.setCylinder(childEntity)

Transform.create(childEntity, {
	scale: Vector3.create(0.2, 0.2, 0.2),
	position: Vector3.create(0, 0.4, 0),
	parent: engine.CameraEntity,
})
```

객체를 아바타의 뼈대 중 하나에 부착하여 아바타의 애니메이션과 함께 움직이게 하려면 다음을 추가하세요: `AvatarAttach` 엔터티에 연결하는 컴포넌트.

아바타에서 서로 다른 앵커 포인트를 선택할 수 있으며, 이 포인트들 대부분은 플레이어의 스켈레톤에 연결되어 플레이어의 애니메이션을 따라갑니다. 예를 들어 오른손 앵커 포인트를 사용하면, 플레이어가 달리면서 아바타가 팔을 흔들거나 휘두를 때 연결된 엔터티도 함께 움직이며, 마치 플레이어가 손에 엔터티를 들고 있는 것처럼 보입니다.

```ts
// 아바타가 설정되지 않았으면 메인 플레이어에 연결합니다. avatarId가 설정되지 않으면 기본적으로 engine.PlayerEntity가 사용됩니다
AvatarAttach.create(myEntity, {
	anchorPointId: AvatarAnchorPointType.AAPT_NAME_TAG,
})

// ID로 플레이어에 연결
AvatarAttach.create(myEntity, {
	avatarId: '0xAAAAAAAAAAAAAAAAA',
	anchorPointId: AvatarAnchorPointType.AAPT_NAME_TAG,
})
```

다음과 같은 `AvatarAttach` 컴포넌트를 생성할 때는 다음 데이터를 포함한 객체를 전달하세요:

* `avatarId`: *선택 사항* 연결할 플레이어의 ID입니다. Ethereum 지갑에 연결된 플레이어의 경우 이는 플레이어의 Ethereum 주소와 동일합니다. 지정하지 않으면 엔터티가 로컬 플레이어의 아바타에 연결됩니다.
* `anchorPointId`: enum의 값을 사용하여 엔터티를 아바타 스켈레톤의 어느 앵커 포인트에 연결할지 지정합니다 `AvatarAnchorPointType`.


> [!WARNING]
> **📔 참고**: 씬의 모든 플레이어가 같은 플레이어에 연결된 오브젝트를 보게 하려면, 예를 들어 모두가 플레이어 A가 오브젝트를 집어 들어 왼손에 들고 있는 것을 보게 하려면, 다음 값에 값을 제공해야 합니다 `avatarId`. 지정하지 않으면 모든 플레이어는 오브젝트가 자신의 아바타에 연결된 것으로 보게 됩니다.


다음 예시는 특정 아바타에 연결된 엔터티를 배치하여, 다른 모든 플레이어가 그것이 같은 아바타에 연결되어 있는 것을 보게 합니다.

```ts
import { getPlayer } from '@dcl/sdk/src/players'
import { AvatarAnchorPointType, AvatarAttach, engine, Entity } from '@dcl/sdk/ecs'
import { syncEntity } from '@dcl/sdk/network'

async function attachToPlayer(){

 let userData = await getPlayer()
 console.log(userData)

 if (!userData || !userData.wearables) return

  let entity = engine.addEntity()

  AvatarAttach.create(entity, {
    avatarId: userData.userId,
    anchorPointId: AvatarAnchorPointType.AAPT_RIGHT_HAND,
  })

  // 다른 컴포넌트

  syncEntity(entity, [AvatarAttach.componentId])

}
```

다음 앵커 포인트를 사용할 수 있습니다 `AvatarAnchorPointType` enum:

* `AAPT_RIGHT_HAND`: 플레이어의 오른손에 고정
* `AAPT_LEFT_HAND`: 플레이어의 왼손에 고정
* `AAPT_HEAD`: 플레이어 머리 중앙에 고정.
* `AAPT_NECK`: 플레이어 목의 아래쪽에 고정.
* `AAPT_SPINE`: 척추 상단 부분에 고정.
* `AAPT_SPINE1`: 척추 중간 부분에 고정.
* `AAPT_SPINE2`: 척추 아래쪽 부분에 고정.
* `AAPT_HIP`: 엉덩이뼈에 고정.
* `AAPT_LEFT_SHOULDER`: 왼쪽 어깨에 고정.
* `AAPT_LEFT_ARM`: 왼쪽 첫 번째 팔뼈의 어깨 높이에 고정.
* `AAPT_LEFT_FOREARM`: 왼쪽 아래팔 뼈에 고정.
* `AAPT_LEFT_HAND_INDEX`: 왼손 검지손가락 끝에 고정.
* `AAPT_RIGHT_SHOULDER`: 오른쪽 어깨에 고정.
* `AAPT_RIGHT_ARM`: 오른쪽 첫 번째 팔뼈의 어깨 높이에 고정.
* `AAPT_RIGHT_FOREARM`: 오른쪽 아래팔 뼈에 고정.
* `AAPT_RIGHT_HAND_INDEX`: 오른손 검지손가락 끝에 고정.
* `AAPT_LEFT_UP_LEG`: 왼쪽 다리의 위쪽 다리 뼈에 고정.
* `AAPT_LEFT_LEG`: 왼쪽 다리의 아래쪽 다리 뼈에 고정.
* `AAPT_LEFT_FOOT`: 왼쪽 발목에 고정.
* `AAPT_LEFT_TOE_BASE`: 왼쪽 발가락 끝에 고정.
* `AAPT_RIGHT_UP_LEG`: 오른쪽 다리의 위쪽 다리 뼈에 고정.
* `AAPT_RIGHT_LEG`: 오른쪽 다리의 아래쪽 다리 뼈에 고정.
* `AAPT_RIGHT_FOOT`: 오른쪽 발목에 고정.
* `AAPT_RIGHT_TOE_BASE`: 오른쪽 발가락 끝에 고정.
* `.AAPT_NAME_TAG`: 플레이어 이름 태그 바로 위에 떠 있으며, 플레이어의 애니메이션 영향을 받지 않습니다.

  > 참고: 이름 태그의 높이는 플레이어가 착용한 웨어러블의 높이에 따라 동적으로 조정됩니다. 따라서 키가 큰 모자를 쓴 플레이어의 이름 태그는 다른 플레이어보다 조금 더 높게 표시됩니다.
* `AAPT_POSITION` *더 이상 사용되지 않음*: 플레이어의 전체 위치입니다. 이는 플레이어 발에서 0.8 높이에 표시됩니다.

  >


> [!WARNING]
> \> \*\*📔 참고\*\*: \`AAPT\_POSITION\`은 더 이상 사용되지 않습니다. 플레이어의 전체 위치를 따라가게 하려면 엔터티를 Avatar Entity의 자식으로 만드는 것이 가장 좋습니다. 이 섹션의 시작 부분에서 예시를 확인하세요. >



> [!NOTE]
> **💡 팁**: 이 값들을 사용하려면 다음과 같이 작성하세요 `AvatarAnchorPointType.` 그러면 VS Code에서 드롭다운으로 전체 옵션 목록이 표시됩니다.


![](/docs/_remote/a3894bbddd0ce5df1254538a.webp)

엔터티 렌더링은 씬의 각 인스턴스에서 로컬로 결정됩니다. 한 플레이어에 엔터티를 연결해도 그 플레이어를 보고 있는 다른 플레이어에게는 보이지 않습니다. 엔터티가 기본 로컬 플레이어에 연결되면, 각 플레이어는 그 엔터티가 자신의 아바타에 연결된 것처럼 경험하게 됩니다.


> [!WARNING]
> **📔 참고**: 아바타에 연결된 엔터티는 렌더링되려면 씬 경계 안에 있어야 합니다. 플레이어가 씬 밖으로 나가면 연결된 엔터티는 플레이어가 다시 들어올 때까지 렌더링이 중지됩니다. 스마트 웨어러블에는 이 제한이 없습니다.


다음 `AvatarAttach` 컴포넌트는 다음의 값을 덮어씁니다 `Transform` 컴포넌트에 넣은 값들은 `Transform` 플레이어의 Transform에 대한 엔터티의 상대적 위치로 대체되며, 플레이어가 이동하고 애니메이션할 때 프레임마다 업데이트됩니다.

아바타의 앵커 포인트에서 오프셋을 두거나 다른 회전이나 스케일을 적용해야 한다면, 부모 엔터티를 통해 처리하세요.

1. 다음만 가진 보이지 않는 엔터티를 생성하세요: `Transform` 및 하나의 `AvatarAttach` 컴포넌트. 해당 컴포넌트의 `Transform` 값은 플레이어가 이동할 때 덮어써집니다
2. 연결할 엔터티를 이 부모의 자식으로 설정하세요. 해당 엔터티의 `Transform` 값은 앵커 포인트로부터의 오프셋을 나타낼 수 있습니다.

```ts
// 부모 엔터티 생성
const parentEntity = engine.addEntity()

// 부모 엔터티를 플레이어에 연결
AvatarAttach.create(parentEntity, {
	anchorPointId: AvatarAnchorPointType.AAPT_NAME_TAG,
})

// 자식 엔터티 생성
let childEntity = engine.addEntity()

MeshRenderer.setCylinder(childEntity)

Transform.create(childEntity, {
	scale: Vector3.create(0.2, 0.2, 0.2),
	position: Vector3.create(0, 0.4, 0),
	parent: parentEntity,
})
```


> [!WARNING]
> **📔 참고**: 연결된 엔터티에 콜라이더가 있으면, 그 콜라이더가 플레이어의 이동을 막거나 떨림 현상을 유발할 수 있습니다. 연결된 엔터티의 콜라이더에 대해 물리 레이어를 비활성화하는 것이 좋을 수 있습니다. 참조: [충돌 레이어](/docs/creator/content-creator-ko/sdk7/3d/colliders/#collision-layers)



> [!WARNING]
> **📔 참고**: 자식 엔터티가 있고 그 전역 위치 및/또는 회전을 알고 싶다면, 다음을 사용할 수 있습니다 `getWorldPosition` 및 `getWorldRotation` 함수들입니다. 다음 섹션에서 확인할 수 있습니다 [전역 위치 및 회전 얻기](#getting-global-position-and-rotation-of-an-entity) 섹션.


### 다른 플레이어에 연결

다음을 사용할 수 있습니다 `AvatarAttach` 컴포넌트를 사용하여 엔터티를 다른 플레이어에 연결할 수 있습니다. 그러려면 플레이어의 ID를 알아야 합니다.

다른 플레이어의 아바타에 엔터티를 연결하려면, 필드에 사용자의 ID를 제공해야 합니다 `avatarId`. 여기에는 [다양한 방법이](/docs/creator/content-creator-ko/sdk7/interactivity/user-data/#get-player-data) 이 데이터를 얻는 방법이 있습니다.


> [!WARNING]
> **📔 참고**: Ethereum 지갑으로 연결된 플레이어의 경우, 그들의 `userId` 는 Ethereum 주소와 동일합니다.


다음을 가져오세요 `userId` 주변의 다른 모든 플레이어에 대해 다음을 통해 `getPlayer()`

```ts
executeTask(async () => {
	for (const [entity, data] of engine.getEntitiesWith(PlayerIdentityData)) {
		console.log('Player id: ', data.address)
	}
})
```

이를 다음과 함께 사용하면 `AvatarAttach`, 씬의 다른 모든 플레이어 머리 위에 떠 있는 큐브를 추가하는 다음 코드를 사용할 수 있습니다:

```ts
executeTask(async () => {
        for (const [entity, data] of engine.getEntitiesWith(PlayerIdentityData)) {
            const myEntity = engine.addEntity()
            MeshRenderer.setBox(myEntity)
            AvatarAttach.create(myEntity, {
                anchorPointId: AvatarAnchorPointType.AAPT_LEFT_HAND,
                avatarId: data.address,
            })
        }
    })
```

다른 사용자의 ID를 가져오는 다른 방법은 [플레이어 데이터 가져오기](/docs/creator/content-creator-ko/sdk7/interactivity/user-data/#get-player-data).


> [!WARNING]
> **📔 참고**: 다른 플레이어의 엔터티를 다음의 `parent` 엔터티의 부모로 설정하지 마세요. 엔진은 해당 엔터티의 `Transform` 씬이 읽을 수 있도록 해당 엔터티의 값을 쓰지만, 엔터티가 매달릴 수 있는 부모는 아니므로 자식은 씬 루트로 돌아가 씬의 원점에 머뭅니다. 다음을 사용하세요 `AvatarAttach` 대신 위에서 보인 것처럼 플레이어의 주소를 사용하세요.


플레이어 엔터티 ID는 사람들이 들어오고 나가면서 재사용되므로, 같은 엔터티가 나중에는 다른 사람의 것이 될 수 있습니다. 플레이어마다 하나의 마커를 유지한다면, 만들 때 사용한 주소를 저장하고 매 프레임 비교하세요. 엔터티의 주소가 바뀌면 이전 마커를 삭제하고 새 마커를 만드세요:

```ts
import { AvatarAttach, AvatarAnchorPointType, engine, Entity, PlayerIdentityData } from '@dcl/sdk/ecs'

// 재사용된 엔터티 ID를 알아챌 수 있도록, 마커가 만들어진 주소
const markers = new Map<Entity, { address: string; marker: Entity }>()

function markerSystem() {
	// 플레이어가 떠났거나 현재 엔터티 ID가 다른 사람에게 속한 마커를 제거합니다
	for (const [player, entry] of markers) {
		if (PlayerIdentityData.getOrNull(player)?.address === entry.address) continue
		engine.removeEntity(entry.marker)
		markers.delete(player)
	}

	for (const [player, data] of engine.getEntitiesWith(PlayerIdentityData)) {
		if (markers.has(player)) continue
		const marker = engine.addEntity()
		AvatarAttach.create(marker, {
			avatarId: data.address,
			anchorPointId: AvatarAnchorPointType.AAPT_NAME_TAG,
		})
		markers.set(player, { address: data.address, marker })
	}
}

engine.addSystem(markerSystem)
```

## 씬 경계

씬의 모든 엔터티는 씬 경계 안에 있어야 합니다. 경계 밖은 다른 플레이어가 소유한 토지 parcel입니다.

미리보기를 실행할 때 모델의 일부가 이 한계를 넘어서면, 넘친 부분은 잘려 나가 미리보기와 게시된 씬 모두에서 렌더링되지 않습니다.

씬의 엔터티 위치는 이동하는 동안 계속 확인됩니다. 엔터티가 씬을 벗어났다가 다시 돌아오면 제거된 뒤 다시 정상적으로 렌더링됩니다.

씬 바닥의 그리드는 씬의 한계를 보여주며, 기본적으로 0에서 16까지의 *x* 및 *z* 축과 최대 20까지의 *y* : 엔터티를 지하에 배치할 수 있으며, 0보다 낮은 *y* 축입니다.


> [!NOTE]
> **💡 팁**: 씬에 더 많은 parcel이 필요하다면, 프로젝트의 `scene.json` 파일에 추가할 수 있습니다. 다음을 참조하세요 [씬 메타데이터](/docs/creator/content-creator-ko/sdk7/kinds-of-projects/scene-metadata/) 에서 안내를 확인하세요. 추가하면 그리드가 확장되어 추가 parcel까지 덮는 것을 볼 수 있습니다.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/3d/entity-positioning.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
