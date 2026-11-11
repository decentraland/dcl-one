> Muestra y controla el avatar NPC

# Avatares NPC

Muestra un avatar como una Entity en una Scene.


> [!NOTE]
> **💡 Consejo**: Prueba la [librería NPC Toolkit](https://github.com/decentraland-scenes/dcl-npc-toolkit) para una experiencia más sencilla al trabajar con NPCs, especialmente si necesitas interactuar mediante un árbol de conversación.


## Crear un avatar

El siguiente fragmento crea un avatar con wearables y body shape aleatorios, y el nombre "NPC".

```ts
const myAvatar = engine.addEntity()
AvatarShape.create(myAvatar)

Transform.create(myAvatar, {
	position: Vector3.create(4, 0.25, 5),
})
```

Al pasar datos para generar un `AvatarShape`, se requieren los siguientes campos:

* `id`: (obligatorio) Identificador interno del Avatar. Los últimos 4 caracteres del `id` se muestran después del nombre del NPC, en texto atenuado, como el sufijo de la Wallet de un jugador. Un `id` más corto que 4 caracteres muestra `0000`. Dale a cada NPC un `id` cuyo final se lea bien junto a su nombre, por ejemplo `'guide-0001'`.

Los siguientes campos opcionales también están disponibles:

* `nombre`: Nombre para mostrar sobre la cabeza del Avatar. Predeterminado: "NPC". Establécelo en una cadena vacía para no mostrar ningún nombre, lo cual es útil cuando solo quieres un [Avatar Nametag](/docs/creator/content-creator-es/scenes-sdk7/interactividad/avatares/avatar-nametags/) placa encima del NPC.
* `bodyShape`: Cadena para definir qué body shape usar. Las opciones válidas son 'urn:decentraland:off-chain:base-avatars:BaseMale' y 'urn:decentraland:off-chain:base-avatars:BaseFemale'.
* `wearables`: Array con la lista de URNs de wearables que el avatar lleva puestos actualmente. Si los wearables entran en conflicto (como si dos de ellos fueran sombreros), el último de la lista reemplaza al otro.
* `emotes`: Array con la lista de URNs de emotes NFT que el avatar puede reproducir
* `eyeColor`: *Color3* para el color de ojos (cualquier color es válido)
* `skinColor`: *Color3* para el color de piel (cualquier color es válido)
* `hairColor`: *Color3* para el color de pelo (cualquier color es válido)
* `talking`: Si *true*, muestra un conjunto verde de barras junto al nombre, como cuando los jugadores usan el chat de voz en el mundo.
* <div data-gb-custom-block data-tag="hint" data-style="info" class="hint hint-info"><p><strong>💡 Consejo</strong>: Consulta <a href="/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/color-types/">tipos de color</a> para más detalles sobre cómo configurar los colores.</p></div>


> [!WARNING]
> **📔 Nota**: El `AvatarShape`componente debe importarse mediante
> 
> > `import { AvatarShape } from "@dcl/sdk/ecs"`
> 
> Consulta [Imports](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/coding-scenes/#imports) para ver cómo manejarlos fácilmente.



> [!WARNING]
> **📔 Nota**: Los campos URN deben seguir el mismo formato usado para [NFTShapes](/docs/creator/content-creator-es/scenes-sdk7/media/display-a-certified-nft/): `urn:decentraland:<CHAIN>:<CONTRACT_STANDARD>:<CONTRACT_ADDRESS>:<TOKEN_ID>`


## Mover un NPC

Para mover un NPC, cambia el `posición` de su `Transform`. El avatar no salta a la nueva posición: camina hasta allí desde donde está, a unos 4 metros por segundo, y se mueve más rápido cuando el objetivo está a 1,5 metros de distancia o más. Mientras se mueve, se gira para mirar en la dirección en la que camina. El `rotation` que estableces en el `Transform` solo se aplica una vez que llega.

```ts
// El NPC camina desde su posición actual hasta la nueva
Transform.getMutable(myAvatar).position = Vector3.create(10, 0.25, 5)
```

Para controlar tú mismo el movimiento, usa un [Tween](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/move-entities/). Un Tween en la posición reemplaza esta caminata automática, y un Tween en la rotación evita que el NPC se gire para mirar hacia donde camina. Una Entity solo puede tener un Tween a la vez, así que no puedes hacer tween de la posición y la rotación de la misma Entity al mismo tiempo.

## Animaciones

Los Avatars reproducen animaciones idle predeterminadas cuando están quietos.

Para reproducir animaciones en el avatar, establece el `expressionTriggerId` string con el nombre de la animación que quieres reproducir.

```ts
const myAvatar = engine.addEntity()
AvatarShape.create(myAvatar, {
	id: 'dancer-0001',
	emotes: [],
	wearables: [],
	expressionTriggerId: 'robot',
})

Transform.create(myAvatar, {
	position: Vector3.create(4, 0.25, 5),
})
```

El `expressionTriggerId` campo admite todas las [animaciones predeterminadas](/docs/creator/content-creator-es/scenes-sdk7/interactividad/avatares/avatar-animations/#default-animations), así como animaciones personalizadas [de un archivo de Scene](/docs/creator/content-creator-es/scenes-sdk7/interactividad/avatares/avatar-animations/#custom-animations), e incluso URNs de emotes que están publicados en el Marketplace.

Para usar una animación personalizada, indica la ruta al `_emote.glb` archivo desde la carpeta raíz de tu proyecto, por ejemplo `'assets/animations/Cheer_emote.glb'`. Al igual que con [`triggerSceneEmote()`](/docs/creator/content-creator-es/scenes-sdk7/interactividad/avatares/avatar-animations/#custom-animations), una ruta incorrecta no reproduce nada.


> [!WARNING]
> **📔 Nota**: `AvatarShape` no tiene `mask` campo, así que una `expressionTriggerId` animación siempre se reproduce en todo el cuerpo. [Animar solo la parte superior del cuerpo](/docs/creator/content-creator-es/scenes-sdk7/interactividad/avatares/avatar-animations/#animate-only-the-upper-body) está disponible en el Avatar del propio jugador, no en los NPCs.


### Mover un NPC mientras reproduce un emote

Escribir la `Transform.position`, o ejecutar un [Tween](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/move-entities/) sobre él, cancela el emote que se está reproduciendo. El avatar vuelve a su ciclo de caminata, porque Decentraland calcula qué animación de locomoción debe mezclar a partir de cuánto se movió el avatar desde el último frame.

Para mover un NPC durante un emote, si el movimiento cubre un área pequeña quizá quieras hornear el movimiento dentro de la propia animación, como movimiento del root o de la cadera dentro del `_emote.glb` archivo.

### Animaciones en bucle

Las animaciones en un `AvatarShape` se reproducen una sola vez; si quieres que el avatar siga repitiendo una animación, deberías crear un System que le indique que reproduzca la animación otra vez cada par de segundos.

La única excepción son los emotes predeterminados en bucle, `sittingChair1`, `sittingChair2`, `sittingGround1` y `sittingGround2`, que mantienen su pose en un `AvatarShape` sin necesidad de reproducirlos de nuevo. Una animación personalizada desde un archivo nunca se repite en un `AvatarShape`, incluso si el clip en sí está marcado como en bucle, así que una pose personalizada sostenida necesita el patrón de repetición de abajo.

Usa el `expressionTriggerTimestamp` para reproducir el mismo emote. El valor de este campo es un [timestamp de Lamport](https://en.wikipedia.org/wiki/Lamport_timestamp), lo que significa que no es un valor temporal, sino más bien un índice que aumenta en 1 por cada repetición del emote.

Así que la primera vez que reproduces un emote, estableces `expressionTriggerTimestamp` en *0*. Para reproducir el emote de nuevo, debes actualizar este valor a 1. Así es como el Engine sabe que esta es una nueva instrucción, y no una instrucción sobre la que ya actuó.

El siguiente fragmento crea un System que ejecuta el mismo emote cada 2 segundos:

```ts
const myAvatar = engine.addEntity()
AvatarShape.create(myAvatar, {
	id: 'fan-0001',
	emotes: [],
	wearables: [],
	expressionTriggerId: 'clap',
    expressionTriggerTimestamp: 0
})

Transform.create(myAvatar, {
	position: Vector3.create(4, 0.25, 5),
})

let clapTimer = 0
let emoteDuration = 2  // 2 seconds

// sistema
engine.addSystem((dt: number) => {
    clapTimer += dt
      
    if (clapTimer >= emoteDuration) {
        // Activar el emote de aplauso
        AvatarShape.getMutable(myAvatar).expressionTriggerTimestamp += 1
        
        clapTimer = 0 // Reset timer
    }
})
```


> [!NOTE]
> **💡 Consejo**: Debes conocer la duración del emote y hacer que esa sea la duración del System. Si creas un emote que mantiene al avatar inmóvil en una misma pose, es recomendable hacer que la duración del emote sea mayor que la del System. De ese modo, puedes asegurarte de que no haya artefactos al terminar y reiniciar la animación.


## Añade una etiqueta encima del nombre

Usa el `AvatarNametag` componente para mostrar una placa con texto personalizado encima del nombre del NPC, por ejemplo un rol como "Shopkeeper" o "Boss". Adjúntalo a la misma Entity que tiene el `AvatarShape`:

```ts
AvatarNametag.create(myAvatar, { label: 'Shopkeeper' })
```

Si el `AvatarShape` tiene un `nombre`, solo se muestra la placa, sin una caja de nametag vacía debajo. Esto es útil para etiquetar un NPC únicamente con un título.


> [!NOTE]
> **💡 Consejo**: Para una placa con código de color y sin texto, establece `label` en una cadena de espacios y elige un `backgroundColor`. Los espacios se conservan, así que más espacios hacen una placa más ancha.
> 
> ```ts
> AvatarNametag.create(myAvatar, {
> 	label: '      ',
> 	backgroundColor: Color3.Red(),
> })
> ```
> 


Consulta [Avatar Nametags](/docs/creator/content-creator-es/scenes-sdk7/interactividad/avatares/avatar-nametags/) para la lista completa de campos, incluidos los colores.

## Copiar wearables del jugador

El siguiente fragmento cambia los wearables y otras características de un avatar NPC para que coincidan con los que el jugador lleva puestos actualmente. Esto podría usarse en una Scene como un maniquí, para mostrar un wearable o emote concreto combinado con el atuendo actual del jugador.

```ts
import { getPlayer } from '@dcl/sdk/src/players'


export function swapAvatar(avatar: Entity) {

  let userData = getPlayer()
  console.log(userData)

  if (!userData || !userData.wearables) return

  const mutableAvatar = AvatarShape.getMutable(avatar)

  mutableAvatar.wearables = userData.wearables
  mutableAvatar.bodyShape = userData.avatar?.bodyShapeUrn
  mutableAvatar.eyeColor = userData.avatar?.eyesColor
  mutableAvatar.skinColor = userData.avatar?.skinColor
  mutableAvatar.hairColor = userData.avatar?.hairColor
  
}
```

## Mostrar solo wearables

Usa el `showOnlyWearables` campo para mostrar solo los wearables enumerados de un avatar. El resto del cuerpo del avatar será invisible.

```ts
const myAvatar = engine.addEntity()
AvatarShape.create(myAvatar, {
	id: 'mannequin-0001',
	emotes: [],
	wearables: [
    'urn:decentraland:matic:collections-v2:0x90e5cb2d673699be8f28d339c818a0b60144c494:0'
  ],
	showOnlyWearables: true,
})

Transform.create(myAvatar, {
	position: Vector3.create(4, 0.25, 5),
})
```

Esto es útil para mostrar wearables, por ejemplo en una tienda.


> [!NOTE]
> **💡 Consejo**: Si un wearable es bastante pequeño, intenta establecer el `scale` del `Transform` a un valor mayor.


## Adjuntar una Entity a un NPC

Puedes usar la `AvatarAttach` función para fijar una Entity a uno de los bones de un avatar NPC, por ejemplo para que el NPC esté sosteniendo un objeto en su mano. La Entity se moverá junto con el avatar cuando se anime.

Para usar esta función, usa la `id` propiedad en el `AvatarShape` para asignar un id arbitrario a este avatar, y luego hacer referencia a ese id en el `AvatarAttach`. El id puede ser cualquier string que quieras.

```ts
// Crear NPC, con un ID
const myAvatar = engine.addEntity()
Transform.create(myAvatar, {
  position: Vector3.create(8, 0.25, 8),
})
AvatarShape.create(myAvatar, {
  id: "my-avatar-id", 
  wearables: [],
  emotes: []
})

// Crear objeto para adjuntar al NPC
const attachedEntity = engine.addEntity()
Transform.create(attachedEntity, {
    position: Vector3.create(4, 2, 4),
    scale: Vector3.create(0.15,0.15,0.15)
})
MeshRenderer.setBox(attachedEntity)
Material.setBasicMaterial(attachedEntity, { diffuseColor: Color4.Blue() })
AvatarAttach.create(attachedEntity, {
    avatarId: "my-avatar-id",
    anchorPointId: AvatarAnchorPointType.AAPT_LEFT_HAND
})
```

Aprende más sobre `AvatarAttach` component [aquí](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/entity-positioning/#attach-an-entity-to-an-avatar).

## Reemplaza el avatar del jugador

No puedes cambiar qué wearables lleva puestos el avatar del jugador, pero en su lugar puedes sustituir el avatar del jugador por un avatar NPC que puedes personalizar por completo.

Para hacer esto, oculta el propio avatar del jugador con una [Avatar Modifier Area](/docs/creator/content-creator-es/scenes-sdk7/interactividad/avatares/modifier-areas/#hide-avatars) que usa el `AMT_HIDE_AVATARS` modifier, y muestra una `AvatarShape` Entity en la posición del jugador en su lugar.


> [!WARNING]
> **📔 Nota**: Para permitir que el jugador tenga control total sobre ese avatar, deberías escuchar los Button Events para detectar cuándo pulsa un botón y luego activar la animación correspondiente en el avatar NPC. Consulta [Button Events](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/system-based-events/) para obtener más detalles.
> 
> La fluidez del control puede no ser perfecta al hacer esto; quizá quieras usarlo solo en casos muy específicos.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/interactividad/avatares/npc-avatars.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
