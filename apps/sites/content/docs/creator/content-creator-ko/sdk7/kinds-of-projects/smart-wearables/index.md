> 상호작용 기능이 있는 웨어러블 만들기

# 스마트 웨어러블

스마트 웨어러블은 일종의 전역 씬입니다. 예를 들면 [휴대형 경험](/docs/creator/content-creator-ko/sdk7/kinds-of-projects/portable-experiences/), 플레이어가 메타버스를 이동하면서 함께 가져가는 게임플레이입니다. 예를 들어, 전역 씬을 실행하는 동안 플레이어는 Genesis Plaza의 바닥에서 눈덩이를 집어 들고, 다른 씬으로 걸어간 다음, 같은 게임을 플레이하고 있는 다른 플레이어에게 그 눈덩이를 던질 수 있습니다.

스마트 웨어러블은 플레이어가 특정 의상을 착용할 때 활성화되는 휴대형 경험입니다. 스마트 웨어러블은 플레이어가 비행할 수 있게 해 주는 제트팩처럼 새로운 능력을 부여하거나, Genesis City 전체에 무작위로 코인을 배치해 수집하게 하는 것처럼 세계의 나머지 위에 새로운 콘텐츠 레이어를 추가할 수 있습니다.


> [!WARNING]
> **📔 참고**: 스마트 웨어러블은 SDK 7로만 생성할 수 있습니다.
> 
> 다음의 **Creator Hub** 현재 스마트 웨어러블 프로젝트 생성은 지원하지 않습니다.


## 시작하기

### CLI 사용하기

1. 새 폴더에서 명령줄을 열고 다음을 실행하세요

`npx @dcl/sdk-commands init --project smart-wearable`

이 명령은 새 스마트 웨어러블을 위한 기본 파일과 구조를 생성합니다.

## 템플릿의 파일

새로운 Smart Wearable 프로젝트의 폴더는 매우 유사합니다 [디센트럴랜드 씬](/docs/creator/content-creator-ko/sdk7/kinds-of-projects/scene-files/), 하지만 다음과 같은 차이점이 있습니다:

* `wearable.json` 휴대형 경험의 모든 메타데이터를 포함합니다
* 검은색 안경 한 쌍을 위한 자리 표시자 3D 모델(glasses.glb)과 썸네일(glasses.png)이 있습니다. 이를 실제로 제작 중인 웨어러블로 교체해야 합니다
* `scene.json` 포함합니다 `"isPortableExperience": true`. 파셀은 게시된 웨어러블에는 중요하지 않지만, 로컬 테스트용으로 설정할 수 있습니다. 미리보기는 `scene.json`

## wearable.json에 관하여

기본 `wearable.json` 파일은 다음과 같습니다:

```json
{
	"data": {
		"replaces": [],
		"hides": [],
		"tags": ["special", "new", "eyebrows"],
		"representations": [
			{
				"bodyShapes": [
					"urn:decentraland:off-chain:base-avatars:BaseMale",
					"urn:decentraland:off-chain:base-avatars:BaseFemale"
				],
				"mainFile": "glasses.glb",
				"contents": ["glasses.glb"],
				"overrideHides": [],
				"overrideReplaces": []
			}
		],
		"category": "eyewear"
	},
	"name": "스마트 웨어러블 예시",
	"description": "안경을 착용하고 새로운 세계를 보세요",
	"rarity": "mythic"
}
```

다음은 wearable.json에서 가장 일반적인 필드입니다 `wearable.json`:

* `id`: 스마트 웨어러블의 고유 ID.
* `이름` (필수): 마켓플레이스에서 사용자가 보게 될 웨어러블의 이름
* `설명`: 마켓플레이스에서 사용자가 보게 될 웨어러블의 설명입니다. 스마트 웨어러블이 무엇을 할 수 있는지 반드시 표시하세요. 마켓플레이스 사용자는 구매하기 전에 기능을 미리 볼 수 없기 때문입니다. 길이는 최대 **64자** 입니다. 업로드한 후 Builder의 아이템 편집기에서 더 긴 설명을 작성할 수 있습니다.
* `희귀도`: 토큰의 희귀도 공급량입니다. 가능한 값은 다음과 같습니다:
  * 고유(1개)
  * 신화급(10개)
  * 이색(50개)
  * 전설급(100개)
  * 에픽(1000개)
  * 레어(5000개)
  * 언커먼(10,000개)
  * 커먼(100,000개)


> [!WARNING]
> **📔 참고**: 프로젝트를 기존 프로젝트에서 포크했고 그 안에 `id`,가 포함되어 있다면, 웨어러블을 게시하기 전에 그 값이 고유한지 확인하세요. 다음을 사용하세요 [uuidgenerator.net](https://www.uuidgenerator.net/) 새 무작위 UUID를 생성하려면


* `data` (필수): 다음을 포함합니다. `가 다음을 대체합니다`, `hides`, `태그`, `representations` 그리고 `category` 모두 포함되어야 합니다. `가 다음을 대체합니다`, `hides` 그리고 `태그` 빈 목록일 수 있습니다 `[]`, 하지만 `representations` 항목이 최소 1개는 필요합니다. 스마트 웨어러블을 업로드한 후 Builder UI에서 이 설정을 조정할 수도 있습니다.
  * `가 다음을 대체합니다`: 이 카테고리의 기본값에 더해, 이 웨어러블을 착용할 때 함께 장착 해제해야 하는 다른 웨어러블 카테고리의 목록입니다. 예: 망토 상체를 착용할 때 발도 함께 숨깁니다.
  * `hides`: 이 카테고리의 기본값에 더해, 이 웨어러블을 착용할 때 숨겨야 하지만(장착 해제하지는 않음) 하는 다른 웨어러블 카테고리의 목록입니다.
  * `태그`: 마켓플레이스에서 웨어러블을 검색 가능하게 만드는 데 사용되는 태그입니다.
  * `representations`:
    * `bodyShapes`: 이 웨어러블을 사용할 수 있는 아바타 표현의 목록입니다. 모든 스마트 웨어러블은 남성과 여성 바디 셰이프 모두에서 사용할 수 있어야 합니다.
    * `mainFile`: 웨어러블의 3D 모델이 들어 있는 메인 파일입니다.
    * `contents`: 웨어러블의 3D 모델을 렌더링하는 데 사용되는 파일의 전체 목록입니다. 예를 들어, 3D 모델에 텍스처가 별도 파일로 포함될 수 있습니다.
    * `overrideHides`: 이 웨어러블 카테고리의 기본 *숨기기* 동작과 다른 예외 사항입니다.
    * `overrideReplaces`: 이 웨어러블 카테고리의 기본 *대체* 동작과 다른 예외 사항입니다.
  * `category`: 사용할 웨어러블 카테고리입니다. 가능한 값은 다음과 같습니다:
    * 'eyebrows'
    * 'eyes'
    * 'facial\_hair'
    * 'hair'
    * 'mouth'
    * 'upper\_body'
    * 'lower\_body'
    * 'feet'
    * 'earring'
    * 'eyewear'
    * 'hat'
    * 'helmet'
    * 'mask'
    * 'tiara'
    * 'top\_head'
    * 'skin'
    * 'hands\_wear'


> [!WARNING]
> **📔 참고**: 위의 필드 외에도 Builder는 선택적 최상위 `매핑` 필드와 선택적 `removesDefaultHiding` 그리고 `blockVrmExport` 필드를 `data`. 스마트 웨어러블은 일반적으로 이것들이 필요하지 않습니다. Builder는 다음을 가진 `wearable.json` 을 거부합니다 *"the wearable config file is invalid"*. 이전 버전의 이 페이지에서는 `menuBarIcon`, `model`, `bodyShape` 그리고 `collectionAddress`: 프로젝트에 아직 남아 있다면 제거하세요. 동일한 오류는 다음 경우에도 나타납니다 `설명` 길이가 64자를 넘는 경우.


## 썸네일

이름이 `thumbnail.png` 인 이미지를 폴더의 최상위 수준에 포함해야 합니다. 이 이미지는 배낭과 마켓플레이스에서 모두 표시되어 2D로 웨어러블을 나타냅니다. 권장되는 필수 이미지 크기는 256x256입니다.

플레이어의 기대를 설정하고 제작물을 올바르게 나타내는 이미지를 선택하세요.

## 미리보기

스마트 웨어러블의 미리보기를 실행하는 것은 씬의 미리보기를 실행하는 것과 같습니다. Creator Hub에서 **Play** 를 클릭하거나 명령줄에서 `npm run start` 를 실행하세요. Creator Hub에서는 스마트 웨어러블 프로젝트를 만들 수 없지만, 열고 미리볼 수는 있습니다.

미리보기는 휴대형 경험이 아니라, 목록에 있는 파셀에서 웨어러블의 코드를 일반 씬으로 실행합니다 `scene.json`. 즉, 일부 동작은 게시된 웨어러블과 다릅니다:

* 플레이어가 서 있는 씬에서만 작동하는 코드, 예를 들어 [플레이어에 대한 물리력](/docs/creator/content-creator-ko/sdk7/interactivity/player-physics/)은 미리보기 파셀을 벗어나면 작동을 멈춥니다. 게시되면 스마트 웨어러블은 플레이어가 어디로 가든 계속 작동합니다.
* 웨어러블의 3D 모델이 아바타에 표시되지 않을 수 있습니다.

웨어러블을 착용한 상태로 어디서나 작동하는지 시험하려면 Builder에 업로드하고 다음을 사용하세요 **디센트럴랜드에서 보기** (참조: [게시](#publish)). 이 작업을 위해 컬렉션을 민팅하거나 게시할 필요는 없습니다.

스마트 웨어러블이 씬의 맥락에서 어떻게 동작하는지 테스트하려면, 다음을 사용하여 하나 이상의 씬의 미리보기를 실행하는 동시에 웨어러블의 미리보기도 실행할 수 있습니다 [워크스페이스](/docs/creator/content-creator-ko/sdk7/kinds-of-projects/workspaces/). 예를 들어, [Genesis Plaza](https://github.com/decentraland-scenes/Genesis-Plaza) 씬과 함께 스마트 웨어러블을 실행하여, 붐비는 씬이나 엘리베이터 안 등에서 어떻게 동작하는지 테스트할 수 있습니다.

## 팁

* 엔티티를 배치할 때 위치는 Genesis Plaza의 0,0 좌표를 기준으로 하는 전역 좌표라는 점에 유의하세요.
* 주변 플레이어에 반응하려면:
  * 다음을 참조하세요 [모든 플레이어 가져오기](/docs/creator/content-creator-ko/sdk7/interactivity/user-data/#fetch-all-players) 를 참조하여 주변의 다른 플레이어로부터 데이터를 얻는 방법을 알아보세요.
  * 스마트 웨어러블, 주변 씬, 다른 플레이어의 로딩은 상황에 따라 서로 다른 순서로 일어날 수 있다는 점에 유의하세요. 플레이어가 이미 스마트 웨어러블을 착용한 상태로 Decentraland에 들어오면, 보통 다른 플레이어보다 먼저 웨어러블의 전역 씬이 로드됩니다. 반면 플레이어가 먼저 씬에 들어가고 그다음 웨어러블을 착용하면, 웨어러블의 씬이 실행될 때쯤에는 다른 플레이어가 이미 로드되어 있을 가능성이 큽니다.
  * 다른 플레이어는 플레이어가 자신의 영역 안에 있는 섬에 연결된 경우에만 로드됩니다. realm 데이터를 가져와 ‘room’ 필드를 확인하세요. ‘room’ 필드가 null이면 플레이어는 아직 섬에 연결되지 않은 상태이므로 다른 플레이어도 아직 로드되지 않습니다. ‘room’ 필드가 있을 때까지 1초마다 주기적으로 이를 확인한 다음에만 로직을 초기화할 수 있습니다. 이 작업은 다른 플레이어를 보는 것만 가능하게 하며, 착용자들 간 데이터를 공유하려면 다음을 참조하세요 [제한 사항](#limitations).
* 주변 씬과 상호작용하려면:
  * 주변 씬이나 다른 휴대형 경험에 직접 지시를 보낼 수는 없습니다.  `messageBus` 는 현재 각 휴대형 경험/씬마다 샌드박스 처리되어 있습니다.
  * 중간 서버를 사용하여 휴대형 경험과 씬 사이에 정보를 보낼 수 있습니다.
  * 레이캐스트를 수행하면 주변 씬의 엔티티 콜라이더에 대한 적중을 감지할 수 있습니다. 이를 통해 정확한 적중 위치, 법선 방향, 심지어 3D 모델의 엔티티 이름과 메쉬 이름까지 알 수 있습니다. 이는 SDK7로 작성된 씬의 엔티티에 맞을 때만 작동합니다.
* 휴대형 경험 종료하기: 다음을 실행하세요 `kill()` 메서드로 휴대형 경험을 스스로 종료합니다.
* 플레이어를 밀어내기: [Physics](/docs/creator/content-creator-ko/sdk7/interactivity/player-physics/) 과 같은 함수는 `Physics.applyImpulseToPlayer()` 웨어러블이 있는 씬은 항상 플레이어가 있는 씬으로 간주되므로, 착용자가 어디로 가든 작동합니다. 권한은 필요하지 않습니다. 씬은 방문자의 스마트 웨어러블을 끌 수 있습니다. 다음을 참조하세요 [기능 토글](/docs/creator/content-creator-ko/sdk7/kinds-of-projects/scene-metadata/#feature-toggles).
* 착용자만 들을 수 있는 소리를 재생하려면: 엔티티를 [AvatarAttach](/docs/creator/content-creator-ko/sdk7/3d/entity-positioning/#attach-an-entity-to-an-avatar) 로 플레이어에 첨부하고 거기에 `AudioSource`을 부여하세요. 다른 플레이어는 들을 수 없습니다.
* 필요하기 전에 파일 로드하기: 웨어러블이 시작될 때 소리, 모델 또는 이모트 파일을 가져오도록 [AssetLoad](/docs/creator/content-creator-ko/sdk7/optimizing/pre-load-resources/) 컴포넌트를 사용하세요. 그러면 처음 사용할 때 다운로드를 기다리지 않아도 됩니다.

## 게시

스마트 웨어러블을 게시하려면:

1. 다음의 정보가 정확한지 확인하세요 `wearable.json` 프로젝트를 시작점으로 사용했다면, `id` 가 다른 웨어러블에서 사용되지 않는 고유 식별자인지 확인하세요.
2. 실행하세요 `npm run pack` 을 프로젝트 폴더에서 실행하세요. 그러면 `smart-wearable.zip` 파일이 프로젝트 폴더에 생성됩니다.


> [!WARNING]
> **📔 참고**: 압축되지 않은 `smart-wearable.zip` 의 내용 총합은 3MB 미만이어야 하며, 그렇지 않으면 Builder가 파일이 너무 크다고 거부합니다. `npm run pack` 프로젝트가 2MB를 넘으면 크기 메시지를 출력하지만, zip은 계속 생성합니다. zip 자체의 크기가 아니라 zip 안의 파일 크기를 확인하세요. 다음을 참조하세요 [제한 사항](#limitations).


3. Builder를 열고, Collections 탭을 열고, +를 클릭하여 새 웨어러블을 업로드하세요.
4. 압축된 `smart-wearable.zip` 파일을 Builder로 드래그하고, 모든 정보가 정확한지 확인하세요.

> 참고: 웨어러블에 바디 셰이프마다 다른 3D 모델이 있다면, 바디 셰이프별로 하나의 표현을 나열하세요:
>
> 1. 프로젝트에서 각 표현(`남성` 그리고 `여성`)마다 새 폴더를 만들고, 각 표현의 3D 모델을 해당 폴더에 넣으세요.
> 2. 다음을 업데이트하세요 `wearable.json` 파일에 새 표현을 포함하세요.
> 3. ```lang-json
>    "representations": [{
>      "bodyShapes": ["urn:decentraland:off-chain:base-avatars:BaseMale"],
>      "mainFile": "male/glasses.glb",
>      "contents": ["male/glasses.glb"],
>      "overrideHides": [],
>      "overrideReplaces": []
>    },
>    {
>      "bodyShapes": ["urn:decentraland:off-chain:base-avatars:BaseFemale"],
>      "mainFile": "female/glasses.glb",
>      "contents": ["female/glasses.glb"],
>      "overrideHides": [],
>      "overrideReplaces": []
>    }],
>    ```
> 4. 실행하세요 `npm run pack` 새 smart-wearable.zip 파일을 생성하려면.
> 5. 새 smart-wearable.zip 파일을 Builder로 드래그하세요.

5. 웨어러블 편집기를 열고, 이 웨어러블이 켜질 때 다른 웨어러블 카테고리를 비활성화하도록 “hide”와 “remove” 카테고리가 올바르게 설정되어 있는지 확인하세요.
6. 이 웨어러블과 다른 웨어러블로 새 컬렉션을 만드세요.
7. 컬렉션에서 다음을 클릭하세요 **디센트럴랜드에서 보기**. 그러면 Sepolia 테스트 네트워크에서 Decentraland가 열립니다. 여기서 백팩에서 컬렉션의 웨어러블을 착용하고, Genesis Plaza를 돌아다니는 것처럼 더 현실적인 시나리오에서 어떻게 동작하는지 볼 수 있습니다. 다음을 참조하세요 [디센트럴랜드에서 테스트하기](/docs/creator/content-creator-ko/wearables-and-emotes/manage-collections/uploading-smart-wearables/#testing-in-decentraland).
8. 이 시점에서 웨어러블은 게시할 준비가 되었습니다.

## 권한

스마트 웨어러블은 제한된 권한으로 실행됩니다. 다음 기능 중 하나를 사용하려면 `requiredPermissions` 목록의 `scene.json`,에 해당하는 권한을 선언하세요. 그렇지 않으면 런타임에서 호출이 실패합니다:

* `USE_FETCH`: 다음을 사용하는 HTTP 요청 `fetch` 또는 `signedFetch`
* `USE_WEBSOCKET`: 웹소켓 연결
* `USE_WEB3_API`: 지갑 거래 또는 메시지 서명
* `OPEN_EXTERNAL_LINK`: 플레이어에게 외부 링크를 열도록 요청하기

```json
"requiredPermissions": [
    "USE_FETCH"
  ],
```

스마트 웨어러블이 이들 중 하나를 선언하면, 플레이어는 그것을 착용할 때 승인 프롬프트를 보게 되고, 거부하면 웨어러블은 실행되지 않습니다. 사용하는 것만 선언하세요. 다음을 참조하세요 [필수 권한](/docs/creator/content-creator-ko/sdk7/kinds-of-projects/scene-metadata/#required-permissions) 에서 자세한 내용을 확인하세요.

## 제한 사항

> 중요: 전체 스마트 웨어러블은 3MB 이내여야 합니다. 여기에는 3D 모델, 썸네일, 코드, 라이브러리, 사운드 파일, 추가 3D 모델, UI 이미지 등이 모두 포함됩니다. 이 제한은 압축되지 않은 폴더 기준입니다. Builder는 이보다 큰 웨어러블은 업로드할 수 없습니다. 크기를 확인하려면 생성된 `smart-wearable.zip` 파일의 압축을 풀고 폴더 크기를 확인하거나 다음을 실행하세요 `unzip -l smart-wearable.zip` 를 실행하고 총합을 확인하세요.

파일을 zip에 포함하지 않으려면 프로젝트의 `.dclignore` 파일에 나열하세요. 씬에서 하듯이 하면 됩니다.


> [!WARNING]
> **📔 참고**: Creator Hub에서 스마트 웨어러블 프로젝트를 편집하면, 3D 뷰의 전체 크기 스크린샷을 다음으로 저장할 수 있습니다 `assets/Images/autogenerated-thumbnail.png` 로 지정하고 `scene.json` 가 그것을 가리키게 할 수 있습니다. 그 이미지만으로도 몇 MB가 될 수 있으며 웨어러블이 한도를 초과하게 만들 수 있습니다. 스마트 웨어러블은 그것을 사용하지 않으므로 다음을 추가하세요 `assets/Images/autogenerated-thumbnail.png` 로 설정하여 `.dclignore`.


스마트 웨어러블은 웨어러블을 착용한 플레이어에 대해서만 휴대형 경험을 실행합니다. 다른 플레이어는 효과를 보지 못합니다. 예를 들어, 휴대형 경험이 플레이어를 따라다니는 펫을 렌더링한다면, 주변의 다른 플레이어는 이 펫을 보지 못합니다. 그러나 다른 플레이어는 웨어러블의 씬의 일부로 실행되는 아바타 애니메이션은 볼 수 있습니다. 심지어 [사용자 지정 아바타 애니메이션](/docs/creator/content-creator-ko/sdk7/interactivity/avatars/avatar-animations/#custom-animations) 이 스마트 웨어러블의 파일의 일부로 업로드된 경우에도 마찬가지입니다.

스마트 웨어러블에는 내장 멀티플레이어가 없습니다. 웨어러블을 착용한 각 플레이어는 두 착용자가 나란히 서 있어도 코드의 별도 복사본을 각각 실행합니다.  `MessageBus`, 다음과의 엔티티 동기화 `syncEntity()` 및 [멀티플레이어 서버](/docs/creator/content-creator-ko/sdk7/networking/authoritative-servers/) 는 스마트 웨어러블에서 작동하지 않습니다. 공유 점수처럼 착용자 간 데이터를 공유하는 유일한 방법은, 웨어러블이 `fetch` 또는 WebSocket으로 통신하는 자체 서버입니다. 다음을 참조하세요 [네트워크 연결](/docs/creator/content-creator-ko/sdk7/networking/network-connections/).

사용자 지정 애니메이션에는 props가 포함될 수 있으며, props에는 애니메이션된 뼈대에서 만들어진 파티클 효과가 포함될 수 있습니다(다음을 참조 [Props 및 사운드](/docs/creator/content-creator-ko/wearables-and-emotes/emotes/props-and-sounds/) 그리고 [이모트의 파티클](/docs/creator/content-creator-ko/wearables-and-emotes/emotes/particles-in-emotes/)). 다른 플레이어도 이것들을 볼 수 있습니다. 사운드는 다릅니다:  `triggerSceneEmote()` 로 재생된 사용자 지정 애니메이션은 emote 파일에 사운드가 포함되어 있어도 오디오를 재생하지 않습니다. 소리를 재생하려면 `AudioSource`을 사용하세요. 이것은 착용자만 들을 수 있습니다.

스마트 웨어러블은 플레이어가 착용했을 때만 작동합니다. 이러한 이유로 플레이어는 활성화된 스마트 웨어러블을 제한된 개수만 가질 수 있으며, 신체의 어느 부분을 차지하느냐에 따라 서로 호환되지 않는 것들도 있습니다. 예를 들어, 동시에 두 개의 모자를 쓸 수는 없고, 헬멧과 머리카락을 동시에 착용할 수도 없습니다.

다른 웨어러블을 착용했을 때 웨어러블이 “숨겨지지만” “제거되지는” 않았다면, 해당 웨어러블이 보이지 않더라도 스마트 웨어러블은 계속 켜져 있을 수 있습니다.

## 예시

[스마트 웨어러블 샘플](https://github.com/decentraland/smart-wearable-sample))


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/kinds-of-projects/smart-wearables.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
