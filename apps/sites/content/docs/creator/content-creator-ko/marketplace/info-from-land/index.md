> CLI를 사용해 랜드 구획, 에스테이트 또는 주소에서 정보를 가져옵니다.

# LAND 정보

Decentraland CLI를 사용하면 블록체인의 LAND 토큰과 콘텐츠 서버에 업로드된 씬 파일에서 정보를 직접 조회할 수 있습니다.

이 명령을 실행하려면 먼저 [CLI를 설치하세요](/docs/creator/content-creator-ko/sdk7/getting-started/using-the-cli/).

## 씬에 대한 정보를 가져오기

해당 `dcl info` 명령은 씬의 파일의 내용을 반환합니다 *scene.json* 파일에는 소유자, 연락처 정보, 그리고 씬의 파슬이 포함되어 있습니다.

* `dcl info` 씬의 폴더에서 실행하면 해당 특정 씬에 대한 정보를 반환합니다.
* `dcl info x,y`로 이름 지어야 하며, 여기서 *x* 및 *y* 는 파슬 좌표이며, 해당 위치의 씬에 대한 정보를 반환합니다.

  {% hint style="warning" %} **📔 참고**: 두 좌표 사이에 공백을 추가하지 마세요. {% endhint %}
* `dcl info id`로 이름 지어야 하며, 여기서 *id* 는 estate ID이며, 해당 estate ID를 가진 씬에 대한 정보를 반환합니다.

해당 `dcl status` 명령은 씬에 배포된 파일 목록을 반환합니다. 여기에는 파일 이름과 크기만 포함됩니다. CLI를 통해 파일 내용에 접근할 수는 없습니다.

* `dcl status` 씬의 폴더에서 실행하면 해당 특정 씬에 대한 정보를 반환합니다.
* `dcl status x,y`로 이름 지어야 하며, 여기서 *x* 및 *y* 는 파슬 좌표이며(서로 사이에 공백이 없습니다), 해당 위치의 씬에 대한 정보를 반환합니다.


> [!WARNING]
> **📔 참고**: 두 좌표 사이에 공백을 추가하지 마세요.


* `dcl status id`로 이름 지어야 하며, 여기서 *id* 는 estate ID이며, 해당 estate ID를 가진 씬에 대한 정보를 반환합니다.


> [!WARNING]
> **📔 참고**: 콘텐츠 서버에 업로드된 모든 것은 공개되어 있으며 해당 네트워크를 통해 접근할 수 있습니다. 씬을 배포할 때 기본적으로 씬의 원본 소스 코드를 업로드하는 것이 아니라, 축소된 JavaScript로 컴파일된 버전을 업로드하게 되며, 이는 훨씬 덜 읽기 쉽습니다.


## 지갑 주소에서 정보 가져오기

해당 `dcl info` 명령을 사용하면 이더리움 지갑과 그 지갑이 소유한 LAND 토큰에 대한 정보도 볼 수 있습니다.

* `dcl info xxxx`로 이름 지어야 하며, 여기서 *xxxx* 는 관심 있는 이더리움 주소이며, 해당 주소가 소유한 모든 LAND 파슬과 에스테이트 및 그 세부 정보를 목록으로 반환합니다.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/marketplace/info-from-land.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
