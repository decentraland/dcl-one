# Coleções

Coleções são grupos de [wearables](/docs/contributor/contributor-pt/conteudo/tipos-de-entity/wearables/) e [emotes](/docs/contributor/contributor-pt/conteudo/tipos-de-entity/emotes/) definidos no mesmo espaço de nomes off-chain ou contrato on-chain.

Elas não são [entidades](/docs/contributor/contributor-pt/conteudo/entities/) por si só, mas seus itens individuais podem ser obtidos dos servidores de conteúdo normalmente usando [pointers](/docs/contributor/contributor-pt/conteudo/pointers/).

### URNs e Pointers <a href="#pointers" id="pointers"></a>

As Coleções são identificadas usando URNs, que são prefixos dos URNs de seus itens. Existem 4 tipos gerais de URNs:

* `v1` coleções com conteúdo aprovado pela Foundation (obsoletas).
* `v2` coleções com conteúdo aprovado pela comunidade.
* `thirdparty` coleções com itens associados a NFTs fora do Decentraland.
* `off-chain` coleções, contendo principalmente itens padrão para novos avatares.


> [!NOTE]
> Apesar do fato de que `v1` as coleções estão obsoletas e não serão mais criadas ou atualizadas, elas ainda são válidas e seus itens podem ser usados.


#### Coleções das versões 1 e 2

Tanto o antigo `v1` quanto o atual `v2` espaços de nomes de coleção têm URNs da mesma forma:

```
urn:decentraland:<blockchain>:<collections-version>:<contract-address>
```

Por exemplo:

```
# Uma coleção no espaço v1 obsoleto:
urn:decentraland:mainnet:collections-v1:DCL Test Masks

# Uma coleção no espaço v2 aprovado pela comunidade:
urn:decentraland:matic:collections-v2:0x25a1d66891d44cdf7b8c45802489c1dea7aadf8b
```

Se o `:<id>` segmento é acrescentado ao final, a URN se torna um [pointer](/docs/contributor/contributor-pt/conteudo/pointers/) para um item dentro da coleção. Veja [wearable pointers](/docs/contributor/contributor-pt/conteudo/tipos-de-entity/wearables/#pointers) e [emote pointers](/docs/contributor/contributor-pt/conteudo/tipos-de-entity/emotes/#pointers) para informações sobre isso.

#### Third-party Collections

As collections de terceiros, também conhecidas como [wearables vinculados](https://github.com/decentraland/docs/tree/main/creator/wearables-and-emotes/wearables/linked-wearables.md), são diferentes dos wearables normais porque estão ligadas a NFTs existentes fora do Decentraland. Elas permitem que os jogadores exibam NFTs do seu conjunto pessoal em seus avatares no mundo.

Suas URNs têm esta aparência:

```
urn:decentraland:<blockchain>:collections-thirdparty:<third-party-id>:<collection-id>
```

Você pode encontrar muitas informações sobre isso na [wearables vinculados](https://github.com/decentraland/docs/tree/main/creator/wearables-and-emotes/wearables/linked-wearables.md) documentação para criadores.

### Descobrindo Collections

Os content servers podem fornecer uma lista de todas as collections conhecidas, usando o [`/lambdas/collections`](https://decentraland.github.io/catalyst-api-specs/#tag/Lambdas/operation/getCollections) endpoint.

A resposta será um objeto com uma `collections` propriedade, contendo um array de URNs e nomes. Por exemplo:

```json
{
  "collections": [
    {
      "id": "urn:decentraland:amoy:collections-v2:0xff5d4ebc6bc1ff7262cab42d3c693d953f4614d2",
      "name": "Coleção de roupas de inverno"
    },
    // ... muitas mais
  ]
}
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/conteudo/collections.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
