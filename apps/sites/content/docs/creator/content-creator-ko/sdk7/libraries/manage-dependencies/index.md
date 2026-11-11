> 씬에 라이브러리를 추가하는 방법

# 종속성 관리

종종 여러분의 씬에는 이미 재사용 가능한 라이브러리에 캡슐화되어 있는 기능이 필요할 수 있습니다. 외부 라이브러리를 사용하면 작업이 훨씬 쉬워질 수 있습니다.

Decentraland 프로젝트에 외부 라이브러리를 가져온 다음, 해당 라이브러리에 설명된 시스템, 컴포넌트 또는 함수를 참조할 수 있습니다.

확인하세요 [예제 페이지](https://studios.decentraland.org/resources?sdk_version=SDK7&resource_type=Library) 일반적인 문제를 해결하는, Decentraland Foundation과 커뮤니티 구성원들이 만든 다양한 라이브러리를 찾을 수 있습니다.

## CLI를 통해

### 설치

프로젝트 폴더에 라이브러리를 설치하려면 다음을 실행하세요 `npm i` 그리고 프로젝트의 루트 경로에서 라이브러리 이름을 입력하세요. 예를 들면:

`npm i @dcl-sdk/utils`

> **📔 참고**: 라이브러리를 설치한 후 라이브러리의 종속성이 설치되도록 하려면 `npm run start` 또는 `npm run build` 라이브러리를 설치한 후 라이브러리의 종속성이 설치되도록 하려면 다음을 실행해야 할 수도 있습니다.

### 업데이트

만약 여러분의 `package.json` 파일이 라이브러리 버전을 다음과 같이 지정하고 있다면 `@latest`, 그러면 다음을 실행하세요 `npm i` 모든 라이브러리를 해당하는 최신 버전으로 업데이트합니다.

만약 여러분의 `package.json` 특정 버전 번호를 참조하는 경우, 라이브러리를 설치하는 명령을 실행할 때 다음을 추가하여 업데이트할 수 있습니다 `@` 마지막에 버전 번호를 추가하세요. 예를 들면:

`npm i @dcl-sdk/utils@1.7.5`

### 삭제

씬의 종속성에서 라이브러리를 삭제하려면 다음을 실행하세요 `npm rm` 그리고 라이브러리 이름을 입력하세요. 예를 들면:

`npm rm @dcl-sdk/utils`

사용하지 않는 라이브러리는 제거하는 것이 좋습니다. 사용하지 않는 라이브러리도 여전히 씬 공간을 차지하여, 플레이어가 씬을 다운로드하고 실행하는 경험에 영향을 미칩니다.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/libraries/manage-dependencies.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
