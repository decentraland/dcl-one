> 지난 1년 동안 SDK와 Creator Hub에서 변경된 내용 중 크리에이터에게 영향을 주는 항목만 필터링한 것.

# 릴리스 노트

이 페이지는 다음의 변경 사항을 나열합니다 [SDK](#sdk) 및 [Creator Hub](#creator-hub) 에서 지난 1년간 씬을 만드는 방식에 영향을 주는 변경 사항입니다. 빌드 도구, 테스트, 추적, 리팩터링과 같은 내부 작업은 제외되었습니다. 각 항목은 GitHub의 해당 풀 리퀘스트로 연결되며, 더 자세한 내용을 확인할 수 있습니다.

전체, 필터링되지 않은 노트는 다음에서 확인하세요 [SDK 릴리스](https://github.com/decentraland/js-sdk-toolchain/releases) 및 [Creator Hub 릴리스](https://github.com/decentraland/creator-hub/releases) GitHub에서.


> [!NOTE]
> Creator Hub 릴리스는 다음의 변경 사항을 묶습니다 `@dcl/inspector` (씬 편집기) 및 `@dcl/asset-packs` (스마트 아이템) 패키지의 변경 사항입니다. 이러한 변경 사항은 처음 포함한 Creator Hub 버전 아래에 나열되어 있습니다.


## SDK

### 7.31.0 (2026년 10월 22일)

* **사용자 지정 글꼴**: 새로운 `fontSrc` 속성은 텍스트를 `.ttf` 씬의 글꼴 파일로 렌더링하며, `TextShape`, `UiText`, `UiInput` 및 `UiDropdown` 및 React ECS의 `Label`, `Button`, `Input` 및 `Dropdown`. 다음을 참조하세요 [사용자 지정 글꼴](/docs/creator/content-creator-ko/sdk7/3d/text/#custom-fonts) 및 [UI의 사용자 지정 글꼴](/docs/creator/content-creator-ko/sdk7/2d-ui/ui_text/#custom-fonts). [#1656](https://github.com/decentraland/js-sdk-toolchain/pull/1656)
* **대형 씬 게시하기**: `npm run deploy` 50 MiB 이상의 씬을 배치 단위로 업로드하므로, 약 200MB가 넘는 씬도 게시할 수 있으며, 브라우저의 배포 페이지에서 업로드 진행 상황을 표시하고 취소할 수 있습니다. 다음을 참조하세요 [대형 씬 게시하기](/docs/creator/content-creator-ko/sdk7/publishing/publishing/#publish-large-scenes). [#1635](https://github.com/decentraland/js-sdk-toolchain/pull/1635)
* **수정 사항**: 로컬 미리보기는 각 씬 엔티티를 한 번만 제공하므로, 여러 parcel에 걸친 씬도 미리보기에서 로드됩니다 [#1650](https://github.com/decentraland/js-sdk-toolchain/pull/1650).

### 7.30.0 및 7.30.1 (2026년 10월 8일)

* **플레이어 언어**: `getPlayerLanguage()` 는 플레이어의 클라이언트에 설정된 언어를 BCP 47 태그로 반환하며, `onPlayerLanguageChanged` 는 세션 중간에 발생한 변경 사항을 보고합니다. 다음을 참조하세요 [플레이어 언어 가져오기](/docs/creator/content-creator-ko/sdk7/interactivity/runtime-data/#get-player-language). [#1622](https://github.com/decentraland/js-sdk-toolchain/pull/1622)
* **탐색기 패널이 닫힐 때까지 기다리기**: 새로운 `openExplorerUiAndWait()` 의 헬퍼는 `@dcl/sdk/explorer-ui` 에서 패널을 열고 플레이어가 닫으면, 선택적 시간 제한과 함께 해결됩니다. 다음을 참조하세요 [플레이어가 패널을 닫을 때까지 기다리기](/docs/creator/content-creator-ko/sdk7/interactivity/event-listeners/#wait-until-the-player-closes-a-panel). [#1543](https://github.com/decentraland/js-sdk-toolchain/pull/1543)
* **수정 사항**: 번들 감시기는 `node_modules` [#1640](https://github.com/decentraland/js-sdk-toolchain/pull/1640)를 무시하며; 미리보기는 모든 watch 재빌드 때 편집기 씬을 다시 확인합니다 [#1644](https://github.com/decentraland/js-sdk-toolchain/pull/1644).

### 7.29.0 (2026년 9월 15일)

* **UI 모듈 간 쌓임 순서**: `zIndex` 렌더러 옵션은 어떤 UI 모듈이 앞에 그려질지 결정합니다. 이것이 없으면 모듈은 처음 표시되는 순서대로 쌓이며, 메인 UI가 맨 뒤에 놓입니다. 다음을 참조하세요 [UI 모듈 간 쌓임 순서](/docs/creator/content-creator-ko/sdk7/2d-ui/onscreen-ui/#stacking-order-between-ui-modules). [#1607](https://github.com/decentraland/js-sdk-toolchain/pull/1607)
* **수정 사항**: 포인터 이벤트 핸들러를 제거하면 이제 그 핸들러가 생성한 항목도 제거되므로, 비활성화된 UI 버튼이 더 이상 처리할 수 없는 클릭을 광고하지 않으며, 핸들러를 다시 등록해도 중복 항목이 쌓이지 않습니다. [#1577](https://github.com/decentraland/js-sdk-toolchain/pull/1577)

### 7.28.0 (2026년 9월 10일)

* **포인터 이벤트는 카메라 거리를 사용합니다**: `maxCameraDistance` 가 다음을 대체합니다 `maxPlayerDistance` 포인터 이벤트에서. 이전 필드는 여전히 작동하지만 더 이상 사용되지 않습니다. 호버 힌트와 클릭은 이제 카메라와 아이템 사이의 거리에 따라 달라지며, 이는 플레이어가 보는 것과 일치합니다. [#1560](https://github.com/decentraland/js-sdk-toolchain/pull/1560)
* **AvatarNametag 컴포넌트**: 아바타 위에 표시되는 이름 레이블을 사용자 지정합니다. [#1600](https://github.com/decentraland/js-sdk-toolchain/pull/1600)
* **리얼름으로 텔레포트**: `teleportTo()` 는 특정 리얼름을 대상으로 할 수 있습니다. [#1601](https://github.com/decentraland/js-sdk-toolchain/pull/1601)
* **수정 사항**: 비활성화된 UI는 `Button` 더 이상 씬의 색을 어둡게 하지 않습니다 [#1578](https://github.com/decentraland/js-sdk-toolchain/pull/1578); 한 엔티티의 포인터 입력이 같은 프레임에서 다른 엔티티의 입력을 더 이상 숨기지 않습니다 [#1576](https://github.com/decentraland/js-sdk-toolchain/pull/1576); 연결되지 않은 `ActionCallback` 매개변수는 Script 스마트 아이템에서 다음으로 해석됩니다 `undefined` 대신 실패하지 않습니다 [#1605](https://github.com/decentraland/js-sdk-toolchain/pull/1605); composite가 루트 엔티티에 올바르게 인스턴스화됩니다 [#1587](https://github.com/decentraland/js-sdk-toolchain/pull/1587); 다음으로 설정된 선택적 컴포넌트 필드 `0` 또는 `false` 는 이제 저장됩니다 [#1582](https://github.com/decentraland/js-sdk-toolchain/pull/1582); 원시 또는 배열 스키마에서 정의된 컴포넌트는 다시 읽을 수 있습니다 [#1593](https://github.com/decentraland/js-sdk-toolchain/pull/1593); 여러 직렬화 및 네트워킹 견고성 수정 [#1592](https://github.com/decentraland/js-sdk-toolchain/pull/1592), [#1595](https://github.com/decentraland/js-sdk-toolchain/pull/1595), [#1568](https://github.com/decentraland/js-sdk-toolchain/pull/1568), [#1570](https://github.com/decentraland/js-sdk-toolchain/pull/1570).

### 7.27.0 (2026년 8월 27일)

* **이벤트 헬퍼 개선**: 포인터, 트리거, 오디오 및 기타 이벤트 헬퍼 시스템은 이제 고유한 이름과 일관된 동작을 가집니다. [#1536](https://github.com/decentraland/js-sdk-toolchain/pull/1536)
* **로딩 화면 상태**: `EngineInfo` 는 로딩 화면이 아직 보이는지 알려줍니다. [#1555](https://github.com/decentraland/js-sdk-toolchain/pull/1555)
* **더 빠른 빌드**: 프로젝트 파일은 빌드 및 배포 시 병렬로 처리됩니다. [#1481](https://github.com/decentraland/js-sdk-toolchain/pull/1481)
* **수정 사항**: 클릭 이벤트는 첫 번째 플레이어뿐 아니라 모든 플레이어에 대해 감지됩니다 [#1468](https://github.com/decentraland/js-sdk-toolchain/pull/1468); 콜백이 예외를 던진 뒤에도 타이머는 계속 작동합니다 [#1470](https://github.com/decentraland/js-sdk-toolchain/pull/1470); 마운트 해제된 React ECS UI 엔티티가 해제되어 메모리 누수를 수정합니다 [#1471](https://github.com/decentraland/js-sdk-toolchain/pull/1471); observable 구독은 더 이상 서로 간섭하지 않습니다 [#1467](https://github.com/decentraland/js-sdk-toolchain/pull/1467); 배포 시 로그인 단계가 헤더를 올바르게 전달합니다 [#1551](https://github.com/decentraland/js-sdk-toolchain/pull/1551); 렌더러 예약 엔티티 ID는 생성되거나 재활용되지 않습니다 [#1544](https://github.com/decentraland/js-sdk-toolchain/pull/1544), [#1460](https://github.com/decentraland/js-sdk-toolchain/pull/1460).

### 7.26.0 (2026년 8월 13일)

* **TouchScreenControls 컴포넌트**: 모바일에서 화면상의 컨트롤을 표시, 숨기기 및 사용자 지정합니다. 다음을 참조하세요 [화면 내 컨트롤](/docs/creator/content-creator-ko/sdk7/interactivity/touch-screen-controls/). [#1441](https://github.com/decentraland/js-sdk-toolchain/pull/1441), [#1510](https://github.com/decentraland/js-sdk-toolchain/pull/1510)
* **씬에서 Explorer UI 열기**: `openExplorerUi()` 제한된 액션은 지도, 백팩, 설정 및 기타 탐색기 화면을 열며, `ExplorerUiEventsResult` 는 닫힐 때를 보고합니다. [#1503](https://github.com/decentraland/js-sdk-toolchain/pull/1503), [#1511](https://github.com/decentraland/js-sdk-toolchain/pull/1511)
* **오디오 이벤트 및 이모트 상태**: 새로운 `audioEventSystem` 은 오디오 재생 이벤트에 반응하며, `AvatarEmoteCommand` 는 이모트 `상태를 노출합니다`. [#1530](https://github.com/decentraland/js-sdk-toolchain/pull/1530)
* **UI 화면 기본값**: UI는 기본 가상 화면과 화면 안쪽 여백 영역을 사용하며, 장치 픽셀 비율은 더 이상 레이아웃에 영향을 주지 않습니다. [#1489](https://github.com/decentraland/js-sdk-toolchain/pull/1489)
* **Bevy Web 미리보기**: 브라우저 미리보기는 기존 웹 탐색기 대신 Bevy Web 탐색기를 사용합니다. [#1502](https://github.com/decentraland/js-sdk-toolchain/pull/1502)
* **미리보기에서 최적화된 에셋**: 미리보기는 모델을 에셋 번들로 로드할 수 있어, 게시된 이후의 모습과 일치합니다. 다음을 참조하세요 [최적화된 에셋으로 미리보기](/docs/creator/content-creator-ko/sdk7/getting-started/preview-scene/#preview-with-optimized-assets). [#1539](https://github.com/decentraland/js-sdk-toolchain/pull/1539)
* **수정 사항**: `npm run start` 초기 빌드가 실패해도 계속 실행되므로, 오류를 수정하면 다시 빌드됩니다 [#1501](https://github.com/decentraland/js-sdk-toolchain/pull/1501); 사용 중인 포트가 올바르게 감지됩니다 [#1542](https://github.com/decentraland/js-sdk-toolchain/pull/1542).

### 7.24.5 (2026년 7월 15일)

* **빌보드 대상**: `빌보드` 는 카메라 대신 대상 엔티티를 향할 수 있습니다. [#1445](https://github.com/decentraland/js-sdk-toolchain/pull/1445)
* **더 작은 씬 번들**: `@dcl/ecs` 의 두 번째 복사본이 더 이상 모든 씬에 번들되지 않습니다. [#1450](https://github.com/decentraland/js-sdk-toolchain/pull/1450)
* **수정**: `MessageBus` 메시지는 순서대로 전송됩니다. [#1462](https://github.com/decentraland/js-sdk-toolchain/pull/1462)

### 7.24.4 (2026년 7월 3일)

* **React ECS용 InteractableArea**: 화면상의 상호작용 영역을 위한 새로운 UI 컴포넌트입니다. [#1411](https://github.com/decentraland/js-sdk-toolchain/pull/1411)

### 7.24.3 (2026년 7월 1일)

* **더 이상 45x45 parcel 제한 없음** 씬에 대해 [#1429](https://github.com/decentraland/js-sdk-toolchain/pull/1429)
* **더블 점프 및 글라이드**: `AvatarLocomotionSettings` 에는 더블 점프와 글라이딩을 켜고 끄는 필드가 있습니다. [#1402](https://github.com/decentraland/js-sdk-toolchain/pull/1402)
* **수정**: UI 가상 화면이 장치 픽셀 비율을 고려합니다. [#1433](https://github.com/decentraland/js-sdk-toolchain/pull/1433)

### 7.24.2 (2026년 6월 25일)

* **수정 사항**: 다시 무장되는 `setTimeout` 은 긴 프레임 후 씬을 더 이상 멈추게 하지 않습니다 [#1420](https://github.com/decentraland/js-sdk-toolchain/pull/1420); React ECS는 처음으로 설정된 선택적 prop을 유지합니다 [#1422](https://github.com/decentraland/js-sdk-toolchain/pull/1422); 배포 및 기타 CLI 요청 중 발생하던 "Premature close" 오류가 사라졌습니다 [#1428](https://github.com/decentraland/js-sdk-toolchain/pull/1428).

### 7.24.1 (2026년 6월 12일)

* **플레이어 충돌 레이어**: 메인 플레이어를 위한 새로운 충돌 레이어입니다. [#1414](https://github.com/decentraland/js-sdk-toolchain/pull/1414)
* **수정**: 다시 트리거된 `AudioSource` 는 더 이상 중복된 소리를 재생하지 않습니다. [#1382](https://github.com/decentraland/js-sdk-toolchain/pull/1382)

### 7.24.0 (2026년 6월 4일)

* **ScreenInsetArea**: 모바일에서 UI를 안전 영역 안에 유지하는 React ECS 컴포넌트입니다. 다음을 참조하세요 [모바일 안전 영역](/docs/creator/content-creator-ko/build-for-mobile/develop/safe-area/). [#1386](https://github.com/decentraland/js-sdk-toolchain/pull/1386)
* **Composite 제공자**: `getCompositeProvider()` 및 `setCompositeProvider()` 는 엔진에서 composite가 로드되는 방식을 고급으로 제어합니다. [#1333](https://github.com/decentraland/js-sdk-toolchain/pull/1333)

### 7.23.2 (2026년 5월 13일)

* **수정**: 플레이어가 안에 머무르는 동안 트리거 영역이 더 이상 네트워크 메시지를 남발하지 않습니다. [#1397](https://github.com/decentraland/js-sdk-toolchain/pull/1397)

### 7.23.1 (2026년 5월 5일)

* **오디오 분석**: 재생 중인 오디오 소스에서 실시간 데이터를 읽어 효과를 구동합니다. [#1256](https://github.com/decentraland/js-sdk-toolchain/pull/1256)

### 7.23.0 (2026년 5월 5일)

* **AvatarLocomotionSettings 컴포넌트**: 플레이어의 걷기, 달리기, 점프 설정을 제어합니다. [#1389](https://github.com/decentraland/js-sdk-toolchain/pull/1389)
* **수정자 입력 액션**: `IA_MODIFIER` 는 입력 명령 목록에서 사용할 수 있습니다. [#1390](https://github.com/decentraland/js-sdk-toolchain/pull/1390)

### 7.22.5 (2026년 4월 23일)

* **코드 전용 씬용 더 작은 번들**: 씬이 composite를 사용할 때만 스마트 아이템 라이브러리가 번들됩니다. [#1381](https://github.com/decentraland/js-sdk-toolchain/pull/1381)

### 7.22.4 (2026년 4월 16일)

* **배포에서 소스맵 제외** 를 통해 `.dclignore`. [#1373](https://github.com/decentraland/js-sdk-toolchain/pull/1373)
* **파티클 시스템 버스트** 속성이 업데이트되었습니다. [#1375](https://github.com/decentraland/js-sdk-toolchain/pull/1375)

### 7.22.3 (2026년 4월 8일)

* **ParticleSystem 컴포넌트**: 엔티티에서 파티클을 방출합니다. 다음을 참조하세요 [파티클 시스템](/docs/creator/content-creator-ko/sdk7/3d/particle-system/). [#1369](https://github.com/decentraland/js-sdk-toolchain/pull/1369)
* **더 적은 네트워크 트래픽**: 변경되지 않은 값을 가변 컴포넌트에 쓰더라도 더 이상 메시지를 보내지 않습니다. [#1359](https://github.com/decentraland/js-sdk-toolchain/pull/1359)
* **더 명확한 배포 오류** World가 없거나, 권한이 없거나, catalyst에 도달할 수 없을 때. [#1361](https://github.com/decentraland/js-sdk-toolchain/pull/1361), [#1370](https://github.com/decentraland/js-sdk-toolchain/pull/1370)

### 7.22.2 (2026년 3월 26일)

* **근접 상호작용**: 플레이어가 엔티티에 가까워질 때 반응합니다. 다음을 참조하세요 [근접 이벤트](/docs/creator/content-creator-ko/sdk7/interactivity/button-events/proximity-events/). [#1356](https://github.com/decentraland/js-sdk-toolchain/pull/1356)

### 7.22.1 (2026년 3월 26일)

* **업데이트된 로그인 페이지** CLI에서 배포할 때 사용됩니다. [#1362](https://github.com/decentraland/js-sdk-toolchain/pull/1362)

### 7.22.0 (2026년 3월 25일)

* **물리 힘과 임펄스**: 힘과 임펄스로 플레이어를 밀어냅니다. 다음을 참조하세요 [플레이어 물리](/docs/creator/content-creator-ko/sdk7/interactivity/player-physics/). [#1338](https://github.com/decentraland/js-sdk-toolchain/pull/1338)
* **멀티 씬 Worlds**: 여러 씬을 같은 World에 배포합니다. [#1355](https://github.com/decentraland/js-sdk-toolchain/pull/1355)
* **CI에서 배포** 파이프라인. [#1357](https://github.com/decentraland/js-sdk-toolchain/pull/1357)
* **수정 사항**: React ECS에서 키가 있는 목록 재정렬 [#1351](https://github.com/decentraland/js-sdk-toolchain/pull/1351); 액션 유형은 네트워크 동기화에서 제외됩니다 [#1354](https://github.com/decentraland/js-sdk-toolchain/pull/1354).

### 7.21.0 (2026년 3월 10일)

* **더블 점프와 글라이딩 비활성화** 씬에서. [#1323](https://github.com/decentraland/js-sdk-toolchain/pull/1323)
* **Tween MoveRotateScale 모드**: 위치, 회전, 크기를 하나의 트윈으로 애니메이션화합니다. [#1339](https://github.com/decentraland/js-sdk-toolchain/pull/1339)
* **플랫폼 감지**: 씬이 데스크톱, 웹 또는 모바일에서 실행되는지 알 수 있습니다. 다음을 참조하세요 [플랫폼 감지](/docs/creator/content-creator-ko/build-for-mobile/develop/detect-platform/). [#1340](https://github.com/decentraland/js-sdk-toolchain/pull/1340)

### 7.20.4 (2026년 3월 3일)

* **다중 인스턴스 미리보기**: 여러 플레이어 인스턴스를 하나의 미리보기에 연결해 멀티플레이어를 테스트합니다. [#1336](https://github.com/decentraland/js-sdk-toolchain/pull/1336)
* **수정 사항**: 엔티티 ID 충돌 [#1332](https://github.com/decentraland/js-sdk-toolchain/pull/1332); `UiInput` 는 더 이상 값을 다시 에코하지 않습니다 [#1331](https://github.com/decentraland/js-sdk-toolchain/pull/1331).

### 7.20.2 및 7.20.3 (2026년 2월 23일)

* **모바일 QR에는 위치가 포함됩니다** 에서 스폰합니다. [#1328](https://github.com/decentraland/js-sdk-toolchain/pull/1328)
* **수정 사항**: 동일한 타임스탬프를 가진 값(예: 포인터 이벤트 결과)은 올바른 순서로 정렬됩니다 [#1322](https://github.com/decentraland/js-sdk-toolchain/pull/1322); 모바일 미리보기 엔드포인트 수정([7.20.3 릴리스](https://github.com/decentraland/js-sdk-toolchain/releases/tag/7.20.3)).

### 7.20.1 (2026년 2월 16일)

* **멀티 씬 World에 배포할 때 흐름이 개선되었습니다.** [#1317](https://github.com/decentraland/js-sdk-toolchain/pull/1317)

### 7.20.0 (2026년 2월 5일)

* **에셋 사전 로드**: 필요해지기 전에 로드할 에셋을 표시합니다. [#1287](https://github.com/decentraland/js-sdk-toolchain/pull/1287)

### 7.19.0 (2026년 1월 29일)

* **SDK 헬퍼 함수**: 지시를 지연시키는 것과 같은 일반적인 작업을 위한 바로가기입니다. [#1275](https://github.com/decentraland/js-sdk-toolchain/pull/1275)
* **포인터 이벤트 최대 플레이어 거리**: 상호작용할 수 있는 플레이어의 최대 거리를 제한합니다. [#1293](https://github.com/decentraland/js-sdk-toolchain/pull/1293)

### 7.18.0 (2026년 1월 26일)

* **여러 UI 렌더러**: `addUiRenderer()` 및 `removeUiRenderer()` 는 React ECS 렌더러에서 사용할 수 있습니다. [#1285](https://github.com/decentraland/js-sdk-toolchain/pull/1285)
* **자동 UI 스케일 팩터** 는 React ECS에서. [#1290](https://github.com/decentraland/js-sdk-toolchain/pull/1290)
* **코드에서의 Script 인스턴스**: 코드 어디에서든 스마트 아이템의 Script 컴포넌트를 호출하고, 다음을 사용해 편집기에서 구성한 액션을 트리거할 수 있습니다 `ActionCallback`. [#1284](https://github.com/decentraland/js-sdk-toolchain/pull/1284)
* **Markdown 파일은 배포에서 제외됩니다.** [#1282](https://github.com/decentraland/js-sdk-toolchain/pull/1282)

### 7.17.0 (2026년 1월 5일)

* **빌드 오류에 유형 검사 출력이 표시됩니다.** [#1263](https://github.com/decentraland/js-sdk-toolchain/pull/1263)
* **모바일 미리보기는 좌표와 딥 링크를 출력합니다.** [#1268](https://github.com/decentraland/js-sdk-toolchain/pull/1268)

### 7.16.0 (2025년 12월 30일)

* **가시성이 자식에게 전파됩니다**: `VisibilityComponent` 로 엔티티를 숨기면 그 자식도 함께 숨겨집니다. [#1262](https://github.com/decentraland/js-sdk-toolchain/pull/1262)
* **모바일에서 미리보기**: 모바일 앱에서 미리보기를 실행합니다. [#1261](https://github.com/decentraland/js-sdk-toolchain/pull/1261)
* **Bevy Web 브라우저 미리보기** 지원됨. [#1241](https://github.com/decentraland/js-sdk-toolchain/pull/1241)
* **`npm run start` 새 데스크톱 탐색기를 기본적으로 엽니다.** [#1264](https://github.com/decentraland/js-sdk-toolchain/pull/1264)
* **수정**: 스마트 웨어러블 프로젝트 템플릿 다운로드가 다시 가능합니다. [#1255](https://github.com/decentraland/js-sdk-toolchain/pull/1255)

### 7.15.0 및 7.15.1 (2025년 12월 8일)

* **공간 오디오** 용 `AudioSource` 및 `VideoPlayer`. [#1243](https://github.com/decentraland/js-sdk-toolchain/pull/1243)
* **Script 컴포넌트 지원** 코드가 있는 스마트 아이템용 [#1245](https://github.com/decentraland/js-sdk-toolchain/pull/1245)
* **수정 사항**: Windows는 누락된 경우 Desktop Client 다운로드를 안내합니다 [#1244](https://github.com/decentraland/js-sdk-toolchain/pull/1244); 중첩된 곳에서도 스마트 아이템이 올바르게 해석됩니다 `node_modules` [#1258](https://github.com/decentraland/js-sdk-toolchain/pull/1258).

### 7.12.1 (2025년 11월 3일)

* **`code-to-composite` 명령**: 씬 코드를 편집기가 열 수 있는 composite로 변환합니다. [#1232](https://github.com/decentraland/js-sdk-toolchain/pull/1232)

### 7.12.0 (2025년 10월 30일)

* **CopyToClipboard 제한된 액션**. [#1234](https://github.com/decentraland/js-sdk-toolchain/pull/1234)

### 7.11.0 (2025년 10월 15일)

* **콘텐츠 서버의 더 명확한 오류** 배포 시. [#1217](https://github.com/decentraland/js-sdk-toolchain/pull/1217)
* **`dclcontext` 및 thumbnails 폴더는 배포에서 제외됩니다.** [#1227](https://github.com/decentraland/js-sdk-toolchain/pull/1227), [#1229](https://github.com/decentraland/js-sdk-toolchain/pull/1229)

### 7.10.6 (2025년 10월 9일)

* **트윈 연속 모드**: 계속 회전하거나 계속 이동합니다. [#1221](https://github.com/decentraland/js-sdk-toolchain/pull/1221)

### 7.10.5 (2025년 10월 6일)

* **`get-context-files` 명령**: AI 컨텍스트 문서를 씬에 다운로드하여 코딩 도우미가 SDK를 알 수 있게 합니다. [#1220](https://github.com/decentraland/js-sdk-toolchain/pull/1220)

### 7.10.3 (2025년 9월 29일)

* **트리거 영역 컴포넌트**: 플레이어가 영역에 들어오고, 머무르고, 나갈 때를 감지합니다. 다음을 참조하세요 [트리거 영역](/docs/creator/content-creator-ko/sdk7/3d/trigger-areas/). [#1210](https://github.com/decentraland/js-sdk-toolchain/pull/1210)
* **Tags 컴포넌트**: 엔티티에 태그를 지정하고 태그로 쿼리합니다. [#1216](https://github.com/decentraland/js-sdk-toolchain/pull/1216)

## Creator Hub

### 0.52.0 (2026년 10월 20일)

* **Avatar Animations 에셋 팩**: 바로 사용할 수 있는 아바타 애니메이션 45개와, 모든 씬이 이미 재생할 수 있는 기본 내장 애니메이션 32개가 포함되어 있습니다. 카드를 호버하면 미리 볼 수 있고, 하나를 드래그하여 파일을 씬에 복사한 다음 코드에서 재생할 수 있습니다. 다음을 참조하세요 [아바타 애니메이션](/docs/creator/content-creator-ko/scene-editor/get-started/about-editor/#avatar-animations). [#1689](https://github.com/decentraland/creator-hub/pull/1689)
* **AI Assistant 프롬프트에서 에셋 언급하기**: **로컬 에셋** 에서 에셋을 더블클릭해 다음 메시지에서 언급할 수 있으며, 선택한 항목과 함께 포함됩니다. 다음을 참조하세요 [프롬프트에서 에셋을 가리키기](/docs/creator/content-creator-ko/scene-editor/extend-with-code/ai-assistant/#point-at-an-asset-in-your-prompt). [#1689](https://github.com/decentraland/creator-hub/pull/1689)
* **400개가 넘는 새 에셋과 Buildings 및 Materials 카테고리**: Genesis Plaza와 Decentraland의 템플릿 씬의 모델, 물이 튀는 Water Plane 및 Water Cylinder 스마트 아이템, 그리고 플레이어가 위에 서 있는 동안 눌리는 Floor Pressure Button이 포함됩니다. 다음을 참조하세요 [물](/docs/creator/content-creator-ko/scene-editor/interactivity/smart-items/#water). [#1696](https://github.com/decentraland/creator-hub/pull/1696)
* **접을 수 있는 에셋 패널**: 패널 탭 바 오른쪽의 쉐브론을 누르면 패널이 탭만 남도록 줄어들어 3D 뷰가 창 전체 높이를 차지하게 됩니다. 참조 [항목 추가](/docs/creator/content-creator-ko/scene-editor/get-started/about-editor/#add-items). [#1691](https://github.com/decentraland/creator-hub/pull/1691)
* **터미널에서 AI Assistant 설정**: Claude와 Cursor가 이제 Gemini처럼 터미널에서 설치 및 로그인됩니다. 각 설정 화면에는 사용 중인 운영 체제에 맞는 공급업체의 설치 명령과 설치 가이드 링크가 표시되며, **다시 확인** 재시작 없이 새 설치를 찾아냅니다. 참조 [코딩 CLI 연결](/docs/creator/content-creator-ko/scene-editor/extend-with-code/ai-assistant/#connect-a-coding-cli). [#1684](https://github.com/decentraland/creator-hub/pull/1684)
* **게시 후 공유 가능한 링크**: 성공 화면이 데스크톱 클라이언트가 필요한 링크 대신, 누구나 브라우저에서 열 수 있는 웹 링크를 표시하고 복사합니다. 참조 [씬 게시](/docs/creator/content-creator-ko/scene-editor/publish/publish-scene/). [#1688](https://github.com/decentraland/creator-hub/pull/1688)
* **컬렉션 카드가 크리에이터 앱을 엽니다**: 착용 아이템과 이모트 **컬렉션** 카드의 **더 보기** 페이지는 이제 기존 Builder 대신 새 크리에이터 앱을 엽니다. [#1687](https://github.com/decentraland/creator-hub/pull/1687)
* **수정 사항**: 게시 실패를 보고하며 씬 썸네일이 "배포에 포함된 파일이어야 합니다"라고 표시됩니다. 영향을 받은 씬은 다음 썸네일 저장 시 스스로 수정됩니다 [#1657](https://github.com/decentraland/creator-hub/pull/1657); 레이아웃 탭에서 크기를 조정한 레이아웃이 유지되므로, 이제 게시해도 Bevy 렌더러에서 씬이 단일 필지로 초기화되지 않습니다 [#1682](https://github.com/decentraland/creator-hub/pull/1682); AI Assistant 신뢰성 개선: 무응답 턴이 이제 오류를 표시하고, 만료된 질문은 닫히며, **중지** Windows에서 턴을 정말 종료하며, 턴 도중에 보낸 메시지는 사라지지 않고 작성기로 돌아갑니다 [#1681](https://github.com/decentraland/creator-hub/pull/1681); 에이전트가 로그인되지 않았다고 보고할 때의 로그인 안내 [#1685](https://github.com/decentraland/creator-hub/pull/1685).

### 0.51.0 (Oct 8, 2026)

* **AI Assistant 프롬프트에 파일 첨부**: 파일을 메시지 상자로 끌어다 놓거나, 클립 버튼으로 선택하거나, 클립보드에서 이미지를 붙여넣을 수 있습니다. 이미지, 3D 모델, 오디오 및 기타 모든 파일이 지원되며, 메시지당 최대 8개까지 가능합니다. 참조 [프롬프트에 파일 첨부](/docs/creator/content-creator-ko/scene-editor/extend-with-code/ai-assistant/#attach-files-to-a-prompt). [#1670](https://github.com/decentraland/creator-hub/pull/1670)
* **미디어 드롭으로 빠른 항목**: 에서 이미지, 사운드 또는 비디오를 드래그하면 **로컬 에셋** 이제 캔버스에 놓으면 해당 파일을 이미 가리키는 바로 사용할 수 있는 항목이 만들어집니다. 참조 [커스텀 에셋 가져오기](/docs/creator/content-creator-ko/scene-editor/build/import-items/). [#1616](https://github.com/decentraland/creator-hub/pull/1616)
* **툴바 버튼의 레이블이 다시 재생으로 표시됩니다**그리고 해당 메뉴는 어디에서 미리 볼지 선택하는 메인 패널과 **데스크톱 클라이언트** 해당 환경에만 적용되는 옵션용 하위 메뉴로 나뉩니다. 이제 디버그 콘솔이 기본적으로 열리므로 더 이상 메뉴 옵션이 아닙니다. 참조 [씬 미리보기](/docs/creator/content-creator-ko/sdk7/getting-started/preview-scene/). [#1671](https://github.com/decentraland/creator-hub/pull/1671), [#1656](https://github.com/decentraland/creator-hub/pull/1656)
* **UI 편집기에서 정확한 확대 수준 입력**: 확대 비율이 이제 필드가 되었습니다. 클릭한 다음 10에서 200 사이의 값을 입력하고 Enter를 누르세요. 참조 [UI 편집기](/docs/creator/content-creator-ko/scene-editor/build/ui-editor/). [#1659](https://github.com/decentraland/creator-hub/pull/1659)
* **편집 중 씬 오디오 음소거**: 편집기 툴바의 스피커 버튼이 Bevy 렌더러에서 모든 오디오 소스와 스트림을 음소거합니다. 참조 [씬 렌더러](/docs/creator/content-creator-ko/scene-editor/get-started/about-editor/#scene-renderer). [#1669](https://github.com/decentraland/creator-hub/pull/1669)
* **씬 인셋 옵션 이름 변경**: UI 편집기에서 "Gameplay Safe Area"가 이제 "Interactable Safe Area"로 바뀝니다. 이는 동일한 `screenInset: 'interactable'` 값을 이전과 동일하게 기록합니다. 참조 [UI 편집기](/docs/creator/content-creator-ko/scene-editor/build/ui-editor/). [#1673](https://github.com/decentraland/creator-hub/pull/1673)
* **수정 사항**: Trigger Area 복제 및 모양 편집 필드 손실 [#1658](https://github.com/decentraland/creator-hub/pull/1658); MobileHUD 미리보기는 활성 GUI가 아니라 모바일 화면을 기준으로 표시됩니다 [#1648](https://github.com/decentraland/creator-hub/pull/1648); 씬 이름 변경 시 더 이상 키 입력이 누락되지 않으며, 편집기 헤더가 업데이트됩니다 [#1651](https://github.com/decentraland/creator-hub/pull/1651); 디버그 콘솔이 더 이상 다음 이후 사라지지 않습니다 **디스크에서 씬 다시 불러오기** [#1652](https://github.com/decentraland/creator-hub/pull/1652); Bevy 편집기 실행 취소 정리 및 종속성 토스트 [#1669](https://github.com/decentraland/creator-hub/pull/1669); 스마트 항목 툴팁에 Decentraland 로고가 렌더링됩니다 [#1677](https://github.com/decentraland/creator-hub/pull/1677); 삭제된 엔터티는 실행 취소로 복원할 수 있습니다 [#1679](https://github.com/decentraland/creator-hub/pull/1679); 긴 체크박스 레이블이 줄바꿈됩니다 [#1676](https://github.com/decentraland/creator-hub/pull/1676).

### 0.50.0 (Sep 23, 2026)

* **프로젝트 폴더에 드롭한 에셋이 자동으로 표시됩니다**: 프로젝트의 `assets` 폴더에 편집기 밖에서 복사된 파일이 표시됩니다 **로컬 에셋** 누르지 않고도 **새로고침**. 다음을 참조하세요 [커스텀 에셋 가져오기](/docs/creator/content-creator-ko/scene-editor/build/import-items/). [#1606](https://github.com/decentraland/creator-hub/pull/1606)
* **UI 편집기가 안정화되었습니다**: 더 이상 실험 설정 뒤에 있지 않습니다. 2D/3D 전환에서 열 수 있습니다. 씬에는 `@dcl/sdk` 7.26.0 이상이 필요합니다. 참조 [UI 편집기](/docs/creator/content-creator-ko/scene-editor/build/ui-editor/). [#1643](https://github.com/decentraland/creator-hub/pull/1643)
* **AI Assistant가 안정화되었습니다** 그리고 기본적으로 켜져 있으며, 별도의 **AI** 설정 탭이 있습니다. 참조 [AI Assistant](/docs/creator/content-creator-ko/scene-editor/extend-with-code/ai-assistant/). [#1646](https://github.com/decentraland/creator-hub/pull/1646)
* **편집기에서 3D 모델 최적화**: **최적화** 제목 표시줄과 **로컬 에셋** 패널의 최적화 작업이 모델을 압축하며, **되돌리기** 로 다시 복원할 수 있습니다. 참조 [모델 최적화](https://github.com/decentraland/docs/tree/main/creator/scene-editor/build/optimize-models.md). [#1566](https://github.com/decentraland/creator-hub/pull/1566)
* **Trigger Area 스마트 항목**: 보이지 않는 상자를 배치하고, 플레이어에게만 반응할지 모두에게 반응할지 선택한 뒤, 누군가 들어오거나 나갈 때 어떤 일이 일어나야 하는지 평이한 말로 설명하세요. 참조 [Trigger Area](/docs/creator/content-creator-ko/scene-editor/interactivity/trigger-area/). [#1604](https://github.com/decentraland/creator-hub/pull/1604)
* **UI 편집기에서 사용자 지정 가능한 MobileHUD**: 메인 버튼을 선택하고, 다른 버튼, 조이스틱, 십자선을 표시하거나 숨기며, 사용자 지정 아이콘을 적용할 수 있고, 캔버스에서 실시간 미리보기를 제공합니다. 참조 [모바일 컨트롤 사용자 지정](/docs/creator/content-creator-ko/scene-editor/build/ui-editor/#customize-the-mobile-controls). [#1615](https://github.com/decentraland/creator-hub/pull/1615)
* **16:9 씬 썸네일**: 썸네일 미리보기가 전체 너비로 표시되고, 다른 종횡비의 이미지는 거부되며, 자동 생성 썸네일은 16:9로 캡처됩니다. 참조 [씬 썸네일](/docs/creator/content-creator-ko/scene-editor/configure/scene-settings/#scene-thumbnail). [#1601](https://github.com/decentraland/creator-hub/pull/1601)
* **컴포넌트 설정이 SDK와 맞춰집니다**: 이제 인스펙터에서 비디오 재생 속도, 시작 위치와 공간 오디오, 텍스트 줄바꿈과 상자 크기, 오디오 피치, 그리고 색상과 텍스처를 지우는 방법이 포함된 재질 이동 및 회전 모드를 노출합니다. 코드에서 설정한 속성은 컴포넌트를 편집할 때 더 이상 사라지지 않습니다. 참조 [컴포넌트](/docs/creator/content-creator-ko/scene-editor/build/components/). [#1470](https://github.com/decentraland/creator-hub/pull/1470)
* **분리 가능한 디버그 콘솔**: 콘솔을 별도 창으로 띄워 두 번째 모니터에 둘 수 있습니다. 참조 [Creator Hub 콘솔](/docs/creator/content-creator-ko/sdk7/getting-started/preview-scene/#the-creator-hub-console). [#1642](https://github.com/decentraland/creator-hub/pull/1642)
* **AI Assistant는 Cursor와 Gemini에서 Creator Hub 씬 도구를 사용할 수 있습니다**, Claude와 Codex뿐만 아니라요. 참조 [AI Assistant](/docs/creator/content-creator-ko/scene-editor/extend-with-code/ai-assistant/). [#1614](https://github.com/decentraland/creator-hub/pull/1614)
* **수정 사항**: **모바일용 QR 코드 표시** 선택한 미리보기 클라이언트와 상관없이 작동하며, Bevy(Web) 미리보기는 해당 탭을 재사용합니다 [#1608](https://github.com/decentraland/creator-hub/pull/1608); 이제 게시가 LOD 생성까지 기다리지 않으므로 **최적화 중** 단계가 사라졌습니다 [#1603](https://github.com/decentraland/creator-hub/pull/1603); UI 편집기가 더 이상 고아 `./ui` 가 오래된 씬에서 남지 않습니다 [#1609](https://github.com/decentraland/creator-hub/pull/1609); Bevy 렌더러가 더 이상 "code elements" 미리보기 경고를 표시하지 않습니다 [#1607](https://github.com/decentraland/creator-hub/pull/1607); Admin Toolkit이 더 이상 씬의 나머지 UI z 순서를 변경하지 않습니다 [#1612](https://github.com/decentraland/creator-hub/pull/1612); Bevy에서 키보드 단축키 오버레이가 작동하고, 탐색 시 스크롤이 초기화되며, 엔터티 트리 아이콘 토글이 올바르게 작동합니다 [#1617](https://github.com/decentraland/creator-hub/pull/1617); AI Assistant 작성기, 채팅 드롭다운 및 알림을 다듬음 [#1632](https://github.com/decentraland/creator-hub/pull/1632), [#1621](https://github.com/decentraland/creator-hub/pull/1621).

### 0.49.0 (Sep 16, 2026)

* **이동 설정 스마트 항목**: 코드 대신 슬라이더로 플레이어의 이동 속도와 점프 높이를 변경합니다. 참조 [이동 설정](/docs/creator/content-creator-ko/scene-editor/interactivity/smart-items/#locomotion-settings). [#1584](https://github.com/decentraland/creator-hub/pull/1584)
* **AI Assistant가 Cursor와 Gemini를 지원합니다**, Claude와 Codex와 함께요. 참조 [AI Assistant](/docs/creator/content-creator-ko/scene-editor/extend-with-code/ai-assistant/). [#1591](https://github.com/decentraland/creator-hub/pull/1591)
* **정확한 변환 값 입력**: 드래그하는 대신 도구 키, 축, 숫자를 눌러 정확한 양만큼 이동, 회전 또는 크기 조정할 수 있습니다. **F** 는 더 이상 자유 기즈모의 단축키가 아닙니다. 참조 [정확한 값 입력](/docs/creator/content-creator-ko/scene-editor/get-started/scene-editor-essentials/#type-an-exact-amount). [#1599](https://github.com/decentraland/creator-hub/pull/1599)
* **수정 사항**: UI 편집기가 더 이상 전체 화면 컨테이너로 보기를 흐리게 하지 않으며, 중첩된 GUI 미리보기 값과 자유 배치가 작동합니다 [#1590](https://github.com/decentraland/creator-hub/pull/1590); Animator 컴포넌트가 Bevy 편집기 아래에 표시됩니다 [#1587](https://github.com/decentraland/creator-hub/pull/1587); Bevy 편집기 핫 리로드가 실제 빌드 이벤트에 의해 구동됩니다 [#1592](https://github.com/decentraland/creator-hub/pull/1592); AI Assistant 후속 턴이 Codex에서 다시 작동합니다 [#1600](https://github.com/decentraland/creator-hub/pull/1600); 드롭다운과 케밥 메뉴가 더 이상 잘리지 않습니다 [#1595](https://github.com/decentraland/creator-hub/pull/1595); 한 번의 클릭으로 씬 생성, 엔터티 이름의 연속 공백 유지, 씬 카드 메뉴가 더 이상 그리드를 재배치하지 않음, 에셋 툴팁이 검색 결과에서 올바르게 표시됨, 그리고 로컬 에셋 트리에서 드래그할 수 있습니다 [#1593](https://github.com/decentraland/creator-hub/pull/1593).

### 0.48.0 (Sep 10, 2026)

* **UI 편집기 모바일 미리보기**: 클라이언트와 동일한 기기 프리셋을 사용하고, 필요에 따라 켜고 끌 수 있는 HUD 가이드(조이스틱, 버튼, 채팅)를 포함한 실제 휴대폰 프레임과 정확한 안전 영역을 제공합니다. [#1579](https://github.com/decentraland/creator-hub/pull/1579)
* **AI Assistant 재설계**: 더 깔끔한 패널, 안정적인 닫기 버튼, 유지되는 소개 메시지, 향상된 설정을 제공합니다. 참조 [AI Assistant](/docs/creator/content-creator-ko/scene-editor/extend-with-code/ai-assistant/). [#1586](https://github.com/decentraland/creator-hub/pull/1586)
* **분석에서 평균 대신 중간 플레이 시간이 표시됩니다** 평균 대신. [#1564](https://github.com/decentraland/creator-hub/pull/1564)
* **변경 사항**: Bevy 렌더러에 "실험적" 라벨이 붙습니다 [#1583](https://github.com/decentraland/creator-hub/pull/1583); 씬 설정 필드가 이제 "Creator name"입니다 [#1561](https://github.com/decentraland/creator-hub/pull/1561); 게시 화면의 에셋 번들 지연 경고가 사라졌습니다 [#1582](https://github.com/decentraland/creator-hub/pull/1582).
* **수정**: Windows에서 명령줄이 너무 길어져도 AI Assistant 턴이 더 이상 실패하지 않습니다. [#1588](https://github.com/decentraland/creator-hub/pull/1588)

### 0.47.0 (Sep 4, 2026)

* **씬 에이전트**: 편집기에 내장된 AI Assistant입니다. 씬을 읽고 SDK7 코드를 작성하며, 뷰포트에서 실시간으로 엔터티, 컴포넌트, 스마트 항목 및 스크립트를 생성하고 편집할 수 있으며, 미리보기를 실행해 결과를 확인할 수 있습니다. 설치된 Claude 또는 Codex CLI를 사용하므로 API 키를 설정할 필요가 없습니다. [#1499](https://github.com/decentraland/creator-hub/pull/1499)
* **스마트 항목이 Script 컴포넌트에서 실행됩니다**: 각 스마트 항목의 동작이 이제 코드로 `.ts` 항목 폴더 안의 파일에 들어 있으며, 주요 함수는 액션으로 노출됩니다. 내용을 읽고 수정할 수 있습니다. [#1354](https://github.com/decentraland/creator-hub/pull/1354)
* **Script 스마트 항목용 슬라이더 입력**: 숫자 매개변수에 범위를 지정하면 슬라이더로 표시됩니다. 참조 [Script 컴포넌트 사용](/docs/creator/content-creator-ko/scene-editor/extend-with-code/script-component/). [#1513](https://github.com/decentraland/creator-hub/pull/1513)
* **UI 편집기**: 노드 자체의 위치(고정 또는 흐름형)는 자식 배치 방식과 독립적이며, 노드에는 Figma 스타일 아이콘이 있습니다. [#1552](https://github.com/decentraland/creator-hub/pull/1552)
* **공유 튜토리얼 목록 하나** 씬 사이드바와 Learn 페이지에 있습니다. [#1551](https://github.com/decentraland/creator-hub/pull/1551)
* **수정 사항**: Intel Mac이 다시 실행됩니다 [#1545](https://github.com/decentraland/creator-hub/pull/1545); UI 편집기가 2D 또는 3D 모드를 기억합니다 [#1532](https://github.com/decentraland/creator-hub/pull/1532); 가져온 애니메이션의 기본 가중치가 1입니다 [#1557](https://github.com/decentraland/creator-hub/pull/1557); Script 컴포넌트가 엔터티 패널에서 GLTF 바로 뒤에 표시됩니다 [#1556](https://github.com/decentraland/creator-hub/pull/1556); 사용자 지정 항목이 Bevy 렌더러에서 로드됩니다 [#1555](https://github.com/decentraland/creator-hub/pull/1555).

### 0.46.0 (Aug 27, 2026)

* **전체 화면 컨테이너** UI 편집기에서: 한 번의 클릭으로 전체 화면을 채우는 컨테이너입니다. [#1540](https://github.com/decentraland/creator-hub/pull/1540)

### 0.45.0 (Aug 27, 2026)

* **UI 편집기(실험적)**: 노드 트리와 속성 패널을 사용해 2D 캔버스에서 씬의 화면 인터페이스를 시각적으로 디자인하세요. 당신의 `@dcl/react-ecs` 코드는 단일 진실 소스로 유지됩니다. 설정 > 실험에서 켜세요. SDK 7.26.0 이상이 필요합니다. 참조 [UI 편집기](/docs/creator/content-creator-ko/scene-editor/build/ui-editor/). [#1523](https://github.com/decentraland/creator-hub/pull/1523), [#1522](https://github.com/decentraland/creator-hub/pull/1522), [#1521](https://github.com/decentraland/creator-hub/pull/1521), [#1520](https://github.com/decentraland/creator-hub/pull/1520), [#1524](https://github.com/decentraland/creator-hub/pull/1524)
* **아바타 및 카메라 수정 영역**: 아바타가 보이는 방식과 영역 내에서 카메라가 동작하는 방식을 바꿉니다. 컴포넌트 또는 스마트 항목으로 추가할 수 있습니다. 참조 [수정 영역](/docs/creator/content-creator-ko/scene-editor/interactivity/modifier-areas/). [#1471](https://github.com/decentraland/creator-hub/pull/1471)
* **Friendzone Buildathon 2026 워크숍** Learn > Videos에 있습니다. [#1510](https://github.com/decentraland/creator-hub/pull/1510)
* **수정 사항**: 이제 LAND에 게시할 때 "Publishing…"에서 멈추지 않습니다 [#1496](https://github.com/decentraland/creator-hub/pull/1496); Bevy 편집기에서 월드에 게시할 때 확인 버튼이 작동합니다 [#1493](https://github.com/decentraland/creator-hub/pull/1493); Analytics에 배포된 모든 씬이 나열됩니다 [#1492](https://github.com/decentraland/creator-hub/pull/1492); 새로운 NAME과 월드 권한이 재시작 없이 표시됩니다 [#1509](https://github.com/decentraland/creator-hub/pull/1509); Windows의 "Creator Hub cannot be closed" 업데이트 대화상자가 사라졌습니다 [#1497](https://github.com/decentraland/creator-hub/pull/1497); More 탭에서 Legacy Web Editor 카드가 हट고 Names가 마켓플레이스에 연결됩니다 [#1495](https://github.com/decentraland/creator-hub/pull/1495).

### 0.44.2 (Aug 14, 2026)

* **분석**: 데스크톱과 모바일로 나누어, 소유하거나 공동 작업 중인 모든 월드와 Genesis City 씬의 방문자, 유지율, 방문 및 참여도를 보여줍니다. 참조 [씬 분석](/docs/creator/content-creator-ko/sdk7/other/scene-analytics/). [#1447](https://github.com/decentraland/creator-hub/pull/1447)
* **홈 화면 재설계.** [#1481](https://github.com/decentraland/creator-hub/pull/1481)
* **수정 사항**: GLB 가져오기가 더 이상 스피너에서 멈추지 않습니다 [#1487](https://github.com/decentraland/creator-hub/pull/1487); 로컬 멀티플레이어 서버가 씬 미리보기에서 다시 시작됩니다 [#1455](https://github.com/decentraland/creator-hub/pull/1455).

### 0.44.0 (Aug 4, 2026)

* **Bevy 렌더러(실험적)**: 편집기 뷰포트를 위한 대체 렌더러입니다. [#1330](https://github.com/decentraland/creator-hub/pull/1330), [#1363](https://github.com/decentraland/creator-hub/pull/1363)
* **에셋 최적화 미리보기 옵션**: 에셋 번들로 미리보기하여 모델이 게시 후와 같아 보이게 합니다. 참조 [최적화된 에셋으로 미리보기](/docs/creator/content-creator-ko/sdk7/getting-started/preview-scene/#preview-with-optimized-assets). [#1396](https://github.com/decentraland/creator-hub/pull/1396)
* **Explorer MCP 전환** 미리보기 옵션에서. [#1440](https://github.com/decentraland/creator-hub/pull/1440)
* **엔터티 트리 검색.** [#1385](https://github.com/decentraland/creator-hub/pull/1385)
* **프로젝트의 폴더 이름 변경** 씬 보기에서. [#1357](https://github.com/decentraland/creator-hub/pull/1357)
* **"Hide Landscape Terrain" 씬 설정.** [#1383](https://github.com/decentraland/creator-hub/pull/1383)
* **관리 도구 디자인 변경** 새로운 Decentraland 디자인에 맞춰. [#1360](https://github.com/decentraland/creator-hub/pull/1360)
* **Test Server 게시 옵션 제거.** [#1441](https://github.com/decentraland/creator-hub/pull/1441)
* **수정 사항**: 모바일 QR 미리보기가 더 이상 데스크톱 클라이언트를 열지 않습니다 [#1452](https://github.com/decentraland/creator-hub/pull/1452); 엔터티 트리의 잠금 및 숨기기 토글을 다시 끌 수 있습니다 [#1429](https://github.com/decentraland/creator-hub/pull/1429); Custom Items에 Placeholder 컴포넌트 에셋이 포함됩니다 [#1287](https://github.com/decentraland/creator-hub/pull/1287); DCL Cast 관리 패널이 자동으로 열리고, 슬라이드 상태를 유지하며, 늦게 합류한 사람에게도 복제됩니다 [#1358](https://github.com/decentraland/creator-hub/pull/1358); 관리 도구가 권한 서버에서 초기화를 건너뜁니다 [#1381](https://github.com/decentraland/creator-hub/pull/1381).

### 0.43.0 (Jul 6, 2026)

* **더 간단한 로그인**: 코드를 입력하는 대신 브라우저에서 한 번 확인하면 됩니다. [#1338](https://github.com/decentraland/creator-hub/pull/1338)
* **수정 사항**: 배포가 시간 초과될 때 명확한 오류 표시 [#1355](https://github.com/decentraland/creator-hub/pull/1355); 음수 및 양수 좌표에 걸친 씬의 필지 수를 올바르게 표시 [#1352](https://github.com/decentraland/creator-hub/pull/1352); 다시 가져올 때 "씬이 이미 가져와졌습니다" 메시지 표시 [#1350](https://github.com/decentraland/creator-hub/pull/1350).

### 0.42.0 (Jun 26, 2026)

* **Particle System 컴포넌트** 편집기에서. 참조 [파티클 시스템](/docs/creator/content-creator-ko/sdk7/3d/particle-system/). [#1311](https://github.com/decentraland/creator-hub/pull/1311)
* **엔터티 생성 액션**: 숨겨진 복사본을 미리 배치하는 대신 액션 체인에서 스마트 항목을 생성합니다. [#1322](https://github.com/decentraland/creator-hub/pull/1322)
* **컴포넌트 값 복사 및 붙여넣기** 컴포넌트의 세 점 메뉴에서. [#1321](https://github.com/decentraland/creator-hub/pull/1321)
* **월드당 최대 100명의** 공동 작업자 [#1310](https://github.com/decentraland/creator-hub/pull/1310)
* **공동 작업자는 월드 씬의 게시를 취소할 수 있습니다.** [#1209](https://github.com/decentraland/creator-hub/pull/1209)
* **변경 사항**: Teen 연령 등급이 제거됩니다 [#1329](https://github.com/decentraland/creator-hub/pull/1329); 싱글 플레이어 모드에서는 라이브 스트리밍이 지원되지 않는다는 경고가 표시됩니다 [#1336](https://github.com/decentraland/creator-hub/pull/1336).
* **수정 사항**: Wind Area와 Impulse Area는 현재 플레이어에게만 반응합니다 [#1323](https://github.com/decentraland/creator-hub/pull/1323); 관리 도구 UI가 모바일 화면에 맞습니다 [#1256](https://github.com/decentraland/creator-hub/pull/1256); 여러 컴포넌트의 컨트롤 [#1282](https://github.com/decentraland/creator-hub/pull/1282); 인스펙터 시작 오류 [#1334](https://github.com/decentraland/creator-hub/pull/1334).

### 0.41.1 (Apr 29, 2026)

* **새로고침 버튼** 상단 바의 버튼으로 편집기를 다시 시작할 수 있어, AI 도구로 씬을 편집한 뒤 유용합니다. [#1312](https://github.com/decentraland/creator-hub/pull/1312)
* **콘솔 자동 스크롤** 그리고 텍스트 복사를 지원합니다. [#1305](https://github.com/decentraland/creator-hub/pull/1305)

### 0.41.0 (Apr 27, 2026)

* **모바일 디버그 세션**: QR로 모바일 클라이언트와 페어링하고, 엔터티 상태, 콘솔 로그 및 성능 데이터를 편집기로 스트리밍하며, 틱 스크러빙과 일시정지, 재개, 다시 불러오기 명령을 사용할 수 있습니다. [#1306](https://github.com/decentraland/creator-hub/pull/1306)
* **DCL Cast용 스피커 쇼케이스 및 프레젠테이션 패널**: 관리 도구에서 라이브 스피커와 स्ल라이드를 관리합니다. [#1213](https://github.com/decentraland/creator-hub/pull/1213)
* **엔터티 수준 오류** 엔터티 트리에서: 빨간 아이콘이 잘못된 컴포넌트 값을 가진 엔터티를 표시합니다. [#1258](https://github.com/decentraland/creator-hub/pull/1258)
* **"주변 음성 채팅 비활성화"** 씬 제한. [#1289](https://github.com/decentraland/creator-hub/pull/1289)
* **"저장소 보기" 버튼** 월드에서 권한 서버 데이터를 보고 편집하기 위한. [#1307](https://github.com/decentraland/creator-hub/pull/1307)
* **Wind Area 및 Impulse Area 스마트 항목** 플레이어를 밀어내는. [#1296](https://github.com/decentraland/creator-hub/pull/1296), [#1308](https://github.com/decentraland/creator-hub/pull/1308)
* **보상 스마트 항목**: 보상 캠페인에서 착용 아이템을 지급하는 Balloons, 2개의 Wearable 분배기, 그리고 Clap claim 분배기입니다. [#1124](https://github.com/decentraland/creator-hub/pull/1124)
* **수정 사항**: 회전 값이 필드를 벗어날 때 더 이상 드리프트하지 않습니다 [#1266](https://github.com/decentraland/creator-hub/pull/1266); "Fixed Skybox" 체크박스가 다른 체크박스와 함께 뒤집히지 않습니다 [#1254](https://github.com/decentraland/creator-hub/pull/1254); 월드에 배포할 때 필지가 겹치는 씬만 배포 해제됩니다 [#1271](https://github.com/decentraland/creator-hub/pull/1271); 중복 항목이 트리에서 원본 옆에 표시됩니다 [#1150](https://github.com/decentraland/creator-hub/pull/1150); 트리거 영역 모델 업데이트 [#1285](https://github.com/decentraland/creator-hub/pull/1285).

### 0.39.1 (Mar 30, 2026)

* **"장소에 표시"** 월드 설정의 체크박스. [#1264](https://github.com/decentraland/creator-hub/pull/1264)
* **수정 사항**: 씬 코드가 손상되었을 때 미리보기가 조용히 실패하지 않고 오류를 표시합니다 [#1238](https://github.com/decentraland/creator-hub/pull/1238); 배포 파일에서 다중 씬 플래그가 검증됩니다 [#1270](https://github.com/decentraland/creator-hub/pull/1270).

### 0.39.0 (Mar 26, 2026)

* **수정 사항**: Windows 설치 관리자 업데이트 [#1252](https://github.com/decentraland/creator-hub/pull/1252); 다중 씬 배포 [#1262](https://github.com/decentraland/creator-hub/pull/1262); 작동하지 않던 스마트 항목 [#1260](https://github.com/decentraland/creator-hub/pull/1260); 트리거 및 클릭 영역에 포인터 메쉬가 표시됩니다 [#1255](https://github.com/decentraland/creator-hub/pull/1255).

### 0.38.0 (Mar 20, 2026)

* **수정 사항**: 에셋 팩이 다시 스크롤됩니다 [#1239](https://github.com/decentraland/creator-hub/pull/1239); 새 씬에서 사용자 지정 스마트 항목을 생성할 때 오디오 파일이 유지됩니다 [#1234](https://github.com/decentraland/creator-hub/pull/1234).

### 0.37.0 (Mar 16, 2026)

* **내장 디버그 콘솔** 에셋 패널의 탭으로. [#1200](https://github.com/decentraland/creator-hub/pull/1200)
* **수정 사항**: 사용자 지정 항목이 텍스처 파일을 유지합니다 [#1217](https://github.com/decentraland/creator-hub/pull/1217); 기즈모가 첫 선택에서 작동합니다 [#1218](https://github.com/decentraland/creator-hub/pull/1218); 이미지 표시 액션의 기본값이 가운데 정렬입니다 [#1221](https://github.com/decentraland/creator-hub/pull/1221); 비디오 소스 복사본이 더 이상 Vimeo만 언급하지 않습니다 [#1216](https://github.com/decentraland/creator-hub/pull/1216).

### 0.36.0 (Mar 10, 2026)

* **수정 사항**: 엔터티 트리에서 스폰 포인트 선택 [#1199](https://github.com/decentraland/creator-hub/pull/1199); 월드 콘텐츠 등급에서 "Restricted"가 제거됩니다 [#1205](https://github.com/decentraland/creator-hub/pull/1205); 게시 관리에는 게시된 씬이 있는 월드만 표시됩니다 [#1208](https://github.com/decentraland/creator-hub/pull/1208); 월드 제한을 초과할 때 배포 오류 발생 [#1210](https://github.com/decentraland/creator-hub/pull/1210).

### 0.35.0 (Mar 5, 2026)

* **스폰 포인트 시각화** 편집기에서. 참조 [스폰 영역](/docs/creator/content-creator-ko/scene-editor/build/spawn-areas/). [#1129](https://github.com/decentraland/creator-hub/pull/1129)
* **동적 에셋 카탈로그**: 최신 항목이 런타임에 로드되며, SDK 호환성 검사가 포함됩니다. [#1172](https://github.com/decentraland/creator-hub/pull/1172)
* **다중 인스턴스 미리보기** 멀티플레이어를 테스트하는 전환 버튼. SDK 7.20.4 이상이 필요합니다. [#1187](https://github.com/decentraland/creator-hub/pull/1187)
* **월드 권한 Access 탭**, 그리고 멀티플레이어 및 Skybox 설정이 World Settings로 돌아옵니다. [#1186](https://github.com/decentraland/creator-hub/pull/1186), [#1189](https://github.com/decentraland/creator-hub/pull/1189)
* **UI 새로고침** 색상, 컴포넌트 스타일, 엔터티 인스펙터를. [#1178](https://github.com/decentraland/creator-hub/pull/1178)
* **수정 사항**: TextShape가 올바르게 초기화됩니다 [#1194](https://github.com/decentraland/creator-hub/pull/1194) 그리고 잘림 없이 렌더링됩니다 [#1185](https://github.com/decentraland/creator-hub/pull/1185); 바닥 타일 중복 [#1179](https://github.com/decentraland/creator-hub/pull/1179); 고급 모드에서 씬 레이아웃이 업데이트됩니다 [#1181](https://github.com/decentraland/creator-hub/pull/1181); 선택 강조가 더 이상 지연되거나 객체를 검게 만들지 않습니다 [#1196](https://github.com/decentraland/creator-hub/pull/1196); 관리되는 프로젝트가 로드됩니다 [#1182](https://github.com/decentraland/creator-hub/pull/1182); Scene Admin UI가 모든 화면에 맞게 크기 조정됩니다 [#1190](https://github.com/decentraland/creator-hub/pull/1190).

### 0.34.0 (Feb 25, 2026)

* **멀티 씬 Worlds**: 여러 씬을 하나의 월드에 게시합니다. [#1089](https://github.com/decentraland/creator-hub/pull/1089), [#1163](https://github.com/decentraland/creator-hub/pull/1163)
* **월드 관리**: 월드 설정과 관리 페이지. [#1132](https://github.com/decentraland/creator-hub/pull/1132), [#1049](https://github.com/decentraland/creator-hub/pull/1049), [#1112](https://github.com/decentraland/creator-hub/pull/1112)
* **공동 작업자 권한** 월드용. [#1128](https://github.com/decentraland/creator-hub/pull/1128), [#1083](https://github.com/decentraland/creator-hub/pull/1083), [#1125](https://github.com/decentraland/creator-hub/pull/1125)
* **월드에서 씬 게시를 취소합니다.** [#1123](https://github.com/decentraland/creator-hub/pull/1123)
* **Placeholder 컴포넌트**: 씬 지표에 영향을 주지 않는, 앉기 지점이나 트리거 영역 같은 보이지 않는 엔터티입니다. [#1154](https://github.com/decentraland/creator-hub/pull/1154)
* **로그 및 삭제 액션** 스마트 항목용. [#1131](https://github.com/decentraland/creator-hub/pull/1131)
* **수정 사항**: 트리거 영역은 자신의 플레이어에게만 활성화됩니다 [#1142](https://github.com/decentraland/creator-hub/pull/1142); 씬 웹 트래픽용 devtools가 다시 작동합니다 [#1109](https://github.com/decentraland/creator-hub/pull/1109); "카메라 변경"에서 가상 카메라 지속 시간 및 자기 선택 [#1160](https://github.com/decentraland/creator-hub/pull/1160); 크기 조절 기즈모 [#1082](https://github.com/decentraland/creator-hub/pull/1082); Photo Wall 스마트 아이템 [#1110](https://github.com/decentraland/creator-hub/pull/1110); "Custom Code" 경고가 더 정확해졌습니다 [#1171](https://github.com/decentraland/creator-hub/pull/1171); World 접근 문제 [#1133](https://github.com/decentraland/creator-hub/pull/1133); 씬 썸네일 [#1168](https://github.com/decentraland/creator-hub/pull/1168); 사용하지 않는 저장 아이콘과 "느린 연결" 경고가 제거되었습니다 [#1152](https://github.com/decentraland/creator-hub/pull/1152), [#1169](https://github.com/decentraland/creator-hub/pull/1169).

### 0.33.1 (2026년 2월 18일)

* **모바일 앱에서 미리보기**, 미리보기 설정에서 구성할 수 있습니다. [#1121](https://github.com/decentraland/creator-hub/pull/1121)
* **스크립트 구성 요소 툴팁** 스크립트 파일의 주석에서 가져온 스크립트 구성 요소 툴팁과 개선된 템플릿. [#1130](https://github.com/decentraland/creator-hub/pull/1130), [#1019](https://github.com/decentraland/creator-hub/pull/1019)
* **글꼴 선택** TextShape 구성 요소에서. [#1096](https://github.com/decentraland/creator-hub/pull/1096)
* **수정 사항**: 회전 기즈모 [#1081](https://github.com/decentraland/creator-hub/pull/1081); 조명 스마트 아이템 편집 [#1139](https://github.com/decentraland/creator-hub/pull/1139); 게시 시 잘못된 "signal is aborted" 오류 [#1144](https://github.com/decentraland/creator-hub/pull/1144); 복제 후 기즈모가 더 이상 자유 이동으로 전환되지 않습니다 [#1088](https://github.com/decentraland/creator-hub/pull/1088); Clap Meter 및 Photo Wall 스마트 아이템 [#1101](https://github.com/decentraland/creator-hub/pull/1101).

### 0.33.0 (2026년 2월 4일)

* **작업으로서의 스크립트 메서드**: 작업 유형 드롭다운에 Script 구성 요소의 메서드가 포함됩니다. [#1026](https://github.com/decentraland/creator-hub/pull/1026), [#1071](https://github.com/decentraland/creator-hub/pull/1071)
* **`isAdmin` 함수** 플레이어가 씬 관리자 여부를 확인하기 위한. [#1037](https://github.com/decentraland/creator-hub/pull/1037)
* **Worlds 저장소 페이지.** [#1036](https://github.com/decentraland/creator-hub/pull/1036)
* **오프라인 모드** "연결 없음" 페이지와 함께. [#991](https://github.com/decentraland/creator-hub/pull/991)
* **수정 사항**: 자산 검색이 태그와 설명을 고려합니다 [#1022](https://github.com/decentraland/creator-hub/pull/1022); 크기 조절 기즈모가 좌우 드래그에 반응하며 크기를 0으로 설정할 수 없습니다 [#1065](https://github.com/decentraland/creator-hub/pull/1065); 1보다 작은 값의 비례 스케일링 [#1076](https://github.com/decentraland/creator-hub/pull/1076); 잠금 상태가 자식에게 상속되지 않습니다 [#1085](https://github.com/decentraland/creator-hub/pull/1085); Photo Mural이 Worlds의 이미지를 지원합니다 [#1074](https://github.com/decentraland/creator-hub/pull/1074); 이미지 전용 사용자 지정 항목이 해당 폴더를 생성합니다 [#1033](https://github.com/decentraland/creator-hub/pull/1033); 슬라이더의 소수 값 [#1075](https://github.com/decentraland/creator-hub/pull/1075).

### 0.32.1 (2026년 1월 21일)

* **새 작업**: 플레이어 고정, 플레이어 고정 해제, 항목 바라보기, 충돌 변경, 스카이박스 변경, 스카이박스 재설정, 그리고 "카메라 변경"용 카메라 스마트 아이템. [#997](https://github.com/decentraland/creator-hub/pull/997), [#989](https://github.com/decentraland/creator-hub/pull/989)
* **여러 상태를 한 번에 변경** 여러 항목을 선택하여. [#975](https://github.com/decentraland/creator-hub/pull/975)
* **재게시 바로가기**: 구성 메뉴를 거치지 않고 동일한 World 또는 좌표에 다시 배포합니다. [#1003](https://github.com/decentraland/creator-hub/pull/1003)
* **새 스크립트형 스마트 아이템**: Clap Meter, Wearables Scanner, Zombie 및 Photo Mural. Padlock은 다른 엔티티의 "열기"를 트리거할 수 있습니다. [#1008](https://github.com/decentraland/creator-hub/pull/1008)
* **`ActionRef` 및 `ActionCallback` 입력 유형** 스크립트에서 코드로 다른 항목의 작업을 처리하기 위해. [#1054](https://github.com/decentraland/creator-hub/pull/1054)
* **툴팁** 구성 요소, 작업 및 스마트 아이템 전반의 툴팁. [#1009](https://github.com/decentraland/creator-hub/pull/1009)
* **프로젝트 관리 페이지.** [#1032](https://github.com/decentraland/creator-hub/pull/1032)
* **수정 사항**: 비디오 화면 모델이 양면으로 표시됩니다 [#1007](https://github.com/decentraland/creator-hub/pull/1007); 씬 메트릭이 삼각형, 재질, 텍스처를 올바르게 계산합니다 [#1020](https://github.com/decentraland/creator-hub/pull/1020).

### 0.32.0 (2026년 1월 5일)

* **여러 엔티티의 구성 요소를 한 번에 편집**: Tag, Material, AudioSource, AudioStream, MeshCollider, TextShape, Visibility 및 LightSource. [#949](https://github.com/decentraland/creator-hub/pull/949), [#950](https://github.com/decentraland/creator-hub/pull/950), [#958](https://github.com/decentraland/creator-hub/pull/958), [#959](https://github.com/decentraland/creator-hub/pull/959), [#960](https://github.com/decentraland/creator-hub/pull/960), [#963](https://github.com/decentraland/creator-hub/pull/963), [#964](https://github.com/decentraland/creator-hub/pull/964)
* **제작자 필드** 엔티티 검사기에서. [#944](https://github.com/decentraland/creator-hub/pull/944)
* **새 앱 설정 UI.** [#988](https://github.com/decentraland/creator-hub/pull/988)
* **사용하지 않는 자산 정리**: 전체 선택, 중간 상태 및 실행 취소. [#993](https://github.com/decentraland/creator-hub/pull/993)
* **더 많은 구성 요소** 편집기에서 사용할 수 있습니다. [#999](https://github.com/decentraland/creator-hub/pull/999)
* **수정 사항**: 숨겨진 항목은 계속 보이지 않음 [#974](https://github.com/decentraland/creator-hub/pull/974); 투명한 재질 [#976](https://github.com/decentraland/creator-hub/pull/976); 축 표시기가 북쪽을 표시합니다 [#978](https://github.com/decentraland/creator-hub/pull/978); 만료된 ID로 배포 [#981](https://github.com/decentraland/creator-hub/pull/981); 게시 모달이 더 이상 배경 클릭으로 닫히지 않습니다 [#982](https://github.com/decentraland/creator-hub/pull/982); 배포 오류 표시 [#985](https://github.com/decentraland/creator-hub/pull/985); 스마트 아이템 드롭다운 겹침 [#995](https://github.com/decentraland/creator-hub/pull/995); 실행 취소 시 더 이상 삭제할 수 없는 템플릿이 생성되지 않습니다 [#998](https://github.com/decentraland/creator-hub/pull/998); 위치 기즈모 성능 [#972](https://github.com/decentraland/creator-hub/pull/972).

### 0.31.0 및 0.31.1 (2025년 12월 16일)

* **평면 위치 기즈모.** [#943](https://github.com/decentraland/creator-hub/pull/943)
* **카메라 팬** 그리고 **축 도우미** 뷰포트에서. [#952](https://github.com/decentraland/creator-hub/pull/952), [#951](https://github.com/decentraland/creator-hub/pull/951)
* **DCL Cast** 씬으로 비디오를 스트리밍하기 위한 스마트 아이템. [#957](https://github.com/decentraland/creator-hub/pull/957)
* **새 작업** 스마트 아이템에서. [#955](https://github.com/decentraland/creator-hub/pull/955)
* **빈 영역을 클릭하여 선택 해제합니다.** [#961](https://github.com/decentraland/creator-hub/pull/961)
* **"링크가 복사되었습니다" 피드백** 공유 버튼에. [#965](https://github.com/decentraland/creator-hub/pull/965)
* **수정 사항**: 여러 파일 가져오기 [#931](https://github.com/decentraland/creator-hub/pull/931); 여러 객체를 스케일링할 때의 최종 위치 [#962](https://github.com/decentraland/creator-hub/pull/962).

### 0.30.0 (2025년 12월 9일)

* **스크립트 구성 요소**: 모든 항목에 사용자 지정 코드를 첨부합니다. 참조 [Script 컴포넌트 사용](/docs/creator/content-creator-ko/scene-editor/extend-with-code/script-component/). [#886](https://github.com/decentraland/creator-hub/pull/886)
* **F를 누르세요** 선택한 엔티티에 카메라를 맞추세요. [#948](https://github.com/decentraland/creator-hub/pull/948)
* **씬에 사용자 지정 코드가 있을 때 경고.** [#903](https://github.com/decentraland/creator-hub/pull/903)
* **사용자 지정 자산 탭에 튜토리얼 표시** 비어 있을 때. [#925](https://github.com/decentraland/creator-hub/pull/925)
* **수정 사항**: 여러 항목을 함께 회전 [#946](https://github.com/decentraland/creator-hub/pull/946); 씬, 카메라 및 플레이어 엔티티에서 구성 요소 추가가 숨겨짐 [#923](https://github.com/decentraland/creator-hub/pull/923); 씬 로드 실패 시 더 명확한 오류 [#921](https://github.com/decentraland/creator-hub/pull/921); Windows가 런처 다운로드를 안내합니다 [#899](https://github.com/decentraland/creator-hub/pull/899); 모든 복제본이 계속 선택된 상태로 유지됩니다 [#929](https://github.com/decentraland/creator-hub/pull/929); 비디오 플레이어 자리표시자 방향 [#909](https://github.com/decentraland/creator-hub/pull/909); 복제본 번호 매기기 [#933](https://github.com/decentraland/creator-hub/pull/933); 누락된 경우 Custom Items 폴더가 생성됩니다 [#940](https://github.com/decentraland/creator-hub/pull/940); 사용하지 않는 자산 정리가 빈 폴더를 제거합니다 [#924](https://github.com/decentraland/creator-hub/pull/924); 사용자 지정 코드가 있는 씬 게시 [#941](https://github.com/decentraland/creator-hub/pull/941); 스마트 아이템 기본 보기의 체크박스 필드 [#922](https://github.com/decentraland/creator-hub/pull/922).

### 0.29.3 (2025년 11월 20일)

* **수정 사항**: 새로 가져온 씬이 더 이상 로딩에 멈추지 않습니다 [#908](https://github.com/decentraland/creator-hub/pull/908); 더 이상 Finder 또는 Explorer 창이 여러 개 열리지 않습니다 [#906](https://github.com/decentraland/creator-hub/pull/906); 배포 흐름 [#896](https://github.com/decentraland/creator-hub/pull/896); 비디오 화면 스마트 아이템 [#898](https://github.com/decentraland/creator-hub/pull/898).

### 0.29.1 및 0.29.2 (2025년 11월 13일)

* **수정 사항**: 비디오 플레이어 렌더링과 구성 요소 목록에서의 항목을 드래그 앤 드롭으로 처리 [#872](https://github.com/decentraland/creator-hub/pull/872), [#873](https://github.com/decentraland/creator-hub/pull/873); NFT 스마트 아이템 [#885](https://github.com/decentraland/creator-hub/pull/885); 사용하지 않는 자산 정리가 더 이상 모델이나 썸네일을 손상시키지 않습니다 [#887](https://github.com/decentraland/creator-hub/pull/887), [#882](https://github.com/decentraland/creator-hub/pull/882); 의존성 업데이트 중 편집기가 회색으로 표시됩니다 [#891](https://github.com/decentraland/creator-hub/pull/891).

### 0.29.0 (2025년 11월 5일)

* **사용하지 않는 자산 정리** 도구. [#857](https://github.com/decentraland/creator-hub/pull/857)
* **씬 도움말 패널**: 씬의 마크다운 도움말 파일이 편집기에 표시되며, 템플릿에 유용합니다. [#833](https://github.com/decentraland/creator-hub/pull/833)
* **씬 폴더를 여는 바로가기** Finder 또는 Explorer에서. [#853](https://github.com/decentraland/creator-hub/pull/853)
* **수정 사항**: 템플릿 다운로드 [#845](https://github.com/decentraland/creator-hub/pull/845); NFT 스마트 아이템 계약 필드 [#839](https://github.com/decentraland/creator-hub/pull/839); 카메라 및 플레이어 엔티티는 부모로 설정할 수 없습니다 [#855](https://github.com/decentraland/creator-hub/pull/855).

### 0.28.0 (2025년 10월 23일)

* **조명 및 노드 수정자** 편집기에서. 참조 [조명](/docs/creator/content-creator-ko/sdk7/3d/lights/). [#813](https://github.com/decentraland/creator-hub/pull/813)
* **수정**: 복제된 항목을 이동할 수 있습니다. [#815](https://github.com/decentraland/creator-hub/pull/815)

### 0.27.5 (2025년 10월 15일)

* **엔티티 태깅.** [#790](https://github.com/decentraland/creator-hub/pull/790)
* **AI 컨텍스트 파일** 생성되거나 업데이트될 때 각 씬으로 다운로드되어, 코딩 도우미가 SDK를 알 수 있습니다. [#817](https://github.com/decentraland/creator-hub/pull/817)
* **이벤트 제출** More 섹션의 카드. [#811](https://github.com/decentraland/creator-hub/pull/811)
* **수정 사항**: 애니메이션 가중치가 소수를 허용합니다 [#826](https://github.com/decentraland/creator-hub/pull/826); 사운드 및 모델 썸네일 [#822](https://github.com/decentraland/creator-hub/pull/822), [#804](https://github.com/decentraland/creator-hub/pull/804); 빈 미리보기 디버거 [#821](https://github.com/decentraland/creator-hub/pull/821); 가져오기 후 자산 새로고침 [#809](https://github.com/decentraland/creator-hub/pull/809); 링크 서버를 통해 배포 [#812](https://github.com/decentraland/creator-hub/pull/812).

### 0.25.3 (2025년 10월 6일)

* **Chrome DevTools** Creator Hub에서 씬 디버깅용으로 사용할 수 있습니다. [#766](https://github.com/decentraland/creator-hub/pull/766)
* **수정**: 새 씬 생성. [#794](https://github.com/decentraland/creator-hub/pull/794)

### 0.24.1 (2025년 9월 17일)

* **기본 코드 편집기 설정**: VS Code, Cursor 또는 설치된 다른 편집기를 선택하세요. [#751](https://github.com/decentraland/creator-hub/pull/751)
* **디스크에서 파일 삭제** 프로젝트 삭제 시 선택 사항으로. [#787](https://github.com/decentraland/creator-hub/pull/787)
* **수정 사항**: 텍스처를 제거하고 URL에서 불러올 수 있습니다 [#772](https://github.com/decentraland/creator-hub/pull/772), [#779](https://github.com/decentraland/creator-hub/pull/779); 엔티티 삭제 시 캔버스 크기 조정 [#770](https://github.com/decentraland/creator-hub/pull/770).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/api/releases.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
