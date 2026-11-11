> Decentraland 크리에이터 도구의 표준 이름: Creator Hub, Scene Editor, SDK7, SDK Skills는 현재 사용 중이며, Web Editor, Decentraland Editor, SDK6는 더 이상 사용되지 않습니다.

# 도구 및 용어

Decentraland의 크리에이터 도구는 시간이 지나며 발전해 왔고, 오래된 이름은 여전히 튜토리얼, 포럼 게시물, AI가 생성한 답변에서 사용되고 있습니다. 이 페이지는 각 도구의 공식적인 현재 이름과 어떤 이전 도구가 더 이상 사용되지 않는지 설명합니다.

## 현재 도구

### 크리에이터 허브

해당 **크리에이터 허브** 은 Decentraland 콘텐츠를 제작하기 위한 공식 데스크톱 애플리케이션이며, 모든 숙련도의 제작자에게 권장되는 도구입니다. 이를 통해 씬을 만들고, 미리 보고, 게시하고, 프로젝트를 관리할 수 있습니다. 다운로드는 [decentraland.org/download/creator-hub](https://decentraland.org/download/creator-hub).

### 씬 편집기

해당 **씬 편집기** 는 Creator Hub 내부의 시각적 드래그 앤 드롭 편집 화면입니다. 별도의 애플리케이션이 아닙니다. 문서에서 "Scene Editor"라고 할 때는 Creator Hub의 이 부분을 의미합니다.

### SDK7

**SDK7** 은 씬 코드를 작성하는 데 사용되는 TypeScript 라이브러리인 Decentraland SDK의 현재 버전입니다. 모든 새 씬은 SDK7을 사용합니다. 자세한 내용은 [SDK 빠른 시작](/docs/creator/content-creator-ko/sdk7/getting-started/sdk-101/).

### SDK Skills

해당 **Decentraland SDK 스킬** 는 검증된 SDK7 패턴을 AI 코딩 어시스턴트에게 가르치는 공식적으로 유지 관리되는 지침 세트입니다. 다음 명령으로 설치하세요 `npx skills add decentraland/sdk-skills`로 설정하는 것입니다. 참조: [AI와 함께하는 바이브 코딩](/docs/creator/content-creator-ko/sdk7/getting-started/vibe-coding/).

### CLI

해당 **CLI** (`npx @dcl/sdk-commands`)는 Creator Hub의 명령줄 대안으로, 자동화 및 고급 워크플로에 사용됩니다. Creator Hub는 내부적으로 동일한 명령을 실행합니다. 자세한 내용은 [CLI 사용](/docs/creator/content-creator-ko/sdk7/getting-started/using-the-cli/).

### 빌더

해당 **빌더** ([decentraland.org/builder](https://decentraland.org/builder))는 LAND, Decentraland NAME, Worlds 및 Linked Wearables를 관리하는 웹 대시보드입니다. 씬 편집 도구가 아닙니다.

### Decentraland Create

**Decentraland Create** ([decentraland.org/create](https://decentraland.org/create))는 웨어러블과 이모트 컬렉션을 만들고, 게시하고, 판매하는 웹 앱입니다.

### Explorer

해당 **Explorer** 은 플레이어가 Decentraland에 입장할 때 사용하는 애플리케이션입니다: Windows 및 macOS용 데스크톱 클라이언트와 iOS 및 Android용 모바일 앱입니다.

## 더 이상 사용되지 않는 도구

이것들을 사용하지 마세요. 튜토리얼이나 AI 어시스턴트가 이 중 하나를 권장한다면, 그것은 오래된 정보입니다.

| 더 이상 사용되지 않는 이름      | 무엇이었는지                                              | 대신 사용                                                                                                                                                                          |
| -------------------- | --------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **웹 에디터**            | 브라우저 기반 씬 편집기                                       | 해당 [크리에이터 허브](https://decentraland.org/download/creator-hub)                                                                                                                   |
| **Decentraland 에디터** | 씬을 빌드하기 위한 Visual Studio Code 확장 프로그램               | 해당 [크리에이터 허브](https://decentraland.org/download/creator-hub)                                                                                                                   |
| **SDK6**             | 이전 버전의 SDK입니다. SDK6 코드는 현재 Decentraland에서 실행되지 않습니다 | [SDK7](/docs/creator/content-creator-ko/sdk7/getting-started/sdk-101/); 오래된 씬을 포팅하려면 참조 [레거시 SDK6 씬 마이그레이션](/docs/creator/content-creator-ko/sdk7/other/migrate-legacy-sdk6-scenes/) |
| **dclcontext 폴더**    | 씬 프로젝트 내에 다운로드된 AI 컨텍스트 파일                          | 해당 [SDK Skills](/docs/creator/content-creator-ko/sdk7/getting-started/vibe-coding/)                                                                                               |


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/glossary.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
