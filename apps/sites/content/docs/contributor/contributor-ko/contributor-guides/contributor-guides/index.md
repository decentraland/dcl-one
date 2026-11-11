# 개요

이 섹션에는 Decentraland 서비스에 대한 기여자 가이드와 모범 사례가 포함되어 있습니다. 이 가이드는 모든 프로젝트 전반에서 일관성, 품질, 유지보수성을 보장합니다.

## 섹션

* [API 문서](/docs/contributor/contributor-ko/contributor-guides/api-documentation/) - 서비스 API를 문서화하고 게시하기 위한 OpenAPI 표준
* [테스트 표준](/docs/contributor/contributor-ko/contributor-guides/testing-standards/) - 다양한 유형의 애플리케이션과 서비스에서 테스트를 작성하기 위한 포괄적인 지침
* [Well-Known 구성 요소](https://github.com/decentraland/docs/tree/main/contributor/contributor-guides/well-known-components/README.md) - WKC 아키텍처를 사용하여 모듈식이고 테스트 가능한 서비스를 구축하기 위한 표준
* [UI 표준](https://github.com/decentraland/docs/tree/main/contributor/contributor-guides/ui-standards/README.md) - 현대적인 React 애플리케이션을 구축하기 위한 Redux Toolkit 및 RTK Query 패턴
* [웹 UI 표준](https://github.com/decentraland/docs/tree/main/contributor/contributor-guides/web-ui-standards/README.md) - UI2, Material UI, 그리고 Decentraland 디자인 시스템을 사용하는 컴포넌트 라이브러리 표준

## 이 표준에 대하여

이 표준에서 키워드 "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "MAY", 그리고 "OPTIONAL"은 [RFC 2119](https://datatracker.ietf.org/doc/html/rfc2119).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/contributor-guides/contributor-guides.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
