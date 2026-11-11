# Emotes

Emotes são os [entidades](/docs/contributor/contributor-pt/conteudo/entities/) que contêm animações para os avatares dos jogadores.

Eles incluem ficheiros no formato GLB para diferentes formas corporais.

### Ponteiros <a href="#pointers" id="pointers"></a>

Os Emotes têm exatamente um [pointer](/docs/contributor/contributor-pt/conteudo/pointers/)associado, que indica a coleção a que pertencem e o seu índice dentro dela. São URNs desta forma:

```
urn:decentraland:matic:collections-v2:<collection address>:<item index>
```

Para ilustrar, o `pointers` array para um emote parece-se com isto:

```json
{
  "pointers": [
    "urn:decentraland:matic:collections-v2:0x2d9560df9dd8ba8b2dc3746bc1d217698d258fb5:0"
  ],
  // ... outras propriedades da entity
}
```

### Campos de Metadata

Os Emotes partilham a maioria dos seus campos básicos com [wearables](/docs/contributor/contributor-pt/conteudo/tipos-de-entity/wearables/). A propriedade `emoteDataADR74` é onde se encontra a informação específica do emote.

| Campo               | Valor                                                                                                                     |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| `id`                | O [pointer](/docs/contributor/contributor-pt/conteudo/pointers/) que resolve (ou resolvia) para este emote.                  |
| `name`              | O título exibido para este emote numa [collection](/docs/contributor/contributor-pt/conteudo/collections/).                  |
| `description`       | Uma descrição detalhada deste emote.                                                                                      |
| `image`             | O [nome de arquivo interno](/docs/contributor/contributor-pt/conteudo/entities/#files) com uma imagem deste emote.           |
| `thumbnail`         | O [nome de arquivo interno](/docs/contributor/contributor-pt/conteudo/entities/#files) para uma versão 256x256 da `image`.   |
| `rarity`            | Um de `common`, `uncommon`, `rare`, `epic`, `legendary`, `exotic`, `mythic` ou `unique`.                                  |
| `i18n`              | Um array de traduções para o `name` campo.                                                                                |
| `collectionAddress` | O endereço Ethereum da coleção que contém este emote.                                                                     |
| `metrics`           | Algumas métricas úteis sobre as animações (ver abaixo).                                                                   |
| `emoteDataADR74`    | Os metadados estendidos deste emote, conforme definido em [ADR-74](https://adr.decentraland.org/adr/ADR-74) (ver abaixo). |

É assim que um JSON típico se parece:

```json
{
  "id": "urn:decentraland:matic:collections-v2:0x2d9560df9dd8ba8b2dc3746bc1d217698d258fb5:0",
  "name": "Funny Dance",
  "description": "Move around like a champ",
  "image": "image.png",
  "thumbnail": "thumbnail.png",
  "rarity": "rare",
  "i18n": [
    { "code": "es", "text": "Danza Graciosa" }
  ],
  "collectionAddress": "0x2d9560df9dd8ba8b2dc3746bc1d217698d258fb5",
  "metrics": {
    // Objeto de métricas (ver abaixo).
  },
  "emoteDataADR74": {
    // Metadados estendidos definidos em ADR-74 (ver abaixo).
  }
}
```

### Métricas

Na `metadata.metrics` objeto, encontrará algumas medições simples para a animação empacotada com este emote. Um exemplo:

```json
{
  "triangles": 0,
  "materials": 0,
  "textures": 0,
  "meshes": 0,
  "bodies": 0,
  "entities": 1
}
```

### Dados do Emote ADR-74

Este objeto contém os campos de que o World Explorers (ou outras aplicações gráficas) precisam para animar modelos com este emote.

| Campo             | Valor                                                                                          |
| ----------------- | ---------------------------------------------------------------------------------------------- |
| `category`        | Um de `dance`, `stunt`, `greetings`, `fun`, `poses`, `reactions`, `horror` ou `miscellaneous`. |
| `representations` | Um array de ficheiros de animação associados a diferentes formas corporais.                    |
| `tags`            | Um array de tags de string descritivas deste emote.                                            |
| `loop`            | `true` se a animação deve repetir-se quando terminar.                                          |

Algum JSON para maior clareza:

```json
{
  "category": "dance",
  "representations": [
    // Informação sobre a animação para diferentes formas corporais (ver abaixo).
  ],
  "tags": [ "carnaval", "dance" ],
  "loop": true
}
```

#### Representações

Cada item no `metadata.emoteDataADR74.representations` campo define os ficheiros de animação apropriados para cada forma corporal.

| Campo        | Valor                                                                                                              |
| ------------ | ------------------------------------------------------------------------------------------------------------------ |
| `bodyShapes` | Um array de body shape aplicáveis [pointers](/docs/contributor/contributor-pt/conteudo/pointers/).                    |
| `mainFile`   | O [nome de arquivo interno](/docs/contributor/contributor-pt/conteudo/entities/#files) para o ficheiro GLB principal. |
| `contents`   | Todos os ficheiros, incluindo `mainFile` e quaisquer outros adicionais que possa referenciar.                      |

Por exemplo, um `representação` item:

```json
{
  "bodyShapes": [ "urn:decentraland:off-chain:base-avatars:BaseMale" ],
  "mainFile": "male/funnydance.glb",
  "contents": [
    "male/funnydance.glb"
  ]
}
```

Normalmente encontrará duas entradas de representação, uma para `BaseMale` e outra para `BaseFemale`.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/conteudo/tipos-de-entity/emotes.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
