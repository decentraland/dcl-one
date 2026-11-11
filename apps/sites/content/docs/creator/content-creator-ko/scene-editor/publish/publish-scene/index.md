> LAND 또는 NAME에 씬을 게시하는 방법.

# 씬 게시하기

## 시작하기 전에

다음 사항을 확인하세요:

* 씬이 다음의 모든 사항을 준수하는지 [씬 제한 사항](/docs/creator/content-creator-ko/sdk7/optimizing/scene-limitations/). 이 중 대부분은 씬 미리보기를 실행할 때마다 검증됩니다.
* 다음이 있습니다 [메타마스크](https://metamask.io/) 계정이며, 해당 계정에 LAND 구획 또는 NAME이 할당되어 있어야 합니다.
* 필요한 수의 인접한 LAND 구획 또는 Decentraland NAME을 소유하고 있어야 합니다. 그렇지 않으면 다음에서 LAND를 구매할 수 있습니다 [마켓플레이스](https://decentraland.org/marketplace/) 또는 다음에서 NAME을 [마켓플레이스](https://decentraland.org/shop/items?category=names).


> [!WARNING]
> **📔 참고**: 멀티 구획 씬은 인접한 구획에만 배포할 수 있습니다.


다음을 확인하세요: [씬 세부 정보](/docs/creator/content-creator-ko/scene-editor/configure/scene-settings/#scene-details), 매력적인 이름, 설명, 썸네일, 카테고리 등을 제공하세요.


> [!CAUTION]
> **❗경고**: 라이브 이벤트를 계획할 때는 이벤트 직전에 씬을 마지막 순간에 변경하지 마세요. 최종 버전은 이벤트 시작 최소 1시간 전에 게시하세요.
> 
> 업데이트 전에 이미 현재 세션에서 씬을 불러온 플레이어는 계속 캐시된 버전을 보게 됩니다. 새 버전을 보려면 Decentraland를 종료했다가 다시 들어와야 합니다.


## 씬을 게시하세요

씬을 게시하려면:

1. Scene Editor에서 씬을 열고 다음을 클릭하세요 **게시**. 게시 세부 정보를 보여주는 창이 열립니다.
2. LAND에 게시할지 WORLD에 게시할지 선택하세요. 다음을 참조하세요 [프로젝트 유형](/docs/creator/content-creator-ko/sdk7/kinds-of-projects/kinds-of-project/) 다양한 옵션을 더 잘 이해하려면

![](/docs/_remote/75db93f7a2fdd17bec3b4907.webp)

3. LAND에 게시하는 경우, 지도에서 위치를 선택하세요. 게시 가능한 구획이 빨간색으로 표시됩니다. WORLD에 게시하는 경우, 게시 가능한 NAME이 드롭다운에 표시됩니다.


> [!NOTE]
> **💡 팁**: 구획이나 NAME이 보이지 않으면 올바른 사용자 계정으로 Creator Hub에 연결되어 있는지 확인하세요. 그렇지 않으면 프로젝트를 종료하고 오른쪽 상단의 사용자 설정 아이콘을 클릭한 다음 다음을 선택하세요 **로그아웃** 그리고 다시 로그인하세요.


4. 다음 화면에는 현재 업로드 중인 모든 파일과 해당 크기가 표시됩니다. 작업을 확인하세요.
5. 그러면 게시 프로세스가 시작됩니다. 이 과정에는 두 단계가 있으며, 두 단계가 모두 완료되면 다음 버튼이 표시됩니다 **들어가기** 버튼이 나타납니다.


> [!NOTE]
> **💡 팁**: 성공 화면에는 씬으로 연결되는 웹 링크도 표시되며, 이를 복사하는 버튼이 있습니다. 누구나 데스크톱 클라이언트 설치 여부와 관계없이 브라우저에서 해당 링크를 열 수 있으므로, Discord 메시지나 이벤트 페이지에 붙여넣을 링크입니다. 형식은 다음과 같습니다 `https://decentraland.org/jump?realm=<your-world-name>` WORLD의 경우, 또는 `https://decentraland.org/jump?position=<x>,<y>` Genesis City의 씬의 경우입니다.
> 
> 다음의 **들어가기** 버튼은 청중이 아니라 여러분을 위한 것입니다. 자신의 데스크톱 클라이언트에서 씬을 열어 작업 내용을 확인할 수 있습니다.


![](/docs/_remote/a76ebe12a85e91368d25d90d.webp)


> [!NOTE]
> **📔 참고**: 배포의 두 단계는 다음과 같습니다:
> 
> * **1. 업로드**: 파일을 서버에 업로드합니다. 50 MiB 이상인 씬은 일괄 업로드됩니다. 다음을 참조하세요 [대형 씬 게시](/docs/creator/content-creator-ko/sdk7/publishing/publishing/#publish-large-scenes).
> * **2. 변환**: 씬의 3D 모델이 더 빠른 렌더링을 위해 Asset Bundle로 압축됩니다. 보통 몇 초 정도 걸립니다. 아주 큰 씬은 더 오래 걸릴 수 있습니다.
> 
> Decentraland의 서버는 멀리서 씬을 그릴 때 사용하는 모델의 낮은 수준의 디테일(LOD) 버전도 생성합니다. 이 작업은 씬이 이미 라이브된 뒤 백그라운드에서 실행되므로, 게시가 이를 기다리지는 않습니다.


## 게시에는 얼마나 걸리나요?

* **테스트할 수 있을 때까지**: 몇 초에서 몇 분 정도 걸립니다. 씬의 총 크기와 업로드 대역폭에 따라 달라집니다. 다음 버튼은 **들어가기** 두 단계가 모두 완료되고 씬을 플레이할 수 있게 되면 바로 나타납니다.
* **라이브 이벤트 전에**: 예상치 못한 상황에 대비할 수 있도록 최종 버전은 이벤트 시작 최소 1시간 전에 게시하세요.

다음 요인으로 인해 게시가 더 느려질 수 있습니다:

* **씬의 총 크기**: 업로드와 변환 모두에 영향을 줍니다.
* **서로 다른 3D 모델의 수와 복잡성**: 모델은 하나씩 변환됩니다.
* **변환 서버의 혼잡도**: 씬이 비슷한 시기에 게시된 다른 씬 뒤에 대기열에 들어갈 수 있습니다.

## 월드 관리

Creator Hub에서는 다음을 통해 월드 관리를 할 수 있습니다 **관리** 탭입니다. 다음 **관리** 탭에서는 월드를 추적하고 편집할 수 있습니다. 여기에서 월드 설정, 권한, 씬을 편집할 수 있습니다.

### 월드 저장 공간 할당량

월드에 게시된 씬은 지갑이 소유한 모든 월드가 공유하는 저장 공간 할당량에 포함됩니다. 할당량은 보유량에 따라 늘어납니다. 소유한 각 Decentraland NAME 또는 LAND 구획마다 100 MB가 부여되며, 지갑에 보유한 2,000 MANA마다 추가로 100 MB가 부여됩니다. 대신 ENS 도메인에 게시된 월드는 고정된 36 MB 제한이 적용됩니다.

다음의 **관리** 탭에는 저장 공간 할당량 중 얼마나 사용 중인지 표시됩니다. 다음을 클릭하세요 **세부 정보 보기** MANA, LAND, NAME 보유량이 어떻게 합산되는지 확인할 수 있습니다.

<img src="/docs/_remote/6b3fe18d494c053a67e9cb4f.webp" alt="" width="300">

또한 남은 할당량은 다음의 **월드** 탭에서 [빌더](https://decentraland.org/builder/worlds).

다음을 참조하세요 [월드 크기 제한](/docs/creator/content-creator-ko/sdk7/kinds-of-projects/kinds-of-project/#size-limits) 할당량 계산 방식과 이를 초과할 경우 어떻게 되는지에 대한 자세한 내용은 여기에서 확인하세요.

### 월드 설정

월드 소유자는 원하는 월드로 들어가 **설정** 아래의 **관리** 패널에서 설정을 편집할 수 있으며, 또는 게시 과정 중 다음을 클릭해 접근할 수 있습니다 **설정** 만약 **멀티 씬 월드(고급)** 가 활성화되어 있으면.

<img src="/docs/_remote/c29f510065b494f433726156.webp" alt="" width="600">

* **세부 정보**: 월드의 일반 정보:
  * 월드 제목
  * 설명
  * 콘텐츠 등급
  * 카테고리

다음에 추가된 정보는 **세부 정보** 게시되면 Decentraland Places와 월드 내 월드 정보에 표시됩니다. 월드에 단일 씬이 있는 경우, 이 세부 정보는 매번 게시할 때 씬의 메타데이터로 덮어씌워진다는 점을 기억하세요. 다음을 참조하세요 [월드 메타데이터 vs 씬 메타데이터](#world-metadata-vs-scene-metadata) 아래를 참고하세요.

* **레이아웃**: 멀티 씬 월드에서만 접근할 수 있습니다. 월드에 게시된 모든 씬에 대한 정보를 포함합니다.
  * 점 세 개를 클릭하고 다음을 선택하여 개별 씬을 제거할 수 있습니다 **월드에서 제거**.
  * **월드 맵** 월드 레이아웃을 표시하고 콘텐츠가 있는 구획과 남은 빈 구획을 식별합니다.

<img src="/docs/_remote/f965a7cd250c7251c2c60acb.webp" alt="" width="600">

* **기타**: 기타 유용한 월드 구성:
  * 월드 스폰 좌표: 사용자가 월드 안에서 스폰될 구획(X,Y)을 설정합니다. 해당 구획에 있는 씬이 사용자가 스폰될 정확한 위치를 결정합니다(예: 구획 1,1이 월드 스폰이고, 1,1의 씬에는 스폰 지점이 1,0,1 **해당 씬 내부에**).
  * 스카이박스 설정


> [!NOTE]
> **📔 참고**: 월드 설정은 월드 소유자(NAME를 발행한 주소)만 접근할 수 있습니다. NAME을 얻는 방법에 대한 자세한 내용은 다음을 확인하세요 [Marketplace NAME 섹션](https://decentraland.org/marketplace/names/claim).


### 월드 메타데이터 vs 씬 메타데이터

월드와 그 안에 게시된 각 씬은 서로 별도의 두 메타데이터 집합을 가집니다. 둘 다 이름, 설명, 썸네일을 포함합니다:

* **씬 메타데이터**: 씬 프로젝트에 속하며, 다음 파일에 저장됩니다 `scene.json` 파일입니다. 다음을 통해 편집합니다 [씬 설정](/docs/creator/content-creator-ko/scene-editor/configure/scene-settings/) Scene Editor에서, 그리고 게시할 때마다 씬과 함께 업로드됩니다.
* **월드 메타데이터**: 월드 자체에 속합니다. 월드의 다음에서 편집합니다 **설정**설정 **관리** , Creator Hub의 탭에서. 이 정보는 Decentraland Places와 월드 내 월드 정보에 표시됩니다.

이 둘의 관계는 월드에 포함된 씬 수에 따라 달라집니다:

* **빈 월드**: 빈 월드에 씬을 게시하면 월드의 메타데이터는 씬의 메타데이터로 채워집니다.
* **단일 씬 월드**: 씬을 게시할 때마다 월드의 메타데이터가 씬의 메타데이터와 일치하도록 업데이트됩니다.
* **멀티 씬 월드**: 월드의 메타데이터는 각 씬의 메타데이터와 완전히 독립적입니다. 씬을 게시해도 월드의 메타데이터는 변경되지 않습니다. 이를 변경하는 유일한 방법은 월드의 **설정** 다음의 **관리** 탭.

실제로 월드에 단일 씬만 있다면, 월드의 메타데이터를 별도의 것으로 생각할 필요가 없습니다. 씬 설정을 편집하고 게시하기만 하면 월드에는 항상 그 내용이 반영됩니다.


> [!WARNING]
> **📔 참고**: 단일 씬 월드에서는 월드의 **설정** 다음의 **관리** 탭에서 변경한 내용은 다음에 씬을 게시할 때 씬의 메타데이터로 덮어씌워집니다. 변경 사항을 유지하려면 대신 씬 설정을 편집하세요.


멀티 씬 월드에서는 월드의 **설정** 다음의 **관리** 탭이 월드 메타데이터를 편집할 수 있는 유일한 곳입니다.

### 멀티 씬 월드

월드는 월드 소유자나 다른 제작자가 게시한 여러 씬을 가질 수 있습니다. 이를 통해 각 구획을 서로 다른 협업자가 관리할 수 있는 협업 환경을 만들 수 있습니다.

#### 월드를 멀티 씬으로 만들기

월드 소유자는 다음을 토글하여 월드를 멀티 씬으로 설정할 수 있습니다 **멀티 씬 월드(고급)** 단일 씬 월드에 게시할 때

<img src="/docs/_remote/9f653359d5337efaf125df54.webp" alt="" width="600">

멀티 씬 월드가 게시되면, 월드 소유자는 추가 씬을 게시하거나 협업자를 추가해 월드 안에 게시하도록 할 수 있습니다.


> [!NOTE]
> **📔 참고**: 멀티 씬 월드의 크기는 게시된 모든 씬을 포함하도록 자동으로 조정되며, 게시할 때마다 동적으로 커지고 작아집니다. 멀티 월드의 서로 다른 씬 사이에 남는 공간은 환경으로 채워집니다.


#### 멀티 씬 월드에 협업자 추가하기

다음의 **관리** 패널에서 월드 소유자는 월드의 **권한** 점 세 개를 클릭하여 접근할 수 있습니다. 월드 소유자는 다음 아래에서 협업자를 관리할 수 있습니다 **협업자** 탭.

<img src="/docs/_remote/2586c8bab86636ee8a2193fe.webp" alt="" width="600">

협업자는 모든 구획 또는 특정 사용자 지정 좌표에 대한 배포 권한을 가질 수 있습니다. 사용자 지정 좌표는 월드 설정의 맵과 유사한 대화형 월드 맵을 통해 선택하고 확인할 수 있습니다.

<img src="/docs/_remote/ef6b0e88960c90ce2923d1fe.webp" alt="" width="600">

#### 협업자로서 멀티 씬 월드에 배포하기

월드 협업자는 월드의 설정이나 권한을 편집할 수 없습니다. 다음의 **관리** 탭에서 제작자는 자신이 협업자인 월드를 볼 수는 있지만 다음에는 접근할 수 없습니다 **설정** 또는 **권한**.

<img src="/docs/_remote/f15119b09e3098fe4efe4dbe.webp" alt="" width="600">

게시 과정에서 제작자는 자신이 협업자로 지정된 구획에만 게시하도록 선택할 수 있습니다(월드 소유자가 설정한 대로).

다음의 **협업자** 섹션에서, 월드 소유자가 다음을 설정했다면 **사용자 지정 좌표** 제작자에게, 지정된 구획만 게시할 수 있습니다. 접근 권한이 다음으로 설정되어 있으면 **모든 구획**, 제작자는 월드의 어떤 구획이든 선택해 씬을 게시할 수 있습니다.


> [!WARNING]
> **📔 참고**: 다음 권한이 있는 협업자는 **모든 구획** 게시 권한으로 월드의 어떤 씬이든 덮어쓸 수 있습니다. 소유자나 다른 협업자가 게시한 씬이라도 마찬가지입니다.


<img src="/docs/_remote/4a5bbf90cf2c56710137c17e.webp" alt="" width="600">

### 비공개 월드

WORLD는 다양한 **접근** 설정을 가질 수 있습니다. 누구나 접근 가능하게 할 수도 있고, 다양한 방식으로 제한할 수도 있습니다.

#### WORLD의 접근 설정

다음의 **관리** 패널에서 월드 소유자는 월드의 **권한** 점 세 개를 클릭하여 접근할 수 있습니다. 월드 소유자는 다음 아래에서 접근 제한을 관리할 수 있습니다 **접근** 탭.

**접근 유형**

월드 소유자는 세 가지 유형의 **월드 접근**:

**공개**

누구나 월드에 접근할 수 있습니다. 이는 월드의 기본 설정입니다.

**비밀번호 보호**

비밀번호를 가진 사용자만 월드에 들어갈 수 있습니다.

비밀번호는 최소 8자 이상이어야 하며 숫자가 최소 2개 포함되어야 합니다. 생성 후에는 비밀번호에 접근할 수 없으므로 반드시 복사본을 보관하세요.

**초대 전용**

다음에 추가된 주소와 커뮤니티만 **승인된 주소** 월드에 접근할 수 있습니다.

새 주소나 커뮤니티를 다음 **승인된 주소**에 추가하려면, 다음 단계를 따르세요:

1. 다음을 클릭하세요 **+ 새 초대** 버튼.
2. 주소는 세 가지 방법으로 추가할 수 있습니다:

* **지갑 주소**: 개별 지갑을 하나씩 추가합니다.
* **커뮤니티**: 공개 커뮤니티를 검색하여 추가합니다. 이렇게 하면 **모든 커뮤니티 주소가** 다음에 **승인된 주소**.

  <img src="/docs/_remote/cd2c518c4a9642b70cf0c060.webp" alt="" width="600">
* **CSV 가져오기**: 주소 또는 커뮤니티 ID 목록이 있는 기존 CSV를 사용해 다음에 추가하세요 **승인된 주소**. 형식은 한 줄에 하나의 지갑이며, 예를 들면:

```
0x3bA7fD92eC4a1F6B8d2E9c5A7b1D3f6C8e4A2d9F
0xA1c9E4b7D2f6C8a3B5e9F1d4A7c2E6b8D3f9C5a1
```

가져오면 이미지에 표시된 것처럼 각 주소를 개별적으로 추적합니다.

<img src="/docs/_remote/554211db6a545dd6cf7ee848.webp" alt="" width="600">

3. 확인 후 주소는 다음에 들어갑니다 **승인된 주소**.
4. 새로운 **+ 새 초대**, 주소는 기존 목록에 추가되어 월드 소유자가 필요할 경우 목록을 관리하고 확장하는 데 도움이 됩니다.

<img src="/docs/_remote/c316240a3038949beab28744.webp" alt="" width="600">

5. 개별 주소 또는 주소 집합(커뮤니티의 경우)은 다음을 선택하여 제거할 수 있습니다 **삭제** 다음의 점 세 개에서 **승인된 주소** 섹션.


> [!WARNING]
> **📔 참고**: 다음을 변경하면 **접근** 유형을 **초대 전용**에서 **승인된 주소** 목록이 제거됩니다. 나중에 필요할 경우를 대비해 복사본을 보관하세요.


#### 비공개 월드로 들어가기

사용자가 다음이 없는 월드로 들어갈 때는 다양한 시나리오가 있습니다 **공개** 접근 권한:

* 다음에 있는 주소가 **승인된 주소**: 정상적으로 참여할 수 있습니다. 그렇지 않으면 월드가 다음과 같다는 정보를 받게 됩니다 **초대 전용**.

<img src="/docs/_remote/ac4dca8cd154a2b6ce27d48d.webp" alt="" width="300">

* 월드가 다음과 같습니다 **비밀번호 보호**: 사용자는 비밀번호를 입력할 수 있습니다. 최대 시도 횟수는 10회입니다.

<img src="/docs/_remote/65d948411619e496f8eed2f7.webp" alt="" width="300">

## 하드웨어 지갑에서 게시하기

LAND 토큰을 메타마스크 계정에 보관하는 대신, 다음과 같은 하드웨어 지갑 장치에 보관하는 것이 더 안전할 수 있습니다 [Ledger](https://www.ledger.com/) 또는 [Trezor](https://trezor.io/), 컴퓨터에 물리적으로 연결된 장치입니다.

이런 장치 중 하나를 사용 중이라면, 메시지 서명을 활성화하면서 토큰을 더 안전하게 보관하도록 하드웨어 지갑을 메타마스크에 연결할 수 있습니다. 다음을 참조하세요 [메타마스크의 이 문서](https://support.metamask.io/more-web3/wallets/how-to-connect-a-trezor-or-ledger-hardware-wallet/) 계정을 연결하는 방법은 여기에서 확인하세요.

하드웨어 지갑을 메타마스크에서 사용할 수 있게 되면, 토큰이 메타마스크 계정에 있는 것과 동일한 단계로 배포할 수 있습니다.

## 씬 덮어쓰기

새 씬이 배포되면 해당 씬이 차지하는 구획에 있던 이전 콘텐츠를 덮어씁니다.

보통 게시는 즉시 시작됩니다. 이미 월드에 존재하는 콘텐츠를 교체하려는 경우, Creator Hub가 먼저 확인을 요청하고 다음 옵션을 제공합니다 **다시 표시하지 않기** 옵션.

여러 구획을 차지하는 씬이 다른 씬에 의해 일부만 덮어써지더라도, 해당 씬의 모든 구획은 덮어써지거나 삭제됩니다.

여러분이 씬을 배포했다고 가정해 보겠습니다 *A* 두 구획에 *\[100, 100]* 그리고 *\[100, 101]*. 그런 다음 구획을 판매합니다 *\[100, 101]* 인접한 토지를 소유한 사용자에게, 그리고 그 사용자가 큰 씬(을 배포합니다*B*)을 여러 구획에 걸쳐 배포합니다. 여기에는 *\[100, 101]*.

여러분의 씬 *A* 단 하나의 구획에서 부분적으로만 렌더링될 수 없으므로 *\[100, 100]* 어떤 콘텐츠도 표시되지 않습니다. 씬의 새 버전을 만들어야 합니다 *A* 하나의 구획만 차지하도록 하고, 오직 해당 구획에만 배포하세요 *\[100, 100]*.

## 승인된 토지에 게시

보조금을 통해 할당받은 Decentraland Foundation 소유 토지에 게시하는 경우, 다음을 클릭하세요 **게시** 버튼을 평소처럼 클릭한 다음 다음을 선택하세요 **다른 서버에 게시** 하단의 옵션을 선택하세요. 그런 다음 다음을 선택하세요 **사용자 지정 서버** 드롭다운에서 선택하고 다음 서버 주소를 입력하세요: `https://linker-server.decentraland.org`.


> [!WARNING]
> **📔 참고**: 먼저 레이아웃 설정의 고급 탭에서 씬 좌표를 수동으로 설정해야 합니다. 다음을 참조하세요 [씬 설정](/docs/creator/content-creator-ko/scene-editor/configure/scene-settings/#layout) 자세한 내용은 여기에서 확인하세요.


## 사용자 지정 서버

공식 DAO가 관리하는 catalyst 서버 네트워크에 속하지 않는 사용자 지정 서버에 콘텐츠를 배포할 수 있습니다. 이를 위해 LAND나 NAME 토큰을 소유할 필요는 없으며, 서버가 원하는 검증 로직을 사용하도록 구성해 누가 어디에 배포할 수 있는지 제어할 수 있습니다. 사용자 지정 서버는 공식 서버의 콘텐츠를 가져와 덮어쓸 수도 있고, 빈 상태에서 시작해 완전히 새로운 콘텐츠를 게시할 수도 있습니다.

사용자 지정 서버에 게시하려면 다음을 클릭하세요 **게시** 버튼을 평소처럼 클릭한 다음 다음을 선택하세요 **다른 서버에 게시** 하단의 옵션을 선택하세요. 그런 다음 다음을 선택하세요 **사용자 지정 서버** 드롭다운에서 선택한 뒤 서버 주소를 입력하세요.

다음을 참조하세요 [자체 Catalyst 노드 실행 방법](/docs/contributor/tutorials/how-to-run-a-catalyst/) 자체 서버로 무엇을 할 수 있는지와 설정 방법에 대한 자세한 내용은 여기에서 확인하세요.


> [!WARNING]
> **📔 참고**: 플레이어가 사용자 지정 서버에 접속하려면 URL을 수동으로 입력해야 합니다. 다음과 같은 서비스의 일부 검증은 [보상 서버](/docs/creator/content-creator-ko/rewards/getting-started/) 이러한 상황에서 실패할 수 있는데, 이러한 서비스는 종종 요청이 공식 서버에서 와야 하기 때문입니다.


플레이어는 이 서버로 안내되지 않으며, 접속하려면 URL을 직접 입력하는 방법밖에 없습니다.

## 배포 성공 확인

씬을 배포한 후, 이러한 변경 사항이 업로드되고 처리되는 데 몇 분 정도 걸릴 수 있습니다. 배포 직후 Decentraland에 들어가면 잠시 이전 버전의 콘텐츠가 보일 수 있습니다.

Creator Hub는 게시가 다음 단계를 거치는 동안 진행 상황을 표시합니다 **업로드** 그리고 **변환** 단계를 거치며, 다음을 표시합니다 **들어가기** 씬을 플레이할 수 있게 되면 바로 버튼을 표시합니다.

### 변환 상태 확인

새 버전의 3D 모델이 변환되는 동안 플레이어에게는 의도적으로 가장 최근의 완전히 동작하는 버전의 씬이 제공됩니다. 변환은 길어도 몇 분 정도면 끝나야 하며, 더 오래 걸린다면 무언가 실패했을 수 있다는 신호일 수 있습니다.

브라우저에서 변환 상태를 직접 조회할 수 있습니다:

* `https://asset-bundle-registry.decentraland.org/entities/status/<pointer>`: 다음으로 바꾸세요 `<pointer>` 씬 좌표 중 하나로(예: `20,-34`) 또는 배포의 엔터티 ID로 바꾸세요. 응답에는 다음 아래에서 각 플랫폼의 변환 상태가 표시됩니다 `assetBundles`그리고 다음 아래에서 LOD의 상태가 표시됩니다 `lods`. 다음이 `완료되고` 이 `true`이면, 씬은 모든 플랫폼에서 완전히 변환된 것입니다. 월드의 씬인 경우, 쿼리 매개변수로 월드 이름을 추가하세요. 예: `.../entities/status/0,0?world_name=myname.dcl.eth`.
* `https://asset-bundle-registry.decentraland.org/queues/status`: 현재 변환 대기 중인 모든 씬의 ID를 플랫폼별로 나열합니다. 여기에서 엔터티 ID가 보인다면 씬이 다른 변환 뒤에 대기 중인 것이므로 조금 더 기다리기만 하면 됩니다.


> [!WARNING]
> **📔 참고**: 이미 씬을 불러온 상태의 Decentraland 탐색기 세션이 열려 있다면 앱을 닫았다가 다시 열어야 합니다. 그렇지 않으면 캐시된 버전만 계속 보게 됩니다.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/scene-editor/publish/publish-scene.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
