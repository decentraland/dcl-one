> 플레이어의 카메라를 제어하는 방법을 알아보세요

# 카메라

크리에이터로서 플레이어의 카메라를 완전히 제어할 수 있습니다. 기본적으로 플레이어는 씬을 탐색하는 동안 1인칭 또는 3인칭 카메라 모드 중에서 자유롭게 선택할 수 있지만, 다른 카메라 모드를 강제할 수도 있습니다.

가상 카메라는 정적일 수도 있고, 플레이어나 다른 엔티티를 항상 바라보도록 회전할 수도 있으며, 플레이어나 다른 엔티티에 부착되어 항상 함께 움직이도록 할 수도 있습니다.


> [!WARNING]
> **📔 참고**: 기본 1인칭 및 3인칭 카메라 간 전환 방법은 다음을 참조하세요 [카메라 수정 영역](#1st-and-3rd-person-camera-modes).


## 1인칭 및 3인칭 카메라 모드

플레이어는 보통 키보드에서 V 키를 눌러 1인칭과 3인칭 카메라를 자유롭게 전환할 수 있습니다. 다음을 사용하세요 `CameraModeArea` 을 사용하여 씬의 특정 영역 안에 있는 모든 플레이어에게 카메라 모드를 1인칭 또는 3인칭으로 강제할 수 있습니다.

```ts
const entity = engine.addEntity()

CameraModeArea.create(entity, {
	area: Vector3.create(4, 3, 4),
	mode: CameraType.CT_FIRST_PERSON,
})
```

플레이어의 현재 카메라 모드가 `CameraModeArea`와 일치하지 않으면 해당 카메라 모드로 전환됩니다. 화면에는 이 변경이 씬 때문임을 설명하는 토스트가 나타납니다. 내부에 있는 동안 플레이어는 카메라 모드를 바꿀 수 없습니다. 플레이어가 `CameraModeArea`를 벗어나면, 카메라 모드는 들어오기 전에 사용하던 상태로 복원됩니다.

다음을 사용하세요 `CameraModeArea` 특정 카메라 모드를 사용했을 때 플레이어 경험이 크게 좋아지는 영역에서 유용합니다. 예를 들어 플레이어가 작은 오브젝트를 클릭해야 한다면 1인칭이 이상적이고, 플레이어가 씬에 배치된 어떤 엔티티를 머리 위로 알아차리게 하고 싶다면 3인칭이 유용할 수 있습니다. 플레이어가 카메라 모드를 바꾸는 방법을 안다고 가정하지 마세요. 처음인 플레이어들 중에는 그 옵션이 있는지 모를 수도 있고, 바꾸는 키를 기억하지 못할 수도 있습니다.


> [!WARNING]
> **📔 참고**: 카메라 수정 영역은 호스트 엔티티의 Transform 컴포넌트의 *위치* 및 *회전* 의 영향을 받지만, *크기*.



> [!WARNING]
> **📔 참고**: 여러 카메라 수정 영역이 겹치면, 씬 코드에서 마지막으로 인스턴스화된 영역이 다른 영역보다 우선합니다.


다음과 같은 `CameraModeArea` 컴포넌트를 사용하려면 다음을 제공해야 합니다:

* `area`: 수정 영역의 크기
* `mode`: 이 영역에서 강제할 카메라 모드. 다음 중에서 선택합니다 `CameraType` 열거형.

지원되는 카메라 모드는 다음과 같습니다:

* `CameraType.CT_FIRST_PERSON`
* `CameraType.CT_THIRD_PERSON`

### 카메라 모드 조회

다음을 사용하여 플레이어의 카메라 모드를 조회할 수 있습니다 `CameraMode` 컴포넌트를 `engine.CameraEntity`.

```ts
const cameraMode = CameraMode.get(engine.CameraEntity)
if (cameraMode.mode === CameraType.CT_FIRST_PERSON) {
	console.log('플레이어가 1인칭 카메라를 사용 중입니다')
} else {
	console.log('플레이어가 3인칭 카메라를 사용 중입니다')
}
```

또한 다음을 사용하여 카메라 모드 변경을 구독할 수 있습니다 `onChange` 다음의 함수 `CameraMode` 컴포넌트.

```ts
CameraMode.onChange(engine.CameraEntity, (cameraMode) => {
	if (!cameraMode) return
	console.log('플레이어의 카메라 모드가 변경되었습니다:', cameraMode.mode)
})
```

## 가상 카메라 사용하기

씬에서 커스텀 카메라 동작을 사용하려면 두 가지가 필요합니다:

* 가상 카메라 만들기: 씬에 엔티티를 만들고 여기에 `VirtualCamera`.
* 를 추가하세요: 다음을 추가하여 이 가상 카메라를 할당합니다 `MainCamera` 다음에 컴포넌트를 [예약된 엔티티](/docs/creator/content-creator-ko/sdk7/architecture/entities-components/#reserved-entities) `engine.CameraEntity`및 `VirtualCamera` 컴포넌트.

가 있는 엔티티 참조를 포함합니다. 그러면 카메라는 `VirtualCamera` 컴포넌트가 있는 엔티티에 부착됩니다. 엔티티가 이동하거나 회전하면 카메라도 함께 움직입니다.

```ts
function main() {
	const myCustomCamera = engine.addEntity()
	Transform.create(myCustomCamera, {
		position: Vector3.create(1, 2, 1),
	})
	VirtualCamera.create(myCustomCamera, {})

	const mainCamera = MainCamera.createOrReplace(engine.CameraEntity, {
		virtualCameraEntity: myCustomCamera,
	})
}
```

이 예시에서 플레이어가 씬 경계 안에 있는 한 카메라는 항상 씬의 고정된 위치에 있게 됩니다. 플레이어가 씬 경계를 벗어나자마자 기본 카메라 동작이 복원됩니다.

### 시야각

다음을 사용하여 가상 카메라의 시야각(FOV)을 설정합니다 `fov` 속성을 사용하며, 단위는 도입니다. 이 값은 해당 가상 카메라가 활성화되어 있는 동안에만 적용됩니다.

```ts
const cinematicCamera = engine.addEntity()
Transform.create(cinematicCamera, {
	position: Vector3.create(8, 3, 2),
})
VirtualCamera.create(cinematicCamera, {
	fov: 45,
})
```

더 좁은 FOV(낮은 값)는 확대에 유용하며, 컷씬이나 조준에 좋습니다. 더 넓은 FOV(높은 값)는 씬을 더 많이 보여주며, 파노라마 뷰에 유용합니다.


> [!WARNING]
> **📔 참고**: 설정하세요 `fov` 생성하는 모든 가상 카메라에 대해. 이를 생략한다고 해서 중립적인 60도가 되는 것은 아닙니다. 클라이언트는 렌즈를 초기화하지 않은 채 풀링된 카메라 인스턴스를 재사용하므로, `fov` 가 없는 카메라는 이전 카메라가 그 인스턴스에 남겨둔 시야각을 그대로 유지합니다. 넓은 항공 뷰를 보여주려던 카메라가 확대된 상태로 열릴 수도 있습니다.


씬에는 `VirtualCamera`컴포넌트가 있는 엔티티를 원하는 만큼 포함할 수 있으며, 플레이어가 이동하거나 특정 행동을 할 때 여러 가상 카메라 사이를 동적으로 전환할 수 있습니다. 한 번에 활성화되는 가상 카메라는 하나뿐이며, 이는 다음에 의해 할당됩니다 `MainCamera` 컴포넌트가 있는 `engine.CameraEntity`.

기본 카메라 동작으로 되돌리려면 값을 다음으로 설정하세요 `undefined` 의 `MainCamera.virtualCameraEntity`. 그러면 플레이어는 1인칭과 3인칭 카메라를 자유롭게 전환할 수 있습니다. 플레이어가 이 두 모드 중 하나만 사용하게 하고 싶다면 다음을 사용할 수 있습니다 [카메라 수정 영역](#1st-and-3rd-person-camera-modes) 둘 중 하나를 강제합니다.


> [!WARNING]
> **📔 참고**: 다음으로 카메라를 해제하세요 `undefined`, 절대로 `0`. 비어 있는 값만 제어권을 플레이어에게 돌려줍니다. 값 `0` 은 `engine.RootEntity`, 여기에는 `VirtualCamera`이 없으므로 엔진은 마지막으로 활성화된 가상 카메라를 그대로 유지하고 플레이어는 그 시점에 묶이게 됩니다.
> 
> 이것은 카메라를 저장하고 복원하는 코드를 잡아냅니다. 왜냐하면 `virtualCameraEntity` 는 선택 사항이며 새 `undefined` 에서는 `MainCamera`으로 읽히기 때문입니다. `?? 0` 또는 `|| 0` 대체값은 이것을 나중에 다시 기록되는 `0` 로 바꿉니다. 복원하기 전에 엔티티를 확인하세요:
> 
> ```ts
> function restoreCamera(previous: Entity | undefined) {
> 	const camera = MainCamera.getMutableOrNull(engine.CameraEntity)
> 	if (!camera) return
> 	// 실제로 존재하는 카메라만 복원하고, 그렇지 않으면 제어권을 해제합니다
> 	camera.virtualCameraEntity = previous && VirtualCamera.has(previous) ? previous : undefined
> }
> ```
> 



> [!WARNING]
> **📔 참고**: 가상 카메라가 활성화되어 있지 않을 때만 카메라 수정 영역이 플레이어에게 영향을 줍니다. 씬이 현재 가상 카메라를 사용 중이고 플레이어가 카메라 수정 영역에 들어가면 아무 일도 일어나지 않습니다.
> 
> 3D 모델에 `카메라` 노드가 콘텐츠의 일부로 포함되어 있다면, SDK에서는 이를 사용할 수 없습니다. 모든 카메라는 SDK를 사용해 엔티티로 만들어야 합니다.


```ts
function main() {
	// 커스텀 가상 카메라
	const myCustomCamera = engine.addEntity()
	Transform.create(myCustomCamera, {
		position: Vector3.create(1, 2, 1),
	})
	VirtualCamera.create(myCustomCamera, {})

	const mainCamera = MainCamera.createOrReplace(engine.CameraEntity, {
		virtualCameraEntity: myCustomCamera,
	})

	// 클릭 가능한 큐브
	const clickCube = engine.addEntity()
	Transform.create(clickCube, { position: Vector3.create(8, 0, 8) })
	MeshRenderer.setBox(clickCube)
	MeshCollider.setBox(clickCube)
	pointerEventsSystem.onPointerDown(
		{
			entity: clickCube,
			opts: { button: InputAction.IA_POINTER, hoverText: '카메라 재설정' },
		},
		() => {
			// 카메라를 기본 동작으로 재설정
			const mainCamera = MainCamera.getMutable(engine.CameraEntity)
			mainCamera.virtualCameraEntity = undefined
		}
	)
}
```


> [!NOTE]
> **💡 팁**: 카메라가 아바타에서 시선을 돌릴 때는 아바타의 움직임도 함께 멈추는 것이 좋은 경우가 많습니다. 그렇게 하면 플레이어가 장애물 쪽으로 무작정 움직이지 않게 됩니다. 다음을 참조하세요 [입력 수정자](/docs/creator/content-creator-ko/sdk7/interactivity/avatars/locomotion/#freeze-the-player)


플레이어가 마우스로 가상 카메라를 조종하게 하려면 다음을 읽으세요 `screenDelta` 속성 `PrimaryPointerInfo` 컴포넌트에서 프레임마다 커서가 얼마나 이동했는지 확인한 뒤, 그 움직임을 카메라의 회전에 적용하세요. 이 방법은 커서가 잠겨 있어도 작동합니다. 다음을 참조하세요 [마우스 이동](/docs/creator/content-creator-ko/sdk7/interactivity/mouse-movement/) 에서 전체 마우스 룩 예시를 볼 수 있습니다.

## 조감도

가상 카메라를 사용해 씬을 위에서 내려다보는 뷰로 볼 수 있으며, 이는 아바타의 일반적인 원근감에 대한 재미있는 변형이 될 수 있고 다양한 게임 메커니즘을 가능하게 합니다.

카메라를 완전히 수직 아래 방향으로 향하게 배치하는 것은 피해야 합니다. 대신 카메라는 항상 약간 기울이세요. 1도만 있어도 충분합니다. 플레이어 방향 제어는 아바타의 방향이 아니라 카메라의 원근에 기반하기 때문입니다. 카메라가 완벽한 조감도로 보이면 어느 방향이 어느 쪽인지 분명하지 않습니다. 눈에 띄지 않을 정도의 1도 기울기만으로도 전방 방향을 정할 수 있습니다.

```ts
function main() {
	// 조감도 카메라
	const myCustomCamera = engine.addEntity()
	Transform.create(myCustomCamera, {
		position: Vector3.create(8, 5, 8),
		rotation: Quaternion.fromEulerDegrees(91, 0, 0),
		// 회전이 90º가 아니라 91º라는 점에 유의하세요
	})
	VirtualCamera.create(myCustomCamera, {})

	const mainCamera = MainCamera.createOrReplace(engine.CameraEntity, {
		virtualCameraEntity: myCustomCamera,
	})
}
```

## 시야각

다음을 사용하여 가상 카메라에 커스텀 시야각(FOV)을 설정할 수 있습니다 `fov` 속성. 값은 도 단위입니다. 항상 설정하세요. 대체할 기본값이 없으며, `fov` 이 없는 카메라는 동일한 풀링 인스턴스를 사용한 이전 카메라가 남긴 렌즈를 상속합니다.

```ts
VirtualCamera.create(myCustomCamera, {
	fov: 90,
})
```

더 넓은 FOV(높은 값)는 한 번에 더 많은 씬을 보여주고 속도감을 만들어 레이싱 게임에 유용할 수 있습니다. 더 좁은 FOV(낮은 값)는 확대되며 조준이나 시네마틱 샷에 유용합니다.

### 줌 레벨 변경

클라이언트는 `fov` 카메라가 활성화되는 순간에만 읽습니다. `fov` 이미 활성화된 카메라에서

를 변경해도 아무 효과가 없습니다. 줌하려면 줌 레벨마다 가상 카메라 하나씩 만들고 `MainCamera.virtualCameraEntity` 사이를 전환하세요:

```ts
import { engine, MainCamera, Transform, VirtualCamera } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

// 같은 위치에 있는 두 카메라, 하나는 넓게, 하나는 확대
const position = Vector3.create(8, 2, 8)

const wideCamera = engine.addEntity()
Transform.create(wideCamera, { position })
VirtualCamera.create(wideCamera, { fov: 70 })

const zoomedCamera = engine.addEntity()
Transform.create(zoomedCamera, { position })
VirtualCamera.create(zoomedCamera, { fov: 30 })

// 전환하여 줌하기
function zoomIn() {
	MainCamera.createOrReplace(engine.CameraEntity, { virtualCameraEntity: zoomedCamera })
}

function zoomOut() {
	MainCamera.createOrReplace(engine.CameraEntity, { virtualCameraEntity: wideCamera })
}
```

이미 활성화된 카메라를 다시 할당해도 아무 일도 일어나지 않으므로, 매번 다른 엔티티로 전환하세요.

## 카메라 전환

씬이 가상 카메라 사이를 전환하거나, 기본 카메라 동작과 가상 카메라 사이를 전환할 때마다 플레이어는 전환 효과를 보게 됩니다. 가상 카메라의 위치, 회전 및 기타 파라미터는 일정 시간에 걸쳐 부드럽게 바뀝니다.

가상 카메라의 전환 설정은 카메라로 *들어가는* 방법을 결정하며, 기본값을 포함한 씬의 다른 어떤 카메라에서든 해당 카메라로 전환할 때 적용됩니다. 그러나 *out* 나가는

전환 방식에는 영향을 주지 않습니다. `virtualCameraEntity` 의 `MainCamera`, 플레이어 자신의 카메라로 돌아갈 때는 `을 지울 때 항상 0.5초의 고정 전환을 사용하며, 이 전환은 이징 인/아웃됩니다.` 떠나는 가상 카메라의

```ts
VirtualCamera.create(myCustomCamera1, {
	defaultTransition: { transitionMode: VirtualCamera.Transition.Time(6) },
})
```


> [!NOTE]
> **💡 팁**: 전환을 피하고 즉시 카메라를 바꾸려면 전환 시간 또는 속도를 0으로 설정하세요.


사용 사례에 따라 지속 시간 대신 전환 속도를 설정하는 것이 더 좋을 수 있습니다:

* **고정 시간**: 전환 지속 시간을 설정하면, 카메라는 그 시간 안에 경로를 완료하기 위해 필요한 만큼의 속도로 이동합니다.
* **고정 속도**: 전환 중 가상 카메라가 얼마나 빠르게 이동할지 설정합니다. 지속 시간은 거리와 함께 결정됩니다. 속도에 사용되는 값은 다음으로 해석됩니다 **초당 미터**.

아래는 이 두 전환 모드의 예시입니다:

```ts
// 고정 지속 시간
VirtualCamera.create(myCustomCamera1, {
	defaultTransition: { transitionMode: VirtualCamera.Transition.Time(6) },
})

// 고정 속도
VirtualCamera.create(myCustomCamera1, {
	defaultTransition: { transitionMode: VirtualCamera.Transition.Speed(3) },
})
```

아래는 두 개의 가상 카메라와 그 사이의 전환을 포함한 전체 예시입니다:

```ts
function main() {
	// 커스텀 가상 카메라 1
	const myCustomCamera1 = engine.addEntity()
	Transform.create(myCustomCamera1, {
		position: Vector3.create(1, 2, 1),
	})
	VirtualCamera.create(myCustomCamera1, {
		defaultTransition: { transitionMode: VirtualCamera.Transition.Time(1) },
	})

	// 커스텀 가상 카메라 2
	const myCustomCamera2 = engine.addEntity()
	Transform.create(myCustomCamera2, {
		position: Vector3.create(1, 2, 1),
	})
	VirtualCamera.create(myCustomCamera2, {
		defaultTransition: { transitionMode: VirtualCamera.Transition.Time(3) },
	})

	const mainCamera = MainCamera.createOrReplace(engine.CameraEntity, {
		virtualCameraEntity: myCustomCamera1,
	})

	// 클릭 가능한 큐브
	const clickCube = engine.addEntity()
	Transform.create(clickCube, { position: Vector3.create(8, 0, 8) })
	MeshRenderer.setBox(clickCube)
	MeshCollider.setBox(clickCube)
	pointerEventsSystem.onPointerDown(
		{
			entity: clickCube,
			opts: { button: InputAction.IA_POINTER, hoverText: '카메라 재설정' },
		},
		() => {
			// 카메라를 기본 동작으로 재설정
			const mainCamera = MainCamera.getMutable(engine.CameraEntity)
			mainCamera.virtualCameraEntity =
				mainCamera.virtualCameraEntity == myCustomCamera1
					? myCustomCamera2
					: myCustomCamera1
		}
	)
}
```

전환은 경로상의 장애물을 고려하지 않고 항상 직선으로 이동합니다. 대신 다른 가상 카메라를 중간 매개체로 사용해 전환을 수동으로 만들 수 있으며, 그러면 이동을 완전히 제어할 수 있습니다. 이 중간 가상 카메라는 [Tween](/docs/creator/content-creator-ko/sdk7/3d/move-entities/#move-between-two-points) 을 사용해 첫 번째 카메라의 위치에서 두 번째 카메라의 위치로 이동하거나, 장애물을 피하거나 영화적인 우회를 하는 더 커스텀한 경로를 따를 수 있습니다.

## 카메라 팔로우

가상 카메라가 플레이어나 씬의 특정 엔티티를 항상 바라보도록 설정할 수 있습니다. 카메라의 위치는 고정되어 있지만 회전은 이 엔티티가 항상 중앙에 오도록 변경됩니다.

이는 다음을 사용해 구현할 수 있습니다 `lookAtEntity` 속성 `VirtualCamera` 컴포넌트에서. 플레이어를 따라가려면 다음을 사용하세요 [예약된 엔티티](/docs/creator/content-creator-ko/sdk7/architecture/entities-components/#reserved-entities) `engine.PlayerEntity`.

```ts
const myCustomCamera1 = engine.addEntity()
Transform.create(myCustomCamera1, {
	position: Vector3.create(1, 2, 1),
})
VirtualCamera.create(myCustomCamera1, {
	lookAtEntity: engine.PlayerEntity,
})
```

카메라가 엔티티를 따라가고 있다면 회전만 바뀌고 카메라 위치는 바뀌지 않습니다.

카메라가 회전할 때 `VirtualCamera` 가 있는 엔티티의 Transform은 바뀌지 않습니다. 하지만 `engine.CameraEntity`의 Transform에서 카메라의 회전을 읽을 수 있습니다. 이 엔티티의 회전과 위치는 절대값이며, `VirtualCamera` 컴포넌트가 있는 엔티티의 값에 의해 조건이 붙지 않습니다. 이 Transform의 회전은 `lookAtEntity` 동작의 영향을 받습니다.


> [!WARNING]
> **📔 참고**: 가상 카메라를 `lookAtEntity` 가상 카메라가 있는 동일한 엔티티를 참조하는 `engine.CameraEntity` 또는


## 엔티티로 구성하면 결과 동작은 어떤 엔티티도 할당하지 않은 것과 동일합니다.

플레이어에게 부착하기

```ts
function main() {
	const myCustomCamera = engine.addEntity()
	Transform.create(myCustomCamera, {
		position: Vector3.create(0, 1, 5),
		parent: engine.PlayerEntity,
	})
	VirtualCamera.create(myCustomCamera, {
		defaultTransition: { transitionMode: VirtualCamera.Transition.Time(2) },
	})

	const mainCamera = MainCamera.createOrReplace(engine.CameraEntity, {
		virtualCameraEntity: myCustomCamera,
	})
}
```

## 카메라와 충돌체

플레이어의 카메라가 3인칭 모드로 움직일 때, 엔티티에 할당된 충돌 레이어에 따라 카메라가 충돌체에 의해 막힐 수도 있고 아닐 수도 있습니다. 씬을 설계할 때 이 점에 유의하세요. 카메라가 벽이나 다른 엔티티를 통과하지 못하게 하고 싶을 수 있습니다.

다음을 참조하세요 [충돌체](/docs/creator/content-creator-ko/sdk7/3d/colliders/#cameras-and-colliders) 를 참조하여 씬의 충돌체 설정 방법을 자세히 알아보세요.

## 관전자 모드(오브저버 카메라)

일반적인 아바타 이동에서 자유롭게 돌아다니는 카메라 또는 플레이어를 따라가는 카메라로 전환되는 관전자 모드를 만들 수 있습니다. 이는 경쟁 게임의 관전자 역할, 라이브 이벤트의 감독용 카메라, 리플레이 시스템에 유용합니다.

이 패턴은 여러 SDK 기능을 결합합니다:

| 기능          | SDK API                                   | 목적                       |
| ----------- | ----------------------------------------- | ------------------------ |
| 커스텀 카메라 뷰   | `VirtualCamera` + `MainCamera`            | 플레이어의 카메라를 대체함           |
| 아바타 움직임 고정  | `InputModifier` (`disableAll: true`)      | 카메라를 움직이기 위해 WASD 사용 가능  |
| 씬에서 플레이어 추적 | `onEnterScene` / `onLeaveScene`           | 팔로우 대상 명단을 구성함           |
| 카메라 제어      | `inputSystem.isPressed(InputAction.IA_*)` | 피치/요를 위한 WASD, 줌을 위한 E/F |
| 마우스 룩       | `PrimaryPointerInfo.screenDelta`          | 마우스로 카메라 회전              |

### 카메라 리그 아키텍처

다음을 사용하세요 **두 엔티티 리그** 를 사용해 요와 피치가 독립적으로 유지되도록 하세요:

```
rigRoot (entity)           -- 월드 위치 + 요 회전
└── rigCamera (child)      -- 피치 회전 + 궤도 오프셋
    └── VirtualCamera
```

`rigRoot` 는 요(좌우 회전)를 유지하며 팔로우 대상이나 자유 카메라 피벗을 향해 lerp됩니다. `rigCamera` 는 피치(위아래 기울기)와 루트로부터의 궤도 거리를 처리합니다. 요와 피치를 두 개의 Transform으로 나누면 오일러 계산이 단순해집니다.

### 활성화 및 비활성화

```ts
// 관전자 모드 활성화
const rigRoot = engine.addEntity()
Transform.create(rigRoot, {
  position: Vector3.create(8, 8, 8),
  rotation: Quaternion.fromEulerDegrees(0, 0, 0),
})

const rigCamera = engine.addEntity()
Transform.create(rigCamera, { parent: rigRoot })
VirtualCamera.create(rigCamera, {})

MainCamera.createOrReplace(engine.CameraEntity, { virtualCameraEntity: rigCamera })
InputModifier.createOrReplace(engine.PlayerEntity, {
  mode: InputModifier.Mode.Standard({ disableAll: true }),
})
```

```ts
// 관전자 모드 비활성화
// 중요: VirtualCamera 엔티티를 제거하기 전에 MainCamera를 먼저 지우세요.
// 먼저 엔티티를 제거하면 엔진이 죽은 엔티티에 계속 바인딩됩니다
// 그리고 시점이 플레이어의 발밑으로 떨어집니다.
const mainCamera = MainCamera.getMutableOrNull(engine.CameraEntity)
if (mainCamera) mainCamera.virtualCameraEntity = undefined

engine.removeEntity(rigCamera)
engine.removeEntity(rigRoot)

InputModifier.createOrReplace(engine.PlayerEntity, {
  mode: InputModifier.Mode.Standard({ disableAll: false }),
})
```


> [!CAUTION]
> **경고:** 항상 지우세요 `MainCamera.virtualCameraEntity` 카메라 엔티티를 제거하기 전에. 먼저 엔티티를 제거하면 엔진이 죽은 참조를 가리키게 되어 시점이 깨집니다.


### 카메라 경계

엔진은 `VirtualCamera` 씬의 구획 경계를 벗어나 이동한 엔티티를 비활성화합니다. 카메라가 씬 범위를 벗어나면 조용히 작동을 멈춥니다. 매 프레임마다 카메라 위치를 씬의 축 정렬 경계 상자(AABB) 안에 머물도록 작은 여유를 두고 제한하세요.

```ts
// 1x1 구획 씬(16m x 16m)에 대한 예시 경계
const BOUNDS_MIN = Vector3.create(0, 0, 0)
const BOUNDS_MAX = Vector3.create(16, 20, 16)
const BOUNDS_MARGIN = 0.5
```

더 큰 씬의 경우 경계를 `scene.json` 구획에 맞게 설정하세요. 씬 높이 제한은 구획 수와 관계없이 330미터입니다. 4x4 구획 씬은 다음을 사용할 수 있습니다 `Vector3.create(64, 330, 64)`.

### 플레이어 따라가기

다음을 사용하세요 `onEnterScene` 및 `onLeaveScene` 를 사용해 씬의 플레이어 실시간 명단을 만드세요. 그러면 플레이어는 키로 팔로우 대상을 순환할 수 있습니다(예: `IA_ACTION_3` 및 `IA_ACTION_4`). 플레이어를 따라갈 때, 리그 루트는 따라가는 플레이어의 Transform 위치를 향해 lerp되고, 자식 카메라는 설정 가능한 거리로 궤도 이동합니다.

```ts
import { onEnterScene, onLeaveScene } from '@dcl/sdk/src/players'

const playerEntities = new Map<string, Entity>()
let playerIds: string[] = []

onEnterScene((player) => {
  if (!player) return
  playerIds.push(player.userId)
  playerEntities.set(player.userId, player.entity)
})

onLeaveScene((userId) => {
  if (!userId) return
  playerIds = playerIds.filter((id) => id !== userId)
  playerEntities.delete(userId)
})
```

매 프레임 플레이어를 따라가려면, 리그 루트를 해당 위치 쪽으로 lerp하세요:

```ts
const followEntity = playerEntities.get(followTargetId)
if (followEntity) {
  const targetPos = Transform.get(followEntity).position
  const rootTransform = Transform.getMutable(rigRoot)
  rootTransform.position = Vector3.lerp(
    rootTransform.position,
    Vector3.add(targetPos, Vector3.create(0, 1, 0)),
    0.1
  )
}
```

다음을 참조하세요 [플레이어가 씬에 들어오거나 나갑니다](/docs/creator/content-creator-ko/sdk7/interactivity/event-listeners/#player-enters-or-leaves-scene) 에 대한 자세한 내용은 `onEnterScene` 및 `onLeaveScene`.

### 관전 중 마우스 시점 이동

읽기 `PrimaryPointerInfo.screenDelta` 각 프레임마다 마우스로 카메라를 회전합니다. 포인터가 잠겨 있는 동안에 작동합니다. 참고: [마우스 이동](/docs/creator/content-creator-ko/sdk7/interactivity/mouse-movement/) 에서 전체 마우스 룩 예시를 볼 수 있습니다.

```ts
const MOUSE_SENSITIVITY = 0.15 // degrees per pixel
let yaw = 0
let pitch = 45

function spectateMouseLook() {
  const isLocked = PointerLock.getOrNull(engine.CameraEntity)?.isPointerLocked ?? false
  if (!isLocked) return

  const delta = PrimaryPointerInfo.getOrNull(engine.RootEntity)?.screenDelta
  if (!delta) return

  yaw = (yaw + delta.x * MOUSE_SENSITIVITY) % 360
  // delta.y는 아래로 갈수록 커지므로, 마우스를 위로 움직이면 음수입니다: 이를 더하면 카메라가 위로 기울어집니다; 뒤집힘을 방지하기 위해 클램프합니다
  pitch = Math.max(-25, Math.min(80, pitch + delta.y * MOUSE_SENSITIVITY))
}
```


> [!WARNING]
> **참고:** `screenDelta` 데스크톱 전용입니다. 모바일에서는 항상 보고됩니다 `0`. 장면이 모바일을 대상으로 한다면, 터치 기반 대체 수단을 설계하세요.


### 전체 참조 구현

다음 [`33,20-spectate-mode`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/33,20-spectate-mode) 테스트 장면에는 다음 위치에 자체 포함된 관전 모듈이 있습니다: `src/spectate.ts`. 자유 카메라 및 추적 카메라 모드, WASD + 마우스 시점 조작, 플레이어 순환, 궤도 거리 줌, 경계 클램프, 그리고 조작법과 현재 추적 대상을 표시하는 화면상의 HUD를 지원합니다.

자신의 프로젝트에서 사용하려면:

1. 복사 `src/spectate.ts` 를 장면에 넣으세요.
2. 업데이트 `PIVOT`, `BOUNDS_MIN`, 그리고 `BOUNDS_MAX` 파일 상단에서 장면의 파셀에 맞게 수정하세요. 이것이 가장 흔한 통합 실수입니다.
3. 연결 `toggleSpectate()` 를 어떤 트리거에든 연결하세요: 클릭 가능한 엔터티, UI 버튼, 또는 키 입력.


> [!NOTE]
> **💡 팁**: 카메라 제어의 실제 예시는 다음을 참조하세요: [`2,22-virtual-cameras`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/2,22-virtual-cameras) 테스트 장면을 참조하세요. 이 장면은 여러 `VirtualCamera` 엔터티를 `MainCamera` 포함하여 `lookAtEntity` 플레이어를 향한 카메라와 트윈 처리된 카메라; [`32,20-virtual-camera-mouse-look`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/32,20-virtual-camera-mouse-look), 포인터가 잠겨 있는 동안 마우스 시점 카메라를 구동합니다; [`33,20-spectate-mode`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/33,20-spectate-mode), follow-cam 및 free-cam 모드를 갖춘 완전한 관전/관찰자 카메라를 구현합니다; 그리고 [`9,99-modifier-areas`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/9,99-modifier-areas), 특정 볼륨 내부에서 카메라 모드를 강제로 적용하는 `CameraModeArea`.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/3d/camera.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
