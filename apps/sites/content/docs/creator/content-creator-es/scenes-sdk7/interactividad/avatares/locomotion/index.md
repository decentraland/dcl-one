> Cambia la velocidad y la altura del salto del jugador, o restringe cómo puede moverse

# Configuración y restricciones de locomoción

Puedes cambiar cómo se mueve el avatar del jugador mientras está dentro de tu escena. Usa el `AvatarLocomotionSettings` component para ajustar las velocidades y las alturas de salto, y el `InputModifier` component para congelar al jugador o bloquear tipos específicos de movimiento. Ambos components van en `engine.PlayerEntity`, y ambos solo surten efecto mientras el jugador esté dentro de los límites de tu escena.

## Ajustes de locomoción

Puedes afectar la locomoción del jugador, como su velocidad al correr, altura de salto y más. Esto se puede alterar dinámicamente, por ejemplo para permitir que un jugador obtenga un aumento temporal de velocidad al interactuar con un item, o para deshabilitar la capacidad del jugador de saltar por un corto período de tiempo.


> [!NOTE]
> **💡 Consejo**: Puedes establecer estos valores sin escribir código. Arrastra el **Ajustes de locomoción** smart item en tu escena en el [Scene Editor en Creator Hub](/docs/creator/content-creator-es/scene-editor/primeros-pasos/about-editor/) y mueve sus deslizadores. Consulta [Configuración de locomoción](/docs/creator/content-creator-es/scene-editor/interactividad/smart-items/#locomotion-settings).


Para hacerlo, añade un `AvatarLocomotionSettings` component al `engine.PlayerEntity`.

```ts
import {AvatarLocomotionSettings, engine} from '@dcl/sdk/ecs'

AvatarLocomotionSettings.create(engine.PlayerEntity, {
	runSpeed: 10,
	jumpHeight: 2,
})
```

Las siguientes propiedades están disponibles:

* `walkSpeed`: La velocidad a la que el jugador camina, en metros por segundo. En el cliente de escritorio, los jugadores caminan al presionar la tecla control.
* `jogSpeed`: La velocidad a la que el jugador trota, en metros por segundo. Esta es la forma predeterminada en que se mueve el jugador.
* `runSpeed`: La velocidad a la que el jugador corre, en metros por segundo. En el cliente de escritorio, los jugadores corren al presionar la tecla shift.
* `jumpHeight`: La altura a la que salta el jugador, en metros.
* `runJumpHeight`: La altura a la que salta el jugador después de correr, en metros.
* `doubleJumpHeight`: La altura del segundo salto al hacer doble salto, en metros.
* `glidingSpeed`: La velocidad horizontal a la que se mueve el jugador mientras planea, en metros por segundo.
* `glidingFallingSpeed`: La velocidad máxima de caída del jugador mientras planea, en metros por segundo. Esto solo limita el descenso del jugador: el movimiento ascendente, como un impulso de la fuerza continua de la escena, no está limitado.
* `hardLandingCooldown`: El tiempo de reutilización después de un aterrizaje brusco, en segundos. Este es el tiempo que el jugador debe esperar antes de poder moverse de nuevo después de caer desde una gran altura.

Como referencia, estos son los valores predeterminados de esas propiedades:

* `walkSpeed`: 1.5 m/s
* `jogSpeed`: 8 m/s
* `runSpeed`: 10 m/s
* `glidingSpeed`: 6 m/s
* `glidingFallingSpeed`: 1 m/s
* `jumpHeight`: 1 m
* `runJumpHeight`: 1.5 m
* `doubleJumpHeight`: 2 m
* `hardLandingCooldown`: 0.75 s


> [!NOTE]
> **💡 Consejo**: Mientras planea, las fuerzas continuas aplicadas por la escena son 1.5 veces más fuertes, y las fuerzas ascendentes pueden elevar al jugador. Consulta [Fuerzas mientras se planea](/docs/creator/content-creator-es/scenes-sdk7/interactividad/player-physics/#forces-while-gliding).



> [!NOTE]
> **💡 Consejo**: Ninguna de estas propiedades puede ser menor que 0. Si estableces una de ellas en un valor negativo, se limitará a 0. Establecer estos valores en cero tendrá el mismo efecto que usar el `InputModifier` para bloquear el uso de ciertas teclas.
> 
> Solo puedes afectar la locomoción del jugador si está dentro de los límites de la escena. Para afectar los avatares de otros jugadores, debes ejecutar el código que afecta su locomoción en su propia instancia.


Puedes crear un [Wearable inteligente](/docs/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/smart-wearables/) que haga que el jugador siempre corra más rápido o salte más alto. Si tanto la escena como un smart wearable definen valores distintos para estos parámetros, siempre se usan los valores de la escena.

Para asegurarte de que nadie tenga ventajas injustas en una escena de parkour, puedes imponer los parámetros predeterminados añadiendo explícitamente sus valores predeterminados en tu escena:

```ts
import {AvatarLocomotionSettings, engine} from '@dcl/sdk/ecs'

AvatarLocomotionSettings.create(engine.PlayerEntity, {
	runSpeed: 10,
	walkSpeed: 1.5,
	jogSpeed: 8 ,
	jumpHeight: 1,
	runJumpHeight: 1.5,
	hardLandingCooldown: 0.75
})
```

## Restringir locomoción

Puedes restringir qué acciones puede realizar el jugador en tu escena. Usa esto para congelar al jugador, o para restringir formas específicas de locomoción, por ejemplo para impedir que el jugador salte o corra.

### Congelar al jugador

Puedes congelar al jugador para que ninguna de las teclas de entrada pueda mover el avatar. Esto puede ser útil para muchas mecánicas de juego. También es una buena práctica congelar a un jugador mientras se realiza una animación importante que no debería interrumpirse con el movimiento, o mientras un [Virtual Camera](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/camera/) apunta lejos del avatar y no quieres que el jugador se mueva a ciegas.

Usa la `InputModifier` component en el `engine.PlayerEntity` para evitar que las entradas del jugador afecten la locomoción del avatar. El avatar permanecerá quieto, y el jugador solo podrá rotar la cámara.

```ts
import {InputModifier, engine} from '@dcl/sdk/ecs'

InputModifier.create(engine.PlayerEntity, {
	mode: InputModifier.Mode.Standard({
		disableAll: true,
	}),
})
```

Ten en cuenta las siguientes consideraciones:

* Mientras las interacciones del jugador están deshabilitadas, su avatar sigue afectado por fuerzas externas, como la gravedad o las plataformas móviles.
* El `InputModifier` component solo puede usarse con la `engine.PlayerEntity` entity. Solo puede afectar al jugador actual, no puede afectar a otros jugadores.
* Este component solo afecta al jugador mientras el avatar esté dentro de los límites de tu escena. Su locomoción deja de estar restringida en cuanto sale de la escena.
* Mientras las interacciones del jugador están deshabilitadas, el jugador no puede realizar emotes libremente, pero la escena puede activar animaciones en el avatar.
* Las entradas del jugador no afectan al avatar, pero el [eventos de input globales](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/system-based-events/#global-input-events) puede seguir siendo escuchado por la escena. Podrías usar esto para controlar un vehículo, o usar un [Virtual Camera](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/camera/) para seguir a otra entity a medida que se mueve, tratándola como un avatar alternativo.

### Restringir tipos específicos de locomoción

En lugar de congelar por completo al jugador, puedes restringir ciertas formas específicas de locomoción del jugador. Esto podría usarse por razones de jugabilidad, por ejemplo para preservar la dificultad de un juego de plataformas impidiendo el doble salto y el planeo. Incluso estas habilidades podrían alternarse dinámicamente como una mecánica de juego, por ejemplo dando al jugador una barra de resistencia e impidiendo que corra cuando se agota. También podría usarse para marcar el tono de una escena, por ejemplo impedir correr o saltar en un lugar pensado para ser sereno. El `InputModifier` incluye las siguientes opciones:

* `disableWalk`: El jugador no puede caminar lentamente (presionando control). Si el jugador intenta caminar, en su lugar trotar á o correrá, si está permitido.
* `disableRun`: El jugador no puede correr (presionando shift). Si el jugador intenta correr, en su lugar trotar á, si está permitido.
* `disableJog`: El jugador no puede trotar (esta es la velocidad de movimiento predeterminada). Si el jugador intenta trotar, en su lugar correrá o caminará, si está permitido.
* `disableJump`: El jugador no puede saltar.
* `disableEmote`: El jugador no puede realizar emotes voluntariamente. La escena puede activar animaciones en el avatar del jugador.
* `disableDoubleJump`: El jugador no puede hacer doble salto.
* `disableGliding`: El jugador no puede planear.

```ts
import {InputModifier, engine} from '@dcl/sdk/ecs'

InputModifier.create(engine.PlayerEntity, {
	mode: InputModifier.Mode.Standard({
		disableAll: false,
		disableWalk: false,
		disableRun: true,
		disableJog: true,
		disableJump: true,
		disableEmote: true,
		disableDoubleJump: true,
		disableGliding: true
	}),
})
```

### Sintaxis avanzada

Para usar el component sin helpers, puedes usar la siguiente sintaxis:

```ts
import {InputModifier, engine} from '@dcl/sdk/ecs'

InputModifier.createOrReplace(engine.PlayerEntity, {
	mode: {
		$case: 'standard',
		standard: {
			disableAll: false,
			disableWalk: false,
			disableRun: true,
			disableJog: true,
			disableJump: true,
			disableEmote: true,
		},
	},
})
```


> [!NOTE]
> **💡 Consejo**: Para ver un ejemplo funcional, consulta la [`0,1-input-modifier`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,1-input-modifier) escena de prueba, que activa y desactiva cada `InputModifier` bandera de movimiento por separado y muestra un avatar que sigue siendo transportado por una plataforma móvil mientras el movimiento está bloqueado.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/interactividad/avatares/locomotion.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
