> Haz que el Avatar del jugador reproduzca animaciones predeterminadas o personalizadas, y detecta cuándo termina un emote

# Animaciones del Avatar

Puedes hacer que el jugador ejecute una animación como parte del código de la escena. Esto puede ayudar a aportar más inmersión y también a comunicar a los jugadores lo que otros jugadores están haciendo. Las animaciones del avatar las ven tanto el jugador (en vista de tercera persona) como los demás jugadores que estén cerca.

Las animaciones controladas por el jugador que se ejecutan en todo el cuerpo son anuladas por las animaciones de locomoción predeterminadas, como caminar y saltar. Por lo tanto, las animaciones de cuerpo completo reproducidas por la escena solo se ejecutan mientras el jugador permanece quieto. Si el jugador camina o salta, se interrumpe cualquier animación de cuerpo completo. Las animaciones de la parte superior del cuerpo no se interrumpen con el movimiento.


> [!WARNING]
> **📔 Nota**: Los jugadores solo pueden animarse si ya están dentro de los límites de la escena, no si están en una escena vecina. Los Wearables inteligentes pueden reproducir animaciones en cualquier lugar.
> 
> Mientras un jugador ejecuta una animación, no se ve afectado por las colisiones y sus movimientos no están restringidos por la física de la escena. Ten en cuenta también que, si una animación desplaza al jugador de su posición original (por ejemplo, si la animación implica un salto), el componente Transform del jugador no se verá afectado por este desplazamiento.


### Usa el Scene Editor

La forma más sencilla de hacer que un jugador ejecute una animación es usar el Scene Editor. Usa la opción sin código **Play Emote** para reproducir una animación predeterminada, o la opción **Play Custom Emote** para reproducir una animación desde un archivo. Consulta [Hacer inteligente cualquier elemento](/docs/creator/content-creator-es/scene-editor/interactividad/make-any-item-smart/).

### Animaciones predeterminadas

Usa la función `triggerEmote()` para ejecutar una de las animaciones predeterminadas que los jugadores pueden reproducir en cualquier lugar de Decentraland. Esta función recibe como argumento un objeto con las siguientes propiedades:

* `predefinedEmote`: Un nombre de cadena para un Emote existente.
* `mask`: (opcional) Reproduce la animación solo en una parte del cuerpo del Avatar, utilizando un valor de la enumeración `AvatarMask` . Por ejemplo, `AvatarMask.AM_UPPER_BODY` anima solo la parte superior del cuerpo del Avatar. Consulta [Animar solo la parte superior del cuerpo](#animate-only-the-upper-body).

```ts
import { triggerEmote } from '~system/RestrictedActions'

const emoter = engine.addEntity()
Transform.create(emoter, { position: Vector3.create(8, 0, 8) })
MeshRenderer.setBox(emoter)
MeshCollider.setBox(emoter)
pointerEventsSystem.onPointerDown(
	{
		entity: emoter,
		opts: { button: InputAction.IA_POINTER, hoverText: 'Dance' },
	},
	() => {
		triggerEmote({ predefinedEmote: 'robot' })
	}
)
```

Los siguientes Emotes muestran información sobre las acciones del jugador en tu escena; todos estos son valores válidos para el campo `predefinedEmote` :

* `buttonDown`
* `buttonFront`
* `getHit`
* `knockOut`
* `lever`
* `openChest`
* `openDoor`
* `punch`
* `push`
* `swingWeaponOneHand`
* `swingWeaponTwoHands`
* `throw`
* `sittingChair1`
* `sittingChair2`
* `sittingGround1`
* `sittingGround2`

Los cuatro Emotes `sitting` se repiten: el Avatar mantiene la pose hasta que el jugador se mueve o tu escena llama a `stopEmote({})`. `stopEmote` se resuelve con `{ success: boolean }`, que es `false` cuando el Explorer rechaza la llamada, por ejemplo, porque el jugador ya no está en tu escena. Todos los demás se reproducen una vez y el Avatar vuelve al estado inactivo.

Estos Emotes están disponibles para todos los jugadores en su rueda de Emotes predeterminada y también se pueden usar en cualquier escena.

* `wave`
* `fistpump`
* `robot`
* `raiseHand`
* `clap`
* `money`
* `kiss`
* `tik`
* `hammer`
* `tektonik`
* `dontsee`
* `handsair`
* `shrug`
* `disco`
* `dab`
* `headexplode`
* `crafting`
* `handsintheair`
* `Victory`
* `Waving`
* `fistpump_short`


> [!WARNING]
> **📔 Nota**: Usa estos nombres exactamente como se indican. Un nombre incorrecto falla silenciosamente en tu escena: no se reproduce nada y no aparece nada en la consola de la escena. Ten cuidado con `handsintheair` sin separadores y `fistpump_short` con su sufijo.



> [!NOTE]
> **💡 Consejo**: Si un jugador camina o salta mientras reproduce la animación, la interrumpirá. Si no quieres que esto sea posible, puedes congelar el Avatar con [Input Modifiers](/docs/creator/content-creator-es/scenes-sdk7/interactividad/avatares/locomotion/#freeze-the-player) durante la animación del Avatar.


### Animaciones personalizadas

Usa la función `triggerSceneEmote()` para hacer que el jugador ejecute una animación personalizada, almacenada como un archivo .glb entre los assets de la escena.


> [!NOTE]
> **💡 Consejo**: No tienes que crearlas tú mismo. El paquete **Avatar Animations** del Scene Editor tiene 45 animaciones listas para usar, incluidas poses sentadas, pesca, llevar una caja y bailar. Arrastra una a la vista 3D y el editor copia su archivo en tu escena; luego pasa la ruta de ese archivo a `triggerSceneEmote()`. Los archivos se ubican en `assets/asset-packs/<animation-folder>/`, por ejemplo `assets/asset-packs/whack/Whack_Avatar_emote.glb`. Verifica la ruta exacta en **Local Assets**. Consulta [Agregar elementos](/docs/creator/content-creator-es/scene-editor/primeros-pasos/about-editor/#avatar-animations).



> [!WARNING]
> **📔 Nota**: El nombre del archivo **debe** terminar en `_emote.glb` para funcionar como una animación de Avatar.


Esta función recibe un objeto con las siguientes propiedades:

* `src`: La ruta completa al archivo de Emote, contada desde la carpeta raíz de tu proyecto. Un archivo ubicado en `assets/animations/sit_emote.glb` necesita `'assets/animations/sit_emote.glb'`.
* `loop`: Si es true, la animación se repetirá continuamente hasta que el jugador se mueva o la animación se detenga. El valor predeterminado es false.
* `mask`: (opcional) Reproduce la animación solo en una parte del cuerpo del Avatar, utilizando un valor de la enumeración `AvatarMask` . Por ejemplo, `AvatarMask.AM_UPPER_BODY` anima solo la parte superior del cuerpo del Avatar. Consulta [Animar solo la parte superior del cuerpo](#animate-only-the-upper-body).


> [!WARNING]
> **📔 Nota**: `src` debe ser la ruta completa al archivo. No hay búsqueda por nombre de archivo y el sufijo `_emote` no se añade automáticamente. Si la ruta es incorrecta, `triggerSceneEmote()` se resuelve como `{ success: false }` y no se reproduce nada. Copia la ruta desde la carpeta de tu proyecto e incluye todas las carpetas en ella.


```ts
import { triggerSceneEmote } from '~system/RestrictedActions'
import { AvatarMask } from '@dcl/sdk/ecs'

const emoter = engine.addEntity()
Transform.create(emoter, { position: Vector3.create(8, 0, 8) })
MeshRenderer.setBox(emoter)
MeshCollider.setBox(emoter)
pointerEventsSystem.onPointerDown(
	{
		entity: emoter,
		opts: { button: InputAction.IA_POINTER, hoverText: 'Make snowball' },
	},
	() => {
		triggerSceneEmote({ src: 'assets/animations/Snowball_Throw_emote.glb', loop: false, mask: AvatarMask.AM_UPPER_BODY })
	}
)
```


> [!NOTE]
> **💡 Consejo**: Si un jugador camina o salta mientras reproduce la animación, la interrumpirá. Si no quieres que esto sea posible, puedes congelar el Avatar con [Input Modifiers](/docs/creator/content-creator-es/scenes-sdk7/interactividad/avatares/locomotion/#freeze-the-player) durante la animación del Avatar.


#### Props, partículas y sonido

Un archivo de Emote reproducido con `triggerSceneEmote()` puede incluir props, como una raqueta de tenis o un chorro de llamas, animados junto con el Avatar. Constrúyelo de la misma forma que un Emote de Emotes 2.0, siguiendo las reglas de nomenclatura de [Props y Sounds](/docs/creator/content-creator-es/wearables-y-emotes/emotes/props-and-sounds/):

* El archivo contiene dos clips de Animation: uno cuyo nombre incluye `_Avatar` para el Avatar, y otro cuyo nombre incluye `_Prop` para el Armature del prop.
* Ambos clips tienen la misma duración.
* Los efectos de partículas funcionan si los conviertes en Bones animados; consulta [Partículas en Emotes](/docs/creator/content-creator-es/wearables-y-emotes/emotes/particles-in-emotes/).

Los demás jugadores cercanos ven los props, igual que ven la animación del Avatar.


> [!WARNING]
> **📔 Nota**: `triggerSceneEmote()` no reproduce sonido. El Audio solo se reproduce para Emotes cargados como Emotes independientes en el Builder. Para asociar un sonido con un Emote de escena, reprodúcelo con un [AudioSource](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/sounds/) al mismo tiempo.


### Animar solo la parte superior del cuerpo

Tanto `triggerEmote()` como `triggerSceneEmote()` aceptan una propiedad opcional `mask` que limita la animación a solo una parte del cuerpo del Avatar.

Normalmente, una animación se detiene en cuanto el jugador camina o salta, porque las animaciones de locomoción predeterminadas toman control de todo el cuerpo. Cuando estableces `mask` en `AvatarMask.AM_UPPER_BODY`, la animación solo controla al Avatar de la cintura hacia arriba y las piernas permanecen controladas por las animaciones de locomoción predeterminadas. Esto significa que el jugador puede seguir caminando o corriendo por la escena mientras la parte superior del cuerpo reproduce tu animación. Usa esto para acciones que no deberían interrumpir el movimiento, como llevar una caja, sostener una antorcha o hacer malabares.

```ts
import { triggerSceneEmote } from '~system/RestrictedActions'
import {
	engine,
	AvatarMask,
	InputAction,
	MeshCollider,
	MeshRenderer,
	pointerEventsSystem,
	Transform,
} from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

const cheerButton = engine.addEntity()
Transform.create(cheerButton, { position: Vector3.create(8, 0, 8) })
MeshRenderer.setBox(cheerButton)
MeshCollider.setBox(cheerButton)
pointerEventsSystem.onPointerDown(
	{
		entity: cheerButton,
		opts: { button: InputAction.IA_POINTER, hoverText: 'Cheer' },
	},
	() => {
		triggerSceneEmote({
			src: 'assets/animations/Cheer_emote.glb',
			loop: true,
			mask: AvatarMask.AM_UPPER_BODY,
		})
	}
)
```

En este ejemplo, el jugador hace Click en un Button y comienza a animar con los brazos en alto, en una animación en bucle. Como solo se anima la parte superior del cuerpo, puede seguir corriendo por la escena mientras anima, por ejemplo, para seguir la acción en una carrera o un partido de fútbol.

Algunas cosas que debes tener en cuenta:

* `AvatarMask.AM_UPPER_BODY` es actualmente el único valor de la enumeración `AvatarMask` . Para reproducir una animación en todo el cuerpo, simplemente no establezcas la propiedad `mask` .
* La propiedad `loop` se comporta de la misma manera que con las animaciones de cuerpo completo: con `loop: false` la animación enmascarada se reproduce una vez y luego la parte superior del cuerpo vuelve a la locomoción normal; con `loop: true` se repite hasta que se detenga.
* Para detener desde código una animación enmascarada en bucle, llama a `stopEmote({})`, también importado desde `~system/RestrictedActions`. Se resuelve con `{ success: boolean }`.

### Detectar cuándo finaliza un Emote

Cada evento del ciclo de vida de un Emote se informa a través del componente `AvatarEmoteCommand` en la Entity del jugador. Cada nueva entrada incluye un campo `state` con un valor de la enumeración `EmoteState` :

* `EmoteState.ES_STARTED`: El Emote comenzó a reproducirse. Este también es el valor informado cuando el campo `state` está ausente.
* `EmoteState.ES_FINISHED`: Un Emote sin bucle se reprodujo hasta su final natural.
* `EmoteState.ES_INTERRUPTED`: El Emote se interrumpió: el jugador se movió o saltó, se teletransportó, comenzó otro Emote, el Emote se detuvo explícitamente o el jugador salió de la escena.

Esto funciona para los Emotes activados por la escena (`triggerEmote()` como `triggerSceneEmote()`), así como para los Emotes que el propio jugador reproduce mediante la rueda de Emotes, y también para los Emotes reproducidos por otros jugadores en la escena.

Usa la función `onChange` función en el componente `AvatarEmoteCommand` para reaccionar a cada nueva entrada:

```ts
import { AvatarEmoteCommand, EmoteState } from '@dcl/sdk/ecs'

export function main() {
	AvatarEmoteCommand.onChange(engine.PlayerEntity, (emote) => {
		if (!emote) return

		switch (emote.state ?? EmoteState.ES_STARTED) {
			case EmoteState.ES_STARTED:
				console.log('emote started: ', emote.emoteUrn)
				break
			case EmoteState.ES_FINISHED:
				console.log('emote finished naturally: ', emote.emoteUrn)
				break
			case EmoteState.ES_INTERRUPTED:
				console.log('emote was interrupted: ', emote.emoteUrn)
				break
		}
	})
}
```


> [!WARNING]
> **📔 Nota**: Esta función solo es compatible con el Client de Desktop. Los Emotes reproducidos con una `mask` (Emotes de cuerpo parcial) en el jugador local actualmente no informan eventos del ciclo de vida.



> [!NOTE]
> **💡 Consejo**: Para reproducir animaciones en Avatars que no son jugadores, consulta [NPC Avatars](/docs/creator/content-creator-es/scenes-sdk7/interactividad/avatares/npc-avatars/#animations).



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/interactividad/avatares/avatar-animations.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
