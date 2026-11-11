# Entidades

Cada pieza individual de contenido en el mundo (como una Scene o un Wearable) se llama un *entity*.

Las Entities son paquetes inmutables de [archivos](/docs/contributor/contributor-es/contenido/file-system/) con un identificador de cadena único, derivado determinísticamente de los datos contenidos, que puede utilizarse para descubrir y descargar los archivos relacionados desde el content server.

El archivo principal de una entity es el *manifest*, un documento JSON que describe las propiedades generales de la entity, así como atributos especiales para cada [tipo](#types). El identificador de una entity es en realidad el [identificador del archivo](/docs/contributor/contributor-es/contenido/file-system/#identifiers) de este manifest.

Como son inmutables, las entities no pueden actualizarse en el sentido tradicional. En su lugar, se reemplazan por nuevas entities que pueden descubrirse usando el mismo stable [pointer](/docs/contributor/contributor-es/contenido/pointers/). Se dice que la versión más reciente de una entity está *activa*.

Cada entity está firmada por un propietario (asociado a una cuenta de Ethereum). El propietario puede luego usar las mismas claves de firma para subir una nueva versión de la entity e indicar que reemplaza a la anterior. Los content servers validan estas firmas antes de aceptar nuevas entities, ya sea que lleguen directamente desde un client o que hayan sido reenviadas por otro server.

Puedes ver las entities realmente desplegadas en la [práctica](https://github.com/decentraland/docs/tree/main/contributor/content/practice/README.md) sección.

### Tipos de Entities <a href="#types" id="types"></a>

Hay siete tipos de entities:

* [**Scenes**](/docs/contributor/contributor-es/contenido/tipos-de-entidad/scenes/): espacios virtuales en el mundo con sus propios objetos y comportamiento.
* [**Profiles**](/docs/contributor/contributor-es/contenido/tipos-de-entidad/profiles/): información sobre un player específico, como su nombre y avatar.
* [**Wearables**](/docs/contributor/contributor-es/contenido/tipos-de-entidad/wearables/): prendas y artículos que los players pueden añadir a sus avatars.
* [**Emotes**](/docs/contributor/contributor-es/contenido/tipos-de-entidad/emotes/): animaciones que el avatar de un player puede realizar.
* [**Stores**](/docs/contributor/contributor-es/contenido/tipos-de-entidad/stores/): sitios de Marketplace para Wearables y Emotes que los players pueden comprar.
* [**Outfits**](/docs/contributor/contributor-es/contenido/tipos-de-entidad/outfits/): outfits guardados para un player específico.

Todos los tipos siguen los mismos procedimientos de creación, identificación, propiedad y hosting.

### Propiedades comunes <a href="#properties" id="properties"></a>

Cada entity tiene ciertas propiedades comunes en su manifest, aplicables a todos los tipos. Estos campos de nivel superior siempre estarán presentes:

| Campo       | Valor                                                                                                                                    |
| ----------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| `tipo`      | Uno de `escena`, `perfil`, `wearable`, `emote`, `store` o `outfits`.                                                                     |
| `pointers`  | Una matriz de [pointers](/docs/contributor/contributor-es/contenido/pointers/) asociados a esta entity.                                     |
| `timestamp` | La marca de tiempo UTC de Unix en que se subió esta entity.                                                                              |
| `content`   | Un array de referencias a content adicional [archivos](/docs/contributor/contributor-es/contenido/file-system/) en el paquete de la entity. |
| `metadata`  | Un objeto con información específica de este tipo de entity.                                                                             |

La estructura y los valores del `metadata` campo para cada tipo se detallan en sus páginas específicas. El `pointers` array también tiene contenidos diferentes según el tipo.


> [!NOTE]
> Los manifests antiguos de entity pueden contener el `version` campo, obsoleto en [ADR-45](https://adr.decentraland.org/adr/ADR-45). Puedes ignorarlo con seguridad, ya que el `timestamp` campo ahora se usa para el versionado.


Este es un manifest JSON típico que describe una entity:

```json
{
  "type": "wearable",
  "pointers": ["urn:decentraland:matic:collections-v2:0xbdf21eaf54ebf4a6cadc2dcb371df7afce98bc1d:0"],
  "timestamp": 1628181913506,
  "content": [
    // ...referencias de archivos, ver más abajo
  ],
  "metadata": {
    // ...campos específicos para este tipo de entity, ver la página correspondiente
  }
}
```

Puedes encontrar los schemas de estas estructuras JSON, junto con otros objetos del protocolo de Decentraland, en el [Common Schemas](https://github.com/decentraland/common-schemas) repositorio.


> [!NOTE]
> Al revisar manifests de entity, puedes encontrar campos sin documentar. Esto se debe a que el schema de entity permite propiedades personalizadas adicionales, establecidas libremente por el propietario.


### Files <a href="#files" id="files"></a>

Como se mencionó arriba, todas las entities tienen al menos un archivo asociado: el manifest JSON que describe la entity en sí. El identificador de la entity es en realidad el [identificador del archivo](/docs/contributor/contributor-es/contenido/file-system/#identifiers) de este archivo especial.

El `content` campo dentro de cada manifest es un array de referencias a archivos adicionales. Estos suelen ser assets, como modelos 3D y animaciones, o scripts para scenes.

Todos los archivos se almacenan en el [sistema de archivos distribuido](/docs/contributor/contributor-es/contenido/file-system/), y cada elemento del array tiene dos propiedades:

| Campo     | Valor                                                                                                                                    |
| --------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| `archivo` | El nombre interno que usan los archivos de esta entity para referenciarse entre sí.                                                      |
| `hash`    | El [identificador global de este archivo](/docs/contributor/contributor-es/contenido/file-system/#identifiers), único en todo el contenido. |

Así es como suele verse dentro del `content` campo:

```json
[
  {
    "file": "thumbnail.png",
    "hash": "bafkreiglecvpnqibvf6pltcnid5nhbx3caj77lu4ia4xitpmp3lrcouuhm",
  },
  {
    "file": "model.glb",
    "hash": "bafkreic2i3awiu7srhatf3k47l3c5lmadisznjigppor2a35saosjfbo25",
  },
  // ...más archivos
]
```


> [!NOTE]
> El `archivo` El valor del campo siempre está en minúsculas, para evitar problemas al construir entities en diferentes sistemas operativos, donde las mayúsculas y minúsculas del nombre de archivo pueden ser importantes.


La vida útil de un archivo está ligada a la entity que lo contiene. Para las entities activas (es decir, aún no reemplazadas por su propietario), el protocolo exige que los content servers conserven todos los archivos asociados. Si se elimina la entity, los archivos pueden conservarse o descartarse a discreción del server.

### Propiedad y autenticación <a href="#ownership" id="ownership"></a>

Para demostrar la propiedad y autorizar acciones relacionadas con las entities, se utiliza el [cadena de autenticación](/docs/contributor/contributor-es/autenticacion/authchain/) mecanismo.

El [`decentraland-crypto`](https://github.com/decentraland/decentraland-crypto) repositorio contiene la implementación de todos los procedimientos criptográficos.

### Descubrir y descargar Entities

Los content servers pueden usarse para localizar entities usando [pointers](/docs/contributor/contributor-es/contenido/pointers/), y para descargar sus manifests y cualquier archivo adicional.

* Para resolver un pointer en un ID de entity, puedes usar el [`/entities/active`](https://decentraland.github.io/catalyst-api-specs/#tag/Content-Server/operation/getListOfEntities) endpoint.
* Usando el ID de la entity, puedes descargar el manifest con el [`/contents/<id>`](https://decentraland.github.io/catalyst-api-specs/#tag/Content-Server/operation/getContentFile) endpoint.
* Para obtener todas las entities activas de cierto tipo, empieza descargando un [instantánea](/docs/contributor/contributor-es/contenido/snapshots/).

Consulta la [práctica](https://github.com/decentraland/docs/tree/main/contributor/content/practice/README.md) sección para ver ejemplos y guías.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/contenido/entities.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
