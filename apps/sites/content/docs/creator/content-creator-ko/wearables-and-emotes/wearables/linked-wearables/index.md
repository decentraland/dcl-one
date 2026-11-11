> 3자 토큰의 웨어러블 표현

# 연결된 웨어러블

## 소개

다음에 따라 [Linked Wearables의 초기 DAO 제안](https://governance.decentraland.org/proposal/?id=14e76cc0-2bc7-11ec-ac84-77607720a240) (이전 명칭: Third Party Wearables), [최종 정의가 포함된 제안 초안](https://governance.decentraland.org/proposal/?id=f69c4d40-aaaf-11ec-87a7-6d2a41508231) 및 [Linked Wearables 재설계 제안](https://decentraland.org/governance/proposal/?id=65caf8d1-8601-49a5-ae11-b0b99d7fdd3c), 이 문서는 Linked Wearables 기능과 관련된 모든 세부 정보를 다루는 문서로 활용됩니다.

이 문서는 주로 사용자가 Decentraland를 돌아다닐 때 자신의 NFT를 웨어러블로 표현할 수 있도록 하려는 NFT 커뮤니티의 대표자를 대상으로 합니다.

## Linked Wearables란 무엇인가요?

Linked Wearables는 Decentraland 외부에서 생성된 NFT의 3D 표현으로, 월드 내에서 웨어러블로 사용할 수 있고 아바타에 장착할 수 있으며 백팩에서 찾을 수 있습니다. 이는 [일반 웨어러블](https://github.com/decentraland/docs/tree/main/creator/wearables-and-emotes/wearables/README.md#what-are-wearables)이 아닙니다. 외형은 동일하며 일반 웨어러블 [가이드라인](/docs/creator/content-creator-ko/wearables-and-emotes/wearables/creating-wearables/) 을 따르지만 완전히 다른 의미를 가집니다.

Linked Wearables는 기존 웨어러블 컬렉션에 존재하지 않으며(특수한 유형의 컬렉션에 속함), 희귀도가 없고 [1차](https://market.decentraland.org/browse?assetType=item&section=wearables) 또는 [2차](https://market.decentraland.org/browse?assetType=nft&section=wearables&vendor=decentraland&page=1&sortBy=recently_listed&onlyOnSale=true&viewAsGuest=false&onlySmart=false) 시장에서 판매할 수 없습니다. 이들은 오직 **외부 NFT에 연결된 월드 내 표현입니다.**

> 각 NFT가 서로 다른 종류의 2D 재킷인 ‘Cryptojackets’라는 NFT 프로젝트가 있으며, 사용자가 Decentraland 백팩에서 자신의 재킷을 3D로 표현할 수 있게 하려고 한다고 가정해 보겠습니다. Linked Wearables를 사용하면 NFT의 3D 표현을 Decentraland의 웨어러블로 제출할 수 있습니다. 새 토큰을 민팅할 필요가 없으며, 기존 NFT 프로젝트에 즉시 사용할 수 있는 새로운 기능을 제공할 수 있습니다!

모든 Linked Wearables는 **Linked Wearable 컬렉션** 내에 정의됩니다. 이 문서의 아래쪽에서 생성 방법을 살펴보겠습니다.

## Linked Wearables는 NFT를 어떻게 표현하나요?

웨어러블은 [Builder 사이트](https://decentraland.org/builder) 에서 Linked Wearable 컬렉션을 생성하고 웨어러블 생성 시 NFT를 표현할 방식을 설정하여 NFT에 연결됩니다.

웨어러블을 NFT에 연결하는 4가지 메커니즘을 지원합니다. 모든 메커니즘은 NFT의 토큰 ID를 사용하여 일치시킵니다.

다음 표는 앞서 언급한 메커니즘을 보여 줍니다:

| 일치     |                                                                                                                                                               |
| ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 모든 NFT | 컬렉션의 NFT 중 하나라도 소유한 사용자는 웨어러블을 소유하게 됩니다                                                                                                                       |
| 단일 NFT | TOKEN ID로 지정된 NFT를 소유한 사용자는 웨어러블을 소유하게 됩니다. *예: 123456. TOKEN ID가 123456인 NFT를 소유한 경우 사용자는 웨어러블을 소유하게 됩니다*                                                    |
| 여러 NFT | 쉼표로 구분하여 TOKEN ID로 지정된 여러 NFT 중 하나를 소유한 사용자는 웨어러블을 소유하게 됩니다. *예: 123456, 123457, 123458. TOKEN ID가 123456, 123457 또는 123458인 NFT를 소유한 경우 사용자는 웨어러블을 소유하게 됩니다* |
| NFT 범위 | TOKEN ID 범위로 지정된 여러 NFT 중 하나를 소유한 사용자는 웨어러블을 소유하게 됩니다. *예: \[1, 1000]. TOKEN ID가 1, 2, 3, 4, 5 ... 또는 100인 NFT를 소유한 경우 사용자는 웨어러블을 소유하게 됩니다*                   |

## Linked Wearables 컬렉션 유형

일반적으로 NFT 컬렉션에는 두 가지 유형이 있습니다:

* **수작업 제작:** 각 토큰 자산이 자동화 과정 없이 맞춤 제작되거나 수작업으로 제작되는 경우입니다.
* **프로그래밍 방식:** 각 토큰 자산이 개별적으로 수작업 제작된 것이 아니라 코드로 자동 생성되는 경우이며, 대개 미리 디자인하고 모델링한 특성에서 생성됩니다. 예: [CryptoPunks](https://opensea.io/collection/cryptopunks) 및 [BAYC](https://opensea.io/collection/boredapeyachtclub) 는 프로그래밍 방식으로 생성된 2D 프로필 사진 NFT 컬렉션의 예입니다.

Decentraland의 Linked Wearables 컬렉션에도 동일한 원칙을 적용합니다. 수작업 제작 컬렉션을 위한 Standard Linked Wearable 컬렉션과 자동 생성 컬렉션을 위한 Programmatic Linked Wearable 컬렉션이 있습니다. 두 컬렉션은 웨어러블 게시 수수료 비용이 다릅니다. 자세한 게시 수수료 정보는 [비용 섹션](#costs) 을 확인하세요.

## Linked Wearables 만들기

## Linked Wearable 컬렉션 만들기

Linked Wearable 컬렉션을 만드는 것은 Linked Wearables를 만드는 첫 번째 단계입니다.

Linked Wearables는 소유자가 생성, 편집 및 삭제할 수 있는 컬렉션으로 그룹화됩니다. 각 컬렉션은 임의 개수의 Linked Wearables를 포함할 수 있습니다. 각 컬렉션은 NFT 컬렉션에 연결되므로, **Linked Wearable 컬렉션을 만들려면 NFT 계약(ERC721 또는 ERC1155 호환)이 필요합니다.**

새 Linked Wearable을 만들려면 다음 단계를 따르세요:

1. 컬렉션 섹션에서 **컬렉션 생성** 버튼을 클릭합니다.

   ![](/docs/_remote/4e2aa2f187322d76f74c1b4c.webp)
2. Linked Collections 섹션 아래의 **컬렉션 생성** 버튼을 클릭하여 Linked Wearable 컬렉션 옵션을 선택합니다.

   ![](/docs/_remote/23dbdab92c38a595e6d609ae.webp)
3. 컬렉션 이름을 선택하고 계약 주소와 해당 네트워크를 설정하여 컬렉션을 NFT 컬렉션에 연결합니다. **계약이 NFT 계약 표준을 준수하는지 확인하기 위해 검증됩니다.**

   ![](/docs/_remote/8365f405886986e6be3328d6.webp)
4. 다음을 클릭합니다 **생성** 버튼을 클릭하여 컬렉션을 만듭니다.

## Linked Wearables 컬렉션에 웨어러블 추가하기 - 하나씩

일반 웨어러블과 마찬가지로 웨어러블의 3D 모델을 하나씩 업로드할 수 있습니다.

이렇게 하려면 다음 단계를 따르세요:

1. 다음을 클릭합니다 **새 아이템** 버튼을 클릭합니다.

   ![](/docs/_remote/e3843eed8d736f40c93c6f2e.webp)
2. 다음을 선택합니다 **단일 아이템** 옵션.

   ![](/docs/_remote/444a582037b01c922c963b76.webp)
3. 다음에 설명된 대로 웨어러블을 업로드하고 구성하는 단계를 따르세요 [웨어러블 생성 가이드라인](/docs/creator/content-creator-ko/wearables-and-emotes/wearables/creating-wearables/) 을 따르고, NFT에 연결되는 방식을 구성합니다. 구성 방법에 대한 자세한 내용은 "[Linked Wearables는 NFT를 어떻게 표현하나요?](#how-do-linked-wearables-represent-nfts)" 섹션을 확인하세요.

   ![](/docs/_remote/3f230f70eb074d1a7d79fecf.webp)

## Linked Wearables 컬렉션에 웨어러블 추가하기 - 일괄

Linked Wearable 컬렉션에는 많은 수의 아이템을 포함할 수 있으므로, 웨어러블의 3D 모델과 정보를 일괄 업로드할 수 있습니다. 이 과정에는 각 아이템에 필요한 모든 자산을 포함한 ZIP 파일을 만드는 작업이 포함됩니다. **웨어러블 일괄 업로드는 프로그래밍 방식 컬렉션에만 권장됩니다**.

### 웨어러블 ZIP 빌드하기

각 아이템에는 다음 자산을 포함하는 ZIP 파일이 필요합니다:

* 다음 **필수** 웨어러블의 3D 모델 파일(GLB, GLTF, 텍스처 파일 등).
* 다음 **필수** `wearable.json` 웨어러블 정보를 포함하는 파일.
* 다음 **선택 사항** `thumbnail.png` Builder와 월드에서 표시될 웨어러블 썸네일을 포함하는 파일. 제공하지 않으면 3D 모델을 사용하여 생성됩니다.

3D 모델과 선택 사항인 `thumbnail.png` 는 다음을 따릅니다 [사용자 지정 썸네일 섹션](/docs/creator/content-creator-ko/wearables-and-emotes/manage-collections/uploading-wearables/#custom-thumbnails) 에서 사용자 지정 썸네일 생성 방법을 확인하려면 웨어러블 업로드 문서를 참조하세요.

다음 `wearable.json` 웨어러블 콘텐츠에 포함되는 파일은 다음 형식입니다(Typescript로 작성한 경우):

```typescript
type WearableConfiguration = {
  /** 웨어러블의 URN */
  id?: string
  /** 웨어러블 이름 */
  name: string
  /** 웨어러블 설명 */
  description?: string
  data: {
    /** 웨어러블 장착 시 교체할 웨어러블 */
    replaces: WearableCategory[]
    /** 웨어러블 장착 시 숨길 웨어러블 */
    hides: WearableCategory[]
    /** 웨어러블을 식별하기 위한 태그 */
    tags: string[]
    /** 웨어러블의 표현 */
    representations: WearableRepresentation[]
    /** 웨어러블 카테고리 */
    category: WearableCategory
  },
  /** NFT와 웨어러블 간 매핑 정의 */
  mapping: Mapping
}

type WearableRepresentation = {
  /** 표현의 체형 */
  bodyShapes: BodyShape[];
  /** 표현의 기본 파일(GLB, GLTF 등)에 대한 파일 경로 */
  mainFile: string;
  /** 표현에 속하는 파일들의 파일 경로 목록 */
  contents: string[];
  /** 이 표현을 장착할 때 숨길 웨어러블 */
  overrideHides: WearableCategory[];
  /** 이 표현을 장착할 때 교체할 웨어러블 */
  overrideReplaces: WearableCategory[];
}

enum WearableCategory = {
  EYEBROWS = 'eyebrows',
  EYES = 'eyes',
  FACIAL_HAIR = 'facial_hair',
  HAIR = 'hair',
  HEAD = 'head',
  BODY_SHAPE = 'body_shape',
  MOUTH = 'mouth',
  UPPER_BODY = 'upper_body',
  LOWER_BODY = 'lower_body',
  FEET = 'feet',
  EARRING = 'earring',
  EYEWEAR = 'eyewear',
  HAT = 'hat',
  HELMET = 'helmet',
  MASK = 'mask',
  TIARA = 'tiara',
  TOP_HEAD = 'top_head',
  SKIN = 'skin'
}

enum WearableBodyShape {
  MALE = 'urn:decentraland:off-chain:base-avatars:BaseMale',
  FEMALE = 'urn:decentraland:off-chain:base-avatars:BaseFemale'
}

type Mapping = SingleMapping | AnyMapping | RangeMapping | MultipleMapping

enum MappingType {
  SINGLE = 'single',
  ANY = 'any',
  MULTIPLE = 'multiple',
  RANGE = 'range'
}

type SingleMapping = {
  type: MappingType.SINGLE
  id: string
}

type AnyMapping = {
  type: MappingType.ANY
}

type RangeMapping = {
  type: MappingType.RANGE
  from: string
  to: string
}

type MultipleMapping = {
  type: MappingType.MULTIPLE
  ids: string[]
}
```

다음은 `wearable.json` 각 체형에 대한 모델을 포함하는 파일의 예입니다:

```json
{
  "id": "urn:decentraland:matic:collections-thirdparty:my-third-party:my-collection:1",
  "name": "특별한 모자",
  "description": "웨어러블에 대한 설명",
  "data": {
    "replaces": [],
    "hides": ["hair"],
    "tags": ["special", "new", "hat"],
    "representations": [
      {
        "bodyShapes": ["urn:decentraland:off-chain:base-avatars:BaseMale"],
        "mainFile": "aMaleModelFile.glb",
        "contents": ["aMaleModelFile.glb", "aTextureFile.png"],
        "overrideHides": [],
        "overrideReplaces": []
      },
      {
        "bodyShapes": ["urn:decentraland:off-chain:base-avatars:BaseFemale"],
        "mainFile": "aFemaleModelFile.glb",
        "contents": ["aFemaleModelFile.glb", "anotherTextureFile.png"],
        "overrideHides": [],
        "overrideReplaces": []
      }
    ],
    "category": "hat",
    "mapping": {
      "type": "single",
      "id": "123"
    }
  }
}
```

이 파일은 다음 파일과 함께 ZIP으로 압축됩니다 `aMaleModelFile.glb`, `aTextureFile.png`, `aFemaleModelFile.glb` 및 `anotherTextureFile.png`.

웨어러블에 사용자 지정 썸네일을 추가하려면 다음을 추가할 수 있습니다 `thumbnail.png` 파일.

다음에 대해 고려할 사항 `wearable.json` 파일:

* 웨어러블 카테고리 및 선택 방법에 대한 모든 정보는 다음에서 찾을 수 있습니다 [웨어러블 생성 가이드라인](/docs/creator/content-creator-ko/wearables-and-emotes/wearables/creating-wearables/).
* representations 배열에는 각 체형이 표시되는 방식에 대한 정보가 포함됩니다. 각 웨어러블에는 최소 하나의 표현이 반드시 포함되어야 합니다(하나 또는 둘 모두를 포함할 수 있음). 즉, 현재 제공되는 체형을 고려하면 다음 중 하나입니다 `urn:decentraland:off-chain:base-avatars:BaseMale` 또는 `urn:decentraland:off-chain:base-avatars:BaseFemale`. 각 표현은 각 체형에 어떤 모델을 사용할지 설명합니다.
* mapping 객체는 "[Linked Wearables는 NFT를 어떻게 표현하나요?](#how-do-linked-wearables-represent-nfts)" 섹션에 따라 NFT를 웨어러블에 연결하는 데 사용할 수 있는 메커니즘 중 하나로 구성해야 합니다.

**아이템의 사용자 지정 ID 또는 URN 설정**

다음 `id` 필드는 선택 사항이며, 나중에 일괄 업데이트할 특정 ID의 웨어러블을 만드는 데 사용할 수 있습니다(자세한 내용은 [웨어러블 일괄 편집](#editing-wearables-in-bulk) 섹션에서 설명합니다).

다음의 경우 `id` 필드를 사용하는 경우 웨어러블의 전체 ID를 포함해야 합니다. 웨어러블의 ID(또는 URN)는 다음과 같이 작성됩니다 `urn:decentraland:matic:collections-thirdparty:third-party-id:collection-id:item-id`. 여기서 `urn:decentraland:matic:collections-thirdparty:third-party-id:collection-id` 는 컬렉션 페이지에서 가져올 수 있으며 `item-id` 는 사용하려는 아이템의 사용자 지정 식별자입니다.

ID 또는 URN은 특정 형식을 따르며, 다음을 허용합니다:

* 다음 범위의 소문자 `a` 부터 `z`.
* 모든 숫자.
* 다른 유형의 문자나 공백을 포함할 수 없습니다. 공백은 다음으로 대체하는 것을 권장합니다 `-`

다음 단계에 따라 컬렉션 페이지에서 ID(또는 URN)를 가져올 수 있습니다:

1. URN을 복사하려는 컬렉션 보기로 이동하여 옵션 메뉴에서 Edit URN 옵션을 클릭합니다:

![](/docs/_remote/72142577aacca52f35921c2a.webp)

2. 텍스트 필드 아래의 식별자를 복사합니다:

![](/docs/_remote/8d7643cea8e5a23b82de3fbb.webp)

예를 들어 컬렉션의 UI에서 가져온 URN 또는 ID가 다음과 같고 `urn:decentraland:matic:collections-thirdparty:my-third-party:my-collection` 웨어러블을 숫자로 식별하는 경우, 예시의 URN은 다음과 같습니다 `urn:decentraland:matic:collections-thirdparty:my-third-party:my-collection:1`, 여기서 `1` 는 웨어러블의 번호입니다.

### 업로드 과정

모든 파일이 준비되면 웨어러블을 일괄 업로드하기 위해 다음 단계를 따르세요:

1. 다음을 클릭합니다 **새 아이템** 버튼을 클릭합니다.

   ![](/docs/_remote/e3843eed8d736f40c93c6f2e.webp)
2. 다음을 선택합니다 **여러 아이템** 옵션.

   ![](/docs/_remote/444a582037b01c922c963b76.webp)
3. 다음을 클릭합니다 **컴퓨터 찾아보기** 링크를 클릭하여 파일 관리자를 열고 웨어러블이 포함된 모든 ZIP 파일을 선택합니다.

   ![](/docs/_remote/d5fe0416bee0c4f5d15fe1ac.webp)
4. 모든 파일이 올바른지 또는 수정이 필요한지 검토합니다. 이 경우 웨어러블의 모델이 설정되지 않았거나 `wearable.json` 파일에 잘못 설정된 표현이 있습니다.

   ![](/docs/_remote/9af2b80ef49b3ab34f730589.webp)
5. 다음을 클릭하여 오류를 수정합니다 **더 추가** 버튼을 클릭하고 동일한 이름으로 실패한 파일을 다시 업로드하거나, 모달 오른쪽 상단의 휴지통 아이콘을 사용해 오류를 삭제합니다.

   ![](/docs/_remote/bebf438cdc984a5547355b9e.webp)
6. 다음을 클릭하여 모든 웨어러블을 업로드합니다 **아이템 업로드**.

   ![](/docs/_remote/56c36770612421c06d0c4eb5.webp)
7. 잠시 시간이 걸릴 수 있으니 기다려 주세요!

   ![](/docs/_remote/2418ecc5ff115dc5b7837888.webp)
8. 성공했습니다! 이제 아이템을 컬렉션에서 사용할 수 있습니다.

   ![](/docs/_remote/e983b87c56423a0df39dd0f5.webp)
9. 컬렉션이 프로그래밍 방식인지 표준 방식인지 선택합니다. 다음을 확인하세요 [NFT 컬렉션 및 Linked Wearables 컬렉션](#types-of-linked-wearables-collections) 섹션에서 아이템을 만들 컬렉션 유형을 올바르게 설정하세요.

   ![](/docs/_remote/e70222ca6d6afc784bbdbab3.webp)

### 일괄 아이템 업로드 시 일반적인 오류

* 다음 `id` 필드가 다른 웨어러블에서 이미 사용 중인 값으로 설정되었습니다.
* 다음 `id` 필드가 유효한 ID가 아닌 값으로 설정되었습니다. 예를 들어, 서드 파티 ID 또는 컬렉션 ID가 다른 서드 파티 또는 컬렉션에 속합니다.
* ZIP에 `wearable.json` 파일이 없습니다.
* ZIP 파일의 루트 디렉터리에 다음이 없습니다 `wearable.json` 파일.
* 다음 `wearable.json` 의 형식 또는 값이 올바르지 않습니다.
* 파일 크기가 3MB를 초과합니다. Linked Wearables는 일반 웨어러블과 마찬가지로 표준 웨어러블과 동일한 크기 제한을 가집니다.
* 사용자 지정 선택 썸네일 이미지가 PNG 파일이 아닙니다.

## Decentraland에서 웨어러블 보기

Linked Wearables는 게시 및 승인 후 모델이 어떻게 작동할지 검토하기 위해 월드에서 확인할 수 있습니다.

월드에서 웨어러블을 보려면 다음 단계를 따르세요:

1. 월드에서 확인하려는 아이템 오른쪽의 미트볼 메뉴(가로 점 3개)를 클릭합니다. 드롭다운이 나타납니다. 다음을 선택합니다 **Decentraland에서 보기**.

   ![](/docs/_remote/c816f508064933bb206516a8.webp)
2. Decentraland 월드가 열립니다. 백팩으로 이동하여 웨어러블을 확인하세요.

   ![](/docs/_remote/e27530d66df008ecb1568921.webp)


> [!WARNING]
> **⚠️ 안내**: 월드 내 미리보기는 이전 버전의 Decentraland 클라이언트 웹 버전에서 작동합니다. Decentraland Desktop Client 2.0에서는 아직 테스트할 수 없습니다.


## Linked Wearables 편집

## 컬렉션 이름 편집

컬렉션은 생성자가 이름을 변경할 수 있습니다 **컬렉션에 게시된 웨어러블이 없는 경우에만**.

Linked Wearable 컬렉션의 이름을 편집하려면 다음 단계를 따르세요:

1. 컬렉션 이름을 클릭합니다.

   ![](/docs/_remote/39383f19e2bc56803e0a516b.webp)
2. 컬렉션의 새 이름을 선택하고 저장 버튼을 클릭합니다.

   ![](/docs/_remote/30585b27d6df4a4b988c3e13.webp)

## 컬렉션 삭제

컬렉션은 소유자가 삭제할 수 있습니다 **컬렉션에 게시된 웨어러블이 없는 경우에만**.

Linked Wearable 컬렉션을 삭제하려면 다음 단계를 따르세요:

1. 버튼 모음의 맨 오른쪽에 있는 미트볼 메뉴(가로 점 3개)를 클릭합니다. 드롭다운이 나타납니다. 다음을 선택합니다 **삭제**.

   ![](/docs/_remote/72142577aacca52f35921c2a.webp)
2. 확인 모달이 나타납니다. 계속하려면 다음을 클릭합니다 **확인**, 그렇지 않으면 다음을 클릭합니다 **취소**.

   ![](/docs/_remote/7188871c151f9edf2199a42e.webp)

## 단일 웨어러블 편집

### 웨어러블 속성 편집

단일 웨어러블을 편집하려면 다음 단계를 따르세요:

1. 월드에서 확인하려는 아이템 오른쪽의 미트볼 메뉴(가로 점 3개)를 클릭합니다. 드롭다운이 나타납니다. 다음을 선택합니다 **편집기에서 열기**.

   ![](/docs/_remote/abfed6b24f3e8e71867a0391.webp)
2. 일반 웨어러블을 편집하는 방식으로 웨어러블을 편집합니다. 다음을 따르세요 **아이템 편집** 섹션의 [웨어러블 생성 가이드라인](/docs/creator/content-creator-ko/wearables-and-emotes/wearables/creating-wearables/) 에서 사용자 지정 썸네일을 만드는 방법을 확인하세요.

### 링크드 웨어러블 연결 편집

웨어러블과 NFT 컬렉션 간의 연결은 링크드 웨어러블의 가장 중요한 속성 중 하나입니다. NFT와 연결되는 방식을 편집하려면 다른 페이지로 이동할 필요 없이 컬렉션 보기에서 연결 값을 빠르게 변경할 수 있습니다.

![](/docs/_remote/b9bb316d91d55debb38713cb.webp)

"[Linked Wearables는 NFT를 어떻게 표현하나요?](#how-do-linked-wearables-represent-nfts)" 섹션에서 웨어러블 연결 방법에 관한 자세한 정보를 확인하세요.

## 웨어러블 일괄 편집

이전에 살펴본 동일한 방식을 따라 [웨어러블 일괄 생성](#adding-wearables-to-the-linked-wearables-collection-in-bulk) 섹션에서와 같이, 서드파티 관리자는 웨어러블을 일괄 변경할 수 있습니다.

웨어러블을 일괄 변경하려면 변경할 각 웨어러블에 대한 ZIP 파일을 생성해야 합니다.

이 ZIP 파일은 다음에 설명된 형식에 따라 생성해야 합니다 [웨어러블 일괄 생성](#adding-wearables-to-the-linked-wearables-collection-in-bulk) 단, 다음의 경우 한 가지 예외가 있습니다 `wearable.json` 파일에서 `id` 속성은 **반드시** 다음으로 설정되어야 합니다 `id` 또는 변경할 웨어러블의 URN으로 설정해야 합니다. 이는 변경할 웨어러블을 식별할 수 있는 유일한 방법이므로 필수입니다. 에서 ID를 제공하여 웨어러블을 일괄 생성했다면 `wearable.json` 파일에 있는 해당 항목의 `wearable.json` 파일을 재사용하여 업데이트할 수 있습니다.

다음 섹션의 예를 고려하면 [웨어러블 일괄 생성](#adding-wearables-to-the-linked-wearables-collection-in-bulk) 섹션에서, 웨어러블의 일부 속성을 변경하고자 할 때(예: 이름에 숫자를 추가하는 것을 잊은 경우) 다음과 같이 ZIP에 `wearable.json` 파일을 포함해야 합니다. 다음 예시를 참고하세요:

```json
{
  "id": "urn:decentraland:matic:collections-thirdparty:my-third-party:my-collection:1",
  "name": "모자 1",
  "description": "웨어러블에 대한 설명",
  "data": {
    "replaces": [],
    "hides": ["hair"],
    "tags": ["special", "new", "hat"],
    "representations": [
      {
        "bodyShapes": ["urn:decentraland:off-chain:base-avatars:BaseMale"],
        "mainFile": "aMaleModelFile.glb",
        "contents": ["aMaleModelFile.glb", "aTextureFile.png"],
        "overrideHides": [],
        "overrideReplaces": []
      },
      {
        "bodyShapes": ["urn:decentraland:off-chain:base-avatars:BaseFemale"],
        "mainFile": "aFemaleModelFile.glb",
        "contents": ["aFemaleModelFile.glb", "anotherTextureFile.png"],
        "overrideHides": [],
        "overrideReplaces": []
      }
    ],
    "category": "모자"
  },
  "mapping": {
    "type": "single",
    "id": "123"
  }
}
```

여기서 `id` 필드는 다음으로 설정됩니다 `id` 또는 변경할 웨어러블의 URN이며, `이름` 필드는 웨어러블의 새 이름으로 설정됩니다.

ZIP 파일이 준비되면 다음 단계에 따라 항목을 일괄 편집하세요:

1. 버튼 모음의 맨 오른쪽에 있는 미트볼 메뉴(가로 점 3개)를 클릭합니다. 드롭다운이 나타납니다. 다음을 선택합니다 **일괄 편집**.

   ![](/docs/_remote/0c8a87dd214ea40c8a6ed252.webp)
2. 다음 항목의 모달과 유사한 모달이 **모델 일괄 업로드** 표시됩니다. 다음을 클릭하세요 **컴퓨터에서 찾아보기** 링크를 클릭하여 파일 관리자를 여세요.

   ![](/docs/_remote/a39bb531aa4d0dbae170bd92.webp)
3. 편집할 항목의 모든 ZIP 파일을 선택하세요.

   ![](/docs/_remote/d5fe0416bee0c4f5d15fe1ac.webp)
4. 모든 파일이 올바른지 또는 수정이 필요한지 검토합니다. 이 경우 웨어러블의 모델이 설정되지 않았거나 `wearable.json` 파일에 잘못 설정된 표현이 있습니다.

   ![](/docs/_remote/4a7baded9874d3cc155f2fbd.webp)
5. 다음을 클릭하여 오류를 수정합니다 **더 추가** 버튼을 클릭하고 동일한 이름으로 실패한 파일을 다시 업로드하거나, 모달 오른쪽 상단의 휴지통 아이콘을 사용해 오류를 삭제합니다.

   ![](/docs/_remote/bebf438cdc984a5547355b9e.webp)
6. 다음을 클릭하여 모든 웨어러블을 업로드합니다 **아이템 업로드**.

   ![](/docs/_remote/42f683d2a6f504f05485aec8.webp)
7. 잠시 시간이 걸릴 수 있으니 기다려 주세요!

   ![](/docs/_remote/5894f0726ee565293b5d41a2.webp)
8. 성공했습니다! 이제 항목을 컬렉션에서 사용할 수 있습니다. 큐레이터의 업데이트는 포럼 게시물에서 확인하세요.

   ![](/docs/_remote/fb076a01ef98b4db599d68cf.webp)

## 링크드 웨어러블 게시하기

링크드 웨어러블은 일반 웨어러블과 마찬가지로 게시 및 큐레이션 과정을 거쳐야 합니다. 게시 및 큐레이션 과정은 일반 웨어러블의 과정과 동일하지는 않지만, 같은 단계를 따릅니다. 모든 항목은 지정된 큐레이터의 큐레이션을 받기 전에 먼저 게시되어야 합니다.

다음 섹션에서는 큐레이션을 위해 링크드 웨어러블을 게시하는 방법을 안내합니다.

## 비용

링크드 웨어러블 생성 비용은 선택한 링크드 웨어러블 컬렉션 유형에 따라 달라집니다:

1. **표준**: 게시되는 웨어러블 하나당 일반 웨어러블과 동일한 비용이 발생합니다: MANA로 지불하는 미화 100달러.
2. **프로그래밍 방식**: 게시할 모든 웨어러블에 대해 한 번만 지불하는 고정 수수료입니다. 이 수수료는 일반 웨어러블 20개를 게시하는 비용과 동일하며, MANA로 지불하는 미화 2,000달러입니다.

생성 중인 컬렉션 유형에 관한 자세한 정보는 다음을 확인하세요 [NFT 컬렉션 및 Linked Wearables 컬렉션](#types-of-linked-wearables-collections) 섹션.

## 검토를 위한 웨어러블 게시

웨어러블이 준비되면 큐레이션을 위해 게시해야 합니다. 웨어러블은 항목 그룹 단위로 게시되며, 항목을 선택하고 다음을 클릭하여 큐레이션 준비가 된 항목을 선택할 수 있습니다 `게시` 버튼. 항목을 게시한 후에는 이미 게시된 항목이 큐레이션될 때까지 추가 게시가 차단됩니다.

웨어러블을 게시하려면 다음을 수행해야 합니다:

1. 게시할 항목 옆의 체크박스를 클릭하여 선택하세요. 선택을 마쳤으면 다음을 클릭하세요 **게시** 버튼.

   ![](/docs/_remote/52a58c4e5c9954ebc8fc269b.webp)
2. 컬렉션 이름을 확인하세요. 항목을 게시한 후에는 컬렉션 이름을 변경할 수 없으므로 꼼꼼히 확인하세요.

   ![](/docs/_remote/52a58c4e5c9954ebc8fc269b.webp)
3. 게시할 항목을 확인하세요. 다음을 클릭하세요 **항목 확인** 준비가 되면 버튼을 클릭하세요.

   ![](/docs/_remote/8a8e23ad31bfa01cd5132c54.webp)
4. 지갑에서 항목 게시 확인에 서명하세요.

   ![](/docs/_remote/88c6a6a00343c5fac618b93d.webp)
5. 이용 약관을 읽고 확인하세요.

   ![](/docs/_remote/5eeaa4e5bf1e5c04ddcfe174.webp)
6. 게시 수수료를 확인하세요. 웨어러블 게시에 필요한 수수료는 다음에 설명되어 있습니다 [비용 섹션](#costs).

   ![](/docs/_remote/763b602c672547a4d2a6101b.webp)
7. 링크드 웨어러블을 처음 게시하는 경우, 링크드 웨어러블 스마트 컨트랙트가 귀하를 대신하여 MANA를 처리하도록 승인해야 합니다. 이 단계는 게시 수수료를 지불하는 데 사용되는 MANA를 지갑에서 차감하기 위해 필요합니다.

   ![](/docs/_remote/86176e8cb7ef6bc5bbe8c139.webp)
8. 게시 수수료를 지불하고 트랜잭션을 수행하여 게시를 완료하세요. 네트워크 혼잡도에 따라 다소 시간이 걸릴 수 있습니다.

   ![](/docs/_remote/bdbd4028179d589523236310.webp)
9. 성공했습니다! 항목이 게시되었습니다. 항목은 일반 컬렉션과 동일한 큐레이션 과정을 거칩니다. 포럼 게시물을 통해 큐레이터와 소통할 수 있습니다.

   ![](/docs/_remote/1c4b8bde3d0569bee971bd77.webp)

## 검토를 위한 변경 사항 제출

게시 및 승인된 웨어러블을 편집한 경우 다시 큐레이션 과정을 거쳐야 합니다. 걱정하지 마세요. 이미 게시된 웨어러블에는 수수료가 적용되지 않습니다.

변경 사항을 제출하여 큐레이션을 받으려면 다음을 수행해야 합니다:

1. 변경된 항목 옆의 체크박스를 클릭하여 선택하세요. 다음을 클릭하세요 **변경 사항 제출** 버튼.

   ![](/docs/_remote/2cb347394e74593c7becf04c.webp)
2. 변경 사항 제출 절차를 진행하세요. 모달에 표시된 대로, 이러한 변경 사항은 다시 한번 큐레이션 단계를 거쳐야 합니다.

   ![](/docs/_remote/3a60e8ad8fb155002a8e4cef.webp)
3. 이용 약관을 읽고 확인하세요. 약관에 동의하면 항목은 다시 큐레이션을 받을 준비가 됩니다.

   ![](/docs/_remote/14b970d046bc0d27c4efbb13.webp)

## 큐레이션

일반 웨어러블과 마찬가지로 3D 모델은 큐레이터 위원회의 승인을 받아야 합니다. Decentraland의 미적 요소와 게임플레이를 계속 보호해야 하므로 이 규칙의 예외가 될 수 없습니다.

큐레이션 과정은 웨어러블을 생성하는 데 사용된 방식에 따라 달라집니다. 링크드 웨어러블 컬렉션은 수작업 웨어러블과 프로그래밍 방식으로 생성된 웨어러블을 허용합니다.

### 수작업 웨어러블

자동화된 과정 없이 개별적으로 제작된 3D 모델(대부분의 일반 웨어러블에 사용되는 방식)의 경우, 큐레이터는 모든 항목이 다음을 준수하는지 확인하기 위해 컬렉션의 모든 항목을 개별적으로 검토해야 합니다 [웨어러블 가이드라인](/docs/creator/content-creator-ko/wearables-and-emotes/wearables/creating-wearables/).

### 프로그래밍 방식 컬렉션

프로그래밍 방식 컬렉션의 경우 모든 항목을 개별적으로 큐레이션할 필요는 없습니다. 각 컬렉션에서 큐레이션할 항목 수는 컬렉션 규모에 따라 달라지며, 이는 DAO에서 다음을 통해 정의되었습니다 [이 제안](https://governance.decentraland.org/proposal/?id=f69c4d40-aaaf-11ec-87a7-6d2a41508231).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/wearables-and-emotes/wearables/linked-wearables.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
