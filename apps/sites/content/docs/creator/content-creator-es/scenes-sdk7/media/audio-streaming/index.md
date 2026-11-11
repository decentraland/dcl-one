> Reproduce streams de audio en vivo en tu escena.

# Streaming de audio

Puedes transmitir audio desde una URL. Esto es útil para reproducir música directamente desde una radio por Internet, o transmitir una conferencia a tu escena.


> [!NOTE]
> **💡 Consejo**: En el [Scene Editor en Creator Hub](/docs/creator/content-creator-es/scene-editor/primeros-pasos/about-editor/), puedes usar un **Stream de audio** [Smart Item](/docs/creator/content-creator-es/scene-editor/interactividad/smart-items/) para una forma sin código de lograrlo.


El audio en la fuente debe estar en uno de los siguientes formatos: `.mp3`, `ogg`, o `aac`. La fuente también debe ser un *https* URL (*http* no se admiten), y la fuente debe contar con [políticas CORS (Cross Origin Resource Sharing)](https://en.wikipedia.org/wiki/Cross-origin_resource_sharing) que permita acceder externamente a él. Si este no es el caso, es posible que necesites configurar un server para actuar como proxy y exponer el stream de una forma válida.


> [!WARNING]
> **📔 Nota**: Para reproducir en su lugar un sonido pregrabado en tu escena, consulta [Sounds](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/sounds/).


Para añadir un audio stream a tu escena, simplemente añade un `AudioStream` component a un entity:

```ts
const streamEntity = engine.addEntity()

AudioStream.create(streamEntity, {
	url: 'https://icecast.ravepartyradio.org/ravepartyradio-192.mp3',
	playing: true,
	volume: 0.8,
})
```


> [!WARNING]
> **📔 Nota**: El sonido transmitido no es posicional; se escuchará a un volumen constante en toda tu escena. Si un jugador sale de la escena, no escuchará la transmisión en absoluto.


Establece el volumen del `AudioStream` component cambiando su `volume` propiedad.

Activa o desactiva el `AudioStream` component estableciendo su `playing` propiedad a *true* o *false*.


> [!NOTE]
> **📔 Nota**: No todos los servicios de streaming permiten reproducir su audio fuera de su sitio. Los siguientes son algunos ejemplos que funcionan en Decentraland:
> 
> ```ts
> DELTA = "https://cdn.instream.audio/:9069/stream?_=171cd6c2b6e"
> GRAFFITI = "https://n07.radiojar.com/2qm1fc5kb.m4a?1617129761=&rj-tok=AAABeIR7VqwAilDFeUM39SDjmw&rj-ttl=5"
> ISLA NEGRA = "https://radioislanegra.org/listen/up/basic.aac"
> ```
> 


## Estado del stream

Puedes monitorizar el estado de un audio stream usando cualquiera de los `audioEventsSystem` (basado en callback, recomendado) o consultándolo con `AudioStream.getAudioState()`.

### Usando audioEventsSystem (recomendado)

Registra un callback que se activa solo cuando cambia el estado del stream. Este es el mismo system utilizado para `AudioSource` entities (ver [Detectar cuándo un sonido termina](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/sounds/#detect-when-a-sound-finishes)).

```ts
import { engine, AudioStream, MediaState } from '@dcl/sdk/ecs'
import { audioEventsSystem } from '@dcl/sdk/ecs'

export function main() {
	const entity = engine.addEntity()

	AudioStream.create(entity, {
		playing: true,
		volume: 1,
		url: 'https://audio-edge-es6pf.mia.g.radiomast.io/ref-128k-mp3-stereo',
	})

	audioEventsSystem.registerAudioEventsEntity(entity, (event) => {
		console.log('Stream state: ', event.state)

		if (event.state === MediaState.MS_ERROR) {
			// Intentar reconexión
		}
	})
}
```

### Consulta con getAudioState

También puedes consultar el estado de un stream en cada frame usando `AudioStream.getAudioState()`. Esto devuelve un `PBAudioEvent` objeto (o `undefined` si aún no se ha reportado ningún estado), con un `state` campo y un `timestamp` campo. El `state` campo es un valor de `MediaState` enum:

* `MS_NONE`
* `MS_LOADING`
* `MS_READY`
* `MS_PLAYING`
* `MS_PAUSED`
* `MS_BUFFERING`
* `MS_SEEKING`
* `MS_ERROR`

```ts
export function main() {
	const entity = engine.addEntity()

	AudioStream.create(entity, {
		playing: true,
		volume: 1,
		url: 'https://audio-edge-es6pf.mia.g.radiomast.io/ref-128k-mp3-stereo',
	})

	let lastState: MediaState | undefined = undefined
	engine.addSystem(() => {
		const currentState = AudioStream.getAudioState(entity)?.state
		if (lastState !== currentState) {
			console.log('Stream state: ', currentState)
			lastState = currentState

			if (currentState == MediaState.MS_ERROR) {
				// Intentar reconexión
			}
		}
	})
}
```

## Audio espacial

Por defecto, el audio de un `AudioStream` component es global, lo que significa que se oirá con un volumen constante en toda tu escena. Si un jugador sale de la escena, no oirá la transmisión en absoluto.

Para hacer que el audio sea espacial, establece el `spatial` propiedad a *true*.

```ts
AudioStream.create(entity, {
	url: 'https://radioislanegra.org/listen/up/stream',
    playing: true,
	spatial: true,
})
```

El audio ahora se escuchará desde la posición del entity que posee el `AudioStream` component, y sonará más fuerte a medida que el jugador se acerque a ella.

Controla el audio espacial con las siguientes propiedades:

* `spatialMinDistance`: La distancia mínima a la que el audio se vuelve espacial. Si el jugador está más cerca, el audio se escuchará a volumen completo. *0* por defecto.
* `spatialMaxDistance`: La distancia máxima a la que se oye el audio. Si el jugador está más lejos, el audio se escuchará a volumen 0. *60* por defecto

```ts
const audioStreamEntity = engine.addEntity();

Transform.create(audioStreamEntity, {
    position: Vector3.create(8, 0, 8),
});

AudioStream.create(audioStreamEntity, {
    url: 'https://radioislanegra.org/listen/up/stream',
    playing: true,
    volume: 1.0,
    spatial: true,
    spatialMinDistance: 5,
    spatialMaxDistance: 10
});
```


> [!WARNING]
> **📔 Nota**: Algunos formatos de audio no admiten audio espacial. Asegúrate de que el audio del stream esté codificado en *mp3*, *AAC-LC* o *FLAC*.


## Análisis de audio

Puedes leer datos de amplitud y frecuencia en tiempo real desde un `AudioStream` entity para impulsar visuales reactivos como ecualizadores o luces que pulsan al ritmo. Consulta [Análisis de audio](/docs/creator/content-creator-es/scenes-sdk7/media/audio-analysis/).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/media/audio-streaming.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
