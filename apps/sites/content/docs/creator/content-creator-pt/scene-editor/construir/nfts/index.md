> Adicionando retratos de NFT às suas scenes

# NFTs

Você pode adicionar NFTs (Tokens Não Fungíveis) à sua cena, exibidos como molduras de quadros.

Todos os formatos de imagem e GIF suportados no OpenSea também são suportados pela Decentraland em molduras de quadros. NFTs em formato de vídeo ou áudio não são atualmente suportados. NFTs que também têm representações 3D, como wearables da Decentraland, são exibidos em molduras de quadros como imagens 2D.

## Adicionando um NFT

Use o smart item NFT, que pode encontrar no **Smart Items** asset pack, ou simplesmente pesquisando *NFT* na barra de pesquisa acima. Depois de arrastar uma cópia do item NFT para a sua cena e selecioná-lo, há alguns campos que pode configurar.

![](/docs/_remote/e6ffb898b183dc0d0a5e93be.webp)

Os principais campos a configurar determinam que NFT exibir:

* **Rede**: A rede blockchain em que o seu NFT está. Usa, por predefinição, a mainnet da Ethereum, mas também pode escolher Polygon (matic) ou uma das várias redes de teste.
* **Contrato**: O endereço do contrato da coleção à qual este NFT pertence (por exemplo: Cryptokitties, SuperRare, Decentraland Halloween Wearables 2019, etc.)
* **Token**: O id único deste NFT específico

Para obter estes, a forma mais simples é procurá-los no Decentraland Marketplace e depois verificar o URL. Por exemplo, a partir do URL do seguinte item:

*<https://market.decentraland.org/contracts/0xb932a70a57673d89f4acffbe830e8ed7f75fb9e0/tokens/20175>*

Pode inferir que o contrato é *0xb932a70a57673d89f4acffbe830e8ed7f75fb9e0* (referindo-se a SuperRare) e o ID é *20175*.

De forma semelhante, também pode obter estes a partir do URL OpenSea do token. Por exemplo, a partir do URL do seguinte item:

*<https://opensea.io/assets/0x31385d3520bced94f77aae104b406994d8f2168c/2614>*

Pode inferir que o contrato é *0x31385d3520bced94f77aae104b406994d8f2168c* e o ID é *2614*.

Outros campos opcionais que podem ser configurados no smart item NFT são:

* **Estilo da moldura**: O estilo de moldura predefinido tem uma margem luminosa que pode não corresponder ao estilo da obra de arte ou da sua cena. Há várias outras opções para escolher, com estilos variados, do barroco ao minimalista, ou até fita nos cantos da pintura.
* **Cor**: NFTs com fundo transparente recebem uma cor de fundo, violeta por defeito. Pode escolher qualquer outra cor. Note que alguns estilos de moldura, como *Nenhuma*, não incluem qualquer cor de fundo.


> [!WARNING]
> **📔 Nota**: Só pode ver estas alterações entrarem em vigor ao entrar na cena no modo Preview. Nenhuma destas alterações modifica a representação do smart item que arrasta em modo de edição.


Veja [Exibir um NFT](/docs/creator/content-creator-pt/scenes-sdk7/media/display-a-certified-nft/) para mais detalhes sobre como a Decentraland lida com retratos NFT.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scene-editor/construir/nfts.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
