> Muestra una etiqueta personalizada encima del nametag de un Avatar para mostrar un rango, un rol o un equipo.

# Nametags de Avatar

El `AvatarNametag` El componente añade una pequeña placa encima de la placa de nombre normal de un avatar, mostrando el texto que tu escena elija. Úsalo para mostrar un rango, un rol, un equipo o cualquier otra cosa que tu escena rastree sobre un jugador, como "VIP", "Equipo Rojo" o "Propietario del club".

<figure><img src="/docs/_remote/bb915fa6311eac2059c6e8e8.webp" alt="An avatar with a Club Owner plate floating above its regular nametag"><figcaption><p>Una placa proporcionada por la escena encima de la placa de nombre normal del jugador</p></figcaption></figure>

Esta página explica cómo añadir una placa, darle estilo y mantenerla sincronizada a medida que los jugadores entran y salen.


> [!WARNING]
> **📔 Nota**: `AvatarNametag` requiere `@dcl/sdk` la versión 7.28.0 o posterior.


## Añadir una placa al jugador

El único campo obligatorio es `label`, el texto que se mostrará.

```ts
import { AvatarNametag, engine } from '@dcl/sdk/ecs'

// Mostrar "Propietario del club" sobre la placa de nombre del jugador actual
AvatarNametag.create(engine.PlayerEntity, {
	label: 'Propietario del club',
})
```

La placa usa por defecto los colores de la placa de nombre del propio cliente, así que coincide con el resto de la interfaz sin trabajo extra.


> [!NOTE]
> **💡 Consejo**: No necesitas esperar a que el avatar termine de cargarse antes de escribir el componente. Si el avatar aún se está cargando, la placa se aplica en cuanto esté lista.


## Dónde funciona el componente

`AvatarNametag` solo hace algo en una Entity que tenga un avatar:

* `engine.PlayerEntity`, el jugador que ejecuta la escena.
* La Entity de otro jugador, encontrada a través de su [`PlayerIdentityData`](/docs/creator/content-creator-es/scenes-sdk7/interactividad/user-data/) component.
* Una [avatar NPC](/docs/creator/content-creator-es/scenes-sdk7/interactividad/avatares/npc-avatars/), es decir, cualquier Entity con un `AvatarShape` component.

En cualquier otra Entity, el componente se ignora.


> [!WARNING]
> **📔 Nota**: La placa es local para el cliente que la dibuja. Nunca se envía a otros jugadores. Si quieres que todos vean las mismas placas, cada cliente debe ejecutar la misma lógica, basada en datos que todos puedan leer. Consulta [Placas en escenas multijugador](#plates-in-multiplayer-scenes).


## Cambiar los colores

Tres campos opcionales toman un [Color3](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/color-types/):

* `labelColor`: el color del texto. Por defecto, usa el color de texto nativo de la placa de nombre del cliente.
* `backgroundColor`: el relleno de la placa. Por defecto, usa el color de fondo nativo de la placa de nombre del cliente.
* `borderColor`: el contorno de la placa. Por defecto, `backgroundColor`, así que no hay borde visible a menos que lo configures.

`Color3` no tiene canal alfa, y el componente no tiene campo de opacidad, así que la placa siempre es totalmente opaca.

```ts
import { AvatarNametag, engine } from '@dcl/sdk/ecs'
import { Color3 } from '@dcl/sdk/math'

// Una etiqueta blanca sobre una placa azul oscuro, con un borde azul claro
AvatarNametag.create(engine.PlayerEntity, {
	label: 'Estudiante',
	labelColor: Color3.White(),
	backgroundColor: Color3.create(0.1, 0.2, 0.6),
	borderColor: Color3.create(0.78, 0.85, 1),
})
```

Para volver a los colores nativos de un campo, vuelve a escribir el componente sin ese campo. Usa `createOrReplace` en lugar de `getMutable`, porque reemplazar el componente completo es lo que borra los campos que omitiste:

```ts
// Mantén la etiqueta, quita los colores personalizados y vuelve al aspecto nativo
AvatarNametag.createOrReplace(engine.PlayerEntity, { label: 'Estudiante' })
```

## Eliminar una placa

Usa `deleteFrom` para quitar la placa. La placa de nombre normal del avatar no se ve afectada:

```ts
AvatarNametag.deleteFrom(engine.PlayerEntity)
```

## Comportamiento de la etiqueta

Hay algunos detalles que conviene conocer antes de elegir el texto de la etiqueta:

* **Las etiquetas son de una sola línea.** Nunca se ajustan en varias líneas.
* **Las etiquetas largas se recortan** con puntos suspensivos. Mantén las etiquetas en una o dos palabras.
* **Una etiqueta vacía** (`''`) dibuja la placa sin texto.
* **Los espacios se conservan**, así que una etiqueta formada solo por espacios ensancha una placa sin texto. Es una forma de darle tamaño a una placa de color liso.
* **Para ocultar la palabra pero mantener su ancho**, establece `labelColor` al mismo valor que `backgroundColor`.


> [!NOTE]
> **💡 Consejo**: Una placa no necesita ningún texto. Para mostrar una placa codificada por color por sí sola, por ejemplo para marcar la pertenencia a un equipo, establece `label` a una cadena de espacios y elige un `backgroundColor`. Más espacios hacen una placa más ancha.
> 
> ```ts
> AvatarNametag.createOrReplace(engine.PlayerEntity, {
> 	label: '      ',
> 	backgroundColor: Color3.Red(),
> })
> ```
> 


## Etiquetar a los jugadores a medida que llegan

La forma más sencilla de etiquetar a otros jugadores es adjuntar el componente cuando entren en la escena, usando el `onEnterScene` evento. Te entrega una Entity nueva cada vez, así que no hay nada que buscar ni almacenar en caché:

```ts
import { onEnterScene } from '@dcl/sdk/players'
import { AvatarNametag } from '@dcl/sdk/ecs'

export function main() {
	onEnterScene((player) => {
		AvatarNametag.createOrReplace(player.entity, { label: 'VIP' })
	})
}
```

Consulta [Obtener datos del jugador](/docs/creator/content-creator-es/scenes-sdk7/interactividad/user-data/) para obtener más información sobre hacer seguimiento de los jugadores cuando entran y salen.

## Etiquetar a todos los jugadores en la escena

Para mostrar una placa sobre todos a la vez, itera sobre las Entity que tienen un `PlayerIdentityData` component.

Recopila las Entity en un array **antes de** escribir en ellas. Escribir un componente mientras aún se itera sobre `engine.getEntitiesWith()` puede invalidar la consulta a mitad de camino.

```ts
import { AvatarNametag, engine, Entity, PlayerIdentityData } from '@dcl/sdk/ecs'

const LABELS = ['Equipo Azul', 'Equipo Rojo']

function tagAllPlayers() {
	// 1. Recopila primero
	const players: { entity: Entity; address: string }[] = []
	for (const [entity, identity] of engine.getEntitiesWith(PlayerIdentityData)) {
		players.push({ entity, address: identity.address.toLowerCase() })
	}

	// Ordenar por dirección hace que todos los clientes lleguen al mismo resultado,
	// sin importar en qué orden se unieron los jugadores
	players.sort((a, b) => a.address.localeCompare(b.address))

	// 2. Luego escribe, pero solo cuando la etiqueta realmente cambie
	players.forEach(({ entity }, index) => {
		const label = LABELS[index % LABELS.length]
		if (AvatarNametag.getOrNull(entity)?.label === label) return
		AvatarNametag.createOrReplace(entity, { label })
	})
}
```

Comparar con el valor actual antes de escribir evita actualizaciones innecesarias del componente en cada pasada.

### Mantente al día con los jugadores que entran y salen

Los ids de Entity de los jugadores son **inestables entre desconexiones**. Cuando un jugador se va, su id de Entity se recicla y puede asignarse al siguiente jugador que se conecte, así que una placa escrita en una Entity obsoleta termina sobre el avatar equivocado.

Dos reglas mantienen esto correcto:

* Resuelve la Entity de nuevo justo antes de **cada** escritura, en lugar de almacenar la Entity en caché.
* Elimina el componente cuando un jugador se vaya.

Volver a ejecutar la función de etiquetado con un temporizador detecta a los jugadores que se unen más tarde y nunca conserva Entity obsoletas. Una vez por segundo es suficiente:

```ts
let timer = 0

engine.addSystem((dt: number) => {
	timer += dt
	if (timer < 1) return
	timer = 0
	tagAllPlayers()
})
```

## Placas en escenas multijugador

Las placas son locales para el cliente de cada jugador. La placa nunca se envía a otros jugadores; la copia de la escena de cada jugador calcula sus propias placas. Si todos los jugadores deben ver las mismas etiquetas, la lógica que las asigna debe producir el mismo resultado en cada cliente a partir de datos que todos compartan. Ordenar a los jugadores por dirección de Wallet, como en el ejemplo anterior, es una forma de hacerlo. De lo contrario, tu escena necesita sincronizar las asignaciones por sí misma; consulta [Multijugador sin servidor](/docs/creator/content-creator-es/scenes-sdk7/networking/serverless-multiplayer/).

## Interacción con placas de nombre ocultas

Una [`AvatarModifierArea`](/docs/creator/content-creator-es/scenes-sdk7/interactividad/avatares/modifier-areas/#hide-nametags) con el `AMT_HIDE_NAMETAGS` modificador oculta estas placas junto con las placas de nombre normales. Lo mismo aplica a `AMT_HIDE_AVATARS`, que oculta el avatar completo.

Si quieres que un avatar muestre **solo** tu placa y ningún nombre debajo, establece el `AvatarShape` del componente `nombre` a una cadena vacía. Consulta [Avatares NPC](/docs/creator/content-creator-es/scenes-sdk7/interactividad/avatares/npc-avatars/#add-a-label-above-the-name).

## Páginas relacionadas

* [Áreas modificadoras de avatares](/docs/creator/content-creator-es/scenes-sdk7/interactividad/avatares/modifier-areas/): oculta avatares y placas de nombre dentro de una región de la escena.
* [Mover al jugador](/docs/creator/content-creator-es/scenes-sdk7/interactividad/avatares/move-player/) y [Animaciones de avatar](/docs/creator/content-creator-es/scenes-sdk7/interactividad/avatares/avatar-animations/): controla el avatar del jugador.
* [Avatares NPC](/docs/creator/content-creator-es/scenes-sdk7/interactividad/avatares/npc-avatars/): muestra avatares que no son jugadores.
* [Obtener datos del jugador](/docs/creator/content-creator-es/scenes-sdk7/interactividad/user-data/): lee datos sobre el jugador y todos los demás en la escena.
* [Tipos de color](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/color-types/): cómo construir `Color3` valores.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/interactividad/avatares/avatar-nametags.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
