# Emotes

Los Emotes son los [entities](/docs/contributor/contributor-es/contenido/entities/) que contienen animaciones para los avatares del jugador.

Incluyen archivos en formato GLB para diferentes body shapes.

### Punteros <a href="#pointers" id="pointers"></a>

Los Emotes tienen exactamente un [pointer](/docs/contributor/contributor-es/contenido/pointers/), lo que indica la colección a la que pertenecen y su índice dentro de ella. Son URN de esta forma:

```
urn:decentraland:matic:collections-v2:<collection address>:<item index>
```

Para ilustrar, el `pointers` array para un Emote se ve así:

```json
{
  "pointers": [
    "urn:decentraland:matic:collections-v2:0x2d9560df9dd8ba8b2dc3746bc1d217698d258fb5:0"
  ],
  // ... otras propiedades de la entity
}
```

### Campos de Metadata

Los Emotes comparten la mayoría de sus campos básicos con [wearables](/docs/contributor/contributor-es/contenido/tipos-de-entidad/wearables/). El `emoteDataADR74` propiedad es donde se encuentra la información específica del Emote.

| Campo               | Valor                                                                                                                         |
| ------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| `id`                | El [pointer](/docs/contributor/contributor-es/contenido/pointers/) que resuelve (o solía resolver) a este Emote.                 |
| `name`              | El título visible de este Emote en un [collection](/docs/contributor/contributor-es/contenido/collections/).                     |
| `description`       | Una descripción ampliada de este Emote.                                                                                       |
| `image`             | El [nombre de archivo interno](/docs/contributor/contributor-es/contenido/entities/#files) con una imagen de este Emote.         |
| `thumbnail`         | El [nombre de archivo interno](/docs/contributor/contributor-es/contenido/entities/#files) para una versión 256x256 del `image`. |
| `rarity`            | Uno de `común`, `poco común`, `raro`, `épico`, `legendario`, `exótico`, `mítico` o `único`.                                   |
| `i18n`              | Un arreglo de traducciones para el `name` campo.                                                                              |
| `collectionAddress` | La dirección de Ethereum de la colección que contiene este Emote.                                                             |
| `metrics`           | Algunas mediciones útiles sobre las animaciones (ver abajo).                                                                  |
| `emoteDataADR74`    | Los metadatos ampliados de este Emote, según se definen en [ADR-74](https://adr.decentraland.org/adr/ADR-74) (ver abajo).     |

Así luce un JSON típico:

```json
{
  "id": "urn:decentraland:matic:collections-v2:0x2d9560df9dd8ba8b2dc3746bc1d217698d258fb5:0",
  "name": "Baile Divertido",
  "description": "Muévete como un campeón",
  "image": "image.png",
  "thumbnail": "thumbnail.png",
  "rarity": "raro",
  "i18n": [
    { "code": "es", "text": "Danza Graciosa" }
  ],
  "collectionAddress": "0x2d9560df9dd8ba8b2dc3746bc1d217698d258fb5",
  "metrics": {
    // Objeto de mediciones (ver abajo).
  },
  "emoteDataADR74": {
    // Metadatos ampliados definidos en ADR-74 (ver abajo).
  }
}
```

### Métricas

En la `metadata.metrics` objeto, encontrarás algunas mediciones sencillas para la animación empaquetada con este Emote. Un ejemplo:

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

### Datos de Emote ADR-74

Este objeto contiene los campos que World Explorers (u otras aplicaciones gráficas) necesitan para animar modelos con este Emote.

| Campo              | Valor                                                                                            |
| ------------------ | ------------------------------------------------------------------------------------------------ |
| `category`         | Uno de `baile`, `acrobacia`, `saludos`, `diversión`, `poses`, `reacciones`, `terror` o `varios`. |
| `representaciones` | Un arreglo de archivos de animación asociados a diferentes body shapes.                          |
| `tags`             | Un arreglo de etiquetas de cadena descriptivas de este Emote.                                    |
| `loop`             | `true` si la animación debe repetirse una vez que termine.                                       |

Algunos JSON para mayor claridad:

```json
{
  "category": "baile",
  "representations": [
    // Información sobre la animación para diferentes body shapes (ver abajo).
  ],
  "tags": [ "carnaval", "baile" ],
  "loop": true
}
```

#### Representaciones

Cada elemento en el `metadata.emoteDataADR74.representations` campo define los archivos de animación apropiados para cada body shape.

| Campo        | Valor                                                                                                                  |
| ------------ | ---------------------------------------------------------------------------------------------------------------------- |
| `bodyShapes` | Un arreglo de body shape aplicables [pointers](/docs/contributor/contributor-es/contenido/pointers/).                     |
| `mainFile`   | El [nombre de archivo interno](/docs/contributor/contributor-es/contenido/entities/#files) para el archivo GLB principal. |
| `contents`   | Todos los archivos, incluyendo `mainFile` y cualquier otro adicional al que pueda hacer referencia.                    |

Por ejemplo, un `representación` elemento:

```json
{
  "bodyShapes": [ "urn:decentraland:off-chain:base-avatars:BaseMale" ],
  "mainFile": "male/funnydance.glb",
  "contents": [
    "male/funnydance.glb"
  ]
}
```

Normalmente encontrarás dos entradas de representación, una para `BaseMale` y otra para `BaseFemale`.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/contenido/tipos-de-entidad/emotes.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
