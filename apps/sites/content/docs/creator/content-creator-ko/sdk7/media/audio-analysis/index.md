> 씬에서 재생 중인 오디오의 실시간 진폭 및 주파수 데이터를 읽어 반응형 비주얼을 구동합니다.

# 오디오 분석

이 `AudioAnalysis` 컴포넌트는 씬에서 재생 중인 오디오 소스에서 실시간 데이터를 읽어오므로, 음악에 맞춰 비주얼을 구동할 수 있습니다. 매 프레임마다 전체 **진폭** 값과 **8개 주파수 대역** (낮은 것부터 높은 것까지)를 보고하며, 이를 사용해 엔티티의 크기를 조절하거나, 색을 바꾸거나, 애니메이션을 적용할 수 있습니다.

일반적인 용도:

* 베이스에 맞춰 튀는 큐브.
* 박자에 맞춰 맥박치듯 빛나는 조명.
* 이퀄라이저 스타일의 막대 시각화.
* 음악과 맞물려 반응하는 머티리얼 또는 크기가 조절된 소품.

`AudioAnalysis` 은 읽기 전용 피드입니다. 씬에서는 값만 소비하고, 런타임이 그 값을 채워 넣습니다. 오디오를 통해 [AudioSource](/docs/creator/content-creator-ko/sdk7/3d/sounds/)를 사용하는 엔티티, [AudioStream](/docs/creator/content-creator-ko/sdk7/media/audio-streaming/)을 사용하는 엔티티, 또는 [VideoPlayer](/docs/creator/content-creator-ko/sdk7/media/video-playing/).


> [!WARNING]
> **📔 참고**: `AudioAnalysis` 는 현재 공식 Decentraland 데스크톱 앱에서만 지원됩니다. 다른 클라이언트에서는 컴포넌트의 값이 채워지지 않으므로, 데이터가 끝내 도착하지 않더라도 씬이 정상 동작하는지 확인하세요.


## 최소 예제

다음 씬은 소리를 재생하고, `AudioAnalysis` 을 같은 엔티티에 붙인 뒤, 오디오의 진폭을 사용해 매 프레임 큐브의 크기를 조절합니다.

```ts
import {
  engine,
  Transform,
  MeshRenderer,
  AudioSource,
  AudioAnalysis,
  AudioAnalysisView,
} from "@dcl/sdk/ecs";
import { Vector3 } from "@dcl/sdk/math";

export function main() {
  // 엔티티에서 소리 재생
  const audioEntity = engine.addEntity();
  Transform.create(audioEntity);
  AudioSource.create(audioEntity, {
    audioClipUrl: "sounds/music.mp3",
    playing: true,
    loop: true,
  });

  // 분석 데이터를 받기 위해 같은 엔티티에 AudioAnalysis 연결
  AudioAnalysis.createAudioAnalysis(audioEntity);

  // 컴포넌트가 매 프레임 기록해 넣을 재사용 가능한 뷰 객체
  const analysis: AudioAnalysisView = {
    amplitude: 0,
    bands: new Array<number>(8),
  };

  // 오디오 진폭에 맞춰 맥박치는 큐브
  const cube = engine.addEntity();
  MeshRenderer.setBox(cube);
  Transform.create(cube, { position: Vector3.create(8, 1, 8) });

  // 매 프레임 최신 분석 값을 읽어 큐브의 크기를 조절
  engine.addSystem(() => {
    AudioAnalysis.readIntoView(audioEntity, analysis);
    const s = 1 + analysis.amplitude * 10;
    Transform.getMutable(cube).scale = Vector3.create(s, s, s);
  });
}
```

핵심 호출은 다음과 같습니다:

1. `AudioAnalysis.createAudioAnalysis(entity)` — 컴포넌트를 `AudioSource`, `AudioStream`소유한 엔티티에 붙입니다. 또는 `VideoPlayer`. 기본값은 로그 모드입니다( [모드](#modes)).
2. `AudioAnalysis.readIntoView(entity, analysis)` — 최신 `진폭` 및 8개의 `대역` 값을 제공한 뷰 객체에 복사합니다.


> [!WARNING]
> **📔 참고**: `대역` 배열을 **8개 요소로** 미리 할당한 다음 `readIntoView`를 호출하세요. 컴포넌트가 배열 크기를 자동으로 조정해 주지 않습니다.


## 매 프레임 데이터 읽기

`AudioAnalysis` 는 시스템에서 프레임당 한 번 읽도록 설계되었습니다. 권장 패턴은 다음과 같습니다:

1. 단 하나의 `AudioAnalysisView` 객체를 한 번만 할당합니다.
2. 시스템에서 `readIntoView` 를 호출해 갱신합니다.
3. 값을 직접 사용하거나, 여러 시스템이 같은 뷰를 공유하도록 하세요.

```ts
const analysis: AudioAnalysisView = {
  amplitude: 0,
  bands: new Array<number>(8),
};

engine.addSystem(() => {
  AudioAnalysis.readIntoView(audioEntity, analysis);
  // 이제 analysis.amplitude 및 analysis.bands[0..7]에 최신 값이 들어 있습니다
});
```

분석 대상 엔티티에 아직 `AudioAnalysis` 컴포넌트가 없을 수 있다면(예: 나중에 생성되거나 동적으로 제거되는 경우), 대신 `tryReadIntoView` 를 사용하세요. 이 함수는 `false` 를 반환하고, `true` 는 값이 기록되었을 때 반환합니다.

```ts
engine.addSystem(() => {
  if (!AudioAnalysis.tryReadIntoView(audioEntity, analysis)) return;
  // 여기서는 analysis를 안전하게 사용할 수 있음
});
```


> [!NOTE]
> **💡 팁**: `readIntoView` 컴포넌트를 반복해서 읽는 것보다 한 번 호출한 뒤 여러 시스템이 같은 뷰 객체를 공유하게 하는 편이 더 저렴합니다.


## 특정 주파수 대역에 반응하기

8개 대역은 들을 수 있는 스펙트럼을 낮은 것부터 높은 것까지 모두 덮습니다. `bands[0]` 가 가장 낮은(베이스) 대역이고 `bands[7]` 가 가장 높은(트레블) 대역입니다. 서로 다른 엔티티를 서로 다른 대역으로 구동해 전통적인 이퀄라이저를 만들 수 있습니다.

```ts
import {
  engine,
  Transform,
  MeshRenderer,
  Material,
  AudioSource,
  AudioAnalysis,
  AudioAnalysisView,
  Schemas,
} from "@dcl/sdk/ecs";
import { Color4, Vector3 } from "@dcl/sdk/math";

// 각 막대를 표시하고 추적 중인 대역을 기억하는 커스텀 컴포넌트
const VisualBar = engine.defineComponent("visual-bar", {
  index: Schemas.Number,
});

const BARS_HEIGHT = 12;

export function main() {
  const audioEntity = engine.addEntity();
  Transform.create(audioEntity);
  AudioSource.create(audioEntity, {
    audioClipUrl: "sounds/music.mp3",
    playing: true,
    loop: true,
  });
  AudioAnalysis.createAudioAnalysis(audioEntity);

  const analysis: AudioAnalysisView = {
    amplitude: 0,
    bands: new Array<number>(8),
  };

  // 8개의 막대를 생성, 대역당 1개
  for (let i = 0; i < 8; i++) {
    const bar = engine.addEntity();
    VisualBar.create(bar, { index: i });
    Transform.create(bar, { position: Vector3.create(4 + i, 0, 8) });
    MeshRenderer.setBox(bar);
    Material.setPbrMaterial(bar, { albedoColor: Color4.Yellow() });
  }

  // 매 프레임 한 번 읽기
  engine.addSystem(() => {
    AudioAnalysis.readIntoView(audioEntity, analysis);
  });

  // 각 막대를 해당 대역에 맞춰 크기 조절
  engine.addSystem(() => {
    for (const [entity] of engine.getEntitiesWith(VisualBar, Transform)) {
      const index = VisualBar.get(entity).index;
      const transform = Transform.getMutable(entity);
      transform.scale = Vector3.create(
        1,
        analysis.bands[index] * BARS_HEIGHT,
        1
      );
    }
  });
}
```

이것은 [오디오 시각화 예제 씬](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/88,-10-audio-visualization)에서 사용된 것과 동일한 패턴이며, 핵심만 남겨 축약한 것입니다.

## 모드

`AudioAnalysis` 는 컴포넌트를 생성할 때 설정하는 두 가지 분석 모드를 지원합니다:

* `PBAudioAnalysisMode.MODE_LOGARITHMIC` (기본값): 값이 로그 곡선으로 스케일링되며, 이는 사람이 음량 변화를 인지하는 방식에 더 가깝습니다. 시각적 반응에 가장 적합합니다.
* `PBAudioAnalysisMode.MODE_RAW`: 처리되지 않은 진폭 및 FFT 대역 값입니다. 직접 스케일링을 적용하고 싶을 때 사용하세요.

```ts
import { AudioAnalysis, PBAudioAnalysisMode } from "@dcl/sdk/ecs";

// 원시 값 사용
AudioAnalysis.createAudioAnalysis(audioEntity, PBAudioAnalysisMode.MODE_RAW);
```

### 로그 모드 조정하기

로그 모드에서는 두 개의 선택적 게인 배율을 전달할 수 있습니다:

* `amplitudeGain` — 전체 진폭에 적용되는 배율입니다. 기본값은 `5`.
* `bandsGain` — 8개 모든 대역에 적용되는 배율입니다. 기본값은 `0.05`.

게인을 높이면 값이 더 민감해지고, 낮추면 더 미세해집니다.

```ts
// 더 강한 진폭 반응, 더 부드러운 대역 반응
AudioAnalysis.createAudioAnalysis(
  audioEntity,
  PBAudioAnalysisMode.MODE_LOGARITHMIC,
  10, // amplitudeGain
  0.03 // bandsGain
);
```


> [!WARNING]
> **📔 참고**: `amplitudeGain` 및 `bandsGain` 는 로그 모드에서만 적용됩니다. 원시 모드에서는 무시됩니다.


## 기존 컴포넌트 교체

`createAudioAnalysis` 는 엔티티에 이미 `AudioAnalysis` 컴포넌트가 있으면 실패합니다. 런타임 중 모드나 게인을 바꾸려면 `createOrReplaceAudioAnalysis`를 사용하세요. 시그니처는 동일합니다.

```ts
// 씬 도중에 원시 모드로 전환
AudioAnalysis.createOrReplaceAudioAnalysis(
  audioEntity,
  PBAudioAnalysisMode.MODE_RAW
);
```

## 컴포넌트 참조

### `AudioAnalysis.createAudioAnalysis(entity, mode?, amplitudeGain?, bandsGain?)`

컴포넌트를 `AudioAnalysis` 에 부착합니다. `entity`. 이미 컴포넌트가 있으면 실패합니다.

* `entity` (`Entity`: `AudioSource`, `AudioStream`소유한 엔티티에 붙입니다. 또는 `VideoPlayer` 분석하려는
* `mode` (`PBAudioAnalysisMode`(선택 사항): `MODE_LOGARITHMIC` (기본값) 또는 `MODE_RAW`.
* `amplitudeGain` (`number`(선택 사항): 로그 모드 진폭 배율. 기본값 `5`.
* `bandsGain` (`number`(선택 사항): 로그 모드 대역 배율. 기본값 `0.05`.

### `AudioAnalysis.createOrReplaceAudioAnalysis(entity, mode?, amplitudeGain?, bandsGain?)`

위와 같지만, 실패하는 대신 엔티티의 기존 `AudioAnalysis` 컴포넌트를 교체합니다.

### `AudioAnalysis.readIntoView(entity, out)`

최신 값을 `out`에 읽어옵니다. `entity` 에 `AudioAnalysis` 컴포넌트가 없으면 예외를 던집니다.

* `entity` (`Entity`: 컴포넌트를 가진 엔티티입니다.
* `out` (`AudioAnalysisView`: 제공하는 뷰 객체입니다. 8개의 숫자로 `대역` 미리 할당되어 있어야 합니다.

### `AudioAnalysis.tryReadIntoView(entity, out): boolean`

과 동일하지만, 엔티티에 `readIntoView`컴포넌트가 없을 경우 예외를 던지는 대신 `false` 를 반환합니다. `AudioAnalysis` 값이 기록되었을 때 `true` 를 반환합니다.

### `AudioAnalysisView`

분석 데이터를 받을 수 있도록 할당하는 일반 객체:

```ts
type AudioAnalysisView = {
  amplitude: number; // 전체 신호 세기
  bands: number[]; // 8개 주파수 대역, 낮은(0) 것부터 높은(7) 것까지
};
```

## 참고 및 제한 사항

* **8개 대역, 고정.** 주파수 대역 수는 8개로 고정되어 있습니다. 더 많거나 적은 대역을 요청하는 API는 없습니다.
* **분석당 오디오 소스 하나.** 각 `AudioAnalysis` 컴포넌트는 부착된 엔티티의 오디오를 분석합니다. 여러 소스를 분석하려면 각 소스마다 `AudioAnalysis` 를 붙이세요.
* **비디오 사운드트랙도 분석됩니다.** `AudioAnalysis` 는 [`VideoPlayer`](/docs/creator/content-creator-ko/sdk7/media/video-playing/) 를 읽는 것과 같은 방식으로 `AudioSource`의 오디오를 읽으며, 프로그레시브 비디오 파일( `.mp4`)과 라이브 HLS 스트림(`.m3u8`) 모두에 적용됩니다. 같은 엔티티에 `AudioAnalysis` 를 붙여서 `VideoPlayer`.
* **일시정지된 오디오는 업데이트를 보고하지 않습니다.** 기반 오디오가 정지되거나 일시정지되면 컴포넌트 값은 더 이상 변하지 않습니다. 마지막으로 읽은 값은 오디오가 다시 재생될 때까지 뷰 객체에 남아 있습니다.
* **저렴하지만 공짜는 아닙니다.** 프레임별 분석은 비용이 낮게 설계되었습니다(데스크톱에서 소스당 밀리초 미만). 데이터가 필요하지 않다면 `AudioAnalysis` 를 한 번에 너무 많은 소스에 붙이지 마세요. 시각화기가 보이지 않을 때는 컴포넌트를 제거하세요.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/media/audio-analysis.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
