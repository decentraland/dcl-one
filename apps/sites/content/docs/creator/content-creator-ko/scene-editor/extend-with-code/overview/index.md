> Scene Editor에서 만든 콘텐츠와 코드 작성의 강력함을 결합합니다.

# 코드와 결합



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/55H37rygD7M" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=55H37rygD7M>">Watch on YouTube</a></p>



Creator Hub와 사용자 지정 코드를 함께 사용하면 콘텐츠를 만드는 데 매우 강력한 조합이 됩니다. 캔버스를 사용해 항목을 직관적으로 시각적으로 배치한 다음, 이러한 항목과 완전히 자유롭게 상호작용하는 코드를 작성할 수 있습니다. 자체 기본 동작을 가진 스마트 아이템을 배치하고, 해당 아이템이 활성화될 때 반응하는 코드를 작성할 수도 있습니다.

예를 들어, 이미 자체 사운드와 애니메이션, 상태를 갖춘 기존의 레버 스마트 아이템을 활용하고, 레버가 당겨졌을 때 이를 감지해 자체 사용자 지정 로직을 실행하는 코드를 작성할 수 있습니다.

참조 [코드에서 항목 참조하기](/docs/creator/content-creator-ko/scene-editor/extend-with-code/reference-items/) 코드에서 이름이나 태그로 항목을 가져오는 방법을 알아보려면.

## 코드 편집

씬의 코드를 편집하려면 컴퓨터에 코드 편집기를 설치해야 합니다. 권장 옵션은 다음과 같습니다:

* <img src="/docs/_remote/358795a1c08dfc479355a174.webp" alt="VS Code" data-size="line"> [Visual Studio Code](https://code.visualstudio.com/): 이는 숙련된 개발자에게 권장되는 옵션입니다.
* <img src="/docs/_remote/9510a847d1493f0d8b4cf7e3.webp" alt="Cursor" data-size="line"> [Cursor AI](https://www.cursor.com/): AI와 통합된 강력한 코드 편집기입니다. 코드를 작성하는 데 도움이 되도록 여러 AI 모델을 선택할 수 있으며, 무료 요금제가 제공되고 더 고급 모델은 유료 요금제가 필요합니다. Decentraland 또는 TypeScript를 처음 사용하는 개발자에게 좋은 옵션이며, 코드를 작성하는 시간을 절약하고 싶을 때도 적합합니다.


> [!WARNING]
> **📔 참고**: macOS를 사용하는 경우, 코드 편집기 앱이 Applications 디렉터리에 있는지 확인하세요.


설치가 완료되면 Creator Hub의 설정에서 코드 편집기를 선택해야 할 수 있습니다. 이렇게 하려면,

1. 화면 왼쪽 상단의 Creator Hub 로고를 클릭하고 **설정**.
2. 을 엽니다. **편집기** 탭.
3. 아래에서 **선호하는 코드 편집기**에서 코드 편집기를 선택하세요. 드롭다운 목록에 편집기가 표시될 수도 있고, **기기에서 선택...** 을 선택해 찾아야 할 수도 있습니다.

![편집기 탭의 앱 환경설정 대화상자이며, 선호하는 코드 편집기 드롭다운이 Visual Studio Code로 설정되어 있습니다.](/docs/_remote/be0b7d493dc2e145af5207ac.webp)

## 씬의 코드 열기

컴퓨터에 코드 편집기를 설치하고 Creator Hub 설정에서 선택했다면, **< > 코드** 버튼을 클릭해 씬 프로젝트에서 열 수 있습니다.

![](/docs/_remote/b1e812c7bf7175e15d141046.webp)

그러면 코드 편집기가 있는 별도의 창이 열립니다. 왼쪽 여백에서 프로젝트의 파일과 폴더 구조를 탐색할 수 있습니다.

![](/docs/_remote/89a525be913ad9dc95c15692.webp)

사용자 지정 코드를 `index.ts` 파일에 추가하세요. `/src` 안에 있는 `main()` 함수 내부에 넣으세요. 그 밖에 해당 함수 외부에 사용자 지정 코드를 추가하거나 새 `.ts` 파일을 `/src` 폴더 안에 만들 수도 있지만, 이 파일들은 `main()` 함수 안에서 어떤 식으로든 참조되어야 합니다. `index.ts`.


> [!WARNING]
> **📔 참고**: VS Code 또는 Cursor가 설치되어 있지만 **코드** 버튼으로 열리지 않는다면, 명령줄을 통해 열도록 VS Code가 컴퓨터에서 올바르게 구성되어 있지 않을 수 있습니다. 대부분의 경우 이는 기본 설치의 일부로 처리되지만, 그렇지 않다면 [VS에서 제공하는 이 안내를](https://code.visualstudio.com/docs/setup/mac#_launching-from-the-command-line) 참고해 명령줄에서 VS Code를 활성화하세요.


씬을 실행 중인 미리보기 창이 열려 있다면, 파일의 코드를 변경하고 저장할 때마다 씬이 변경 사항과 함께 자동으로 다시 로드됩니다.

## AI 사용하기

Creator Hub에는 내장된 [AI 어시스턴트](/docs/creator/content-creator-ko/scene-editor/extend-with-code/ai-assistant/) 가 있어 앱을 벗어나지 않고도 열린 씬을 보고 대신 편집해 줍니다. 이는 실험적 기능이며 기본적으로 꺼져 있습니다.

Cursor나 Claude Code 같은 AI 어시스턴트를 활용해 씬 코드를 작성할 수도 있습니다. 예를 들어 Cursor를 사용하려면 다음과 같이 하세요:

1. 다음의 버튼을 클릭해 Cursor AI 어시스턴트를 엽니다. **AI** 화면 오른쪽 상단의
2. 그곳에서 AI 어시스턴트에게 코드를 작성하도록 요청할 수 있습니다. 프롬프트에는 문서 페이지 링크, 프로젝트의 특정 파일 경로, 또는 이미지까지 포함할 수 있습니다. 드롭다운에서 사용할 특정 모델을 선택할 수도 있습니다.

AI 어시스턴트에 Decentraland 컨텍스트를 제공하려면 공식 Decentraland SDK Skills를 설치하세요. 이는 Decentraland Foundation이 관리하는 미리 만들어진 지침 세트로, 전체 씬 구성, 3D 모델 추가, 멀티플레이어, 배포 등 모든 주제에 대해 검증된 SDK7 패턴을 AI 에이전트에게 가르쳐 줍니다.

```bash
npx skills add decentraland/sdk-skills
```

참조 [AI와 함께하는 바이브 코딩](/docs/creator/content-creator-ko/sdk7/getting-started/vibe-coding/) 에서 사용 가능한 전체 스킬 목록을 포함한 완전한 가이드를 확인하세요.


> [!WARNING]
> **📔 참고**: 이전 씬 프로젝트에는 `/dclcontext` 폴더가 포함되어 있을 수 있으며, 이는 사용 중단된 `npx sdk-commands get-context-files` 명령으로 다운로드된 것입니다. 해당 컨텍스트는 더 이상 유지되지 않으며 SDK Skills로 대체되었습니다. 폴더를 안전하게 삭제할 수 있습니다.


## 버전 관리

프로젝트용 저장소를 GitHub에 만들고, 이를 사용해 프로젝트 버전을 추적하고 다른 사람과 협업하는 것을 권장합니다.

이 방법에 익숙하지 않다면 [저장소 빠른 시작](https://docs.github.com/en/repositories/creating-and-managing-repositories/quickstart-for-repositories)을 참고하거나, 더 간단한 GUI 기반 흐름을 원하면 [GitHub 데스크톱 애플리케이션](https://desktop.github.com/download/) 을 사용하세요.


> [!WARNING]
> **📔 참고**: 전체 프로젝트 폴더를 GitHub 저장소에 업로드하되, `/node_modules` 또는 `/bin` 폴더와 `package-lock.json` 파일이 모두 `.gitignore` 파일에 포함되어 있는지 확인해 동기화되지 않도록 하세요. 저장소 유형을 `node`로 구성하면 이렇게 됩니다. 이 파일들은 모두 자동 생성되며, 컴퓨터마다 내용이 다를 수 있습니다.


## 참고 항목

* [AI와 함께하는 바이브 코딩](/docs/creator/content-creator-ko/sdk7/getting-started/vibe-coding/): AI 어시스턴트에게 원하는 것을 설명해 씬을 만드세요.
* [스마트 아이템 - 기본](/docs/creator/content-creator-ko/scene-editor/interactivity/smart-items/)
* [스마트 아이템 - 고급](/docs/creator/content-creator-ko/scene-editor/interactivity/smart-items-advanced/)
* [상태와 조건](/docs/creator/content-creator-ko/scene-editor/interactivity/states-and-conditions/)
* [모든 항목을 스마트하게 만들기](/docs/creator/content-creator-ko/scene-editor/interactivity/make-any-item-smart/)
* [SDK 빠른 시작](/docs/creator/content-creator-ko/sdk7/getting-started/sdk-101/): 빠른 입문을 위해 이 짧은 튜토리얼을 따라가세요.
* [개발 워크플로](/docs/creator/content-creator-ko/sdk7/getting-started/dev-workflow/): 씬 생성 과정을 처음부터 끝까지 이해하려면 이것을 읽어보세요.
* [예제](/docs/creator/content-creator-ko/tutorials-and-examples/examples/): 실제 동작하는 예제 씬으로 바로 들어가 보세요.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/scene-editor/extend-with-code/overview.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
