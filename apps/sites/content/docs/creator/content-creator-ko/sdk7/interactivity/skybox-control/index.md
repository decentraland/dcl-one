> 스카이박스 시간을 변경하고, 하늘을 자신만의 텍스처로 바꾸거나, 하늘, 구름, 별의 색을 변경하고 태양과 안개를 제어하세요

# 스카이박스 제어

플레이어가 당신의 씬에 서 있을 때 보이는 스카이박스의 모습을 바꿀 수 있으며, 이는 전역 조명의 색조와 방향에도 영향을 줍니다.

디센트럴랜드의 하늘은 완료하는 데 2시간이 걸리는 기본 낮/밤 주기를 따르므로, 실제 하루마다 12번의 완전한 주기가 있습니다. 씬이 고정된 시간대를 강제하지 않는다면, 플레이어는 UI의 슬라이더를 바꿔 특정 시간대로 전환할 수도 있습니다.

플레이어가 다른 시간대의 씬에 들어가거나 씬이 시간대를 동적으로 변경하면, 스카이박스는 몇 초에 걸쳐 새 값으로 부드럽게 전환됩니다.

## 고정 시간대

씬에 고정된 시간대를 설정할 수 있습니다. 모든 플레이어는 이 시간대로 씬을 보게 되며, 스카이박스는 낮/밤 주기를 따르지 않습니다.

Creator Hub에서 씬 설정을 열고 다음을 클릭하세요 **설정** 탭에서 다음을 찾으세요 **스카이박스** 섹션을 찾으세요. 다음의 선택을 해제하세요 **자동** 옵션을 선택하고 원하는 시간대를 설정하세요.

![](/docs/_remote/6242ab0073574f8cc2c9e7d9.webp)

씬 코드에서도 스카이박스 시간대를 설정할 수 있습니다. 이렇게 하려면 다음 섹션을 당신의 코드에 추가하세요 `scene.json` 루트 수준에서:

```json
 "skyboxConfig": {
    "fixedTime": 36000
  }
```

이 숫자는 하루가 시작된 후 경과한 초 수를 의미하며, 범위는 0(이는 다음을 의미함 *00:00*)부터 86400(이는 다음을 의미함 *24:00*)까지입니다. 86400보다 큰 숫자는 모두 자정으로 해석됩니다.

유효한 값의 추가 예시는 다음과 같습니다:

* 0초 => *00:00*
* 21600초 => *06:00*
* 43200초 => *12:00*
* 64800초 => *18:00*
* 86400초 => *24:00*

## 시간대 읽기

다음을 사용하여 씬 코드에서 시간대를 읽을 수 있습니다 `getWorldTime()` 함수.

```ts
import { getWorldTime } from '~system/Runtime'

executeTask(async () => {
  let time = await getWorldTime({})
  console.log(time.seconds)
})
```

이 함수는 0에서 86400 사이의 숫자를 반환하며, 0은 자정이고 86400은 24:00입니다. 씬이 시간대를 동적으로 변경하거나 플레이어가 UI에서 시간대를 변경하면 이 값은 업데이트됩니다. 그렇지 않으면 기본 낮/밤 주기에 따른 값을 반환합니다.

## 시간대를 동적으로 변경하기

다음을 사용해 시간대를 동적으로 변경할 수 있습니다 `SkyboxTime` 컴포넌트입니다. 이 컴포넌트는 씬의 루트 엔티티에만 추가할 수 있습니다 `engine.RootEntity`.

```ts
import { SkyboxTime } from '@dcl/sdk/ecs'

function main() {
  SkyboxTime.create(engine.RootEntity, { fixedTime: 36000 })
}
```

다음 `fixedTime` 속성은 0에서 86400 사이의 숫자이며, 0은 자정이고 86400은 24:00입니다. 86400보다 큰 숫자는 모두 자정으로 해석됩니다.

이 컴포넌트가 추가되거나 제거되거나, 또는 `fixedTime` 속성이 변경되면 스카이박스의 시간대는 몇 초에 걸쳐 새 값으로 부드럽게 전환됩니다. 플레이어가 씬 밖으로 나가거나 씬 안으로 들어올 때도 같은 일이 발생합니다. 스카이박스 시간대가 고정되어 있는 동안에는 스카이박스가 더 이상 낮/밤 주기의 진행을 따르지 않으며, 플레이어는 UI를 통해 시간대를 변경할 수 없습니다.

기본적으로 전환은 항상 정방향으로 일어나지만, 다음을 설정하여 이를 바꿀 수 있습니다 `transitionMode` 속성을 다음으로 `TransitionMode.TM_FORWARD` 또는 `TransitionMode.TM_BACKWARD`.

```ts
import { SkyboxTime, TransitionMode } from '@dcl/sdk/ecs'

function main() {
  SkyboxTime.create(engine.RootEntity, { fixedTime: 36000, transitionMode: TransitionMode.TM_BACKWARD })
}
```

### 더 짧은 전환을 선택하세요

아무것도 갑자기 튀지 않습니다. 모든 변경은 그 사이의 모든 시간을 순차적으로 지나갑니다. `TM_FORWARD` 항상 정방향으로 진행되며 자정을 넘어 순환하므로, 10:00에서 08:40으로 갈 때 80분을 되돌아가는 대신 거의 하루치 하늘을 모두 지나갑니다.

어느 쪽이 더 짧은지 계산한 뒤 해당 모드를 전달하세요:

```ts
import { engine, SkyboxTime, TransitionMode } from '@dcl/sdk/ecs'

const DAY = 86400

// 더 짧은 경로로 새 시간대로 이동
function setTimeOfDay(fixedTime: number) {
  const current = SkyboxTime.getOrNull(engine.RootEntity)?.fixedTime
  // 아직 SkyboxTime이 설정되지 않았다면 씬은 현재 시간을 알지 못하므로, 그냥 정방향으로 진행하세요
  let transitionMode = TransitionMode.TM_FORWARD
  if (current !== undefined) {
    const forward = (fixedTime - current + DAY) % DAY
    // 반나절보다 더 많이 정방향으로 가는 것은 역방향이 더 짧다는 뜻입니다
    if (forward > DAY / 2) transitionMode = TransitionMode.TM_BACKWARD
  }
  SkyboxTime.createOrReplace(engine.RootEntity, { fixedTime, transitionMode })
}
```

플레이어가 보지 않았으면 하는 큰 점프는 전체 화면 UI 뒤에 숨기세요: 페이드 인하고, 시간을 바꾼 뒤, 잠시 기다렸다가 페이드 아웃하세요.

## 사용자 지정 하늘, 태양, 안개

세 개의 컴포넌트를 사용하면 플레이어가 그 안에 서 있는 동안 씬이 환경을 대신할 수 있습니다:

* `스카이박스`: 하늘을 자체 텍스처로 바꾸거나, 절차적 하늘과 구름, 별의 색을 다시 입힙니다.
* `태양`: 태양(과 달) 빛의 색을 바꾸거나, 어둡게 하거나, 숨기거나, 방향을 고정합니다.
* `EnvironmentFog`: 원거리 안개의 색을 바꾸고 밀도를 변경합니다.

처럼 `SkyboxTime`, 이 셋 모두 씬의 루트 엔티티에만 추가할 수 있습니다, `engine.RootEntity`. 다른 엔티티에서는 아무 효과가 없습니다. 모든 필드는 선택 사항이며, 생략한 항목은 기본 시간대 동작을 유지합니다.


> [!WARNING]
> **📔 참고** :  `스카이박스`, `태양` 및 `EnvironmentFog` 컴포넌트는 아직 모바일 앱이나 Bevy 익스플로러에서 지원되지 않습니다. 해당 클라이언트의 플레이어는 일반 하늘, 태양, 안개를 보게 되므로, 이러한 오버라이드 없이도 씬이 정상 동작하는지 확인하세요.


```ts
import { engine, Skybox, Sun, EnvironmentFog } from '@dcl/sdk/ecs'
import { Color4, Vector3 } from '@dcl/sdk/math'

function main() {
  Skybox.create(engine.RootEntity, {
    skyboxTexture: 'images/sky.png'
  })
  Sun.create(engine.RootEntity, { intensity: 0.5, direction: Vector3.create(-1, -1, 0) })
  EnvironmentFog.create(engine.RootEntity, {
    color: { keys: [{ time: 0, color: Color4.create(0.6, 0.2, 0.1, 1) }] },
    density: 0.01
  })
}
```

### 범위와 초기화

다음 `스카이박스`, `태양` 및 `EnvironmentFog` 오버라이드는 플레이어가 씬 안에 있는 동안에만 적용됩니다. 플레이어가 씬을 떠나거나, 컴포넌트가 제거되거나, 필드가 해제되면 하늘과 조명은 기본값으로 돌아가며, 씬에 다시 들어가면 다시 적용됩니다. 변경 사항은 전환 없이 즉시 적용됩니다.


> [!WARNING]
> **📔 참고**: 활성화되어 있는 동안 이 오버라이드는 전역적으로 적용됩니다. 씬 내부에서 보이는 인접한 구획도 당신의 하늘, 태양, 안개로 렌더링됩니다.


### 사용자 지정 하늘 텍스처

`skyboxTexture` 씬 에셋의 이미지로 보이는 하늘을 대체합니다. 이미지 파일 경로를 지정하세요. 이는 텍스처의 `src` 처럼 설정하면 됩니다. 이미지는 등거리 직사각형 파노라마(2:1 위도-경도 이미지)여야 하며: 가로 중앙은 씬의 양의 Z축(북쪽)을 향하고, 맨 위 행은 천정이며 맨 아래 행은 천저입니다. 큐브 크로스, 3x2, 여섯 면 큐브맵 레이아웃은 지원되지 않습니다.

```ts
import { engine, Skybox } from '@dcl/sdk/ecs'

function main() {
  Skybox.create(engine.RootEntity, {
    skyboxTexture: 'images/sky.png'
  })
}
```

이미지는 가로 방향으로 이어지며 양선형 필터링됩니다. 이미지 로드에 실패하면 기본 하늘이 유지됩니다.

이미지 크기는 적당하게 유지하세요: 씬이 로드되는 동안 전체 파노라마가 다운로드되어 메모리에 유지됩니다. 대부분의 하늘에는 2048x1024 픽셀이면 충분하며, 4096x2048을 넘기지 마세요. 다음과 같은 압축 형식을 사용하세요 `.jpg` 사진 같은 하늘에는 적합합니다.

텍스처는 하늘의 모습만 바꿉니다. 햇빛은 계속 시간대를 따르고, 주변광은 계속 다음을 따릅니다 `skyColors` (그래서 이미지에 맞출 수 있습니다), 그리고 `태양` 및 `EnvironmentFog` 컴포넌트는 그 위에 계속 적용됩니다. 고품질 프리셋에서는 반짝이는 재질의 반사도 당신의 하늘 텍스처를 따릅니다.

### 하늘 색상, 구름 및 별

절차적 하늘을 유지한다면, `스카이박스` 컴포넌트로 색을 다시 입히고 구름 층과 별 필드를 조정할 수 있습니다.

```ts
import { engine, Skybox, ColorGradient } from '@dcl/sdk/ecs'
import { Color4 } from '@dcl/sdk/math'

// 키가 하나인 그라디언트는 고정 색상입니다
function constant(color: Color4): ColorGradient {
  return { keys: [{ time: 0, color }] }
}

function main() {
  Skybox.create(engine.RootEntity, {
    skyColors: {
      zenith: constant(Color4.create(0.55, 0.25, 0.12, 1)),
      horizon: constant(Color4.create(0.95, 0.55, 0.3, 1)),
      nadir: constant(Color4.create(0.35, 0.15, 0.08, 1))
    },
    clouds: { opacity: 0.3, speed: 0.01, color: constant(Color4.create(0.9, 0.6, 0.4, 1)) },
    stars: { brightness: 4.62 }
  })
}
```

* `skyColors`: 하늘의 색, 그 `천정` (정수직 위쪽), 그리고 `수평선`, 그리고 그 `천저` (수평선 아래의 하늘로, 지형이 가리지 않는 곳에서 보입니다. 예를 들어 고도 높은 곳이나 다음이 비활성화된 월드에서 볼 수 있습니다 [풍경 지형](/docs/creator/content-creator-ko/sdk7/kinds-of-projects/scene-metadata/#landscape-terrain) 가 꺼져 있을 때). 이 색상들은 씬의 주변광도 결정합니다. 천정 색은 위에서부터 물체를 비추고, 수평선 색은 옆에서, 천저 색은 아래에서 비추어 물체와 아바타가 주변 하늘과 어울리게 합니다. `skyColors.rim` 는 일몰 빛처럼 수평선 바로 위에 그려지는 좁은 빛띠입니다. 설정하지 않으면 다음을 따릅니다 `수평선` 색상을 설정했다면 그것을 따르며(그렇지 않으면 기본값을 유지합니다), 강조 효과에만 필요합니다(예: 어두운 하늘에 주황색 일출 테두리).
* `clouds.opacity`: 0(구름 없음)부터 기본값인 1까지입니다. `clouds.speed`: 구름 층이 하늘을 도는 속도이며, 초당 라디안 단위입니다. 기본값은 0.01(약 10분에 한 바퀴)이고, 0이면 고정된 구름입니다. 음수 값은 0으로 간주됩니다. `clouds.color`: 구름 층의 색조입니다. 이것이 없으면 하늘의 색을 바꿔도 구름은 기본 시간대 색상을 유지합니다.
* `stars.brightness`: 별 텍스처에 곱해지는 배율이며, 기본값은 4.62입니다(1보다 큰 값은 HDR이며, 0이면 별이 숨겨지고, 음수 값은 0으로 간주됩니다). 별은 하루 중 밤에만 보입니다.

어떤 것이 `skyboxTexture` 설정되어 있으면, 텍스처가 절차적 하늘을 대체합니다: `구름` 및 `별` 은/는 아무 효과가 없으며, `skyColors` 더 이상 하늘의 색을 바꾸지는 않지만, 주변광은 계속 결정합니다.

### 태양

다음 `태양` 컴포넌트는 태양과 달의 방향성 조명을 변경합니다.

```ts
import { engine, Sun } from '@dcl/sdk/ecs'
import { Color4, Vector3 } from '@dcl/sdk/math'

function main() {
  Sun.create(engine.RootEntity, {
    color: { keys: [{ time: 0, color: Color4.create(1, 0.65, 0.4, 1) }] },
    intensity: 0.5,
    direction: Vector3.create(-1, -1, 0)
  })
}
```

* `color`: 방향성 조명의 색상입니다. 태양 원반의 색조도 바뀝니다.
* `버튼은`: `true` 기본값입니다. 다음으로 설정하세요 `false` 태양과 달의 원반, 그리고 태양 렌즈 플레어를 숨기려면, 예를 들어 그것들이 다음을 통해 보일 때 `skyboxTexture`. 태양이 비추는 빛은 영향을 받지 않습니다.
* `intensity`: 시간대 광 강도에 곱해지는 배율이며, 기본값은 1입니다. 0이면 햇빛이 꺼지고, 음수 값은 0으로 간주됩니다.
* `direction`: 씬 좌표계에서 햇빛이 이동하는 방향입니다: `(0, -1, 0)` 는 태양을 정수직 위에 놓고, 그리고 `(-1, -1, 0)` 는 하늘의 양의 X(동쪽) 쪽, 수평선 위 45도 위치에 둡니다. 이 값이 설정되어 있는 동안 태양은 모든 시간대에서 고정되지만, 색상과 강도는 여전히 시간대를 따릅니다. 이 값을 해제하거나 다음으로 설정하세요 `(0, 0, 0)`, 그러면 태양이 하늘을 가로지르는 일반적인 경로가 유지됩니다.

### 안개

다음 `EnvironmentFog` 컴포넌트는 거리 안개를 변경합니다.

```ts
import { engine, EnvironmentFog } from '@dcl/sdk/ecs'
import { Color4 } from '@dcl/sdk/math'

function main() {
  EnvironmentFog.create(engine.RootEntity, {
    color: { keys: [{ time: 0, color: Color4.create(0.85, 0.5, 0.3, 1) }] },
    density: 0.01
  })
}
```

* `color`: 안개의 색상입니다.
* `density`: 물체가 안개 속으로 얼마나 빨리 사라지는지 나타내며, 미터당 지수 밀도로 표현됩니다. 주어진 거리에서의 안개는 `1 - e^(-density * distance)`, 따라서 `1 / density` 는 대략 시야의 3분의 2가 안개에 가려지는 거리입니다. 기본값은 0.0005(약 2km)이며, 0.02는 50m에서 시야의 약 3분의 2를 안개로 만들고 150m쯤 되면 거의 전부를 가리며, 0이면 안개가 보이지 않습니다. 음수 값은 0으로 간주됩니다.

안개가 렌더링되는지는 플레이어가 선택한 품질 설정에 따라 달라집니다. 씬에서는 이를 켜거나 끌 수 없습니다.

### 하루에 따른 색상 그라디언트

이들 컴포넌트의 모든 색상은 `ColorGradient`: 다음의 목록입니다 `키`, 각 키에는 다음이 포함됩니다 `시간` 와 `Color4`. 다음은 `시간` 는 정규화된 시간대로, 0(*00:00*)에서 1(*24:00*), 따라서 0.5는 정오입니다. 이는 `SkyboxTime` 를 86400으로 나눈 값입니다: `fixedTime: 21600` (오전 6시)는 `time: 0.25`. 색상은 인접한 키 사이에서 보간됩니다. 첫 번째 키 이전과 마지막 키 이후에는 해당 키의 색상이 사용됩니다. 0에서 1 범위를 벗어난 시간은 그 범위로 고정되며, 두 키가 같은 `시간`, 값을 공유하면 목록에서 나중에 있는 키가 우선합니다. 키가 하나인 그라디언트는 단순히 고정 색상입니다.

```ts
import { engine, Skybox } from '@dcl/sdk/ecs'
import { Color4 } from '@dcl/sdk/math'

function main() {
  Skybox.createOrReplace(engine.RootEntity, {
    skyColors: {
      horizon: {
        keys: [
          { time: 0, color: Color4.create(0.05, 0.05, 0.2, 1) }, // 자정
          { time: 0.25, color: Color4.create(0.95, 0.5, 0.4, 1) }, // 새벽
          { time: 0.5, color: Color4.create(0.7, 0.9, 1, 1) }, // 정오
          { time: 0.75, color: Color4.create(0.95, 0.4, 0.2, 1) }, // 황혼
          { time: 1, color: Color4.create(0.05, 0.05, 0.2, 1) } // 다시 자정이므로, 점프가 없습니다
        ]
      }
    }
  })
}
```

그라디언트는 자정을 기준으로 이어지지 않으므로, 같은 색을 다음에서 반복하세요 `time: 0` 및 `time: 1` 하루가 끊김 없이 반복되어야 한다면. 색상 값은 1로 제한되지 않습니다: 1보다 큰 값은 더 밝은 HDR 태양이나 하늘을 만듭니다. 알파 채널은 무시됩니다. 그라디언트당 최대 8개의 키를 사용하세요. 더 긴 그라디언트는 8개의 균등 간격 키로 다시 샘플링되어 그 사이의 세부 정보가 매끄럽게 처리됩니다. 그라디언트를 다음과 결합하세요 `SkyboxTime` 그러면 하루를 그라디언트의 특정 지점에 고정할 수 있습니다.

### 완전한 어둠

씬에 배치한 빛만 유일한 광원이 되게 하려면, 환경이 제공하는 모든 것을 어둡게 만드세요: 햇빛과 그 원반, 하늘 색상(그리고 그에 따른 주변광), 안개, 구름, 별입니다.

```ts
import { engine, Skybox, Sun, EnvironmentFog } from '@dcl/sdk/ecs'
import { Color4 } from '@dcl/sdk/math'

function main() {
  const black = { keys: [{ time: 0, color: Color4.Black() }] }

  Skybox.createOrReplace(engine.RootEntity, {
    skyColors: { zenith: black, horizon: black, nadir: black },
    clouds: { opacity: 0 },
    stars: { brightness: 0 }
  })
  Sun.createOrReplace(engine.RootEntity, { intensity: 0, visible: false })
  EnvironmentFog.createOrReplace(engine.RootEntity, { color: black })
}
```


> [!WARNING]
> **📔 참고**: 낮음 품질 프리셋에서는 씬 조명이 꺼지므로, 완전히 어두운 씬은 해당 플레이어에게 완전히 검은색으로 렌더링됩니다. 꼭 필요한 것은 보이도록 발광 재질을 사용하세요.


다음을 참조하세요 [조명](/docs/creator/content-creator-ko/sdk7/3d/lights/) 를 사용해 씬에 포인트 라이트와 스포트라이트를 추가하세요.


> [!NOTE]
> **💡 팁**: 스카이박스 제어의 작동 예시는 다음을 참조하세요 [`2,1-skybox-sdk-scene-a`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/2,1-skybox-sdk-scene-a) 및 [`3,1-skybox-sdk-scene-b`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/3,1-skybox-sdk-scene-b) 테스트 씬이며, 다음을 구동합니다 `SkyboxTime` 및 `TransitionMode` 를 UI 패널에서 실시간으로 제어합니다. 또한 [`2,0-skybox-scene-json`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/2,0-skybox-scene-json), 다음을 읽습니다 `fixedTime` 에 설정된 `scene.json` 를 런타임에 다음을 통해 다시 `getSceneInformation()`. 다음은 [`2,2-skybox-control`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/2,2-skybox-control) 씬은 다음을 다룹니다 `스카이박스`, `태양` 및 `EnvironmentFog` 컴포넌트: 하늘 텍스처, 환경 프리셋, 태양 숨기기, 완전한 어둠 모드.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/interactivity/skybox-control.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
