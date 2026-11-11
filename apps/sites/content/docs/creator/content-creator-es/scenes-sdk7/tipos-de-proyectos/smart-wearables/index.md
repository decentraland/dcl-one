> Crea wearables con capacidades interactivas

# Smart Wearables

Las prendas inteligentes son un tipo de escena global. Al igual que [las experiencias portátiles](/docs/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/portable-experiences/), son mecánicas de juego que los jugadores llevan consigo mientras se desplazan por el metaverso. Por ejemplo, mientras ejecuta una escena global, un jugador podría coger una bola de nieve del suelo en Genesis Plaza, alejarse a otra escena y lanzar la bola de nieve a otro jugador que también esté jugando el mismo juego.

Las prendas inteligentes son experiencias portátiles que se activan cuando el jugador se pone una determinada prenda. Las prendas inteligentes pueden otorgar a los jugadores nuevas habilidades, como una jetpack que les permite volar, o añadir una nueva capa de contenido sobre el resto del mundo, como colocar monedas aleatoriamente para que se recojan por toda Genesis City.


> [!WARNING]
> **📔 Nota**: Smart Wearables solo se pueden crear usando SDK 7.
> 
> El **Creator Hub** no admite actualmente la creación de proyectos de Smart Wearables.


## Empezar

### Usar la CLI

1. Abre una línea de comandos en una carpeta nueva y ejecuta

`npx @dcl/sdk-commands init --project smart-wearable`

Este comando crea los archivos y la estructura básicos para un nuevo wearable inteligente.

## Los archivos de la plantilla

La carpeta de un proyecto de Smart Wearable recién creado es muy similar a la de una [escena de Decentraland](/docs/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/scene-files/), pero notarás las siguientes diferencias:

* `wearable.json` incluye todos los metadatos de la experiencia portátil
* Hay un modelo 3D de marcador de posición (glasses.glb) y una miniatura (glasses.png) para un par de gafas oscuras. Debes reemplazarlos por el wearable real que estás creando
* `scene.json` incluye `"isPortableExperience": true`. Los Parcels no son relevantes para el wearable publicado, que funciona en cualquier lugar, pero puedes configurarlos para pruebas locales: la vista previa solo se ejecuta en los parcels enumerados en `scene.json`

## Acerca de wearable.json

El `wearable.json` archivo predeterminado se ve así:

```json
{
	"data": {
		"replaces": [],
		"hides": [],
		"tags": ["special", "new", "eyebrows"],
		"representations": [
			{
				"bodyShapes": [
					"urn:decentraland:off-chain:base-avatars:BaseMale",
					"urn:decentraland:off-chain:base-avatars:BaseFemale"
				],
				"mainFile": "glasses.glb",
				"contents": ["glasses.glb"],
				"overrideHides": [],
				"overrideReplaces": []
			}
		],
		"category": "eyewear"
	},
	"name": "Smart Wearable Example",
	"description": "Pon las gafas para ver un nuevo mundo",
	"rarity": "mythic"
}
```

Estos son los campos más comunes en `wearable.json`:

* `id`: Identificador único del wearable inteligente.
* `nombre` (obligatorio): El nombre del wearable que los usuarios verán en Marketplace
* `description`: La descripción del wearable que los usuarios verán en Marketplace. Asegúrate de indicar qué puede hacer el wearable inteligente, ya que los usuarios de Marketplace no tendrán forma de previsualizar su funcionalidad antes de comprarlo. Puede tener como máximo **64 caracteres** de longitud. Puedes escribir una descripción más larga en el editor de ítems de Builder después de subirlo.
* `rarity`: La cantidad disponible del token según su rareza. Los valores posibles son:
  * único (1 copia)
  * mítico (10 copias)
  * exótico (50 copias)
  * legendario (100 copias)
  * épico (1000 copias)
  * raro (5000 copias)
  * poco común (10.000 copias)
  * común (100.000 copias)


> [!WARNING]
> **📔 Nota**: Si hiciste un fork de tu proyecto a partir de uno existente y incluye un `id`, asegúrate de que el valor sea único antes de publicar tu wearable. Usa [uuidgenerator.net](https://www.uuidgenerator.net/) para generar un nuevo UUID aleatorio


* `data` (obligatorio): Incluye lo siguiente. `reemplaza a`, `hides`, `tags`, `representations` y `category` deben estar todos presentes. `reemplaza a`, `hides` y `tags` pueden ser listas vacías `[]`, pero `representations` necesita al menos una entrada. También puedes ajustar esta configuración desde la interfaz de Builder una vez que subas el wearable inteligente.
  * `reemplaza a`: Lista de categorías de otros wearables que deben quitarse al equipar este wearable, además del valor predeterminado de esta categoría. Ej.: Al ponerse una capa de torso superior, también ocultar los pies.
  * `hides`: Lista de categorías de otros wearables que deben ocultarse (pero no quitarse) al equipar este wearable, además del valor predeterminado de esta categoría.
  * `tags`: Etiquetas usadas para que el wearable sea localizable en Marketplace.
  * `representations`:
    * `bodyShapes`: La lista de representaciones de avatar que pueden usar este wearable. Todos los smart wearables deben estar disponibles para ambas body shapes, masculina y femenina.
    * `mainFile`: El archivo principal con el modelo 3D del wearable.
    * `contents`: La lista completa de archivos usados para renderizar el modelo 3D del wearable. Por ejemplo, el modelo 3D podría incluir texturas como archivos separados.
    * `overrideHides`: Cualquier excepción al comportamiento predeterminado de ocultar de esta categoría de wearable. *hide* de esta categoría de wearable.
    * `overrideReplaces`: Cualquier excepción al comportamiento predeterminado de ocultar de esta categoría de wearable. *replace* de esta categoría de wearable.
  * `category`: Qué categoría de wearable usar. Los valores posibles son:
    * 'eyebrows'
    * 'eyes'
    * 'facial\_hair'
    * 'hair'
    * 'mouth'
    * 'upper\_body'
    * 'lower\_body'
    * 'feet'
    * 'earring'
    * 'eyewear'
    * 'hat'
    * 'helmet'
    * 'mask'
    * 'tiara'
    * 'top\_head'
    * 'skin'
    * 'hands\_wear'


> [!WARNING]
> **📔 Nota**: Además de los campos anteriores, Builder también acepta un campo opcional de nivel superior `mapping` y campos opcionales `removesDefaultHiding` y `blockVrmExport` dentro de `data`. Los smart wearables normalmente no los necesitan. Builder rechaza un `wearable.json` que tenga cualquier otro campo de nivel superior, con el error *"the wearable config file is invalid"*. Las versiones anteriores de esta página enumeraban `menuBarIcon`, `model`, `bodyShape` y `collectionAddress`: elimínalos si tu proyecto todavía los tiene. Aparece el mismo error si `description` es más largo que 64 caracteres.


## La miniatura

Debes incluir una imagen llamada `thumbnail.png` en el nivel raíz de tu carpeta. Esta imagen se mostrará tanto en la mochila como en Marketplace, para representar tu wearable en 2D. El tamaño de imagen recomendado y requerido es 256x256.

Elige una imagen que establezca las expectativas del jugador y represente correctamente tu creación.

## La vista previa

Ejecutar una vista previa de un wearable inteligente es igual que ejecutar la de una escena: haz clic **Play** en Creator Hub, o ejecuta `npm run start` en la línea de comandos. Creator Hub no puede crear un proyecto de wearable inteligente, pero sí puede abrirlo y previsualizarlo.

La vista previa ejecuta el código de tu wearable como una escena normal en los parcels enumerados en `scene.json`, no como una experiencia portátil. Esto significa que algunos comportamientos difieren del wearable publicado:

* El código que solo funciona en la escena en la que está parado el jugador, como [las fuerzas físicas sobre el jugador](/docs/creator/content-creator-es/scenes-sdk7/interactividad/player-physics/), deja de funcionar cuando sales de los parcels de la vista previa. Una vez publicado, el wearable inteligente sigue funcionando dondequiera que vaya el jugador.
* Es posible que el modelo 3D del wearable no se muestre en tu avatar.

Para probar el wearable puesto y funcionando en cualquier lugar, súbelo a Builder y usa **Ver en Decentraland** (consulta [Publicar](#publish)). No necesitas mintear ni publicar la colección para hacer esto.

Para probar cómo se comporta el wearable inteligente en el contexto de una escena, también puedes ejecutar una vista previa de tu wearable al mismo tiempo que ejecutas una vista previa de una o varias escenas usando un [espacio de trabajo](/docs/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/workspaces/). Por ejemplo, puedes ejecutar tu wearable inteligente junto con la [escena de Genesis Plaza](https://github.com/decentraland-scenes/Genesis-Plaza) para probar cómo se comporta en una escena concurrida, mientras estás en un ascensor, etc.

## Consejos

* Al posicionar una Entity, ten en cuenta que las posiciones son globales, relativas a las coordenadas 0,0 de Genesis Plaza.
* Para reaccionar ante jugadores cercanos:
  * Consulta [Obtén todos los jugadores](/docs/creator/content-creator-es/scenes-sdk7/interactividad/user-data/#fetch-all-players) para saber cómo obtener datos de otros jugadores en los alrededores.
  * Ten en cuenta que la carga del wearable inteligente, las escenas circundantes y los demás jugadores puede producirse en distintos órdenes según la situación. Si el jugador entra en Decentraland con el wearable inteligente ya puesto, es probable que la escena global de tu wearable se cargue antes que la de otros jugadores. Por otro lado, si el jugador carga primero una escena y luego se pone el wearable, es probable que los demás jugadores ya estén cargados cuando la escena del wearable empiece a ejecutarse.
  * Los demás jugadores solo se cargan una vez que el jugador está conectado a una isla dentro de su realm. Obtén los datos del realm y comprueba el campo ‘room’. Si el campo ‘room’ es null, el jugador aún no está conectado a una isla y los demás jugadores todavía no se cargarán. Puedes comprobar esto periódicamente cada 1 segundo hasta que el campo ‘room’ esté presente, y solo inicializar tu lógica entonces. Esto solo te permite ver a otros jugadores: para compartir datos entre quienes llevan wearables, consulta [Limitaciones](#limitations).
* Para interactuar con las escenas circundantes:
  * No puedes enviar directamente ninguna instrucción a escenas cercanas ni a otras experiencias portátiles; el `messageBus` está actualmente aislado en un sandbox para cada experiencia portátil/escena.
  * Puedes usar un servidor intermedio para enviar información entre la experiencia portátil y una escena.
  * Si haces un raycast, puedes detectar impactos contra los colliders de Entities de las escenas circundantes. Esto puede indicarte la ubicación exacta del impacto, la dirección normal e incluso el nombre de la Entity y el nombre del mesh del modelo 3D. Esto solo funciona al golpear Entities en escenas escritas con SDK7.
* Terminar una experiencia portátil: ejecuta el `kill()` método para finalizar por sí misma una experiencia portátil.
* Empujar al jugador: [Física](/docs/creator/content-creator-es/scenes-sdk7/interactividad/player-physics/) funciones como `Physics.applyImpulseToPlayer()` funcionan en cualquier lugar al que vaya quien lo lleve, ya que la escena de un wearable inteligente siempre cuenta como la escena en la que está el jugador. No se necesita permiso. Las escenas pueden desactivar los wearables inteligentes para sus visitantes; consulta [Conmutadores de funciones](/docs/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/scene-metadata/#feature-toggles).
* Reproducir un sonido que solo escucha quien lo lleva: adjunta una Entity al jugador con [AvatarAttach](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/entity-positioning/#attach-an-entity-to-an-avatar) y dale un `AudioSource`. Los demás jugadores no lo oyen.
* Cargar archivos antes de que se necesiten: usa el [AssetLoad](/docs/creator/content-creator-es/scenes-sdk7/optimizacion/pre-load-resources/) componente para obtener sonidos, modelos o archivos de emotes cuando el wearable se inicie, para que el primer uso no tenga que esperar una descarga.

## Publicar

Para publicar tu wearable inteligente:

1. Asegúrate de que la información en `wearable.json` sea correcta. Si usaste otro proyecto como punto de partida, asegúrate de que el `id` sea un identificador único, no usado por otros wearables.
2. Ejecuta `npm run pack` en la carpeta de tu proyecto. Esto genera un `archivo smart-wearable.zip` en la carpeta de tu proyecto.


> [!WARNING]
> **📔 Nota**: El contenido sin comprimir de `archivo smart-wearable.zip` debe sumar menos de 3 MB, o Builder rechaza el archivo por ser demasiado grande. `npm run pack` muestra un mensaje de tamaño cuando el proyecto supera los 2 MB, pero aun así crea el zip. Revisa el tamaño de los archivos dentro del zip, no el tamaño del zip en sí. Consulta [Limitaciones](#limitations).


3. Abre Builder, abre la pestaña Collections y haz clic en + para subir un nuevo wearable.
4. Arrastra tu archivo comprimido `archivo smart-wearable.zip` al Builder y verifica que toda la información sea correcta.

> Nota: Si tu wearable tiene un modelo 3D diferente para cada body shape, enumera una representación por cada body shape:
>
> 1. En tu proyecto, crea una nueva carpeta para cada representación(`male` y `female`), y coloca el modelo 3D de cada representación en su carpeta correspondiente.
> 2. Actualiza tu `wearable.json` archivo para incluir las nuevas representaciones.
> 3. ```lang-json
>    "representations": [{
>      "bodyShapes": ["urn:decentraland:off-chain:base-avatars:BaseMale"],
>      "mainFile": "male/glasses.glb",
>      "contents": ["male/glasses.glb"],
>      "overrideHides": [],
>      "overrideReplaces": []
>    },
>    {
>      "bodyShapes": ["urn:decentraland:off-chain:base-avatars:BaseFemale"],
>      "mainFile": "female/glasses.glb",
>      "contents": ["female/glasses.glb"],
>      "overrideHides": [],
>      "overrideReplaces": []
>    }],
>    ```
> 4. Ejecuta `npm run pack` para generar un nuevo archivo smart-wearable.zip.
> 5. Arrastra el nuevo archivo smart-wearable.zip al Builder.

5. Abre el editor de wearables y asegúrate de que las categorías “hide” y “remove” estén configuradas correctamente para desactivar otras categorías de wearables cuando este wearable esté puesto.
6. Crea una nueva colección con este wearable y quizá otros wearables.
7. En la colección, haz clic **Ver en Decentraland**. Esto abre Decentraland en la red de pruebas Sepolia, donde puedes equipar los wearables de tu colección desde la mochila y ver cómo se comportan en un escenario más real, por ejemplo corriendo por Genesis Plaza. Consulta [Pruebas en Decentraland](/docs/creator/content-creator-es/wearables-y-emotes/gestionar-colecciones/uploading-smart-wearables/#testing-in-decentraland).
8. En este punto, tu wearable está listo para publicarse.

## Permissions

Los smart wearables se ejecutan con permisos restringidos. Para usar cualquiera de las siguientes funciones, declara el permiso correspondiente en la lista `requiredPermissions` de tu `scene.json`, o la llamada fallará en tiempo de ejecución:

* `USE_FETCH`: solicitudes HTTP con `fetch` ni a `signedFetch`
* `USE_WEBSOCKET`: conexiones WebSocket
* `USE_WEB3_API`: transacciones de Wallet o firma de mensajes
* `OPEN_EXTERNAL_LINK`: Pedir al jugador que abra un enlace externo

```json
"requiredPermissions": [
    "USE_FETCH"
  ],
```

Cuando un wearable inteligente declara cualquiera de estos, los jugadores ven una solicitud de autorización cuando se lo ponen, y el wearable no se ejecuta si la rechazan. Declara solo lo que uses. Consulta [Permisos requeridos](/docs/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/scene-metadata/#required-permissions) para más detalles.

## Limitaciones

> IMPORTANTE: Todo el wearable inteligente debe caber dentro de 3 MB. Esto incluye el modelo 3D, la miniatura, el código, las bibliotecas, los archivos de sonido, modelos 3D adicionales, imágenes de UI, etc. Este límite corresponde a la carpeta sin comprimir. Builder no te permitirá subir wearables más grandes que esto. Para comprobar el tamaño, descomprime el `archivo smart-wearable.zip` archivo generado y comprueba el tamaño de la carpeta, o ejecuta `unzip -l smart-wearable.zip` y lee el total.

Para mantener archivos fuera del zip, enuméralos en el `.dclignore` archivo del proyecto, de la misma manera que lo harías para una escena.


> [!WARNING]
> **📔 Nota**: Si editas tu proyecto de wearable inteligente en Creator Hub, puede guardar una captura de pantalla a tamaño completo de la vista 3D como `assets/Images/autogenerated-thumbnail.png` y apuntar `scene.json` a ella. Esa imagen puede ocupar varios MB por sí sola y hacer que el wearable supere el límite. Los wearables inteligentes no la usan, así que añade `assets/Images/autogenerated-thumbnail.png` en `.dclignore`.


Los smart wearables solo ejecutan la experiencia portátil para el jugador que lleva el wearable puesto. Los demás jugadores no ven los efectos. Por ejemplo, si la experiencia portátil renderiza una mascota que sigue al jugador, los demás jugadores alrededor no verán esa mascota. Sin embargo, los demás jugadores sí verán a los avatares realizar animaciones que se ejecutan como parte de la escena del wearable, incluso [animaciones personalizadas de avatar](/docs/creator/content-creator-es/scenes-sdk7/interactividad/avatares/avatar-animations/#custom-animations) subidas como parte de los archivos del wearable inteligente.

Los smart wearables no tienen multijugador integrado. Cada jugador que lleva el wearable ejecuta su propia copia separada de su código, incluso cuando dos personas que lo llevan están una al lado de la otra. El `MessageBus`, sincronizando Entities con `syncEntity()` y en el [Servidor multijugador](/docs/creator/content-creator-es/scenes-sdk7/networking/authoritative-servers/) no funciona en un wearable inteligente. La única forma de compartir datos entre quienes lo llevan, como una puntuación compartida, es un servidor propio con el que el wearable hable mediante `fetch` o un WebSocket, consulta [Conexiones de red](/docs/creator/content-creator-es/scenes-sdk7/networking/network-connections/).

Una animación personalizada puede incluir props, y los props pueden incluir efectos de partículas hechos a partir de huesos animados (consulta [Props y sonidos](/docs/creator/content-creator-es/wearables-y-emotes/emotes/props-and-sounds/) y [Partículas en Emotes](/docs/creator/content-creator-es/wearables-y-emotes/emotes/particles-in-emotes/)). Los demás jugadores también ven esto. El sonido es diferente: las animaciones personalizadas reproducidas con `triggerSceneEmote()` no reproducen audio, aunque el archivo de emote viniera con uno. Para reproducir un sonido, usa un `AudioSource`, que solo oye quien lleva el wearable.

Los smart wearables solo funcionan cuando los jugadores los llevan puestos. Por esta razón, los jugadores solo pueden tener un número limitado de smart wearables activados y, dependiendo de qué parte del cuerpo ocupen, algunos serán incompatibles con otros. Por ejemplo, no puedes llevar dos sombreros al mismo tiempo, ni un casco y cabello al mismo tiempo.

Si un wearable está “hidden” pero no “removed” por otros wearables que se lleven puestos, entonces el wearable inteligente puede seguir puesto, aunque el wearable correspondiente no sea visible.

## Ejemplos

[Ejemplo de wearable inteligente](https://github.com/decentraland/smart-wearable-sample))


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/smart-wearables.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
