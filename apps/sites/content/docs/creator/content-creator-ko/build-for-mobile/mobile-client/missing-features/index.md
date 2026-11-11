> Decentraland 데스크톱 클라이언트에서는 사용할 수 있지만 모바일 앱에서는 아직 지원되지 않는 기능과, 알려진 크로스플랫폼 불일치 사항입니다.

# 누락된 기능


> [!NOTE]
> 이 페이지는 Decentraland 데스크톱(Unity) 클라이언트와 모바일 앱 간의 기능 격차를 추적합니다. 이 정보의 출처는 [godot-explorer 기능 동등성 추적기](https://github.com/decentraland/godot-explorer/issues/2402) 이며 정기적으로 업데이트됩니다. ETA는 추정치이며 변경될 수 있습니다.
> 
> 마지막 검토: **2026년 10월** (모바일 앱 v1.15.0).


## 최근 출시됨

이는 모바일 앱 기준으로 더 이상 누락 기능이 아닙니다 **v1.15.0**, 이는 2026년 10월 앱 스토어에 배포되고 있습니다:

* [`stopEmote` 제한된 동작](/docs/creator/content-creator-ko/sdk7/interactivity/avatars/avatar-animations/#animate-only-the-upper-body). 씬에서 시작된 상체 이모트는 플레이어가 씬을 떠나면 일시 중지되고, 돌아오면 다시 시작되며, 씬이 언로드되면 중지됩니다 — v1.15.0
* [`트윈` `MoveRotateScale` 모드](/docs/creator/content-creator-ko/sdk7/3d/move-entities/#simultaneous-tweens) — v1.15.0
* 다른 플레이어의 위치는 로컬 플레이어의 위치처럼 씬 기준으로 씬에 보고됩니다 (`getPlayer({ userId }).position`, `onEnterScene`, 다른 플레이어에게 부착된 아이템) — v1.15.0
* 플레이어 이동, Unity와의 첫 번째 동등성 반영: 캡슐 크기, 계단 높이, 점프 궤적과 중력, 달리기 속도 및 공중 제어 — v1.15.0
* 플레이어로부터 아래 방향으로 쏜 레이캐스트가 올바른 `hitDistance` 경사 지오메트리에서 보고합니다 — v1.15.0
* 상체 이모트는 다른 모바일 플레이어에게도 상체 이모트로 보입니다 — v1.15.0

## 모바일에서 누락된 SDK 기능

* [사용자 지정 글꼴 (`fontSrc` 에 의해 정의됩니다 `TextShape`, `Label`, `버튼`, `입력` 및 `드롭다운`)](/docs/creator/content-creator-ko/sdk7/2d-ui/ui_text/#custom-fonts) — 모바일 앱은 이를 무시합니다 `fontSrc` 그리고 대체 폰트를 렌더링합니다 `글꼴` — ETA 없음
* [`아바타 이름표` (플레이어 이름표 위의 씬 작성 레이블)](https://github.com/decentraland/godot-explorer/issues/2811) — ETA 없음
* [트윈 시퀀스 (`TweenSequence`)](/docs/creator/content-creator-ko/sdk7/3d/move-entities/#tween-sequences) — 구현되지 않음; 순차 트윈은 애니메이션되지 않음 — ETA 없음
* [늘리기 대신 SDK7 UiBackground 나인슬라이스 타일](https://github.com/decentraland/godot-explorer/issues/2060) — ETA 없음
* Draco 메시 압축 (`KHR_draco_mesh_compression`) — 모바일 앱에는 런타임 Draco 디코더가 없습니다. Draco로 압축된 `.gltf`/`.glb` 모델은 에셋 서버가 이미 압축을 해제한 씬의 최적화된 에셋에서만 로드됩니다. 로컬 미리보기처럼 씬에 최적화된 에셋이 없으면 오류 없이 로드에 실패합니다. 안전하게 하려면 모델을 Draco 압축 없이 내보내세요. 이 경우의 오류 대화상자는 다음에서 추적됩니다 [#2432](https://github.com/decentraland/godot-explorer/issues/2432) — ETA 없음
* 스마트 아이템 — 모바일에서 공식 지원되지 않음

## 모바일에 없는 데스크톱 클라이언트 기능

* [근접 음성 채팅](https://github.com/decentraland/godot-explorer/issues/888) — 2027년 1분기
* [월드에서 가리키기](https://github.com/decentraland/godot-explorer/issues/1736) — ETA 없음
* [이름표 색상 변경](https://github.com/decentraland/godot-explorer/issues/1684) — ETA 없음
* [커뮤니티](https://github.com/decentraland/godot-explorer/issues/656) — 2027년 1분기
* [사진 갤러리](https://github.com/decentraland/godot-explorer/issues/680) — 2026년 12월
* [커뮤니티 스트림](https://github.com/decentraland/godot-explorer/issues/676) — 2027년 1분기
* [프로필 배지](https://github.com/decentraland/godot-explorer/issues/678) — 2026년 12월
* [일일 퀘스트](https://github.com/decentraland/godot-explorer/issues/682) — ETA 없음
* [채팅 반응](https://github.com/decentraland/godot-explorer/issues/1824) — 2027년 1분기
* [채팅 자동 번역](https://github.com/decentraland/godot-explorer/issues/2260) — 2027년 1분기
* [채팅: 다이렉트 메시지](https://github.com/decentraland/godot-explorer/issues/1120) — 2027년 1분기
* [모바일에서 DCL Cast 지원](https://github.com/decentraland/godot-explorer/issues/1881) — ETA 없음
* [YouTube / Google Drive 동영상 URL은 지원되지 않음](https://github.com/decentraland/godot-explorer/issues/2081) — 스토어 준수를 위해 모바일에서 제한됨
* [백팩의 의상 슬롯](https://github.com/decentraland/godot-explorer/issues/1625) — 2026년 12월
* [다른 플레이어에게 웨어러블 선물하기](https://github.com/decentraland/godot-explorer/issues/2957) — 2026년 12월
* [같은 계정이 다른 기기에서 로그인할 때 "세션 종료" 알림](https://github.com/decentraland/godot-explorer/issues/2880) — ETA 없음
* [카테고리 탐색(카테고리별로 체험 둘러보기)](https://github.com/decentraland/godot-explorer/issues/3037) — ETA 없음

## 플랫폼 간 불일치

* **Unity 대비 플레이어 이동 및 충돌**
  * [경사면, 모서리, 벽, 착지 및 이동 플랫폼](https://github.com/decentraland/godot-explorer/issues/2852) 및 [미끄러짐 동작](https://github.com/decentraland/godot-explorer/issues/2854) — 수정 검토 중, ETA 없음
  * [두께가 1cm보다 얇은 씬 콜라이더는 위에서만 충돌하며, 보이지 않는 블로커와 일부 SDK 충돌 레이어는 플레이어를 막지 않습니다](https://github.com/decentraland/godot-explorer/issues/2853) — ETA 없음
  * 스폰, 착지 및 점프 애니메이션, 이동 블렌딩 ([#1553](https://github.com/decentraland/godot-explorer/issues/1553), [#1554](https://github.com/decentraland/godot-explorer/issues/1554), [#2855](https://github.com/decentraland/godot-explorer/issues/2855), [#2856](https://github.com/decentraland/godot-explorer/issues/2856)) — 수정 검토 중, ETA 없음
* [다른 클라이언트의 일부 이모트는 모바일에서 재생되지 않습니다: Bevy Web 플레이어의 씬 트리거 이모트와 LiveKit을 통해 도착하는 Unity 플레이어의 이모트](https://github.com/decentraland/godot-explorer/issues/2986) — 수정 검토 중, ETA 없음
* [Creator Hub 모바일 미리보기에서 모바일 플레이어의 아바타가 데스크톱 테스터에게 보이지 않습니다](https://github.com/decentraland/godot-explorer/issues/2773) — 수정 진행 중, ETA 없음
* [아바타 치아가 흰색 대신 어둡게/회색으로 렌더링됩니다](https://github.com/decentraland/godot-explorer/issues/1994) — ETA 없음

## 입력/플랫폼 제약

* **터치 전용 입력** — 마우스 호버 상태, 키보드 단축키 또는 우클릭이 없습니다.
* **제한된 제스처 지원** — 핀치 투 줌(1인칭과 3인칭 카메라 전환)이 v1.14.0에 추가되었습니다. 현재 다른 제스처는 계획되어 있지 않습니다.

## 누락된 기능 신고

여기에 나열되지 않은 제한 사항을 발견하면, [신고해 주세요](/docs/creator/content-creator-ko/sdk7/debugging/report-bug/) 그래서 저희가 문서화하고 우선순위를 정할 수 있도록요.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/build-for-mobile/mobile-client/missing-features.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
