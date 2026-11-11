> Decentraland CLI를 사용해 씬을 실행하고 배포합니다. Creator Hub는 대부분의 크리에이터에게 권장되는 도구이며, CLI는 고급 및 자동화 워크플로에 적합합니다.

# CLI 사용


> [!NOTE]
> **💡 팁**: AI 코딩 도우미(Claude Code, Cursor, Copilot 등)를 사용 중이라면, 코드가 작성되기 전에 공식 Decentraland SDK Skills를 설치해 검증된 SDK7 패턴을 알게 하세요:
> 
> ```bash
> npx skills add decentraland/sdk-skills
> ```
> 
> 참조 [AI와 함께하는 바이브 코딩](/docs/creator/content-creator-ko/sdk7/getting-started/vibe-coding/) 전체 AI 지원 워크플로우를 보려면.


Decentraland 씬을 빌드하려면 다음 중 하나를 사용할 수 있습니다:

* 해당 [크리에이터 허브](/docs/creator/content-creator-ko/scene-editor/get-started/editor-installation/)
* 명령줄 인터페이스(CLI)

두 도구 모두 씬을 "오프체인" 개발 환경에서 컴파일하고 미리 볼 수 있게 해줍니다. 씬을 로컬에서 테스트한 후에는 콘텐츠를 콘텐츠 서버에 업로드하여 LAND 또는 WORLD와 연결할 수 있습니다.

Creator Hub의 Scene Editor는 사용하기 더 쉽지만, CLI는 더 큰 유연성을 제공하며 자동화된 프로세스에서도 쉽게 사용할 수 있습니다.


> [!WARNING]
> **📔 참고**: Scene Editor는 뒤에서 동일한 명령줄 작업을 실행합니다.



> [!NOTE]
> **💡 팁**: 참조 [설치 가이드](/docs/creator/content-creator-ko/scene-editor/get-started/editor-installation/) Creator Hub를 설치하는 방법에 대한 지침은.


## 시작하기 전에

명령줄로 씬을 다루려면, CLI 명령을 씬과 함께 실행하기 전에 다음 종속성을 설치하세요:

* [Node.js](https://nodejs.org) (버전 20 이상)

## 새 프로젝트 시작하기

실행 `npx @dcl/sdk-commands init` 빈 폴더에서 실행하여 Decentraland의 기본 파일로 채우세요 [씬](/docs/creator/content-creator-ko/sdk7/kinds-of-projects/scene-metadata/) 프로젝트. 또한 다음을 사용하여 새 씬을 만들 수 있습니다. **새 씬** Creator Hub의 버튼으로, 동일한 프로젝트 구조를 생성합니다.

다른 종류의 프로젝트로 시작하려면 다음을 사용하세요. `--project` 플래그를 사용하세요. 예를 들어, 다음을 만들려면 [스마트 웨어러블](/docs/creator/content-creator-ko/sdk7/kinds-of-projects/smart-wearables/) 프로젝트:

```bash
npx @dcl/sdk-commands init --project smart-wearable
```

사용 가능한 옵션은 다음과 같습니다 `--project` 입니다 `scene-template` (기본값), `px-template`, `smart-wearable`, 그리고 `library`.

## 씬의 SDK 버전 업데이트하기

씬 폴더에서 다음 명령을 실행하세요:

```bash
npm i @dcl/sdk@latest
```

다음에서 확인하여 제대로 작동했는지 확인할 수 있습니다 `package.json` 씬의 파일을 확인하고, 다음을 찾아보세요 `@dcl/sdk` 그곳의 버전.

## 미리보기 실행하기

실행 `npm run start` 씬, 작업 공간 또는 스마트 웨어러블 프로젝트의 루트 수준에서 실행하여 Decentraland 데스크톱 클라이언트에서 미리보기를 여세요.

```bash
npm run start
```

씬을 미리 보려면 [Decentraland 모바일 앱](/docs/creator/content-creator-ko/build-for-mobile/mobile-client/overview/) 대신 다음을 실행하세요 `npm run start -- --mobile` (별칭 `-- -m`). CLI는 컴퓨터와 동일한 Wi-Fi 네트워크에 연결된 휴대폰에서 씬을 여는 QR 코드를 출력합니다. 참조 [모바일에서 미리보기](/docs/creator/content-creator-ko/build-for-mobile/develop/preview-on-mobile/) 전체 가이드를 보려면.

```bash
npm run start -- --mobile
```

미리보기를 더 부드럽게 실행하고 게시된 후의 모습과 똑같이 3D 모델을 렌더링하려면 다음을 추가하세요 `--asset-bundles` 플래그를 추가하세요. 참조 [최적화된 에셋으로 미리보기](/docs/creator/content-creator-ko/sdk7/getting-started/preview-scene/#preview-with-optimized-assets).

```bash
npm run start -- --asset-bundles
```

참조 [미리보기 씬](/docs/creator/content-creator-ko/sdk7/getting-started/preview-scene/) 미리보기를 실행할 때의 자세한 내용과 특수 옵션은.

## 빌드

실행 `npm run build` 프로젝트를 빌드하려면. Decentraland 씬은 TypeScript로 작성되지만, 게시될 때는 축소된 JavaScript로 빌드됩니다. 참조 [씬 코딩](https://github.com/decentraland/docs/blob/main/creator/sdk7/getting-started/coding-scenes.md) 자세한 내용은.

빌드 명령은 선택 사항입니다. 배포 전에 백그라운드에서도 실행되기 때문입니다(비활성화 플래그를 추가할 수는 있지만).

빌드 명령은 `npm run start`과 함께 실행되는 것보다 더 엄격한 타입 검사를 수행하므로, 씬 디버깅에 도움이 될 때가 있습니다.

## 씬 배포하기

실행 `npm run deploy` 를 실행하여 씬을 Decentraland에 게시하세요. 이 명령은 배포를 승인하기 위해 지갑으로 서명할 수 있는 브라우저 창을 엽니다.

참조 [게시](/docs/creator/content-creator-ko/sdk7/publishing/publishing/) 씬 게시에 대한 자세한 내용과 특수 옵션은.

## 문제 해결

문제가 발생하면 다음을 참조하세요. [문제 해결](/docs/creator/content-creator-ko/sdk7/debugging/troubleshooting/) 섹션.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/getting-started/using-the-cli.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
