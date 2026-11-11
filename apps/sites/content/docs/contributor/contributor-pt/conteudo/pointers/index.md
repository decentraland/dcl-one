# Ponteiros

Ponteiros são strings únicas e sem distinção entre maiúsculas e minúsculas que fazem referência a uma entity ativa [entity](/docs/contributor/contributor-pt/conteudo/entities/). Os content servers podem resolver esses ponteiros para obter o identificador da entity.

Lembre-se de que tanto [entidades](/docs/contributor/contributor-pt/conteudo/entities/) e suas [files](/docs/contributor/contributor-pt/conteudo/file-system/) são imutáveis no sistema de conteúdo da Decentraland, e seus identificadores mudam quando versões de substituição são enviadas. Os ponteiros, por outro lado, são referências estáveis que persistem entre substituições. Isso é alcançado redirecionando automaticamente o ponteiro para uma nova entity quando o proprietário envia uma substituição.

Vários ponteiros podem resolver para a mesma [entity](/docs/contributor/contributor-pt/conteudo/entities/), como costuma acontecer com [que as cenas](/docs/contributor/contributor-pt/conteudo/tipos-de-entity/scenes/).

Como você pode perceber, o uso mais comum de ponteiros é obter o ID da versão ativa (isto é, a substituição mais recente) de um [entity](/docs/contributor/contributor-pt/conteudo/entities/), para baixá-lo.

### Tipos de Ponteiros

Há 4 tipos de ponteiros que os content servers podem resolver para entities, cada um com sua própria sintaxe:

* [Ponteiros de Scene](/docs/contributor/contributor-pt/conteudo/tipos-de-entity/scenes/#pointers) são coordenadas de parcel, como `"0,0"`.
* [Ponteiros de profile](/docs/contributor/contributor-pt/conteudo/tipos-de-entity/profiles/#pointers) são o endereço Ethereum do proprietário.
* [Ponteiros de Wearables](/docs/contributor/contributor-pt/conteudo/tipos-de-entity/wearables/#pointers) e [ponteiros de emotes](/docs/contributor/contributor-pt/conteudo/tipos-de-entity/emotes/#pointers) são, ou [collection](/docs/contributor/contributor-pt/conteudo/collections/) URNs de item ou off-chain.
* [Ponteiros da store](/docs/contributor/contributor-pt/conteudo/tipos-de-entity/stores/#pointers) são URNs off-chain com o endereço Ethereum do proprietário.
* [Ponteiros de outfits](/docs/contributor/contributor-pt/conteudo/tipos-de-entity/outfits/#pointers) são o endereço Ethereum do proprietário com o sufixo `:outfits`.

Você pode encontrar mais detalhes nas seções específicas.

### Resolvendo Ponteiros

Os content servers têm um endpoint que pode resolver ponteiros para o manifest da entity ativa, fazendo uma `GET` requisição para:

```
https://<content-base-url>/entities/<entity-type>?pointer=<pointer>
```


> [!NOTE]
> Por motivos históricos, a resposta deste endpoint é um array contendo um elemento.


Alguns exemplos úteis, usando o content server da Foundation:

```bash
# Um ponteiro on-chain de coleção para um wearable:
curl "https://peer.decentraland.org/content/entities/wearables/?pointer=urn:decentraland:matic:collections-v2:0x30517529cb5c16f686c6d0b48faae5d250d43005:0"

# Um ponteiro off-chain de coleção para um asset padrão:
curl https://peer.decentraland.org/content/entities/wearables/?pointer=urn:decentraland:off-chain:base-avatars:BaseFemale

# Um ponteiro de parcel:
curl https://peer.decentraland.org/content/entities/wearables/?pointer=0,0

# Um ponteiro de profile:
curl https://peer.decentraland.org/content/entities/profiles/?pointer=0xe2c2b80ca5ad868f4b30fa83cca2bf12cc95b4fe
```

Observe que, se você já souber o ID da entity, você pode [baixar o manifest](/docs/contributor/contributor-pt/conteudo/file-system/#downloading) o arquivo diretamente.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/conteudo/pointers.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
