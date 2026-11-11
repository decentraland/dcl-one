# Trajes

Os Outfits são os conjuntos guardados de um jogador. Um outfit é composto pelas configurações de body shape, skin, hair, eyes e wearables.

### Ponteiros <a href="#pointers" id="pointers"></a>

Outfits [pointers](/docs/contributor/contributor-pt/conteudo/pointers/) são o endereço Ethereum do proprietário com um "`:outfits`" sufixo. Por exemplo:

```
0x210c4415d6a71195af76beef9b85dd0eb43f35df:outfits
```

### Campos de Metadata

| Campo                | Valor                                                                                                                  |
| -------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| `outfits`            | Um array de descrições para cada outfit (ver abaixo).                                                                  |
| `namesForExtraSlots` | Um array de nomes possuídos usado para validar slots extras. Para cada nome possuído, é concedido um slot extra até 5. |

### Outfits

Cada outfit em `metadata.outfits[]` representa um outfit do utilizador e tem um número de propriedades:

| Campo    | Valor                              |
| -------- | ---------------------------------- |
| `slot`   | Número do slot do outfit guardado. |
| `outfit` | O outfit guardado (ver abaixo).    |

Um exemplo:

```json
{
  "slot": 1,
  "outfit": {
    // Ver abaixo
  }
}
```

#### Campo do Outfit

O `metadata.outfits[].outfit` este campo tem toda a informação para guardar o outfit de um jogador e renderizá-lo.

| Campo       | Valor                                                                                                                           |
| ----------- | ------------------------------------------------------------------------------------------------------------------------------- |
| `bodyShape` | O [pointer](/docs/contributor/contributor-pt/conteudo/pointers/) para a entidade bodyShape do avatar.                              |
| `eyes`      | Um objeto com um `color` no formato de um `{ r, g, b }` objeto para os eyes do avatar.                                          |
| `hair`      | Um objeto com um `color` no formato de um `{ r, g, b }` objeto para o hair do avatar.                                           |
| `skin`      | Um objeto com um `color` no formato de um `{ r, g, b }` objeto para a skin do avatar.                                           |
| `wearables` | Um array de [wearable pointers](/docs/contributor/contributor-pt/conteudo/tipos-de-entity/wearables/#pointers) em uso pelo avatar. |

Para ilustrar:

```json
{
  "bodyShape": "urn:decentraland:off-chain:base-avatars:BaseMale",
  "eyes": {
    "color": { "r": 0.37109375, "g": 0.22265625, "b": 0.1953125 }
  },
  "hair": {
    "color": { "r": 0.234375, "g": 0.12890625, "b": 0.04296875 }
  },
  "skin": {
    "color": { "r": 0.80078125, "g": 0.609375, "b": 0.46484375 }
  },
  "wearables": [
    "urn:decentraland:off-chain:base-avatars:eyes_00",
    "urn:decentraland:off-chain:base-avatars:eyebrows_00",
    "urn:decentraland:off-chain:base-avatars:mouth_00",
    "urn:decentraland:off-chain:base-avatars:m_mountainshoes.glb",
    "urn:decentraland:off-chain:base-avatars:sport_jacket",
    "urn:decentraland:off-chain:base-avatars:brown_pants",
    "urn:decentraland:off-chain:base-avatars:aviatorstyle",
    "urn:decentraland:off-chain:base-avatars:casual_hair_03",
    "urn:decentraland:off-chain:base-avatars:Mustache_Short_Beard"
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
GET https://docs.decentraland.org/contributor/contributor-pt/conteudo/tipos-de-entity/outfits.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
