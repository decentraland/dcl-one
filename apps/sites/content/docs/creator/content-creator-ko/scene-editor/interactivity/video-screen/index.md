> 씬에서 비디오 재생하기

# 비디오 화면

씬에 있는 화면에서 미리 녹화된 동영상이나 스트리밍 동영상을 재생하려면 Video Player를 사용하세요 [스마트 아이템](/docs/creator/content-creator-ko/scene-editor/interactivity/smart-items/).

![](/docs/_remote/bb8266cdd73eaeb440e3fe4d.webp)

## 일반 설정

이 설정은 동영상을 재생하든 스트리밍하든 모든 시나리오에 적용됩니다.

![](/docs/_remote/6418389e00c134e192913d0e.webp)

동영상 오디오의 볼륨을 설정할 수 있습니다. 스트림의 오디오는 위치 기반이 아니며, 씬 전체에서 동일한 볼륨으로 들립니다.

다음 **기본 미디어 소스** 드롭다운을 사용하면 두 가지 다른 유형의 소스 중에서 선택할 수 있습니다:

* **비디오 URL**: URL 또는 로컬 비디오 파일에서 비디오나 스트림을 가져옵니다
* **라이브 스트림**: Decentraland의 무료 스트리밍 인프라를 사용해 스트림을 표시합니다. 이를 사용하려면 다음도 포함해야 합니다 [관리자 도구](/docs/creator/content-creator-ko/scene-editor/operate-live/scene-admin/) 씬의 스마트 아이템입니다.

## 동영상 재생

다음 중 하나에서 미리 녹화된 동영상을 재생할 수 있습니다:

* **로컬 파일**: 씬의 일부로 비디오 파일을 업로드한 다음 *URL* 필드에 해당 파일 경로를 지정합니다.
* **URL에서 스트리밍**: Bunny 같은 제공업체에서 제공하는 것과 같이 웹상의 라이브 또는 미리 녹화된 스트림을 가리킵니다. 참고하세요 [스트리밍 동영상](#streaming-from-other-sources)

Video Player 스마트 아이템이 동영상을 재생하는 시점은 여러 요소에 따라 달라질 수 있습니다:

* **자동**: 씬이 로드되자마자 동영상 재생이 시작됩니다. 이를 위해 기본 미디어 소스 드롭다운을 **비디오 URL** 로 설정하고 URL을 직접 붙여넣으세요 **비디오 경로 또는 .m3u8 URL** 필드에 저장됩니다.

  ![](/docs/_remote/6418389e00c134e192913d0e.webp)
* **관리자가 트리거**: [씬 관리자](/docs/creator/content-creator-ko/scene-editor/operate-live/scene-admin/) 는 현재 씬에 있는 경우 관리자 UI를 사용해 비디오 URL을 붙여넣고, 현재 씬에 있는 모든 플레이어에게 재생할 수 있습니다.
* **플레이어 행동 기반**: 유형이 **동영상 재생**인 액션을 정의하세요. 이를 통해 방에 들어가거나 버튼을 누르는 등 다른 스마트 아이템과 상호작용한 결과로 동영상 재생을 트리거할 수 있습니다. 참고하세요 [스마트 아이템 - 고급](/docs/creator/content-creator-ko/scene-editor/interactivity/smart-items-advanced/).

  ![](/docs/_remote/87a1916dbb31dbea1502aad6.webp)

모든 경우에 동영상을 반복 재생할지 한 번만 재생할지 설정합니다.


> [!WARNING]
> **📔 참고**: 씬에서 동시에 재생되는 동영상이 너무 많으면 일부는 엔진에 의해 일시 중지됩니다. 우선순위는 플레이어와의 거리, 카메라 방향, 화면 크기를 기준으로 결정됩니다. 동시에 재생 가능한 최대 동영상 수는 플레이어의 품질 설정에 따라 달라집니다.
> 
> * 낮음: 1
> * 보통: 5
> * 높음: 10
> 
> 플레이어가 가까이 있을 때 또는 그런 동작을 수행했을 때 동영상 재생을 시작하는 것도 권장합니다. 씬이 지평선 저편에서 로드될 때 동영상을 재생하기 시작하면 플레이어가 인근 씬을 방문하는 동안 성능에 불필요한 영향을 미칩니다.


## 여러 비디오 화면

같은 동영상을 여러 화면에서 동시에 재생할 수 있습니다. 이를 위해서는 Video Player 스마트 아이템의 고급 속성을 편집해야 합니다.


> [!WARNING]
> **📔 참고**: 서로 다른 동영상을 동시에 하나 이상 재생하지 마세요. 성능을 크게 저하시킵니다.
> 
> 두 비디오 플레이어에 동일한 URL을 단순히 붙여넣으면 엔진은 이것이 같은 동영상인지 알지 못하고, 둘을 각각 따로 재생합니다. 아래 단계를 따라 두 번째 비디오 플레이어가 첫 번째와 같은 동영상을 재생하도록 설정하세요.


1. 씬에 비디오 플레이어 스마트 아이템 두 개를 추가하세요. 화면마다 하나씩입니다.
2. 첫 번째는 [동영상 재생](#play-videos) 섹션.
3. 에서 설명한 대로 정상적으로 설정하세요. **비디오 플레이어** 컴포넌트가

![](/docs/_remote/a77f00900280e771283a1861.webp)


> [!WARNING]
> **📔 참고**: 이 단계는 중요합니다. 그렇지 않으면 두 번째 비디오 플레이어가 보이지 않더라도 엔진에 의해 처리됩니다.


4. 여전히 두 번째 비디오 플레이어에서 **머티리얼** 컴포넌트의 **텍스처** 섹션을 펼친 다음 **비디오 소스 엔티티** 드롭다운을 선택해 첫 번째 비디오 플레이어를 가리키도록 합니다.

![](/docs/_remote/ac9747acfdb69510f8b3a247.webp)

각각이 같은 비디오 플레이어를 가리키도록 설정하는 한, 이와 같은 방식으로 원하는 수만큼 비디오 플레이어에 적용할 수 있습니다.

다음을 수행할 때 [라이브 스트리밍](/docs/creator/content-creator-ko/scene-editor/operate-live/live-streaming/), 두 화면 모두 같은 스트림을 표시합니다.


> [!NOTE]
> **💡 팁**: 위 단계는 또한 **머티리얼 바꾸기** 컴포넌트가 있는 아이템에도 반복 적용할 수 있으며, 어떤 3D 모델이든 비디오 화면으로 바꿀 수 있습니다.  **텍스처** 섹션을 **머티리얼 바꾸기** 구성해 비디오 플레이어 엔티티를 가리키도록 합니다.


## 비디오 파일 소개

다음 파일 형식을 지원합니다:

* *.mp4*
* *.ogg*
* *.webm*

비디오 파일은 씬의 전체 크기에 추가되므로, 플레이어가 씬에 들어올 때 다운로드하는 데 더 오래 걸립니다. 비디오 크기로 인해 다음 한도를 초과할 수도 있습니다 [씬 제한](/docs/creator/content-creator-ko/sdk7/optimizing/scene-limitations/), parcel당 최대 15MB만 사용할 수 있기 때문입니다. 비디오를 가능한 한 많이 압축하여 문제를 줄이는 것을 권장합니다.

## 라이브 스트리밍

종단 간 라이브 스트리밍은 [라이브 스트리밍](/docs/creator/content-creator-ko/scene-editor/operate-live/live-streaming/).

### 다른 소스에서 스트리밍

다른 스트리밍 인프라를 사용해 동영상을 스트리밍할 수도 있습니다. 이렇게 하려면 Video Player 스마트 아이템이 다음을 사용하도록 설정하고 **비디오 URL** 미디어 소스 **비디오 경로 또는 .m3u8 URL** 필드에 저장됩니다.

그리고 스트림 URL을 다음에 붙여넣으세요 *https*  URL (*http*  URL은 지원되지 않으며), 소스에는 [CORS 정책(Cross Origin Resource Sharing)](https://en.wikipedia.org/wiki/Cross-origin_resource_sharing) 이 있어 외부에서 액세스할 수 있어야 합니다. 즉, YouTube나 이와 유사한 사이트의 동영상을 스트리밍할 수 없습니다. 이런 사이트들은 브랜드화된 HTML 위젯에서만 콘텐츠 표시를 허용하기 때문입니다. 옵션과 팁은 [외부 스트리밍에 대하여](/docs/creator/content-creator-ko/sdk7/media/video-playing/#about-external-streaming) 를 참고하세요.

동영상을 스트리밍하는 방법에는 여러 가지가 있습니다. 가장 간단한 방법은 다음과 같은 관리형 호스팅 제공업체를 이용하는 것입니다 [Vimeo](https://vimeo.com/), [Bunny](https://bunny.net), [Livepeer Studio](https://livepeer.studio/) 또는 [Serraform](https://serraform.gitbook.io/streaming-docs/guides/decentraland-playback) 여기서는 스트리밍 인프라 전반을 관리하도록 제공업체에 수수료를 지불합니다.

읽기 [성공적인 스트리밍을 위한 OBS 설정](/docs/creator/content-creator-ko/sdk7/media/video-playing/#setting-up-obs-for-successful-streaming) Decentraland로 콘텐츠를 가장 잘 스트리밍하는 방법에 대한 팁을 보려면


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/scene-editor/interactivity/video-screen.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
