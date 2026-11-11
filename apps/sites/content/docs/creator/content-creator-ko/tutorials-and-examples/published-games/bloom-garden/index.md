> 지속적인 식물 상태, 서버 시계 오프셋 추정, 풀링된 꽃잎 애니메이션이 있는 협동 정원.

# 블룸 가든

![Bloom Garden 미리보기](/docs/_remote/3a9e2127ba7a6f3cdcd435b3.webp)

플레이어들이 함께 살아 있는 정원을 가꾸고 Big Bloom을 깨우기 위해 협력하는 협동 정원 체험입니다.

멀티플레이어 서버는 식물 상태를 위한 내장 스토리지, 주간 리더보드, 그리고 플레이어별 물주기 제한을 사용합니다.

* [Decentraland에서 플레이](https://decentraland.org/jump?realm=bloomgarden.dcl.eth)

## 프로젝트 정보

**관리자:** 커뮤니티 · TheLivingGarden

[업스트림 소스 보기](https://github.com/TheLivingGarden/TheLivingGarden/tree/main-main)

**라이선스:** [MIT](https://github.com/TheLivingGarden/TheLivingGarden/blob/main/LICENSE). 재사용하기 전에 상위 저장소의 약관을 확인하세요. 코드와 자산에는 서로 다른 약관이 적용될 수 있습니다.


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

### 서버와 클라이언트 시계 간의 오프셋을 추정합니다

시계 도우미는 오프셋 샘플 다섯 개를 유지하고, 큰 편차를 거부하며, 서버 타임스탬프를 로컬 시간으로 변환합니다. 물주기 클라이언트는 씬의 메시지 정의를 통해 수신한 타임스탬프를 이를 통해 전달합니다.

**프로젝트 맥락:** 이는 경량화된 시간 추정 방식을 보여 줍니다. 네트워크 전송 시간은 샘플에 포함됩니다.

**소스 참조:**

* [시계 도우미](https://github.com/TheLivingGarden/TheLivingGarden/blob/b2c9a90791b1eed5c92a4ffadf6ac480d91510ea/src/shared/clockSync.ts#L3-L55)
* [물주기 클라이언트](https://github.com/TheLivingGarden/TheLivingGarden/blob/b2c9a90791b1eed5c92a4ffadf6ac480d91510ea/src/wateringSystem.ts#L1144-L1146)

### 임시 효과가 자연스럽게 끝나도록 합니다

꽃잎 설정과 컨트롤은 씬의 이름이 지정된 Petal 모델에서 재사용 가능한 풀을 만든 다음, 효과를 시작하거나 멈춥니다. 업데이트 단계는 꽃잎을 확대하고, 떨어뜨려 쉬게 한 뒤, 재활용하기 전에 다시 줄입니다. 기존 객체가 애니메이션을 완료하는 동안 효과가 더 이상 새 객체를 생성하지 않아야 할 때 이를 참고하세요.

**프로젝트 맥락:** 배치는 BLOOM\_CENTER와 모듈의 타이밍 상수에 따라 달라집니다.

**소스 참조:**

* [꽃잎 설정과 컨트롤](https://github.com/TheLivingGarden/TheLivingGarden/blob/b2c9a90791b1eed5c92a4ffadf6ac480d91510ea/src/petalSystem.ts#L119-L173)
* [업데이트 단계](https://github.com/TheLivingGarden/TheLivingGarden/blob/b2c9a90791b1eed5c92a4ffadf6ac480d91510ea/src/petalSystem.ts#L207-L310)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/tutorials-and-examples/published-games/bloom-garden.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
