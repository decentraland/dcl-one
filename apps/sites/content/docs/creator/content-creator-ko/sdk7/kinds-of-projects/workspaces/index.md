> 여러 DCL 프로젝트를 동시에 실행합니다

# 작업공간

미리 보기에서 여러 Decentraland 프로젝트를 작업 공간으로 묶어 실행하세요. 여러 인접한 씬을 실행해 어떻게 맞물리는지 확인하거나, 또한 여러 [스마트 웨어러블](/docs/creator/content-creator-ko/sdk7/kinds-of-projects/smart-wearables/) 함께 실행해 서로 그리고 다른 씬과 어떻게 상호작용하는지 확인하세요.

작업 공간에서 여러 프로젝트를 실행하면 서로 다른 콘텐츠가 함께 잘 작동하는지 확인할 수 있는 훨씬 더 완전한 테스트 방법을 제공합니다. 작업 공간은 디버깅 기능이며, 게시된 씬의 경험에는 영향을 주지 않습니다.


> [!WARNING]
> **📔 참고**: **Creator Hub** 현재는 작업 공간 처리를 지원하지 않습니다.


## 작업 공간 만들기

1. 다운로드하세요 [Goerli Plaza](https://github.com/decentraland/sdk7-goerli-plaza) 저장소.
2. 작업 공간을 담을 별도의 최상위 폴더를 만드세요.
3. Goerli Plaza 저장소에서 다음 파일을 작업 공간으로 복사하세요:
   * `dcl-workspace.json`
   * `package.json`
   * `.gitignore`
4. 이 폴더 안에 작업하려는 각 프로젝트마다 루트 수준에 폴더를 하나씩 추가하세요. 씬이나 스마트 웨어러블이 들어 있는 기존 폴더를 끌어다 놓을 수 있습니다. 새 폴더의 경우, 다음을 실행하세요 `npx @dcl/sdk-commands init` 각각 안에서 실행해 Decentraland 프로젝트를 만드세요.

   > 참고: 각 씬의 토지가 서로 겹치지 않도록 하세요.
5. 작업 공간 폴더에서 다음을 실행해 필요한 파일을 만드세요:

   `npm run update-parcels && npm run sync && npm run test && npm run format`

다음을 열어 작업 공간에 포함된 프로젝트를 확인할 수 있습니다: `dcl-workspace.json` 파일을 열고 그 안에 나열된 경로를 확인하세요. `폴더` 배열.

## 작업 공간 실행

실행 `npm run start` 작업 공간의 루트 폴더에서 실행하세요. 이렇게 하면 모든 프로젝트가 동시에 실행되며, 하나의 미리 보기 창에서 볼 수 있습니다. 이 미리 보기는 단일 씬을 미리 볼 때와 똑같이 작동합니다.

작업 공간에 있는 모든 스마트 웨어러블은 백팩에서 찾아 착용해 볼 수 있습니다.

## 프로젝트 추가

작업 공간이 생성되면, 다음을 편집해 추가 프로젝트를 추가할 수 있습니다. `dcl-workspace.json` 파일을 수동으로 편집하세요. 다음에 항목을 추가하세요. `폴더` 추가하려는 폴더의 상대 경로를 사용한 배열. 예를 들어 `my-other-example`.

절대 경로를 사용해 작업 공간 폴더 밖에 있는 프로젝트도 추가할 수 있습니다.


> [!WARNING]
> **📔 참고**: 폴더에는 이미 다음으로 초기화된 Decentraland 프로젝트가 있어야 합니다 `npx @dcl/sdk-commands init`. 비어 있는 폴더이면 안 됩니다.


프로젝트를 추가하거나 제거하려면 다음을 편집하세요. `dcl-workspace.json` 파일에 작업 공간의 각 프로젝트에 대한 상대 경로를 포함시키세요. `폴더` 배열.

```json
{
	"folders": [
		{
			"path": "example-scene"
		},
		{
			"path": "example-scene2"
		}
	]
}
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/kinds-of-projects/workspaces.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
