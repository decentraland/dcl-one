# 파티클 시스템

불, 비, 불꽃, 마법 오라 같은 파티클 효과를 씬에 추가합니다

파티클 시스템을 사용하면 수많은 작은 스프라이트를 방출하고 애니메이션하여 역동적인 시각 효과를 만들 수 있습니다. 불, 연기, 비, 눈, 불꽃, 마법 오라, 폭발 등 정적 메시로는 구현하기 비현실적인 다양한 효과를 만드는 데 사용하세요.

## 파티클 시스템 추가하기

씬에 파티클 시스템을 추가하려면 엔티티를 만들고 `ParticleSystem` 컴포넌트를 추가해야 합니다.

```ts
import { engine, Transform, ParticleSystem } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

const emitter = engine.addEntity()

Transform.create(emitter, {
	position: Vector3.create(8, 1, 8),
})

ParticleSystem.create(emitter, {})
```

속성을 아무것도 설정하지 않으면, 컴포넌트는 모든 기본값을 사용합니다: 초당 10개의 파티클을 발사하는 점 방출기이며, 파티클 수명은 5초입니다.

## 방출기 모양

방출기 모양은 파티클이 어디에서 생성되는지를 결정합니다. 다음의 `ParticleSystem.Shape` 헬퍼를 통해 네 가지 모양을 사용할 수 있습니다.

### 포인트

파티클은 단일 점(엔티티의 위치)에서 생성됩니다. 모양을 지정하지 않으면 이것이 기본값입니다.

```ts
ParticleSystem.create(emitter, {
	shape: ParticleSystem.Shape.Point(),
})
```

### 스피어

파티클은 구의 표면 또는 내부에서 생성됩니다.

```ts
ParticleSystem.create(emitter, {
	shape: ParticleSystem.Shape.Sphere({ radius: 2 }),
})
```

### 원뿔

파티클은 원뿔의 밑면에서 생성되어 원뿔의 방향으로 바깥쪽으로 이동합니다. `angle` 은(는) 반각을 도 단위로 나타냅니다; `반경` 은(는) 밑면 반지름을 미터 단위로 나타냅니다. 원뿔의 방향을 설정하려면 엔티티의 `Transform`.

```ts
ParticleSystem.create(emitter, {
	shape: ParticleSystem.Shape.Cone({ angle: 25, radius: 1 }),
})
```

### 박스

파티클은 상자 볼륨 내부의 어디에서나 생성됩니다.

```ts
ParticleSystem.create(emitter, {
	shape: ParticleSystem.Shape.Box({ size: Vector3.create(3, 1, 3) }),
})
```

## 방출 속성

생성되는 파티클 수와 수명을 제어합니다.

| 속성             | 기본값    | 설명                                |
| -------------- | ------ | --------------------------------- |
| `rate`         | `10`   | 초당 방출되는 파티클 수(연속).                |
| `maxParticles` | `1000` | 동시에 살아 있을 수 있는 파티클의 최대 한도.        |
| `lifetime`     | `5`    | 각 파티클의 수명(초).                     |
| `active`       | `true` | 시스템이 현재 새 파티클을 적극적으로 방출하는지 여부입니다. |

```ts
ParticleSystem.create(emitter, {
	rate: 50,
	maxParticles: 500,
	lifetime: 3,
})
```

## 동작

### 중력

해당 `gravity` 속성은 씬의 중력(-9.81 m/s²)에 적용되는 배율입니다. 이를 `0` 로 설정하면 파티클이 제자리에서 떠 있게 되고, 음수 값은 위로 밀어 올리며, 양수 값은 아래로 가속합니다.

```ts
ParticleSystem.create(emitter, {
	gravity: -0.5, // particles float slowly upward
})
```

### 초기 속도

`initialVelocitySpeed` 은(는) `FloatRange` 이며, 각 파티클이 발사될 때의 무작위 속도 범위의 최소값과 최대값을 설정합니다(단위: m/s). 두 값의 기본값은 `1`.

```ts
ParticleSystem.create(emitter, {
	initialVelocitySpeed: { start: 2, end: 8 },
})
```

### 추가 힘

중력 외에 매 프레임 모든 파티클에 적용되는 일정한 힘 벡터입니다. 바람이나 자기 효과에 유용합니다.

```ts
ParticleSystem.create(emitter, {
	additionalForce: Vector3.create(0.5, 0, 0), // constant sideways push
})
```

### 속도 제한

다음을 사용하세요 `limitVelocity` 를 사용하면 파티클의 최대 속도를 제한하고, 필요에 따라 매 프레임 초과 속도를 감쇠할 수 있습니다.

```ts
ParticleSystem.create(emitter, {
	limitVelocity: {
		speed: 5, // 최대 허용 속도(m/s)
		dampen: 0.8, // 프레임마다 제거되는 초과 속도의 비율
	},
})
```

## 시각 속성

### 색상

`initialColor` 는 주어진 범위 내에서 무작위로 선택된, 생성 시의 파티클 색상을 설정합니다. `colorOverTime` 는 파티클의 색상이 `시작` 다음에 `끝` 값에서 파티클 수명 동안 점진적으로 변하도록 합니다. 둘 다 `ColorRange` 를 사용하세요 `시작` 및 `끝` 값.

```ts
import { Color4 } from '@dcl/sdk/math'

ParticleSystem.create(emitter, {
	initialColor: {
		start: Color4.create(1, 0.5, 0, 1), // 주황색
		end: Color4.create(1, 1, 0, 1), // 노란색
	},
	colorOverTime: {
		start: Color4.create(0.5, 0, 0, 0.5), // 어두운 빨간색, 반투명
		end: Color4.create(0, 0, 0, 0), // 완전 투명(서서히 사라짐)
	},
})
```

다음 사항에 유의하세요:  `Color4` 에서 네 번째 값은  *알파*알파입니다. 최종 색상을 알파가 0인 값으로 설정하면 파티클이 점차 사라져 보이지 않게 되며, 이는 종종 좋은 효과입니다.

### 크기

`initialSize` 및 `sizeOverTime` 은 `FloatRange` 값을 참조하세요. `initialSize` 는 주어진 범위 내에서 무작위로 선택된, 생성 시의 파티클 크기를 제어합니다. `sizeOverTime` 는 파티클의 크기가 `시작` 다음에 `끝` 값에서 파티클 수명 동안 점진적으로 변하도록 합니다. 값이 `1` 이면 원본 텍스처 크기와 같습니다.

```ts
ParticleSystem.create(emitter, {
	initialSize: { start: 0.1, end: 0.3 },
	sizeOverTime: { start: 0.5, end: 1.0 }, // 수명 동안 커짐
})
```


> [!WARNING]
> **📔 참고**: `크기` 엔티티의 `Transform` 은(는) 파티클의 크기에 영향을 주지 않습니다.


### 텍스처

기본적으로 파티클은 흰색 사각형으로 렌더링됩니다. 사용자 지정 이미지를 사용하려면 텍스처를 제공하세요.

```ts
ParticleSystem.create(emitter, {
	texture: { src: 'assets/scene/Images/spark.png' },
})
```

### 블렌드 모드

파티클 색상이 뒤에 있는 씬과 어떻게 합성되는지 제어합니다.

| 값                                      | 설명                                           |
| -------------------------------------- | -------------------------------------------- |
| `ParticleSystemBlendMode.PSB_ALPHA`    | 표준 투명도(기본값).                                 |
| `ParticleSystemBlendMode.PSB_ADD`      | 가산 블렌딩 — 파티클이 씬을 더 밝게 만듭니다. 불과 발광 효과에 적합합니다. |
| `ParticleSystemBlendMode.PSB_MULTIPLY` | 파티클 색상에 뒤에 있는 씬을 곱합니다.                       |

```ts
import { ParticleSystemBlendMode } from '@dcl/sdk/ecs'

ParticleSystem.create(emitter, {
	texture: { src: 'assets/scene/textures/ember.png' },
	blendMode: ParticleSystemBlendMode.PSB_ADD,
})
```

### 빌보드

텍스처 사용이 `billboard` 가 `true` (기본값)인 경우, 방출기의 방향과 관계없이 각 파티클은 항상 카메라를 향합니다. 3D 공간에서 회전해야 하는 파티클에는 이를 `false` 로 설정하세요.

```ts
ParticleSystem.create(emitter, {
	billboard: false,
})
```

## 회전

### 초기 회전과 시간에 따른 회전

`initialRotation` 은(는) 파티클이 생성될 때의 방향입니다. `rotationOverTime` 은(는) 매초 적용되는 축별 각속도입니다. 둘 다 `Quaternion`.

```ts
import { Quaternion } from '@dcl/sdk/math'

ParticleSystem.create(emitter, {
	initialRotation: Quaternion.fromEulerDegrees(0, 0, 45),
	rotationOverTime: Quaternion.fromEulerDegrees(0, 0, 90), // Z축으로 초당 90° 회전
})
```


> [!WARNING]
> **📔 참고**: 만약 `billboard` 가 `true`로 설정하면 파티클은 한 축으로만 회전하며, 항상 카메라를 향한 방향을 유지합니다.


### 이동 방향을 향함

텍스처 사용이 `faceTravelDirection` 가 `true`이면 각 파티클은 소행성이나 공중을 떨어지는 나뭇잎처럼 이동 방향을 향하도록 자동으로 회전합니다.

```ts
ParticleSystem.create(emitter, {
	faceTravelDirection: true,
	billboard: false,
})
```


> [!WARNING]
> **📔 참고**: 만약 `faceTravelDirection` 가 true이면 `billboard` 의 값은 무시됩니다.


## 스프라이트 시트 애니메이션

텍스처를 애니메이션 프레임의 그리드로 취급하여 파티클을 애니메이션할 수 있습니다. 시트의 열과 행 수, 그리고 재생 속도를 지정하세요.

```ts
ParticleSystem.create(emitter, {
	texture: { src: 'assets/scene/textures/flame-sheet.png' },
	spriteSheet: {
		tilesX: 4, // 4열
		tilesY: 3, // 3행(총 12프레임)
		framesPerSecond: 12,
	},
})
```

## 재생 제어

파티클 시스템은 다음이 제어하는 세 가지 상태 중 하나가 될 수 있습니다. `playbackState`.

| 값                                        | 설명                                |
| ---------------------------------------- | --------------------------------- |
| `ParticleSystemPlaybackState.PS_PLAYING` | 활성 방출 중(기본값).                     |
| `ParticleSystemPlaybackState.PS_PAUSED`  | 현재의 모든 파티클을 그 자리에 고정하고 방출을 중지합니다. |
| `ParticleSystemPlaybackState.PS_STOPPED` | 방출을 중지하고 기존의 모든 파티클을 제거합니다.       |

```ts
import { ParticleSystemPlaybackState } from '@dcl/sdk/ecs'

// 플레이어가 멀어지면 일시 정지
ParticleSystem.getMutable(emitter).playbackState =
	ParticleSystemPlaybackState.PS_PAUSED

// 다시 시작
ParticleSystem.getMutable(emitter).playbackState =
	ParticleSystemPlaybackState.PS_PLAYING
```

### 루프 및 사전 워밍

기본적으로 시스템은 무한히 반복됩니다. 모든 파티클이 사라진 뒤 자동으로 중지되는 한 번만 재생되는 효과를 원하면 `loop` 을(를) `false` 로 설정하세요.

설정하세요 `prewarm` 을(를) `true` (다음이 필요합니다: `loop: true`) 시스템이 처음부터 실행된 것처럼 시뮬레이션하여, 플레이어가 처음 볼 때 이미 파티클이 씬을 채우도록 합니다.

```ts
ParticleSystem.create(emitter, {
	loop: false, // 한 번 재생 후 중지
	prewarm: false,
})
```

## 버스트 방출

버스트를 사용하면 일정한 속도 대신 특정 순간에 많은 파티클을 방출할 수 있습니다. 폭발, 불꽃놀이 또는 기타 일회성 이벤트에 유용합니다.

하나의 파티클 시스템은 여러 버스트를 가진 주기를 거칠 수 있으며, 간격이나 확률을 다르게 하여 더 자연스럽게 보이게 할 수도 있습니다.

```ts
ParticleSystem.create(emitter, {
	rate: 0, // 연속 방출 비활성화
	bursts: {
		values: [
			{
				time: 0, // 재생 시작 후 시간(초)
				count: 200, // 각 버스트에서 방출할 파티클 수
				cycles: 1, // 반복할 횟수(0 = 무한)
				interval: 0.2, // 반복 사이클 사이의 초
				probability: 1, // 각 사이클마다 버스트가 발생할 확률(0~1)
			},
		],
	},
})
```

간격을 두고 버스트가 발생하는 루프형 불꽃놀이 효과의 예:

```ts
ParticleSystem.create(emitter, {
	loop: true,
	rate: 0,
	lifetime: 2,
	bursts: {
		values: [
			{ time: 0.0, count: 80, cycles: 0, interval: 3 },
			{ time: 0.7, count: 100, cycles: 0, interval: 3 },
			{ time: 1.4, count: 60, cycles: 0, interval: 3 },
		],
	},
})
```

## 시뮬레이션 공간

파티클이 방출기에 상대적으로 움직이는지(`PSS_LOCAL`, 기본값) 또는 생성 후 월드 좌표에 고정된 채로 남는지(`PSS_WORLD`).

다음을 사용하세요 `PSS_WORLD` 를 제어합니다. 움직이는 방출기에서 뒤에 흔적을 남겨야 할 때(예: 소행성이나 로켓)에 유용합니다.

```ts
import { PBParticleSystem_SimulationSpace } from '@dcl/sdk/ecs'

ParticleSystem.create(emitter, {
	simulationSpace: PBParticleSystem_SimulationSpace.PSS_WORLD,
})
```

## 성능

엔진은 씬당 파티클 예산을 적용하며, 총 파티클 수가 한도를 초과할 경우 씬 내 모든 활성 파티클 시스템의 방출 속도를 자동으로 줄입니다. 씬을 다음과 같이 계획하세요:

* 많은 저효과 시스템보다, 적지만 시각적으로 임팩트 있는 시스템을 우선하세요.
* 다음을 사용하세요 `maxParticles` 개별 시스템을 제한하려면
* 다음을 사용하세요 `active` 또는 `playbackState` 화면 밖이거나 범위를 벗어난 시스템을 비활성화하려면
* 짧은 `lifetime` 값은 높은 방출 `rate` 만으로 예상되는 것보다 살아 있는 파티클 수를 더 낮게 유지합니다.

엔진은 한 번에 렌더링되는 최대 파티클 수를 1000개로 제한합니다. 이보다 더 많은 파티클을 방출하고 있다면, 모두 보이지 않을 수 있습니다.

또한 플레이어가 씬 안에 서 있을 때에만 파티클을 볼 수 있다는 점을 기억하세요. 밖에서 씬을 바라보는 플레이어는 안으로 들어오기 전까지 어떤 파티클도 볼 수 없습니다.

## 파티클 랩

월드에 들어가기 [ParticleLab.dcl.eth](decentraland://?realm=particlelab.dcl.eth\&dclenv=org) 를 사용해 다양한 파티클 시스템을 실험해 보세요. 파티클 시스템에 가까이 가면 UI에 사용 가능한 모든 필드가 표시되며, 씬을 다시 로드하지 않고도 실시간으로 조정할 수 있습니다. 만족스러우면 **복사** 버튼을 클릭하여 해당 파티클 시스템의 코드를 클립보드에 복사하세요.


> [!NOTE]
> **💡 팁**: 모든  `ParticleSystem` 필드를 다루는 동작 예시를 보려면 [`0,7-particle-system`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,7-particle-system) 테스트 씬을 확인하세요. 네 가지 방출기 모양 모두와 `PSB_ADD`/`PSB_ALPHA`/`PSB_MULTIPLY` 블렌드 모드, `PS_PLAYING`/`PS_PAUSED`/`PS_STOPPED` 재생 상태, 스프라이트 시트, 연속 `rate` 과 한 번만 재생되는 및 루프형 `버스트`, 그리고 `PSS_LOCAL` 와 `PSS_WORLD` 움직이는 방출기에서의 동작을 다룹니다.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/3d/particle-system.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
