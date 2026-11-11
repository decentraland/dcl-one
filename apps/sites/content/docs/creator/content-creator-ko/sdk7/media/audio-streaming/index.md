> 씬에서 라이브 오디오 스트림을 재생합니다.

# 오디오 스트리밍

URL에서 오디오를 스트리밍할 수 있습니다. 이는 인터넷 라디오에서 음악을 직접 재생하거나, 콘퍼런스를 씬으로 스트리밍하는 데 유용합니다.


> [!NOTE]
> **💡 팁**: [Creator Hub의 씬 에디터에 추가한 뒤](/docs/creator/content-creator-ko/scene-editor/get-started/about-editor/), 다음을 사용할 수 있습니다 **오디오 스트림** [스마트 아이템](/docs/creator/content-creator-ko/scene-editor/interactivity/smart-items/) 이것을 노코드 방식으로 구현하려면 이 방법을 사용하세요.


소스의 오디오는 다음 형식 중 하나여야 합니다: `.mp3`, `ogg`, 또는 `aac`. 소스는 또한 다음이어야 합니다 *https* URL (*http* URL은 지원되지 않으며), 소스에는 다음이 있어야 합니다 [CORS 정책(Cross Origin Resource Sharing)](https://en.wikipedia.org/wiki/Cross-origin_resource_sharing) 외부에서 액세스할 수 있도록 허용해야 합니다. 그렇지 않은 경우, 프록시 역할을 하는 서버를 설정하여 스트림을 유효한 방식으로 노출해야 할 수 있습니다.


> [!WARNING]
> **📔 참고**: 대신 씬에서 미리 녹음된 사운드를 재생하려면 다음을 참조하세요 [사운드](/docs/creator/content-creator-ko/sdk7/3d/sounds/).


씬에 오디오 스트림을 추가하려면 간단히 다음을 추가하세요 `AudioStream` 컴포넌트를 엔티티에:

```ts
const streamEntity = engine.addEntity()

AudioStream.create(streamEntity, {
	url: 'https://icecast.ravepartyradio.org/ravepartyradio-192.mp3',
	playing: true,
	volume: 0.8,
})
```


> [!WARNING]
> **📔 참고**: 스트리밍된 사운드는 위치 기반이 아니므로 씬 전체에서 일정한 볼륨으로 들립니다. 플레이어가 씬 밖으로 나가면 스트리밍을 전혀 들을 수 없습니다.


다음의 볼륨을 설정하세요 `AudioStream` 컴포넌트의 다음을 변경하여 `volume` 프로퍼티로 정의됩니다.

다음을 전환하세요 `AudioStream` 컴포넌트를 다음을 설정하여 켜거나 끄세요 `playing` 속성을 *true* 또는 *false*.


> [!NOTE]
> **📔 참고**: 모든 스트리밍 서비스가 사이트 외부에서 오디오 재생을 허용하는 것은 아닙니다. 다음은 Decentraland에서 작동하는 몇 가지 예시입니다:
> 
> ```ts
> DELTA = "https://cdn.instream.audio/:9069/stream?_=171cd6c2b6e"
> GRAFFITI = "https://n07.radiojar.com/2qm1fc5kb.m4a?1617129761=&rj-tok=AAABeIR7VqwAilDFeUM39SDjmw&rj-ttl=5"
> ISLA NEGRA = "https://radioislanegra.org/listen/up/basic.aac"
> ```
> 


## 스트림 상태

다음 중 하나를 사용하여 오디오 스트림의 상태를 모니터링할 수 있습니다 `audioEventsSystem` (콜백 기반, 권장) 또는 다음으로 폴링하여 `AudioStream.getAudioState()`.

### audioEventsSystem 사용(권장)

스트림 상태가 변경될 때만 실행되는 콜백을 등록합니다. 이는 다음에도 사용되는 동일한 시스템입니다 `AudioSource` 엔티티(참조 [사운드가 끝났을 때 감지](/docs/creator/content-creator-ko/sdk7/3d/sounds/#detect-when-a-sound-finishes)).

```ts
import { engine, AudioStream, MediaState } from '@dcl/sdk/ecs'
import { audioEventsSystem } from '@dcl/sdk/ecs'

export function main() {
	const entity = engine.addEntity()

	AudioStream.create(entity, {
		playing: true,
		volume: 1,
		url: 'https://audio-edge-es6pf.mia.g.radiomast.io/ref-128k-mp3-stereo',
	})

	audioEventsSystem.registerAudioEventsEntity(entity, (event) => {
		console.log('Stream state: ', event.state)

		if (event.state === MediaState.MS_ERROR) {
			// 재연결 시도
		}
	})
}
```

### getAudioState로 폴링

다음을 사용하여 각 프레임마다 스트림 상태를 폴링할 수도 있습니다 `AudioStream.getAudioState()`. 이는 다음을 반환합니다 `PBAudioEvent` 객체(또는 `undefined` 아직 상태가 보고되지 않은 경우), 다음을 포함하는 `state` 필드와 `타임스탬프` 필드입니다. 다음 `state` 필드는 다음의 값입니다 `MediaState` 열거형:

* `MS_NONE`
* `MS_LOADING`
* `MS_READY`
* `MS_PLAYING`
* `MS_PAUSED`
* `MS_BUFFERING`
* `MS_SEEKING`
* `MS_ERROR`

```ts
export function main() {
	const entity = engine.addEntity()

	AudioStream.create(entity, {
		playing: true,
		volume: 1,
		url: 'https://audio-edge-es6pf.mia.g.radiomast.io/ref-128k-mp3-stereo',
	})

	let lastState: MediaState | undefined = undefined
	engine.addSystem(() => {
		const currentState = AudioStream.getAudioState(entity)?.state
		if (lastState !== currentState) {
			console.log('Stream state: ', currentState)
			lastState = currentState

			if (currentState == MediaState.MS_ERROR) {
				// 재연결 시도
			}
		}
	})
}
```

## 공간 오디오

기본적으로 다음의 오디오는 `AudioStream` 컴포넌트는 전역이므로 씬 전체에서 일정한 볼륨으로 들립니다. 플레이어가 씬 밖으로 나가면 스트리밍이 전혀 들리지 않습니다.

오디오를 공간화하려면 다음을 설정하세요: `spatial` 속성을 *true*.

```ts
AudioStream.create(entity, {
	url: 'https://radioislanegra.org/listen/up/stream',
    playing: true,
	spatial: true,
})
```

이제 오디오는 다음을 소유한 엔티티의 위치에서 들리게 됩니다 `AudioStream` 컴포넌트이며, 플레이어가 가까이 갈수록 더 크게 들립니다.

다음 속성으로 공간 오디오를 제어할 수 있습니다:

* `spatialMinDistance`: 오디오가 공간화되기 시작하는 최소 거리입니다. 플레이어가 더 가까우면 오디오는 최대 음량으로 들립니다. *0* 기본값입니다.
* `spatialMaxDistance`: 오디오가 들리는 최대 거리입니다. 플레이어가 더 멀리 있으면 오디오는 음량 0으로 들립니다. *60* 기본값

```ts
const audioStreamEntity = engine.addEntity();

Transform.create(audioStreamEntity, {
    position: Vector3.create(8, 0, 8),
});

AudioStream.create(audioStreamEntity, {
    url: 'https://radioislanegra.org/listen/up/stream',
    playing: true,
    volume: 1.0,
    spatial: true,
    spatialMinDistance: 5,
    spatialMaxDistance: 10
});
```


> [!WARNING]
> **📔 참고**: 일부 오디오 형식은 공간 오디오를 지원하지 않습니다. 스트림 오디오가 다음 형식으로 인코딩되어 있는지 확인하세요 *mp3*, *AAC-LC* 또는 *FLAC*.


## 오디오 분석

다음에서 실시간 진폭 및 주파수 데이터를 읽어 `AudioStream` 이퀄라이저나 비트에 반응하는 조명 같은 반응형 비주얼을 구현할 수 있습니다. 다음을 참조하세요 [오디오 분석](/docs/creator/content-creator-ko/sdk7/media/audio-analysis/).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/media/audio-streaming.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
