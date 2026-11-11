> 프로젝트를 어떻게 게시하나요?

# 게시

## 시작하기 전에

다음 사항을 확인하세요:

* 당신의 씬이 다음의 모든 [씬 제한 사항](/docs/creator/content-creator-ko/sdk7/optimizing/scene-limitations/)을 준수합니다. 이러한 사항의 대부분은 씬 미리보기를 실행할 때마다 검증됩니다.
* 씬이 모바일 플레이어에게도 잘 동작하길 원한다면, 먼저 실제 기기에서 미리보기와 테스트를 해보세요 — 다음을 참고하세요 [모바일용 빌드](/docs/creator/content-creator-ko/build-for-mobile/mobile-client/overview/) 및 [모바일에서 미리보기](/docs/creator/content-creator-ko/build-for-mobile/develop/preview-on-mobile/). 한 번 게시되면, 당신의 씬은 데센트럴랜드 모바일 앱에서도 데스크톱과 동일한 방식으로 접근할 수 있습니다.
* 다음이 있습니다. [메타마스크](https://metamask.io/) 계정에 당신의 LAND 파셀이 할당되어 있습니다.
* 필요한 수의 인접한 LAND 파셀 또는 Decentraland NAME을 소유하고 있어야 합니다. 그렇지 않으면 다음에서 LAND를 구매할 수 있습니다. [마켓](https://market.decentraland.org) 또는 NAME을 다음에서 [마켓플레이스](https://decentraland.org/shop/items?category=names).


> [!WARNING]
> \*\*📔 참고\*\*: 여러 파셀로 이루어진 씬은 인접한 파셀에만 배포할 수 있습니다.



> [!CAUTION]
> **❗경고**: 라이브 이벤트를 계획할 때는 이벤트 직전에 씬을 마지막 순간에 변경하지 마세요. 이벤트가 시작되기 최소 1시간 전에는 최종 버전을 게시하세요.
> 
> 게시할 때마다 내부 프로세스가 렌더링되기 전에 모든 3D 모델을 최적화합니다. 보통 몇 초밖에 걸리지 않지만, 씬이 매우 크거나 서버가 바쁠 때는 더 오래 걸릴 수 있습니다. 완료되기 전까지는 플레이어에게 이전 버전의 씬이 제공됩니다.


## 씬 데이터 확인

배포할 때 CLI는 *scene.json* 파일에서 정보를 읽어 씬을 어디에 배포할지 결정합니다.

씬의 *scene.json* 파일을 열고 다음 데이터를 완성하세요:

* **제목**: 제목은 플레이어가 씬에 들어올 때마다 미니맵 아래 UI에 표시됩니다. 또한 텔레포트 팝업에도 표시됩니다.
* **설명**: 플레이어가 당신의 씬에서 무엇을 발견하게 될지에 대한 설명입니다. 이것은 텔레포트 팝업에 표시됩니다.
* **navmapThumbnail**: 당신의 씬을 나타내는 이미지입니다. 이 이미지는 텔레포트 팝업에 표시됩니다. 이미지는 *.png* 또는 *.jpg* 형식이어야 하며, 권장 크기는 *1920x1080* 픽셀입니다. 중요한 내용은 중앙 *1080x1080* 정사각형 안에 유지하세요. 플랫폼의 일부 영역에서는 이 이미지의 정사각형 잘라낸 버전을 표시하기 때문입니다. 다음을 참고하세요 [씬 썸네일](/docs/creator/content-creator-ko/sdk7/kinds-of-projects/scene-metadata/#scene-thumbnail).
* **파셀**: 씬이 차지하게 될 파셀들의 좌표
* **기준점**: 씬의 \[0,0] 좌표로 간주될 파셀의 좌표입니다. 씬이 여러 파셀로 구성된 경우, 가장 아래쪽 왼쪽(남서쪽) 파셀이어야 합니다.
* **spawnPoints**: 플레이어가 생성되는 씬 내부의 좌표 집합(씬의 기준 파셀을 기준으로 한 상대 좌표)입니다. 기본적으로 플레이어는 *0,0,0* 씬의 위치(왼쪽 아래 모서리)에서 생성됩니다. 플레이어가 특정 위치에서 시작하도록 하거나, 처음 나타날 때 서로 겹치지 않도록 영역을 설정하는 데 사용할 수 있습니다.
* **태그**: 당신의 씬을 설명하는 태그 집합입니다. 이것은 플레이어가 관심 있는 콘텐츠를 더 쉽게 찾을 수 있게 해 Decentraland 탐색을 돕습니다. [사용 가능한 카테고리 목록 보기](/docs/creator/content-creator-ko/sdk7/kinds-of-projects/scene-metadata/#categories).
* **등급**: 이것은 씬의 콘텐츠를 분류하는 데 사용됩니다. Decentraland는 18세 이상 플랫폼이므로, 설정할 값은 `A` 성인용입니다. 다음을 참고하세요 [연령 등급](/docs/creator/content-creator-ko/sdk7/kinds-of-projects/scene-metadata/#age-rating).


> [!WARNING]
> **📔 참고**: 다음을 참고하세요 [씬 메타데이터](/docs/creator/content-creator-ko/sdk7/kinds-of-projects/scene-metadata/) 에서 이러한 매개변수를 설정하는 방법에 대한 자세한 내용을 확인하세요.


## 씬을 게시하려면

### Creator Hub의 씬 편집기 사용하기

Creator Hub의 씬 편집기를 사용하면 씬을 쉽게 게시할 수 있습니다. 다음을 확인하세요 [Creator Hub를 설치했는지](/docs/creator/content-creator-ko/scene-editor/get-started/editor-installation/).

1. 씬 프로젝트를 여세요.
2. 다음을 클릭하세요. **게시** 오른쪽 상단 모서리의 버튼.
3. 프롬프트가 나타나 게시할지 묻습니다. **WORLD** 또는 **LAND**.
   * 선택하세요 **PUBLISH TO WORLD** 를 선택하면 당신의 [WORLDs](/docs/creator/content-creator-ko/sdk7/publishing/publishing-options/#decentraland-worlds)중 하나에서 씬을 사용할 수 있게 됩니다. 그런 다음 게시할 NAME 또는 ENS 도메인을 선택하세요.
   * 선택하세요 **PUBLISH TO LAND** 를 선택하세요. LAND를 소유하고 있거나 소유자로부터 배포 권한을 부여받은 경우입니다. 그런 다음 지도에서 배포할 파셀을 선택하세요. 배포가 허용된 파셀은 분홍색으로 표시됩니다.
   * 선택하세요 **대체 서버** 다음에 게시하려면 [커스텀 서버](#custom-servers).

![](/docs/_remote/75db93f7a2fdd17bec3b4907.webp)

4. 그러면 업로드 세부 정보를 보여주는 새 브라우저 탭이 열립니다. 지갑으로 배포에 서명하세요. 이는 소유권을 증명하기 위한 서명일 뿐이며, 무료이고 어떤 블록체인 거래도 포함하지 않습니다.
   * 메타마스크 브라우저 계정의 LAND인 경우, 배포를 확인하세요. 그런 다음 메타마스크 브라우저 확장 프로그램에서 메시지에 서명하세요.
   * Wallet Connect로 사용할 수 있는 지갑에 연결된 LAND인 경우, **지갑 연결**을 클릭한 뒤 모바일 기기로 QR 코드를 스캔하고 Wallet Connect의 단계를 따르세요.

### CLI를 통해

1. Decentraland에서 파셀에 연결된 동일한 공개 주소로 메타마스크 계정에 로그인하세요.
2. 실행하세요 `npm run deploy` 를 씬의 폴더에서.


> [!NOTE]
> \*\*💡 팁\*\*: 프로젝트 폴더에 배포하고 싶지 않은 파일이 있다면, 배포 전에 \_.dclignore\_ 파일에 그 파일들을 나열하세요.


3. 브라우저 탭이 열리고, 배포할 파셀을 보여줍니다. 다음을 클릭하세요. **서명 및 배포**.
4. 메타마스크가 열리며 서명이 요청되었다고 알려줍니다. 다음을 클릭하세요. **서명** 하여 이 작업을 확인하세요.


> [!NOTE]
> **💡 팁**: 씬의 변경 사항이 자동으로 배포되는 지속적 통합 흐름을 구현 중이라면, `export DCL_PRIVATE_KEY` 환경 변수를 배포 권한이 있는 계정의 개인 키로 설정할 수 있습니다.



> [!NOTE]
> **💡 팁**: `npm run deploy` 를 실행하면 `npm run build`가 실행되며, 이는 다음을 실행할 때보다 타입 오류를 더 엄격하게 검사합니다. `npm run start`. 이러한 오류를 피할 수 없고(예: 외부 라이브러리에서 발생) 씬에 영향을 주지 않는다면, `npm run deploy -- --skip-build` 를 사용하여 `npm run build` 단계를 건너뛰고 씬을 있는 그대로 배포할 수 있습니다.


## Worlds에 게시하기

씬을 Decentraland World에 게시하려면 Decentraland NAME 또는 ENS 도메인을 소유해야 합니다. 다음을 참고하세요 [게시 옵션](/docs/creator/content-creator-ko/sdk7/publishing/publishing-options/#decentraland-worlds) 에서 이를 얻는 방법에 대한 정보를 확인하세요.

### scene.json 설정하기

어떤 **이름** 으로 배포할지 지정해야 합니다. 다음 섹션을 `scene.json`:

```json
{
	"worldConfiguration": {
		"name": "my-name.dcl.eth"
	}
}
```

의 **이름** 에 지정된 `scene.json` 값은 Decentraland NAME 또는 ENS 도메인일 수 있습니다. 이 값은 배포에 서명하는 지갑이 소유하고 있거나, 액세스 제어 목록(ACL)을 통해 권한을 부여받은 지갑이 소유해야 합니다.

다음 사항을 염두에 두세요:

* 배포에 서명하는 지갑은 `scene.json` 파일에 지정된 NAME을 소유해야 합니다
* 씬에는 파셀 제한이 없습니다(2023년 1월부터)
* 모든 World는 아래에 자세히 설명된 대로 제외하지 않는 한 자동으로 Places 페이지에 등록됩니다

### World 메타데이터

World에는 게시된 씬의 메타데이터와는 별개로 자체적인 이름, 설명, 썸네일이 있습니다. 비어 있는 World에 게시하거나 단일 씬이 포함된 World에 게시할 때, World의 메타데이터는 게시할 때마다 씬의 메타데이터와 일치하도록 업데이트됩니다. World에 둘 이상의 씬이 포함된 경우, 메타데이터는 독립적입니다. 씬을 게시해도 변경되지 않으며, Creator Hub의 **관리** 섹션에서만 편집할 수 있습니다. 다음을 참고하세요 [World 메타데이터 vs 씬 메타데이터](/docs/creator/content-creator-ko/scene-editor/publish/publish-scene/#world-metadata-vs-scene-metadata).

### World 저장 공간 예산

World에 배포된 씬은 지갑이 소유한 모든 World에 공유되는 저장 공간 예산에 포함됩니다. 이 예산은 당신의 Decentraland 보유량에 따라 증가합니다:

* 소유한 각 Decentraland NAME마다 100MB가 부여됩니다.
* 소유한 각 LAND 파셀마다 추가로 100MB가 부여됩니다.
* 지갑에 보유한 MANA 2,000개마다 추가로 100MB가 부여됩니다.

이 예산은 원하는 대로 각 World에 자유롭게 분배할 수 있습니다. 예를 들어, 500MB 예산으로 300MB 씬 하나와 200MB 씬 하나를 배포하거나, 100MB 씬 다섯 개를 배포할 수 있습니다. ENS 도메인에 게시된 World는 이 예산을 사용하지 않으며, 대신 World당 36MB의 고정 한도가 적용됩니다.

예산의 사용량과 남은 양은 두 곳에서 확인할 수 있습니다:

* 의 **관리** Creator Hub의 섹션에서 **세부 정보 보기** 를 클릭하면 MANA, LAND, NAME 보유량이 어떻게 합산되는지 세부 내역을 볼 수 있습니다.
* 의 **Worlds** 탭의 [Builder](https://decentraland.org/builder/worlds).

예산을 초과하면(예: 자산을 판매하거나 이전한 후) Worlds가 접근 불가능해지기 전에 공간을 확보하거나 예산을 늘릴 수 있도록 48시간의 시간이 주어집니다. 다음을 참고하세요 [World 크기 제한](/docs/creator/content-creator-ko/sdk7/kinds-of-projects/kinds-of-project/#size-limits) 에서 자세한 내용을 확인하세요.

### 여러 씬으로 이루어진 World

기본적으로 CLI를 통해 World에 배포할 때 씬은 단일 씬 World로 처리됩니다. World에 이미 씬이 있는 경우, 새 씬을 배포하면 좌표가 다르더라도 이전 씬이 덮어써집니다.

여러 씬으로 이루어진 World로 배포하려면 매개변수 `--multi-scene` 를 배포 명령에 추가하세요:

`npm run deploy -- --multi-scene --target-content https://worlds-content-server.decentraland.org`

이 매개변수를 사용할 때, 게시하는 씬의 좌표가 해당 World의 기존 씬과 다르다면, 이 씬들은 모두 함께 존재하게 됩니다.


> [!WARNING]
> \*\*📔 참고\*\*: 소유하지 않은 World에 대해 운영자 권한이 있고, 그 World의 특정 파셀에 대해서만 권한을 부여받은 경우, 해당 파셀에만 게시할 수 있으며 World의 다른 파셀의 콘텐츠에는 영향을 줄 수 없습니다.


### Places 목록에서 제외

Worlds가 Places에 색인되는 것을 제외하고 싶다면, 다음 섹션을 `scene.json`:

```json
{
	"worldConfiguration": {
		"name": "my-name.dcl.eth",
		"placesConfig": {
			"optOut": true
		}
	}
}
```

### Creator Hub의 씬 편집기 사용하기

1. 씬 프로젝트를 여세요.
2. 다음을 클릭하세요. **게시** 오른쪽 상단 모서리의 버튼.
3. 선택하세요 **PUBLISH TO WORLD**.
4. 게시할 NAME 또는 ENS 도메인을 선택하세요.

### CLI를 통해

다음 명령을 사용하세요:

```bash
npm run deploy -- --target-content https://worlds-content-server.decentraland.org
```

명령을 실행하면 지갑으로 배포에 서명하라는 안내가 나타나며, 씬을 허용하거나 거부하기 위해 일련의 검증이 실행됩니다.


> [!WARNING]
> **📔 참고**: 당신의 `scene.json` 에 `worldConfiguration` 섹션이 있으면, `npm run deploy` 에는 `--target-content` 플래그(또는 `--target`)가 필요합니다. 둘 중 어느 것도 없으면 명령은 다음 오류로 중단됩니다. `DEPLOY_MISSING_TARGET_CONTENT`.


### World 접근하기

씬이 Worlds 서버에 업로드되면 다음 URL을 사용하여 Decentraland Explorer에서 접근할 수 있습니다: `decentraland://?realm=NAME.dcl.eth`, 여기서 `NAME` 은 배포한 Decentraland NAME 또는 ENS 도메인으로 바꾸면 됩니다.

Decentraland가 이미 열려 있다면, 채팅 상자에 다음을 입력하여 World로 바로 이동할 수도 있습니다. `/goto NAME.dcl.eth` 명령.

다음을 참고하세요 [탐색 가능하게 만들기](/docs/creator/content-creator-ko/sdk7/publishing/make-discoverable/) 에서 World를 탐색 가능하게 만드는 방법에 대한 자세한 내용을 확인하세요.

### World를 Genesis City로 이전하기

LAND 소유자이고 World 씬을 Genesis City에 배포하고 싶다면, Genesis City의 콘텐츠 서버인 분산 Catalyst 네트워크에 씬을 다시 배포하면 됩니다.

기억할 사항:

* 다음을 제거하세요. `worldConfiguration` 섹션을 `scene.json`
* World의 크기 제한(보유량에 따라 동적으로 결정됨)은 LAND 파셀의 제한(파셀당 15MB)과 다르므로, Genesis City에 배포할 때 씬의 크기가 올바른지 확인하세요!

## 대형 씬 게시하기

대형 씬은 배치 단위로 업로드되므로, 약 200MB보다 큰 씬도 Genesis City 또는 World에 게시할 수 있습니다. 이를 위해서는 프로젝트에 `@dcl/sdk` 7.31.0 이상이 필요합니다. Creator Hub는 프로젝트에 설치된 SDK로 게시하므로, Creator Hub에서 게시할 때도 동일하게 적용됩니다.

업로드 방식:

* **총 50 MiB 미만** (약 52MB)의 파일: 이전과 마찬가지로 씬이 단일 요청으로 업로드됩니다.
* **50 MiB 이상**: 파일이 32 MiB 단위로, 한 번에 세 배치씩 업로드됩니다. 파일은 절대 배치 간에 분할되지 않습니다. 하나의 파일이 32 MiB보다 크면, 그 파일에 맞게 배치 크기가 커집니다.
* 각 배치 후 CLI는 다음과 같은 진행 상황 줄을 출력합니다. `268.8 MB 중 96.0 MB 업로드됨(배치 3/9)`.

배치 업로드는 크기 제한을 바꾸지 않습니다. 각 파일은 여전히 50MB 미만이어야 하며, 씬 전체는 해당 파셀들의 [총 파일 크기 제한](/docs/creator/content-creator-ko/sdk7/optimizing/scene-limitations/) 또는 당신의 [World 저장 공간 예산](#worlds-storage-budget).


> [!WARNING]
> **📔 참고**: 배포하는 서버가 배치 업로드를 지원하지 않는 경우(예: [커스텀 서버](#custom-servers)경우), CLI는 서버가 누락한 파일을 확인합니다. 그 합계가 200MB 이하이면 경고를 표시합니다. `대상 서버는 배치 업로드를 지원하지 않으며, 단일 요청으로 업로드합니다.` 그리고 이를 한 번의 요청으로 업로드합니다. 합계가 더 크면 배포는 다음 오류로 실패합니다. `DEPLOY_PARTIAL_NOT_SUPPORTED`. 콘텐츠 서버 앞의 인프라에서 시간 초과가 발생하므로 단일 요청은 200MB를 초과할 수 없습니다.


### 배포 취소하기

다음을 실행할 때 열리는 브라우저 탭은 `npm run deploy` 업로드 진행 상황을 보여주며, **취소** 버튼이 있습니다. 취소하면 업로드가 중지됩니다. 취소가 도착할 때 서버가 이미 씬을 게시했다면, 배포는 게시된 것으로 종료됩니다.


> [!CAUTION]
> **❗경고**: World에 다음 없이 게시할 때 `--multi-scene`그리고 World에 다른 파셀의 씬이 있는 경우, CLI는 당신의 씬을 업로드하기 전에 그 씬들을 제거합니다(다음을 참고하세요 [여러 씬으로 이루어진 World](#multi-scene-worlds)). 제거된 후에 취소하면 World가 비어 있을 수 있으며, CLI는 다음 오류로 종료됩니다. `DEPLOY_CANCELLED_AFTER_UNDEPLOY`. 씬을 게시하려면 다시 배포하세요.


CLI가 서버가 씬을 게시했는지 확인할 수 없으면, 다음 오류로 종료됩니다. `DEPLOY_CANCEL_OUTCOME_UNCERTAIN`. 다시 배포하기 전에 씬을 확인하세요.

브라우저 탭 없이 배포를 실행하는 도구는 토큰을 사용하여 배포 API를 통해 이를 취소할 수 있습니다. CLI는 실행할 때마다 임의의 토큰을 생성합니다. 직접 설정하려면 다음을 전달하세요. `--deploy-token` 최소 16자 값을 함께:

```bash
npm run deploy -- --deploy-token my-own-deploy-token-0001
```

다음을 사용할 때 `--programmatic`명령 결과에는 이 토큰이 다음으로 포함됩니다. `deployToken`. 다음을 실행하세요. `npx sdk-commands deploy --help` 에서 배포 API 경로를 확인하세요.

### 배포 오류 코드

| 오류 코드                             | 의미                                                                                 |
| --------------------------------- | ---------------------------------------------------------------------------------- |
| `DEPLOY_PARTIAL_NOT_SUPPORTED`    | 서버가 배치 업로드를 지원하지 않으며, 누락된 파일의 총합이 200MB를 초과합니다.                                    |
| `DEPLOY_CONTENT_CHECK_FAILED`     | CLI가 서버가 이미 보유한 파일을 확인할 수 없었습니다. 이 검사는 서버가 배치 업로드를 지원하지 않고 씬이 200MB를 초과할 때만 실행됩니다. |
| `DEPLOY_CANCELLED`                | 배포가 취소되었으며, 씬은 게시되지 않았습니다.                                                         |
| `DEPLOY_CANCELLED_AFTER_UNDEPLOY` | 배포가 World의 이전 씬이 제거된 후에 취소되었습니다.                                                   |
| `DEPLOY_CANCEL_OUTCOME_UNCERTAIN` | 배포가 취소되었지만, CLI는 서버가 이를 게시했는지 확인할 수 없었습니다.                                         |
| `DEPLOY_MISSING_TARGET_CONTENT`   | 씬에는 `worldConfiguration` 가 있지만 명령에는 `--target-content` 또는 `--target`.              |

## 하드웨어 지갑에서 게시하기

LAND 토큰을 메타마스크 계정에 보관하는 대신, [Ledger](https://www.ledger.com/) 또는 [Trezor](https://trezor.io/)와 같은 물리적으로 컴퓨터에 연결된 하드웨어 지갑 장치에 보관하는 것이 더 안전할 수 있습니다.

이러한 장치를 사용하는 경우, 토큰을 더 안전하게 보관하면서 메시지 서명을 가능하게 하기 위해 하드웨어 지갑을 메타마스크에 연결할 수 있습니다. 다음을 참고하세요 [메타마스크의 이 글](https://metamask.zendesk.com/hc/en-us/articles/360020394612-How-to-connect-a-Trezor-or-Ledger-Hardware-Wallet) 에서 계정 연결 방법을 확인하세요.

하드웨어 지갑을 메타마스크를 통해 사용할 수 있게 되면, 토큰이 메타마스크 계정에 있는 것과 동일한 단계로 배포할 수 있습니다.

## 씬 덮어쓰기

새 씬이 배포되면, 해당 씬이 차지하는 파셀에 존재하던 이전 콘텐츠를 덮어씁니다.

여러 파셀을 차지하는 씬이 다른 씬에 의해 부분적으로만 덮어써지더라도, 그 씬의 모든 파셀은 덮어써지거나 지워집니다.

당신이 씬을 *A* 두 개의 파셀 위에 *\[100, 100]* 및 *\[100, 101]*&#xBC30;포했다고 가정해 보세요. 그런 다음 파셀 *\[100, 101]* 을 인접한 땅을 소유한 사용자에게 판매하고, 그 사용자가 큰 씬(*B*)를 여러 파셀에 배포하며 그 안에 *\[100, 101]*.

당신의 씬 *A* 은 한 파셀만으로 부분 렌더링될 수 없으므로, *\[100, 100]* 은 어떤 콘텐츠도 표시하지 않습니다. 새 버전의 씬 *A* 을 만들어 한 파셀만 차지하도록 하고, 그것을 오직 파셀 *\[100, 100]*.

## 콘텐츠 서버란 무엇인가

콘텐츠 서버는 커뮤니티가 소유한 서버들의 네트워크로, 파일시스템이 콘텐츠 주소 지정 방식으로 구성되어 있습니다. 즉, 각 파일은 임의의 파일 이름이 아니라 그 내용으로 식별됩니다.

우리는 콘텐츠 서버를 사용해 BitTorrent와 유사한 방식으로 모든 씬 콘텐츠를 호스팅하고 배포하며, Decentraland 네트워크를 분산된 상태로 유지합니다.

1. 콘텐츠 서버는 씬을 렌더링하는 데 필요한 모든 자산을 저장하고 배포합니다.
2. 의 `npm run deploy` 명령은 이러한 자산을 **scene.json** 파일에 지정된 LAND 파셀에 연결합니다. 씬을 다시 배포할 때마다 콘텐츠 서버는 기록을 업데이트하여 파셀이 가장 최근 콘텐츠를 가리키도록 합니다. 배포에는 어떤 블록체인 거래도 포함되지 않으며, 소유권을 증명하기 위해 지갑으로 새 씬 엔티티에 서명하기만 하면 됩니다.

각 서버 사본의 정보는 검증 가능하며, 각 씬은 LAND 소유자의 해시로 서명됩니다. 즉, 서버 사본을 호스팅하는 사람이 콘텐츠를 조작하여 부적절한 내용을 표시할 수 없습니다. 또한 커뮤니티는 DAO를 사용하여 이러한 서버를 승인하거나 제거하는 투표를 할 수 있습니다.

## 커스텀 서버

공식 DAO가 관리하는 catalyst 서버 네트워크에 속하지 않는 커스텀 서버에 콘텐츠를 배포할 수 있습니다. 이를 위해 LAND나 NAME 토큰을 소유할 필요는 없습니다. 배포 가능 대상을 제어하기 위해 원하는 검증 로직을 서버에 구성할 수 있기 때문입니다. 커스텀 서버는 공식 서버의 콘텐츠를 포함하도록 선택할 수 있으며(이 콘텐츠는 덮어쓸 수 있음), 아니면 완전히 새 콘텐츠를 게시하도록 빈 상태에서 시작할 수도 있습니다.

다음을 참고하세요 [catalyst-owner 저장소](https://github.com/decentraland/catalyst-owner) 에서 자체 서버로 무엇을 할 수 있는지와 설정 방법에 대한 자세한 내용을 확인하세요.


> [!WARNING]
> **📔 참고**: 플레이어는 커스텀 서버에 접근하기 위해 URL을 직접 입력해야 합니다. 다음과 같은 서비스의 일부 검증은 [보상 서버](/docs/creator/content-creator-ko/rewards/getting-started/) 와 같은 맥락에서는 실패할 수 있습니다. 보통 이러한 서비스는 요청이 공식 서버에서 왔어야 하기 때문입니다.


플레이어는 이 서버로 직접 안내되지 않으며, 접근하려면 URL을 명시적으로 입력하는 것만 가능합니다.

### CLI를 통해

커스텀 서버에 배포하려면 다음을 실행하세요:

`npm run deploy -- --target <커스텀 서버 도메인>`

커스텀 서버에 배포한 씬에 들어가기 위한 URL은 호스팅되는 도메인에 따라 달라집니다.

## 배포 성공 확인

씬을 배포한 후에는, 이러한 변경 사항이 네트워크의 여러 콘텐츠 서버에 전파되기까지 몇 분이 걸릴 수 있습니다. 배포 직후 Decentraland에 들어가면 이전 버전의 콘텐츠가 잠시 보일 수 있습니다.

CLI로 배포하는 경우, 서명 dapp은 새 버전의 콘텐츠가 네트워크의 모든 서버에 전파되었다는 확인을 표시하기 시작합니다.

Decentraland의 콘텐츠 네트워크를 구성하는 각 서버 목록을 보게 됩니다. 각 서버에는 해당 파셀에 마지막으로 업로드된 변경 사항의 타임스탬프가 지정되어 있습니다. 각 서버는 서로 다른 realm을 나타냅니다. 이러한 서버 이름이 realm 이름에 어떻게 매핑되는지는 [catalyst monitor 화면](https://decentraland.github.io/catalyst-monitor/).

을 통해 확인할 수 있습니다. 전파는 전체 그림의 일부일 뿐입니다. 업로드 후 씬의 3D 모델은 각 플랫폼(Windows 및 Mac)에 맞게 최적화된 자산 번들로 변환됩니다. 그 변환이 완료되기 전까지는 플레이어에게 의도적으로 씬의 마지막 완전히 작동하는 버전이 제공됩니다. 보통 몇 초밖에 걸리지 않지만, 매우 큰 씬이거나 서버가 바쁠 때는 더 오래 걸릴 수 있습니다. 변환 진행 상황은 직접 조회할 수 있습니다. 다음을 참고하세요 [변환 상태를 확인하세요](/docs/creator/content-creator-ko/scene-editor/publish/publish-scene/#check-the-conversion-status).

다음 명령을 명령줄 콘솔에서 실행하면 언제든지 전파 정보를 얻을 수도 있습니다:

`npx @dcl/opscli pointer-consistency --pointer 0,0`


> [!WARNING]
> **📔 참고**: 대신 장면의 좌표를 사용하세요 `0,0`. 장면에 여러 구획이 있는 경우, 어느 구획이든 동일한 출력을 생성합니다. 좌표가 음수로 시작하면 `\` 좌표 시작 부분에 추가하여 `-` 문자가 명령줄에서 잘못 해석되는 것을 방지합니다.


## 자동 배포

장면 콘텐츠를 정기적으로 변경하고 간소화된 게시 파이프라인을 구현하고 싶다면, GitHub 액션을 통해 장면의 배포를 자동화할 수 있습니다.

이를 위해 먼저 장면을 GitHub 프로젝트에 저장해야 합니다. 그런 다음 다음과 같은 [GitHub 액션](https://docs.github.com/en/actions) 스크립트를 설정할 수 있으며, 이 스크립트는 다음으로 병합될 때마다 실행됩니다. `main` 브랜치입니다. 이 스크립트는 모든 종속 항목을 설치하고, 프로젝트를 빌드한 다음 Decentraland에 배포합니다.

```yaml
name: DCL PROD에 배포

on:
  push:
    branches:
      - main

env:
  DCL_PRIVATE_KEY: ${{ secrets.DCL_PRIVATE_KEY }}

jobs:
  deploy:
    name: 배포
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - name: npm 패키지 설치
        run: |
          npm install
      - name: 장면 빌드
        run: |
          npm run build
      - name: 장면 배포
        run: |
          npm run deploy
```

> 중요: 이 프로세스를 실행하려면 GitHub에서 지갑의 개인 키를 환경 변수로 설정해야 하며, 이는 배포 서명에 사용됩니다. 언제나 그렇듯 개인 키는 매우 안전하게 보관해야 합니다. 실제로 토지 토큰을 소유한 계정의 개인 키는 절대 사용하지 마세요. 그렇게 하면 심각한 보안 위험이 발생합니다. 대신 가치 있는 토큰을 보유하지 않은 일회용 지갑에 운영자 권한을 위임하세요. 이 개인 키가 유출되더라도, 해당 계정에서 그 운영자 권한을 쉽게 취소하고 새 지갑을 설정할 수 있습니다.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/publishing/publishing.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
