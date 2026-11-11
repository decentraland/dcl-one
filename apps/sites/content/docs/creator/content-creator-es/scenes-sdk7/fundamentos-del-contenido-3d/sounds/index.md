> Aprende cómo añadir sonidos a tu escena.

# Sonidos

El sonido es una excelente manera de proporcionar retroalimentación a las acciones y eventos del jugador; los sonidos de fondo también pueden darle más contexto a tu escena y mejorar la inmersión del jugador en ella.


> [!WARNING]
> **📔 Nota**: Ten en cuenta que los sonidos solo son escuchados por los jugadores que estén dentro de los parcels que componen la escena donde se generó el sonido, incluso si por lo demás estuvieran dentro del rango de audición. Los jugadores también pueden elegir desactivar todos los sonidos en sus configuraciones.


Los formatos de sonido compatibles varían según el navegador, pero se recomienda usar *.mp3*.

*.wav* también son compatibles, pero por lo general no se recomiendan porque son significativamente más pesados.

## Reproducir sonidos

La forma más fácil de reproducir un sonido es añadir un **AudioSource** componente visualmente en el [Scene Editor en Creator Hub](/docs/creator/content-creator-es/scene-editor/primeros-pasos/about-editor/) y configúralo en **Start Playing** y **Loop**. Consulta [Add Components](/docs/creator/content-creator-es/scene-editor/construir/components/#add-components).

![](/docs/_remote/341e53c596dc617859805f3b.webp)

También puedes activar la reproducción de un sonido de forma sin código mediante **Actions**, consulta [Make any item smart](/docs/creator/content-creator-es/scene-editor/interactividad/make-any-item-smart/).

Para reproducir un sonido mediante código, usa la `AudioSource.playSound` función.

```ts
// Crear entidad
const sourceEntity = engine.addEntity()
Transform.create(sourceEntity, { position: Vector3.create(8, 1, 8) })

// Reproducir sonido
AudioSource.playSound(sourceEntity, 'assets/sounds/sound-effect.mp3')
```


> [!WARNING]
> **📔 Nota**: Toda Entity con un `AudioSource` también necesita un `Transform`, incluidos los sonidos globales. Sin uno, no se reproduce nada y no se registra ningún error, lo que hace que este sea un problema difícil de detectar. Los items colocados en el Scene Editor siempre tienen un `Transform`Transform


El archivo de sonido debe estar dentro de la carpeta del proyecto. En el ejemplo anterior, el archivo de audio se encuentra en una `assets/sounds` carpeta, que está ubicada en el nivel raíz de la carpeta del proyecto de la scene.


> [!WARNING]
> **📔 Nota**: El `AudioSource` componente debe importarse mediante
> 
> > `import { AudioSource } from "@dcl/sdk/ecs"`
> 
> Consulta [Imports](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/coding-scenes/#imports) para ver cómo manejarlos fácilmente.


La `AudioSource.playSound()` función toma los siguientes argumentos:

* `entity`: En qué entity aplicar el sonido. El sonido se escuchará desde la posición de esta entity, lo que significa que sonará más fuerte a medida que el jugador se acerque.
* `src`: La ubicación del archivo de sonido dentro de tu proyecto.


> [!NOTE]
> **💡 Consejo**: Para mayor claridad, recomendamos mantener tus archivos de sonido separados en una `assets/sounds` carpeta dentro de tu scene.


* `resetCursor`: *(opcional)* Si es true, el sonido siempre comienza desde el inicio. De lo contrario, continúa desde la posición actual del cursor. Útil para pausar y reanudar.

`playSound()` crea el `AudioSource` componente si la entity aún no tiene uno. Si ya tiene uno, `playSound()` conserva sus otros valores, como `volume`, `pitch`, `loop` y `global`, así que puedes configurarlos una vez y luego llamar `playSound()` cada vez que quieras reproducir el sonido. Llamarlo de nuevo con el mismo archivo reproduce el sonido otra vez. Siempre devuelve `true`.

Otra forma de reproducir sonidos es crear manualmente un `AudioSource` componente en una entity. Usa este enfoque para tener más control sobre el sonido, por ejemplo para hacerlo repetir o establecer el volumen.

```ts
// Crear entidad
const sourceEntity = engine.addEntity()
Transform.create(sourceEntity, { position: Vector3.create(8, 1, 8) })

// Crear componente AudioSource
AudioSource.create(sourceEntity, {
	audioClipUrl: 'sounds/sound-effect.mp3',
	loop: true,
	playing: true,
})
```

Se pueden establecer las siguientes propiedades:

* `audioClipUrl`: La ubicación del archivo de sonido dentro de tu proyecto.
* `playing`: Si es true, el sonido comienza a reproducirse. Puedes crear un sonido con `playing` establecido en false y luego ponerlo en true más adelante.
* `volume`: *(opcional)* El volumen del archivo de sonido. 1 por defecto, que es el volumen máximo.
* `pitch`: *(opcional)* Modifica el pitch de un sonido. 1 es el valor predeterminado; bájalo para un sonido más grave y súbelo para un sonido más agudo.


> [!NOTE]
> **💡 Consejo**: Para evitar que un efecto de sonido se vuelva demasiado repetitivo durante un juego, es útil aleatorizar pequeñas variaciones en el pitch del sonido cada vez que se reproduzca.


* `currentTime`: *(opcional)* 0 por defecto. Establece este valor para evitar comenzar desde el inicio del archivo de sonido.

Cada Entity solo puede tener un único `AudioSource` componente, que solo puede reproducir un único clip a la vez. Esta limitación puede superarse fácilmente modificando la fuente de audio en el momento de reproducir un nuevo sonido, o incluyendo múltiples child entities invisibles, cada una con su propio sonido.


> [!WARNING]
> **📔 Nota**: Los sonidos se reproducen en la instancia local de cada jugador. Los demás jugadores cercanos no escucharán los mismos sonidos a menos que su scene local también los reproduzca explícitamente.


### Precarga de un sonido

Si una Entity usa un sonido, pero no se reproduce de inmediato en tiempo de ejecución de la scene, puede tardar un poco en descargarse. Puede estar disponible en tiempo de ejecución de la scene usando el `AssetLoad` componente.

```ts
import { AssetLoad } from "@dcl/sdk/ecs"

AssetLoad.create(engine.RootEntity, {
  assets: [
    "assets/scene/bundle1/explosionSound.mp3",
  ],
})
```

Para más información, consulta la [Recursos de precarga](/docs/creator/content-creator-es/scenes-sdk7/optimizacion/pre-load-resources/) documentación.

## Detener sonidos

Para impedir que una Entity reproduzca su sonido, usa la `AudioSource.stopSound()` función. Solo necesitas especificar la Entity, ya que cada Entity tiene un único `AudioSource` componente, y cada `AudioSource` componente reproduce un solo archivo a la vez.

```ts
AudioSource.stopSound(sourceEntity)
```

`stopSound()` devuelve `false` si la Entity no tiene ningún `AudioSource` componente.

Otra forma de detener un sonido es establecer la `playing` propiedad en false.

```ts
// Crear entidad
const sourceEntity = engine.addEntity()

// Crear componente AudioSource
AudioSource.create(sourceEntity, {
	audioClipUrl: 'sounds/explosion.mp3',
	playing: true,
})

// Definir una función simple
function stopSound(entity: Entity) {
	// obtener la versión mutable del componente de fuente de audio
	const audioSource = AudioSource.getMutable(entity)

	// modificar su valor playing
	audioSource.playing = false
}

// llamar a la función
stopSound(sourceEntity)
```

## Detectar cuándo termina un sonido

Cuando un sonido que no se repite termina de reproducirse por sí solo, el engine establece el `playing` property del `AudioSource` componente de nuevo en *false*. El código de tu scene puede leer este valor para saber cuándo terminó el sonido, por ejemplo para encadenar otro sonido o acción justo después.

```ts
let wasPlaying = false

engine.addSystem(() => {
	const audio = AudioSource.get(sourceEntity)
	const isPlaying = audio.playing ?? false

	if (wasPlaying && !isPlaying) {
		console.log('sound finished playing')
		// reacciona aquí: reproduce otro sonido, avanza una secuencia, etc.
	}

	wasPlaying = isPlaying
})
```


> [!WARNING]
> **📔 Nota**: Al consultar el componente en cada frame, léelo siempre con `AudioSource.get()` (solo lectura). Usar `AudioSource.getMutable()` marcaría el componente como modificado en cada frame, lo que provocaría trabajo de sincronización innecesario.
> 
> La `playing` La propiedad solo la cambia el engine cuando el sonido termina por sí solo. Si la scene detiene el sonido explícitamente (mediante `AudioSource.stopSound()` o al establecer `playing` en *false*), ese cambio proviene de tu propio código, y los sonidos en loop se reproducen hasta que se detienen, así que nunca cambian la propiedad por sí solos.


Como alternativa, usa `audioEventsSystem.registerAudioEventsEntity` para definir una función que se ejecute cada vez que cambie el estado de reproducción del audio de la Entity, igual que [video events](/docs/creator/content-creator-es/scenes-sdk7/media/video-playing/#video-events) para los videos. La función recibe un evento con un `state` campo, usando el `MediaState` enum.

```ts
import { engine, AudioSource, audioEventsSystem, MediaState } from '@dcl/sdk/ecs'

// ... Crear sourceEntity con un componente AudioSource ...

audioEventsSystem.registerAudioEventsEntity(sourceEntity, (audioEvent) => {
	switch (audioEvent.state) {
		case MediaState.MS_LOADING:
			console.log('audio event - sound is LOADING')
			break
		case MediaState.MS_READY:
			console.log('audio event - sound is READY (loaded, or finished playing)')
			break
		case MediaState.MS_PLAYING:
			console.log('audio event - sound started PLAYING')
			break
		case MediaState.MS_ERROR:
			console.log('audio event - sound ERROR (e.g. the file failed to load)')
			break
	}
})
```

Una transición de `MS_PLAYING` en `MS_READY` significa que el sonido dejó de reproducirse. El `MediaState` enum completo también incluye `MS_NONE`, `MS_LOADING`, `MS_PAUSED`, `MS_BUFFERING`, `MS_SEEKING` y `MS_ERROR`, y cada evento lleva un `timestamp` de cuándo ocurrió el cambio de estado. También puedes consultar el estado reportado más reciente en cualquier momento con `audioEventsSystem.getAudioState(entity)`, desregistra el callback con `audioEventsSystem.removeAudioEventsEntity(entity)`, o comprueba si una Entity está registrada con `audioEventsSystem.hasAudioEventsEntity(entity)`. El mismo sistema también funciona para entities con un [`AudioStream`](/docs/creator/content-creator-es/scenes-sdk7/media/audio-streaming/) componente.


> [!WARNING]
> **📔 Nota**: Estas funciones solo son compatibles en el cliente Desktop.


## Repetición

Para mantener un sonido reproduciéndose en un loop continuo, establece el `loop` campo del `AudioSource` componente en *true* antes de empezar a reproducirlo.

```ts
// Crear entidad
const sourceEntity = engine.addEntity()

// Crear componente AudioSource
AudioSource.create(sourceEntity, {
	audioClipUrl: 'sounds/sound-effect.mp3',
	playing: true,
	loop: true,
})
```

Hacer looping de sonidos es especialmente útil para añadir música de fondo u otros sonidos de fondo.

## Establecer volumen

Puedes establecer el `volume` property del `AudioSource` componente para cambiar el volumen de un sonido.

El volumen se expresa como un número de *0* en *1*.

```ts
// Crear entidad
const sourceEntity = engine.addEntity()

// Crear componente AudioSource
AudioSource.create(sourceEntity, {
	audioClipUrl: 'sounds/sound-effect.mp3',
	playing: true,
	volume: 0.5,
})
```


> [!WARNING]
> **📔 Nota**: Por supuesto, el volumen de un sonido también se ve afectado por la distancia del jugador a la fuente de audio. A medida que el jugador se aleja, el volumen será menor.


## Sonidos globales

De forma predeterminada, todos los sonidos de un `AudioSource` son posicionales. Esto significa que parecen generarse desde la posición del `Transform` componente, y sonarán más fuerte a medida que el jugador se acerque. Pero también puedes configurar un sonido como global, de modo que el volumen sea constante sin importar dónde esté parado el jugador. Esto es ideal para usarlo en música de fondo, sonidos de notificación y otros sonidos no posicionales.


> [!WARNING]
> **📔 Nota**: Los sonidos globales son una función que solo es compatible en el cliente de escritorio DCL 2.0.


Para hacer que un sonido sea global, establece el `global` propiedad en *true*.

```ts
AudioSource.create(sourceEntity, {
	audioClipUrl: 'sounds/music.mp3',
	playing: true,
	global: true,
})
```

Un sonido global todavía necesita un `Transform` en su entity, aunque su posición ya no afecte lo que oye el jugador. Cualquier posición servirá.

## Reproducir un segmento de un sonido

Para reproducir un segmento de un archivo de sonido más largo, usa la `playSoundSegment()` en la biblioteca SDK Utils. Consulta [SDK7 Utils](https://github.com/decentraland/sdk7-utils).

También puedes lograr esto estableciendo explícitamente el `currentTime` propiedad en un `AudioSource` componente, y luego deteniéndolo después de esperar un período de tiempo.

## Streaming de audio

Consulta [Streaming de audio](/docs/creator/content-creator-es/scenes-sdk7/media/audio-streaming/) para aprender cómo puedes reproducir un streaming de audio en vivo desde una fuente externa.

## Análisis de audio

Consulta [Análisis de audio](/docs/creator/content-creator-es/scenes-sdk7/media/audio-analysis/) para aprender cómo leer datos de amplitud y frecuencia en tiempo real desde un `AudioSource` y crear visuales reactivos a partir de ello, como cubos que pulsan con el bajo o visualizadores de barras al estilo ecualizador.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/sounds.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
