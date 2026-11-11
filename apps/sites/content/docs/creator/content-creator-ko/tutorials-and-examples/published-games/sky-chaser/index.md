> 지속적인 리더보드, 풀링된 발사체, 라운드 상태에서 시드된 픽업 배치가 있는 멀티플레이어 패키지 잡기 라운드.

# 스카이 체이서

![Sky Chaser 미리보기](/docs/_remote/3033913f0028e180a375c5bf.webp)

플레이어들이 각 라운드에서 탈출하는 패키지를 잡기 위해 경쟁하는 멀티플레이어 게임입니다. 장면에는 주간 및 전체 기간 리더보드가 포함됩니다.

* [Decentraland에서 플레이](https://decentraland.org/jump?realm=skychaser.dcl.eth)

## 프로젝트 정보

**관리자:** 커뮤니티 · stom66

[업스트림 소스 보기](https://github.com/stom66/dcl-sky-chaser)

**라이선스:** [CC BY-NC-ND 4.0](https://github.com/stom66/dcl-sky-chaser/blob/main/LICENSE.md). 재사용하기 전에 상위 저장소의 약관을 확인하세요. 코드와 자산에는 서로 다른 약관이 적용될 수 있습니다.


> [!WARNING]
> **커뮤니티가 관리하는 코드 — 사용 전에 검토하세요**
> 
> 이 커뮤니티 프로젝트는 Decentraland Foundation과 DAO GitHub 조직 외부에서 관리됩니다. 소스 코드와 라이브 배포는 독립적으로 변경될 수 있습니다.
> 
> 이 카탈로그에 포함되었다고 해서 보안 감사가 이루어졌거나 안전이 보장된다는 뜻은 아닙니다. 실행하거나 재사용하기 전에 코드, 종속성, 설치 스크립트를 검토하세요.
> 
> [카탈로그 관련 문제 신고](https://github.com/decentraland/docs/issues/new). 의심되는 취약점은 업스트림 프로젝트의 보안 정책을 따르세요. 공개 이슈에 익스플로잇 세부 정보나 자격 증명을 포함하지 마세요.


## 주요 기능

* 멀티플레이어 서버
* 동기화된 Mulberry32 RNG를 통한 동기화된 게임플레이
* 리더보드용 Storage API
* 부스터/트램폴린/폭발을 위한 플레이어 이동 조정 및 물리 힘
* 파티클 시스템(부스트 궤적, 폭발, 획득 아이템)

## 코드 살펴보기

이 링크는 특정 소스 스냅샷을 가리키므로 업스트림 개발이 계속되어도 설명이 계속 유용합니다. 이는 학습용 참고 자료이며, 보안 평가가 아닙니다.

### 플레이 중 투사체 엔티티 재사용

`fireProjectile` 및 `getIdleProjectile` 비활성 투사체를 재사용하고, 풀을 모두 사용했을 때는 새로 생성하며, 활성 투사체만 업데이트합니다.

**프로젝트 맥락:** 프로젝트의 `투사체` 이동, 충돌 반응, 라운드 종료 정리를 위한 클래스 및 이벤트 핸들러.

**소스 참조:**

* [`fireProjectile` 및 `getIdleProjectile`](https://github.com/stom66/dcl-sky-chaser/blob/e546854d2a2634ca5e28635c8b9be124a173f6dc/dcl/src/client/projectileManager.ts#L76-L124)

### 라운드 상태에서 픽업 배치 시드 설정

`onGameStart` 및 `getRandomPosition` 라운드 시작 시간을 사용해 프로젝트의 난수 생성기를 초기화한 다음, 픽업 거리와 높이를 샘플링합니다.

**프로젝트 맥락:** 픽업 설정과 함께 컴포넌트 저장소를 읽으세요: 모바일 제한과 이후 픽업 활동은 난수 호출 순서에 영향을 주므로, 공유 시드만으로는 모든 클라이언트에서 동일한 레이아웃이 보장되지 않습니다.

**소스 참조:**

* [`onGameStart` 및 `getRandomPosition`](https://github.com/stom66/dcl-sky-chaser/blob/e546854d2a2634ca5e28635c8b9be124a173f6dc/dcl/src/client/spawners/pickupSpawner.ts#L115-L164)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/tutorials-and-examples/published-games/sky-chaser.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
