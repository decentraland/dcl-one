> 아이디어 구상부터 테스트와 게시까지, Creator Hub로 Decentraland 씬을 만드는 권장 워크플로.

# 개발 워크플로

이 문서는 아이데이션부터 퍼블리싱 그 이후까지, Decentraland용 씬을 개발할 때 권장되는 단계를 설명합니다.

## Decentraland 씬은 어떻게 만들나요?

1. **설치하세요** [**Creator Hub**](https://decentraland.org/download/creator-hub), 씬을 만들고, 미리 보고, 퍼블리싱할 수 있는 공식 데스크톱 앱입니다. 모든 숙련도의 크리에이터에게 권장되는 도구입니다.
2. **AI 코딩 어시스턴트를 사용하는 경우** (예: Claude Code, Cursor 또는 Copilot) 공식 [Decentraland SDK Skills](/docs/creator/content-creator-ko/sdk7/getting-started/vibe-coding/) 를 설치해 검증된 SDK7 패턴을 알게 하세요: `npx skills add decentraland/sdk-skills`
3. **다음** [**CLI**](/docs/creator/content-creator-ko/sdk7/getting-started/using-the-cli/) 는 고급 사용자와 자동화된 워크플로를 위한 대안입니다.

아래 섹션에서는 이 워크플로의 각 단계를 자세히 설명합니다.

## Creator Hub 설치

Decentraland Creator Hub가 설치되어 있는지 확인하세요.

* [설치 가이드](/docs/creator/content-creator-ko/scene-editor/get-started/editor-installation/)

코드 작업을 할 예정이라면, 다음도 설치했는지 확인하세요 [Visual Studio Code](https://code.visualstudio.com/) 또는 [Cursor AI](https://www.cursor.com/).


> [!NOTE]
> **💡 팁**: 일반 언어 설명에서 씬 코드를 생성하는 데 AI 어시스턴트를 사용할 수도 있습니다. 시작 방법은 [AI와 함께하는 바이브 코딩](/docs/creator/content-creator-ko/sdk7/getting-started/vibe-coding/) 에서 확인하세요.


## 경험을 설계하세요

얼마나 많은 공간이 필요한지, 어떤 형태의 배치가 필요한지, 플레이어가 수행할 수 있기를 원하는 메커니즘은 무엇인지 등을 생각해 보세요. 다음 문서들이 가이드가 될 수 있습니다:

* [UX 및 UI 가이드](/docs/creator/content-creator-ko/sdk7/designing-the-experience/ux-ui-guide/)
* [게임을 위한 설계 제약](/docs/creator/content-creator-ko/sdk7/designing-the-experience/design-games/)
* [씬 MVP 지침](/docs/creator/content-creator-ko/sdk7/designing-the-experience/mvp-guidelines/)

## 퍼블리시할 위치

Decentraland에서는 콘텐츠가 제한된 공간의 인접한 땅 플롯에 퍼블리시됩니다. 플레이어는 한 곳에서 다른 곳으로 자유롭게 걸어갈 수 있습니다. 각 씬은 고유한 독립된 작은 세계이며, 한 씬의 아이템은 다른 씬으로 확장될 수 없고 각 씬의 코드는 다른 모든 씬과 샌드박스 처리됩니다.

이들 각각에 퍼블리시할 수 있는 권한은 토큰으로 관리됩니다. 씬을 개발하는 데 LAND는 필요하지 않지만, 퍼블리시할 준비가 되면 LAND에 대한 접근 권한이 필요합니다.

대안으로, Decentraland [Worlds](/docs/creator/content-creator-ko/sdk7/publishing/publishing-options/#decentraland-worlds), 즉 자체 포함형이고 격리된 씬에 퍼블리시할 수도 있습니다.

다음 옵션을 사용할 수 있습니다:

* LAND 임대
* LAND 구매
* 토지 소유자로부터 권한 얻기
* Decentraland World에 퍼블리시하려면 [worlds](/docs/creator/content-creator-ko/sdk7/publishing/publishing-options/#decentraland-worlds) 를 확인하세요.

다음을 참조하세요. [퍼블리싱 옵션](/docs/creator/content-creator-ko/sdk7/publishing/publishing-options/) 을 참고하세요.

## 템플릿과 예제

새 씬을 만들 때, 기본 코드와 3D 모델이 포함된 여러 기본 템플릿 씬 중에서 선택하세요. 이를 사용하면 더 빨리 시작할 수 있습니다.

* [예제 씬](/docs/creator/content-creator-ko/tutorials-and-examples/examples/): 여기에서 재사용하거나 참고할 수 있는 메커니즘을 보여 주는, 엄선된 작동 중인 씬과 저장소 모음을 찾을 수 있습니다.
* [도우미 라이브러리](https://studios.decentraland.org/resources?sdk_version=SDK7&resource_type=Library): 이는 많은 일반적인 작업을 단순화해 줍니다.

## 아트 에셋

숙련된 아티스트이거나 그런 사람의 도움을 받을 수 있다면, 씬을 위한 맞춤형 `.gltf` 또는 `.glb` 모델을 만들 수 있습니다. 다음을 확인하세요 [3D 모델 필수 사항](/docs/creator/content-creator-ko/3d/3d-models/) 에서 Decentraland용 3D 모델을 만드는 팁을 확인하세요.

하지만 직접 에셋을 만들 필요는 없습니다. 다음을 확인하세요 [유용한 리소스](/docs/creator/content-creator-ko/sdk7/getting-started/useful-resources/) 에서 3D 모델 소싱에 사용할 수 있는 에셋 라이브러리와 생성형 AI 도구, 그리고 워크플로를 더 빠르게 해 줄 다른 도구들을 확인할 수 있습니다.

## 로컬 미리보기 실행

씬 미리보기를 실행하려면 Creator Hub에서 열고 **실행** 버튼을 클릭하세요. 또는 명령줄에서 작업하는 경우 `npm run start` 를 프로젝트의 루트 폴더에서 실행하세요.

* [씬 미리보기](/docs/creator/content-creator-ko/sdk7/getting-started/preview-scene/) 을 참고하세요.
* 다음을 확인하세요 [씬 디버그](/docs/creator/content-creator-ko/sdk7/debugging/debug-in-preview/) 에서 문제를 디버그하는 방법에 대한 팁을 확인하세요.
* 활성화하세요 **에셋 압축** 다음의 **데스크톱 클라이언트** 의 하위 메뉴 **실행 옵션** (또는 다음을 실행하세요 `npm run start -- --local-ab` 를 CLI에서) 3D 모델을 최적화된 에셋 번들로 불러오세요. 미리보기는 더 부드럽게 실행되며, 퍼블리시된 후 보이게 될 모습 그대로 모델을 보여 줍니다. 다음을 확인하세요 [최적화된 에셋으로 미리 보기](/docs/creator/content-creator-ko/sdk7/getting-started/preview-scene/#preview-with-optimized-assets).


> [!NOTE]
> **💡 팁**: Creator Hub를 사용할 때 씬에서 변경을 할 때마다 미리보기가 자동으로 업데이트됩니다. 실행 중일 때도 마찬가지입니다.


## Decentraland에 퍼블리시

씬이 만족스럽다면, 이제 Decentraland에 퍼블리시할 차례입니다. 이를 위해서는 LAND, Decentraland NAME, ETH ENS 이름을 소유하고 있거나, 소유한 사람에게서 권한을 받아야 합니다.

다음을 참조하세요. [게시](/docs/creator/content-creator-ko/sdk7/publishing/publishing/) 에서 방법을 확인하세요.

대안으로, 다음에 퍼블리시할 수 있습니다 [Worlds](/docs/creator/content-creator-ko/sdk7/publishing/publishing-options/#decentraland-worlds), LAND가 필요 없는 개인용 3D 공간입니다.

## 홍보

이제 씬이 공개되었으니, 널리 알려 보세요! 다음은 그 방법입니다:

* 소셜 미디어에 공유하기(#DCL)
* 다음에 알리기 [Discord](https://dcl.gg/discord)
* 다음에 소개되도록 제출하기 [events.decentraland.org](https://events.decentraland.org/)
* 씬에서 이벤트를 조직하세요
* 씬으로 연결되는 유동 인구가 많은 지역에 스폰 지점을 추가하세요

## 반복 개선

씬이 한동안 라이브로 운영되고 플레이어의 피드백을 받았다면, 이를 바탕으로 개선하기에 아주 좋은 시점입니다!

개선 사항과 새 기능으로 콘텐츠를 업데이트하고, 같은 좌표에 씬의 새 버전을 배포하세요.

## 기여하기

자랑스러운 씬, 게임 또는 애플리케이션을 만들었다면 오픈 소스로 공개하는 것을 고려해 보세요! 그러면 다른 사람들이 여러분의 코드에서 배우고 작업을 발전시킬 수 있습니다. 전체 프로젝트를 [Awesome Repository](https://github.com/decentraland-scenes/Awesome-Repository).

재사용 가능한 기능을 만들었다면, 다른 사람들이 자신의 프로젝트에 가져다 사용할 수 있는 라이브러리로 만드는 것도 좋습니다.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/getting-started/dev-workflow.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
