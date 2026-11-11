> 씬에서 사용할 자신만의 3D 모델, 이미지, 사운드 등을 가져옵니다.

# 사용자 지정 에셋 가져오기

씬 편집기에 자신만의 3D 모델을 가져올 수 있습니다. 인터넷에서 무료 또는 유료로 제공되는 다양한 소스에서 모델을 선택하거나, 직접 맞춤 모델을 만들 수 있습니다. 이미지, 사운드 파일, 비디오 같은 다른 에셋도 가져올 수 있습니다.



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/UepXpH-k0EI" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=UepXpH-k0EI>">Watch on YouTube</a></p>



## 에셋 가져오기

로컬 디스크에서 씬으로 3D 모델, 이미지, 사운드 파일 또는 비디오를 가져오려면:

1. 파일을 하단 패널로 직접 끌어다 놓으세요. 또한 다음을 클릭할 수도 있습니다 **+ 에셋 가져오기** 하단 패널 왼쪽 상단의 버튼을 클릭하고 로컬 드라이브에서 선택하세요.

![](/docs/_remote/7a7e1bf757e28411bf4bd633.webp)

2. 모델 썸네일을 확인한 다음 **가져오기**. 여러 에셋을 가져올 때는 화살표 버튼을 사용해 각 에셋을 순환하세요.

![](/docs/_remote/114b427e0cc71cef23245e2a.webp)

이제 에셋을 **로컬 에셋** 탭에서 찾을 수 있습니다. 에셋은 유형별로 폴더에 정리되며: 3D 모델은 *assets/Models* 폴더에, 이미지는 *assets/Images*, 사운드 파일은 *assets/Audio*, 그리고 비디오는 *assets/Video*.

내장 무료 에셋 팩의 항목은 *assets/asset-packs/*&#xC5D0; 저장되며, 편집기에서 만든 사용자 지정 항목은 *assets/custom/*&#xC5D0; 저장됩니다. 이전 씬에는 사용자 가져오기가 *assets/scene/* 에 있을 수도 있습니다. 이 모든 경로는 씬 코드에서 사용할 수 있으며, 에셋이 들어 있는 폴더를 그대로 참조하면 됩니다.

* 3D 모델의 경우, `.glb` 또는 `.gltf` 파일을 캔버스로 끌어다 놓아 씬의 항목으로 추가하세요.
* 이미지, 사운드 또는 비디오 파일도 캔버스로 끌어다 놓을 수 있습니다. 편집기는 이미 해당 파일을 가리키는 준비된 항목을 만듭니다:

  * 하나는 `.png`, `.jpg` 또는 `.jpeg` 파일은 **이미지** 항목이 됩니다.
  * 하나의 `.mp3`, `.ogg` 또는 `.wav` 파일은 주변음 항목이 됩니다.
  * 하나의 `.mp4` 파일은 하나의 **비디오 화면** 항목이 됩니다.

  새 항목의 이름은 파일 이름을 따르며, 에셋 팩의 해당 항목과 동일한 구성 요소와 설정으로 생성되므로 인스펙터에서 바로 조정할 수 있습니다.
* 어떤 에셋이든 이미 씬에 있는 항목의 필드 위로 끌어다 놓을 수도 있습니다. 예를 들어 `.mp3` 파일을 *경로* 의 필드에 *오디오 소스* 컴포넌트도 부여해야 합니다.


> [!NOTE]
> **💡 팁**: 또한 파일을 프로젝트의 `에셋` 폴더에 Finder, Explorer 또는 다른 곳에서 직접 붙여넣을 수도 있습니다. 그러면 **로컬 에셋** 탭에 복사가 완료된 직후 자동으로 표시됩니다. 표시되지 않으면 **새로 고침** 옆의 버튼을 누르세요 **에셋 가져오기** 버튼이 있습니다.
> 
> <img src="/docs/_remote/bef94fa811ae48e5d4c837f0.webp" alt="" data-size="original">


### 지원 형식

#### 오디오

다음 오디오 형식이 지원됩니다:

* *.mp3*
* *.wav*
* *.ogg*

#### 이미지

다음 이미지 형식이 지원됩니다:

* *.png*
* *.jpg*
* *.jpeg*

#### 비디오

다음 비디오 형식이 지원됩니다:

* *.mp4*

#### 3D 모델

다음 3D 모델 형식이 지원됩니다:

* *.glTF*
* *.glb*

둘 다 외부 텍스처 이미지 파일 또는 외부 바이너리(*.bin*) 파일을 포함할 수 있습니다.

다른 형식은 다양한 편집기와 도구를 사용해 이 형식들로 변환할 수 있습니다.  [3D 모델링](/docs/creator/content-creator-ko/3d/3d-models/) 에서 권장 사항과 팁을 확인하세요.

모델의 모든 머티리얼은 다음 중 하나여야 합니다 *기본 머티리얼* 또는 *PBR*이며, 또한 모든 텍스처는 2의 거듭제곱 크기(예: 256, 512)여야 합니다.  [씬 제한 사항](/docs/creator/content-creator-ko/sdk7/optimizing/scene-limitations/) 를 참조하세요.

어떤 유형이든 가져온 각 파일은 씬에서 사용할 수 있으려면 50MB 미만이어야 합니다. 더 큰 파일은 지원되지 않습니다.

**3D 모델용 무료 라이브러리**

직접 3D 모델을 만드는 대신, 여러 무료 또는 유료 라이브러리에서 다운로드하거나 AI 도구로 생성할 수도 있습니다.  [유용한 자료](/docs/creator/content-creator-ko/sdk7/getting-started/useful-resources/) 에서 권장 에셋 라이브러리와 생성형 AI 도구 목록을 확인하세요.


> [!WARNING]
> **📔 참고**: 다운로드하는 콘텐츠의 라이선스 제한 사항에 유의하세요.


이들 사이트 중 일부에서는 모델을 다운로드할 형식을 선택할 수 있습니다. 항상 *.glTF* 또는 *.glb* 형식을 선택하세요. 사용할 수 없는 경우에는 이를 *.glTF* 또는 *.glb* 씬에서 사용하기 전에 변환해야 합니다. 이를 위해 Blender로 가져온 뒤 사용 가능한 *.glTF* 내보내기 애드온 중 하나로 내보내는 것을 권장합니다.

### 콜라이더

미리보기를 실행하면 플레이어가 가져온 3D 모델을 통과해 걸어갈 수 있습니다. 이는 아마도 모델에 *콜라이더 메시* 충돌 형상을 정의할 [콜라이더](/docs/creator/content-creator-ko/3d/colliders/) 수 있는 항목이 없기 때문일 것입니다. 자세한 내용과 안내는&#x20;


> [!NOTE]
> **💡 팁**: 모델을 수정해 *콜라이더 메시*를 추가하는 대신, 더 간단한 대안은 *보이지 않는 벽* 스마트 항목을 비슷한 모양으로 추가해 그 자리를 대신하게 하는 것입니다.


### 애니메이션

가져온 모델에 애니메이션이 포함되어 있으면, 모델에 포함된 첫 번째 애니메이션이 반복 재생됩니다.

애니메이션이 언제 시작하거나 멈출지, 또는 여러 애니메이션이 있을 경우 어떤 애니메이션이 재생될지 직접 제어할 수 없다는 점에 유의하세요.

씬에 여러 플레이어가 있으면 서로 애니메이션이 동기화되지 않은 상태로 보일 수 있습니다.

이 동작을 바꾸려면 **애니메이터** 컴포넌트. 다음을 참조하세요 [어떤 항목이든 스마트 항목으로 만들기](/docs/creator/content-creator-ko/scene-editor/interactivity/make-any-item-smart/) 코드 없이 도구를 사용해 항목을 상호작용 가능하게 만들 수 있는


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/scene-editor/build/import-items.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
