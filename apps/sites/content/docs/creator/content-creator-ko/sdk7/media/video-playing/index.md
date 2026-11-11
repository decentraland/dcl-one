> 씬에 비디오 스트리밍

# 비디오 재생

씬에서 비디오를 표시하는 방법은 세 가지가 있습니다:

* 씬 콘텐츠의 일부로 비디오 파일 업로드
* 외부 소스에서 비디오 스트리밍
* Decentraland cast를 통해 라이브 스트리밍


> [!NOTE]
> **💡 팁**: 에서 [Creator Hub의 Scene Editor](/docs/creator/content-creator-ko/scene-editor/get-started/about-editor/), 이를 위해 다음을 사용할 수 있습니다 **비디오 플레이어** [스마트 아이템](/docs/creator/content-creator-ko/scene-editor/interactivity/smart-items/) 이를 노코드 방식으로 구현하는 방법입니다.


모든 경우에 다음이 필요합니다:

* 다음과 같은 [프리미티브 형태의](/docs/creator/content-creator-ko/sdk7/3d/shape-components/) 평면, 큐브, 또는 원뿔 같은 것.
* 다음 [머티리얼](/docs/creator/content-creator-ko/sdk7/3d/materials/) 에 `VideoTexture` 를 텍스처로 지정한
* 다음 `VideoPlayer` 비디오 상태를 제어하는 컴포넌트.

## 성능 고려 사항

스트리밍 비디오는 플레이어의 기기에 상당한 부담을 준다는 점을 기억하세요. 한 번에 하나 이상의 비디오를 재생하지 않는 것이 좋습니다.

씬에서 너무 많은 비디오가 동시에 재생되면 일부는 엔진에 의해 일시중지됩니다. 화면을 일시중지할 우선순위는 플레이어와의 거리, 크기, 화면이 플레이어의 시야 안에 있는지 여부를 포함한 여러 요소를 기준으로 결정됩니다. 동시에 재생 가능한 최대 비디오 수는 플레이어의 품질 설정에 따라 달라집니다.

* 낮음: 1
* 보통: 5
* 높음: 10

플레이어가 가까이 있을 때 또는 그런 동작을 수행했을 때 동영상 재생을 시작하는 것도 권장합니다. 씬이 지평선 저편에서 로드될 때 동영상을 재생하기 시작하면 플레이어가 인근 씬을 방문하는 동안 성능에 불필요한 영향을 미칩니다.

또한 매우 높은 해상도의 비디오 스트리밍도 피하세요. 다음을 초과하는 것은 사용하지 마세요 *HD*.

아래의 모든 예제 스니펫과 마찬가지로, 성능 부담을 줄이기 위해 비디오는 Basic(비조명) 머티리얼에서 재생하는 것이 가장 좋습니다.

## 비디오 표시

다음 지침은 세 가지 비디오 표시 옵션 모두에 적용됩니다:

1. 비디오 스크린 역할을 할 엔티티를 만듭니다. 이 엔티티에 `MeshRenderer` 컴포넌트를 부여하여 눈에 보이는 형태를 갖게 합니다.
2. 다음 `VideoPlayer` 컴포넌트를 만듭니다. 스트리밍 URL 또는 비디오 파일 경로를 참조할 수 있습니다. 여기서 비디오의 `재생 중인` 상태와 볼륨도 설정할 수 있습니다. 이 컴포넌트는 비디오 스크린 엔티티나 씬의 다른 어떤 엔티티에도 할당할 수 있습니다.
3. 다음 `VideoTexture` 객체의 `videoPlayerEntity` 속성에 `VideoPlayer` 컴포넌트가
4. 다음 `머티리얼`, 스크린 엔티티에 할당하고 `텍스처를` 부터 `VideoTexture` 만든 것입니다.

이 예제는 로컬에 저장된 비디오를 사용합니다 `/videos` 씬 프로젝트의 폴더에 있는:

```ts
// #1
const screen = engine.addEntity()
MeshRenderer.setPlane(screen)
Transform.create(screen, { position: { x: 4, y: 1, z: 4 } })

// #2
VideoPlayer.create(screen, {
	src: 'videos/myVideo.mp4',
	playing: true,
})

// #3
const videoTexture = Material.Texture.Video({ videoPlayerEntity: screen })

// #4
Material.setBasicMaterial(screen, {
	texture: videoTexture,
})
```

외부 스트리밍 URL의 비디오를 사용하려면, 2단계에서 `src` 다음의 속성 `VideoPlayer` 컴포넌트가 스트리밍 URL을 참조하도록 변경하세요.

```ts
// #2
VideoPlayer.create(screen, {
	src: 'https://vz-7c61c1b5-d59.b-cdn.net/ccea595a-b910-4de6-b160-092819db021d/playlist.m3u8',
	playing: true,
})
```

다음을 참조하세요. [라이브 스트리밍](#live-streaming) 이 세 번째 대체 방법을 사용하는 방법에 대한 자세한 내용은

## 외부 스트리밍에 대하여

그리고 스트림 URL을 다음에 붙여넣으세요 *https*  URL (*http* URL은 지원되지 않습니다).

대부분의 인기 있는 비디오 스트리밍 사이트의 비디오 URL을 붙여넣을 수 있어야 합니다. 이러한 플랫폼의 서비스 약관을 주의하세요.

로컬 컴퓨터에 있는 비디오 파일을 스트리밍하려면, 가장 쉬운 방법은 이 비디오를 공개 Google Drive에 업로드하고 링크를 붙여넣는 것입니다.

또 다른 옵션은 다음과 같은 관리형 호스팅 제공업체를 사용하는 것입니다 [Vimeo](https://vimeo.com/) , [Livepeer Studio](https://livepeer.studio/) 또는 [Serraform](https://serraform.gitbook.io/streaming-docs/guides/decentraland-playback) 여기서는 스트리밍 인프라 전반을 관리하도록 제공업체에 수수료를 지불합니다.

가장 강력한 방법은 다음과 같은 플랫폼에서 호스팅 비용을 지불하면서 무료 소프트웨어를 사용해 직접 서버를 설정하는 것입니다 [Digital Ocean](https://try.digitalocean.com/developerbrand/?_campaign=emea_brand_kw_en_cpc&_adgroup=digitalocean_exact_exact&_keyword=digitalocean&_device=c&_adposition=&_content=conversion&_medium=cpc&_source=bing&msclkid=160bfc160a2a1bab9bbf9933594bd9c5&utm_source=bing&utm_medium=cpc&utm_campaign=emea_brand_kw_en_cpc&utm_term=digitalocean&utm_content=DigitalOcean%20Exact_Exact) 또는 [Cloudflare](https://www.cloudflare.com/products/cloudflare-stream/). 다음과 같은 것을 배포할 수 있습니다 [Node Media Server](https://github.com/illuspas/Node-Media-Server), 이는 필요한 대부분의 기능을 기본 제공으로 제공합니다.

이 모든 옵션은 각기 다른 상황에서 장단점이 있습니다. 필요, 기술 수준, 예산을 고려해 자신에게 가장 적합한 것을 평가해야 합니다.

## 성공적인 스트리밍을 위한 OBS 설정

[OBS](https://obsproject.com/) 는 스트림을 관리하는 데 널리 쓰이는 무료 도구입니다.

장소의 스트림 키를 사용하든 자신의 RTMP 서버를 사용하든, OBS 설정은 스트림 성공에 중요합니다. 안정적이고 일관된 연결을 목표로 해야 합니다.

### 간단한 OBS 설정

다음의 간단한 설정을 권장합니다:

* 비트레이트 2500kbps(모든 Decentraland 장소에서 작동함)
* 오디오 비트레이트 160kbps
* 비디오 인코더 프리셋: Hardware NVENC
* 오디오 인코더 AAC
* 최대 해상도: 720(그보다 높으면 DCL에서 문제가 발생함)
* 프레임 속도 30fps


> [!WARNING]
> **📔 참고**: OBS에서 멀티트랙 스트리밍을 비활성화하세요. Settings > Stream에서 찾을 수 있습니다.


### 새로운 스트리머를 위한 조언

* 초기 음향 점검은 장소와 함께 설정을 테스트하는 데 필수적입니다.
* 스트림 키에 숫자 하나만 잘못 들어가도 스트림이 망가질 가능성이 가장 큽니다.
* 720 해상도 또는 2500 kbps 비트레이트를 넘기지 마세요.

## 라이브 스트리밍

다음을 사용해 카메라로 라이브스트리밍하거나 화면을 공유할 수 있습니다 [라이브 스트리밍](/docs/creator/content-creator-ko/scene-editor/operate-live/live-streaming/) 의 [관리자 도구](/docs/creator/content-creator-ko/scene-editor/operate-live/scene-admin/) 스마트 아이템 기능.

이 스트리밍 방식은 플레이어 간 라이브 통신에 사용되는 것과 동일한 통신 아키텍처를 사용합니다. 설정이 쉽고 외부 소스에서 스트리밍하는 것보다 지연이 훨씬 적습니다.

1. 추가하세요 [관리자 도구](/docs/creator/content-creator-ko/scene-editor/operate-live/scene-admin/) 씬에 스마트 아이템을, 그리고 [비디오 플레이어](/docs/creator/content-creator-ko/scene-editor/interactivity/video-screen/) 스마트 아이템 기능.
2. 씬을 World 또는 Genesis City에 게시하세요.
3. Admin 도구를 사용할 권한이 있는 플레이어로 씬에 입장하세요.
4. Admin 콘솔을 열고 **비디오** 탭을 선택한 다음 **라이브** 기능을 선택하고 **스트림 키 받기** 버튼을 클릭합니다.
5. 다음 파일을 복사하세요. **서버 URL** 및 *스트리밍 키*\*를 스트리밍 소프트웨어(예: OBS)에 입력하세요.
6. 다음을 누르세요 **활성화** 버튼을 눌러 스트리밍을 시작하세요.

씬에 Video player 스마트 아이템을 추가하는 대신, URL `livekit-video://current-stream` 을 비디오 소스로 사용해 씬에서 스트림을 재생할 수도 있습니다. 스트림 키를 가져오려면 여전히 Admin 도구가 필요합니다.

```ts
// #1
const screen = engine.addEntity()
MeshRenderer.setPlane(screen)
Transform.create(screen, { position: { x: 4, y: 1, z: 4 } })

// #2
VideoPlayer.create(screen, {
	src: `livekit-video://current-stream`,
	playing: true,
})

// #3
const videoTexture = Material.Texture.Video({ videoPlayerEntity: screen })

// #4
Material.setBasicMaterial(screen, {
	texture: videoTexture,
})
```

## 비디오 머티리얼

대부분의 경우, 비디오는 PBR 머티리얼이 아니라 조명이 없는 [Basic 머티리얼](/docs/creator/content-creator-ko/sdk7/3d/materials/#unlit-materials)에서 재생하는 것이 좋습니다. 이렇게 하면 훨씬 더 밝고 선명한 이미지가 되며 성능에도 더 좋습니다.

```ts
Material.setBasicMaterial(screen, {
	texture: videoTexture,
})
```

일반적으로는 성능에 더 좋기 때문에 Basic 비조명 머티리얼에서 비디오를 재생하는 것이 권장됩니다. 하지만 비디오를 PBR 머티리얼에 투사하고 싶다면, 기본 속성으로는 비디오가 다소 불투명하게 보인다는 점을 기억하세요. 머티리얼의 다른 속성을 조정해 이를 개선할 수 있습니다. 비디오를 더 잘 드러나게 만드는 데 권장되는 설정은 다음과 같습니다:

```ts
Material.setPbrMaterial(screen, {
	texture: videoTexture,
	roughness: 1.0,
	specularIntensity: 0,
	metallic: 0,
	emissiveTexture: videoTexture,
	emissiveIntensity: 0.6,
	emissiveColor: Color3.White(),
})
```


> [!NOTE]
> **💡 팁**: 비디오는 머티리얼에 추가되는 텍스처이므로, 색으로 틴트를 주거나 다른 텍스처 레이어를 추가하는 등 다른 머티리얼 속성도 실험해 볼 수 있습니다. 예를 들어 더럽혀진 화면 효과를 만들 수 있습니다.
> 
> 다음을 참조하세요. [머티리얼](/docs/creator/content-creator-ko/sdk7/3d/materials/) 을 참고하세요.


## 비디오 파일 소개

다음 파일 형식을 지원합니다:

* *.mp4*
* *.ogg*
* *.webm*

비디오 파일은 씬의 전체 크기에 추가되므로, 플레이어가 씬에 들어올 때 다운로드하는 데 더 오래 걸립니다. 비디오 크기로 인해 다음 한도를 초과할 수도 있습니다 [씬 제한](/docs/creator/content-creator-ko/sdk7/optimizing/scene-limitations/), parcel당 최대 15MB만 사용할 수 있기 때문입니다. 비디오를 가능한 한 많이 압축하여 문제를 줄이는 것을 권장합니다.

플레이어가 가까이 있을 때나 트리거하는 동작을 했을 때 비디오 재생을 시작하는 것도 권장합니다. 씬이 멀리 있는 지평선에 로드된 상태에서 비디오를 재생하기 시작하면 플레이어가 인접한 씬을 방문하는 동안 불필요하게 성능에 영향을 줍니다.

## 비디오 시작, 일시중지, 정지

비디오를 재생하거나 일시중지하려면 `재생 중인` 속성을 *true* 또는 *false*. 만약 `재생 중인` 가 false로 설정되면 비디오는 마지막으로 표시된 프레임에서 일시중지됩니다.

아래와 같이 포인터 이벤트를 추가해 화면을 토글 가능하게 만들 수 있습니다:

```ts
pointerEventsSystem.onPointerDown(
	{
		entity: screen,
		opts: { button: InputAction.IA_POINTER, hoverText: 'Play/Pause' },
	},
	function () {
		const videoPlayer = VideoPlayer.getMutable(screen)
		videoPlayer.playing = !videoPlayer.playing
	}
)
```

비디오를 정지하고 첫 프레임으로 되돌리려면 `position` 속성을 0으로 설정하세요. 다음 예제에서는 비디오를 클릭하면 정지합니다.

```ts
pointerEventsSystem.onPointerDown(
	{
		entity: screen,
		opts: { button: InputAction.IA_POINTER, hoverText: 'STOP' },
	},
	function () {
		const videoPlayer = VideoPlayer.getMutable(screen)
		videoPlayer.playing = false
		videoPlayer.position = 0
	}
)
```

## 비디오 플레이어 구성하기

다음의 선택적 속성을 `VideoPlayer` 컴포넌트에 설정할 수 있습니다:

* `재생 중인`: 비디오가 현재 재생 중인지 결정합니다. false이면 비디오는 일시중지됩니다.


> [!WARNING]
> **📔 참고**: 플레이어의 품질 설정에 따라 동시에 재생할 수 있는 비디오 수가 제한됩니다. 씬이 그보다 더 많이 재생하면 엔진이 우선순위가 가장 낮은 것들을 일시중지합니다. 다음을 참조하세요 [성능 고려 사항](#performance-considerations).


* `playbackRate`: 비디오 재생 속도를 변경합니다. *1* 기본값으로.
* `volume`: 오디오 볼륨을 변경할 수 있습니다. *1* 기본값으로.
* `position`: 비디오의 다른 시작 위치를 설정할 수 있게 합니다. 비디오 원래 시작점 이후의 초 단위로 표현됩니다. *0* 기본값은
* `loop`: 비디오를 반복 재생할지, 한 번 재생 후 멈출지를 결정하는 불리언입니다. *false* 기본값으로.

## 여러 비디오 재생하기

성능 문제를 피하려면 동시에 재생되는 서로 다른 비디오 수를 낮게 유지하세요. 엔진은 플레이어의 품질 설정에 따라 동시에 재생할 수 있는 비디오 수를 제한합니다( [성능 고려 사항](#performance-considerations)). 하지만 씬은 여러 다른 스크린에서 동일한 비디오 텍스처의 여러 복사본을 재생할 수 있습니다. 이는 별도의 비디오를 재생하는 것보다 성능 영향이 훨씬 적으므로 제한되지 않습니다. 같은 비디오를 여러 엔티티에서 재생하려면 비디오 텍스처 객체의 같은 인스턴스를 각 스크린 엔티티의 `머티리얼` 컴포넌트에 할당하면 됩니다.

```ts
// #1
const screen1 = engine.addEntity()
MeshRenderer.setPlane(screen1)
Transform.create(screen1, { position: { x: 4, y: 1, z: 4 } })

const screen2 = engine.addEntity()
MeshRenderer.setPlane(screen2)
Transform.create(screen2, { position: { x: 6, y: 1, z: 4 } })

// #2
VideoPlayer.create(screen1, {
	src: 'https://vz-7c61c1b5-d59.b-cdn.net/ccea595a-b910-4de6-b160-092819db021d/playlist.m3u8',
	playing: true,
})

// #3
const videoTexture = Material.Texture.Video({ videoPlayerEntity: screen1 })

// #4
Material.setBasicMaterial(screen1, {
	texture: videoTexture,
})

Material.setBasicMaterial(screen2, {
	texture: videoTexture,
})
```

위 예제에서는 하나의 `VideoPlayer` 컴포넌트만 만들면 된다는 점에 유의하세요. 이 컴포넌트가 두 비디오 화면의 상태를 모두 제어합니다. 이 경우 이 컴포넌트는 `screen1` 엔티티에 할당되지만, 반드시 화면 중 하나일 필요는 없고 씬의 다른 어떤 엔티티에도 할당할 수 있습니다.

## 비디오 이벤트

비디오가 재생 시작, 일시중지 등 상태가 바뀔 때 쉽게 처리하세요. 예를 들어 애니메이션이 비디오와 완전히 동기화되도록, 비디오가 시작되는 동시에 애니메이션을 재생하는 데 사용할 수 있습니다.

다음을 사용하세요 `videoEventsSystem.registerVideoEventsEntity` 를 사용해 엔티티에 할당된 비디오의 상태가 바뀔 때마다 실행되는 함수를 정의합니다. 상태가 바뀔 때마다 함수는 새 상태를 확인하고 그에 맞게 응답할 수 있습니다.

```ts
import {
	engine,
	Entity,
	VideoPlayer,
	videoEventsSystem,
	VideoState,
} from '@dcl/sdk/ecs'

// ... VideoPlayer 컴포넌트, Transform, MeshRenderer.setPlane() 등을 사용해 videoPlayerEntity를 생성합니다. ...

videoEventsSystem.registerVideoEventsEntity(
	videoPlayerEntity,
	function (videoEvent) {
		console.log(
			'비디오 이벤트 - 상태: ' +
				videoEvent.state +
				'\n현재 오프셋:' +
				videoEvent.currentOffset +
				'\n비디오 길이:' +
				videoEvent.videoLength
		)

		switch (videoEvent.state) {
			case VideoState.VS_READY:
				console.log('비디오 이벤트 - 비디오가 준비됨')
				break
			case VideoState.VS_NONE:
				console.log('비디오 이벤트 - 비디오에 상태가 없음')
				break
			case VideoState.VS_ERROR:
				console.log('비디오 이벤트 - 비디오 오류')
				break
			case VideoState.VS_SEEKING:
				console.log('비디오 이벤트 - 비디오 탐색 중')
				break
			case VideoState.VS_LOADING:
				console.log('비디오 이벤트 - 비디오 로딩 중')
				break
			case VideoState.VS_BUFFERING:
				console.log('비디오 이벤트 - 비디오 버퍼링 중')
				break
			case VideoState.VS_PLAYING:
				console.log('비디오 이벤트 - 비디오 재생 시작')
				break
			case VideoState.VS_PAUSED:
				console.log('비디오 이벤트 - 비디오가 일시중지됨')
				break
		}
	}
)
```

함수의 입력으로 전달되는 videoEvent 객체에는 다음 속성이 포함됩니다:

* `currentOffset` (*숫자*): 비디오의 `position` 속성의 현재 값입니다. 이 값은 비디오의 원래 시작 시점 이후의 초를 나타냅니다. *-1* 비디오가 아직 재생을 시작하지 않은 경우.
* `state`: 새 비디오 상태입니다. 다음의 값으로 표현됩니다 `VideoState` 열거형의 값으로 표현됩니다. 이 열거형에는 다음과 같은 가능한 값이 있습니다.
  * `VideoState.VS_READY`
  * `VideoState.VS_NONE`
  * `VideoState.VS_ERROR`
  * `VideoState.VS_SEEKING`
  * `VideoState.VS_LOADING`
  * `VideoState.VS_BUFFERING`
  * `VideoState.VS_PLAYING`
  * `VideoState.VS_PAUSED`
* `videoLength` (*숫자* ): 전체 비디오의 길이(초)입니다. *-1* 길이를 알 수 없는 경우.
* `timestamp` ( *숫자*): A *람포트* 타임스탬프입니다. 비디오가 상태를 바꿀 때마다 증가합니다.
* `tickNumber` (*숫자*): 씬이 실행되기 시작한 이후의 틱 수로 표현되는 이벤트 발생 시각입니다.

이미 콜백이 있는 엔티티에 콜백을 다시 등록하면(예: 상태 변경 시 동작을 바꾸기 위해), 새 콜백이 이전 콜백을 대체하고 마지막으로 보고된 상태는 유지됩니다. 즉 이전 콜백에 이미 보고된 상태에 대해서는 새 콜백이 다시 호출되지 않습니다.

### 엔티티가 등록되어 있는지 확인

다음을 사용하세요 `videoEventsSystem.hasVideoEventsEntity()` 를 사용해 엔티티에 이미 등록된 비디오 이벤트 콜백이 있는지 확인하세요.

```ts
if (videoEventsSystem.hasVideoEventsEntity(videoPlayerEntity)) {
	console.log('이 엔티티에는 이미 비디오 이벤트 리스너가 있습니다')
}
```

### 최신 비디오 이벤트

사용해 비디오의 마지막 상태 변경을 조회하세요 `videoEventsSystem.getVideoState()`. 이 함수는 엔티티의 최신 `VideoEvent` 값을 반환하거나 `undefined` 아직 보고된 이벤트가 없으면 undefined를 반환합니다. 콜백을 등록하지 않고도 시스템에서 이 함수를 호출해 매 프레임 비디오 상태를 폴링할 수 있습니다.

```ts
function mySystem() {
	const latestVideoEvent = videoEventsSystem.getVideoState(videoPlayerEntity)
	if (!latestVideoEvent) return

	console.log(`state: ${latestVideoEvent.state}
    \ncurrentOffset: ${latestVideoEvent.currentOffset}
    \nvideoLength: ${latestVideoEvent.videoLength}`)
}
```

## 비디오의 알파 마스크

직사각형이 아닌 비디오 화면을 만드는 멋진 방법은 평면 위에 알파 텍스처를 적용하는 것입니다. 원하는 어떤 모양으로든 평면의 일부를 잘라낼 수 있습니다.

다음 이미지를 사용해 비디오를 원형 모양으로 자르고, 모서리는 투명하게 처리하세요.

![](/docs/_remote/1f517d241f0753831e85b88c.webp)

```ts
const videoTexture = Material.Texture.Video({
	videoPlayerEntity: screen,
})
const alphaMask = Material.Texture.Common({
	src: 'assets/scene/circle_mask.png',
	wrapMode: TextureWrapMode.TWM_MIRROR,
})

Material.setBasicMaterial(screen, {
	texture: videoTexture,
	alphaTexture: alphaMask,
})
```

![](/docs/_remote/9b242e1d8862ee66017fbceb.webp)


> [!WARNING]
> **📔 참고**: 이전 버전에서는 `alphaTexture` 속성은 PBR 머티리얼에만 있었습니다. 현재는 Basic 머티리얼에서만 작동합니다.


## glTF 모델에서 비디오 재생하기

다음 *glTF* 모델에서 비디오를 재생할 수 있습니다. [GltfNodeModifiers](/docs/creator/content-creator-ko/sdk7/3d/materials/#modify-gltf-materials) 컴포넌트를 사용하여 모든 플레이어를 순회하세요. 자세한 내용은 [glTF 머티리얼 수정](/docs/creator/content-creator-ko/sdk7/3d/materials/#modify-gltf-materials) 을 참고하세요.

이를 통해 평면뿐 아니라 어떤 형태에서도 비디오를 재생할 수 있습니다. 예를 들어 곡면 스크린이나 NPC의 몸 전체에도 비디오를 재생할 수 있습니다.

```ts
const myEntity = engine.addEntity()

GltfContainer.create(myEntity, {
	src: 'models/myModel.glb',
})

Transform.create(myEntity, {
	position: Vector3.create(4, 0, 4),
})

VideoPlayer.create(myEntity, {
	src: 'https://vz-7c61c1b5-d59.b-cdn.net/ccea595a-b910-4de6-b160-092819db021d/playlist.m3u8',
	playing: true,
})

GltfNodeModifiers.create(myEntity, {
	modifiers: [
		{
			path: '',
			material: {
				material: {
					$case: 'pbr',
					pbr: {
						texture: Material.Texture.Video({
							videoPlayerEntity: myEntity,
						}),
					},
				},
			},
		},
	],
})
```

비디오의 매핑은 모델이 사용하는 원래 UV 매핑을 따릅니다. 즉 모델에 특정 부분에 매핑된 텍스처가 있다면, 비디오는 그 동일한 부분에 매핑됩니다.

다음 `GltfNodeModifiers` 컴포넌트를 사용해 모델 내부의 특정 메시에서만 비디오를 재생할 수도 있습니다. 예를 들어 모델이 건물 전체를 차지하더라도 건물의 특정 벽에만 비디오를 재생할 수 있습니다. 다음을 참조하세요 [glTF 머티리얼 수정](/docs/creator/content-creator-ko/sdk7/3d/materials/#modify-gltf-materials) 을 참고하세요.

## 공간 오디오

기본적으로 `VideoPlayer` 컴포넌트의 비디오는 전역입니다. 즉 씬 전체에서 일정한 볼륨으로 들립니다. 플레이어가 씬을 벗어나면 스트리밍을 전혀 들을 수 없습니다.

오디오를 공간 오디오로 만들려면 `spatial` 속성을 *true*.

```ts
VideoPlayer.create(entity, {
	src: 'https://player.vimeo.com/progressive_redirect/playback/1145666916/rendition/540p/file.mp4%20%28540p%29.mp4?loc=external&signature=db1cd6946851313cb8f7be60d1f6c30af0902bcc46fdae0ba2a06e5fdf44c329',
	playing: true,
	spatial: true,
})
```

이제 비디오는 `VideoPlayer` 컴포넌트를 소유한 엔티티의 위치에서 들리며, 플레이어가 가까워질수록 더 크게 들립니다.

다음 속성으로 공간 오디오를 제어합니다:

* `spatialMinDistance`: 오디오가 공간 오디오가 되기 시작하는 최소 거리입니다. 플레이어가 더 가까우면 오디오는 최대 볼륨으로 들립니다. *0* 기본값으로.
* `spatialMaxDistance`: 오디오가 들리는 최대 거리입니다. 플레이어가 더 멀면 오디오는 0 볼륨으로 들립니다. *60* 기본값

```ts
const videoPlayerEntity = engine.addEntity()

Transform.create(videoPlayerEntity, {
	position: Vector3.create(8, 2, 8),
})

VideoPlayer.create(videoPlayerEntity, {
	src: 'https://player.vimeo.com/progressive_redirect/playback/1145666916/rendition/540p/file.mp4%20%28540p%29.mp4?loc=external&signature=db1cd6946851313cb8f7be60d1f6c30af0902bcc46fdae0ba2a06e5fdf44c329',
	playing: true,
	spatial: true,
	spatialMinDistance: 5,
	spatialMaxDistance: 10,
})

MeshRenderer.setPlane(videoPlayerEntity)

Material.setBasicMaterial(videoPlayerEntity, {
	texture: Material.Texture.Video({ videoPlayerEntity: videoPlayerEntity }),
})
```


> [!WARNING]
> **📔 참고**: 일부 비디오 형식은 공간 오디오를 지원하지 않습니다. 스트림이 다음으로 인코딩되었는지 확인하세요 *mp4*, *m4a*또는 *mov*.


## 오디오 분석

오디오 트랙에서 실시간 진폭 및 주파수 데이터를 읽을 수 있습니다 `VideoPlayer` 비디오 사운드트랙과 동기화되는 반응형 비주얼을 구동하는 엔티티입니다. 참고 [오디오 분석](/docs/creator/content-creator-ko/sdk7/media/audio-analysis/).


> [!NOTE]
> **💡 팁**: 프리미티브가 아니라 GLTF 모델에서 재생되는 비디오를 보려면 다음을 참조하세요 [`74,-8-gltfnodemodifier`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/74,-8-gltfnodemodifier) 특정 GLTF 노드에 HLS 스트림을 적용하는 테스트 장면으로, `GltfNodeModifiers` 비디오 텍스처.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/media/video-playing.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
