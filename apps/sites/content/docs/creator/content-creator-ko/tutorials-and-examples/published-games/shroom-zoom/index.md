> 트리거로 활성화되는 바운스 레이캐스트, 플레이어 임펄스, 소유자를 따라가거나 공전하는 수집형 스프라이트가 있는 플랫폼 게임.

# 슘 줌

![Shroom Zoom 미리보기](/docs/_remote/abaf092d80ad05ebe9c8fc1b.webp)

더블 점프, 글라이딩, 수집 요소, 탐험이 있는 플랫폼 및 타임 트라이얼 게임.

* [Decentraland에서 플레이하기](https://decentraland.org/jump?realm=shroomzoom.dcl.eth)

## 프로젝트 정보

**관리자:** 커뮤니티 · CanessaDCL

[상위 소스 보기](https://github.com/CanessaDCL/dcl-shroomzoom)

**라이선스:** 이 카탈로그에서는 식별되지 않았습니다. 재사용하기 전에 적용 가능한 조건은 상위 저장소와 관리자를 확인하세요.


> [!WARNING]
> **커뮤니티가 유지 관리하는 코드 — 사용 전 검토하세요**
> 
> 이 커뮤니티 프로젝트는 Decentraland Foundation 및 DAO GitHub 조직 외부에서 유지 관리됩니다. 소스 코드와 라이브 배포는 독립적으로 변경될 수 있습니다.
> 
> 이 카탈로그에 포함되었다고 해서 보안 감사를 받았거나 안전이 보장된다는 뜻은 아닙니다. 실행하거나 재사용하기 전에 코드, 종속성, 설치 스크립트를 검토하세요.
> 
> [카탈로그 관련 문제 신고](https://github.com/decentraland/docs/issues/new). 의심되는 취약점은 상위 프로젝트의 보안 정책을 따르세요. 공개 이슈에는 익스플로잇 세부 정보나 자격 증명을 포함하지 마세요.


## 주요 기능

* 멀티플레이어 서버
* 탄성 있는 버섯을 위한 물리 임펄스

## 코드 살펴보기

이 링크는 특정 소스 스냅샷을 가리키므로 상위 프로젝트의 개발이 계속되어도 설명이 유용하게 유지됩니다. 이는 학습용 참고 자료이며 보안 평가는 아닙니다.

### 가까운 트리거와 정밀한 접촉 검사를 결합하기

BounceTrigger 콜백은 플레이어가 트리거 안에 있을 때 레이캐스팅을 활성화하고, 나갈 때 해당 시스템을 제거합니다. 레이캐스트와 바운스 로직은 아래쪽 히트를 구성된 버섯 엔티티와 비교해 확인하고, 표면 근처에서 플레이어 임펄스를 적용합니다.

**프로젝트 맥락:** 장면의 트리거 크기와 임펄스 설정은 BounceZoom 또는 BounceRegular과 함께 읽어보세요.

**소스 참조:**

* [BounceTrigger 콜백](https://github.com/CanessaDCL/dcl-shroomzoom/blob/042968e3c13b5d1093d2922d5713570ad4cb0284/src/bounce/class.bounce.ts#L75-L101)
* [레이캐스트 및 바운스 로직](https://github.com/CanessaDCL/dcl-shroomzoom/blob/042968e3c13b5d1093d2922d5713570ad4cb0284/src/bounce/class.bounce.ts#L103-L164)

### 수집한 아이템을 애니메이션 수행원으로 바꾸기

Follower target 헬퍼는 플레이어 주변의 오프셋과 궤도 주변의 위치를 계산합니다. followSpritesSystem은 각 스프라이트를 목표 지점으로 보간하고, 수집이 완료되면 궤도 회전으로 전환하며 스프라이트를 소유자를 향해 회전시킵니다.

**프로젝트 맥락:** 장면의 수집 세트, 추종자 엔티티, 플레이어별 궤도 타임스탬프가 이 시각적 동작의 상태를 제공합니다.

**소스 참조:**

* [Follower target 헬퍼](https://github.com/CanessaDCL/dcl-shroomzoom/blob/042968e3c13b5d1093d2922d5713570ad4cb0284/src/spriteFollowers.ts#L400-L437)
* [followSpritesSystem](https://github.com/CanessaDCL/dcl-shroomzoom/blob/042968e3c13b5d1093d2922d5713570ad4cb0284/src/spriteFollowers.ts#L678-L706)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/tutorials-and-examples/published-games/shroom-zoom.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
