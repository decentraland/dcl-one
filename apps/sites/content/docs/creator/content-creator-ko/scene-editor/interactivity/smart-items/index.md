> 씬에서 스마트 아이템을 사용하여 상호작용 기능을 추가합니다.

# 스마트 아이템 - 기본

씬 에디터의 카탈로그에 있는 일부 아이템은 **스마트 아이템입니다**. 플레이어는 이들과 상호작용할 수 있고, 구성 가능한 속성을 가지며, 다른 스마트 아이템에 동작을 트리거할 수 있습니다. 예를 들어 열고 닫을 수 있는 문, 위아래로 움직이는 플랫폼, 또는 다른 아이템을 활성화할 수 있는 버튼과 레버 등이 있습니다.



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/z7HF4GR01hE" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=z7HF4GR01hE>">Watch on YouTube</a></p>



자산 팩 탐색기에서는 이런 아이템에 번개 아이콘과 다른 색상의 배경이 있어서 알아볼 수 있습니다.

![](/docs/_remote/4401edd6b0ae269ede4c9467.webp)

엔티티 트리에서 번개 아이콘이 붙어 있으므로, 씬에 있는 어떤 아이템이 스마트 아이템인지 알아볼 수 있습니다.

![](/docs/_remote/d3ff99754b5e412ff6b047ee.webp)

## 아이템 사용하기

스마트 아이템을 사용하려면 다른 아이템처럼 씬으로 드래그하세요. 모든 아이템에는 기본 동작이 포함되어 있으니, 씬 미리보기를 실행해 직접 사용해 보세요.

다음은 몇 가지 일반적인 아이템과 기본 동작입니다:

* **문**: 클릭하면 문이 열리거나 닫힙니다. 이 동작을 바꿔 버튼, 트리거 영역 등으로 열리게 할 수 있습니다.
* **버튼**: 클릭하면 피드백으로 소리와 애니메이션을 재생합니다. 트리거 이벤트에 동작을 더 추가해 다른 스마트 아이템을 활성화할 수 있습니다.
* **레버**: 클릭하면 두 상태 사이를 전환합니다. 레버의 각 위치가 다른 스마트 아이템에 서로 다른 동작을 하도록 만들 수 있습니다.
* **상자**: 문처럼 동작하며, 기본적으로 클릭하면 열리거나 닫힙니다. 안에 더 작은 아이템을 넣을 수 있습니다.
* **플랫폼**: 두 위치 사이를 이동합니다. tween 동작을 사용해 이동 위치, 속도 등을 제어하세요.
* **트리거 영역**: 플레이어가 해당 영역에 들어가면 다른 스마트 아이템을 트리거할 수 있는 보이지 않는 아이템입니다. 참조 [트리거 영역 정보](#trigger-areas).
* **아바타 수정 영역**: 아바타를 숨기거나 여권 팝업을 비활성화하는 등, 내부에서 아바타의 동작이나 보이는 방식을 바꾸는 보이지 않는 영역입니다. 참조 [수정 영역](/docs/creator/content-creator-ko/scene-editor/interactivity/modifier-areas/).
* **카메라 수정 영역**: 내부에 있을 때 플레이어의 카메라를 1인칭 또는 3인칭으로 강제하는 보이지 않는 영역입니다. 참조 [수정 영역](/docs/creator/content-creator-ko/scene-editor/interactivity/modifier-areas/).
* **비디오 플레이어**: 비디오 또는 라이브 스트림을 보여 주는 화면입니다. 참조 [비디오 재생](#playing-videos).
* **오디오 스트림**: 라이브 스트림에서 오디오를 재생합니다. 참조 [오디오 스트림 재생](#playing-audio-streams)
* **NFT**: NFT 이미지를 초상화로 표시합니다. 참조 [NFT 표시](#displaying-nfts)
* **바닥 압력 버튼**: 플레이어가 그 위에 서 있으면 내려가고, 내려오면 다시 올라오는 원형 판입니다. 그 **누를 때** 및 **놓을 때** 훅을 사용해 동작을 실행하세요. 예를 들어 누군가 그 위에 서 있는 동안 문을 열어 둘 수 있습니다.
* **물 평면** 및 **물 원기둥**: 플레이어가 걸어 들어가거나 뛰어들면 물보라가 이는 물 표면입니다. 참조 [물](#water).

모든 스마트 아이템은 사용자 지정 방식으로 동작하도록 설정할 수 있습니다. 예를 들어 플랫폼이 얼마나 멀리 움직이는지, 또는 버튼이 무엇을 활성화하는지 등을 설정할 수 있습니다.

## 아이템 구성하기

씬 에디터에서 아이템을 선택하면 오른쪽에서 해당 아이템의 모든 속성을 볼 수 있습니다.

대부분의 스마트 아이템은 동작을 [스크립트 컴포넌트](/docs/creator/content-creator-ko/scene-editor/extend-with-code/script-component/)패널에 **스크립트** 보관합니다. 설정은 각 매개변수마다 한 필드씩 표시되며, 각 필드에는 무엇을 하는지 설명하는 툴팁이 있습니다. 사용하려면 코드를 작성할 필요가 없습니다.

많은 아이템에서 볼 수 있는 일반적인 필드는 다음과 같습니다:

* **호버 텍스트**: 플레이어가 커서를 아이템 위로 올렸을 때 UI에 힌트로 표시되는 텍스트입니다. 예를 들어 문에는 "열기"라고 표시될 수 있습니다.
* **상호작용**: 어떤 버튼으로 아이템이 활성화되나요? 일반적인 키보드에서는:
  * **주** 가 **E**
  * **보조** 가 **F**
  * **포인터** 가 **마우스 왼쪽 버튼**
  * **Action3** 키입니다 **1**
  * **Action4** 키입니다 **2**
  * **Action5** 키입니다 **3**
  * **Action6** 키입니다 **4**
* **아이템에 반응하는 훅**: 특정 순간에 실행할 동작을 선택하는 매개변수입니다. 어떤 훅이 있는지는 아이템이 무엇을 하는지에 따라 달라집니다. 버튼에는 **onClick**이 있고, 레버에는 **onActivate** 및 **onDeactivate**가 있으며, 움직이는 플랫폼에는 **onReachStart** 및 **onReachEnd**가 있고, 종에는 **onRing**이 있습니다. 훅을 비워 두면 그 순간에는 아무 일도 일어나지 않습니다.

각 아이템에는 고유한 특정 설정이 있으며, 아이템마다 달라질 수 있습니다.

몇몇 아이템은 여전히 이전의 **Config** 패널을 사용합니다. 예를 들어 조명은 여전히 그곳에서 색상과 강도를 설정합니다.

모든 아이템에는 **고급 모드** 가 있어서 거의 모든 것을 구성할 수 있습니다. 여기에는 어떤 소리를 재생할지, 플랫폼이 어느 방향으로 움직일지 같은 것들이 포함됩니다. 또한 플레이어를 텔레포트시키거나, 아바타 애니메이션을 재생하거나, 아이템을 플레이어의 손에 붙이는 등 다양한 동작을 포함하는 사용자 지정 동작을 추가할 수도 있습니다. 특정 상황에서만 무엇인가를 활성화하도록 조건 논리도 추가할 수 있습니다. 참조 [스마트 아이템 - 고급](/docs/creator/content-creator-ko/scene-editor/interactivity/smart-items-advanced/).

![](/docs/_remote/ff0b8ab0126b0a18e07ec387.webp)

## 다른 아이템에 동작 호출하기

스마트 아이템은 다른 스마트 아이템의 동작을 트리거할 수 있으므로, 아이템이 활성화될 때마다 그 동작이 실행됩니다. 씬에 있는 모든 아이템 목록에서 호출할 아이템을 선택한 다음 동작을 선택하기만 하면 됩니다. 아이템마다 노출하는 동작은 다릅니다.

예를 들어 버튼 하나가 문을 열거나 닫는 경우를 보세요. 버튼을 누를 때마다 문이 열리거나 닫힙니다.

![](/docs/_remote/564f4fe0ae016962310472b6.webp)

활성화하면 문을 열고, 비활성화하면 그 문을 닫는 레버입니다.

![](/docs/_remote/dc29efe9b4e6df7d2b39a714.webp)

서로 다른 아이템의 다양한 동작을 여러 개 한꺼번에 트리거하도록 추가할 수 있습니다. 그냥 **+ 동작 지정**.

동작 옆의 세 점을 클릭한 다음 *동작 제거*.

아이템이 제공하는 동작은 다음과 같은 익숙한 이름을 유지합니다. **Open**, **닫기**, **여기에 앉기**, **켜기**, 또는 **텔레포트**이므로, 아이템 간 연결은 늘 하던 방식대로 작동합니다.

동작을 연쇄적으로 연결할 수도 있습니다. 예를 들어 레버의 **onActivate** 가 문을 열고, 그 문의 훅이 또 다른 것을 트리거하면, 레버가 전체 체인을 간접적으로 작동시키는 셈입니다.

다음을 사용하면 [고급 모드](/docs/creator/content-creator-ko/scene-editor/interactivity/smart-items-advanced/) 이런 종류의 동작에도 조건 논리를 추가할 수 있습니다.

## 특수 스마트 아이템

몇몇 스마트 아이템은 일반적인 상황에서 매우 유용하게 쓸 수 있는 고유한 특징이 있습니다:

### 트리거 영역

플레이어가 영역에 들어올 때 동작을 트리거하려면 트리거 영역 스마트 아이템을 사용하세요.

![](/docs/_remote/9dcff76b0c7616640d8295ac.webp)

그것을 드래그해서 넣고, 상자나 구의 크기를 원하는 영역을 덮도록 조절한 다음, **플레이어가 들어오면…** 및 **플레이어가 나가면…** 그 안의 버튼을 **반응** 섹션에서 어떤 일이 일어나야 하는지 설명하세요.  [AI 도우미](/docs/creator/content-creator-ko/scene-editor/extend-with-code/ai-assistant/) 스크립트를 대신 작성해 줍니다. 참조 [트리거 영역](/docs/creator/content-creator-ko/scene-editor/interactivity/trigger-area/) 전체 안내를 보려면

![](/docs/_remote/83142e91c462787a36ca8b35.webp)

참조 [트리거 영역](/docs/creator/content-creator-ko/scene-editor/interactivity/trigger-area/) 자세한 내용은.

### 보이지 않는 벽

플레이어가 어떤 영역을 통과하거나 클릭하는 것을 막을 수 있는 보이지 않는 형태들의 모음입니다.

이 보이지 않는 벽은 충돌 메시가 없는 3D 모델을 가져올 때나, 플레이어에게 보이지 않는 벽을 만들고 싶을 때 유용합니다.

참조 [콜라이더](/docs/creator/content-creator-ko/sdk7/3d/colliders/) 자세한 내용은.

### 클릭 영역

플레이어가 클릭하여 다른 스마트 아이템의 동작을 트리거할 수 있는 보이지 않는 큐브입니다. 이 아이템은 다른 스마트 아이템에 의해 활성화되거나 비활성화될 수 있으며, 비활성화되면 클릭할 수 없습니다. 또한 플레이어가 커서를 올렸을 때 보이는 텍스트도 설정할 수 있습니다.

![](/docs/_remote/43e0c40165045ebe199d3264.webp)

### 좌석

의자, 벤치, 기타 좌석은 클릭하면 플레이어가 앉을 수 있게 해 줍니다.

* 좌석은 그 자리에 앉은 플레이어가 **멀어질 때만** 1.5미터 이상 떨어져야 비워집니다.
* 벤치처럼 여러 자리가 있는 아이템은 클릭하면 **가장 가까운 빈자리**.
* 모든 자리가 차면 호버 텍스트에 **좌석이 이미 사용 중입니다** 가 표시되고 클릭해도 아무 일도 일어나지 않습니다.

### 물

다음 아래의 두 아이템은 **머티리얼** 카테고리는 플레이어가 첨벙거릴 수 있는 물을 제공합니다:

* **물 평면**평평한 16 x 16m 표면입니다. 연못, 운하, 바다를 덮도록 크기를 조절하세요.
* **물 원기둥**지름 8m, 깊이 2m의 둥근 풀입니다.

둘 다 플레이어가 걸어 들어가거나 뛰어들면 물보라가 일어납니다. 다음을 설정할 수 있습니다:

* **활성화 대상**: `내 플레이어` 그래서 자기 물보라만 보이게 하거나 `모든 플레이어` 모든 아바타가 물보라를 일으키게 할 수 있습니다.
* **깊이**: 표면 아래 얼마나 깊게 물보라 영역이 도달하는지, 미터 단위입니다. 풀 바닥을 넘어서지는 않습니다.
* **물보라 크기**: 물보라 효과의 크기, 미터 단위입니다.
* **사운드**: 아이템 폴더 안의 오디오 파일 이름으로, 물보라가 생길 때마다 재생됩니다. 소리 없는 물로 하려면 비워 두세요.

또한 **진입** 및 **퇴장** 이벤트와, **첨벙** 물 한가운데에 필요할 때 물보라를 일으키는 동작입니다.


> [!WARNING]
> **📔 참고**: 각 물 레이어에는 노멀 맵이 필요합니다. 노멀 맵이 없는 레이어는 더 이상 애니메이션되지 않으며, 남은 것이 없으면 물보라 영역이 1 x 1m로 줄어듭니다.


### 텔레포트

해당 **텔레포트** 아이템은 플레이어가 상호작용하면 다른 곳으로 이동시킵니다.

다음을 설정하세요 **x** 및 **y** 매개변수를 사용해 Genesis City의 한 구역으로 보낼 수 있습니다. 대신 다른 곳으로 보내려면 [월드](/docs/creator/content-creator-ko/sdk7/publishing/publishing-options/#decentraland-worlds) 대신 다음을 채우세요 **월드** 매개변수에 월드 이름을 넣으세요. 예를 들어 `myname.dcl.eth`를 사용하고 좌표는 비워 두세요.


> [!WARNING]
> **📔 참고**: 예전에는 별도의 **월드 텔레포트** 아이템이 있었습니다. 이제 제거되었습니다. 대신 **텔레포트** 아이템을 해당 **월드** 매개변수와 함께 사용하세요.


### 이동 설정

해당 **이동 설정** 아이템은 씬 안에 있는 동안 플레이어가 얼마나 빠르게 움직이고 얼마나 높이 점프하는지를 바꿉니다. 씬을 더 둥둥 떠 있는 느낌으로 만들거나, 박물관에서 플레이어를 느리게 하거나, 넓은 지도에서 더 빠르게 이동하게 할 때 사용할 수 있습니다.

씬 어디에나 드래그해 넣으세요. 씬이 시작될 때 값이 적용되며, 자리표시자 큐브는 에디터에서만 보이고 플레이어에게는 보이지 않습니다.

각 필드는 슬라이더이며, Decentraland의 기본값으로 설정되어 있습니다:

| 필드                    | 기본값     | 범위    |
| --------------------- | ------- | ----- |
| `walkSpeed`           | 1.5 m/s | 0\~10 |
| `jogSpeed`            | 8 m/s   | 0\~30 |
| `runSpeed`            | 10 m/s  | 0\~50 |
| `jumpHeight`          | 1 m     | 0\~10 |
| `runJumpHeight`       | 1.5 m   | 0\~10 |
| `doubleJumpHeight`    | 2 m     | 0\~15 |
| `glidingSpeed`        | 6 m/s   | 0\~30 |
| `glidingFallingSpeed` | 1 m/s   | 0\~10 |
| `hardLandingCooldown` | 0.75 s  | 0\~5  |

조깅은 플레이어가 기본적으로 이동하는 속도입니다. 걷기와 달리기는 각각 control 키나 shift 키를 누르고 있을 때 사용됩니다.

이 아이템은 다른 스마트 아이템이 호출할 수 있는 두 가지 동작도 제공합니다:

* `apply`: 현재 필드 값을 플레이어에게 다시 적용합니다. 예를 들어 트리거 영역이나 버튼 뒤에 사용할 수 있습니다.
* `restoreDefaults`: 값을 지워 플레이어를 Decentraland의 기본값으로 되돌립니다.

참조 [이동 설정 및 제한](/docs/creator/content-creator-ko/sdk7/interactivity/avatars/locomotion/) 각 설정의 자세한 동작 방식과, 코드에서 이 값들을 바꾸는 방법에 대해서는

### 비디오 재생

다음에서 비디오를 재생할 수 있습니다:

* **로컬 파일**
* **URL에서 스트리밍**
* **다음에서 라이브 스트리밍** [**Decentraland Cast**](/docs/creator/content-creator-ko/scene-editor/operate-live/live-streaming/#dcl-cast-easy)
* **다음에서 라이브 스트리밍** [**RTMP 소프트웨어**](/docs/creator/content-creator-ko/scene-editor/operate-live/live-streaming/#stream-advanced) **(OBS, XSplit, StreamYard 등)**


> [!WARNING]
> **📔 참고**: 비디오 재생은 성능 부담이 크므로 동시에 재생되는 비디오 수를 적게 유지하세요. 엔진은 각 플레이어의 품질 설정에 따라 동시에 재생할 수 있는 비디오 수를 제한하며(낮음 1개, 중간 5개, 높음 10개), 그 제한을 넘는 비디오는 일시 정지합니다. 참조 [비디오 재생](/docs/creator/content-creator-ko/scene-editor/interactivity/video-screen/#play-videos).


참조 [비디오 재생](/docs/creator/content-creator-ko/scene-editor/interactivity/video-screen/) 자세한 내용은.

### 오디오 스트림 재생

다음을 사용해 URL에서 오디오 스트림을 재생하세요. **오디오 스트림** 스마트 아이템을 사용합니다.


> [!NOTE]
> **📔 참고**: 모든 스트리밍 서비스가 사이트 밖에서 오디오를 재생할 수 있게 허용하는 것은 아닙니다. Decentraland에서 작동하는 몇 가지 예시는 다음과 같습니다:
> 
> ```ts
> GRAFFITI = 'https://n07.radiojar.com/2qm1fc5kb.m4a?1617129761=&rj-tok=AAABeIR7VqwAilDFeUM39SDjmw&rj-ttl=5'
> SIGNS = 'https://edge.singsingmusic.net/MC2.mp3'
> DELTA = 'https://cdn.instream.audio/:9069/stream?_=171cd6c2b6e'
> JAZZ = 'https://live.vegascity.fm/radio/8010/the_flamingos.mp3'
> ```
> 


스트림의 볼륨을 조절할 수 있습니다. 스트림의 오디오는 위치 기반이 아니므로, 씬 전체에서 균일한 볼륨으로 들립니다.

### NFT 표시

액자에 NFT를 표시하려면 다음 스마트 아이템을 사용하세요. **NFT** 스마트 아이템을 사용하세요. 다음 필드를 제공해야 합니다:

* 네트워크


> [!NOTE]
> **📔 참고**: 현재 **ethereum** 가 유일하게 지원되는 네트워크입니다.


* NFT 컬렉션 컨트랙트: NFT 컬렉션의 스마트 컨트랙트입니다.
* 토큰 ID: 이 특정 NFT 수집품의 토큰 ID입니다.

<img src="/docs/_remote/9bae5521b60927140f632d59.webp" alt="NFT 모양" width="400">

이 정보는 다음에서 얻을 수 있습니다 [OpenSea](https://opensea.io)에서 **세부 정보** 탭을 확인하면 됩니다.

<img src="/docs/_remote/2ef80788a2ffef0f4c7b360b.webp" alt="OpenSea 세부 정보" width="400">


> [!NOTE]
> **📔 참고**: 이 정보는 opensea URL에서도 얻을 수 있습니다. 예를 들어 NFT의 URL이 다음과 같다면:
> 
> > `https://opensea.io/assets/ethereum/0x32b7495895264ac9d0b12d32afd435453458b1c6/1956`
> 
> 다음과 같이 필드를 채울 수 있습니다:
> 
> * 네트워크: ethereum
> * 컨트랙트: 0x32b7495895264ac9d0b12d32afd435453458b1c6
> * 토큰: 1956


배경색도 설정할 수 있으며, 특히 배경이 투명한 NFT에 유용합니다.

또한 다음도 선택할 수 있습니다 **프레임 스타일**, NFT를 다양한 클래식 및 모던 스타일로 액자에 넣을 수 있습니다.

참조 [NFT](/docs/creator/content-creator-ko/scene-editor/build/nfts/) 자세한 내용은.

### 체력 바

![](/docs/_remote/4b7b9bba4ad2a27559774acf.webp)

해당 **체력 바** 스마트 아이템은 여러 게임 메커니즘을 위한 훌륭한 구성 요소입니다. 다양한 방식으로 사용할 수 있습니다:

* 다음 아래에 중첩하면 **플레이어** 아바타 위에 플레이어의 체력을 표시합니다

  ![](/docs/_remote/7ec4155f634538c1243f6333.webp)
* 다음 아래에 중첩하면 **카메라** UI에 고정된 상태로 표시합니다

  ![](/docs/_remote/8f666b7f68658ee843a4304c.webp)
* 씬의 어떤 아이템 아래에라도 중첩하면 해당 아이템의 체력을 추적할 수 있습니다

  ![](/docs/_remote/1bc82b628c58c582bc684876.webp)

다른 아이템은 체력 바와 상호작용하여 체력을 더하거나 뺄 수 있습니다.

* 다음과 같은 아이템은 **가시** 또는 **로봇 적** 체력을 줄일 수 있습니다

  ![](/docs/_remote/8a3aa562c2ae8a2e3b67d7c6.webp)
* 다음과 같은 아이템은 **응급 처치** 또는 **치유 패드** 체력을 회복시킬 수 있습니다.

  ![](/docs/_remote/4b71eac34757d0b4ca625eca.webp)

체력이 0이 되었을 때 무엇이 일어날지 정의하려면 체력 바를 구성해야 합니다. 플레이어를 다음 아이템의 위치로 다시 생성할 수 있습니다. **리스폰 패드** 스마트 아이템, 점수 카운터 초기화, 적 재생성, UI 텍스트 표시 등 게임 로직에 맞는 아무 동작이나 설정할 수 있습니다.

또한 체력이 특정 값보다 낮을 때 동작을 트리거할 수도 있습니다. 예를 들어 체력이 3보다 낮아지면 특별한 음악을 재생하거나 UI 힌트를 표시할 수 있습니다.

체력 바는 무엇에든 영향을 주도록 설정할 수 있습니다! 예를 들어 다음 아래에 중첩된 체력 바를 추가하세요. **나무 문** 스마트 아이템. 이 바는 플레이어가 다음 아이템을 사용해 체력을 낮출 수 있습니다. **검** 스마트 아이템으로도, 다음의 폭발로도 **통** 또는 다음의 공격으로도 **로봇 적**. 이렇게 하려면 체력 바가 값이 0일 때 부모 아이템에 동작을 수행하도록 구성하세요.

![](/docs/_remote/03f177d0727bf4a6bb82243f.webp)

다음과 같은 무기는 **검** 플레이어가 집어 들 수 있으며, 동작을 수행할 때 플레이어 근처에 있는 체력 바가 있는 다른 아이템에 피해를 입히는 데 사용할 수 있습니다.

## 멀티플레이어

거의 모든 스마트 아이템에는 멀티플레이어 동작이 있어서, 아이템의 상태가 바뀔 때 씬의 모든 플레이어가 같은 경험을 공유합니다. 플레이어 A가 문을 열면 플레이어 B도 그 문이 열린 것을 봅니다. 그러고 나서 플레이어 C가 다른 플레이어들이 아직 있는 동안 씬에 들어오면, 그녀 역시 문이 이미 열린 것으로 보게 됩니다.

하지만 씬 근처에 플레이어가 아무도 없으면 씬은 기본 상태로 복원됩니다. 따라서 모든 플레이어가 떠났다가 플레이어 A가 돌아오면, 그녀는 문이 닫혀 있는 것을 보게 됩니다(그것이 문의 기본 상태였다면).

한 플레이어의 행동이 나중에 들어오는 다른 사람들에게 씬을 망치지 않도록 씬을 설계하세요. 예를 들어 씬이 퍼즐 게임이라면 *지연* 동작을 *도구* 스마트 아이템에 사용하여 퍼즐이 풀린 뒤 몇 초 후 씬의 모든 아이템이 초기 상태로 재설정되게 할 수 있습니다.

아이템의 멀티플레이어 동작도 비활성화할 수 있습니다. 참조 [스마트 아이템 - 고급](/docs/creator/content-creator-ko/scene-editor/interactivity/smart-items-advanced/).

## 문제 해결

* *내 씬의 아이템은 클릭 가능해야 하는데 클릭되지 않습니다*.

다른 무언가에 의해 가려져 있지 않은지 확인하세요. 다른 아이템을 통과해 클릭할 수는 없습니다. 일부 아이템에는 *콜라이더 메시* 간소화된 기하를 가진 것이 있어서, 보이는 모양은 그렇지 않아 보여도 아이템을 가로막고 있을 수 있습니다. 아이템을 옮겨서 어떻게 되는지 확인해 보세요.

## 또한 참조

* [스마트 아이템 - 고급](/docs/creator/content-creator-ko/scene-editor/interactivity/smart-items-advanced/)
* [상태 및 조건](/docs/creator/content-creator-ko/scene-editor/interactivity/states-and-conditions/)
* [어떤 항목이든 스마트 항목으로 만들기](/docs/creator/content-creator-ko/scene-editor/interactivity/make-any-item-smart/)
* [코드와 결합](/docs/creator/content-creator-ko/scene-editor/extend-with-code/overview/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/scene-editor/interactivity/smart-items.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
