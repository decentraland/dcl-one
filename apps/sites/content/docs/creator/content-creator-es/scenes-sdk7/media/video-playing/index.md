> Transmite video a una escena

# Reproducción de video

Hay tres formas diferentes de mostrar un video en una escena:

* Sube un archivo de video como parte del contenido de la escena
* Haz streaming del video desde una fuente externa
* Haz streaming en vivo a través de Decentraland cast


> [!NOTE]
> **💡 Consejo**: En el [Scene Editor en Creator Hub](/docs/creator/content-creator-es/scene-editor/primeros-pasos/about-editor/), puedes usar un **Video Player** [Smart Item](/docs/creator/content-creator-es/scene-editor/interactividad/smart-items/) para lograrlo sin código.


En todos los casos, necesitarás:

* Una entity con un [forma primitiva](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/shape-components/) como un plane, cube o incluso un cone.
* Un [material](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/materials/) con un `VideoTexture` asignado a su texture
* Un `VideoPlayer` component para controlar el state del video.

## Consideraciones de rendimiento

Ten en cuenta que el streaming de video exige un esfuerzo considerable por parte de la máquina del jugador. Se recomienda evitar reproducir más de un video a la vez.

Si se están reproduciendo demasiados videos al mismo tiempo en tu escena, algunos serán pausados por el engine. La prioridad para pausar una screen se determina en función de varios factores, incluida la proximidad al player, el tamaño y si la screen está en el campo de visión del player. El número máximo de videos simultáneos depende de la configuración de calidad del player.

* Baja: 1
* Media: 5
* Alta: 10

También recomendamos empezar a reproducir el video cuando el jugador esté cerca o realice una acción para hacerlo. Empezar a reproducir un video cuando tu escena se carga lejos en el horizonte afectará innecesariamente el rendimiento mientras los jugadores visitan escenas vecinas.

También evita hacer streaming de videos con una resolución muy alta. No uses nada por encima de *HD*.

También es ideal reproducir videos en materiales Basic (unlit), para reducir la carga de rendimiento, como ocurre en todos los ejemplos de abajo.

## Mostrar un video

Las siguientes instrucciones se aplican a las tres opciones de mostrar video:

1. Crea una entity para que sirva como la pantalla del video. Dale a esta entity un `MeshRenderer` component para que tenga una forma visible.
2. Crea un `VideoPlayer` component, ya sea haciendo referencia a una URL de streaming o a una ruta a un archivo de video. Aquí también puedes configurar el `playing` state y su volume. Este component puede asignarse a la entity de la pantalla del video o a cualquier otra entity de la escena.
3. Crea un `VideoTexture` objeto, y en su `videoPlayerEntity` propiedad asigna la entity que posee el `VideoPlayer` componente.
4. Crea un `Material`, asígnalo a la entity de la pantalla y configura su `texture` hasta la `VideoTexture` que creaste.

Este ejemplo usa un video que se almacena localmente en una `/videos` carpeta en el proyecto de la escena:

```ts
// #1
const screen = engine.addEntity()
MeshRenderer.setPlane(screen)
Transform.create(screen, { position: { x: 4, y: 1, z: 4 } })

// #2
VideoPlayer.create(screen, {
	src: 'videos/myVideo.mp4',
	playing: true,
})

// #3
const videoTexture = Material.Texture.Video({ videoPlayerEntity: screen })

// #4
Material.setBasicMaterial(screen, {
	texture: videoTexture,
})
```

Para usar un video desde una URL de streaming externa, cambia el paso 2 para que el `src` propiedad en el `VideoPlayer` component haga referencia a la URL de streaming.

```ts
// #2
VideoPlayer.create(screen, {
	src: 'https://vz-7c61c1b5-d59.b-cdn.net/ccea595a-b910-4de6-b160-092819db021d/playlist.m3u8',
	playing: true,
})
```

Consulta [Streaming en vivo](#live-streaming) para más detalles sobre cómo usar este tercer método alternativo.

## Acerca del streaming externo

La fuente del streaming debe ser una *https* URL (*http* las URLs no son compatibles).

Deberías poder pegar una URL que apunte a un video de la mayoría de los sitios populares de streaming de video. Ten en cuenta los términos de servicio de estas plataformas.

Para hacer streaming desde un archivo de video que tienes en tu máquina local, la forma más sencilla es subir este video a un Google Drive público y pegar el enlace.

Otra opción es usar un proveedor de hosting gestionado como [Vimeo](https://vimeo.com/) , [Livepeer Studio](https://livepeer.studio/) o [Serraform](https://serraform.gitbook.io/streaming-docs/guides/decentraland-playback) donde pagas una tarifa al proveedor para que gestione toda la infraestructura de streaming.

El enfoque más potente es configurar tu propio server, usando software libre pero pagando el hosting en una plataforma como [Digital Ocean](https://try.digitalocean.com/developerbrand/?_campaign=emea_brand_kw_en_cpc&_adgroup=digitalocean_exact_exact&_keyword=digitalocean&_device=c&_adposition=&_content=conversion&_medium=cpc&_source=bing&msclkid=160bfc160a2a1bab9bbf9933594bd9c5&utm_source=bing&utm_medium=cpc&utm_campaign=emea_brand_kw_en_cpc&utm_term=digitalocean&utm_content=DigitalOcean%20Exact_Exact) o [Cloudflare](https://www.cloudflare.com/products/cloudflare-stream/). Puedes desplegar algo como un [Node Media Server](https://github.com/illuspas/Node-Media-Server), que proporciona la mayor parte de lo que necesitas de inmediato.

Todas estas opciones tienen ventajas y desventajas para distintos escenarios. Debes evaluar qué es lo mejor para ti teniendo en cuenta tus necesidades, habilidades técnicas y presupuesto.

## Configurar OBS para una transmisión exitosa

[OBS](https://obsproject.com/) es una herramienta popular y gratuita para gestionar tus streams.

Ya sea que estés usando la stream key de un venue o tu propio server RTMP, tus ajustes en OBS son importantes para el éxito de tu stream. Debes apuntar a una conexión sólida y estable.

### Configuración simple de OBS

Se recomienda la siguiente configuración simple:

* Bitrate 2500kbps (funcionará con todos los venues de Decentraland)
* Bitrate de audio 160kbps
* Preajuste del codificador de video: Hardware NVENC
* Codificador de audio AAC
* Resolución máxima: 720 (cualquier valor mayor causa problemas en DCL)
* Frame rate 30fps


> [!WARNING]
> **📔 Nota**: Asegúrate de desactivar Multitrack streaming en OBS. Puedes encontrar esto en Settings > Stream.


### Consejos para nuevos streamers

* Las comprobaciones de sonido tempranas son esenciales para probar tu configuración con el venue.
* Pequeños errores como un dígito incorrecto en la stream key son los más probables de arruinar el stream.
* No pases de una resolución de 720 ni de un bitrate de 2500 kbps.

## Streaming en vivo

Puedes hacer livestream desde tu cámara o compartir tu pantalla usando la [Streaming en vivo](/docs/creator/content-creator-es/scene-editor/operar-en-vivo/live-streaming/) función del [Herramientas de administración](/docs/creator/content-creator-es/scene-editor/operar-en-vivo/scene-admin/) smart item.

Este método de streaming usa la misma arquitectura de comms que se usa para las comunicaciones en vivo entre players. Es fácil de configurar y tiene mucho menos delay que hacer streaming desde fuentes externas.

1. Añade un [Herramientas de administración](/docs/creator/content-creator-es/scene-editor/operar-en-vivo/scene-admin/) smart item a tu escena, así como un [Video player](/docs/creator/content-creator-es/scene-editor/interactividad/video-screen/) smart item.
2. Publica tu escena, ya sea en un World o en Genesis City.
3. Entra a la escena como un player con permiso para usar las herramientas de Admin.
4. Abre la consola de Admin, selecciona la **Video** pestaña, luego selecciona la **Live** funcionalidad y haz clic en el **Get Stream Key** botón.
5. Copia el **Server URL**  y *Streaming key*a tu software de streaming (por ejemplo OBS).
6. Pulsa el **Activar** botón para empezar a transmitir.

En lugar de añadir un Video player smart item a tu escena, también puedes usar la URL `livekit-video://current-stream` como source del video, para reproducir el stream en tu escena. Aun así, necesitarás las herramientas de Admin para obtener la stream key.

```ts
// #1
const screen = engine.addEntity()
MeshRenderer.setPlane(screen)
Transform.create(screen, { position: { x: 4, y: 1, z: 4 } })

// #2
VideoPlayer.create(screen, {
	src: `livekit-video://current-stream`,
	playing: true,
})

// #3
const videoTexture = Material.Texture.Video({ videoPlayerEntity: screen })

// #4
Material.setBasicMaterial(screen, {
	texture: videoTexture,
})
```

## Materiales de video

La mayor parte del tiempo, querrás reproducir videos en un [material Basic](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/materials/#unlit-materials), en lugar de un material PBR. Esto da como resultado una imagen mucho más brillante y nítida y es mejor para el rendimiento.

```ts
Material.setBasicMaterial(screen, {
	texture: videoTexture,
})
```

Por lo general, se recomienda reproducir videos en materiales Basic unlit, ya que esto es mejor para el rendimiento. Sin embargo, si quieres proyectar un video sobre un material PBR, ten en cuenta que las propiedades predeterminadas hacen que el video se vea bastante opaco. Puedes mejorar esto alterando otras propiedades del material. Aquí tienes algunos ajustes recomendados para que el video destaque más:

```ts
Material.setPbrMaterial(screen, {
	texture: videoTexture,
	roughness: 1.0,
	specularIntensity: 0,
	metallic: 0,
	emissiveTexture: videoTexture,
	emissiveIntensity: 0.6,
	emissiveColor: Color3.White(),
})
```


> [!NOTE]
> **💡 Consejo**: Como el video es una texture que se añade a un material, también puedes experimentar con otras propiedades de los materials, como teñirlo con un color o añadir otras capas de texture, por ejemplo para producir un efecto de pantalla sucia.
> 
> Consulta [materials](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/materials/) para más detalles.


## Acerca de los archivos de video

Se admiten los siguientes formatos de archivo:

* *.mp4*
* *.ogg*
* *.webm*

Ten en cuenta que un archivo de video aumenta el tamaño total de la escena, lo que hace que la escena tarde más en descargarse para los jugadores que entren en tu escena. El tamaño del video también podría hacer que superes las [limitaciones de la escena](/docs/creator/content-creator-es/scenes-sdk7/optimizacion/scene-limitations/), ya que tienes un máximo de 15 MB por parcel para usar. Recomendamos comprimir el video todo lo posible, para que sea menos problemático.

También recomendamos empezar a reproducir el video cuando el player esté cerca o realice una action para activarlo. Empezar a reproducir un video cuando tu escena se carga lejos, en el horizonte, afectará innecesariamente al rendimiento mientras los players visitan escenas vecinas.

## Iniciar, pausar y detener un video

Para empezar a reproducir el video o pausar, establece el `playing` propiedad a *true* o *false*. Si `playing` está configurado en false, el video se pausa en el último frame mostrado.

Puedes hacer que una screen sea alternable añadiéndole un pointer event como se muestra abajo:

```ts
pointerEventsSystem.onPointerDown(
	{
		entity: screen,
		opts: { button: InputAction.IA_POINTER, hoverText: 'Play/Pause' },
	},
	function () {
		const videoPlayer = VideoPlayer.getMutable(screen)
		videoPlayer.playing = !videoPlayer.playing
	}
)
```

Para detener el video y devolverlo al primer frame, establece el `posición` propiedad a 0. En el siguiente ejemplo, al hacer clic sobre el video se detiene.

```ts
pointerEventsSystem.onPointerDown(
	{
		entity: screen,
		opts: { button: InputAction.IA_POINTER, hoverText: 'STOP' },
	},
	function () {
		const videoPlayer = VideoPlayer.getMutable(screen)
		videoPlayer.playing = false
		videoPlayer.position = 0
	}
)
```

## Configura el video player

Las siguientes propiedades opcionales están disponibles para configurarse en el `VideoPlayer` component:

* `playing`: Determina si el video se está reproduciendo actualmente. Si es false, el video se pausa.


> [!WARNING]
> **📔 Nota**: Solo puede reproducirse un número limitado de videos al mismo tiempo, dependiendo de la configuración de calidad del player. Si la escena reproduce más que eso, el engine pausa los de menor prioridad. Ver [Consideraciones de rendimiento](#performance-considerations).


* `playbackRate`: Cambia la velocidad a la que se reproduce el video. *1* por defecto.
* `volume`: Te permite cambiar el volumen del audio. *1* por defecto.
* `posición`: Te permite establecer una posición de inicio diferente en el video. Se expresa en segundos después del inicio original del video. *0* por defecto, lo que hace que comience al inicio real del video.
* `loop`: Booleano que determina si el video se reproduce continuamente en un loop, o si se detiene después de reproducirse una vez. *false* por defecto.

## Reproducir varios videos

Para evitar problemas de rendimiento, mantén baja la cantidad de videos diferentes reproduciéndose al mismo tiempo; el engine solo permite un número limitado de videos simultáneos, dependiendo de la configuración de calidad del player (ver [Consideraciones de rendimiento](#performance-considerations)). Sin embargo, una escena puede reproducir varias copias de la misma video texture en varias screens diferentes. Esto no está restringido, ya que impacta mucho menos en el rendimiento que reproducir videos separados. Para reproducir el mismo video en múltiples entities, simplemente asigna la misma instancia del objeto de video texture a los `Material` components de cada entity de pantalla.

```ts
// #1
const screen1 = engine.addEntity()
MeshRenderer.setPlane(screen1)
Transform.create(screen1, { position: { x: 4, y: 1, z: 4 } })

const screen2 = engine.addEntity()
MeshRenderer.setPlane(screen2)
Transform.create(screen2, { position: { x: 6, y: 1, z: 4 } })

// #2
VideoPlayer.create(screen1, {
	src: 'https://vz-7c61c1b5-d59.b-cdn.net/ccea595a-b910-4de6-b160-092819db021d/playlist.m3u8',
	playing: true,
})

// #3
const videoTexture = Material.Texture.Video({ videoPlayerEntity: screen1 })

// #4
Material.setBasicMaterial(screen1, {
	texture: videoTexture,
})

Material.setBasicMaterial(screen2, {
	texture: videoTexture,
})
```

Ten en cuenta que en el ejemplo anterior, solo es necesario crear un `VideoPlayer` component, que controla el state de ambas video screens. En este caso, el component se asigna a la `screen1` entity, pero también podría asignarse a cualquier otra entity de la escena, no necesariamente a una de las screens.

## Eventos de video

Gestiona fácilmente los cambios de state en un video, para responder cuando un video empieza a reproducirse, se pausa, etc. Esto puede usarse, por ejemplo, para reproducir animaciones perfectamente sincronizadas con un video, asegurando que comiencen al mismo tiempo que el video.

Usa `videoEventsSystem.registerVideoEventsEntity` para definir una función que se ejecuta cada vez que cambia el state del video asignado a una entity. Cada vez que cambia el state, tu función puede comprobar el nuevo state y responder en consecuencia.

```ts
import {
	engine,
	Entity,
	VideoPlayer,
	videoEventsSystem,
	VideoState,
} from '@dcl/sdk/ecs'

// ... Crea videoPlayerEntity con VideoPlayer component, Transform, MeshRenderer.setPlane(), etc. ...

videoEventsSystem.registerVideoEventsEntity(
	videoPlayerEntity,
	function (videoEvent) {
		console.log(
			'video event - state: ' +
				videoEvent.state +
				'\ncurrent offset:' +
				videoEvent.currentOffset +
				'\nvideo length:' +
				videoEvent.videoLength
		)

		switch (videoEvent.state) {
			case VideoState.VS_READY:
				console.log('video event - video is READY')
				break
			case VideoState.VS_NONE:
				console.log('video event - video is in NO STATE')
				break
			case VideoState.VS_ERROR:
				console.log('video event - video ERROR')
				break
			case VideoState.VS_SEEKING:
				console.log('video event - video is SEEKING')
				break
			case VideoState.VS_LOADING:
				console.log('video event - video is LOADING')
				break
			case VideoState.VS_BUFFERING:
				console.log('video event - video is BUFFERING')
				break
			case VideoState.VS_PLAYING:
				console.log('video event - video started PLAYING')
				break
			case VideoState.VS_PAUSED:
				console.log('video event - video is PAUSED')
				break
		}
	}
)
```

El objeto videoEvent pasado como input para la función contiene las siguientes propiedades:

* `currentOffset` (*number*): El valor actual de la `posición` propiedad del video. Este valor muestra los segundos transcurridos desde el comienzo original del video. *-1* si el video aún no ha empezado a reproducirse.
* `state`: El nuevo estado del video, expresado como un valor de `VideoState` enum. Este enum puede contener los siguientes valores posibles:
  * `VideoState.VS_READY`
  * `VideoState.VS_NONE`
  * `VideoState.VS_ERROR`
  * `VideoState.VS_SEEKING`
  * `VideoState.VS_LOADING`
  * `VideoState.VS_BUFFERING`
  * `VideoState.VS_PLAYING`
  * `VideoState.VS_PAUSED`
* `videoLength` (*number* ): La duración en segundos de todo el video. *-1* si se desconoce la duración.
* `timestamp` ( *number*): Un *timestamp de Lamport* que se incrementa cada vez que el video cambia de state.
* `tickNumber` (*number*): El momento en que ocurrió el evento, expresado como el conteo de ticks desde que la escena empezó a ejecutarse.

Si vuelves a registrar un callback en una entity que ya tiene uno (por ejemplo, para cambiar lo que ocurre cuando cambia el state), el nuevo callback reemplaza al anterior y se conserva el último state informado. Esto significa que el nuevo callback no se llama de nuevo para un state que ya se había informado al callback anterior.

### Comprueba si una entity está registrada

Usa `videoEventsSystem.hasVideoEventsEntity()` para comprobar si una entity ya tiene registrado un callback de video events.

```ts
if (videoEventsSystem.hasVideoEventsEntity(videoPlayerEntity)) {
	console.log('Esta entity ya tiene un listener de video events')
}
```

### Último video event

Consulta el último cambio de state de un video usando `videoEventsSystem.getVideoState()`. Esto devuelve el último `VideoEvent` valor para la entity, o `undefined` si todavía no se ha reportado ningún evento. Puedes llamar a esto desde un system para consultar el state del video en cada frame sin registrar un callback.

```ts
function mySystem() {
	const latestVideoEvent = videoEventsSystem.getVideoState(videoPlayerEntity)
	if (!latestVideoEvent) return

	console.log(`state: ${latestVideoEvent.state}
    \ncurrentOffset: ${latestVideoEvent.currentOffset}
    \nvideoLength: ${latestVideoEvent.videoLength}`)
}
```

## Máscaras alfa en videos

Un truco útil para tener screens de video no rectangulares es aplicar una alpha texture sobre un plane. Puedes recortar parte del plane en la forma que quieras.

Usa la siguiente imagen para recortar tu video en una forma circular, con las esquinas transparentes.

![](/docs/_remote/1f517d241f0753831e85b88c.webp)

```ts
const videoTexture = Material.Texture.Video({
	videoPlayerEntity: screen,
})
const alphaMask = Material.Texture.Common({
	src: 'assets/scene/circle_mask.png',
	wrapMode: TextureWrapMode.TWM_MIRROR,
})

Material.setBasicMaterial(screen, {
	texture: videoTexture,
	alphaTexture: alphaMask,
})
```

![](/docs/_remote/9b242e1d8862ee66017fbceb.webp)


> [!WARNING]
> **📔 Nota**: En versiones anteriores, la `alphaTexture` property solo estaba presente en los materiales PBR. Actualmente, solo funciona en materiales basic.


## Reproducir un video en un modelo glTF

Puedes reproducir un video en un *glTF* modelo usando el [GltfNodeModifiers](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/materials/#modify-gltf-materials) componente. Vea [Modificar materiales glTF](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/materials/#modify-gltf-materials) para más detalles.

Esto te permite reproducir tus videos en cualquier forma, no solo en planes. Por ejemplo, puedes reproducir videos en una screen curva, o incluso en todo el cuerpo de un NPC.

```ts
const myEntity = engine.addEntity()

GltfContainer.create(myEntity, {
	src: 'models/myModel.glb',
})

Transform.create(myEntity, {
	position: Vector3.create(4, 0, 4),
})

VideoPlayer.create(myEntity, {
	src: 'https://vz-7c61c1b5-d59.b-cdn.net/ccea595a-b910-4de6-b160-092819db021d/playlist.m3u8',
	playing: true,
})

GltfNodeModifiers.create(myEntity, {
	modifiers: [
		{
			path: '',
			material: {
				material: {
					$case: 'pbr',
					pbr: {
						texture: Material.Texture.Video({
							videoPlayerEntity: myEntity,
						}),
					},
				},
			},
		},
	],
})
```

La asignación del video seguirá el mapeo UV original que usa el modelo. Esto significa que, si el modelo tiene una texture mapeada a una parte específica del modelo, el video se mapeará a esa misma parte.

También puedes usar el `GltfNodeModifiers` component para reproducir un video solo en una mesh específica dentro del modelo. Por ejemplo, puedes reproducirlo en una wall específica de un building, aunque el modelo abarque todo el building. Ver [Modificar materiales glTF](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/materials/#modify-gltf-materials) para más detalles.

## Audio espacial

De forma predeterminada, el video de un `VideoPlayer` component es global, lo que significa que se escuchará con un volumen constante en toda tu escena. Si un player sale de la escena, no escuchará el streaming en absoluto.

Para hacer que el audio sea spatial, establece el `spatial` propiedad a *true*.

```ts
VideoPlayer.create(entity, {
	src: 'https://player.vimeo.com/progressive_redirect/playback/1145666916/rendition/540p/file.mp4%20%28540p%29.mp4?loc=external&signature=db1cd6946851313cb8f7be60d1f6c30af0902bcc46fdae0ba2a06e5fdf44c329',
	playing: true,
	spatial: true,
})
```

El video ahora se escuchará desde la posición de la entity que posee el `VideoPlayer` component, y sonará más fuerte a medida que el player se acerque.

Controla el audio spatial con las siguientes propiedades:

* `spatialMinDistance`: La distancia mínima a la que el audio se vuelve spatial. Si el player está más cerca, el audio se escuchará a volumen completo. *0* por defecto.
* `spatialMaxDistance`: La distancia máxima a la que se escucha el audio. Si el player está más lejos, el audio se escuchará a volumen 0. *60* por defecto

```ts
const videoPlayerEntity = engine.addEntity()

Transform.create(videoPlayerEntity, {
	position: Vector3.create(8, 2, 8),
})

VideoPlayer.create(videoPlayerEntity, {
	src: 'https://player.vimeo.com/progressive_redirect/playback/1145666916/rendition/540p/file.mp4%20%28540p%29.mp4?loc=external&signature=db1cd6946851313cb8f7be60d1f6c30af0902bcc46fdae0ba2a06e5fdf44c329',
	playing: true,
	spatial: true,
	spatialMinDistance: 5,
	spatialMaxDistance: 10,
})

MeshRenderer.setPlane(videoPlayerEntity)

Material.setBasicMaterial(videoPlayerEntity, {
	texture: Material.Texture.Video({ videoPlayerEntity: videoPlayerEntity }),
})
```


> [!WARNING]
> **📔 Nota**: Algunos formatos de video no admiten audio spatial. Asegúrate de que el stream esté codificado en *mp4*, *m4a*, o *mov*.


## Análisis de audio

Puedes leer datos de amplitud y frecuencia en tiempo real de la pista de audio de un `VideoPlayer` entity para impulsar visuales reactivos que se sincronizan con la banda sonora del video. Ver [Análisis de audio](/docs/creator/content-creator-es/scenes-sdk7/media/audio-analysis/).


> [!NOTE]
> **💡 Consejo**: Para ver el video reproducido en un modelo GLTF en lugar de un primitive, consulta la [`74,-8-gltfnodemodifier`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/74,-8-gltfnodemodifier) escena de prueba, que envía un stream HLS a nodos específicos de GLTF usando `GltfNodeModifiers` texturas de video.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/media/video-playing.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
