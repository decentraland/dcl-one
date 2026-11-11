> 씬에 보유한 인증된 NFT를 표시하는 방법을 알아봅니다

# 인증된 NFT 표시

보유한 2D NFT(대체 불가능 토큰)를 Decentraland 씬에 표시할 수 있습니다.

NFT의 이미지와 기타 데이터는 토큰의 계약과 ID를 기반으로 API에서 가져옵니다. 지원되는 모든 NFT는 [OpenSea](https://opensea.io/) 를 Decentraland의 NFT 액자에 표시할 수도 있습니다.


> [!NOTE]
> **💡 팁**: [Creator Hub의 씬 에디터에 추가한 뒤](/docs/creator/content-creator-ko/scene-editor/get-started/about-editor/), 다음을 사용할 수 있습니다 **NFT** [스마트 아이템](/docs/creator/content-creator-ko/scene-editor/interactivity/smart-items/) 이것을 노코드 방식으로 구현하려면 이 방법을 사용하세요.


액자는 NFT 이미지의 크기에 맞게 조정되어 표시됩니다. 이미지의 크기가 512 X 512픽셀인 경우, 액자는 원래 크기를 유지합니다. 이미지의 크기가 다른 경우, 액자는 해당 크기에 맞게 조정되고 늘어납니다.


> [!NOTE]
> **💡 팁**: 기본적으로 생성된 이미지에서 늘리거나 크기를 조정하고 싶다면, `크기` 속성을 엔티티의 `Transform` 컴포넌트도 부여해야 합니다.


## NFT 추가

다음 항목을 추가하세요: `NftShape` 컴포넌트를 엔티티에 추가하여 씬에 2D 토큰을 표시합니다.

```ts
const nft = engine.addEntity()

Transform.create(nft, {
	position: Vector3.create(8, 1, 8),
})

NftShape.create(nft, {
	urn: 'urn:decentraland:ethereum:erc721:0x06012c8cf97bead5deae237070f9587f8e7a266d:558536',
})
```

해당 `NftShape` 컴포넌트는 다음을 포함하는 매개변수로 인스턴스화해야 합니다:

* `urn` 필드입니다. 이 필드에는 다음 구조를 따라야 하는 문자열이 들어갑니다:

`urn:decentraland:<CHAIN>:<CONTRACT_STANDARD>:<CONTRACT_ADDRESS>:<TOKEN_ID>`

이 문자열에는 다음이 포함됩니다:

* 해당 *네트워크* 이 토큰이 존재하는 곳입니다. 아래 지원되는 값 목록을 참조하세요.
* 해당 *계약 표준* 이 토큰이 기반으로 하는 표준입니다. 예를 들어 `erc721`
* 해당 *계약* 토큰의 계약(예: CryptoKitties 계약)
* 해당 *id* 표시할 특정 토큰의

예를 들면:

`urn:decentraland:ethereum:erc721:0x06012c8cf97BEaD5deAe237070F9587f8E7A266d:558536`

위 예시는 계약 주소가 `0x06012c8cf97BEaD5deAe237070F9587f8E7A266d`이고, 특정 식별자가 `558536`인 NFT를 가져옵니다. 해당 자산은 OpenSea에서 다음 주소로 찾을 수 있습니다. [https://opensea.io/assets/0x06012c8cf97bead5deae237070f9587f8e7a266d/558536](https://opensea.io/assets/0x06012c8cf97BEaD5deAe237070F9587f8E7A266d/558536).

다음 값이 지원됩니다: *네트워크*:

* `ethereum`: Ethereum 메인넷
* `matic`: Polygon Matic
* `klaytn`: Klaytn
* `bsc`: BNB Chain
* `arbitrum`: Arbitrum
* `arbitrum_nova`: Arbitrum Nova
* `avalanche`: Avalanche
* `optimism`: Optimism
* `solana`: Solana
* `base`: Base
* `blast`: Blast
* `zora`: Zora


> [!WARNING]
> **📔 참고**: 이미지의 최대 크기는 **6 MB**입니다. 이미지가 더 크면 씬에 렌더링되지 않습니다.


## 프레임 사용자 지정

기본적으로 이미지는 보라색 배경을 가지며, 그 주위에 맥박치는 발광 텍스처가 있는 프레임이 표시됩니다. NFT와 씬의 스타일에 더 잘 맞도록 다음 속성을 설정할 수 있습니다:

* `색상`: 모델의 뒷면과, NFT 이미지에 투명도가 있는 경우 이미지의 배경을 결정합니다.
* `스타일`: 미리 정의된 여러 옵션이 들어 있는 열거형에서 프레임 모델을 선택합니다. 다음 열거형의 값을 사용하세요 `NftFrameType`, 여기에는 사용 가능한 모든 스타일 목록이 들어 있습니다.

```ts
NftShape.create(nftEntity, {
  urn: 'urn:decentraland:ethereum:erc721:0x06012c8cf97bead5deae237070f9587f8e7a266d:558536',
  color: Color3.Green(),
  style: NftFrameType.NFT_GOLD_EDGES,
})
```

![](/docs/_remote/8698f94b4eeaa8c4125b5ae6.webp)

지원되는 전체 프레임 스타일 목록은 다음과 같습니다:

* `NFT_CLASSIC`
* `NFT_BAROQUE_ORNAMENT`
* `NFT_DIAMOND_ORNAMENT`
* `NFT_MINIMAL_WIDE`
* `NFT_MINIMAL_GREY`
* `NFT_BLOCKY`
* `NFT_GOLD_EDGES`
* `NFT_GOLD_CARVED`
* `NFT_GOLD_WIDE`
* `NFT_GOLD_ROUNDED`
* `NFT_METAL_MEDIUM`
* `NFT_METAL_WIDE`
* `NFT_METAL_SLIM`
* `NFT_METAL_ROUNDED`
* `NFT_PINS`
* `NFT_MINIMAL_BLACK`
* `NFT_MINIMAL_WHITE`
* `NFT_TAPE`
* `NFT_WOOD_SLIM`
* `NFT_WOOD_WIDE`
* `NFT_WOOD_TWIGS`
* `NFT_CANVAS`
* `NFT_NONE`

일부 프레임은 다른 프레임보다 더 많은 재질을 사용합니다. 예를 들어, 기본 프레임은 NFT 자체에 1개의 재질, 배경색이 있는 평면에 1개의 재질, 그리고 프레임에 2개의 재질(같은 스타일의 다른 액자와 공유됨)을 추가합니다. 재질 수를 줄여 [씬 제한 내에 머물러야 한다면](/docs/creator/content-creator-ko/sdk7/optimizing/scene-limitations/), 더 단순한 스타일을 선택하세요. 예를 들어 "none"은 NFT 자체에만 1개의 재질을 사용합니다.


> [!NOTE]
> **💡 팁**: Visual Studio Code(또는 다른 IDE)를 사용한다면, 다음을 입력하여 전체 목록을 확인하세요. `NftFrameType.` 를 입력하고 스마트 제안이 옵션 목록을 표시할 때까지 기다리세요. 다음을 사용하세요 `NftFrameType.NFT_NONE`을 사용하면 프레임이나 배경색 없이 평범한 NFT를 그대로 표시할 수 있습니다.


## NFT UI 열기

NFT의 이름, 소유자, 설명을 표시하는 미리 제작된 UI를 엽니다. 또한 해당되는 경우 NFT의 현재 가격과 마지막 판매 가격도 포함되며, 더 많은 정보를 볼 수 있고 구매할 수 있는 OpenSea의 NFT 페이지로 연결되는 버튼이 있습니다.

![](/docs/_remote/2e438b55141b8a0db1d92f61.webp)

다음 함수를 호출하여 이 UI를 엽니다. `openNftDialog()`. 이 함수는 인수로 하나의 `urn` 필드입니다. 이 필드에는 다음 구조를 따라야 하는 문자열이 들어갑니다:

`urn:decentraland:<CHAIN>:<CONTRACT_STANDARD>:<CONTRACT_ADDRESS>:<TOKEN_ID>`

예를 들면:

`urn:decentraland:ethereum:erc721:0x00...000:123`


> [!WARNING]
> **📔 참고**를 포함하는 객체가 필요합니다: 남용성 스팸을 방지하기 위해 UI는 버튼 이벤트의 결과로 열려야 합니다. 버튼 이벤트는 반드시 동일한 액자나 `NftShape`.


클릭 동작의 결과로 이 UI를 열려면 다음을 추가하세요:

```ts
import { openNftDialog } from '~system/RestrictedActions'

// 클릭할 수 있도록 포인터 콜라이더 추가
MeshCollider.setBox(myEntity, ColliderLayer.CL_POINTER)

// onPointerDown 콜백 이벤트 추가.
pointerEventsSystem.onPointerDown(
	{
		entity: myEntity,
		opts: { button: InputAction.IA_PRIMARY, hoverText: 'Click' },
	},
	function () {
		openNftDialog({
			urn: 'urn:decentraland:ethereum:erc721:0x06012c8cf97BEaD5deAe237070F9587f8E7A266d:558536',
		})
	}
)
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/media/display-a-certified-nft.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
