> 수집형 진행, 리더보드, 사용자 지정 비트맵 텍스트가 있는 트리거 기반 몬스터 추격으로 이어지는 기억 맞추기.

# 몬스터 리콘

![Monster Recon 미리보기](/docs/_remote/276e8747b4e8b433f9cfa9f6.webp)

시간 제한이 있는 보드를 클리어한 뒤, 드러난 몬스터가 도망치기 전에 추적해 잡는 메모리 매치 게임입니다. Codex에서 희귀한 발견물을 수집하고 주간 및 역대 리더보드에 올라보세요.

* [Decentraland에서 플레이](https://decentraland.org/jump?realm=monsterrecon.dcl.eth)

## 프로젝트 정보

**관리자:** 커뮤니티 · carlosmu

[업스트림 소스 보기](https://github.com/carlosmu/dcl-monster-recon)

**라이선스:** 이 카탈로그에서는 식별되지 않았습니다. 재사용하기 전에 적용 가능한 조건은 업스트림 저장소와 관리자에게 문의하세요.


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
* 진행 상황, 최고 기록, 리더보드, 주간 보상을 위한 저장소.
* 가상 카메라
* 장면 이모트
* 트리거 영역
* 파티클
* 트윈
* TextShapes
* 3D 리더보드(플레이어 프로필 사진 포함)
* UI용 사용자 지정 비트맵 글꼴
* 다른 플레이어의 포획 결과가 동기화된 실시간 피드

## 코드 살펴보기

이 링크는 특정 소스 스냅샷을 가리키므로 업스트림 개발이 계속되어도 설명이 계속 유용합니다. 이는 학습용 참고 자료이며, 보안 평가가 아닙니다.

### 장면 마커와 트리거 영역으로 추격전을 구성하세요

`spawnHop` 선택한 마커에 흔들리는 트윈, 구형 트리거, 사운드, 빌보드 스프라이트를 결합합니다. 포획 핸들러는 진입한 엔티티가 로컬 플레이어인지 확인한 뒤 정리하고 보상 콜백을 호출합니다.

**프로젝트 맥락:** 다음에 제공된 마커 이름, 스프라이트 아틀라스, 콜백과 함께 이 문서를 읽으세요 `startPrizeChase`.

**소스 참조:**

* [`spawnHop`](https://github.com/carlosmu/dcl-monster-recon/blob/eb0bc6fa87b9ebf98c8c068ebb9bba686a9467cb/src/prizeChase.ts#L216-L271)
* [포획 핸들러](https://github.com/carlosmu/dcl-monster-recon/blob/eb0bc6fa87b9ebf98c8c068ebb9bba686a9467cb/src/prizeChase.ts#L183-L214)

### ReactEcs에서 사용자 지정 비트맵 텍스트 렌더링

`BitmapTextLine` 아틀라스 UV, 글리프 오프셋, 커닝을 사용해 개별 글리프 이미지를 배치합니다. 앞선 측정 및 줄바꿈 헬퍼는 측정된 너비를 기준으로 텍스트를 줄로 나눕니다.

**프로젝트 맥락:** 렌더러에는 일치하는 비트맵 글꼴 메트릭과 텍스처 에셋이 필요합니다.

**소스 참조:**

* [`BitmapTextLine`](https://github.com/carlosmu/dcl-monster-recon/blob/eb0bc6fa87b9ebf98c8c068ebb9bba686a9467cb/src/bitmapFont.tsx#L268-L327)
* [측정 및 줄바꿈 헬퍼](https://github.com/carlosmu/dcl-monster-recon/blob/eb0bc6fa87b9ebf98c8c068ebb9bba686a9467cb/src/bitmapFont.tsx#L229-L266)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/tutorials-and-examples/published-games/monster-recon.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
