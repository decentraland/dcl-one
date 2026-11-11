> AI 도우미를 사용해 기존 SDK6 씬을 SDK7로 마이그레이션합니다.

# 기존 SDK6 씬 마이그레이션

오래된 SDK6 씬을 최신 상태로 업데이트하고 싶다면 AI 어시스턴트의 도움을 받아 SDK7로 자동 마이그레이션할 수 있습니다. 그 [Decentraland SDK 스킬](/docs/creator/content-creator-ko/sdk7/getting-started/vibe-coding/#install-skills-for-any-ai-agent) 에는 AI가 단계별로 따라가는 전용 마이그레이션 워크플로가 포함되어 있어, 코드를 직접 번역할 필요가 없습니다.

이 가이드는 전체 과정을 단계별로 안내합니다.

## 1. 새 빈 SDK7 씬 만들기

새롭고 빈 SDK7 프로젝트로 시작하세요. AI가 기존 코드와 에셋을 이 깔끔한 구조로 옮깁니다.

**옵션 A — Creator Hub**

1. Creator Hub를 여세요.
2. 다음 **씬** 탭을 선택하고 **새 씬**.
3. 다음 **빈 씬** 템플릿.

**옵션 B — 명령줄**

빈 폴더에서 다음을 실행하세요:

```bash
npx @dcl/sdk-commands init
```

그러면 해당 폴더에 SDK7 프로젝트의 기본 파일이 생성됩니다.

## 2. SDK 스킬 설치

SDK 스킬은 AI 에이전트가 Decentraland SDK7로 작업하는 방법, 특히 올바른 SDK6 → SDK7 마이그레이션을 수행하는 방법을 익히게 합니다.

설치는 두 가지 방법으로 할 수 있습니다.

**옵션 A — AI에게 설치를 요청하기**

씬 폴더에서 AI 어시스턴트를 열고 다음과 같이 말하세요:

> Decentraland SDK 스킬을 설치해 주세요.

AI는 이 명령을 알고 있으며 대신 설치해 줄 것입니다.

**옵션 B — 명령줄에서 설치하기**

```bash
npx skills add decentraland/sdk-skills --all
```

해당 `--all` 플래그는 사용 가능한 모든 스킬을 설치합니다. 이 플래그를 생략하면 설치할 스킬을 선택할 수 있는 대화형 선택기가 열립니다.

사용 가능한 스킬과 동작 방식에 대한 자세한 내용은 다음을 참조하세요. [AI와 함께하는 바이브 코딩](/docs/creator/content-creator-ko/sdk7/getting-started/vibe-coding/#install-skills-for-any-ai-agent).

## 3. AI에게 씬 마이그레이션을 요청하기

기존 SDK6 프로젝트 파일을 AI가 읽을 수 있는 곳에 두세요. 예를 들어 컴퓨터의 별도 폴더나 공개 GitHub 저장소에 두면 됩니다.

그런 다음 AI에게 다음과 같이 요청하세요:

> 이 SDK6 씬을 SDK7로 마이그레이션해 주세요.

스킬에는 전체 마이그레이션 가이드가 포함되어 있으므로, AI는 다음을 수행합니다:

* SDK6 씬에서 사용된 진입점과 컴포넌트를 식별합니다.
* 엔티티, 컴포넌트, 시스템을 SDK7에 해당하는 항목으로 변환합니다.
* 3D 모델, 오디오 및 기타 에셋을 새 프로젝트로 옮깁니다.
* 업데이트 `scene.json` 및 프로젝트 설정.
* 더 이상 사용되지 않는 API를 현재 SDK7 대응 항목으로 교체합니다.

## 4. 테스트하고 반복하기

마이그레이션 작업이 끝나면 씬을 미리 보고 직접 플레이해 보세요.

* Creator Hub에서 다음을 클릭하세요: **Play**.
* 명령줄에서 다음을 실행하세요: `npm run start`.

테스트하면서 다음을 확인하세요:

* 시각적 문제 — 누락된 모델, 잘못된 위치나 크기, 깨진 재질.
* 상호작용 문제 — 클릭이 작동하지 않음, 트리거가 활성화되지 않음, 대화 상자가 나타나지 않음.
* 씬에서 사용했다면 멀티플레이어나 네트워킹 문제도 확인하세요.
* 미리보기 창의 콘솔 오류.

버그를 발견하면 자연어로 AI에게 설명하세요:

> 테이블 위의 빨간 버튼이 더 이상 문을 열지 않습니다. 예전에는 소리를 재생하고 문을 90도 회전시켰어요.

AI는 주변 스킬(상호작용, 애니메이션, 오디오 등)을 사용해 문제를 하나씩 수정할 수 있습니다. 씬이 원본처럼 동작할 때까지 반복하세요.


> [!NOTE]
> **💡 팁**: 한 번에 하나의 버그만 수정하고, 변경할 때마다 미리보기로 확인하세요. 변경 사항이 작을수록 회귀를 잡아내기 훨씬 쉽습니다.


## 다음 단계

* [AI와 함께하는 바이브 코딩](/docs/creator/content-creator-ko/sdk7/getting-started/vibe-coding/) — AI 어시스턴트와 SDK 스킬을 사용하는 방법에 대한 전체 참고 자료.
* [SDK 빠른 시작](/docs/creator/content-creator-ko/sdk7/getting-started/sdk-101/) — SDK7 기초를 익혀 마이그레이션된 코드를 검토하고 다듬을 수 있습니다.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/other/migrate-legacy-sdk6-scenes.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
