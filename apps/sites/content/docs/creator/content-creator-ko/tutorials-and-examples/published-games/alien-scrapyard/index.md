> 데이터로 정의된 건설 퍼즐과 단계 기반 시네마틱 카메라가 있는 협동 건설 라운드.

# 외계인 고철 처리장

![외계인 고철장 미리보기](/docs/_remote/c94532935e9e0629a121ed5f.webp)

플레이어들이 함께 외계 기술을 조립하고 새로운 점수와 도형을 해금하는 협동식 시간 제한 건설 게임입니다.

* [Decentraland에서 플레이](https://decentraland.org/jump?realm=alienscrapyard.dcl.eth)

## 프로젝트 정보

**관리자:** 커뮤니티 · procesista

[업스트림 소스 보기](https://github.com/procesista/Alienscrap)

**라이선스:** 이 카탈로그에서는 식별되지 않았습니다. 재사용하기 전에 적용 가능한 조건은 업스트림 저장소와 관리자에게 문의하세요.


> [!WARNING]
> **커뮤니티가 관리하는 코드 — 사용 전에 검토하세요**
> 
> 이 커뮤니티 프로젝트는 Decentraland Foundation과 DAO GitHub 조직 외부에서 관리됩니다. 소스 코드와 라이브 배포는 독립적으로 변경될 수 있습니다.
> 
> 이 카탈로그에 포함되었다고 해서 보안 감사가 이루어졌거나 안전이 보장된다는 뜻은 아닙니다. 실행하거나 재사용하기 전에 코드, 종속성, 설치 스크립트를 검토하세요.
> 
> [카탈로그 관련 문제 신고](https://github.com/decentraland/docs/issues/new). 의심되는 취약점은 업스트림 프로젝트의 보안 정책을 따르세요. 공개 이슈에 익스플로잇 세부 정보나 자격 증명을 포함하지 마세요.


## 코드 살펴보기

이 링크는 특정 소스 스냅샷을 가리키므로 업스트림 개발이 계속되어도 설명이 계속 유용합니다. 이는 학습용 참고 자료이며, 보안 평가가 아닙니다.

### 건설 퍼즐을 데이터로 표현하기

슬롯 정의는 각 조각에 필요한 부품, 위치, 크기, 레이블을 설명합니다. 템플릿 도우미는 레이아웃을 중앙으로 재배치하고 크기를 조정하며, 퍼즐 시퀀스를 노출하고 개별 슬롯을 조회합니다. 이는 데이터를 통해 새 건설물을 추가하는 방법을 연구하는 데 유용합니다.

**프로젝트 맥락:** 공유되는 장면 중심 및 부품 유형 상수와 함께 읽으세요.

**소스 참조:**

* [슬롯 정의](https://github.com/procesista/Alienscrap/blob/be0b1b93580452f9cc778dc36554fe9354ee0ea2/src/shared/templates.ts#L3-L39)
* [템플릿 도우미](https://github.com/procesista/Alienscrap/blob/be0b1b93580452f9cc778dc36554fe9354ee0ea2/src/shared/templates.ts#L140-L184)

### 시네마틱 카메라를 라운드 상태에 연결하기

시네마틱 시스템은 카운트다운 및 퍼포먼스 단계에서 샷을 활성화하고, 라운드 도중 합류한 플레이어를 처리하며, 스냅샷이 오래되면 카메라를 해제합니다. 최종 업데이트는 카메라를 해당 샷을 따라 이동시키기 전에 라운드 변경과 경과 시간 종료 조건을 추가합니다.

**프로젝트 맥락:** 이 예제는 클라이언트 스냅샷, 공유 타이밍 상수, 그리고 HUD 표시 도우미에 의존합니다.

**소스 참조:**

* [시네마틱 시스템](https://github.com/procesista/Alienscrap/blob/be0b1b93580452f9cc778dc36554fe9354ee0ea2/src/client/cinematic.ts#L93-L129)
* [최종 업데이트](https://github.com/procesista/Alienscrap/blob/be0b1b93580452f9cc778dc36554fe9354ee0ea2/src/client/cinematic.ts#L158-L176)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/tutorials-and-examples/published-games/alien-scrapyard.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
