> Decentraland 씬, 웨어러블, 이모트를 만드세요. Creator Hub는 권장 도구이며, AI 코딩 어시스턴트를 위한 SDK Skills와 고급 워크플로를 위한 CLI도 제공합니다.

# 크리에이터 환영

함께 Decentraland를 만들어 봅시다!

모든 크리에이터를 환영합니다! Decentraland에서는 다양한 재능과 숙련도를 가진 사람들을 위한 폭넓은 창작 가능성을 제공합니다!

Decentraland는 데스크톱(Windows 및 macOS)과 모바일 기기(iOS 및 Android)에서 이용할 수 있습니다. 플레이어는 동일한 계정, 아바타, 인벤토리로 클라이언트 간에 매끄럽게 이동합니다. 즉, 여러분의 씬은 플레이하는 기기와 상관없이 모두에게 잘 작동해야 합니다. 이 가이드 전반에서 지원되는 모든 플랫폼에서 씬이 훌륭하게 작동하도록 설계, 제작, 테스트하는 방법에 대한 권장 사항을 찾을 수 있습니다. 모바일 전용 안내는 [모바일용 제작](/docs/creator/content-creator-ko/build-for-mobile/mobile-client/overview/).

## 크리에이터 허브

크리에이터 허브는 모든 수준의 지식을 가진 크리에이터에게 권장되는 도구입니다. 장면을 만들 수 있는 데스크톱 애플리케이션입니다 [씬](#scenes).

![](/docs/_remote/32e1b985101d928b718678f9.webp)

Creator Hub 다운로드 [여기](https://decentraland.org/download/creator-hub).

## 웨어러블

웨어러블은 플레이어 아바타가 착용할 수 있는 의류 아이템입니다. 이는 NFT로 판매되며 다음에서 구매할 수 있습니다 [마켓플레이스](https://decentraland.org/marketplace/browse?section=wearables&vendor=decentraland&page=1&sortBy=newest&status=on_sale).

에 대해 모든 것을 알아보세요 [웨어러블 만들기](/docs/creator/content-creator-ko/wearables-and-emotes/wearables/creating-wearables/).

SDK의 코드와 웨어러블을 결합해 [스마트 웨어러블](/docs/creator/content-creator-ko/sdk7/kinds-of-projects/smart-wearables/). 플레이어가 웨어러블을 착용하면 전역 씬이 활성화됩니다. 참조하세요 [프로젝트 종류](/docs/creator/content-creator-ko/sdk7/kinds-of-projects/kinds-of-project/) 를 참고하여 다양한 옵션을 더 잘 이해하세요.

## 이모트

이모트는 플레이어의 아바타가 수행할 수 있는 애니메이션입니다. 이는 NFT로 판매되며 다음에서 구매할 수 있습니다 [마켓플레이스](https://decentraland.org/marketplace/browse?assetType=item&section=emotes&vendor=decentraland&page=1&sortBy=newest&status=on_sale).

에 대해 모든 것을 알아보세요 [이모트 만들기](/docs/creator/content-creator-ko/wearables-and-emotes/emotes/creating-emotes/).

## 씬

Decentraland의 3D 콘텐츠는 씬들로 이루어져 있으며, 각 씬은 제한된 공간을 차지하고 플레이어가 자유롭게 걸어다닐 수 있도록 서로 나란히 표시됩니다.

크리에이터 허브를 사용하면 간편한 드래그 앤 드롭 인터페이스로 씬을 만들고, 상호작용을 완전히 제어할 수 있도록 코드를 편집할 수도 있습니다. 미리 보기 실행, 디버그, 코드 편집, 게시가 가능합니다.

[자세히 알아보기](/docs/creator/content-creator-ko/scene-editor/get-started/about-editor/)

### Decentraland 씬은 어떻게 만들나요?

1. **먼저** [**크리에이터 허브**](https://decentraland.org/download/creator-hub), 씬을 만들고, 미리 보고, 게시하기 위한 공식 데스크톱 앱입니다. 모든 숙련도의 크리에이터에게 권장되는 도구입니다.
2. **AI 코딩 어시스턴트를 사용하는 경우** (Claude Code, Cursor, Copilot 같은) 공식 [Decentraland SDK 스킬](/docs/creator/content-creator-ko/sdk7/getting-started/vibe-coding/) 를 설치하여 검증된 SDK7 패턴을 알 수 있도록 하세요: `npx skills add decentraland/sdk-skills`
3. **해당** [**CLI**](/docs/creator/content-creator-ko/sdk7/getting-started/using-the-cli/) 는 고급 사용자와 자동화 워크플로를 위한 대안입니다.

### 3D 아트

Decentraland 씬은 3D 모델로 구성됩니다.

* 씬 에디터의 다양한 기본 자산 카탈로그에서 선택하세요. 이들은 바로 사용할 수 있으며 Decentraland에서 사용하도록 최적화되어 있습니다

  ![](/docs/_remote/b193f61ea826d9783815e90e.webp)
* Blender 또는 선호하는 3D 도구를 사용해 직접 3D 모델을 만드세요. 그런 다음 씬 에디터로 가져오세요.

  ![](/docs/_remote/6469c594dcf3e77e830e77da.webp)


> [!WARNING]
> **📔 참고**: Decentraland의 콘텐츠는 특정 [크기 제한](/docs/creator/content-creator-ko/sdk7/optimizing/scene-limitations/) 을 준수해야 씬이 원활하게 실행됩니다.
> 
> 참조 [3D 모델링](/docs/creator/content-creator-ko/3d/3d-models/) 최적화 팁과 요령, 그리고 3D 모델의 지원 기능 및 형식에 대한 정보는


### 상호작용

씬을 상호작용 가능하게 만들려면:

* **코드 없음**: 씬 에디터의 UI를 사용해 [스마트 아이템입니다](/docs/creator/content-creator-ko/scene-editor/interactivity/smart-items/) 를 씬에 배치하세요. 이는 자체 동작이 미리 내장된 모델이며, 높은 수준으로 사용자 정의할 수 있습니다. 또한 동일한 동작을 자신만의 커스텀 모델에 할당할 수 있습니다(코드 불필요).

  ![](/docs/_remote/a417642641d975327ba066bf.webp)
* **코드**: 사용자 지정 로직을 통합하고 싶은 개발자는 SDK를 사용해 코드를 작성하고 상상할 수 있는 모든 것을 구현하세요. SDK 사용법 알아보기:
  * [SDK 빠른 시작](/docs/creator/content-creator-ko/sdk7/getting-started/sdk-101/): 이 미니 튜토리얼을 따라 빠르게 핵심을 익혀 보세요.
  * [개발 워크플로](/docs/creator/content-creator-ko/sdk7/getting-started/dev-workflow/): 처음부터 끝까지 씬 제작 과정을 이해하려면 이것을 읽어보세요.
  * [AI와 함께하는 바이브 코딩](/docs/creator/content-creator-ko/sdk7/getting-started/vibe-coding/): 원하는 것을 평범한 언어로 설명해 씬을 만들고, AI 코딩 어시스턴트가 대신 코드를 작성하도록 하세요.
  * [예시](/docs/creator/content-creator-ko/tutorials-and-examples/examples/): 바로 작동하는 예시 씬으로 들어가 보세요.

    ![](/docs/_remote/3d5fae2810e08190c0c1ad38.webp)


> [!WARNING]
> **📔 참고**: 또한 [Visual Studio Code](https://code.visualstudio.com/) 설치되어 있어야 합니다.


### 씬 게시

씬 에디터로 씬 제작을 시작하는 데 토큰을 소유할 필요는 없습니다. 씬을 게시하려면 다음 옵션 중에서 선택할 수 있습니다:

* **제네시스 시티의 LAND**: Decentraland의 주요 오픈 월드로, 16x16미터 크기의 구획으로 나뉘어 있습니다. 다음에서 인접한 한 구획 또는 여러 구획을 구매하세요 [마켓플레이스](https://decentraland.org/marketplace/lands), 그리고 그곳에 씬을 배포하세요.
* **Decentraland Worlds**: [Worlds](/docs/creator/content-creator-ko/sdk7/publishing/publishing-options/#decentraland-worlds) 는 메타버스에서 여러분만의 공간입니다. 필요한 것은 [Decentraland 이름](https://decentraland.org/marketplace/names/claim)을 소유하는 것뿐이며, 원하는 만큼 큰 씬을 게시할 수 있습니다!

참조 [프로젝트 종류](/docs/creator/content-creator-ko/sdk7/kinds-of-projects/kinds-of-project/) 를 참고하여 다양한 옵션을 더 잘 이해하세요.

참조 [게시](/docs/creator/content-creator-ko/sdk7/publishing/publishing/) 씬을 제네시스 시티 또는 Worlds에 게시할 때의 세부 사항과 특별 옵션은

## 유용한 리소스

확인해 보세요 [유용한 리소스](/docs/creator/content-creator-ko/sdk7/getting-started/useful-resources/) 창작 워크플로를 빠르게 진행할 수 있는 도구, 애드온, 에셋 라이브러리, 예제 프로젝트의 엄선된 목록을 위해


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/readme.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
