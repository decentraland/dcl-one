> 어떤 아이템이든 스마트 아이템처럼 동작하도록 설정합니다.

# 어떤 아이템이든 스마트하게 만들기



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/wnnEU8GCLjc" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=wnnEU8GCLjc>">Watch on YouTube</a></p>



스마트 아이템은 단지 일반 아이템에 하나의 **동작** 및/또는 **트리거** 컴포넌트가 추가된 것일 뿐입니다. 이러한 컴포넌트는 씬의 어떤 아이템에도 추가할 수 있습니다. 또한 사용자 지정 3D 모델을 가져와 그 모델들에도 동일하게 추가할 수 있습니다.

아이템에 컴포넌트를 추가하려면 **더하기 아이콘** 아이템 이름 옆에서 클릭한 뒤, 드롭다운 목록에서 추가할 컴포넌트를 선택하세요.

![엔터티에서 열리는 컴포넌트 추가 메뉴는 동작과 트리거를 포함하여 3D 콘텐츠 및 상호작용 아래에 그룹화된 컴포넌트들을 나열합니다.](/docs/_remote/e0e52c0c060b63e97e2466dd.webp)

이를 통해 엄청난 창의적 가능성이 열립니다. 촛불을 책장 뒤의 비밀 통로를 여는 레버로 바꾸고, 우물 안에서 신비한 소리를 재생하며, 다이아몬드를 클릭하면 0으로 줄어드는 수집 가능한 아이템으로 만들 수 있습니다. 이 메커니즘들을 조합할 수 있는 상상력 넘치는 방법은 정말 많습니다!

## 상호작용성

아이템이 플레이어의 다양한 동작에 반응하게 만들 수 있습니다.


> [!NOTE]
> **💡 팁**: 플레이어가 아이템과 상호작용할 때는 그 상호작용이 분명하게 드러나도록 어떤 형태로든 피드백을 보여줘야 합니다. 모델에 애니메이션이 없다면, 적어도 소리를 재생하는 것을 고려하세요. 경우에 따라서는 피드백의 형태로 아이템의 크기를 약간 트윈 애니메이션으로 바꾼 뒤 원래 크기로 돌아오게 하는 것도 효과적일 수 있습니다.


다음 **트리거** 컴포넌트를 추가하여 플레이어의 다양한 동작을 감지할 수 있습니다:

* **포인터 이벤트**: 플레이어가 커서를 아이템 위에 두고 클릭하거나 키를 눌렀을 때.
* **전역 버튼 이벤트** 플레이어가 씬의 어디에 있든 키를 눌렀을 때.
* **플레이어 근접**: 플레이어가 아이템의 위치로 걸어 들어왔을 때.

해당 **트리거** 컴포넌트는 이러한 유형의 트리거 중 어느 것에도 반응하도록 설정할 수 있습니다. 트리거가 발생할 때마다 자신의 **동작** 컴포넌트나 **동작** 씬에 있는 다른 아이템의 컴포넌트에서 동작을 호출할 수 있습니다. 참고 [스마트 아이템 - 고급](/docs/creator/content-creator-ko/scene-editor/interactivity/smart-items-advanced/).


> [!NOTE]
> **💡 팁**: 또한 이러한 트리거를 [조건 논리](/docs/creator/content-creator-ko/scene-editor/interactivity/states-and-conditions/)와 결합하여, 트리거가 발생할 때마다 동작이 호출되는 것이 아니라 조건이 참일 때만 호출되도록 할 수 있습니다.
> 
> 예를 들어, **포인터 이벤트** 트리거를 문에 추가하여 클릭하면 열리게 만들되, 잠겨 있을 때만 열리도록 조건 논리를 포함할 수 있습니다.


### 포인터 이벤트

다음 **트리거** 컴포넌트와 **클릭 시** 또는 **입력 동작 시** 트리거 이벤트.

* **클릭 시** 플레이어가 아이템을 가리킨 상태에서 왼쪽 마우스 버튼을 클릭할 때마다 반응합니다.
* **입력 동작 시** 플레이어가 아이템을 가리킨 상태에서 기본 버튼(E)을 누를 때마다 반응합니다.

![](/docs/_remote/342cba6acbd2ae9ce5e8ce5f.webp)

**콜라이더**

아이템을 클릭할 수 있으려면 반드시 **콜라이더**가 있어야 합니다. 그렇지 않으면 클릭이 모델을 그대로 통과해 뒤에 있는 대상과 상호작용하려고 하게 됩니다. 기본 애셋 팩의 3D 모델에는 모두 콜라이더가 있어야 하지만, 직접 모델을 만들거나 다른 곳에서 가져온 경우 콜라이더가 없을 수 있습니다.

모델에 콜라이더가 없다면, 다음 방법 중 하나로 해결할 수 있습니다:

* 다음 **메시 콜라이더** 컴포넌트. 이렇게 하면 [기본 도형](/docs/creator/content-creator-ko/sdk7/3d/shape-components/#primitive-shapes) (큐브, 평면, 실린더, 구) 형태의 콜라이더가 생성됩니다.
* 다음의 속성을 변경하세요 **충돌** 섹션의 **GLTF** 컴포넌트.  **보이는 레이어** 는 다음에 할당되어야 합니다 **포인터**.
* Blender에서 3D 모델을 편집하여 보이지 않는 콜라이더 지오메트리를 포함시키세요(이름이 `_collider`로 끝나는 모든 메시). 참고 [콜라이더](/docs/creator/content-creator-ko/3d/colliders/).


> [!NOTE]
> **💡 팁**: 만약 **메시 렌더러** 컴포넌트를 사용해 모델에 기본 도형을 부여했다면, 그것만으로는 콜라이더가 생기지 않습니다. 반드시 **메시 콜라이더** 컴포넌트도 지정해야 합니다.


**포인터 이벤트 사용자 지정**

아이템에 **클릭 시** 또는 **입력 동작 시** 트리거 동작이 있을 때 사용되는 기본 설정을 재정의할 수 있습니다.

* **호버 텍스트**: 아이템 위에 마우스를 올렸을 때 플레이어가 커서 옆에서 보게 되는 힌트를 변경하세요. 이는 아이템이 무엇을 하는지 명확히 설명하는 데 매우 도움이 될 수 있습니다.
* **최대 거리**: 플레이어가 아이템과 상호작용할 수 있는 최대 거리입니다.
* **피드백 표시**: 체크하지 않으면, 플레이어가 커서를 아이템 위에 올려도 호버 힌트가 표시되지 않습니다.
* **버튼**: **입력 동작 시** 트리거 동작을 사용하는 경우 기본 **기본(E)** 을 다른 키로 재할당할 수 있습니다. 호버 힌트에는 어떤 키를 사용해야 하는지 명확히 보여 주는 아이콘이 포함됩니다. 사용할 수 있는 것은 **보조(F)**&#xB610;는 **동작 3\~6** (숫자 키 1\~4)입니다.

### 전역 버튼 이벤트

다음 **트리거** 컴포넌트와 **전역 클릭 시**, **전역 기본 버튼 시** 또는 **전역 보조 버튼 시** 트리거 이벤트.

* **전역 클릭 시** 플레이어가 씬의 어디에 있든 왼쪽 마우스 버튼을 클릭할 때마다 반응합니다.
* **전역 기본 버튼 시** 플레이어가 씬의 어디에 있든 기본 버튼(E)을 누를 때마다 반응합니다.
* **전역 보조 버튼 시** 플레이어가 씬의 어디에 있든 보조 버튼(F)을 누를 때마다 반응합니다.


> [!NOTE]
> **💡 팁**: 종종 이 기능을 [상태와 조건](/docs/creator/content-creator-ko/scene-editor/interactivity/states-and-conditions/)과 결합하는 것이 타당합니다. 그러면 플레이어가 방 안에 있을 때나 다른 조건이 충족될 때만 아이템이 버튼 이벤트에 반응하게 할 수 있습니다.


### 플레이어 위치

다음 **트리거** 컴포넌트와 **플레이어가 영역에 들어감**, **플레이어가 영역을 벗어남** 트리거 이벤트.

이는 플레이어가 아이템 중심에 위치한 기본 크기 1x1x1의 영역에 들어가거나 나갈 때 반응합니다.


> [!NOTE]
> **💡 팁**: 종종 다음을 사용하는 것이 더 좋습니다 [**트리거 영역**](/docs/creator/content-creator-ko/scene-editor/interactivity/smart-items/#trigger-areas) 스마트 아이템 대신 사용하는 것이 더 좋습니다. 이 아이템의 크기는 씬 편집기에서 명확하게 시각화할 수 있기 때문입니다.


## 참고 항목

* [스마트 아이템 - 기본](/docs/creator/content-creator-ko/scene-editor/interactivity/smart-items/)
* [스마트 아이템 - 고급](/docs/creator/content-creator-ko/scene-editor/interactivity/smart-items-advanced/)
* [상태와 조건](/docs/creator/content-creator-ko/scene-editor/interactivity/states-and-conditions/)
* [코드와 결합](/docs/creator/content-creator-ko/scene-editor/extend-with-code/overview/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/scene-editor/interactivity/make-any-item-smart.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
