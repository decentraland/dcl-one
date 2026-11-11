> 씬이 빠르게 로드되고 모든 플레이어에게 원활하게 실행되도록 최적화합니다.

# 성능 최적화

플레이어가 방문했을 때 가능한 최고의 경험을 보장하기 위해 씬에서 최적화할 수 있는 여러 요소가 있습니다. 이 문서는 씬이 얼마나 빠르게 로드되는지, 그리고 그 씬이나 인접한 씬에 있는 플레이어에게 얼마나 부드럽게 실행되는지에 큰 차이를 만들 수 있는 몇 가지 모범 사례를 다룹니다.

많은 플레이어가 게임용이 아닌 하드웨어, 브라우저를 통해, 또는 [모바일 앱](/docs/creator/content-creator-ko/build-for-mobile/mobile-client/overview/) 휴대폰으로 Decentraland를 방문할 수 있다는 점을 염두에 두세요. 이 모든 경우 씬에 사용할 수 있는 처리 성능이 제한됩니다. 씬을 방문하는 경험은 모든 사람에게 부드러워야 합니다.



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/tc1PwYKW1Kc" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=tc1PwYKW1Kc>">Watch on YouTube</a></p>



확인해 보세요 [유용한 자료](/docs/creator/content-creator-ko/sdk7/getting-started/useful-resources/) 도움이 되는 도구들을. 예를 들어 Decentraland Scene Optimizer는 씬의 3D 모델에서 텍스처를 추출하고, 중복을 제거하고, 압축합니다.


> [!NOTE]
> **📱 모바일**: 모바일 기기는 보통 가장 자원이 제한된 클라이언트입니다. 씬이 모바일 플레이어를 대상으로 한다면 [모바일용으로 빌드하기](/docs/creator/content-creator-ko/build-for-mobile/mobile-client/overview/) 모바일 전용 지침도 확인해 보세요.


Decentraland 익스플로러는 엔진 수준에서 많은 최적화를 강제합니다. 이러한 최적화는 큰 차이를 만들지만, 브라우저에서 여러 사용자 생성 경험을 동시에 렌더링하는 문제는 매우 큽니다. 원활하게 실행될 수 있도록 여러분의 도움이 필요합니다.

## 타이밍

### 비디오 재생

비디오 재생은 엔진이 처리해야 하는 작업 중 가장 비용이 큰 것 중 하나입니다. 씬에 비디오가 포함되어 있다면, 반드시 *하나만* 한 번에 사용 중이 되도록 하세요. 수십 개의 평면이 같은 VideoTexture를 공유하더라도 성능에는 큰 영향이 없지만, 두 번째 VideoTexture를 추가하는 순간 프레임률에 미치는 영향이 매우 눈에 띄게 됩니다.

또한 보이지 않는 영역에서 비디오가 재생되지 않도록 해야 합니다. 예를 들어 실내에 화면이 있다면, 플레이어가 들어오고 나갈 때를 기준으로 트리거 영역을 사용해 비디오를 켜고 끄세요.


> [!NOTE]
> **💡 팁**: 여러 씬에서 사용해 온 한 가지 요령은, 서로 다른 평면에 다르게 매핑된 여러 영역을 가진 단일 비디오를 스트리밍하는 것입니다. 각 비디오 화면은 [UV 매핑](/docs/creator/content-creator-ko/sdk7/3d/materials/#using-textures) 을 사용해 VideoTexture의 특정 부분만 표시합니다. 덕분에 여러 VideoTexture를 사용하는 비용 없이도 별도의 비디오가 재생되는 것처럼 보이게 할 수 있습니다.



> [!NOTE]
> **💡 팁**: 플레이어가 씬 밖에 서 있을 때는 VideoTexture가 매 프레임마다 업데이트되지 않습니다. 이는 주변 씬에 미치는 영향을 줄이는 데 도움이 됩니다. 그럼에도 불구하고, 비디오 재생은 플레이어가 [당신의 씬 안으로 들어올 때만 켜는 것이 이상적입니다.](/docs/creator/content-creator-ko/sdk7/interactivity/event-listeners/#player-enters-or-leaves-scene) .


### 지연 로딩

씬이 크거나 항상 보이지 않는 실내 구역이 있다면, 시작 시점부터 모든 엔티티를 전부 로드하지 않도록 선택할 수 있습니다. 대신 플레이어가 씬의 다른 부분을 방문할 때마다 영역별로 콘텐츠를 로드하세요. 이렇게 하면 씬의 로드 시간을 크게 줄일 수 있고, 엔진이 매 프레임 처리해야 하는 텍스처와 3D 콘텐츠의 양도 줄일 수 있습니다.

예를 들어, 박물관의 본관은 처음부터 로드하되 각 층의 그림은 플레이어가 해당 층을 방문할 때만 로드할 수 있습니다.

참조 [이 예시 씬에서](https://github.com/decentraland-scenes/lazy-loading) 그 방식이 어떻게 작동할 수 있는지 확인해 보세요.

끊김을 피하는 관점에서 가장 좋은 결과를 얻으려면, 엔티티의 shape의 `visible` 속성을 false로 바꿔서 엔티티를 숨기세요. 이 방법을 쓰면 생성할 때 엔진에 추가하되, 모델을 보이게만 하지 않으면 됩니다.

대안으로는 필요할 때까지 엔티티를 엔진에 추가하지 않는 방법도 있습니다. 이 경우 엔티티가 처음 나타날 때 약간의 끊김이 생길 수 있고, 보이기까지 몇 초가 걸릴 수도 있습니다. 이 접근법의 장점은 [씬 제한 사항](/docs/creator/content-creator-ko/sdk7/optimizing/scene-limitations/). 씬 제한 수치는 언제든 씬에서 렌더링되고 있는 콘텐츠를 기준으로 하며, 렌더링될 수 있는 총 콘텐츠를 기준으로 하는 것은 아니라는 점을 기억하세요. 씬의 일부를 로드하고 언로드하면 이러한 제한을 우회할 수 있어야 합니다.


> [!WARNING]
> **📔 참고**: 보이지 않지만 엔진에 추가된 엔티티도 씬 제한 수치에 포함됩니다.


또한 멀리 있거나 가려진 엔티티의 애니메이션을 켜거나 끌 수도 있습니다. 예를 들어, 매우 미세한 대기 애니메이션을 재생하는 NPC의 경우, 플레이어가 20미터 이내에 있을 때만 그 애니메이션이 재생되게 할 수 있습니다. NPC 주변에 트리거 영역을 사용하고 그에 따라 애니메이션을 켜거나 끄세요.


> [!NOTE]
> **💡 팁**: 엔티티가 멀리 있고 충분히 작으면 엔진이 이를 컬링합니다. 이 컬링은 드로우콜 수준에서 도움이 되지만, 엔티티를 엔진에서 제거하는 것이 항상 더 좋습니다. 또한 이 컬링은 다른 엔티티에 의한 가림을 고려하지 않으므로, 충분히 작지 않지만 벽 뒤에 숨겨진 엔티티도 여전히 렌더링됩니다.


### 비동기 블록

블록 [비동기 코드](/docs/creator/content-creator-ko/sdk7/programming-patterns/async-functions/) 는 응답을 기다리는 동안 다른 모든 작업의 진행을 막지 않습니다. 씬은 계속 실행되고, 비동기 블록은 기다리던 응답이 도착하면 다시 시작됩니다. 씬은 단일 스레드에서 실행되므로 이것은 병렬 처리가 아니라, 외부 응답을 기다리는 동안 씬이 멈추는 것을 피하는 것일 뿐입니다.

다음과 같이 비동기 서비스의 응답에 의존하는 모든 프로세스는 `getPlayerData()` 또는 `getRealm()` 항상 비동기 블록에서 실행해야 합니다. 그렇지 않으면 응답을 기다리는 동안 씬의 나머지 로딩이 막힙니다. 제3자 서버에 대한 호출도 마찬가지입니다.

비동기가 아닌 모든 작업이 끝나면 씬은 완전히 로드된 것으로 간주된다는 점에 유의하세요. 플레이어가 씬에 들어올 때에도 비동기 프로세스가 아직 실행 중일 수 있습니다. 비동기 프로세스 때문에 엔티티가 로드되고, 그 엔티티의 지오메트리 안에 플레이어가 갇힐 수 있는 상황은 피하세요.

### 이벤트에 의존하기

씬의 로직이 다음을 듣는 데 의존하도록 하세요 [이벤트](/docs/creator/content-creator-ko/sdk7/interactivity/event-listeners/) 매 프레임마다 체크를 실행하는 대신 가능한 한 그렇게 하세요.

해당 `update()` 함수에 들어가는 [system](/docs/creator/content-creator-ko/sdk7/architecture/systems/) 는 매 프레임마다, 이상적으로는 초당 30회 실행됩니다. 반복적인 체크 대신 이벤트를 구독할 수 있다면 그렇게 하세요.

예를 들어 플레이어의 착용 아이템을 계속 확인하는 대신 `onProfileChanged` 이벤트를 구독하고, 플레이어의 착용 아이템은 변경되었을 때만 확인할 수 있습니다.

시스템을 반드시 사용해야 한다면, 모든 프레임마다 체크하거나 조정하지 마세요. update 함수의 일부로 타이머를 포함시켜 1초마다 한 번씩만, 또는 적절한 주기로 체크를 실행할 수 있습니다.

## 3D 모델 최적화

3D 모델을 더 가볍게 최적화할 수 있는 방법은 여러 가지가 있습니다.

다음을 사용할 때 [크리에이터 허브](/docs/creator/content-creator-ko/scene-editor/get-started/editor-installation/), 씬의 3D 모델이 사용하는 리소스에 대한 통계를 볼 수 있고, 다음 항목 중 하나를 초과하는지 확인할 수 있습니다. [씬 제한 사항](/docs/creator/content-creator-ko/sdk7/optimizing/scene-limitations/).

![](/docs/_remote/ab87e37107598ee524831639.webp)

이 메뉴를 펼쳐 세부 정보를 볼 수 있습니다.

![](/docs/_remote/8d4619b1778683c8d72d3c61.webp)

이 지표를 개선하기 위한 몇 가지 팁은 다음과 같습니다:

* 가능하면 3D 모델 간에 텍스처를 공유하세요. 좋은 방법은 하나의 텍스처를 아틀라스 맵으로 사용해 씬의 모든 모델이 공유하게 하는 것입니다. 여러 개의 작은 텍스처보다 1024x1024 픽셀 크기의 큰 공유 텍스처 하나를 두는 편이 더 좋습니다.

  > 참고: 알베도 텍스처와 머티리얼의 노멀 맵 또는 에미시브 맵에 같은 이미지 파일을 사용하는 것은 피하세요. 동일하더라도 별도의 파일을 사용하세요. 서로 다른 유형의 텍스처 속성에 같은 이미지 파일을 지정하면, 에셋 번들로 압축될 때 원치 않는 시각적 아티팩트가 생길 수 있습니다.
* *.glb* 는 압축 형식이므로 항상 *.gltf*보다 더 가볍습니다. 반면 *.gltf* 에서는 텍스처를 별도 파일로 내보내 텍스처 이미지를 쉽게 공유할 수 있습니다. 다음 [파이프라인](https://github.com/AnalyticalGraphicsInc/gltf-pipeline)을 사용하면 *.glb* 외부 텍스처 파일이 있는 모델을 가질 수 있어 두 가지 장점을 모두 누릴 수 있습니다.
* 블렌드 투명도는 사용하지 마세요. 블렌드 투명도는 렌더링 최적화의 상당수를 우회해야 합니다. 가능하면 불투명 또는 알파 테스트 지오메트리를 선호하세요.
* 스키닝 메시 사용은 피하세요. 성능을 크게 떨어뜨릴 수 있습니다.


> [!NOTE]
> **💡 팁**: 3D 모델 모범 사례에 대한 자세한 내용은 \[3D Modeling Section]\(/creator/3d-modeling/3d-models


### Blender에서 무거운 모델 구조하기

통계가 특정 모델 하나를 가리킨다면, 코드에서 우회하는 것보다 모델을 수정하는 편이 보통 더 빠릅니다. Blender나 유사한 도구에서 가장 유용한 편집은 다음과 같습니다:

* **삼각형 수 줄이기** Decimate 모디파이어를 사용해 줄이세요. 이것은 이미 가지고 있는 모델을 구조하는 도구입니다. 새 모델은 처음부터 로우폴리로 모델링하는 것이 더 좋습니다.
* **텍스처 크기 조정** 을 2의 거듭제곱 크기, 1024x1024 이하로 맞추고, 이를 *.glb*.
* **플레이어가 절대 보지 못하는 면 삭제**예를 들어 오브젝트의 아래면과 뒤면처럼요. 다음을 켜세요 [뒷면 컬링](#backface-culling) 을 사용해 지오메트리를 두 배로 늘리는 대신.
* **머티리얼 병합** 으로 합치세요. 부품마다 텍스처를 하나씩 두는 대신 하나의 아틀라스 텍스처를 사용하세요.
* **다음에서 조명, 카메라, 머티리얼을 제거하세요 `_collider` 메시.** 엔진은 이를 무시하므로 순수한 부담일 뿐입니다.

앞뒤로 삼각형 수와 머티리얼 수를 확인한 다음, 같은 파일 위에 다시 내보내세요. 미리보기가 실행 중이면 변경 사항과 함께 다시 로드됩니다.

### 같은 모델을 여러 번 재사용하기

씬에는 거리의 가로등, 방의 의자, 공원의 나무처럼 반복되는 콘텐츠가 많은 경우가 흔합니다. 이런 것들을 만드는 가장 좋은 방법은 가장 단순한 방법입니다 — **각 복사본에 고유한 엔티티를 부여하고, 모두 같은&#x20;*****.glb*****&#x20;파일을 추가할 수 있습니다.**

```ts
// 좋음: 파일 하나, 엔티티 여러 개
for (const position of lampPostPositions) {
  const lampPost = engine.addEntity()
  Transform.create(lampPost, { position })
  GltfContainer.create(lampPost, { src: 'assets/scene/lampPost.glb' })
}
```

엔진은 이러한 엔티티가 같은 소스를 공유한다는 것을 인식합니다. 파일은 한 번만 다운로드되고, 에셋 번들로 한 번만 변환되며, 그 메시와 텍스처도 메모리에 한 번만 유지됩니다. 즉, 스무 번째 가로등은 세계에서의 위치 비용 외에는 거의 비용이 들지 않습니다. 이는 씬 간에도 마찬가지입니다. 이웃 씬이 같은 파일을 사용한다면 이미 메모리에 있습니다. 반대로 거의 동일한 *.glb* 파일 20개를 내보내면 다운로드도 20번, 메모리 복사본도 20개가 됩니다.

이 방식이 **줄이지 않는** 것은 드로우콜입니다. 가로등 20개는 20개의 그리기 대상이며, 20개의 엔티티에서 오든 20개의 가로등이 모델링된 단일 *.glb* 에서 오든 마찬가지입니다. 이 수를 줄일 수 있는 유일한 방법은 모델링 도구에서 메시를 합치는 것이며, 이는 실제로 트레이드오프가 있습니다 — 다음을 보세요 [인스턴싱 vs 복제 vs 병합](/docs/creator/content-creator-ko/3d/meshes/#instancing-objects-vs-duplicating-objects) 에서 언제 가치가 있는지.

관련 팁 몇 가지:

* **하나의 거대한 모델보다 클러스터로 묶으세요.** 흩어져 있는 오브젝트가 많다면 좋은 중간지점은 하나의 *.glb* 를 그룹마다 두는 것입니다 — 거리 한 블록, 방 하나 분량의 가구처럼요 — 오브젝트 하나당 모델 하나 또는 씬 전체당 모델 하나로 가는 대신 말입니다. 오브젝트 수를 줄이면서 각 클러스터를 충분히 작게 유지해 컬링이 여전히 유용하게 작동하도록 할 수 있습니다.
* **한꺼번에 많은 복사본을 생성하나요?** 먼저 모델을 [`AssetLoad` 컴포넌트](/docs/creator/content-creator-ko/sdk7/optimizing/pre-load-resources/)미리 로드하세요. 그러면 복사본들은 각자 같은 다운로드를 기다리는 대신 이미 메모리에 있는 모델에서 생성됩니다. 각 복사본을 만드는 데도 작업이 필요하므로, 큰 폭발적 생성은 어느 쪽이든 한 프레임의 비용이 들 수 있습니다. 미리 로드는 파일을 기다리는 시간을 없애줄 뿐, 복사본을 만드는 비용은 없애주지 않습니다.
* **매우 큰 모델 하나는 작은 모델 여러 개보다 엔진에 더 부담이 큽니다.** 엔진은 끊김을 피하기 위해 씬을 구성하는 작업을 여러 프레임에 분산하지만, 하나의 거대한 모델은 그렇게 쪼갤 수 없습니다. 그것은 한 프레임에 들어갑니다. 작은 조각 여러 개는 부드럽게 로드되지만, 하나의 거대한 덩어리는 눈에 띄는 끊김을 유발할 가능성이 더 큽니다.
* **모델을 반복해서 사용해도 새로운 텍스처나 지오메트리는 생기지 않습니다.** 하나의 모델로 만든 가로등 20개는 세트에 새로 추가하는 대신 그 모델의 메시와 텍스처를 공유합니다. 스무 번째는 텍스처 메모리를 전혀 추가로 차지하지 않습니다. 예외는 머티리얼입니다. 엔진은 씬의 경계 클리핑을 적용하기 위해 렌더링되는 오브젝트마다 머티리얼 인스턴스를 하나씩 만듭니다. 따라서 머티리얼 수는 사용하는 모델 수가 아니라 렌더링하는 오브젝트 수를 따라갑니다. 그래도 이러한 인스턴스는 같은 텍스처와 셰이더 변형을 공유하므로, 프레임 시간보다는 약간의 메모리만 추가로 사용합니다. 씬이 텍스처 [한도에 닿게 만드는 것은](/docs/creator/content-creator-ko/sdk7/optimizing/scene-limitations/) 여러 개의 *다른* 모델을 각각 고유한 세트와 함께 보유하는 것이므로, 더 적은 수의 재사용 모델 라이브러리로 씬을 통합하면 그 점에 도움이 됩니다.

### 뒷면 컬링

성능 최적화를 위해 Backface Culling은 **켜짐** 에 의해 정의됩니다 **모든** 으로 엔진에서 렌더링된 후 모델 머티리얼에 설정됩니다. 설정과는 무관합니다.

모델의 뒷면이나 내부가 보여야 한다면, 면을 복제하고 노멀을 반전시키세요.

#### 문제 해결

씬에 머티리얼 Backface Culling 문제가 있는지 확인하려면 다음 단계를 따르세요:

1. 다음을 여세요 `디버그` 패널을 씬에서.

* 씬이 게시되어 있다면 다음을 입력하세요 `/debug` 명령어를 채팅에.
* 씬의 Preview 모드에 있다면, 버그 아이콘(

  <img src="/docs/_remote/3a2d6a1437dc36383f2065db.webp" alt="디버그 아이콘" width="32">

  )을 화면 오른쪽 상단 모서리에 있는 것을 클릭하세요.

2. 디버그 패널이 화면 오른쪽 하단 모서리에 나타납니다.
3. 아래 **현재 씬**에서 **뒷면 디버거** 버튼이 있습니다.

![](/docs/_remote/1e0c76604bd561268516ee00.webp)

4. 토글 **뒷면 컬링 강제 적용**: 뒷면 컬링이 켜진 상태로 렌더링된 머티리얼을 보여줍니다. 이것이 최적화가 프로덕션에 적용되었을 때의 실제 렌더링입니다. 켜기/끄기를 전환해 수정이 필요한 머티리얼을 찾아보세요.
5. 다음을 토글하여 **뒷면 디버거** 뒷면 컬링이 꺼진 머티리얼을 쉽게 찾아보세요. 다음을 강조 표시합니다:

* **빨간색**: Backface Culling이 **켜짐**.
* **초록색**: Backface Culling이 적용된 머티리얼 **켜짐**.

<p align="center"><img src="/docs/_remote/0c934b5889f4f369d893365d.webp" alt=""> <img src="/docs/_remote/d2530a38414758e20fb9ebd0.webp" alt=""><br><em>Backface Culling 끔(왼쪽) vs. Backface Culling 켬(오른쪽)의 머티리얼</em></p>

### 에셋 번들 변환

씬을 게시할 때마다 Decentraland 콘텐츠 서버는 그 안의 모든 *.gltf* 및 *.glb* 모델을 에셋 번들 형식으로 압축합니다. 이 형식은 *훨씬* 가벼워서 씬의 로드 속도를 크게 높이고 실행도 더 부드럽게 만듭니다. 변환은 각 배포 직후 시작됩니다. 보통 몇 초밖에 걸리지 않지만, 매우 큰 씬이거나 서버가 바쁠 때는 더 오래 걸릴 수 있습니다. 변환은 플랫폼마다 별도로 실행되며 다른 씬들 뒤에 대기할 수 있기 때문입니다. 다음을 할 수 있습니다 [변환 상태를 확인](/docs/creator/content-creator-ko/scene-editor/publish/publish-scene/#check-the-conversion-status) 하여 씬을 직접 확인할 수 있습니다.

씬을 개발하는 동안에도 같은 변환을 로컬에서 실행할 수 있습니다. 이렇게 하면 미리보기가 더 부드러워지고, 게시하기 전에 압축된 모델의 문제를 확인할 수 있습니다. 다음을 보세요 [최적화된 에셋으로 미리보기](/docs/creator/content-creator-ko/sdk7/getting-started/preview-scene/#preview-with-optimized-assets).


> [!NOTE]
> **💡 팁**: Decentraland에서 이벤트를 계획할 때는 최종 버전을 최소 한 시간 전에 게시해 두세요. 그러면 그때까지 모든 모델이 에셋 번들로 변환되어 있고, 예상치 못한 문제를 수정할 시간도 확보할 수 있습니다. 이벤트 전에 놀라움을 망치고 싶지 않다면, 프로젝트 폴더에 모든 최종 3D 모델이 포함되어 있지만 보이지 않거나 크기가 0으로 설정된 버전의 씬을 배포할 수 있습니다.



> [!WARNING]
> **📔 참고**: 다음과 같은 *어떤* 변경이 3D 모델 파일에 가해지면, 이름만 바뀌는 경우라도 새 파일로 간주되며 다시 에셋 번들 형식으로 변환해야 합니다.


## 연결성

씬이 제3자 서버에 연결하거나 [messagebus](/docs/creator/content-creator-ko/sdk7/networking/serverless-multiplayer/#send-explicit-messagebus-messages) 를 사용해 플레이어 간에 메시지를 보낸다면, 염두에 두고 싶은 사항도 몇 가지 있습니다.

* 씬은 한 번에 활성 WebSockets 연결을 하나만 가져야 합니다.
* HTTP 호출은 엔진에 의해 하나씩만 처리되도록 흐름이 조정됩니다. 추가 요청은 내부적으로 대기열에 쌓이고 다른 요청이 완료될 때까지 기다려야 합니다. 이 대기열 처리 과정은 자동으로 이루어지므로 따로 할 일은 없습니다.
* 다음을 사용할 때 [messagebus](/docs/creator/content-creator-ko/sdk7/networking/serverless-multiplayer/#send-explicit-messagebus-messages) 플레이어 간에 메시지를 보내는 데 사용할 때는, 모든 메시지가 서버 섬의 다른 모든 플레이어에게 전송된다는 점에 유의하세요. 들어온 메시지가 곧바로 또 다른 메시지를 보내는 상황은 피하세요. 씬에 사람이 많을 때 메시지 수가 기하급수적으로 빠르게 늘어날 수 있습니다.

## 씬 UI

많은 개별 요소로 이루어진 씬 UI는 렌더링 비용이 커질 수 있습니다. 각 UI 요소마다 엔진에서 별도의 드로우콜이 필요하다는 점을 기억하세요.


> [!NOTE]
> **💡 팁**: 여러 요소를 하나의 단일 이미지로 합치도록 하세요. 예를 들어 여러 텍스트 요소가 있는 메뉴가 있다면, 타일의 텍스트와 추가 이미지를 배경 이미지에 미리 합쳐 두는 것이 이상적입니다. 그러면 각 텍스트 요소마다 매 프레임 추가 드로우콜을 하나씩 더 수행하는 것을 엔진이 피할 수 있습니다.


매 프레임 UI를 조정하는 일은 피하세요. 이는 특히 비용이 크고, 대기열에 쌓일 수도 있습니다. 예를 들어 UI에 시간이 지나면서 줄어들어야 하는 체력 바가 있다면, 플레이어는 그것이 매 프레임(30 FPS) 업데이트되는지 10 FPS로 업데이트되는지 차이를 아마 알아차리지 못할 것입니다. 이 바를 업데이트하는 시스템은 100밀리초를 세는 짧은 타이머를 사용할 수 있고, 이 타이머가 0에 도달할 때만 UI에 영향을 주면 됩니다.

많은 숨겨진 UI 요소를 두는 것도 피하세요. 이는 렌더링되지 않더라도 성능에 영향을 줍니다. 가능하다면 필요할 때 UI 구성 요소를 생성하도록 하세요.

## Worlds의 랜드스케이프 지형

다음에 게시된 씬은 [Decentraland World](/docs/creator/content-creator-ko/sdk7/publishing/publishing-options/#decentraland-worlds) 은 풀밭, 나무, 바다로 이루어진 자동 생성 랜드스케이프로 둘러싸여 있습니다. 이 랜드스케이프를 렌더링하면 플레이어의 렌더링 예산 일부가 소모됩니다. 씬에 이것이 필요 없다면 [랜드스케이프 지형을 비활성화할 수 있습니다](/docs/creator/content-creator-ko/sdk7/kinds-of-projects/scene-metadata/#landscape-terrain) 에서 `scene.json` 그 자원을 씬 자체의 콘텐츠에 사용할 수 있도록 확보하세요.

## 성능 모니터링

씬이 얼마나 잘 동작하는지 알 수 있는 가장 좋은 지표는 FPS(초당 프레임 수)입니다. 미리보기에서는 디버그 패널에서 현재 씬 FPS를 볼 수 있습니다. 항상 30 FPS 이상을 목표로 하세요.

배포된 씬에서는 채팅 창에 다음을 입력하면 이러한 지표를 보여주는 패널을 토글할 수 있습니다 `/showfps` 를 채팅 창에.

### Scene Stats 패널

데스크톱 클라이언트에서 미리보기로 씬을 실행할 때, 씬 콘텐츠에 대한 실시간 통계를 보여주는 패널을 열 수 있습니다. 화면 오른쪽 상단 모서리의 통계 아이콘을 클릭하세요. 콘솔 및 디버그 아이콘 옆에 있습니다.

<img src="/docs/_remote/96e07557afec5b2ade6528d5.webp" alt="Scene Stats 패널" width="400">

이 패널은 씬의 삼각형 수, 엔티티 수, 바디 수, 텍스처 수를 각 한도와 비교해 집계하고, 각 예산 중 몇 퍼센트를 사용 중인지 보여줍니다. 값이 한도의 80%에 도달하면 주황색으로 바뀝니다. 또한 머티리얼, 지오메트리, 콜라이더, 외부 비디오 또는 오디오의 단순 개수도 보여줍니다. 각 지표 옆의 정보 아이콘 위에 마우스를 올리면 무엇을 세고 왜 중요한지 설명을 볼 수 있습니다.

콘텐츠가 로드되고 언로드될 때 숫자가 실시간으로 업데이트되므로, 이 패널은 [지연 로딩](#lazy-loading) 과 같은 전략이 해당 시점에 엔진이 실제로 처리하는 것에 어떤 영향을 주는지 확인하는 데 유용합니다. 각 한도의 자세한 내용은 [씬 제한 사항](/docs/creator/content-creator-ko/sdk7/optimizing/scene-limitations/) 를 참조하세요. 이것들은 소프트 한도라는 점을 기억하세요. 하나를 초과해도 씬이 차단되지는 않지만, 플레이어의 성능은 저하될 수 있습니다.

### 씬과 엔진 사이의 메시지

씬 성능의 주요 병목 중 하나는 보통 씬의 코드와 엔진 사이에서 메시지를 주고받는 일입니다.

씬을 미리보기로 실행할 때, 오른쪽 상단에 “Y = Toggle Panel”이라고 표시되는 것을 참고하세요. 키보드의 Y를 누르면 실시간으로 업데이트되는 유용한 정보가 담긴 패널이 열립니다.

SDK와 엔진 사이의 메시지가 필요한 것들과 상호작용하다 보면 ‘Processed Messages’ 수가 늘어나는 것을 볼 수 있습니다. ‘Pending on Queue’ 수를 주의 깊게 살펴보세요. 항상 0이거나 0에 가까워야 합니다. 이것은 처리되지 못하고 대기열에 들어간 메시지가 얼마나 되는지를 알려줍니다. ‘Pending on Queue’ 수가 늘어나기 시작하면 위험 구간에 들어선 것이므로 씬을 더 최적화할 방법을 생각해야 합니다.


> [!WARNING]
> **📔 참고**: 사용하지 않을 때는 패널을 열어 두지 마세요. 성능에 부정적인 영향을 줍니다.


미리보기에서 경험하는 성능은 실제 배포 환경과 다를 수 있다는 점을 기억하세요:

* 주변의 인접 씬이 부정적인 영향을 줄 수 있습니다
* 씬의 3D 모델을 에셋 번들로 압축하면 긍정적인 영향을 줄 수 있습니다
* 당신의 씬을 방문하는 일부 플레이어는 더 성능이 낮은 하드웨어에서 실행 중일 수 있습니다

항상 먼저 씬을 다음에 배포해 보는 것이 좋은 습관입니다 [Decentraland World](/docs/creator/content-creator-ko/sdk7/publishing/publishing-options/#decentraland-worlds) 좀 더 철저한 테스트를 하기 위해.

항상 플레이어의 피드백을 요청하세요. 당신이 씬을 어떻게 경험하는지가 다른 사람에게도 같다고 당연하게 여기지 마세요.

## AI로 최적화하기

AI 에이전트가 이 페이지에서 설명한 측정과 수정을 대부분 대신 처리할 수 있습니다. Decentraland는 [AI 스킬](/docs/creator/content-creator-ko/sdk7/getting-started/vibe-coding/#install-skills-for-any-ai-agent) 을 제공하여 Claude Code나 Cursor 같은 코딩 에이전트가 씬을 최적화하는 방법을 배우게 하고, 데스크톱 클라이언트에는 [MCP 서버](/docs/creator/content-creator-ko/sdk7/getting-started/vibe-coding/#let-the-ai-see-your-scene-in-world) 가 포함되어 있어 에이전트가 실행 중인 씬을 직접 검사하고 측정할 수 있습니다.

다음을 사용하면 `optimize-scene` 및 `unity-explorer-mcp` 스킬이 설치되고 MCP 서버가 연결된 상태에서 에이전트는 다음을 할 수 있습니다:

* **씬이 콘텐츠 예산 내에 있는지 확인**: Scene Stats 패널과 같은 실시간 통계를 읽고 이를 [씬 제한 사항](/docs/creator/content-creator-ko/sdk7/optimizing/scene-limitations/)과 비교해 어떤 예산이 빠듯한지 알려줄 수 있습니다.
* **책임이 있는 자산 찾기**: 씬의 3D 모델을 각 모델이 기여하는 삼각형 수, 머티리얼 수, 드로우콜 수 기준으로 순위를 매기며, 특정 시점에서 보이는 것까지 포함하므로 어떤 모델을 먼저 최적화해야 하는지 정확히 알 수 있습니다.
* **실제 프레임률 측정**: 플레이어를 씬의 아무 위치에나 배치하고, 그곳에서 플레이어가 실제로 경험하는 FPS를 순간적인 끊김까지 포함해 샘플링합니다.
* **자체 작업 검증**: 각 변경 후 다시 측정할 수 있으므로, 추천과 수정이 추측이 아니라 실제 수치에 기반합니다.

이렇게 하면 최적화는 단순히 이렇게 요청하면 되는 일이 됩니다:

> "내 씬이 한도 내에 있는지 확인하고, 스폰 지점 근처의 프레임률을 떨어뜨리는 원인을 찾아 고치고, 수정 전후 수치를 보여줘."

### Blender MCP 서버와 결합하기

Decentraland 도구는 에이전트에게 어떤 모델이 가장 무거운지 알려주지만, 모델 자체를 편집하지는 않습니다. 더 나아가려면 다음과 결합해 볼 수 있습니다 [Blender MCP 서버](https://www.blender.org/lab/mcp-server/)는 에이전트가 Blender의 편집 도구에 접근할 수 있게 해 줍니다. 이를 통해 에이전트는 Blender에서 무거운 모델을 열어 삼각형 수를 줄이고, 머티리얼을 병합하거나, 텍스처를 줄인 뒤, 결과를 `.glb` 파일로 씬에 다시 내보낼 수 있으며, 그 후 월드 안에서 프레임률을 다시 측정할 수 있습니다.

자동 메시 편집은 모델의 외형을 눈에 띄게 바꿀 수 있으므로, 항상 결과를 검토하고 원본 파일의 백업을 보관하세요.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/optimizing/performance-optimization.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
