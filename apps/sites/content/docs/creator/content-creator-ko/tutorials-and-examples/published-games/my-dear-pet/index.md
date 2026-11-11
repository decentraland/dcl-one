> 이동을 위한 가속과 제동이 있는 펫 수집, 그리고 에디터에서 배치한 알의 단계 오프셋 애니메이션.

# 마이 디어 펫

![My Dear Pet 미리보기](/docs/_remote/3412460113d21fec792e322a.webp)

펫 수집 게임.

* [Decentraland에서 플레이](https://decentraland.org/jump?realm=marshcolony.dcl.eth)

## 프로젝트 정보

**관리 주체:** Decentraland DAO

[소스 보기](https://github.com/dcl-regenesislabs/marsh-colony)

**라이선스:** 이 카탈로그에서 식별되지 않았습니다. 재사용하기 전에 적용 가능한 조건은 원본 저장소와 유지관리자에게 문의하세요.

## 코드 살펴보기

이 링크들은 특정 소스 스냅샷을 가리키므로 상위 개발이 계속되더라도 설명이 유용하게 유지됩니다. 이는 보안 평가가 아니라 학습용 참고자료입니다.

### 움직이는 펫에 가속과 감속 부여

`petMotionStep` 엔터티별 속도를 유지하고, 남은 정지 거리를 사용해 목표 속도를 제한하며, 프레임 크기의 이동 단계를 반환합니다. 호출자는 여전히 경로를 제공하고 변환을 업데이트합니다.

**프로젝트 맥락:** 도착 거리, 가속, 제동 값을 `shared/config` 와 함께 이 헬퍼를 살펴볼 때 확인하세요.

**소스 참고 자료:**

* [`petMotionStep`](https://github.com/dcl-regenesislabs/marsh-colony/blob/55dea4d3e13b3d821aa20bd277fbc4e0bc920b6f/src/client/petMotion.ts#L9-L35)

### 편집기에서 배치한 소품을 서로 다른 위상으로 애니메이션 처리

`setupEggShake` 이름이 지정된 달걀 엔터티를 찾아 원래 회전을 기억하고, 각 달걀마다 다른 위상의 작은 사인파 회전을 적용합니다. 이렇게 하면 각 소품의 제작된 방향을 유지하면서 변화를 더할 수 있습니다.

**프로젝트 맥락:** 이는 Creator Hub 씬에서 생성된 달걀 이름에 따라 달라집니다.

**소스 참고 자료:**

* [`setupEggShake`](https://github.com/dcl-regenesislabs/marsh-colony/blob/55dea4d3e13b3d821aa20bd277fbc4e0bc920b6f/src/client/eggShake.ts#L29-L53)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/tutorials-and-examples/published-games/my-dear-pet.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
