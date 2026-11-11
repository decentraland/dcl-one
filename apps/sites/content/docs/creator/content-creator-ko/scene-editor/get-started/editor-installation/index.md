> 씬 에디터 설치 방법.

# Creator Hub 설치

![크리에이터 허브](/docs/_remote/9100f2b541affb1c6fecfe89.webp)

Creator Hub 다운로드 [여기](https://decentraland.org/download/creator-hub).

## Creator Hub 업데이트

Creator Hub 애플리케이션은 열 때마다 업데이트를 확인하며, 새 버전이 있으면 자동으로 업데이트됩니다.

수동으로도 업데이트를 확인할 수 있습니다. 이를 위해,

1. 화면 왼쪽 상단의 Creator Hub 로고를 클릭하여 메인 메뉴를 엽니다.
2. 클릭 **업데이트 확인**.

![왼쪽 상단의 로고에서 열리는 Creator Hub 메인 메뉴로, 옵션은 Creator Hub 정보, 업데이트 확인, 설정, 버그 신고, 도움말 및 지원입니다.](/docs/_remote/1394f3aa47147ab15685f4dd.webp)

메인 메뉴에서는 앱의 **설정**와 버그를 신고하거나 지원을 받을 수 있는 링크도 찾을 수 있습니다.

## 코드 편집

씬에서 코드를 읽고 편집할 계획이라면, 다음 중 하나도 설치해야 합니다:

* <img src="/docs/_remote/358795a1c08dfc479355a174.webp" alt="VS Code" data-size="line"> [Visual Studio Code](https://code.visualstudio.com/): 이 옵션은 숙련된 개발자에게 권장됩니다.
* <img src="/docs/_remote/9510a847d1493f0d8b4cf7e3.webp" alt="Cursor" data-size="line"> [Cursor AI](https://www.cursor.com/): 이 도구는 AI와 통합된 강력한 코드 편집기입니다. 코드 작성을 도와줄 다양한 AI 모델을 선택할 수 있으며, 무료 등급도 제공되고 더 고급 모델은 유료 플랜이 필요합니다. Decentraland 또는 TypeScript를 처음 사용하는 개발자에게 적합한 옵션이며, 코드 작성 시간을 절약하고 싶을 때도 좋습니다.

Creator Hub 설정에서 코드 편집기를 선택해야 할 수도 있습니다. 이를 위해,

1. 화면 왼쪽 상단의 Creator Hub 로고를 클릭하고 다음을 선택합니다: **설정**.
2. 다음을 엽니다 **EDITOR** 탭에 표시됩니다.
3. 에서 **선호하는 코드 편집기**, 코드 편집기를 선택합니다. 드롭다운에 편집기가 표시될 수도 있고, 다음을 선택해야 할 수도 있습니다: **기기에서 선택...** 에서 찾아보세요.

![EDITOR 탭의 앱 기본 설정 대화상자로, '선호하는 코드 편집기' 드롭다운이 Visual Studio Code로 설정되어 있고, Play Options 체크박스와 App Warnings 체크박스가 표시됩니다.](/docs/_remote/be0b7d493dc2e145af5207ac.webp)

### AI 스킬

코드 작성에 도움이 되도록 AI 에이전트를 사용할 계획이라면, Decentraland SDK 스킬을 설치하는 것을 권장합니다. 이는 AI 에이전트가 Decentraland SDK를 사용하는 방법을 학습하도록 돕는 미리 만들어진 지침 세트로, 실수를 줄이고 첫 프롬프트부터 더 나은 결과를 제공합니다. 모두 설치하려면 씬 프로젝트 폴더에서 다음 명령을 실행하세요:

```bash
npx skills add decentraland/sdk-skills --all
```

참조 [바이브 코딩](/docs/creator/content-creator-ko/sdk7/getting-started/vibe-coding/) 특정 스킬 설치 방법과 사용 방법에 대한 자세한 내용은

## 문제 해결

문제가 발생하면 다음을 참조하세요. [문제 해결](/docs/creator/content-creator-ko/sdk7/debugging/troubleshooting/) 섹션.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/scene-editor/get-started/editor-installation.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
