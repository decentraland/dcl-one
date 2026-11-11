> 씬에서 조명을 사용하는 방법을 알아봅니다

# 조명

조명은 3D 그래픽의 기본적인 요소입니다. 조명은 장면을 밝히고 깊이와 사실감을 만들어 내는 데 사용되며, 다양한 분위기와 무드를 연출하는 데도 사용할 수 있습니다.

기본적으로 장면은 하나의 방향성 조명으로 비춰집니다. 이는 특정 방향으로 빛을 비추는 조명으로, 태양이나 달을 시뮬레이션하는 데 사용됩니다. 자세한 내용은 [스카이박스 제어](/docs/creator/content-creator-ko/sdk7/interactivity/skybox-control/) 를 참조하세요.

장면의 각 parcel당 최대 1개의 조명을 추가할 수 있습니다.

지원되는 조명 유형은 두 가지입니다:

* 점광원: 특정 지점에서 모든 방향으로 빛을 비추는 조명입니다.
* 스포트라이트: 특정 방향으로 빛을 비추며 원뿔 모양의 영역만 비춥니다.


> [!WARNING]
> **📔 참고**: 다음이 포함된 `.glb` 또는 `.gltf` 파일 안에 내장된 조명은 Decentraland에서 무시됩니다. 3D 모델을 조명이 포함된 상태로 내보냈다면, 장면에서 아무것도 비추지 못합니다. 유효한 광원은 다음을 가진 엔티티뿐입니다. `LightSource` 컴포넌트입니다. 램프 모델이 빛나게 하려면 `LightSource` 를 같은 엔티티 또는 전구가 있는 위치에 배치된 자식 엔티티에 추가하세요.



> [!NOTE]
> **💡 팁**: Creator Hub 자산 카탈로그에는 미리 만들어진 **스포트라이트** 와 **점광원** 스마트 아이템이 **조명** 카테고리에 있습니다. 각각 모델, 설정된 `LightSource`컴포넌트, 그리고 켜기, 끄기, 토글 액션을 함께 제공하므로 코드를 작성할 필요가 없습니다. 또한 장식용 **스포트라이트** 은 **장식**카테고리에 있는데, 이는 모델만 제공하며 빛을 내지 않습니다. 실제 조명이 필요할 때는 **조명** 카테고리에서 선택하세요.


## 조명 추가하기

장면에 조명을 추가하려면 조명 엔티티를 만들고 여기에 `LightSource` 컴포넌트를 추가해야 합니다.

```ts
import { engine, LightSource } from '@dcl/sdk/ecs'

const light = engine.addEntity()

Transform.create(light, {
  position: Vector3.create(10, 3, 10),
})

LightSource.create(light, {
  type: LightSource.Type.Point({})
})
```


> [!WARNING]
> **📔 참고**: 기본 밝기의 조명은 정오의 태양 아래에서는 실제 세계처럼 거의 보이지 않습니다. 다음을 사용해 [스카이박스 제어](/docs/creator/content-creator-ko/sdk7/interactivity/skybox-control/) 스카이박스를 밤 시간대로 강제 설정하거나, 밝기를 높이기 위해 `intensity`  속성의 `LightSource` 값을 더 높게 설정할 수 있습니다.


## 스포트라이트

스포트라이트는 특정 방향으로 빛을 비추며 특정 원뿔형 영역을 덮는 조명입니다. 조명의 방향은 엔티티의 Transform 컴포넌트로 정의되며, 조명은 엔티티의 전방 방향(로컬 +Z 축)을 따라 비춥니다. 원뿔의 개구각은 다음으로 정의됩니다. `innerAngle` 와 `outerAngle` 의 `LightSource` 컴포넌트입니다.

아래 예제는 엔티티를 X축을 기준으로 90도 회전시켜 조명이 바로 아래를 향하게 합니다. -90도 회전시키면 하늘을 향합니다.

```ts
import { engine, LightSource } from '@dcl/sdk/ecs'

const light = engine.addEntity()

Transform.create(light, {
  position: Vector3.create(10, 3, 10),
  rotation: Quaternion.fromEulerDegrees(90, 0, 0),
})

LightSource.create(light, {
	type: LightSource.Type.Spot({
      innerAngle: 30,
      outerAngle: 60
    }),
	shadow: true
})
```

다음 `innerAngle` 는 내부 원뿔의 각도로, 이 영역에서는 조명이 최대 밝기입니다. 그리고 `outerAngle` 는 외부 원뿔의 각도로, 이 영역에서는 조명이 원뿔의 가장자리 쪽으로 점차 사라집니다. 이 값들을 조정해 더 집중된 조명이나 더 확산된 조명 같은 다양한 효과를 만들 수 있습니다.

## 강도와 색상

모든 조명, 즉 점광원과 스포트라이트 모두 색상과 강도를 가집니다. 색상은 다음으로 정의되고, `색상`  속성의 `LightSource` 컴포넌트로, 강도는 다음으로 정의됩니다. `intensity` 속성입니다.

```ts
import { engine, LightSource } from '@dcl/sdk/ecs'

const light = engine.addEntity()

Transform.create(light, {
  position: Vector3.create(10, 3, 10),
})

LightSource.create(light, {
  type: LightSource.Type.Point({}),
  color: Color3.Red(),
  intensity: 10000,
})
```

색상은 `Color3` 객체이며, 지정하지 않으면 흰색이 됩니다. 원하는 어떤 색이든 설정할 수 있으며, 이는 장면의 분위기에 큰 영향을 줄 수 있습니다.

강도는 칸델라 단위로 표현됩니다(1m 거리에서의 루멘/m^2, 또는 루멘을 4\*pi로 나눈 값).

기본 강도는 16000이며, 이는 실제 세계의 평균적인 전구 밝기이고 광원으로부터 약 10미터 떨어진 곳까지 보일 수 있습니다. 더 멀리서 또는 낮에도 조명이 보이게 하려면 강도를 높일 수 있습니다.

조명이 보이는 거리는 강도 값의 네제곱근(`intensity^0.25`).

* )입니다.
* 강도 10000에서는 조명이 약 10미터 거리까지 보입니다.
* 강도 160000에서는 조명이 약 20미터 거리까지 보입니다.

## 그림자

각 조명은 그림자를 드리울 수도 있고 그렇지 않을 수도 있습니다. 기본적으로는 드리우지 않지만, 다음을 설정하여 활성화할 수 있습니다. `shadow`  속성의 `LightSource` 컴포넌트를 `true`.

```ts
import { engine, LightSource } from '@dcl/sdk/ecs'

const light = engine.addEntity()

Transform.create(light, {
  position: Vector3.create(10, 3, 10),
  rotation: Quaternion.fromEulerDegrees(90, 0, 0),
})

LightSource.create(light, {
	type: LightSource.Type.Spot({
      innerAngle: 30,
      outerAngle: 60
    }),
	shadow: true
})
```


> [!WARNING]
> **📔 참고**: 그림자는 스포트라이트에서만 지원됩니다. 점광원은 그림자를 지원하지 않습니다. 장면에 여러 조명이 있으면, 그중 일부는 그림자를 드리우지 않을 수 있습니다. 다음을 참조하세요. [조명 최적화](#light-optimization) 를 참조하세요.


## 조명 켜기와 끄기

LightSource 컴포넌트에는 `active` 속성이 있어 조명을 켜고 끌 수 있습니다. 이는 조명을 장면에서 제거하지 않고 끄고 싶거나, 다음을 `intensity` 0으로 설정해 원래 강도가 얼마였는지에 대한 기준을 잃지 않고 싶을 때 유용합니다.

```ts
import { engine, LightSource } from '@dcl/sdk/ecs'

const light = engine.addEntity()

Transform.create(light, {
  position: Vector3.create(10, 3, 10),
  rotation: Quaternion.fromEulerDegrees(90, 0, 0),
})

LightSource.create(light, {
	type: LightSource.Type.Spot({
      innerAngle: 30,
      outerAngle: 60
    }),
	shadow: true,
	active: true
})

const lightSwitch = engine.addEntity()

Transform.create(lightSwitch, {
  position: Vector3.create(8, 1, 10),
})

MeshRenderer.setBox(lightSwitch)

MeshCollider.setBox(lightSwitch)

pointerEventsSystem.onPointerDown(
	{
		entity: lightSwitch,
		opts: {
			button: InputAction.IA_POINTER,
			hoverText: '클릭',
		},
	},
	function () {
		const lightSource = LightSource.getMutable(light)
		lightSource.active = !lightSource.active
	}
)
```

## 조명 최적화

광원은 장면의 성능에 꽤 큰 영향을 줄 수 있습니다. 이 때문에 엔진은 더 멀리 있는 조명부터 시작해 일부 조명이나 그 그림자를 자동으로 비활성화하여 장면을 최적화합니다.

장면에서 허용되는 활성 조명의 수는 parcel당 최대 1개로 제한되며, 그 이후에는 사용자가 선택한 품질 설정에 따라 달라집니다.

* 낮음 품질: 장면 조명 없음
* 중간 품질: 최대 6개 조명(충분한 parcel이 있는 장면에서)
* 높음 품질: 최대 10개 조명(충분한 parcel이 있는 장면에서)
* 모바일: 앱에는 자체 프리셋이 있으며, Very Low에서는 장면 조명 없음부터 High에서는 최대 8개 조명까지입니다. 다음을 참조하세요. [모바일에서의 조명과 파티클](/docs/creator/content-creator-ko/build-for-mobile/develop/optimize-performance/#lights-and-particles).

품질 설정 슬라이더는 프리셋의 상한을 더 낮출 수도 있습니다.

허용된 조명보다 더 많은 조명이 있으면, 엔진은 광원과 플레이어 간의 근접성에 따라 조명을 자동으로 비활성화합니다. 플레이어가 이동하면 엔진은 플레이어와 충분히 가까운 조명을 다시 활성화합니다.


> [!WARNING]
> **📔 참고**: 낮음 품질 프리셋에서는 장면 조명이 완전히 꺼집니다. 올바르게 빌드된 `LightSource` 장면은 아무것도 보여주지 않으며, 강도, 범위 또는 재질 조정을 아무리 해도 달라지지 않습니다. 조명이 "작동하지 않는" 것처럼 보이면, 무엇보다 먼저 품질 설정을 확인하세요. 장면이 지원하는 최저 설정으로 중간 품질을 사용한다고 생각하고, High를 요구하지 마세요.
> 
> 빛나는 발광 재질이 있다고 해서 조명이 작동한다는 증거는 아닙니다. 빛나는 램프 메쉬와 그 조명이 주변 표면에 비추는 빛은 확인해야 할 서로 다른 두 가지입니다.


일부 플레이어는 장면 조명을 전혀 보지 못하므로(데스크톱에서는 낮음 품질, 모바일에서는 Very Low), 조명이 없어도 장면이 여전히 작동하는지 확인하세요. 앞으로 가는 길이나 상호작용할 수 있는 물체처럼 플레이어가 봐야 할 것은 발광 재질, 텍스처 또는 스카이박스를 통해 두드러지게 보여야 합니다. 다음을 사용하세요. `LightSource` 를 추가적인 수단으로 사용하고, 유일한 단서로 사용하지 마세요. 그림자도 마찬가지입니다. 물체의 위치를 그림자만으로 판단하게 만들지 마세요.

모든 경우에 엔진은 최대 3개의 광원에 대해서만 그림자를 렌더링합니다. 그림자를 가진 조명이 3개보다 많으면, 엔진은 더 멀리 있는 나머지 조명들의 그림자를 자동으로 비활성화합니다.

허용되는 최대 조명 수 외에도, 그림자는 플레이어와의 거리에도 영향을 받습니다. 정확한 거리는 조명 유형과 플레이어의 품질 설정에 따라 다르지만, 일반적으로는 다음과 같습니다:

* 10미터 미만: 그림자가 부드러운 그림자로 렌더링됩니다(고품질)
* 10미터와 20미터 사이: 그림자가 단단한 그림자로 렌더링됩니다(저품질)
* 20미터 초과: 그림자가 렌더링되지 않음

광원 자체는 훨씬 더 먼 거리에서도 장면을 계속 비춥니다. 플레이어가 160미터(10 parcel)보다 멀어져야만 비활성화됩니다. 따라서 조명은 대부분의 관객이 광원에서 멀리 떨어져 있는 라이브 이벤트의 무대 조명 같은 대규모 설정에 적합합니다.

또한 플레이어가 장면 안에 서 있을 때만 조명이 렌더링된다는 점도 중요합니다. 플레이어가 장면 밖에 있으면 조명은 렌더링되지 않습니다.

## 조명 범위

lightSource 컴포넌트에는 `범위` 속성이 있어 조명이 보이는 최대 거리를 설정할 수 있습니다. 기본적으로 `범위` 속성의 값은 -1이며, 이는 조명 범위가 조명의 강도에 따라 달라진다는 뜻입니다.

범위는 강도 값의 네제곱근으로 계산됩니다(`intensity^0.25`).

* 강도 16000에서는 범위가 약 11미터입니다.
* 강도 160000에서는 범위가 약 20미터입니다.
* 강도 1600000에서는 범위가 약 36미터입니다.

기본 설정은 감쇠 곡선이 부드럽고 자연스럽게 보이도록 보장합니다. 하지만 조명의 범위를 제한하고 싶다면, 다음의 `범위` 속성을 양수로 설정할 수 있습니다.

```ts
import { engine, LightSource } from '@dcl/sdk/ecs'

const light = engine.addEntity()

Transform.create(light, {
  position: Vector3.create(10, 3, 10),
})

LightSource.create(light, {
  type: LightSource.Type.Point({}),
  intensity: 16000,
  range: 20,
})
```


> [!WARNING]
> **📔 참고**: 다음을 `범위` 속성을 양수로 설정하면 제공된 거리에서 조명이 갑자기 잘립니다. 특정 영역에서만 보이는 조명을 만들거나 장면 성능을 최적화하려는 경우에 유용합니다. 다음을 `범위` 조명의 현재 강도로 실제 도달할 수 있는 거리보다 더 큰 값으로 설정해도 아무 효과가 없습니다.


## 조명 마스크

조명 마스크를 사용하면 흥미로운 효과를 만들 수 있습니다. 전체 영역을 비추는 대신, 텍스처를 필터로 적용해 영역의 일부만 비추도록 할 수 있습니다.

마스크는 일반적으로 스포트라이트와 함께 사용되지만, 점광원과도 함께 사용할 수 있습니다. 아래는 마스크가 적용된 스포트라이트의 예입니다.

```ts
import { engine, LightSource } from '@dcl/sdk/ecs'

const light = engine.addEntity()

Transform.create(light, {
  position: Vector3.create(10, 3, 10),
})

LightSource.create(light, {
	type: LightSource.Type.Spot({
      innerAngle: 30,
      outerAngle: 60
    }),
	shadow: true,
	shadowMaskTexture: Material.Texture.Common({src: "assets/scene/images/lightmask1.png"})
})
```

예를 들어, 아래 이미지를 적용하면 텍스처가 흰색인 부분에서만 조명이 보이는 재미있는 효과를 만들 수 있습니다. 이 기능은 어떤 색상과 강도의 조명에도 사용할 수 있습니다.

![](/docs/_remote/b2cd4d8e15d52341e551dffb.webp)

흑백 이미지를 사용하면 조명은 해당 영역에 닿거나 닿지 않거나 둘 중 하나입니다. 색상이 있는 이미지를 전달하는 것도 가능하며, 이 색상들이 각 영역에서 조명에 색조를 입혀 표면에 컬러 로고나 이미지를 투영하는 데 사용할 수 있습니다.


> [!WARNING]
> **📔 참고**: 마스크로 사용되는 이미지는 픽셀 높이와 너비가 2의 거듭제곱이어야 합니다(예: 1024, 512, 256). 이 기능은 크기가 다른 이미지에서는 작동하지 않습니다.


점광원에 마스크를 적용하면 텍스처가 광원 주위를 둘러싼 큐브 형태로 래핑됩니다. 큐브의 각 면 사이에 보이는 경계를 피하려면 텍스처의 가장자리에 연속성이 있도록 하세요.

```ts
import { engine, LightSource } from '@dcl/sdk/ecs'

const light = engine.addEntity()

Transform.create(light, {
  position: Vector3.create(10, 3, 10),
})

LightSource.create(light, {
	type: LightSource.Type.Point({}),
	shadowMaskTexture: Material.Texture.Common({src: "assets/scene/images/point-light-mask1.png"})
})
```

예를 들어, 아래 이미지는 큐브의 각 면에 각 글자를 표시합니다(Y는 위, -Y는 아래, X는 오른쪽, -X는 왼쪽, Z는 앞, -Z는 뒤).

![](/docs/_remote/384393f9ee871b7e47ed3a80.webp)


> [!NOTE]
> **💡 팁**: 조명을 [파티클 시스템](/docs/creator/content-creator-ko/sdk7/3d/particle-system/) 과 결합하면 더 풍부한 시각 효과를 만들 수 있습니다. 예를 들어, 가산 혼합 화염 파티클 시스템과 따뜻한 점광원을 함께 사용하면 그럴듯한 캠프파이어를 만들 수 있습니다.



> [!NOTE]
> **💡 팁**: 이 컴포넌트의 동작 예시는 다음을 참조하세요. [`0,4-dynamic-lights`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,4-dynamic-lights) 테스트 장면입니다. 여기에는 다음이 모두 포함됩니다. `LightSource.Type.Point` 와 `LightSource.Type.Spot`, 런타임 변경은 다음을 통해 `LightSource.getMutable()`, 그림자, 그리고 `shadowMaskTexture`.


## 어둠 속의 조명

하늘이 조명과 경쟁하지 않을 때 조명을 가장 잘 감상할 수 있습니다. `Skybox`, `Sun` 와 `EnvironmentFog` 컴포넌트는 햇빛, 하늘 색상(그리고 그에 따른 주변광), 안개, 구름, 별을 모두 어둡게 만들어 조명이 유일한 광원이 되게 할 수 있습니다. 다음을 참조하세요. [완전한 어둠](/docs/creator/content-creator-ko/sdk7/interactivity/skybox-control/#complete-darkness).

낮음 품질 프리셋에서는 장면 조명이 전혀 렌더링되지 않으므로, 완전히 어두운 장면은 해당 플레이어에게 완전히 검게 보입니다. 발광 재질을 사용해 중요한 요소가 보이도록 하세요. 모바일 앱과 Bevy 익스플로러는 아직 이러한 컴포넌트를 지원하지 않으므로, 해당 클라이언트의 플레이어는 계속 일반 하늘을 보게 됩니다.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/3d/lights.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
