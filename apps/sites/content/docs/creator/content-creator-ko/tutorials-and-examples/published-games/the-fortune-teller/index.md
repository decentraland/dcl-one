> 명시적인 대화 상태 전환과 피벗 기반 시네마틱 카메라 리그가 있는 소셜 점술.

# 점술가

![운세 점쟁이 미리보기](/docs/_remote/2aaa323560ef094922b15d6a.webp)

플레이어가 친구에게 운세를 받거나 운세를 봐줄 수 있는 소셜 롤플레잉 경험입니다.

* [Decentraland에서 플레이](https://decentraland.org/jump?realm=fortunes.dcl.eth)

## 프로젝트 정보

**관리자:** 커뮤니티 · carlosmu

[업스트림 소스 보기](https://github.com/carlosmu/circus_fortune_teller)

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

### 리딩을 명시적인 상태 전이로 표현하기

운세 상태 머신은 카테고리와 덱 선택부터 공개까지 허용되는 단계를 나열합니다. `tryTransition` 다음 단계를 확인하고, 세션을 업데이트하며, 상태 진입 리스너에 알립니다.

**프로젝트 맥락:** 세션 타입과 액션 핸들러와 함께 읽어 보면 다단계 상호작용이 어떻게 구성되는지 알 수 있습니다.

**소스 참조:**

* [운세 상태 머신](https://github.com/carlosmu/circus_fortune_teller/blob/f8db0c5630a03208d328a96737224e1293df60c7/src/fortuneFsm/machine.ts#L6-L60)

### 씬 오브젝트 주위에 카메라 리그를 구축하기

`ensureHostPivotRig` 및 `startHostCinematicCamera` 테이블 아래의 피벗에 가상 카메라를 부모로 설정하고, 플레이어 쪽에서 시작 각도와 종료 각도를 선택한 다음, 카메라를 활성화합니다.

**프로젝트 맥락:** 이 리그는 이 모듈의 테이블 엔티티, 구성 값, 업데이트 시스템에 의존합니다. 이 요소들은 씬 컨텍스트에서 상호작용을 어떻게 구성하는지 설명해 줍니다.

**소스 참조:**

* [`ensureHostPivotRig` 및 `startHostCinematicCamera`](https://github.com/carlosmu/circus_fortune_teller/blob/f8db0c5630a03208d328a96737224e1293df60c7/src/cinematicCamera.ts#L141-L197)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/tutorials-and-examples/published-games/the-fortune-teller.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
