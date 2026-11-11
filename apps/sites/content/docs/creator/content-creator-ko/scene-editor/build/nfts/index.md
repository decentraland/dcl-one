> 씬에 NFT 초상화 추가하기

# NFT

NFT(대체 불가능 토큰)를 씬에 추가하여 액자처럼 표시할 수 있습니다.

OpenSea에서 지원되는 모든 이미지 및 GIF 형식은 Decentraland의 액자에서도 지원됩니다. 비디오 또는 오디오 형식의 NFT는 현재 지원되지 않습니다. Decentraland 웨어러블처럼 3D 표현도 있는 NFT는 액자에서 2D 이미지로 표시됩니다.

## NFT 추가하기

다음에서 찾을 수 있는 NFT 스마트 아이템을 사용하세요. **스마트 아이템** 에셋 팩에서 찾거나, 또는 검색하여 *NFT* 위의 검색창에서 찾을 수 있습니다. NFT 아이템의 복사본을 씬으로 끌어다 놓고 선택하면 구성할 수 있는 몇 가지 필드가 있습니다.

![](/docs/_remote/e6ffb898b183dc0d0a5e93be.webp)

구성해야 할 주요 필드는 어떤 NFT를 표시할지 결정합니다:

* **네트워크**: NFT가 속한 블록체인 네트워크입니다. 기본적으로 이더리움 메인넷을 사용하지만, Polygon(matic)이나 여러 테스트 네트워크 중 하나를 선택할 수도 있습니다.
* **컨트랙트**: 이 NFT가 속한 컬렉션의 컨트랙트 주소입니다(예: Cryptokitties, SuperRare, Decentraland Halloween Wearables 2019 등).
* **토큰**: 이 특정 NFT의 고유 ID입니다

이를 확인하는 가장 간단한 방법은 Decentraland 마켓플레이스에서 찾아보고 URL을 확인하는 것입니다. 예를 들어, 다음 항목의 URL에서:

*<https://market.decentraland.org/contracts/0xb932a70a57673d89f4acffbe830e8ed7f75fb9e0/tokens/20175>*

컨트랙트는 *0xb932a70a57673d89f4acffbe830e8ed7f75fb9e0* (SuperRare를 가리키며) 그리고 ID는 *20175*.

마찬가지로, 토큰의 OpenSea URL에서도 이를 얻을 수 있습니다. 예를 들어, 다음 항목의 URL에서:

*<https://opensea.io/assets/0x31385d3520bced94f77aae104b406994d8f2168c/2614>*

컨트랙트는 *0x31385d3520bced94f77aae104b406994d8f2168c* 그리고 ID는 *2614*.

NFT 스마트 아이템에서 구성할 수 있는 다른 선택 필드는 다음과 같습니다:

* **프레임 스타일**: 기본 프레임 스타일에는 작품이나 씬의 스타일과 어울리지 않을 수 있는 빛나는 테두리가 있습니다. 바로크부터 미니멀리스트까지 다양한 스타일의 여러 다른 옵션이 있으며, 그림 모서리에 테이프를 붙인 것 같은 스타일도 있습니다.
* **색상**: 투명한 배경의 NFT에는 배경색이 지정되며, 기본값은 보라색입니다. 다른 색상도 선택할 수 있습니다. 다음과 같은 일부 프레임 스타일은 *없음*, 아예 배경색이 포함되지 않습니다.


> [!WARNING]
> **📔 참고**: 이러한 변경 사항은 미리보기 모드에서 씬에 들어갔을 때만 적용된 것을 볼 수 있습니다. 이러한 변경 사항은 편집 모드에서 드래그하는 스마트 아이템의 표현을 수정하지 않습니다.


참고 [NFT 표시하기](/docs/creator/content-creator-ko/sdk7/media/display-a-certified-nft/) 에서 Decentraland가 NFT 초상화를 처리하는 방법에 대한 자세한 내용을 확인하세요.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/scene-editor/build/nfts.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
