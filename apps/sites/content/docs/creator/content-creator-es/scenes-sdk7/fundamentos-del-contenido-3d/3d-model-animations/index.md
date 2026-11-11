> Cómo animar modelos 3D en tu escena

# Animaciones de modelos 3D

Los modelos 3D en *.glTF* y *.glb* pueden incluir tantas animations como quieras. Las animations le indican al mesh cómo moverse, especificando una serie de *keyframes* que se distribuyen a lo largo del tiempo; luego el mesh mezcla una pose con la otra para simular un movimiento continuo.

La mayoría de las animations de modelos 3D son [*skeletal animations*](https://en.wikipedia.org/wiki/Skeletal_animation). Estas animations simplifican la compleja geometría del modelo en un "muñeco de palitos", vinculando cada vertex del mesh al bone más cercano *bone* en el *skeleton*. Los modeladores ajustan el skeleton en diferentes poses, y el mesh se estira y se dobla para seguir estos movimientos.

Como alternativa, *vertex animations* animan un modelo sin necesidad de un skeleton. Estas animations especifican directamente la posición de cada vertex del modelo. Decentraland también admite estas animations.

Consulta [Animations](https://github.com/decentraland/docs/blob/main/creator/3d-modeling/animations.md) para obtener detalles sobre cómo crear animations para un modelo 3D. Lee [Shape components](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/shape-components/) para obtener instrucciones sobre cómo importar un modelo 3D a una scene.


> [!NOTE]
> **💡 Consejo**: Las animations suelen ser mejores para mover algo en su lugar, no para cambiar la posición de un entity. Por ejemplo, puedes configurar una animation para mover los pies de un personaje en su lugar, pero para cambiar la ubicación del entity es mejor usar el Transform component. Consulta [Positioning entities](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/move-entities/) para más detalles.


## Verifica si un modelo 3D tiene animations

No todos los *archivo glTF* archivos incluyen animations. Para ver si hay alguna disponible, puedes hacer lo siguiente:

* Si usas [VS Code](https://code.visualstudio.com/)(recomendado), instala la *GLTF Tools* extensión y ve el contenido de un archivo glTF allí.
* Abre la [Babylon Sandbox](https://sandbox.babylonjs.com/) sitio y arrastra el archivo glTF (y cualquier *.jpg* o *.bin* dependencia) al navegador.
* Abre la *.glTF* archivo con un editor de texto y desplázate hacia abajo hasta encontrar *"animations":*.


> [!NOTE]
> **💡 Consejo**: En *skeletal* animations, un nombre de animation suele componerse del nombre de su armature, un guion bajo y su nombre de animation. Por ejemplo `myArmature_animation1`.


## Reproducción automática

Si un modelo 3D incluye animations, el comportamiento predeterminado es que la primera de ellas se reproduce siempre en loop.

Para evitar este comportamiento, añade un `Animator` component al entity que tiene el modelo, y luego gestiona la reproducción de animations explícitamente. Si hay un `Animator` component presente en el entity, todas las animations quedan por defecto en un estado `playing: false` y deben reproducirse manualmente.


> [!NOTE]
> **💡 Consejo**: En el [Scene Editor](/docs/creator/content-creator-es/scene-editor/primeros-pasos/about-editor/), puedes añadir un **Animator** component visualmente. Consulta [Add Components](/docs/creator/content-creator-es/scene-editor/construir/components/#add-components). También puedes controlar animations sin código mediante **Actions**, consulta [Haz inteligente cualquier ítem](/docs/creator/content-creator-es/scene-editor/interactividad/make-any-item-smart/).


## Gestionar animations explícitamente

Un `Animator` component se usa para acceder a todas las animations del entity y se puede usar para indicarle explícitamente al entity que reproduzca o detenga una animation. El `Animator` component incluye un array de `states`, esta lista debe incluir un objeto por cada una de las animations que el modelo 3D puede realizar. Un solo `Animator` puede incluir tantos states como sean necesarios.

```ts
// Crear entity
const shark = engine.addEntity()

// Añadir un modelo 3D a él
GltfContainer.create(shark, {
	src: 'models/shark.glb',
})

Animator.create(shark, {
	states: [
		{
			clip: 'swim',
			playing: true,
			loop: true,
		},
	],
})
```

Cada `state` objeto lleva un registro de si una animation se está reproduciendo actualmente.


> [!WARNING]
> **📔 Nota**: El `Animator` component debe importarse mediante
> 
> > `import { Animator } from "@dcl/sdk/ecs"`
> 
> Consulta [Importaciones](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/coding-scenes/#imports) para saber cómo manejarlos fácilmente.


## Obtener una animation

Obtener un clip del `Animator` por nombre usando la `.Animator.getClip()` función. Esta función devuelve una versión mutable del objeto de estado de la animation.

```ts
const swimAnim = Animator.getClip(sharkEntity, 'swim')
```

`Animator.getClip` requiere los siguientes parámetros:

* `entity`: La entity del `Animator` component que quieres consultar.
* `clipName`: Cadena con el nombre del clip que quieres obtener.

`Animator.getClip` obtiene una versión mutable del estado de la animation, por lo que puedes modificar libremente los valores de lo que devuelve esta función.

```ts
const swimAnim = Animator.getClip(sharkEntity, 'swim')
swimAnim.loop = false
```


> [!WARNING]
> **📔 Nota**: Si intentas usar `Animator.getClip()` para obtener un clip que no está listado en el `Animator` component, arroja un error. Usa `Animator.getClipOrNull()` si prefieres obtener una respuesta `null` en ese caso, en lugar de un error.


## Reproducir una animation

El `.playing` el campo de un state de animation determina si la animation se está reproduciendo actualmente. Ten en cuenta que varias animations pueden reproducirse en un solo modelo 3D al mismo tiempo.

Usa la `Animator.playSingleAnimation()` función en un `AnimationState` objeto.

```ts
Animator.playSingleAnimation(sharkEntity, 'swim')
```

Si el entity estaba reproduciendo otras animations, `Animator.playSingleAnimation` las detiene.

`Animator.playSingleAnimation` requiere los siguientes parámetros:

* `entity`: La entity del `Animator` component que quieras afectar.
* `clipName`: Cadena con el nombre del clip que quieres reproducir.
* `resetCursor`: *(opcional)* Si *true*, reproduce la animation desde el comienzo, incluso si la animation estaba pausada anteriormente. Si *false*, seguirá reproduciendo la animation desde donde se pausó. Predeterminado: *true*.

```ts
Animator.playSingleAnimation(sharkEntity, 'swim', false)
```

La siguiente tabla resume cómo `Animator.playSingleAnimation()` se comporta, usando diferentes valores para la `resetCursor` propiedad:

|                                 | `resetCursor` = *false*                       | `resetCursor` = *true* (predeterminado) |
| ------------------------------- | --------------------------------------------- | --------------------------------------- |
| **Actualmente reproduciéndose** | No tiene efecto.                              | Reproduce desde el comienzo.            |
| **Pausado**                     | Se reanuda desde el último frame reproducido. | Reproduce desde el comienzo.            |
| **Finalizado (sin loop)**       | Reproduce desde el comienzo.                  | Reproduce desde el comienzo.            |

## Animations en loop

De forma predeterminada, las animations se reproducen en un loop que repite la animation para siempre.

Cambia este ajuste configurando la `loop` propiedad en el `state` objeto.

```ts
Animator.create(shark, {
	states: [
		{
			clip: 'bite',
			playing: true,
			loop: false,
		},
	],
})
```

Si `loop` se establece en *false*, la animation se reproduce solo una vez y luego se detiene, quedándose en la postura del último frame.

## Detener una animation

Para detener todas las animations que un entity está reproduciendo, usa `Animator.stopAllAnimations()`.

```ts
Animator.stopAllAnimations(shark)
```

`Animator.stopAllAnimations` requiere los siguientes parámetros:

* `entity`: La entity del `Animator` component que quieras afectar.
* `resetCursor`: *(opcional)* Si *true*, vuelve a la postura del primer frame de la animation. Si *false*, permanece en pausa en su postura actual. Predeterminado: *true*.


> [!WARNING]
> **📔 Nota**: Al reproducir una animation con `Animator.playSingleAnimation`, esta función se encarga de detener todas las demás animations en segundo plano. No necesitas detener explícitamente las otras animations en ese caso.


Cuando una animation sin loop termina de reproducirse, de forma predeterminada el modelo 3D permanece en la última postura que tenía. La `shouldReset` propiedad controla lo que sucede cuando se vuelve a reproducir una animation detenida: si *true*, la animation se restaura a su estado inicial (su primer frame, o su último frame si se reproduce con una `speed`) negativa) cada vez que pasa de detenida a reproduciéndose. Si *false* (el valor predeterminado), se reanuda desde donde estaba.

```ts
Animator.create(shark, {
	states: [
		{
			clip: 'bite',
			playing: true,
			shouldReset: true,
			loop: true,
		},
	],
})
```

También puedes usar `Animator.stopAllAnimations()` en cualquier momento para devolver explícitamente la postura al primer frame de la animation.


> [!WARNING]
> **📔 Nota**: Restablecer la postura es un cambio brusco. Si quieres hacer que el modelo transicione suavemente a otra postura, reproduce la otra animation y mezcla entre ambas cambiando gradualmente sus `weight` propiedades. Consulta [Peso de animation](#animation-weight).


## Detectar cuándo termina una animation

Cuando una animation sin loop termina de reproducirse, el engine establece la `playing` propiedad de ese state de animation de vuelta a *false*. El código de tu scene puede leer este valor para saber cuándo terminó la animation, por ejemplo para encadenar otra animation justo después.

```ts
let wasPlaying = false

engine.addSystem(() => {
	const animator = Animator.get(shark)
	const biteState = animator.states.find((state) => state.clip === 'bite')
	const isPlaying = biteState?.playing ?? false

	if (wasPlaying && !isPlaying) {
		console.log('bite animation finished')
		// encadenar la siguiente animation
		Animator.playSingleAnimation(shark, 'swim')
	}

	wasPlaying = isPlaying
})
```


> [!WARNING]
> **📔 Nota**: Al consultar el estado de la animation en cada frame, léelo siempre mediante `Animator.get()` (solo lectura). No uses `Animator.getClip()` o `Animator.getMutable()` para consultar: estos devuelven una versión mutable del component, lo que lo marca como cambiado en cada frame y provoca trabajo de sincronización innecesario.
> 
> El `playing` La propiedad solo se activa por el engine cuando la animation termina por sí sola. Las animations en loop se reproducen hasta que se detienen, así que nunca activan la propiedad por sí solas, `speed` establecido en 0 nunca terminan.



> [!WARNING]
> **📔 Nota**: Esta función solo es compatible en el cliente Desktop.


## Gestionar varias animations

Si un modelo 3D tiene varias animations empaquetadas dentro, un solo `Animator` component puede encargarse de todas ellas.

```ts
// Crear entity
const shark = engine.addEntity()

// Añadir un modelo 3D a él
GltfContainer.create(shark, {
	src: 'models/shark.glb'
})

Animator.create(shark, {
	states:[{
			clip: "swim",
			playing: true,
			loop: true
		}, {
			clip: "bite",
			playing: true,
			loop: true
		}
	]
})
```

En el ejemplo anterior, dos animations se gestionan mediante `state` objects separados, y luego se asignan ambos al mismo `Animator` component.

Cada bone de una animation solo puede verse afectado por una animation a la vez, a menos que estas animations tengan un `weight` que sumado dé un valor de 1 o menos.

Si una animation solo afecta a las piernas de un personaje, y otra solo afecta a la cabeza de un personaje, entonces pueden reproducirse al mismo tiempo sin problema. Pero si ambas afectan a las piernas del personaje, entonces debes reproducir solo una a la vez, o reproducirlas con `weight` más bajos.

Si en el ejemplo anterior, la `bite` animation solo afecta a la boca del shark, y la `swim` animation solo afecta a los bones de la columna vertebral del shark, entonces ambas pueden reproducirse al mismo tiempo.


> [!WARNING]
> **📔 Nota**: `Animator.playSingleAnimation()` detiene todas las demás animations que el entity está reproduciendo actualmente. Para reproducir varias animations al mismo tiempo, modifica la `playing` propiedad en los states de animation manualmente.


## Velocidad de animation

Cambia la velocidad a la que se reproduce una animation cambiando la `speed` propiedad. El valor de la speed es 1 de forma predeterminada.

```ts
Animator.create(shark, {
	states: [
		{
			clip: 'swim',
			playing: true,
			loop: true,
			speed: 2,
		},
	],
})
```

Establece la speed por debajo de 1 para reproducirla más despacio, por ejemplo a 0.5 para reproducirla a la mitad de la speed. Establécela por encima de 1 para reproducirla más rápido, por ejemplo a 2 para reproducirla al doble de la speed.

```ts
const swimAnim = Animator.getClip(sharkEntity, 'swim')

swimAnim.speed = 0.5
```

## Peso de animation

El `weight` La propiedad weight permite que un solo modelo realice varias animations a la vez, calculando un promedio ponderado de todos los movimientos implicados en la animation. El valor de `weight` determina cuánta importancia se le dará a esa animation en el promedio.

De forma predeterminada, `weight` es igual a *1*. El valor de `weight` no puede ser mayor que *1*.

```ts
Animator.create(shark, {
	states: [
		{
			clip: 'swim',
			playing: true,
			loop: true,
			weight: 0.2,
		},
	],
})
```

El `weight` el valor de todas las animations activas en un entity siempre debería sumar 1. Si suma menos de 1, el promedio ponderado usará la posición predeterminada del armature para la parte restante del cálculo.

Por ejemplo, en el ejemplo de código anterior, estamos reproduciendo la *swim* animation, que solo tiene un `weight` de *0.2*. Este movimiento de natación será bastante sutil: solo el 20% de la intensidad que define la animation. El 80% restante del cálculo toma valores de la postura predeterminada del armature.

El `weight` La propiedad puede usarse de formas interesantes, por ejemplo el `weight` propiedad de *swim* podría establecerse en proporción a la rapidez con la que el shark está nadando, así no necesitas crear varias animations para natación lenta y rápida.

También podrías cambiar el `weight` valor gradualmente al iniciar y detener una animation para darle una transición más natural y evitar saltos desde la pose predeterminada a la primera pose de la animation.


> [!WARNING]
> **📔 Nota**: El `weight` valor añadido de todas las animations que actúan sobre un bone de un modelo 3D no puede ser mayor que 1. Si más de una animation está afectando a los mismos bones al mismo tiempo, deben tener el weight configurado en valores que sumen menos de 1.


```ts
const swimAnim = Animator.getClip(sharkEntity, 'swim')

swimAnim.weight = 0.5
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/3d-model-animations.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
