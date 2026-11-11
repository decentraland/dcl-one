# Instantáneas

Los servidores de contenido compilarán periódicamente resúmenes de las entidades activas que alojan, llamados *instantáneas*. Son archivos regulares [archivos](/docs/contributor/contributor-es/contenido/file-system/) y pueden descargarse usando su identificador.

Las instantáneas se crean diariamente, semanalmente, mensualmente y anualmente. Cada una contiene el conjunto de entidades activas que cambiaron desde la instantánea anterior para ese intervalo.

Las instantáneas contendrán versiones en conflicto de las mismas entidades (es decir, diferentes [archivos manifest](/docs/contributor/contributor-es/contenido/entities/#properties) asociados al mismo pointer) a medida que se actualizan. Al analizarlas, los clientes deben conservar la versión de la instantánea más reciente. Dado que los servidores de contenido pueden eliminar archivos inactivos, no se garantiza que las versiones obsoletas de las entidades estén disponibles para su descarga.

Cuando una nueva instantánea *reemplaza* a las anteriores (por ejemplo, una instantánea semanal que combina una serie de instantáneas diarias), sus metadatos indican qué archivos previos son reemplazados para que los clientes no necesiten descargarlos.

El conjunto completo de entidades activas puede descubrirse combinando todas las instantáneas disponibles (más sobre esto a continuación), conservando la entidad más reciente referenciada por cada [pointer](/docs/contributor/contributor-es/contenido/pointers/) descubierto en el camino.

Puedes experimentar con las instantáneas usando código funcional en la [práctica](https://github.com/decentraland/docs/tree/main/contributor/content/practice/README.md) sección.

### Descubriendo instantáneas <a href="#discover" id="discover"></a>

Para localizar el conjunto actual de instantáneas, usa el [`instantáneas` endpoint](https://decentraland.github.io/catalyst-api-specs/#tag/Content-Server/operation/getSnapshots). La respuesta contiene una matriz de elementos con estos campos:

| Campo                     | Valor                                                                                            |
| ------------------------- | ------------------------------------------------------------------------------------------------ |
| `generationTimestamp`     | La marca de tiempo UTC de Unix cuando se creó esta instantánea.                                  |
| `hash`                    | La instantánea [archivo](/docs/contributor/contributor-es/contenido/file-system/).                  |
| `numberOfEntities`        | La cantidad de entradas en el archivo de instantánea.                                            |
| `replacedSnapshotHashes`  | Una matriz con los `hash` hashes de cualquier instantánea reemplazada por esta.                  |
| `timeRange.initTimestamp` | La marca de tiempo UTC de Unix (en milisegundos) para el inicio del intervalo de la instantánea. |
| `timerange.endTimestamp`  | La marca de tiempo UTC de Unix (en milisegundos) para el final del intervalo de la instantánea.  |

Por ejemplo:

```json
{
  "generationTimestamp": 1684979298844,
  "hash": "bafybeiflmm46nr4vv2h3wuzbx3pukcz7ju4fhbfzt6yxmoo533uktlgru4",
  "numberOfEntities": 12345,
  "replacedSnapshotHashes": [ "bafybeicw6x75ieaxfwynekbyhpcsgctpjkt6cb4j6oa7s57qjj6e4b5phd" ],
  "timeRange": {
     "initTimestamp": 1684281600000,
     "endTimestamp": 1684886400000
  }
}
```

### Descargando instantáneas <a href="#download" id="download"></a>

Usando el `hash` campo timeRange de una instantánea, los clientes pueden descargar las entidades asociadas contenidas creadas o actualizadas en ese intervalo de tiempo.

Las instantáneas comienzan con esta línea exacta:

```
### instantánea JSON de Decentraland
```

Después de eso, cada línea es un documento JSON que describe una [entidad](/docs/contributor/contributor-es/contenido/entities/) con los siguientes campos:

| Campo             | Valor                                                                                                                   |
| ----------------- | ----------------------------------------------------------------------------------------------------------------------- |
| `entityId`        | El identificador inmutable de esta [entidad](/docs/contributor/contributor-es/contenido/entities/).                        |
| `entityType`      | Uno de `escena`, `perfil`, `wearable`, `emote`, `store` o `outfits`.                                                    |
| `pointers`        | Una matriz de [pointers](/docs/contributor/contributor-es/contenido/pointers/) que resuelven (o resolvían) a esta entidad. |
| `entityTimestamp` | La marca de tiempo UTC de Unix (en milisegundos) cuando se cargó esta entidad.                                          |
| `authChain`       | El [cadena de autenticación](/docs/contributor/contributor-es/contenido/entities/#ownership) para esta entidad.            |

Una entrada típica luce así:

```json
{
  "entityId": "bafkreigrvaqynmiglpvewwhn2yd63q5dvagrrt5jbhimzvbrn5kimj5zne",
  "entityType": "wearable",
  "pointers": ["urn:decentraland:matic:collections-v2:0x11a6879861f36cbad632a4e7226816a16139fb33:0"],
  "entityTimestamp": 1671117456129,
  "authChain": [
    // ... cargas útiles y firmas de la cadena de autenticación
  ]
}
```


> [!NOTE]
> Si tienes intención de analizar una instantánea línea por línea, recuerda omitir (o mejor aún, validar) la primera con el encabezado, y prepárate para manejar una línea vacía al final del archivo.


#### Iniciar un índice de entidades <a href="#index-start" id="index-start"></a>

Los clientes que quieran indexar el conjunto completo de entidades activas deben procesar todas las instantáneas actualmente disponibles, y conservar la más reciente [entidad](/docs/contributor/contributor-es/contenido/entities/) para cada [pointer](/docs/contributor/contributor-es/contenido/pointers/).

La estrategia más simple es procesar las instantáneas en orden cronológico inverso (es decir, primero la más reciente), ignorando los pointers que ya se hayan descubierto, para conservar la referencia a la entidad más reciente.

En pseudocódigo:

```py
# Descarga el conjunto actual de instantáneas y ordénalas de la más nueva a la más antigua:
snapshots = get_snapshots()
snapshots.sort('timeRange.initTimestamp', DESCENDING)

seen_pointers = set()

# Procesa las instantáneas, conservando la entidad más nueva para cada pointer:
for snapshot in snapshots:
    items = get_snapshot_items(snapshot) 

    for item in items:
        if any(pointer in seen_pointers for pointer in item.pointers):
            discard(item)
        else:
            keep(item)
            seen_pointers.update(item.pointers)
```

Dado que las entidades individuales pueden ser referenciadas por múltiples pointers (como suele ocurrir con [escenas](/docs/contributor/contributor-es/contenido/tipos-de-entidad/scenes/)), todos ellos deben comprobarse antes de decidir si conservar o descartar el elemento.


> [!NOTE]
> Los archivos de instantáneas para los intervalos de tiempo más largos pueden ser muy grandes. Para propósitos de desarrollo y experimentación que no requieran indexar todo el conjunto de entidades, se recomienda usar las instantáneas más pequeñas. El conjunto resultante de entidades será incompleto pero válido.


#### Actualizar un índice de entidades <a href="#index-update" id="index-update"></a>

Los clientes que mantengan un índice de entidades actualizado pueden hacer llamadas periódicas al [`instantáneas`](https://decentraland.github.io/catalyst-api-specs/#tag/Content-Server/operation/getSnapshots) endpoint, y determinar si descargar cada archivo considerando:

* ¿Se ha descargado ya la instantánea identificada por `hash` ?
* ¿Está `hash` en la `replacedSnapshotHashes` lista de otra instantánea que ya se descargó?
* ¿Es la `timeRange` relevante para los propósitos actuales?

Si hay que procesar nuevas instantáneas, puede usarse la misma estrategia que arriba para actualizar un conjunto de datos existente.

### Ejemplos

En la [práctica](https://github.com/decentraland/docs/tree/main/contributor/content/practice/README.md) sección, encontrarás ejemplos de código que funcionan con el sistema de instantáneas.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/contenido/snapshots.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
