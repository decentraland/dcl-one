# Conjuntos

Los outfits son los outfits guardados de un jugador. Un outfit se compone de configuraciones de body shape, skin, hair, eyes y wearables.

### Punteros <a href="#pointers" id="pointers"></a>

Outfits [pointers](/docs/contributor/contributor-es/contenido/pointers/) son la dirección de Ethereum del propietario con un "`:outfits`" como sufijo. Por ejemplo:

```
0x210c4415d6a71195af76beef9b85dd0eb43f35df:outfits
```

### Campos de Metadata

| Campo                | Valor                                                                                                                      |
| -------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| `outfits`            | Un array de descripciones para cada outfit (ver abajo).                                                                    |
| `namesForExtraSlots` | Un array de nombres poseídos usado para validar extra slots. Por cada nombre poseído, se le concede un extra slot hasta 5. |

### Outfits

Cada outfit en `metadata.outfits[]` representa un outfit del usuario y tiene varias propiedades:

| Campo    | Valor                               |
| -------- | ----------------------------------- |
| `slot`   | Número de slot del outfit guardado. |
| `outfit` | El outfit guardado (ver abajo).     |

Un ejemplo:

```json
{
  "slot": 1,
  "outfit": {
    // Ver abajo
  }
}
```

#### Campo del outfit

El `metadata.outfits[].outfit` este campo tiene toda la información para guardar el outfit de un jugador y renderizarlo.

| Campo       | Valor                                                                                                                                 |
| ----------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| `bodyShape` | El [pointer](/docs/contributor/contributor-es/contenido/pointers/) a la entidad de body shape del avatar.                                |
| `eyes`      | Un objeto con un `color` en forma de un `{ r, g, b }` objeto para los ojos del avatar.                                                |
| `hair`      | Un objeto con un `color` en forma de un `{ r, g, b }` objeto para el hair del avatar.                                                 |
| `skin`      | Un objeto con un `color` en forma de un `{ r, g, b }` objeto para la skin del avatar.                                                 |
| `wearables` | Una matriz de [wearable pointers](/docs/contributor/contributor-es/contenido/tipos-de-entidad/wearables/#pointers) en uso por el avatar. |

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
GET https://docs.decentraland.org/contributor/contributor-es/contenido/tipos-de-entidad/outfits.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
