> Representaciones de Wearables de tokens de terceros

# Wearables vinculados

## Acerca de

De acuerdo con la [propuesta inicial de DAO para Linked Wearables](https://governance.decentraland.org/proposal/?id=14e76cc0-2bc7-11ec-ac84-77607720a240) (anteriormente llamada: Third Party Wearables), la [propuesta de borrador con definiciones finales](https://governance.decentraland.org/proposal/?id=f69c4d40-aaaf-11ec-87a7-6d2a41508231) y la [propuesta de rediseño de Linked Wearables](https://decentraland.org/governance/proposal/?id=65caf8d1-8601-49a5-ae11-b0b99d7fdd3c), este documento servirá como documentación para cubrir todos los detalles relevantes sobre la función Linked Wearables.

Este documento está orientado principalmente a representantes de comunidades de NFT que quieren dar a sus usuarios la posibilidad de representar sus NFTs como wearables mientras pasean por Decentraland.

## ¿Qué son los Linked Wearables?

Los Linked Wearables son representaciones 3D de NFTs que provienen de fuera de Decentraland y que pueden usarse como wearables en el mundo, pueden equiparse en el avatar y se encuentran en la mochila. No son [wearables regulares](https://github.com/decentraland/docs/tree/main/creator/wearables-and-emotes/wearables/README.md#what-are-wearables). Tienen el mismo aspecto y siguen las wearables regulares [pautas](/docs/creator/content-creator-es/wearables-y-emotes/wearables/creating-wearables/) pero tienen un significado completamente distinto.

Los Linked Wearables no existen dentro de colecciones tradicionales de wearables (pertenecen a un tipo especial de colección), no tienen rareza y no pueden venderse en [primarios](https://market.decentraland.org/browse?assetType=item&section=wearables) o [secundarios](https://market.decentraland.org/browse?assetType=nft&section=wearables&vendor=decentraland&page=1&sortBy=recently_listed&onlyOnSale=true&viewAsGuest=false&onlySmart=false) mercados. Solo son **representaciones en el mundo vinculadas a NFTs externos.**

> Imagina que tienes un proyecto NFT llamado ‘Cryptojackets’ en el que cada NFT es un tipo diferente de chaqueta 2D y quieres que tus usuarios tengan una representación 3D de su chaqueta en su mochila de Decentraland. Linked Wearables te permite enviar representaciones 3D de tus NFTs como wearables en Decentraland. No hace falta acuñar un nuevo token, y tu proyecto NFT actual tendrá una nueva función lista para usar para ofrecer.

Todos los Linked Wearables se definen dentro de una **Linked Wearable Collection**. Veremos cómo crear una más adelante en el artículo.

## ¿Cómo representan los Linked Wearables a los NFTs?

Los wearables se vinculan a tus NFTs creando una Linked Wearable Collection en el [sitio de Builder](https://decentraland.org/builder) y configurando cómo se representarán tus NFTs al momento de crear los wearables.

Admitimos 4 mecanismos para vincular tus wearables con NFTs. Todos estos mecanismos usan el token id de los NFTs para hacer coincidirlos.

La siguiente tabla muestra los mecanismos mencionados:

| Coincidencia   |                                                                                                                                                                                                                                                                     |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Todos los NFTs | Un usuario que posea cualquiera de los NFTs de la colección será propietario del wearable                                                                                                                                                                           |
| NFT único      | Un usuario que posea un NFT especificado por TOKEN ID será propietario del wearable. *p. ej. 123456. El usuario será propietario del wearable si posee el NFT con TOKEN ID: 123456*                                                                                 |
| Múltiples NFTs | Un usuario que posea uno de varios NFTs especificados por TOKEN IDs, descritos separados por comas, será propietario del wearable. *p. ej. 123456, 123457, 123458. El usuario será propietario del wearable si posee el NFT con TOKEN ID: 123456 o 123457 o 123458* |
| Rango de NFTs  | Un usuario que posea uno de varios NFTs especificados dentro del rango de TOKEN IDs será propietario del wearable. *p. ej. \[1, 1000]. El usuario será propietario del wearable si posee el NFT con TOKEN ID 1 o 2 o 3 o 4 o 5 ... o 100*                           |

## Tipos de colecciones de Linked Wearables

Normalmente hay dos tipos de colecciones de NFT:

* **Hechas a mano:** donde cada activo token está hecho a medida, o hecho a mano, sin ningún proceso de automatización.
* **Programáticas:** donde cada activo token no se elaboró individualmente a mano, sino que se generó automáticamente con código, muchas veces a partir de atributos que previamente se diseñaron y modelaron. Por ejemplo: [CryptoPunks](https://opensea.io/collection/cryptopunks) y [BAYC](https://opensea.io/collection/boredapeyachtclub) son ejemplos de colecciones NFT de pfp en 2D que se crearon de forma programática.

Seguimos el mismo principio en Decentraland con las Linked Wearables Collections también; puede haber colecciones estándar de Linked Wearables para las colecciones hechas a mano y colecciones programáticas de Linked Wearables para las generadas automáticamente. Ambas colecciones difieren en los costos de publicación que tienen los wearables. Consulta la [Costs section](#costs) para obtener más información sobre las tarifas de publicación.

## Creación de Linked Wearables

## Creación de una Linked Wearable Collection

Crear una Linked Wearable Collection es el primer paso para crear nuestros Linked Wearables.

Los Linked Wearables se agrupan en colecciones que pueden ser creadas, editadas y eliminadas por sus propietarios. Cada colección puede contener un número arbitrario de Linked Wearables. Cada colección estará vinculada a una colección de NFT; por esa razón, **se requiere un contrato NFT (compatible con ERC721 o ERC1155) para crear una Linked Wearable Collection.**

Para crear un nuevo Linked Wearable, sigue estos pasos:

1. En la sección de la colección, haz clic en el **Create Collection** botón.

   ![](/docs/_remote/4e2aa2f187322d76f74c1b4c.webp)
2. Selecciona la opción Linked Wearable Collection haciendo clic en el **Create Collection** botón debajo de la sección Linked Collections.

   ![](/docs/_remote/23dbdab92c38a595e6d609ae.webp)
3. Elige un nombre para la colección y vincula la colección a tu colección de NFT configurando su dirección de contrato y la red en la que se encuentra. **El contrato se validará para asegurarse de que cumple con los estándares de contratos NFT.**

   ![](/docs/_remote/8365f405886986e6be3328d6.webp)
4. Haz clic en el **Create** botón para crear la colección.

## Agregar wearables a la Linked Wearables Collection - uno por uno

Es posible, como ya ocurre con los wearables estándar, subir los modelos 3D de tus wearables uno por uno.

Para hacerlo, sigue estos pasos:

1. Haz clic en el **New items** botón.

   ![](/docs/_remote/e3843eed8d736f40c93c6f2e.webp)
2. Selecciona la **Singe items** option.

   ![](/docs/_remote/444a582037b01c922c963b76.webp)
3. Sigue los pasos para subir y configurar tu wearable tal como se describe en las [creating wearables guidelines](/docs/creator/content-creator-es/wearables-y-emotes/wearables/creating-wearables/) y configura cómo se vinculará a tus NFTs. Consulta la "[¿Cómo representan los Linked Wearables a los NFTs?](#how-do-linked-wearables-represent-nfts)" sección para obtener más información sobre cómo configurarlo.

   ![](/docs/_remote/3f230f70eb074d1a7d79fecf.webp)

## Agregar wearables a la Linked Wearables Collection - en lote

Como las colecciones de Linked Wearables pueden contener una gran cantidad de elementos, es posible subir los modelos 3D y la información de los wearables en lote. Este proceso implica crear un archivo zip con todos los assets que necesita un elemento para cada uno de los elementos. **Se recomienda subir wearables en lote solo para colecciones programáticas**.

### Creación del ZIP del wearable

Cada elemento requerirá crear un archivo ZIP que incluya los siguientes assets:

* El **requeridos** archivos de modelo 3D del wearable (GLB, GLTFs, archivos de textura, etc).
* Un **requeridos** `wearable.json` archivo que contiene la información del wearable.
* Un **opcional** `thumbnail.png` archivo que contiene la miniatura del wearable que se verá en el Builder y en el mundo. Si no se proporciona, se generará una usando el modelo 3D.

Los modelos 3D y la opcional `thumbnail.png` siguen la [Custom Thumbnails section](/docs/creator/content-creator-es/wearables-y-emotes/gestionar-colecciones/uploading-wearables/#custom-thumbnails) en el artículo Uploading Wearables sobre cómo crear una miniatura personalizada.

El `wearable.json` que acompaña al contenido de los wearables tiene el siguiente formato (escrito como lo haría TypeScript):

```typescript
type WearableConfiguration = {
  /** The URN of the wearable */
  id?: string
  /** Name of the wearable */
  name: string
  /** Description of the wearable */
  description?: string
  data: {
    /** Wearables to replace when equipping the wearable */
    replaces: WearableCategory[]
    /** Wearables to hide when equipping the wearable */
    hides: WearableCategory[]
    /** Tags to identify the wearable */
    tags: string[]
    /** Representations of the wearable */
    representations: WearableRepresentation[]
    /** Category of the wearable */
    category: WearableCategory
  },
  /** The mapping definition of the wearable with NFTs */
  mapping: Mapping
}

type WearableRepresentation = {
  /** Body shape of the representation */
  bodyShapes: BodyShape[];
  /** File path to the main file of the representation (GLB, GLTF, etc) */
  mainFile: string;
  /** A list of the file paths of the files that belong to the representation */
  contents: string[];
  /** Wearables to hide when equipping this representation */
  overrideHides: WearableCategory[];
  /** Wearables to replace when equipping this representation */
  overrideReplaces: WearableCategory[];
}

enum WearableCategory = {
  EYEBROWS = 'eyebrows',
  EYES = 'eyes',
  FACIAL_HAIR = 'facial_hair',
  HAIR = 'hair',
  HEAD = 'head',
  BODY_SHAPE = 'body_shape',
  MOUTH = 'mouth',
  UPPER_BODY = 'upper_body',
  LOWER_BODY = 'lower_body',
  FEET = 'feet',
  EARRING = 'earring',
  EYEWEAR = 'eyewear',
  HAT = 'hat',
  HELMET = 'helmet',
  MASK = 'mask',
  TIARA = 'tiara',
  TOP_HEAD = 'top_head',
  SKIN = 'skin'
}

enum WearableBodyShape {
  MALE = 'urn:decentraland:off-chain:base-avatars:BaseMale',
  FEMALE = 'urn:decentraland:off-chain:base-avatars:BaseFemale'
}

type Mapping = SingleMapping | AnyMapping | RangeMapping | MultipleMapping

enum MappingType {
  SINGLE = 'single',
  ANY = 'any',
  MULTIPLE = 'multiple',
  RANGE = 'range'
}

type SingleMapping = {
  type: MappingType.SINGLE
  id: string
}

type AnyMapping = {
  type: MappingType.ANY
}

type RangeMapping = {
  type: MappingType.RANGE
  from: string
  to: string
}

type MultipleMapping = {
  type: MappingType.MULTIPLE
  ids: string[]
}
```

El siguiente es un ejemplo de un `wearable.json` archivo que contiene un modelo para cada forma corporal:

```json
{
  "id": "urn:decentraland:matic:collections-thirdparty:my-third-party:my-collection:1",
  "name": "Sombrero especial",
  "description": "Una descripción del wearable",
  "data": {
    "replaces": [],
    "hides": ["hair"],
    "tags": ["especial", "nuevo", "sombrero"],
    "representations": [
      {
        "bodyShapes": ["urn:decentraland:off-chain:base-avatars:BaseMale"],
        "mainFile": "aMaleModelFile.glb",
        "contents": ["aMaleModelFile.glb", "aTextureFile.png"],
        "overrideHides": [],
        "overrideReplaces": []
      },
      {
        "bodyShapes": ["urn:decentraland:off-chain:base-avatars:BaseFemale"],
        "mainFile": "aFemaleModelFile.glb",
        "contents": ["aFemaleModelFile.glb", "anotherTextureFile.png"],
        "overrideHides": [],
        "overrideReplaces": []
      }
    ],
    "category": "hat",
    "mapping": {
      "type": "single",
      "id": "123"
    }
  }
}
```

Este archivo se comprimirá en zip junto con el `aMaleModelFile.glb`, `aTextureFile.png`, `aFemaleModelFile.glb` y `anotherTextureFile.png`.

Para agregar una miniatura personalizada al wearable, puedes añadir una `thumbnail.png` archivo.

Algunas cosas a considerar sobre el `wearable.json` archivo:

* Toda la información sobre las categorías de wearables y cuál elegir puede encontrarse en la [creating wearables guidelines](/docs/creator/content-creator-es/wearables-y-emotes/wearables/creating-wearables/).
* El array representations contendrá la información sobre cómo se verá cada forma corporal. Cada wearable DEBE contener al menos una representación (puede tener una o las dos), es decir, teniendo en cuenta las formas corporales que tenemos actualmente, ya sea `urn:decentraland:off-chain:base-avatars:BaseMale` o `urn:decentraland:off-chain:base-avatars:BaseFemale`. Cada representación describirá qué modelos se usarán para cada forma corporal.
* El objeto mapping debe configurarse como uno de los mecanismos disponibles para vincular tu NFT con tu wearable, siguiendo la "[¿Cómo representan los Linked Wearables a los NFTs?](#how-do-linked-wearables-represent-nfts)" sección.

**Configurar un ID o URN personalizado para los elementos**

El `id` campo es opcional y puede usarse para crear un wearable con un ID específico para actualizarlo en el futuro en lote (lo cual se explica más adelante en la [Editing wearables in bulk](#editing-wearables-in-bulk) sección).

En caso de que el `id` campo se utilice, debe contener el ID completo del wearable. El ID (o URN) del wearable se escribe como `urn:decentraland:matic:collections-thirdparty:third-party-id:collection-id:item-id`. Donde, `urn:decentraland:matic:collections-thirdparty:third-party-id:collection-id` se puede obtener desde la página de la colección y el `item-id` es el identificador personalizado del elemento que quieres usar.

Los IDs o URNs siguen un formato específico y aceptan:

* Caracteres en minúscula, de la `a` hasta la `z`.
* Todos los números.
* No pueden contener otro tipo de caracteres ni espacios en blanco. Te sugerimos reemplazar los espacios en blanco con el `-`

Puedes obtener el ID (o URN) desde la página de la colección siguiendo estos pasos:

1. Entrando en la vista de la colección de la que quieres copiar el URN y haciendo clic en la opción Edit URN del menú de opciones:

![](/docs/_remote/72142577aacca52f35921c2a.webp)

2. Copiando el identificador que está debajo del campo de texto:

![](/docs/_remote/8d7643cea8e5a23b82de3fbb.webp)

Por ejemplo, si el URN o ID obtenido desde la UI para la colección es `urn:decentraland:matic:collections-thirdparty:my-third-party:my-collection` y estás identificando tus wearables numéricamente, el URN del ejemplo sería `urn:decentraland:matic:collections-thirdparty:my-third-party:my-collection:1`, siendo `1` el número del wearable.

### El proceso de carga

Una vez que todos los archivos estén listos, para subir los wearables en lote, sigue estos pasos:

1. Haz clic en el **New items** botón.

   ![](/docs/_remote/e3843eed8d736f40c93c6f2e.webp)
2. Selecciona la **Multiple items** option.

   ![](/docs/_remote/444a582037b01c922c963b76.webp)
3. Haz clic en el **Browse yor computer** enlace para abrir tu gestor de archivos y seleccionar todos los zip que contienen tus wearables.

   ![](/docs/_remote/d5fe0416bee0c4f5d15fe1ac.webp)
4. Revisa si todos los archivos son correctos o si necesitan corregirse. En este caso, el modelo del wearable no está configurado o el `wearable.json` archivo tiene una representación configurada incorrectamente.

   ![](/docs/_remote/9af2b80ef49b3ab34f730589.webp)
5. Corrige cualquier error haciendo clic en el **Add more** botón y volviendo a subir los archivos fallidos con el mismo nombre o descartando los errores usando el icono de papelera en la sección superior derecha del modal.

   ![](/docs/_remote/bebf438cdc984a5547355b9e.webp)
6. Sube todos los wearables haciendo clic en **Upload items**.

   ![](/docs/_remote/56c36770612421c06d0c4eb5.webp)
7. ¡Ten paciencia, esto puede tardar un poco!

   ![](/docs/_remote/2418ecc5ff115dc5b7837888.webp)
8. ¡Éxito! Tus elementos ya están disponibles en tu colección.

   ![](/docs/_remote/e983b87c56423a0df39dd0f5.webp)
9. Selecciona si tu colección es programática o estándar. Consulta la [NFT Collections & Linked Wearables Collections](#types-of-linked-wearables-collections) sección para configurar correctamente para qué tipo de colección estás creando elementos.

   ![](/docs/_remote/e70222ca6d6afc784bbdbab3.webp)

### Errores comunes al subir elementos por lotes

* El `id` el campo está establecido en un valor que ya está siendo usado por otro wearable.
* El `id` el campo está establecido en un valor que no es un ID válido. Por ejemplo, el third party id o el collection id pertenecen a otro third party o colección.
* No hay `wearable.json` archivo en el zip.
* El archivo ZIP no tiene en su directorio raíz el `wearable.json` archivo.
* El `wearable.json` tiene un formato o valores incorrectos.
* El archivo es más grande que 3 MB. Linked Wearables tienen la misma limitación que los wearables regulares en cuanto al tamaño que los estándar.
* La imagen de miniatura opcional personalizada no es un archivo png.

## Ver los wearables en Decentraland

Los Linked Wearables pueden verse en el mundo para revisar cómo funcionará el modelo una vez publicado y aprobado.

Para poder ver un wearable en el mundo, sigue estos pasos:

1. Haz clic en el menú de tres puntos (tres puntos horizontales) a la derecha del elemento que quieres ver en el mundo. Aparecerá un desplegable. Selecciona **See in Decentraland**.

   ![](/docs/_remote/c816f508064933bb206516a8.webp)
2. Se abrirá el World de Decentraland. Ve a tu mochila para ver el wearable.

   ![](/docs/_remote/e27530d66df008ecb1568921.webp)


> [!WARNING]
> **⚠️ Aviso**: La vista previa en el mundo funciona con la versión web desactualizada del Decentraland Client. Aún no es posible probarlos en el Decentraland Desktop Client 2.0.


## Edición de Linked Wearables

## Edición del nombre de la colección

El creador puede cambiar el nombre de una colección **solo si la colección no tiene wearables publicados**.

Para editar el nombre de una Linked Wearable Collection sigue estos pasos:

1. Haz clic en el nombre de la colección.

   ![](/docs/_remote/39383f19e2bc56803e0a516b.webp)
2. Elige un nuevo nombre para la colección y haz clic en el botón de guardar.

   ![](/docs/_remote/30585b27d6df4a4b988c3e13.webp)

## Eliminación de la colección

El propietario puede eliminar una colección **solo si la colección no tiene wearables publicados**.

Para eliminar Linked Wearable Collection sigue estos pasos:

1. Haz clic en el menú de tres puntos (tres puntos horizontales) en el extremo derecho del conjunto de botones. Aparecerá un desplegable. Selecciona **Delete**.

   ![](/docs/_remote/72142577aacca52f35921c2a.webp)
2. Aparecerá un modal de confirmación; si deseas continuar, haz clic en **Ok**, de lo contrario haz clic en **Cancel**.

   ![](/docs/_remote/7188871c151f9edf2199a42e.webp)

## Edición de un wearable individual

### Edición de las propiedades del wearable

Para editar un wearable individual, sigue estos pasos:

1. Haz clic en el menú de tres puntos (tres puntos horizontales) a la derecha del elemento que quieres ver en el mundo. Aparecerá un desplegable. Selecciona **Open in editor**.

   ![](/docs/_remote/abfed6b24f3e8e71867a0391.webp)
2. Edita el wearable como se editan los wearables estándar. Sigue la **Editing items** sección en [creating wearables guidelines](/docs/creator/content-creator-es/wearables-y-emotes/wearables/creating-wearables/) sobre cómo crear una miniatura personalizada.

### Editando la vinculación del wearable

La vinculación de los wearables con la colección NFT es una de las propiedades más importantes de un Linked Wearable. Para editar cómo se vinculan con los NFT, puedes cambiar rápidamente el valor de vinculación desde la vista de la colección, sin necesidad de navegar a otra página.

![](/docs/_remote/b9bb316d91d55debb38713cb.webp)

Consulta la "[¿Cómo representan los Linked Wearables a los NFTs?](#how-do-linked-wearables-represent-nfts)" sección para obtener más información sobre cómo vincular tus wearables.

## Editing wearables in bulk

Siguiendo la misma idea vista anteriormente en la [Creación de wearables en lote](#adding-wearables-to-the-linked-wearables-collection-in-bulk) sección, los administradores de terceros pueden realizar cambios en lote en los wearables.

Para realizar cambios en lote en wearables, es necesario crear un archivo ZIP para cada uno de los wearables que se cambiarán.

Estos archivos ZIP deben crearse siguiendo el formato descrito en [Creación de wearables en lote](#adding-wearables-to-the-linked-wearables-collection-in-bulk) con una excepción, en el `wearable.json` archivo, la `id` propiedad **DEBE** establecerse en el `id` o URN del wearable que se cambiará. Esto es obligatorio, ya que es la única forma de identificar el wearable que se cambiará. Si creaste tus wearables en lote proporcionando un id en el `wearable.json` archivo, puedes reutilizar sus `wearable.json` archivos para actualizarlos.

Teniendo en cuenta el ejemplo de la [Creación de wearables en lote](#adding-wearables-to-the-linked-wearables-collection-in-bulk) sección, si quisiéramos cambiar algunas de las propiedades de un wearable, por ejemplo, el nombre al que olvidamos agregarle un número, deberíamos incluir un `wearable.json` archivo en el zip como en el siguiente ejemplo:

```json
{
  "id": "urn:decentraland:matic:collections-thirdparty:my-third-party:my-collection:1",
  "name": "A hat 1",
  "description": "Una descripción del wearable",
  "data": {
    "replaces": [],
    "hides": ["hair"],
    "tags": ["especial", "nuevo", "sombrero"],
    "representations": [
      {
        "bodyShapes": ["urn:decentraland:off-chain:base-avatars:BaseMale"],
        "mainFile": "aMaleModelFile.glb",
        "contents": ["aMaleModelFile.glb", "aTextureFile.png"],
        "overrideHides": [],
        "overrideReplaces": []
      },
      {
        "bodyShapes": ["urn:decentraland:off-chain:base-avatars:BaseFemale"],
        "mainFile": "aFemaleModelFile.glb",
        "contents": ["aFemaleModelFile.glb", "anotherTextureFile.png"],
        "overrideHides": [],
        "overrideReplaces": []
      }
    ],
    "category": "hat"
  },
  "mapping": {
    "type": "single",
    "id": "123"
  }
}
```

Donde el `id` campo se establece en el `id` o URN del wearable que se cambiará y el `name` campo se establece en el nuevo nombre del wearable.

Una vez que los archivos ZIP estén listos, sigue estos pasos para editar los items en lote:

1. Haz clic en el menú de tres puntos (tres puntos horizontales) en el extremo derecho del conjunto de botones. Aparecerá un desplegable. Selecciona **Editar en lote**.

   ![](/docs/_remote/0c8a87dd214ea40c8a6ed252.webp)
2. Aparecerá un modal similar al de la **Carga de modelos en lote** . Haz clic en el **Explora tu computadora** enlace para abrir tu gestor de archivos.

   ![](/docs/_remote/a39bb531aa4d0dbae170bd92.webp)
3. Selecciona todos los archivos ZIP de los elementos que se editarán.

   ![](/docs/_remote/d5fe0416bee0c4f5d15fe1ac.webp)
4. Revisa si todos los archivos son correctos o si necesitan corregirse. En este caso, el modelo del wearable no está configurado o el `wearable.json` archivo tiene una representación configurada incorrectamente.

   ![](/docs/_remote/4a7baded9874d3cc155f2fbd.webp)
5. Corrige cualquier error haciendo clic en el **Add more** botón y volviendo a subir los archivos fallidos con el mismo nombre o descartando los errores usando el icono de papelera en la sección superior derecha del modal.

   ![](/docs/_remote/bebf438cdc984a5547355b9e.webp)
6. Sube todos los wearables haciendo clic en **Upload items**.

   ![](/docs/_remote/42f683d2a6f504f05485aec8.webp)
7. ¡Ten paciencia, esto puede tardar un poco!

   ![](/docs/_remote/5894f0726ee565293b5d41a2.webp)
8. ¡Éxito! Tus elementos ya están disponibles en tu colección. Revisa la publicación del foro para ver cualquier actualización del curador.

   ![](/docs/_remote/fb076a01ef98b4db599d68cf.webp)

## Publicación de tus Linked Wearables

Tus Linked Wearables deben pasar por un proceso de publicación y curación, igual que los wearables regulares. Aunque el proceso de publicación y curación no es el mismo que el de los wearables regulares, mantiene los mismos pasos: todos los elementos deben publicarse primero para luego ser curados por un curador asignado.

Las siguientes secciones te mostrarán cómo publicar tus Linked Wearables para que sean curados.

## Costos

Crear Linked Wearables tiene un costo según el tipo de Linked Wearable Collection que hayas elegido construir:

1. **Estándar**: cada wearable publicado cuesta lo mismo que un wearable regular: 100 USD pagaderos en MANA.
2. **Programática**: una tarifa fija, pagada una sola vez por todos los wearables que publicarás. Esta tarifa equivale a publicar 20 wearables regulares: 2000 USD pagaderos en MANA.

Para obtener más información sobre el tipo de colección que estás creando, consulta la [NFT Collections & Linked Wearables Collections](#types-of-linked-wearables-collections) sección.

## Publicar wearables para revisión

Una vez que tus wearables estén listos, deben publicarse para su curación. Tus wearables se publican en grupos de elementos; puedes elegir cuáles están listos para ser curados seleccionándolos y haciendo clic en el `Publicar` botón. Después de publicar elementos, la publicación se bloqueará hasta que los que ya están publicados sean curados.

Para publicar tus wearables, necesitas:

1. Selecciona los elementos que se publicarán haciendo clic en la casilla de verificación junto a ellos. Haz clic en el **Publicar** botón cuando estés listo con tu selección.

   ![](/docs/_remote/52a58c4e5c9954ebc8fc269b.webp)
2. Confirma el nombre de tu colección. Una vez que publiques tus elementos, cambiar el nombre de la colección no será posible, así que asegúrate de revisarlo detenidamente.

   ![](/docs/_remote/52a58c4e5c9954ebc8fc269b.webp)
3. Revísalo para los elementos que estás publicando. Haz clic en el **Confirmar elementos** botón cuando estés listo.

   ![](/docs/_remote/8a8e23ad31bfa01cd5132c54.webp)
4. Firma la confirmación de publicación de tus elementos en tu Wallet.

   ![](/docs/_remote/88c6a6a00343c5fac618b93d.webp)
5. Lee y revisa los Términos y Condiciones.

   ![](/docs/_remote/5eeaa4e5bf1e5c04ddcfe174.webp)
6. Revisa tus tarifas de publicación. Las tarifas requeridas para publicar los wearables se describen en la [Costs section](#costs).

   ![](/docs/_remote/763b602c672547a4d2a6101b.webp)
7. Si es la primera vez que publicas Linked Wearables, deberás autorizar al smart contract de Linked Wearables a operar MANA en tu nombre. Este paso es necesario para deducir el MANA utilizado para pagar las tarifas de publicación de tu Wallet.

   ![](/docs/_remote/86176e8cb7ef6bc5bbe8c139.webp)
8. Paga la tarifa de publicación y completa la publicación realizando la transacción. Dependiendo de la congestión de la red, esto puede tardar un poco.

   ![](/docs/_remote/bdbd4028179d589523236310.webp)
9. ¡Éxito! Has publicado tus elementos. Tus elementos pasarán por el proceso de curación, como lo hacen las colecciones regulares. Puedes comunicarte con el curador a través de la publicación del foro.

   ![](/docs/_remote/1c4b8bde3d0569bee971bd77.webp)

## Enviar cambios para revisión

Los wearables publicados y aprobados que se editen deben pasar nuevamente por el proceso de curación. No te preocupes, no se aplicarán tarifas a los wearables que ya estén publicados.

Para enviar cambios y lograr que sean curados, necesitas:

1. Selecciona los elementos con cambios haciendo clic en la casilla de verificación junto a ellos. Haz clic en el **Enviar cambios** botón cuando estés listo con tu selección.

   ![](/docs/_remote/2cb347394e74593c7becf04c.webp)
2. Continúa con el proceso de envío de cambios. Como indica el modal, estos cambios deberán pasar nuevamente por la fase de curación.

   ![](/docs/_remote/3a60e8ad8fb155002a8e4cef.webp)
3. Lee y revisa los Términos y Condiciones. Al aceptarlos, los elementos estarán listos para ser curados nuevamente.

   ![](/docs/_remote/14b970d046bc0d27c4efbb13.webp)

## Curación

Al igual que con los wearables regulares, tus modelos 3D necesitarán la aprobación del Curators Committee. No estás exento de esta regla, ya que la estética y la jugabilidad de Decentraland aún deben estar protegidas.

El proceso de curación diferirá según el proceso utilizado para generar los wearables. Las colecciones de Linked Wearables admiten wearables artesanales y generados programáticamente.

### Wearables artesanales

Para los modelos 3D que se hicieron individualmente sin ningún proceso automatizado (el método habitual para la mayoría de los wearables regulares), el Curator deberá revisar todos los elementos de la colección individualmente para asegurarse de que todos cumplan con las Wearable Guidelines. [Wearable Guidelines](/docs/creator/content-creator-es/wearables-y-emotes/wearables/creating-wearables/).

### Colecciones programáticas

Para las colecciones programáticas, no todos los elementos tienen que ser curados individualmente. La cantidad de elementos que se deben curar en cada colección depende del tamaño de la colección; esto fue definido por la DAO en [esta propuesta](https://governance.decentraland.org/proposal/?id=f69c4d40-aaaf-11ec-87a7-6d2a41508231).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/wearables-y-emotes/wearables/linked-wearables.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
