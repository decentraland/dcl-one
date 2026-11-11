> Decentraland SDK7 예제 장면, 메커니즘, 재사용 가능한 시스템을 엄선해 AI가 읽을 수 있도록 정리한 색인.

# 예제 씬

다른 제작자들이 씬 메커니즘을 어떻게 구현하는지 알아보려면 이 프로젝트들을 살펴보세요. 각 항목은 더 자세히 살펴볼 수 있도록 해당 소스 저장소로 연결됩니다.

이것은 인덱스이며 코드의 두 번째 복사본이 아닙니다. 예제는 이를 유지 관리하는 저장소에 그대로 있습니다.


> [!NOTE]
> **이 예제들에 대해:** 이 카탈로그에는 Decentraland가 유지 관리하는 프로젝트와 커뮤니티가 유지 관리하는 프로젝트가 포함되어 있습니다. 커뮤니티 프로젝트는 독립적으로 관리되며 시간이 지나면서 변경될 수 있습니다. 포함되었다고 해서 보안 감사가 수행되었거나 안전이 보장된다는 의미는 아닙니다. 예제를 실행하거나 재사용하기 전에 코드, 종속 항목, 설치 스크립트를 검토하세요. 유지 관리 라벨은 보안 평가가 아니라 소스 저장소의 GitHub 소유자를 반영합니다.


AI 에이전트인가요? 여기로 이동하세요: [catalog.json](https://raw.githubusercontent.com/decentraland/docs/main/creator/tutorials/examples/catalog.json). 여기에는 동일한 항목과 메타데이터가 기계가 읽을 수 있는 형식으로 들어 있습니다.

## 여기서 시작하세요

새 씬을 위한 핵심 SDK7 참조 자료와 시작점입니다.

| 예시                                                                                                      | 보여주는 내용                             | 관리 주체        | 태그          | 소스                                                            |
| ------------------------------------------------------------------------------------------------------- | ----------------------------------- | ------------ | ----------- | ------------------------------------------------------------- |
| [SDK7 씬 템플릿](/docs/creator/content-creator-ko/tutorials-and-examples/getting-started/sdk7-scene-template/) | 새 TypeScript SDK7 씬을 위한 깔끔한 시작점입니다. | Decentraland | `sdk7`, `씬` | [저장소 열기](https://github.com/decentraland/sdk7-scene-template) |

## 출시된 게임

실제 메커니즘을 보여주는 출시된 Decentraland 씬입니다.


> [!WARNING]
> **커뮤니티가 관리하는 코드 — 사용 전에 검토하세요**
> 
> 여기에 나열된 커뮤니티 프로젝트는 Decentraland Foundation 및 DAO GitHub 조직 외부에서 관리됩니다. 소스 코드와 라이브 배포는 독립적으로 변경될 수 있습니다.
> 
> 이 카탈로그에 포함되었다고 해서 보안 감사가 이루어졌거나 안전이 보장된다는 뜻은 아닙니다. 실행하거나 재사용하기 전에 코드, 종속성, 설치 스크립트를 검토하세요.
> 
> [카탈로그 관련 문제 신고](https://github.com/decentraland/docs/issues/new). 의심되는 취약점은 업스트림 프로젝트의 보안 정책을 따르세요. 공개 이슈에 익스플로잇 세부 정보나 자격 증명을 포함하지 마세요.


| 게임                                                                                                             | 보여주는 내용                                                                    | 관리 주체                  | 태그                                                                                | 소스                                                                         |
| -------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------- | ---------------------- | --------------------------------------------------------------------------------- | -------------------------------------------------------------------------- |
| [Alien Scrapyard](/docs/creator/content-creator-ko/tutorials-and-examples/published-games/alien-scrapyard/)       | 데이터로 정의된 건설 퍼즐과 단계 기반 시네마틱 카메라를 갖춘 협동 건설 라운드입니다.                           | 커뮤니티 · procesista      | `게임`, `멀티플레이어`, `제한 시간 라운드`, `카메라`, `시네마틱`                                        | [저장소 열기](https://github.com/procesista/Alienscrap)                         |
| [Astro Crew](/docs/creator/content-creator-ko/tutorials-and-examples/published-games/astro-crew/)                 | 서버가 소유한 게임 상태, 지속 진행, 터렛 카메라, 화면 가장자리 목표 마커가 있는 협동 우주선 전투입니다.              | 커뮤니티 · DansStuff       | `게임`, `멀티플레이어`, `멀티플레이어 서버`, `진행`, `리더보드`, `저장소`, `카메라`, `UI`, `전투`               | [저장소 열기](https://github.com/DansStuff/AstroCrew)                           |
| [Bloom Garden](/docs/creator/content-creator-ko/tutorials-and-examples/published-games/bloom-garden/)             | 지속적인 식물 상태, 서버 시계 오프셋 추정, 풀링된 꽃잎 애니메이션이 있는 협동 정원입니다.                       | 커뮤니티 · TheLivingGarden | `게임`, `멀티플레이어`, `멀티플레이어 서버`, `저장소`, `리더보드`, `애니메이션`, `효과`                         | [장면 열기](https://github.com/TheLivingGarden/TheLivingGarden/tree/main-main) |
| [Clean The Club](/docs/creator/content-creator-ko/tutorials-and-examples/published-games/clean-the-club/)         | 공유 컴포넌트 폴링과 들고 있는 아이템을 반영하는 아바타 부착형 소품이 있는 협동 나이트클럽 정리 게임입니다.              | 커뮤니티 · CleanTheClub    | `게임`, `멀티플레이어`, `제한 시간 라운드`, `아바타`, `퍼포먼스`, `상호작용`                                | [저장소 열기](https://github.com/CleanTheClub/CleanTheClub)                     |
| [Cozy Farm](/docs/creator/content-creator-ko/tutorials-and-examples/published-games/cozy-farm/)                   | 시간이 정해진 작물 성장 단계, 비료 보정, 하나의 엔터티 계층 구조에서 구성된 반복 농장을 갖춘 소셜 농사 게임입니다.        | Decentraland DAO       | `게임`, `소셜`, `진행`, `에셋`, `씬`                                                       | [저장소 열기](https://github.com/dcl-regenesislabs/cozy-farm)                   |
| [Dead Surge](/docs/creator/content-creator-ko/tutorials-and-examples/published-games/dead-surge/)                 | 무기 진행, 풀링된 보상 텍스트, 다른 좀비와 장애물을 피해 로컬 방식으로 적을 조종하는 좀비 생존 게임입니다.             | Decentraland DAO       | `게임`, `전투`, `진행`, `이동`, `UI`, `퍼포먼스`                                              | [저장소 열기](https://github.com/dcl-regenesislabs/dead-surge)                  |
| [Fastlane Bowling](/docs/creator/content-creator-ko/tutorials-and-examples/published-games/fastlane-bowling/)     | 서버에서 시뮬레이션한 물리, 지속 진행, 일괄 상태 업데이트, 모바일에 맞춰 스케일된 파티클 효과를 사용하는 멀티플레이어 볼링입니다. | 커뮤니티 · stom66          | `게임`, `스튜디오 제작`, `멀티플레이어`, `멀티플레이어 서버`, `물리`, `진행`, `리더보드`, `저장소`, `파티클`          | [저장소 열기](https://github.com/stom66/dcl-bowling)                            |
| [Flag Tag](/docs/creator/content-creator-ko/tutorials-and-examples/published-games/flag-tag/)                     | 시간 제한 라운드, 지속 리더보드, 풀링된 투사체, 근접 기반 조명 예산을 갖춘 멀티플레이어 도망 게임입니다.              | 커뮤니티 · iillee          | `게임`, `스튜디오 제작`, `멀티플레이어`, `멀티플레이어 서버`, `리더보드`, `제한 시간 라운드`, `조명`, `퍼포먼스`         | [저장소 열기](https://github.com/iillee/flagtag)                                |
| [Goal Legends](/docs/creator/content-creator-ko/tutorials-and-examples/published-games/goal-legends/)             | 서버가 진행하는 경기, 지속 리더보드, 단계 기반 카메라, 동기화된 공 애니메이션을 사용하는 멀티플레이어 페널티 슛아웃입니다.     | 커뮤니티 · carlosmu        | `게임`, `멀티플레이어`, `멀티플레이어 서버`, `저장소`, `리더보드`, `카메라`, `애니메이션`                        | [저장소 열기](https://github.com/carlosmu/goal-legends-arena)                   |
| [Kick Off](/docs/creator/content-creator-ko/tutorials-and-examples/published-games/kick-off/)                     | 충전된 공 차기, 즉각적인 로컬 이동 피드백, 반응형 일정 패널이 있는 월드컵 예측 씬입니다.                       | Decentraland DAO       | `게임`, `계절성`, `입력`, `멀티플레이어`, `UI`                                                 | [저장소 열기](https://github.com/dcl-regenesislabs/kickoff-2026)                |
| [Monster Recon](/docs/creator/content-creator-ko/tutorials-and-examples/published-games/monster-recon/)           | 수집 진행, 리더보드, 사용자 지정 비트맵 텍스트와 함께, 트리거 기반 몬스터 추격으로 이어지는 기억 맞추기 게임입니다.        | 커뮤니티 · carlosmu        | `게임`, `수집`, `리더보드`, `트리거`, `UI`, `스프라이트`, `애니메이션`                                 | [저장소 열기](https://github.com/carlosmu/dcl-monster-recon)                    |
| [My Dear Pet](/docs/creator/content-creator-ko/tutorials-and-examples/published-games/my-dear-pet/)               | 이동을 위한 가속과 제동이 있는 펫 수집 게임이며, 에디터에서 배치한 달걀의 위상 오프셋 애니메이션이 포함됩니다.            | Decentraland DAO       | `게임`, `수집`, `이동`, `애니메이션`                                                         | [저장소 열기](https://github.com/dcl-regenesislabs/marsh-colony)                |
| [Pigeon Deluxe](/docs/creator/content-creator-ko/tutorials-and-examples/published-games/pigeon-deluxe/)           | 픽업 및 하차 구역, 아바타에 장착된 방향 포인터, 배달 애니메이션과 분리된 점수 계산이 있는 시간제 소포 배송입니다.         | 커뮤니티 · PepeGawd        | `게임`, `커뮤니티 제작`, `제한 시간 라운드`, `트리거`, `아바타`, `점수`                                  | [저장소 열기](https://github.com/PepeGawd/Pigeon-Deluxe)                        |
| [Prime Drive](/docs/creator/content-creator-ko/tutorials-and-examples/published-games/prime-drive/)               | 재활용된 배경, 거리 기반 기록 유령, 지속적인 리더보드 데이터를 사용하는 3개 차선 레이싱입니다.                    | 커뮤니티 · carlosmu        | `게임`, `멀티플레이어 서버`, `리더보드`, `저장소`, `이동`, `퍼포먼스`                                    | [저장소 열기](https://github.com/carlosmu/dcl-prime-drive)                      |
| [Shroom Zoom](/docs/creator/content-creator-ko/tutorials-and-examples/published-games/shroom-zoom/)               | 트리거로 활성화되는 바운스 레이캐스트, 플레이어 임펄스, 소유자를 따라가거나 도는 수집형 스프라이트가 있는 플랫폼 게임입니다.     | 커뮤니티 · CanessaDCL      | `게임`, `플랫포머`, `수집품`, `트리거`, `레이캐스트`, `물리`, `애니메이션`, `스프라이트`                       | [저장소 열기](https://github.com/CanessaDCL/dcl-shroomzoom)                     |
| [Sky Chaser](/docs/creator/content-creator-ko/tutorials-and-examples/published-games/sky-chaser/)                 | 지속 리더보드, 풀링된 투사체, 라운드 상태에서 시드된 픽업 배치를 사용하는 멀티플레이어 패키지 받기 라운드입니다.           | 커뮤니티 · stom66          | `게임`, `스튜디오 제작`, `멀티플레이어`, `멀티플레이어 서버`, `리더보드`, `퍼포먼스`, `제한 시간 라운드`               | [저장소 열기](https://github.com/stom66/dcl-sky-chaser)                         |
| [Slay the Steps](/docs/creator/content-creator-ko/tutorials-and-examples/published-games/slay-the-steps/)         | 커스텀 아바타, 마켓플레이스 통합, 레이캐스트 기반 컬러 휠, 추적 카메라가 있는 멀티플레이어 Festive Trail 씬입니다.   | 커뮤니티 · stom66          | `게임`, `스튜디오 제작`, `멀티플레이어`, `멀티플레이어 서버`, `아바타`, `마켓플레이스 API`, `UI`, `카메라`, `레이캐스트` | [저장소 열기](https://github.com/stom66/dcl-slay-the-steps)                     |
| [Slide Penguin](/docs/creator/content-creator-ko/tutorials-and-examples/published-games/slide-penguin/)           | 대상과의 거리 기반으로 감속과 점수를 조정하는 시간제 쓸기 동작이 있는 펭귄 슬라이딩입니다.                        | 커뮤니티 · ToxSam          | `게임`, `계절성`, `플랫포머`, `입력`, `이동`, `점수`                                             | [저장소 열기](https://github.com/ToxSam/SlidePenguin)                           |
| [The Fortune Teller](/docs/creator/content-creator-ko/tutorials-and-examples/published-games/the-fortune-teller/) | 대화 상태 전환이 명시적인 사회적 점술 체험과 피벗 기반 시네마틱 카메라 리그입니다.                            | 커뮤니티 · carlosmu        | `게임`, `소셜`, `멀티플레이어`, `대화`, `카메라`, `시네마틱`                                         | [저장소 열기](https://github.com/carlosmu/circus_fortune_teller)                |
| [Tower of Madness](/docs/creator/content-creator-ko/tutorials-and-examples/published-games/tower-of-madness/)     | 변화하는 타워 구역을 통과하는 시간 제한 플랫포밍으로, 에셋 사전 로딩과 스킵 가능한 레벨 카메라 투어가 포함됩니다.          | Decentraland DAO       | `게임`, `플랫포머`, `제한 시간 라운드`, `로딩`, `에셋`, `카메라`, `시네마틱`                              | [저장소 열기](https://github.com/dcl-regenesislabs/towerofmadness)              |

## 게임플레이 및 상호작용

핵심 게임플레이 메커니즘과 플레이어 상호작용입니다.

| 예시                                                                                                                            | 보여주는 내용                                        | 관리 주체        | 태그                    | 소스                                                                                                    |
| ----------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------- | ------------ | --------------------- | ----------------------------------------------------------------------------------------------------- |
| [입력 수정자](/docs/creator/content-creator-ko/tutorials-and-examples/gameplay-interaction/input-modifier/)                           | 씬의 일부에서 플레이어 이동 입력을 변경하거나 제한합니다.               | Decentraland | `이동`, `입력`, `플레이어`    | [장면 열기](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,1-input-modifier)         |
| [파티클 시스템](/docs/creator/content-creator-ko/tutorials-and-examples/gameplay-interaction/particle-system/)                         | 불, 연기, 불꽃, 눈 및 기타 파티클 효과입니다.                   | Decentraland | `파티클`, `효과`, `비주얼`    | [장면 열기](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,7-particle-system)        |
| [포인터 잠금 제어](/docs/creator/content-creator-ko/tutorials-and-examples/gameplay-interaction/pointer-lock-control/)                  | 사용자 지정 조작을 위해 마우스 입력을 캡처합니다.                   | Decentraland | `입력`, `마우스`, `카메라`    | [장면 열기](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/31,20-pointer-lock-control) |
| [레이캐스팅 및 콜라이더 레이어](/docs/creator/content-creator-ko/tutorials-and-examples/gameplay-interaction/raycasting-and-collider-layers/) | 씬 기하를 가리키고 어떤 콜라이더가 레이캐스트에 반응할지 제어합니다.         | Decentraland | `상호작용`, `레이캐스트`, `충돌` | [장면 열기](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/77,-1-raycast-unit-tests)   |
| [트리거 영역](/docs/creator/content-creator-ko/tutorials-and-examples/gameplay-interaction/trigger-areas/)                            | 콜라이더 기반 트리거를 포함하여 플레이어가 영역에 들어오고 나가는 것을 감지합니다. | Decentraland | `상호작용`, `트리거`, `충돌`   | [장면 열기](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/75,-9-trigger-areas)        |
| [Zombie Attack](/docs/creator/content-creator-ko/tutorials-and-examples/gameplay-interaction/zombie-attack/)                     | 적과 점수가 있는 완전한 플레이 가능한 게임 씬입니다.                 | Decentraland | `게임`, `완전한 씬`, `점수`   | [장면 열기](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/zombie-attack)                    |

## UI, 카메라, 플레이어 경험

인터페이스, 카메라, 플레이어 대상 시스템입니다.

| 예시                                                                                                | 보여주는 내용                              | 관리 주체        | 태그                     | 소스                                                                                                    |
| ------------------------------------------------------------------------------------------------- | ------------------------------------ | ------------ | ---------------------- | ----------------------------------------------------------------------------------------------------- |
| [NPC 툴킷](/docs/creator/content-creator-ko/tutorials-and-examples/ui-camera-player/npc-toolkit/)      | SDK7 씬을 위한 재사용 가능한 NPC 동작과 대화 UI입니다. | Decentraland | `NPC`, `대화`, `UI`      | [저장소 열기](https://github.com/decentraland-scenes/dcl-npc-toolkit)                                      |
| [관전 모드](/docs/creator/content-creator-ko/tutorials-and-examples/ui-camera-player/spectate-mode/)     | 관전자 스타일의 카메라와 플레이어 시청 흐름입니다.         | Decentraland | `카메라`, `관전자`, `플레이어`   | [장면 열기](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/33,20-spectate-mode)        |
| [UI 애니메이션](/docs/creator/content-creator-ko/tutorials-and-examples/ui-camera-player/ui-animations/)  | 애니메이션 UI 요소, 스프라이트 시트, 로딩 표시기입니다.    | Decentraland | `UI`, `애니메이션`, `스프라이트` | [장면 열기](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/ui-animations)                    |
| [UI 세이프 영역](/docs/creator/content-creator-ko/tutorials-and-examples/ui-camera-player/ui-safe-areas/) | 화면 여백과 세이프 영역 주변에 인터페이스 요소를 배치합니다.   | Decentraland | `UI`, `HUD`            | [장면 열기](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/81,-2-ui-screen-inset-area) |
| [가상 카메라](/docs/creator/content-creator-ko/tutorials-and-examples/ui-camera-player/virtual-cameras/)  | 씬에서 정의한 카메라를 만들고 전환합니다.              | Decentraland | `카메라`, `시네마틱`, `컨트롤`   | [장면 열기](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/2,22-virtual-cameras)       |

## 멀티플레이어

멀티플레이어 패턴과 서버 소유 공유 상태.

| 예시                                                                                                                                 | 보여주는 내용                                      | 관리 주체        | 태그                            | 소스                                                                                                                |
| ---------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------- | ------------ | ----------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| [미니 게임 라이브러리](/docs/creator/content-creator-ko/tutorials-and-examples/multiplayer-data-services/mini-games-library/)                  | 멀티플레이어 미니 게임을 위한 플레이어 대기열, 점수판, 지원 로직.       | Decentraland | `멀티플레이어`, `점수판`, `게임`         | [저장소 열기](https://github.com/decentraland/sdk-mini-games)                                                          |
| [멀티플레이어 서버 리더보드](/docs/creator/content-creator-ko/tutorials-and-examples/multiplayer-data-services/authoritative-server-leaderboard/) | 점수를 검증하고, 총점을 저장하며, 순위를 브로드캐스트하는 서버 소유 리더보드. | Decentraland | `멀티플레이어`, `리더보드`, `서버`, `저장소` | [장면 열기](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/90,-9-authoritative-server-leaderboard) |

## 외부 서비스

장면을 외부 API, 데이터 소스 및 타사 서버에 연결합니다.

| 예시                                                                                                                      | 보여주는 내용                                  | 관리 주체        | 태그                             | 소스                                                                                                |
| ----------------------------------------------------------------------------------------------------------------------- | ---------------------------------------- | ------------ | ------------------------------ | ------------------------------------------------------------------------------------------------- |
| [스프레드시트의 데이터](/docs/creator/content-creator-ko/tutorials-and-examples/external-services/spreadsheet-data/)                 | Google 시트로 구동되는 장면 콘텐츠로, 재배포 없이 업데이트됩니다. | Decentraland | `api`, `스프레드시트`, `비디오`, `웨어러블` | [저장소 열기](https://github.com/decentraland/Read_Spreadsheet_Scene)                                  |
| [타사 방명록](/docs/creator/content-creator-ko/tutorials-and-examples/external-services/third-party-guest-book/)                | 외부 HTTP 서비스에 의해 지원되는 방명록입니다.             | Decentraland | `api`, `저장소`                   | [장면 열기](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/guest-book-api)               |
| [타사 리더보드](/docs/creator/content-creator-ko/tutorials-and-examples/external-services/third-party-leaderboard/)              | 외부 HTTP 서비스에 의해 지원되는 영속적 리더보드입니다.        | Decentraland | `api`, `저장소`, `리더보드`           | [장면 열기](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/leader-board)                 |
| [타사 플레이어 인증](/docs/creator/content-creator-ko/tutorials-and-examples/external-services/third-party-player-authentication/) | 외부 서비스로 전송되는 요청에 대한 플레이어 신원을 확인합니다.      | Decentraland | `api`, `인증`, `플레이어`            | [장면 열기](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/validate-player-authenticity) |

## 오디오, 비디오 및 환경

미디어, 조명 및 환경 제어입니다.

| 예시                                                                                                           | 보여주는 내용                           | 관리 주체        | 태그                   | 소스                                                                                                    |
| ------------------------------------------------------------------------------------------------------------ | --------------------------------- | ------------ | -------------------- | ----------------------------------------------------------------------------------------------------- |
| [오디오 시각화](/docs/creator/content-creator-ko/tutorials-and-examples/audio-video-environment/audio-visualization/) | 실시간 오디오 데이터를 사용해 시각적 씬 동작을 구동합니다. | Decentraland | `오디오`, `시각화`, `효과`   | [장면 열기](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/88,-10-audio-visualization) |
| [동적 조명](/docs/creator/content-creator-ko/tutorials-and-examples/audio-video-environment/dynamic-lights/)        | 런타임에 조명을 변경합니다.                   | Decentraland | `조명`, `환경`, `효과`     | [장면 열기](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,4-dynamic-lights)         |
| [스카이박스 제어](/docs/creator/content-creator-ko/tutorials-and-examples/audio-video-environment/skybox-control/)     | 씬 코드에서 스카이박스 시간과 전환 설정을 업데이트합니다.  | Decentraland | `스카이박스`, `환경`, `조명`  | [장면 열기](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/2,1-skybox-sdk-scene-a)     |
| [비디오 스트리밍](/docs/creator/content-creator-ko/tutorials-and-examples/audio-video-environment/video-streaming/)    | 씬 표면에서 스트리밍되는 비디오를 재생합니다.         | Decentraland | `비디오`, `스트리밍`, `미디어` | [장면 열기](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/video-streaming)                  |

## 성능 및 씬 시스템

에셋 로딩과 씬 수준 시스템입니다.

| 예시                                                                                                                             | 보여주는 내용                                | 관리 주체        | 태그                   | 소스                                                                                                |
| ------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------- | ------------ | -------------------- | ------------------------------------------------------------------------------------------------- |
| [에셋 로딩](/docs/creator/content-creator-ko/tutorials-and-examples/performance-scene-systems/asset-loading/)                         | 모든 에셋을 한 번에 불러오지 않고 필요할 때 씬 에셋을 로드합니다. | Decentraland | `퍼포먼스`, `로딩`, `에셋`   | [장면 열기](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/88,-12-asset-load)      |
| [머티리얼 및 텍스처 갤러리](/docs/creator/content-creator-ko/tutorials-and-examples/performance-scene-systems/material-and-texture-gallery/) | 머티리얼, 텍스처, GLTF 노드 수정자 패턴입니다.          | Decentraland | `머티리얼`, `텍스처`, `렌더링` | [장면 열기](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/52,-52-testing-gallery) |
| [수정자 영역](/docs/creator/content-creator-ko/tutorials-and-examples/performance-scene-systems/modifier-areas/)                       | 플레이어 이름표 숨기기와 같은 영역별 설정을 적용합니다.        | Decentraland | `플레이어`, `영역`, `수정자`  | [장면 열기](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/9,99-modifier-areas)    |

## 씬 관리 및 피드백

씬을 관리하고 플레이어 피드백을 수집하기 위한 도구입니다.

| 예시                                                                                                         | 보여주는 내용                                                                    | 관리 주체        | 태그                       | 소스                                                                                             |
| ---------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------- | ------------ | ------------------------ | ---------------------------------------------------------------------------------------------- |
| [씬 피드백](/docs/creator/content-creator-ko/tutorials-and-examples/scene-management-feedback/playtest-feedback/) | 플레이 중 또는 피드백 버튼, 키오스크, 영역을 통해 플레이어 평점과 댓글을 수집하며, CSV 결과는 씬 Storage에 저장됩니다. | Decentraland | `UI`, `멀티플레이어 서버`, `저장소` | [장면 열기](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,-1-scene-feedback) |

## 보상

Decentraland Rewards 캠페인을 통해 플레이어에게 착용 아이템을 제공하는 씬입니다.

| 예시                                                                                                  | 보여주는 내용                                        | 관리 주체        | 태그                         | 소스                                                                                        |
| --------------------------------------------------------------------------------------------------- | ---------------------------------------------- | ------------ | -------------------------- | ----------------------------------------------------------------------------------------- |
| [박수로 받기 배포기](/docs/creator/content-creator-ko/tutorials-and-examples/rewards/clap-to-claim-dispenser/) | 트리거 영역에서 박수 이모트를 하면 플레이어가 획득하는 착용 아이템 배포기입니다.  | Decentraland | `보상`, `웨어러블`, `트리거`, `이모트` | [장면 열기](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/clap-claim-dispenser) |
| [보상 수령 배포기](/docs/creator/content-creator-ko/tutorials-and-examples/rewards/reward-claim-dispenser/)   | Rewards 캠페인에서 보상을 수령하는 착용 아이템 배포기와 에어드롭 상자입니다. | Decentraland | `보상`, `웨어러블`, `캡차`, `UI`   | [장면 열기](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/reward-claim)         |

## AI 보조 제작용

가장 가까운 예제의 링크와 함께 명확한 요청을 코딩 에이전트에게 전달하세요. 예를 들어: "멀티플레이어 서버 리더보드 씬을 참고하여 내 게임의 점수판을 만들어 주세요."

먼저 [SDK Skills](https://github.com/decentraland/sdk-skills) 를 설치하여 에이전트가 최신 SDK7 안내를 받을 수 있게 하세요.

에이전트와 함께 작업하는 더 넓은 가이드는 다음을 참고하세요. [AI와 함께하는 바이브 코딩](/docs/creator/content-creator-ko/sdk7/getting-started/vibe-coding/).

## 레거시 참조 자료

[Awesome Repository](https://github.com/decentraland-scenes/Awesome-Repository) 는 역사적인 SDK6 자료에는 유용하지만, 정식 SDK7 소스는 아닙니다. 새 프로젝트에는 위의 SDK7 예제를 우선 사용하세요.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/tutorials-and-examples/examples.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
