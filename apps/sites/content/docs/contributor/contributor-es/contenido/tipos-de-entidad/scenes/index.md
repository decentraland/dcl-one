# Escenas

Las Scenes son las [entities](/docs/contributor/contributor-es/contenido/entities/) que producen comportamiento en diferentes partes de Decentraland. Pueden abarcar múltiples parcelas de LAND, y los jugadores que pasen por ellas en un World Explorer ejecutarán el código especial proporcionado por la scene.

Puedes encontrar un ejemplo completo de la vida real en [el manifest de esta scene](https://peer.decentraland.org/content/contents/bafkreidihplrc5cxarjkji2enmzhicpjoa2vrlqyyffnholmxb3o2xft3u) en el content server de la Foundation.

### Punteros <a href="#pointers" id="pointers"></a>

Las Scenes tienen uno o más [pointers](/docs/contributor/contributor-es/contenido/pointers/) asociados en forma de cadenas de coordenadas de parcela, como `"0,0"`. Cada parcela que ejecute la scene se listará en el [`pointers` array](/docs/contributor/contributor-es/contenido/entities/#properties) del manifest de la entity, y todos esos pointers se resolverán por el content server hacia esa scene.

En otras palabras, puedes elegir las coordenadas x/y de una parcela y localizar la scene que está ejecutando consultando el content server para el `"<x>,<y>"` pointer. Esta es la forma en que los World Explorers deciden si habilitar el comportamiento de una scene mientras el jugador navega por el mundo.

Un `pointers` array se ve así:

```json
{
  "pointers": [
    "-113,-134",
    "-113,-133",
    "-113,-132",
  ],
  // ... otras propiedades de la entity
}
```

Estos datos también están disponibles en el `metadata.scene.parcels` campo detallado abajo, por si quieres guardar solo el objeto específico de la scene en el manifest.

### Campos de Metadata

Hay varias propiedades especiales para [entities](/docs/contributor/contributor-es/contenido/entities/) de tipo `escena` ubicadas en el `metadata` campo de nivel superior.

| Campo                 | Valor                                                                                                                                  |
| --------------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| `owner`               | Información sobre el mantenedor de la scene.                                                                                           |
| `main`                | El [nombre de archivo interno](/docs/contributor/contributor-es/contenido/entities/#files) al archivo JavaScript principal de esta scene. |
| `tags`                | Un array de etiquetas de string descriptivas de esta scene.                                                                            |
| `display`             | Información sobre la scene para que los Explorers se la muestren a los jugadores (ver abajo).                                          |
| `escena`              | Las parcelas en las que esta scene está activa, y su ubicación central (ver abajo).                                                    |
| `contact`             | Nombre y dirección de email para contactar a los creadores o mantenedores de la scene (ver abajo).                                     |
| `spawnPoints`         | Ubicaciones y ángulos de cámara para los jugadores que entren en esta scene (ver abajo).                                               |
| `requiredPermissions` | Permisos recomendados para que los Explorers se los pidan a los jugadores (ver abajo).                                                 |
| `featureToggles`      | Configuración para feature flags                                                                                                       |


> [!NOTE]
> Recuerda que a las entities y a su metadata se les permite tener campos personalizados. Es posible que encuentres algunas propiedades por ahí que no están listadas aquí.


#### Display

En `metadata.display`, encontrarás propiedades que son útiles para informar a los jugadores sobre la scene antes de que entren en ella.

| Campo             | Valor                                                                                                                                                        |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `title`           | El nombre para mostrar de esta scene.                                                                                                                        |
| `description`     | Una descripción extendida de esta scene.                                                                                                                     |
| `favicon`         | *Obsoleto* El [nombre de archivo interno](/docs/contributor/contributor-es/contenido/entities/#files) a un icono, que se muestra cuando esta scene está activa. |
| `navmapThumbnail` | El [nombre de archivo interno](/docs/contributor/contributor-es/contenido/entities/#files) a la miniatura de esta scene para el mapa del mundo.                 |

Un ejemplo:

```json
{
  "title": "my_cool_scene",
  "favicon": "favicon.png",
  "navmapThumbnail": "thumbnail.png"
}
```

#### Scene

El `metadata.scene` property es un objeto que describe la posición de esta scene en el mapa del mundo.

| Campo     | Valor                                                |
| --------- | ---------------------------------------------------- |
| `parcels` | Un array de parcel pointers que ejecutan esta scene. |
| `base`    | El parcel pointer del punto de origen de esta scene. |

En la práctica, se ve así:

```json
{
  "parcels": [
    "17,-112", 
    "17,-113"
  ],
  "base": "17,-112"
}
```

El `base` campo siempre se incluye en el `parcels` array.

#### Contact

El `metadata.contact` objeto contiene la información de contacto del owner o mantenedor de la scene.

| Campo   | Valor                                             |
| ------- | ------------------------------------------------- |
| `name`  | Un nombre para este contacto.                     |
| `email` | (Opcional) Una dirección de email para contactar. |

En JSON:

```json
{
  "name": "cool_scene_maker",
  "email": "hello@decentraland.org"
}
```

Aunque el `name` campo se rellena en cada scene, encontrarás que en la práctica real el `email` campo suele ser una cadena vacía. Esta es una decisión de privacidad tomada por el creador de la scene.

#### Spawn Points

El `metadata.spawnPoints` campo define un array de puntos y direcciones de cámara que determinan dónde deben aparecer los jugadores que entren en la scene, y hacia dónde deben mirar inicialmente. Debe haber al menos un spawn point definido.

Cada elemento del array tiene varias propiedades:

| Campo          | Valor                                                                               |
| -------------- | ----------------------------------------------------------------------------------- |
| `name`         | Una etiqueta para identificar este spawn point.                                     |
| `position`     | La posición inicial en un `{ x, y, z }` objeto con coordenadas float.               |
| `cameraTarget` | La dirección inicial de la cámara en un `{ x, y, z }` objeto con coordenadas float. |
| `default`      | Indica si este spawn point se usa a menos que se especifique lo contrario.          |

Por ejemplo:

```json
[
  {
    "name": "spawn1",
    "position": { "x": 10.02, "y": 5.27, "z": 15.23  },
    "cameraTarget": { "x": 10.02, "y": 6.27, "z": 31.23 },
    "default": true
  }
]
```


> [!NOTE]
> **📔 Nota**: El Spawn Area y el Camera Target pueden configurarse usando el Creator Hub. Para más información, ve a los [Spawn Area](https://github.com/decentraland/docs/tree/main/creator/scene-editor/build/spawn-areas.md) docs.


#### Permisos requeridos <a href="#permissions" id="permissions"></a>

En `metadata.requiredPermissions` encontrarás un array de strings conocidos que declaran qué permisos deben pedirse al jugador antes de que el World Explorer permita ciertas acciones. Esto le corresponde al Explorer; otros clientes creados para distintos propósitos pueden optar por ignorarlo.

Este es el conjunto actual de permisos compatibles:

| Permiso                             | Propósito                                                                         |
| ----------------------------------- | --------------------------------------------------------------------------------- |
| `USE_FETCH`                         | Permite que la scene realice requests HTTP externas.                              |
| `USE_WEBSOCKET`                     | Permite que la scene use la API de WebSocket para establecer conexiones externas. |
| `OPEN_EXTERNAL_LINK`                | Permite que la scene abra una URL (en una pestaña del navegador o una vista web). |
| `USE_WEB3_API`                      | Permite que la scene se comunique con una Wallet.                                 |
| `ALLOW_TO_TRIGGER_AVATAR_EMOTE`     | Permite que la scene anime el avatar del jugador con un emote.                    |
| `ALLOW_TO_MOVE_PLAYER_INSIDE_SCENE` | Permite que la scene cambie la posición del jugador.                              |

#### Feature Toggles

El `metadata.featureToggles` campo permite que una scene indique si ciertas features del World Explorer deben estar habilitadas o deshabilitadas.

El campo contiene un objeto de la forma `{ [featureName]: 'enabled' | 'disabled' }`, como este:

```json
{
  "voiceChat": "enabled"
}
```

Actualmente, `voiceChat` es el único feature flag ampliamente compatible.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/contenido/tipos-de-entidad/scenes.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
