> 서버가 소유한 게임 상태, 지속적인 진행, 포탑 카메라, 화면 가장자리 목표 마커가 있는 협동 우주선 전투.

# 아스트로 크루

![Astro Crew 미리보기](/docs/_remote/31ce14ffd2ff2d42c05fb6b7.webp)

승무원이 한 척의 함선을 함께 사용하는 협동 장면입니다. 각 정거장에서 다가오는 소행성과 비행접시를 쏘고, 선체 파손 부위를 수리하며, 사수와 공학 레벨을 올립니다. 전투는 권한 있는 멀티플레이어 서버에서 실행됩니다.

* [Decentraland에서 플레이](https://decentraland.org/play/?realm=astrocrew.dcl.eth)

## 프로젝트 정보

**관리자:** 커뮤니티 · DansStuff

[업스트림 소스 보기](https://github.com/DansStuff/AstroCrew)

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

* **권한 있는 멀티플레이어** 전투 전체 동안입니다. 클라이언트는 조준, 수리, 과충전을 요청합니다. 서버가 선체, 파손, 스폰을 소유하며, 그리고 동기화되며 `GameState` 및 `플레이어 통계` 멀티플레이어 서버의 쓰기만 허용합니다.
* **저장소** 각 플레이어의 사수 및 공학 레벨과 주간 미션 리더보드.
* **가상 카메라** 세 개의 포탑에 걸쳐, 시간에 맞춘 전환으로 섞입니다. 충격이 그 카메라들을 흔듭니다. 총을 집으면 아바타가 다음과 함께 멈춥니다 **입력 수정자** 그리고 모바일 조이스틱과 조준선을 다음으로 숨깁니다 **터치스크린 컨트롤**.
* **기본 포인터 정보** 포인터의 월드 레이에서 각 발사를 조준합니다. **포인터 잠금** 해제된 상태로 유지되어 **포인터 이벤트** 콘솔, 길게 눌러 수리하는 파손 부위, 그리고 미션 테이블을 계속 조작할 수 있습니다.
* **GltfNodeModifiers** 함선의 발광 메시를 다시 칠합니다: 항해 중에는 초록색, 교전 중에는 빨간색, 그리고 무기가 과충전되면 두 번째 세트로 바뀝니다.
* **ReactEcs** 선체, 스킬 레벨, 그리고 라운드 종료 보드를 위한 HUD입니다. 화면 밖 파손 아이콘은 화면 가장자리에 있으며, 다음을 사용해 배치됩니다 **UiCanvasInformation**.
* **빌보드**, **TextShape**, 그리고 **아바타 텍스처** 각 목표물마다: 카메라를 향하는 조준점, "Target Locked" 라벨, 그리고 그것을 쏘는 사람의 초상화가 표시됩니다.
* **애니메이터** 비행접시 회전 클립을 반복 재생합니다. **AudioSource** 스팅어와 루프를 재생하며, 레이저 원샷 풀을 사용해 마지막 클립이 아직 재생 중이어도 발사를 다시 시작할 수 있습니다. 클립은 다음으로 미리 로드됩니다 **AssetLoad**.
* 게임 내 **TextShape** 주간 보드의 행들입니다. 행을 클릭하면 해당 미션의 결과가 열립니다.
* **AvatarModifierArea** 장면 전체에서 여권 프롬프트를 끕니다.

## 코드 살펴보기

이 링크는 특정 소스 스냅샷을 가리키므로 업스트림 개발이 계속되어도 설명이 계속 유용합니다. 이는 학습용 참고 자료이며, 보안 평가가 아닙니다.

### 화면 밖 목표를 향하도록 플레이어를 안내합니다

edgePositionForBreach는 파손 방향을 카메라의 수평 축에 투영하고, 사각형의 가장자리를 찾은 다음, 캔버스 종횡비를 사용해 아이콘을 화면 안으로 제한합니다. BreachIcon은 그 위치를 맥박치는 ReactEcs 마커와 연결합니다.

**프로젝트 맥락:** 장면의 파손 위치, 활성 파손 상태, UI 상수와 함께 읽으세요.

**소스 참조:**

* [edgePositionForBreach](https://github.com/DansStuff/AstroCrew/blob/dbe11b05be2a753b27b6d5dd866d0554ff73344f/src/ui/breachIcons.tsx#L27-L76)
* [BreachIcon](https://github.com/DansStuff/AstroCrew/blob/dbe11b05be2a753b27b6d5dd866d0554ff73344f/src/ui/breachIcons.tsx#L78-L107)

### 원래 자세로 돌아오는 카메라 흔들림을 추가합니다

shakeEntity는 기본 변환을 기록하고, 강도와 남은 지속 시간을 사용해 겹치는 흔들림을 합칩니다. CameraShakeSystem은 점점 줄어드는 위치 및 회전 오프셋을 적용한 뒤, 기본 자세를 복원하고 컴포넌트를 제거합니다.

**프로젝트 맥락:** 이 효과는 프로젝트의 진폭 상수를 사용하며 ECS 시스템으로 등록되어야 합니다.

**소스 참조:**

* [shakeEntity](https://github.com/DansStuff/AstroCrew/blob/dbe11b05be2a753b27b6d5dd866d0554ff73344f/src/effects/cameraShake.ts#L13-L47)
* [CameraShakeSystem](https://github.com/DansStuff/AstroCrew/blob/dbe11b05be2a753b27b6d5dd866d0554ff73344f/src/effects/cameraShake.ts#L87-L121)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/tutorials-and-examples/published-games/astro-crew.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
