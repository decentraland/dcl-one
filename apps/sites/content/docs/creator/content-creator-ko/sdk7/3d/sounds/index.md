> 씬에 사운드를 추가하는 방법을 알아봅니다.

# 사운드

사운드는 플레이어의 행동과 이벤트에 피드백을 제공하는 좋은 방법이며, 배경음은 장면에 더 많은 맥락을 더하고 플레이어의 몰입감을 높여줍니다.


> [!WARNING]
> **📔 참고**: 사운드는 음성 범위에 있더라도, 사운드가 생성된 장면을 이루는 파셀 안에 서 있는 플레이어만 들을 수 있다는 점을 기억하세요. 플레이어는 설정에서 모든 사운드를 끌 수도 있습니다.


지원되는 사운드 형식은 브라우저에 따라 다르지만, 다음 형식을 사용하는 것이 권장됩니다 *.mp3*.

*.wav* 파일도 지원되지만 훨씬 더 무겁기 때문에 일반적으로 권장되지 않습니다.

## 사운드 재생하기

사운드를 재생하는 가장 쉬운 방법은 다음을 추가하는 것입니다. **오디오 소스** 컴포넌트를 시각적으로 [Creator Hub의 씬 에디터에](/docs/creator/content-creator-ko/scene-editor/get-started/about-editor/) 추가한 뒤 **재생 시작** 및 **반복**. 자세한 내용은 [컴포넌트 추가](/docs/creator/content-creator-ko/scene-editor/build/components/#add-components).

![](/docs/_remote/341e53c596dc617859805f3b.webp)

코드 없이도 다음을 통해 사운드 재생을 트리거할 수 있습니다. **액션**, 다음을 참조하세요. [모든 아이템을 스마트하게 만들기](/docs/creator/content-creator-ko/scene-editor/interactivity/make-any-item-smart/).

코드로 사운드를 재생하려면 `AudioSource.playSound` 함수를 사용하세요.

```ts
// 엔티티 생성
const sourceEntity = engine.addEntity()
Transform.create(sourceEntity, { position: Vector3.create(8, 1, 8) })

// 사운드 재생
AudioSource.playSound(sourceEntity, 'assets/sounds/sound-effect.mp3')
```


> [!WARNING]
> **📔 참고**: 모든 엔티티에는 `AudioSource` 도 필요합니다. `Transform`이 포함되며, 전역 사운드도 마찬가지입니다. 이것이 없으면 아무것도 재생되지 않고 오류도 기록되지 않기 때문에 찾아내기 어려운 문제입니다. 씬 에디터에서 배치한 아이템에는 항상 `Transform`Transform


가 있으므로, 이 문제는 코드에서 생성한 엔티티에만 영향을 줍니다. `assets/sounds` 폴더에 있으며, 이는 씬 프로젝트 폴더의 최상위 수준에 위치합니다.


> [!WARNING]
> **📔 참고**: `AudioSource` 컴포넌트는 다음을 통해 임포트해야 합니다.
> 
> > `import { AudioSource } from "@dcl/sdk/ecs"`
> 
> 다음을 참조하세요. [Imports](/docs/creator/content-creator-ko/sdk7/getting-started/coding-scenes/#imports) 에서 이를 쉽게 처리하는 방법을 확인하세요.


다음 `AudioSource.playSound()` 함수는 다음 인수를 받습니다:

* `entity`: 사운드를 적용할 엔티티입니다. 사운드는 이 엔티티의 위치에서 들리며, 플레이어가 가까워질수록 더 크게 들립니다.
* `src`: 프로젝트 내 사운드 파일의 위치입니다.


> [!NOTE]
> **💡 팁**: 더 명확하게 하려면, 사운드 파일을 씬 내부의 `assets/sounds` 폴더에 따로 보관하는 것을 권장합니다.


* `resetCursor`: *(선택 사항)* true이면 사운드는 항상 처음부터 시작합니다. 그렇지 않으면 현재 커서 위치에서 계속됩니다. 일시 중지 및 재개에 유용합니다.

`playSound()` 는 다음을 생성합니다. `AudioSource` 엔티티에 아직 컴포넌트가 없다면 해당 `playSound()` 다른 값들은 유지합니다. 예를 들어 `volume`, `pitch`, `loop` 및 `global`, 따라서 이를 한 번 설정한 뒤 사운드를 재생할 때마다 `playSound()` 를 호출하면 됩니다. 같은 파일로 다시 호출하면 사운드가 다시 재생됩니다. 항상 `true`.

사운드를 재생하는 또 다른 방법은 엔티티에 `AudioSource` 컴포넌트를 수동으로 만드는 것입니다. 이 방법은 사운드에 대해 더 많은 제어가 필요할 때, 예를 들어 반복 재생이나 볼륨 설정에 유용합니다.

```ts
// 엔티티 생성
const sourceEntity = engine.addEntity()
Transform.create(sourceEntity, { position: Vector3.create(8, 1, 8) })

// AudioSource 컴포넌트 생성
AudioSource.create(sourceEntity, {
	audioClipUrl: 'sounds/sound-effect.mp3',
	loop: true,
	playing: true,
})
```

다음 속성을 설정할 수 있습니다:

* `audioClipUrl`: 프로젝트 내 사운드 파일의 위치입니다.
* `playing`: true이면 사운드가 재생되기 시작합니다. 나중에 `playing` 를 false로 설정해 두었다가, 이후에 true로 변경할 수 있습니다.
* `volume`: *(선택 사항)* 사운드 파일의 볼륨입니다. 기본값은 1이며, 최대 음량입니다.
* `pitch`: *(선택 사항)* 사운드의 피치를 수정합니다. 기본값은 1이며, 더 깊은 소리를 위해 낮출 수 있고 더 높은 피치의 소리를 위해 높일 수 있습니다.


> [!NOTE]
> **💡 팁**: 게임 중 같은 효과음이 너무 반복적으로 들리지 않도록, 재생될 때마다 사운드의 피치를 약간씩 무작위화하는 것이 유용합니다.


* `currentTime`: *(선택 사항)* 기본값은 0입니다. 사운드 파일의 처음부터 시작하지 않도록 이 값을 설정하세요.

각 엔티티는 하나의 `AudioSource` 컴포넌트만 가질 수 있으며, 한 번에 하나의 클립만 재생할 수 있습니다. 이 제한은 새 사운드를 재생할 때 오디오 소스를 수정하거나, 각기 다른 사운드를 가진 보이지 않는 자식 엔티티를 여러 개 포함하면 쉽게 극복할 수 있습니다.


> [!WARNING]
> **📔 참고**: 사운드는 각 플레이어의 로컬 인스턴스에서 재생됩니다. 다른 주변 플레이어는 자신의 로컬 씬에서도 명시적으로 재생하지 않는 한 같은 사운드를 듣지 못합니다.


### 사운드 미리 로드하기

엔티티가 사운드를 사용하지만 씬 런타임에 즉시 재생되지 않는 경우, 다운로드하는 데 시간이 걸릴 수 있습니다. 다음을 사용하면 씬 런타임에 사용할 수 있습니다. `AssetLoad` 컴포넌트.

```ts
import { AssetLoad } from "@dcl/sdk/ecs"

AssetLoad.create(engine.RootEntity, {
  assets: [
    "assets/scene/bundle1/explosionSound.mp3",
  ],
})
```

자세한 내용은 [리소스 미리 로드](/docs/creator/content-creator-ko/sdk7/optimizing/pre-load-resources/) 문서를 확인하세요.

## 사운드 중지하기

엔티티의 사운드 재생을 중지하려면 `AudioSource.stopSound()` 함수를 사용하세요. 엔티티마다 하나의 `AudioSource` 컴포넌트가 있고, 각 `AudioSource` 컴포넌트는 한 번에 하나의 파일만 재생하기 때문입니다.

```ts
AudioSource.stopSound(sourceEntity)
```

`stopSound()` 를 반환합니다 `false` 엔티티에 `AudioSource` 컴포넌트.

사운드를 중지하는 또 다른 방법은 `playing` 속성을 false로 설정하는 것입니다.

```ts
// 엔티티 생성
const sourceEntity = engine.addEntity()

// AudioSource 컴포넌트 생성
AudioSource.create(sourceEntity, {
	audioClipUrl: 'sounds/explosion.mp3',
	playing: true,
})

// 간단한 함수 정의
function stopSound(entity: Entity) {
	// 오디오 소스 컴포넌트의 변경 가능한 버전 가져오기
	const audioSource = AudioSource.getMutable(entity)

	// playing 값을 수정
	audioSource.playing = false
}

// 함수 호출
stopSound(sourceEntity)
```

## 사운드가 끝났는지 감지하기

반복 재생되지 않는 사운드가 스스로 재생을 마치면, 엔진은 `playing` 속성을 `AudioSource` 컴포넌트의 *false*로 되돌립니다. 씬 코드에서는 이 값을 읽어 사운드가 끝났는지 알 수 있으며, 예를 들어 바로 뒤에 다른 사운드나 동작을 연결할 수 있습니다.

```ts
let wasPlaying = false

engine.addSystem(() => {
	const audio = AudioSource.get(sourceEntity)
	const isPlaying = audio.playing ?? false

	if (wasPlaying && !isPlaying) {
		console.log('사운드 재생이 완료되었습니다')
		// 여기서 반응: 다른 사운드 재생, 시퀀스 진행 등
	}

	wasPlaying = isPlaying
})
```


> [!WARNING]
> **📔 참고**: 매 프레임 컴포넌트를 폴링할 때는 항상 `AudioSource.get()` 를 사용해 읽으세요. (읽기 전용) `AudioSource.getMutable()` 를 사용하면 매 프레임 컴포넌트가 변경된 것으로 표시되어 불필요한 동기화 작업이 발생합니다.
> 
> 다음 `playing` 속성은 사운드가 스스로 끝날 때만 엔진이 바꿉니다. 씬이 사운드를 명시적으로 중지하면( `AudioSource.stopSound()` 또는 `playing` 로 설정하여 *false*), 그 변경은 여러분 자신의 코드에서 온 것이며, 반복 재생되는 사운드는 중지될 때까지 계속 재생되므로 스스로 이 속성을 바꾸지 않습니다.


또는 다음을 사용하세요. `audioEventsSystem.registerAudioEventsEntity` 를 사용해 엔티티의 오디오 재생 상태가 바뀔 때마다 실행되는 함수를 정의하세요. 이는 [비디오 이벤트](/docs/creator/content-creator-ko/sdk7/media/video-playing/#video-events) 와 비슷합니다. 함수는 `state` 필드를 가진 이벤트를 받으며, `MediaState` 열거형을 사용합니다.

```ts
import { engine, AudioSource, audioEventsSystem, MediaState } from '@dcl/sdk/ecs'

// ... AudioSource 컴포넌트가 있는 sourceEntity 생성 ...

audioEventsSystem.registerAudioEventsEntity(sourceEntity, (audioEvent) => {
	switch (audioEvent.state) {
		case MediaState.MS_LOADING:
			console.log('오디오 이벤트 - 사운드가 로드 중입니다')
			break
		case MediaState.MS_READY:
			console.log('오디오 이벤트 - 사운드가 준비되었습니다(로드되었거나 재생이 완료됨)')
			break
		case MediaState.MS_PLAYING:
			console.log('오디오 이벤트 - 사운드 재생이 시작되었습니다')
			break
		case MediaState.MS_ERROR:
			console.log('오디오 이벤트 - 사운드 오류(예: 파일 로드 실패)')
			break
	}
})
```

에서 `MS_PLAYING` 로 설정하여 `에서` MS\_READY로 전환되면 사운드 재생이 중지되었다는 뜻입니다. 전체 `MediaState` 열거형에는 다음도 포함됩니다. `MS_NONE`, `MS_LOADING`, `MS_PAUSED`, `MS_BUFFERING`, `MS_SEEKING` 및 `MS_ERROR`가 포함되며, 각 이벤트에는 `타임스탬프` 가 상태 변경이 발생한 시점을 담고 있습니다. 또한 언제든지 다음으로 최신 보고 상태를 조회할 수 있습니다. `audioEventsSystem.getAudioState(entity)`, 콜백 등록 해제는 다음으로 할 수 있습니다. `audioEventsSystem.removeAudioEventsEntity(entity)`, 또는 엔티티가 등록되었는지 다음으로 확인할 수 있습니다. `audioEventsSystem.hasAudioEventsEntity(entity)`. 동일한 시스템은 [`AudioStream`](/docs/creator/content-creator-ko/sdk7/media/audio-streaming/) 컴포넌트.


> [!WARNING]
> **📔 참고**이 있는 엔티티에도 동작합니다. 이러한 기능은 데스크톱 클라이언트에서만 지원됩니다.


## 반복 재생

사운드를 연속 루프로 계속 재생하려면 `loop` 필드를 `AudioSource` 컴포넌트의 *true* 로 설정한 후 재생을 시작하세요.

```ts
// 엔티티 생성
const sourceEntity = engine.addEntity()

// AudioSource 컴포넌트 생성
AudioSource.create(sourceEntity, {
	audioClipUrl: 'sounds/sound-effect.mp3',
	playing: true,
	loop: true,
})
```

반복 재생되는 사운드는 배경 음악이나 기타 배경음을 추가할 때 특히 유용합니다.

## 볼륨 설정

다음 값을 설정할 수 있습니다. `volume` 속성을 `AudioSource` 컴포넌트를 사용해 사운드의 볼륨을 변경할 수 있습니다.

볼륨은 다음 범위의 숫자로 표현됩니다. *0* 로 설정하여 *1*.

```ts
// 엔티티 생성
const sourceEntity = engine.addEntity()

// AudioSource 컴포넌트 생성
AudioSource.create(sourceEntity, {
	audioClipUrl: 'sounds/sound-effect.mp3',
	playing: true,
	volume: 0.5,
})
```


> [!WARNING]
> **📔 참고**: 물론 사운드의 볼륨은 플레이어와 오디오 소스 간의 거리에도 영향을 받습니다. 플레이어가 멀어질수록 볼륨은 낮아집니다.


## 전역 사운드

기본적으로 `AudioSource` 의 모든 사운드는 위치 기반입니다. 즉, 사운드는 `Transform` 컴포넌트의 위치에서 생성되는 것처럼 들리며, 플레이어가 더 가까이 갈수록 더 크게 들립니다. 하지만 사운드를 전역으로 설정하여 플레이어가 어디에 서 있든 볼륨이 일정하게 유지되도록 할 수도 있습니다. 이는 배경 음악, 알림음 및 기타 비위치성 사운드에 이상적입니다.


> [!WARNING]
> **📔 참고**: 전역 사운드는 DCL 2.0 데스크톱 클라이언트에서만 지원되는 기능입니다.


사운드를 전역으로 만들려면 `global` 속성을 *true*.

```ts
AudioSource.create(sourceEntity, {
	audioClipUrl: 'sounds/music.mp3',
	playing: true,
	global: true,
})
```

전역 사운드에도 여전히 `Transform` 가 엔티티에 필요합니다. 위치가 더 이상 플레이어가 듣는 내용에 영향을 주지 않더라도 말입니다. 어떤 위치든 상관없습니다.

## 사운드의 일부 재생하기

더 긴 사운드 파일의 일부를 재생하려면 `playSoundSegment()` 를 SDK Utils 라이브러리에서 사용하세요. 다음을 참조하세요. [SDK7 Utils](https://github.com/decentraland/sdk7-utils).

이것은 다음을 명시적으로 설정하여서도 달성할 수 있습니다. `currentTime` 속성을 `AudioSource` 컴포넌트에 설정한 뒤, 일정 시간 기다린 후 중지하는 방법입니다.

## 오디오 스트리밍

다음을 참조하세요. [오디오 스트리밍](/docs/creator/content-creator-ko/sdk7/media/audio-streaming/) 외부 소스에서 라이브 오디오 스트림을 재생하는 방법을 알아보세요.

## 오디오 분석

다음을 참조하세요. [오디오 분석](/docs/creator/content-creator-ko/sdk7/media/audio-analysis/) 오디오에서 실시간 진폭 및 주파수 데이터를 읽고, 베이스에 맞춰 펄스하는 큐브나 이퀄라이저 스타일 막대 시각화 같은 반응형 시각 효과를 만드는 방법을 알아보세요. `AudioSource` 에서 이를 구동하는 방법을 알아보세요.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/3d/sounds.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
