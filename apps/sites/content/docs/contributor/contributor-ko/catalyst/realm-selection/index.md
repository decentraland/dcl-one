# 렐름 선택

하나의 [카탈리스트](https://github.com/decentraland/catalyst) 은 Decentraland World에 필요한 다양한 서비스를 묶는 서버입니다. 서버의 주요 책임 중 일부는 클라이언트에 필요한 대부분의 콘텐츠를 위한 분산 저장소를 관리하고, 피어 간 통신을 조율하는 것입니다.

Decentraland 클라이언트에 들어가면, 백엔드 서비스인 Content, Lambdas, Comms를 지원하는 특정 카탈리스트에 연결해야 합니다. DAO 카탈리스트 중 하나를 사용해야 하며, 무작위로 선택할 수도 있지만, 그렇게 하면 사용자가 더 높은 지연 시간을 가진 백엔드 서비스나 사용자가 적은 realm에 연결되어 있는 동안 다른 곳에 접근하게 될 수 있습니다.

이를 완화하기 위해 realm 선택을 위한 알고리즘이 Kernel에 구현되어 있으며, 다음에서 설명합니다. [ADR-86](https://rfc.decentraland.org/adr/ADR-86).

이는 순서가 있는 목록에서 서로 다른 기준을 사용하는 구성 가능한 알고리즘입니다. 조건 중 하나가 결정을 내리기에 충분히 강하면 해당 realm을 선택하고, 그렇지 않으면 다음 기준에 결정을 위임합니다. 어느 조건도 충분히 강하지 않으면 기본 결정이 내려집니다(무작위).

기본 구성은 다음과 같습니다:

1. 가장 많은 사용자가 있는 카탈리스트 노드를 우선시함
2. 방문한 위치에 더 많은 사용자가 있는 카탈리스트 노드를 우선시함

사용할 수 있는 다른 기준은 다음과 같습니다:

* 높은 지연 시간: 지연 시간이 더 낮은 카탈리스트를 우선시함
* 로드 밸런싱: 모든 피어를 모든 카탈리스트에 분산시키는 라운드 로빈 메커니즘입니다.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/catalyst/realm-selection.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
