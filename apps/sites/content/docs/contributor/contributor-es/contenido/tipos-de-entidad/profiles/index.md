# Perfiles

Los perfiles son la descripción básica de un jugador, con información como su nombre en el mundo y avatar.

Están disponibles en los servidores de contenido como regulares [entities](/docs/contributor/contributor-es/contenido/entities/), aunque los World Explorers suelen aprovechar el sistema de comunicaciones para obtener versiones actualizadas sobre la marcha.

El sistema está preparado para permitir múltiples identidades con el mismo propietario, todas incluidas como avatares en el entity manifest. En la práctica, sin embargo, la gran mayoría de los jugadores solo tiene una.

### Punteros <a href="#pointers" id="pointers"></a>

Perfil [pointers](/docs/contributor/contributor-es/contenido/pointers/) son la dirección de Ethereum del propietario, sin ningún prefijo ni sufijo. Por ejemplo:

```
0x210c4415d6a71195af76beef9b85dd0eb43f35df
```

### Campos de Metadata

| Campo      | Valor                                                                                    |
| ---------- | ---------------------------------------------------------------------------------------- |
| `avatares` | Un arreglo de descripciones de cada uno de los avatares del propietario (ver más abajo). |

### Avatares

Cada avatar en `metadata.avatars[]` representa una identidad con el mismo propietario y tiene varias propiedades que permiten a los clientes mostrar perfiles, renderizar avatares en el mundo y contactar al propietario.

| Campo            | Valor                                                                                                          |
| ---------------- | -------------------------------------------------------------------------------------------------------------- |
| `userId`         | El [pointer](/docs/contributor/contributor-es/contenido/pointers/) que resuelve (o solía resolver) a este perfil. |
| `name`           | El nombre visible de este jugador.                                                                             |
| `email`          | (Opcional) Una dirección de correo electrónico para este jugador.                                              |
| `description`    | (Opcional) Texto elegido por el jugador para describirse.                                                      |
| `ethAddress`     | La dirección de Ethereum de este jugador (actualmente igual a su `userId`).                                    |
| `hasClaimedName` | Si el `name` campo es un nombre ENS reclamado.                                                                 |
| `unclaimedName`  | Un nombre temporal para usuarios sin un nombre ENS (p. ej., invitados).                                        |
| `tutorialStep`   | El progreso del tutorial para este jugador.                                                                    |
| `avatar`         | Propiedades del avatar en el mundo del jugador (ver abajo).                                                    |

Un ejemplo:

```json
{
  "userId": "0x210c4415d6a71195af76beef9b84dd0eb43f35da",
  "name": "Bob",
  "email": "",
  "hasClaimedName": false,
  "description": "",
  "ethAddress": "0x210c4415d6a71195af76beef9b84dd0eb43f35da",
  "avatar": {
    // Ver abajo
  }
}
```

#### Campo de Avatar

El `metadata.avatars[].avatar` el campo contiene toda la información que un World Explorer (u otros clientes, como un editor de avatares independiente) necesita para renderizar a un jugador.

| Campo          | Valor                                                                                                                                 |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| `bodyShape`    | El [pointer](/docs/contributor/contributor-es/contenido/pointers/) a la entidad de body shape del avatar.                                |
| `instantáneas` | Un objeto con imágenes para este avatar (ver abajo).                                                                                  |
| `eyes`         | Un objeto con un `color` en forma de un `{ r, g, b }` objeto para los ojos del avatar.                                                |
| `hair`         | Un objeto con un `color` en forma de un `{ r, g, b }` objeto para el hair del avatar.                                                 |
| `skin`         | Un objeto con un `color` en forma de un `{ r, g, b }` objeto para la skin del avatar.                                                 |
| `wearables`    | Una matriz de [wearable pointers](/docs/contributor/contributor-es/contenido/tipos-de-entidad/wearables/#pointers) en uso por el avatar. |

Para ilustrar:

```json
{
  "bodyShape": "urn:decentraland:off-chain:base-avatars:BaseMale",
  "snapshots": {
    // Ver abajo
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

El `metadata.avatars[].avatar.snapshots` el campo contiene [identificadores de archivo](/docs/contributor/contributor-es/contenido/file-system/#identifiers) para imágenes (incluidos en el [`content` campo de nivel superior](/docs/contributor/contributor-es/contenido/entities/#properties)). Cada propiedad es un tipo de imagen, y actualmente hay dos: `face256` (una miniatura) y `body` (una versión de tamaño completo).

Por ejemplo:

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
GET https://docs.decentraland.org/contributor/contributor-es/contenido/tipos-de-entidad/profiles.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
