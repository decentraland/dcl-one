> 어떤 씬에서든 재사용할 수 있도록 자신만의 사용자 지정 아이템을 만드세요.

# 사용자 지정 아이템

사용자 지정 항목을 정의하면 어떤 씬에서든 쉽게 재사용하거나 다른 씬 제작자와 공유할 수 있습니다. 사용자 지정 항목은 단일 엔티티로 구성될 수도 있고, 원하는 만큼 많은 엔티티로 구성될 수도 있습니다. 사용자 지정 항목은 기존 스마트 항목의 변형일 수도 있고, 자체적으로 맞춤화된 모델과 기능을 갖춘 완전한 사용자 지정 항목일 수도 있습니다.



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/7cGLu8P7dso" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=7cGLu8P7dso>">Watch on YouTube</a></p>



## 사용자 지정 항목을 정의하는 방법

엔티티를 마우스 오른쪽 버튼으로 클릭하세요 [엔티티 트리](/docs/creator/content-creator-ko/scene-editor/get-started/scene-editor-essentials/#the-entity-tree), 또는 여러 엔티티를 선택한 다음 마우스 오른쪽 버튼으로 클릭하세요. 다음을 선택하세요 **사용자 지정 항목 만들기**.

![](/docs/_remote/52a9b492b9f5042f38574e21.webp)

화면 하단 영역에서 새 사용자 지정 항목의 이름을 입력하라는 메시지가 표시됩니다.

![](/docs/_remote/eb32f501e5b7c6a70d610e46.webp)

항목은 이제 다음에 표시됩니다 **Custom Items** 탭의 화면 하단에 있습니다. 이 탭은 작업 공간에 사용자 지정 항목이 하나 이상 있을 때만 표시됩니다.

![](/docs/_remote/a66e7b1f2ae4e2d83577aeb6.webp)

사용자 지정 항목을 정의할 때는 동일한 계층 수준에 있는 여러 엔티티를 선택할 수 있지만, 공통 조상을 공유하지 않는 별도의 부모 아래 엔티티는 선택할 수 없습니다. 중첩된 엔티티는 자동으로 사용자 지정 항목의 일부로 포함되므로 선택할 필요가 없습니다.

씬에 있는 원래 엔티티는 사용자 지정 항목 정의 작업의 영향을 받지 않습니다.


> [!WARNING]
> **📔 참고**: 사용자 지정 항목을 정의할 때 선택한 엔티티의 각 구성 요소 상태를 스냅샷으로 저장합니다(루트 엔티티의 위치, 회전, 크기는 제외). 여기에는 **동작**, **트리거**, **멀티플레이어**, **스크립트**, **가시성**, 그리고 그 외의 모든 구성 요소가 포함됩니다.
> 
> 모든 스마트 항목의 동작과 트리거는 자체 복사본을 자기 참조합니다. 예를 들어 엘리베이터와 버튼을 포함하는 사용자 지정 항목을 정의하면, 각 엘리베이터 복사본의 버튼은 원본 엘리베이터가 아니라 자신이 속한 엘리베이터 복사본을 제어합니다.


### 사용자 지정 항목 사용하기

다음에서 항목을 끌어다 놓기만 하면 됩니다 **Custom Items** 탭에서 씬으로.

추가한 후에는 사용자 지정 항목의 어떤 속성이든 자유롭게 변경할 수 있으며, 변경 사항은 *해당 복사본에만* 사용자 지정 항목에 적용됩니다. 또한 사용자 지정 항목의 원본이 된 항목을 편집하거나 삭제할 수도 있으며, 이는 기존 또는 향후 복사본에는 영향을 주지 않습니다.

엔티티 트리에서 사용자 지정 항목은 다른 아이콘으로 표시됩니다. 오른쪽의 항목 속성 메뉴 상단에서는 해당 항목이 어떤 사용자 지정 항목에서 생성되었는지도 확인할 수 있습니다.

사용자 지정 항목 정의를 삭제하려면 다음의 항목을 마우스 오른쪽 버튼으로 클릭하세요 **Custom Items** 메뉴에서 다음을 선택하세요 **삭제**. 이 작업은 씬에 있는 해당 항목의 기존 복사본에는 영향을 주지 않으며, 고아 상태의 사용자 지정 항목은 씬에서 변경 없이 그대로 유지됩니다. 사용자 지정 항목 정의를 삭제하면 Custom Items 탭에서만 제거됩니다.

사용자 지정 항목 정의의 이름을 바꾸려면 Custom Items 정의를 마우스 오른쪽 버튼으로 클릭하세요 **Custom Items** 탭에서 다음을 선택하세요 **이름 바꾸기**.

이미 생성된 사용자 지정 항목의 정의는 수정할 수 없으며, 새 정의를 만들어 원본을 삭제해야 합니다.

### 사용자 지정 항목을 동적으로 생성하기

각 사용자 지정 항목은 composite로 저장되며, 이는 엔티티와 구성 요소의 트리를 설명하는 파일입니다. 즉, 씬이 실행되는 동안 각 복사본을 직접 배치하는 대신 사용자 지정 항목의 새 복사본을 생성할 수 있습니다.

사용자 지정 항목은 두 가지 방법으로 생성할 수 있습니다:

* **코드 없이**: 다음을 사용하세요 **엔티티 생성** 작업을 씬 편집기에서 사용하세요. 다음을 참고하세요 [엔티티 생성에 관하여](/docs/creator/content-creator-ko/scene-editor/interactivity/smart-items-advanced/#about-spawning-entities).
* **코드로**: 다음을 사용하세요 `Composite.instance()` SDK에서 사용하세요. 다음을 참고하세요 [컴포지트](/docs/creator/content-creator-ko/sdk7/architecture/composites/).

### 사용자 지정 항목 공유하기

사용자 지정 항목을 다른 제작자와 공유하면, 그들도 자신들의 씬에서 사용할 수 있습니다.

사용자 지정 항목은 각각 로컬 컴퓨터의 별도 폴더에 저장됩니다. 폴더 아이콘을 클릭하여 해당 폴더를 여세요 <img src="/docs/_remote/ad0c0dd6dced1bedf2b986b9.webp" alt="Code" data-size="line"> Custom Items 탭의 오른쪽 상단에 있습니다.

이 폴더는 컴퓨터에서 직접 찾을 수도 있습니다:

* Windows에서는: *User/AppData/Roaming/creator-hub/Custom Items*
* Mac에서는: *Users/username/Library/Application Support/creator-hub/Custom Items*


> [!WARNING]
> **📔 참고**: *Library* 폴더는 Mac에서 기본적으로 숨겨져 있습니다. 가장 쉽게 접근하는 방법은 Go > Go to Folder를 열고 다음을 입력하는 것입니다 *application support/creator-hub*


다른 사람과 공유하려면 Custom Items 폴더를 열고 해당 항목의 전체 폴더를 복사하면 됩니다. 전송을 쉽게 하려면 폴더를 zip으로 압축할 수 있습니다. 이 폴더에는 사용자 지정 항목을 사용하는 데 필요한 모든 것이 들어 있습니다.

사용자 지정 항목을 사용하는 사람은 자신의 컴퓨터에 있는 Custom Items 폴더에서 해당 항목 폴더의 압축을 풀어야 합니다. 필요하면 새로고침 버튼을 클릭해야 할 수도 있습니다 <img src="/docs/_remote/29e0f054f9a96c7fb0c004cd.webp" alt="Refresh" data-size="line"> 그래야 해당 항목이 자신의 **Custom Items** 탭에 표시됩니다.

모든 **에셋** 사용자 지정 항목에서 사용되는 항목도 사용자 지정 항목의 폴더에 저장됩니다. 여기에는 항목에서 참조하는 모든 3D 모델, 이미지, 스크립트, 사운드, 비디오가 포함됩니다.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/scene-editor/interactivity/custom-items.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
