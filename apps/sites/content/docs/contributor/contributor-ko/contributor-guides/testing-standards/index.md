# 테스트 표준

이 섹션에서는 Decentraland 프로젝트 전반의 다양한 서비스와 UI에서 테스트가 수행되는 방식을 설명합니다.

이 문서에서 "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "MAY", 그리고 "OPTIONAL"이라는 키워드는 다음에 설명된 대로 해석해야 합니다 [RFC 2119](https://datatracker.ietf.org/doc/html/rfc2119).

## 개요

테스트는 우리의 개발 프로세스에서 중요한 부분입니다. 우리는 모든 프로젝트에서 테스트 범위와 품질에 대해 높은 기준을 유지합니다. 이 문서는 다음을 위한 포괄적인 지침을 제공합니다:

* [**테스트 작성**](/docs/contributor/contributor-ko/contributor-guides/testing-standards/writing-tests/) - 유지보수 가능하고 명확한 테스트를 작성하기 위한 일반 원칙과 패턴
* [**UI 테스트**](/docs/contributor/contributor-ko/contributor-guides/testing-standards/testing-uis/) - React 컴포넌트, Redux saga, reducer, selector 및 action creator 테스트
* [**테스트 서비스(Well-Known-Component)**](/docs/contributor/contributor-ko/contributor-guides/testing-standards/testing-services-wkc/) - well-known-component 아키텍처로 구축된 모듈식 서비스 테스트
* [**서비스 테스트(사용 중단됨)**](/docs/contributor/contributor-ko/contributor-guides/testing-standards/testing-services-deprecated/) - 서비스 테스트를 위한 레거시 방식(참고용)
* [**종단 간 테스트**](/docs/contributor/contributor-ko/contributor-guides/testing-standards/testing-e2e/) - Cypress를 사용한 E2E 테스트(진행 중)

## 테스트 철학

우리의 테스트 접근 방식은 다음을 중시합니다:

* **명확성** - 테스트는 자체적으로 문서화되어야 하며 이해하기 쉬워야 합니다
* **유지보수성** - 요구사항이 변경될 때 테스트를 쉽게 업데이트할 수 있어야 합니다
* **격리** - 각 테스트는 다른 테스트에 영향을 주지 않고 독립적으로 실행되어야 합니다
* **커버리지** - 모든 핵심 경로와 엣지 케이스를 테스트해야 합니다
* **값** - 테스트는 코드가 의도대로 작동한다는 확신을 제공해야 합니다

## 빠른 링크

* [다음을 사용하여 컨텍스트를 설명하는 방법 `describe`](/docs/contributor/contributor-ko/contributor-guides/testing-standards/writing-tests/#describing-and-building-context)
* [다음을 사용하여 기대값을 작성하는 방법 `it`](/docs/contributor/contributor-ko/contributor-guides/testing-standards/writing-tests/#describing-expectations-and-executing-code)
* [무엇을 테스트할지와 시기](/docs/contributor/contributor-ko/contributor-guides/testing-standards/writing-tests/#what-to-test)
* [언제, 어떻게 모킹할지](/docs/contributor/contributor-ko/contributor-guides/testing-standards/writing-tests/#when-to-mock)

## 관련 표준

* [의존성 관리](/docs/contributor/contributor-ko/contributor-guides/dependency-management/) - npm 의존성과 peerDependencies 관리
* [UI 표준](https://github.com/decentraland/docs/tree/main/contributor/contributor-guides/ui-standards/README.md) - Redux 및 RTK Query 패턴
* [API 문서](/docs/contributor/contributor-ko/contributor-guides/api-documentation/) - OpenAPI 문서화 표준


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/contributor-guides/testing-standards.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
