> 픽업 및 드롭오프 구역, 아바타에 장착된 방향 포인터, 배달 애니메이션과 분리된 점수가 있는 시간 제한 소포 배달.

# 피죤 델럭스

![Pigeon Deluxe 미리보기](/docs/_remote/5140367a547cfe389db77f7e.webp)

플레이어가 소포를 모아 다른 패드로 옮기고, 배달할 때마다 추가 시간을 얻는 시간 제한 배달 게임입니다.

* [Decentraland에서 플레이](https://decentraland.org/jump?realm=pigeondeluxe.dcl.eth)

## 프로젝트 정보

**관리자:** 커뮤니티 · PepeGawd

[상위 소스 보기](https://github.com/PepeGawd/Pigeon-Deluxe)

**라이선스:** 이 카탈로그에서는 식별되지 않았습니다. 재사용하기 전에 적용 가능한 약관은 상위 저장소와 관리자에게 문의하세요.


> [!WARNING]
> **커뮤니티가 관리하는 코드 — 사용 전에 검토하세요**
> 
> 이 커뮤니티 프로젝트는 Decentraland Foundation 및 DAO GitHub 조직 외부에서 관리됩니다. 소스 코드와 실시간 배포는 독립적으로 변경될 수 있습니다.
> 
> 이 카탈로그에 포함되었다고 해서 보안 감사가 이루어졌거나 안전이 보장된다는 뜻은 아닙니다. 실행하거나 재사용하기 전에 코드, 종속성, 설치 스크립트를 검토하세요.
> 
> [카탈로그 문제 신고](https://github.com/decentraland/docs/issues/new). 의심되는 취약점의 경우 상위 프로젝트의 보안 정책을 따르세요. 공개 이슈에는 익스플로잇 세부 정보나 인증 정보를 포함하지 마세요.


## 코드 살펴보기

이 링크들은 특정 소스 스냅샷을 가리키므로, 상위 개발이 계속되더라도 설명이 유용하게 유지됩니다. 이는 학습용 참고 자료이지, 보안 평가는 아닙니다.

### 아바타에 부착된 상태로 목표를 향하게 함

`initParcelDirectionPointer` 및 해당 업데이트 시스템은 아바타 이름표 위에 화살표를 부착하고, 현재 목표까지의 월드 공간 방향을 계산한 뒤, 그 회전을 부모의 로컬 공간으로 변환합니다. 회전은 `dt`를 사용해 진행되므로, 화살표는 시간이 지남에 따라 목표를 향해 회전합니다.

**프로젝트 맥락:** 목표는 소포 시스템에서 가져오며, 화살표는 씬의 `FaceDirection1.glb` 에셋을 사용합니다.

**소스 참조:**

* [`initParcelDirectionPointer` 및 해당 업데이트 시스템](https://github.com/PepeGawd/Pigeon-Deluxe/blob/1c24bcb3c82d73dbb1aad8b1035f3f7edab5d315/src/Game/parcelDirectionPointer.ts#L26-L96)

### 점수 계산을 배달 애니메이션과 분리

`startNewRound` 및 `handleSpawnAreaEnter` 는 패드에 수거 및 배달 역할을 할당하고, 소포를 운반 중인지 추적하며, 배달 패드에 들어가자마자 점수와 시간을 부여합니다. 다음 라운드는 내려놓기 애니메이션이 끝나면 시작됩니다.

**프로젝트 맥락:** 게임플레이 상태와 시각적 피드백이 어떻게 맞물리는지 보려면 이것을 씬의 게임 타이머 및 소포 부착 헬퍼와 함께 읽어보세요.

**소스 참조:**

* [`startNewRound` 및 `handleSpawnAreaEnter`](https://github.com/PepeGawd/Pigeon-Deluxe/blob/1c24bcb3c82d73dbb1aad8b1035f3f7edab5d315/src/Game/parcelCollectDeliver.ts#L363-L433)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/tutorials-and-examples/published-games/pigeon-deluxe.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
