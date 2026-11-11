# Perfis

Os perfis são a descrição básica de um jogador, com informações como o seu nome no mundo e avatar.

Eles estão disponíveis nos content servers como regulares [entidades](/docs/contributor/contributor-pt/conteudo/entities/), embora os Exploradores de Worlds normalmente utilizem o sistema de comunicações para obter versões atualizadas em tempo real.

O sistema está preparado para permitir múltiplas identidades com o mesmo proprietário, todas incluídas como avatares no manifesto da entidade. Na prática, porém, a grande maioria dos jogadores tem apenas uma.

### Ponteiros <a href="#pointers" id="pointers"></a>

Perfil [pointers](/docs/contributor/contributor-pt/conteudo/pointers/) são o endereço Ethereum do proprietário, sem qualquer prefixo ou sufixo. Por exemplo:

```
0x210c4415d6a71195af76beef9b85dd0eb43f35df
```

### Campos de Metadata

| Campo      | Valor                                                                             |
| ---------- | --------------------------------------------------------------------------------- |
| `avatares` | Uma matriz de descrições para cada um dos avatares do proprietário (veja abaixo). |

### Avatares

Cada avatar em `metadata.avatars[]` representa uma identidade com o mesmo proprietário e tem várias propriedades que permitem aos clients exibir perfis, renderizar avatares no mundo e contactar o proprietário.

| Campo            | Valor                                                                                                     |
| ---------------- | --------------------------------------------------------------------------------------------------------- |
| `userId`         | O [pointer](/docs/contributor/contributor-pt/conteudo/pointers/) que resolve (ou resolvia) para este perfil. |
| `name`           | O nome de exibição deste jogador.                                                                         |
| `email`          | (Opcional) Um endereço de email para este jogador.                                                        |
| `description`    | (Opcional) Texto escolhido pelo jogador para se descrever.                                                |
| `ethAddress`     | O endereço Ethereum deste jogador (atualmente igual ao seu `userId`).                                     |
| `hasClaimedName` | Se o `name` campo é um nome ENS reivindicado.                                                             |
| `unclaimedName`  | Um nome temporário para utilizadores sem um nome ENS (por exemplo, convidados).                           |
| `tutorialStep`   | O progresso do tutorial para este jogador.                                                                |
| `avatar`         | Propriedades do avatar do jogador no mundo (veja abaixo).                                                 |

Um exemplo:

```json
{
  "userId": "0x210c4415d6a71195af76beef9b84dd0eb43f35da",
  "name": "Bob",
  "email": "",
  "hasClaimedName": false,
  "description": "",
  "ethAddress": "0x210c4415d6a71195af76beef9b84dd0eb43f35da",
  "avatar": {
    // Ver abaixo
  }
}
```

#### Campo de Avatar

O `metadata.avatars[].avatar` o campo contém todas as informações de que um Explorador de Worlds (ou outros clients, como um editor de avatares independente) precisa para renderizar um jogador.

| Campo       | Valor                                                                                                                           |
| ----------- | ------------------------------------------------------------------------------------------------------------------------------- |
| `bodyShape` | O [pointer](/docs/contributor/contributor-pt/conteudo/pointers/) para a entidade bodyShape do avatar.                              |
| `snapshots` | Um objeto com imagens para este avatar (veja abaixo).                                                                           |
| `eyes`      | Um objeto com um `color` no formato de um `{ r, g, b }` objeto para os eyes do avatar.                                          |
| `hair`      | Um objeto com um `color` no formato de um `{ r, g, b }` objeto para o hair do avatar.                                           |
| `skin`      | Um objeto com um `color` no formato de um `{ r, g, b }` objeto para a skin do avatar.                                           |
| `wearables` | Um array de [wearable pointers](/docs/contributor/contributor-pt/conteudo/tipos-de-entity/wearables/#pointers) em uso pelo avatar. |

Para ilustrar:

```json
{
  "bodyShape": "urn:decentraland:off-chain:base-avatars:BaseMale",
  "snapshots": {
    // Ver abaixo
  },
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

#### Snapshots

O `metadata.avatars[].avatar.snapshots` campo contém [identificadores de ficheiros](/docs/contributor/contributor-pt/conteudo/file-system/#identifiers) para imagens (incluídas no [`content` campo de nível superior](/docs/contributor/contributor-pt/conteudo/entities/#properties)). Cada propriedade é um tipo de imagem, e atualmente existem duas: `face256` (uma miniatura) e `body` (uma versão em tamanho real).

Por exemplo:

```json
{
  "face256": "bafkreidvjwc3gqqdqk646rqnxnqzpsyjsjafqfc76zenfgjnm2ahbvrtsd",
  "body": "bafkreigclygt6vzzu7myzg3weifapb3a5uec245lzndg4f53y4vfb3cmwm"
}
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/conteudo/tipos-de-entity/profiles.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
