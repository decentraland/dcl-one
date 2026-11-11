> 시간 제한 라운드, 지속적인 리더보드, 풀링된 발사체, 근접도 기반 조명 예산이 있는 멀티플레이어 피하기 게임.

# 플래그 태그

![깃발 태그 미리보기](/docs/_remote/a8bf7285858279143edb58ed.webp)

24시간 연중무휴 멀티플레이어 서버 기반의 뺏기 게임입니다. 플레이어는 5분 라운드 동안 부메랑과 바나나 함정을 사용해 깃발을 지키며 경쟁하고, 일간·월간·전체 순위표가 지속적으로 결과를 추적합니다.

* [Decentraland에서 플레이](https://decentraland.org/jump?realm=flagtag.dcl.eth)

## 프로젝트 정보

**관리자:** 커뮤니티 · iillee

[업스트림 소스 보기](https://github.com/iillee/flagtag)

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
* 플레이어 진행도와 잠금 해제 항목 저장소
* 사용자 지정 애니메이션을 위한 아바타 마스킹

## 코드 살펴보기

이 링크는 특정 소스 스냅샷을 가리키므로 업스트림 개발이 계속되어도 설명이 계속 유용합니다. 이는 학습용 참고 자료이며, 보안 평가가 아닙니다.

### 모델 변형별로 투사체 비주얼 재사용

투사체 풀은 각 부메랑 색상마다 숨겨진 엔티티 10개를 미리 생성합니다. 획득 및 반환 헬퍼는 사용 가능한 엔티티를 찾아 소리를 멈춘 뒤 숨김 위치로 되돌립니다.

**프로젝트 맥락:** 비행 코드는 획득한 엔티티를 배치하고 작업이 끝나면 반환해야 하며, 풀은 프로젝트의 부메랑 에셋과 사운드 헬퍼에 의존합니다.

**소스 참조:**

* [투사체 풀](https://github.com/iillee/flagtag/blob/d5d9c8fbf0f69eadb4d79c3ae4054e8947f256b6/src/systems/projectile/pool.ts#L11-L37)
* [획득 및 반환 헬퍼](https://github.com/iillee/flagtag/blob/d5d9c8fbf0f69eadb4d79c3ae4054e8947f256b6/src/systems/projectile/pool.ts#L48-L66)

### 플레이어 주변의 활성 조명 수 제한

proximityLightSystem은 주기적으로 확인하고, 제곱거리 비교와 재사용 가능한 배열을 사용해 가장 가까운 조명을 선택하며, 활성 상태가 다른 조명만 변경합니다. 이 씬은 활성 조명 수를 8개로 제한하고 낮/밤 헬퍼를 통해 이를 제어합니다.

**프로젝트 맥락:** 월드 위치 변환과 기기별 강도 설정을 이해하려면 같은 파일의 설정과 상수를 읽어보세요.

**소스 참조:**

* [근접 조명 시스템](https://github.com/iillee/flagtag/blob/d5d9c8fbf0f69eadb4d79c3ae4054e8947f256b6/src/systems/proximityLights.ts#L109-L168)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/tutorials-and-examples/published-games/flag-tag.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
