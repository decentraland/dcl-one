# Wearables

Los Wearables son los [entities](/docs/contributor/contributor-es/contenido/entities/) que contienen ítems para que los jugadores los lleven y personalicen sus avatares.

El `wearables` campo en el [perfil](/docs/contributor/contributor-es/contenido/tipos-de-entidad/profiles/) contiene una lista de punteros a los ítems que llevan puestos actualmente.

Cada Wearable pertenece a una *categoría*, lo que indica el lugar que ocupa en el cuerpo del avatar, y puede ocultar o reemplazar otras categorías (por ejemplo, un vestido largo puede usarse como ropa superior pero cubrir por completo las piernas).

Hay 18 categorías de Wearables, referenciadas en varios campos de metadatos: `body_shape`, `hair`, `eyes`, `eyebrows`, `facial_hair`, `mouth`, `upper_body`, `lower_body`, `feet`, `hat`, `helmet`, `mask`, `tiara`, `top_head`, `earring`, `eyewear`, `skin` y `hands_wear`.

Las entidades Wearable incluyen archivos en formato GLB para diferentes body shapes.

### Punteros <a href="#pointers" id="pointers"></a>

Los Wearables tienen exactamente un asociado [pointer](/docs/contributor/contributor-es/contenido/pointers/), que indica el [collection](/docs/contributor/contributor-es/contenido/collections/) al que pertenecen y su identificador dentro de él.

Cuando el Wearable es un activo on-chain, el puntero es un URN de esta forma:

```
urn:decentraland:<blockchain>:collections-v2:<collection address>:<item index>
```

A modo de ejemplo, el `pointers` array de un Wearable on-chain se ve así:

```json
{
  "pointers": [
    "urn:decentraland:matic:collections-v2:0x2d9560df9dd8ba8b2dc3746bc1d217698d258fb5:0"
  ],
  // ... otras propiedades de la entity
}
```

Cuando el Wearable es un activo off-chain, como los Wearables predeterminados para nuevos avatares, tendrá esta forma:

```
urn:decentraland:off-chain:<collection>:<item name>
```

La colección en este caso es un nombre, no una dirección, y el identificador del ítem también es un nombre en lugar de un índice.

El `pointers` el array de un Wearable off-chain se ve así:

```json
{
  "pointers": [
    "urn:decentraland:off-chain:base-avatars:BaseFemale"
  ],
  // ... otras propiedades de la entity
}
```

### Campos de Metadata

Los Wearables comparten la mayoría de sus campos básicos con [emotes](/docs/contributor/contributor-es/contenido/tipos-de-entidad/emotes/). La `data` propiedad es donde se encuentra la información específica del Wearable.

| Campo               | Valor                                                                                                                         |
| ------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| `id`                | El [pointer](/docs/contributor/contributor-es/contenido/pointers/) que resuelve (o solía resolver) a este Wearable.              |
| `name`              | El título visible de este Wearable en un [collection](/docs/contributor/contributor-es/contenido/collections/).                  |
| `description`       | Una descripción extendida de este Wearable.                                                                                   |
| `image`             | El [nombre de archivo interno](/docs/contributor/contributor-es/contenido/entities/#files) de una imagen para este Wearable.     |
| `thumbnail`         | El [nombre de archivo interno](/docs/contributor/contributor-es/contenido/entities/#files) de una versión 256x256 de la `image`. |
| `rarity`            | Uno de `common`, `uncommon`, `rare`, `epic`, `legendary`, `exotic`, `mythic` o `unique`.                                      |
| `i18n`              | Un array de traducciones para el `name` campo.                                                                                |
| `collectionAddress` | La dirección de Ethereum de la colección que contiene este Wearable.                                                          |
| `metrics`           | Algunas mediciones útiles sobre las animaciones (ver abajo).                                                                  |
| `data`              | Los metadatos extendidos de este Wearable (ver abajo).                                                                        |

En una forma JSON típica:

```json
{
  "id": "urn:decentraland:matic:collections-v2:0x11a6879861f36cbad632b4e7226816a16139fb33:0",
  "name": "Pretty Dress",
  "description": "Muy agradable de usar",
  "image": "image.png",
  "thumbnail": "thumbnail.png",
  "rarity": "uncommon",
  "i18n": [
    { "code": "es", "text": "Vestido Lindo" }
  ],
  "collectionAddress": "0x11a6879861f36cbad632b4e7226816a16139fb33",
  "metrics": {
    // Objeto de mediciones (ver abajo).
  },
  "data": {
    // Metadatos extendidos (ver abajo).
  }
}
```

### Métricas

En la `metadata.metrics` objeto, encontrarás algunas mediciones sencillas de los modelos empaquetados con este Wearable. Un ejemplo:

```json
{
  "entities": 1,
  "bodies": 12,
  "materials": 2,
  "meshes": 12,
  "textures": 2,
  "triangles": 3321
}
```

### Datos

El `metadata.data` objeto es donde se encuentran las propiedades específicas del Wearable.

| Campo              | Valor                                                                |
| ------------------ | -------------------------------------------------------------------- |
| `categoría`        | Una de las categorías de Wearables enumeradas arriba.                |
| `Representaciones` | Un array de archivos asociados a diferentes body shapes (ver abajo). |
| `hides`            | Un array de categorías que este Wearable oculta.                     |
| `reemplaza`        | Un array de categorías que este Wearable reemplaza por completo.     |
| `tags`             | Un array de etiquetas de texto descriptivas de este Wearable.        |

#### Representations

Cada elemento del array representations asocia un body shape con una colección de modelos, y puede añadir reglas sobre qué Wearables se muestran en cada categoría.

| Campo              | Valor                                                                                                                                                                              |
| ------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `bodyShapes`       | Una matriz de [pointers](/docs/contributor/contributor-es/contenido/pointers/) a entidades body shape.                                                                                |
| `mainFile`         | El archivo del modelo 3D para iniciar el rendering.                                                                                                                                |
| `contents`         | Un array de nombres de archivo (presentes en el campo de nivel superior [`content` field](/docs/contributor/contributor-es/contenido/entities/#properties)) usados por el `mainFile`. |
| `overrideHides`    | Sustituye la `hides` lista anterior para esta representación.                                                                                                                      |
| `overrideReplaces` | Sustituye la `reemplaza` lista anterior para esta representación.                                                                                                                  |

Este es un `Representaciones` ítem:

```json
{
  "bodyShapes": [
      "urn:decentraland:off-chain:base-avatars:BaseFemale"
  ],
  "mainFile": "male/190.glb",
  "contents": [
      "male/190.glb"
  ],
  "overrideHides": [ "lower_body" ],
  "overrideReplaces": []
}
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/contenido/tipos-de-entidad/wearables.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
