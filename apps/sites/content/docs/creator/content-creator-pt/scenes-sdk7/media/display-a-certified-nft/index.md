> Saiba como exibir em sua scene um NFT certificado que você possui

# Exibir um NFT Certificado

Você pode exibir um NFT 2D (Token Não Fungível) que você possui em suas cenas do Decentraland.

A imagem do NFT e outros dados são obtidos de uma API, com base no contrato e no id do token. Qualquer NFT suportado em [OpenSea](https://opensea.io/) também pode ser exibido em uma moldura de imagem de NFT no Decentraland.


> [!NOTE]
> **💡 Dica**: Na [Scene Editor no Creator Hub](/docs/creator/content-creator-pt/scene-editor/primeiros-passos/about-editor/), você pode usar um **NFT** [Smart Item](/docs/creator/content-creator-pt/scene-editor/interatividade/smart-items/) para uma maneira sem código de conseguir isso.


A moldura de imagem é exibida ajustando-se às dimensões da imagem do NFT. Se as dimensões da imagem forem 512 x 512 pixels, a moldura mantém seu tamanho original. Se a imagem tiver dimensões diferentes, a moldura será redimensionada e esticada para corresponder a essas dimensões.


> [!NOTE]
> **💡 Dica**: Se você quiser esticar ou redimensionar a imagem a partir do que é gerado por padrão, você pode alterar a `scale` propriedade na entidade. `Transform` component.


## Adicionar um NFT

Adicione um `NftShape` componente a uma entidade para exibir um token 2D na sua cena.

```ts
const nft = engine.addEntity()

Transform.create(nft, {
	position: Vector3.create(8, 1, 8),
})

NftShape.create(nft, {
	urn: 'urn:decentraland:ethereum:erc721:0x06012c8cf97bead5deae237070f9587f8e7a266d:558536',
})
```

A `NftShape` componente deve ser instanciado com um parâmetro que inclua o seguinte:

* `urn` campo. Este campo recebe uma string que deve seguir esta estrutura:

`urn:decentraland:<CHAIN>:<CONTRACT_STANDARD>:<CONTRACT_ADDRESS>:<TOKEN_ID>`

Essa string inclui:

* A *network* onde este token existe. Veja abaixo a lista de valores suportados.
* A *padrão do contrato* em que este token se baseia, por exemplo `erc721`
* A *contrato* do token (por exemplo, o contrato CryptoKitties)
* A *id* do token específico a ser exibido

Por exemplo:

`urn:decentraland:ethereum:erc721:0x06012c8cf97BEaD5deAe237070F9587f8E7A266d:558536`

O exemplo acima busca um NFT com o endereço do contrato `0x06012c8cf97BeaD5deAe237070F9587f8E7A266d`, e o identificador específico `558536`. O ativo correspondente pode ser encontrado no OpenSea em [https://opensea.io/assets/0x06012c8cf97BeaD5deAe237070F9587f8E7A266d/558536](https://opensea.io/assets/0x06012c8cf97BEaD5deAe237070F9587f8E7A266d/558536).

Os seguintes valores são suportados para *network*:

* `ethereum`: Ethereum Mainnet
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
> **📔 Nota**: As imagens devem ter no máximo **6 MB**. Se a imagem for maior, ela não será renderizada na cena.


## Personalizar a moldura

Por padrão, a imagem terá um fundo roxo e uma moldura com uma textura emissiva pulsante ao redor. Você pode definir as seguintes propriedades para corresponder melhor ao estilo do NFT e da cena:

* `cor`: Determina a parte traseira do modelo e também o fundo da imagem caso a imagem do NFT tenha transparência.
* `style`: Seleciona um modelo de moldura de um enum com várias opções predefinidas. Use um valor do enum `NftFrameType`, que contém uma lista de todos os estilos disponíveis.

```ts
NftShape.create(nftEntity, {
  urn: 'urn:decentraland:ethereum:erc721:0x06012c8cf97bead5deae237070f9587f8e7a266d:558536',
  color: Color3.Green(),
  style: NftFrameType.NFT_GOLD_EDGES,
})
```

![](/docs/_remote/8698f94b4eeaa8c4125b5ae6.webp)

Aqui está a lista completa de estilos de moldura suportados:

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

Algumas molduras usam mais materiais do que outras. Por exemplo, a moldura padrão adiciona 1 material para o próprio NFT, 1 material para um plano de fundo colorido e 2 materiais para a moldura (compartilhados com outras molduras do mesmo estilo). Se você precisar reduzir os materiais para [ficar dentro dos limites da cena](/docs/creator/content-creator-pt/scenes-sdk7/otimizacao/scene-limitations/), escolha um estilo mais simples. Por exemplo, "none" usa apenas 1 material para o próprio NFT.


> [!NOTE]
> **💡 Dica**: Usando o Visual Studio Code (ou outro IDE), veja a lista completa digitando `NftFrameType.` e aguardando as sugestões inteligentes exibirem a lista de opções. Use `NftFrameType.NFT_NONE`para exibir o NFT simples como está, sem moldura ou cor de fundo.


## Abrir uma UI de NFT

Abra uma UI predefinida que exibe o nome, o proprietário e a descrição de um NFT. Ela também inclui o preço atual do NFT e o preço da última venda, se aplicável, e um botão que leva à página do NFT no OpenSea, onde há mais informações e ele pode ser comprado.

![](/docs/_remote/2e438b55141b8a0db1d92f61.webp)

Abra esta UI chamando a função `openNftDialog()`. Essa função requer um objeto como argumento que contenha um único `urn` campo. Este campo recebe uma string que deve seguir esta estrutura:

`urn:decentraland:<CHAIN>:<CONTRACT_STANDARD>:<CONTRACT_ADDRESS>:<TOKEN_ID>`

Por exemplo:

`urn:decentraland:ethereum:erc721:0x00...000:123`


> [!WARNING]
> **📔 Nota**: A UI deve ser aberta como resultado de um evento de botão, para evitar spam abusivo. O evento de botão não precisa necessariamente estar na mesma moldura de imagem nem em uma `NftShape`.


Para abrir esta UI como resultado de uma ação de clique, adicione o seguinte:

```ts
import { openNftDialog } from '~system/RestrictedActions'

// Adicione um collider de pointer para que ele possa ser clicado
MeshCollider.setBox(myEntity, ColliderLayer.CL_POINTER)

// Adicione o evento de callback onPointerDown.
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
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/media/display-a-certified-nft.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
