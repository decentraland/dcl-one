> 서버에서 시뮬레이션하는 물리, 지속적인 진행, 배치된 상태 업데이트, 모바일용으로 스케일된 파티클 이펙트가 있는 멀티플레이어 볼링.

# 패스트레인 볼링

![Fastlane Bowling 미리보기](/docs/_remote/926145e7ae23468dd849d344.webp)

Cannon-ES가 제공하는 사실적인 물리 효과를 갖춘 멀티플레이어 볼링 게임입니다. 여러 게임이 공유 공간의 각기 다른 레인에서 실행됩니다.

* [Decentraland에서 플레이](https://decentraland.org/jump?realm=stom.dcl.eth)

## 프로젝트 정보

**관리자:** 커뮤니티 · stom66

[업스트림 소스 보기](https://github.com/stom66/dcl-bowling)

**라이선스:** [CC BY-NC-ND 4.0](https://github.com/stom66/dcl-bowling/blob/main/LICENSE.md). 재사용하기 전에 상위 저장소의 약관을 확인하세요. 코드와 자산에는 서로 다른 약관이 적용될 수 있습니다.


> [!WARNING]
> **커뮤니티가 관리하는 코드 — 사용 전에 검토하세요**
> 
> 이 커뮤니티 프로젝트는 Decentraland Foundation과 DAO GitHub 조직 외부에서 관리됩니다. 소스 코드와 라이브 배포는 독립적으로 변경될 수 있습니다.
> 
> 이 카탈로그에 포함되었다고 해서 보안 감사가 이루어졌거나 안전이 보장된다는 뜻은 아닙니다. 실행하거나 재사용하기 전에 코드, 종속성, 설치 스크립트를 검토하세요.
> 
> [카탈로그 관련 문제 신고](https://github.com/decentraland/docs/issues/new). 의심되는 취약점은 업스트림 프로젝트의 보안 정책을 따르세요. 공개 이슈에 익스플로잇 세부 정보나 자격 증명을 포함하지 마세요.


## 주요 기능

* [**Cannon-ES**](https://github.com/pmndrs/cannon-es) **물리** 클라이언트가 압축된 키프레임을 재생하는 멀티플레이어 서버에서.
* **저장소** 플레이어 진행도, 통계, 환경설정, 인벤토리, 주간/전체 순위표를 위해.
* 동적 **조명** 회전하는 그림자 마스크와 공을 따라가는 스포트라이트가 있는.
* **파티클** 궤적 및 해제 효과로, 모바일에서는 파티클 수를 줄였습니다.
* **가상 카메라** 도착 플라이인, 조준, 굴림 재생을 위해.
* **`GltfNodeModifiers`** 스크롤되는 네온 텍스처 UV를 위해.
* SDK 제어 **glTF 애니메이션** 레인 범퍼, 카운트다운, 점수 효과를 위해.

## 코드 살펴보기

이 링크는 특정 소스 스냅샷을 가리키므로 업스트림 개발이 계속되어도 설명이 계속 유용합니다. 이는 학습용 참고 자료이며, 보안 평가가 아닙니다.

### 동기화된 상태 변경을 클라이언트 이벤트로 일괄 처리

에서 `LaneWatcher.onLaneChanged`에서는 set이 변경된 레인을 수집하고 한 번의 flush를 예약합니다. `flush` 각 레인의 스냅샷을 읽고 상태 및 단계 전환 이벤트를 발생시킵니다. 이 흐름을 따라 장면이 동기화된 구성 요소를 클라이언트 반응과 어떻게 연결하는지 이해하세요.

**프로젝트 맥락:** 프로젝트의 `LaneStore` 및 이벤트 버스 정의와 함께 읽어보세요.

**소스 참조:**

* [`LaneWatcher.onLaneChanged`](https://github.com/stom66/dcl-bowling/blob/25c5a7f9152835c190d9d20fa18b6010d0c6ce50/dcl/src/client/laneWatcher.ts#L79-L90)
* [`flush`](https://github.com/stom66/dcl-bowling/blob/25c5a7f9152835c190d9d20fa18b6010d0c6ce50/dcl/src/client/laneWatcher.ts#L132-L153)

### 모바일용 파티클 효과 크기 조정

`getParticleRatio` 및 `scaleConfig` 원래 구성을 유지하면서 모바일 비율을 사용해 방출 속도, 파티클 한도, 버스트 횟수를 조정합니다. `spawn` 선택적 부모 지정과 시간 지정 정리 기능을 포함해 해당 구성을 새 이미터에 적용합니다.

**프로젝트 맥락:** 이는 하나의 효과 구성이 서로 다른 기기에서 어떻게 활용될 수 있는지 살펴볼 때 유용합니다.

**소스 참조:**

* [`getParticleRatio` 및 `scaleConfig`](https://github.com/stom66/dcl-bowling/blob/25c5a7f9152835c190d9d20fa18b6010d0c6ce50/dcl/src/client/particleSpawner.ts#L176-L218)
* [`spawn`](https://github.com/stom66/dcl-bowling/blob/25c5a7f9152835c190d9d20fa18b6010d0c6ce50/dcl/src/client/particleSpawner.ts#L91-L121)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/tutorials-and-examples/published-games/fastlane-bowling.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
