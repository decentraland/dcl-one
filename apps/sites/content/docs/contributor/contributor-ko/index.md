# 기여자 환영

디센트럴랜드는 사용자들이 소유하는 가상 세계로, 개방적으로 개발되며 전 세계 사람들의 기여로 함께 성장할 준비가 되어 있습니다.

이 문서는 아키텍처 개요부터 각 구성 요소의 구체적인 기능까지, 디센트럴랜드 오픈 프로토콜에 대한 모든 것을 자세히 설명합니다. 여기에서 다음에 필요한 자료를 찾을 수 있습니다:

1. 생태계 내 제품과 애플리케이션을 설계하기
2. 조사를 수행하고 세계의 콘텐츠를 조사하기
3. 자신만의 월드 익스플로러를 만들기
4. 이 목록에 포함될 수도 있는 새로운 아이디어를 추구하기

**함께 메타버스를 만들어 갑시다!**

## 여기에서 찾을 수 있는 것

이 문서는 디센트럴랜드 프로젝트를 이해하고 기여하는 데 도움이 되도록 설계되었습니다. 처음 기여하는 분이든 경험 많은 개발자든, 다음 내용을 찾을 수 있습니다:

* **아키텍처 개요** - 디센트럴랜드 프로토콜 전체를 이해하기
* **코드 표준** - 우리가 깔끔하고 유지보수하기 쉬운 코드를 작성하는 방법을 배우기
* **테스트 관행** - 우리의 포괄적인 테스트 전략을 이해하기
* **문서 작성 지침** - 우리가 작업 내용을 어떻게 문서화하는지 알아보기
* **API 참조** - 모든 서비스에 대한 완전한 문서

## 어디서 시작할까?

어디서 시작해야 할지 모르겠다면, 먼저 [아키텍처](/docs/contributor/contributor-ko/architecture/architecture/) 섹션을 살펴보며 더 자세한 섹션으로 들어가기 전에 다양한 구성 요소와 그 일반적인 목적을 알아보는 것이 좋습니다.

기여자라면 먼저 [기여자 가이드](/docs/contributor/contributor-ko/contributor-guides/contributor-guides/) 섹션을 살펴보세요. 이 섹션에는 테스트 표준부터 코드 품질 지침까지 모든 내용이 담겨 있습니다.

### 빠른 링크

<table data-view="cards"><thead><tr><th></th><th></th><th></th><th data-hidden data-card-target data-type="content-ref"></th></tr></thead><tbody><tr><td><strong>아키텍처</strong></td><td>시스템 아키텍처 개요</td><td></td><td><a href="/docs/contributor/contributor-ko/architecture/architecture/">개요</a></td></tr><tr><td><strong>기여자 가이드</strong></td><td>지침 및 모범 사례</td><td></td><td><a href="/docs/contributor/contributor-ko/contributor-guides/contributor-guides/">개요</a></td></tr><tr><td><strong>API 참조</strong></td><td>서비스용 API 문서</td><td></td><td><a href="https://github.com/decentraland/docs/tree/main/apis/README.md">https://github.com/decentraland/docs/tree/main/apis/README.md</a></td></tr></tbody></table>

## 추가 자료

추가 정보를 얻고 계속 학습하려면 다음을 확인해 보세요:

* 이 [Ask Me Anything 세션](https://www.notion.so/decentraland/Decentraland-Foundation-Technical-AMA-230fcf1ca1ec4d49922744fe91e6995d?d=8508f4ca20eb4623b578feb235f86cef#bff4e53f1c664d66802e4770f46b959c), 여기서 커뮤니티 구성원들은 디센트럴랜드 재단의 기술 팀에 질문, 의견 또는 우려 사항을 제시할 수 있습니다. 매월 개최되며 누구나 자유롭게 참여할 수 있습니다!
* 이 [상태 페이지](https://status.decentraland.org/), 현재 플랫폼 상태, 장애 이력 및 일부 지표를 확인할 수 있습니다.
* 이 [아키텍처 결정 기록](https://adr.decentraland.org/), 디센트럴랜드의 다양한 측면의 상태와 진화를 설명하는 설계 문서 모음입니다.
* 이 [GitHub의 디센트럴랜드 조직](https://github.com/decentraland), 모든 오픈소스 저장소가 호스팅되는 곳입니다.

## 기여하기

이러한 기준을 따름으로써 디센트럴랜드의 모든 프로젝트 전반에서 코드 품질과 일관성을 유지하는 데 도움이 됩니다. 이 지침은 우리의 발전하는 모범 사례와 개발 경험에서 얻은 교훈을 반영하도록 지속적으로 업데이트됩니다.

이러한 기준에 대한 질문이나 제안이 있다면 디센트럴랜드 개발 커뮤니티에 문의해 주세요.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/readme.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
