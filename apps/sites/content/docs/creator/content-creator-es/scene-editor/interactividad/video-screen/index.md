> Reproduce videos en tu escena

# Pantalla de video

Para reproducir videos pregrabados o transmitidos en una pantalla de tu escena, usa el Video Player [Smart Item](/docs/creator/content-creator-es/scene-editor/interactividad/smart-items/).

![](/docs/_remote/bb8266cdd73eaeb440e3fe4d.webp)

## Configuración general

Estos ajustes son relevantes para todos los escenarios, ya sea que estés reproduciendo videos o haciendo streaming.

![](/docs/_remote/6418389e00c134e192913d0e.webp)

Puedes configurar el volumen del audio del video. Ten en cuenta que el audio del stream no es posicional; se escucha con un volumen uniforme en toda tu escena.

La sección **Fuentes de medios predeterminadas** el dropdown te permite elegir entre dos tipos diferentes de fuentes:

* **URL de video**: Obtén un video o un stream desde una URL o un archivo de video local
* **Live Stream**: Usa la infraestructura gratuita de streaming de Decentraland para mostrar un stream. Para usar esto, también debes incluir un [Herramientas de administración](/docs/creator/content-creator-es/scene-editor/operar-en-vivo/scene-admin/) smart item en tu escena.

## Reproducir videos

Puedes reproducir videos pregrabados desde cualquiera de estas opciones:

* **Archivos locales**: Sube un archivo de video como parte de la escena, luego apunta el *URL* campo a la ruta de ese archivo.
* **Transmitir desde una URL**: Apunta a un stream en vivo o pregrabado en la web, por ejemplo, de un proveedor como Bunny. Consulta [videos en streaming](#streaming-from-other-sources)

El momento en que el smart item Video Player reproduce un video puede depender de diferentes cosas:

* **Automático**: El video comienza a reproducirse tan pronto como se carga la escena. Para esto, establece el dropdown de fuente de medios predeterminada en **URL de video** y pega una URL directamente en el **Video Path o URL .m3u8** campo.

  ![](/docs/_remote/6418389e00c134e192913d0e.webp)
* **Activado por un administrador**: Un [administrador de la escena](/docs/creator/content-creator-es/scene-editor/operar-en-vivo/scene-admin/) que esté actualmente en la escena puede usar la UI de administración para pegar una URL de video y reproducirla para todos los jugadores que estén actualmente en la escena.
* **Basado en las acciones del jugador**: Define una Action de tipo **Reproducir video**. Esto te permite activar la reproducción del video como resultado de interactuar con otro smart item, como entrar en una habitación o pulsar un botón. Consulta [Smart Items - Advanced](/docs/creator/content-creator-es/scene-editor/interactividad/smart-items-advanced/).

  ![](/docs/_remote/87a1916dbb31dbea1502aad6.webp)

En todos los casos configuras el video para que se repita en bucle o se reproduzca una sola vez.


> [!WARNING]
> **📔 Nota**: Si se reproducen demasiados videos al mismo tiempo en tu escena, algunos serán pausados por el Engine. La prioridad se determina según la proximidad al jugador, la dirección de la cámara y el tamaño de la pantalla. La cantidad máxima de videos simultáneos depende de los ajustes de calidad del jugador.
> 
> * Baja: 1
> * Media: 5
> * Alta: 10
> 
> También recomendamos empezar a reproducir el video cuando el jugador esté cerca o realice una acción para hacerlo. Empezar a reproducir un video cuando tu escena se carga lejos en el horizonte afectará innecesariamente el rendimiento mientras los jugadores visitan escenas vecinas.


## Múltiples pantallas de video

Puedes reproducir el mismo video en varias pantallas al mismo tiempo. Para hacerlo, debes editar las propiedades avanzadas del smart item Video Player.


> [!WARNING]
> **📔 Nota**: Evita tener más de un video diferente reproduciéndose al mismo tiempo, ya que eso perjudica mucho el rendimiento.
> 
> Si simplemente pegas la misma URL en dos reproductores de video, el Engine no sabrá que se trata del mismo video y reproducirá ambos por separado. Sigue los pasos a continuación para configurar el segundo reproductor de video para que reproduzca el mismo video que el primero.


1. Agrega dos smart items Video Player a la escena, uno para cada pantalla.
2. Configura el primero normalmente, como se describe en el [Reproducir videos](#play-videos) sección.
3. En el segundo reproductor de video, elimina el **Video Player** componente.

![](/docs/_remote/a77f00900280e771283a1861.webp)


> [!WARNING]
> **📔 Nota**: Este paso es importante; de lo contrario, el Engine procesará el segundo reproductor de video, incluso si no es visible.


4. Todavía en el segundo reproductor de video, abre el **Material** componente, expande la **Texture** sección y selecciona el **Video Source Entity** dropdown para apuntar al primer reproductor de video.

![](/docs/_remote/ac9747acfdb69510f8b3a247.webp)

Puedes hacer lo mismo para cualquier cantidad de reproductores de video, siempre que configures cada uno para apuntar al mismo reproductor de video.

Al hacer [live streaming](/docs/creator/content-creator-es/scene-editor/operar-en-vivo/live-streaming/), ambas pantallas también mostrarán el mismo stream.


> [!NOTE]
> **💡 Consejo**: Los pasos anteriores también se pueden repetir con un elemento que tenga un **Cambiar material** componente, para convertir cualquier modelo 3D en una pantalla de video. Configura la **Texture** sección dentro del **Cambiar material** componente para apuntar a la entidad del reproductor de video.


## Acerca de los archivos de video

Se admiten los siguientes formatos de archivo:

* *.mp4*
* *.ogg*
* *.webm*

Ten en cuenta que un archivo de video aumenta el tamaño total de la escena, lo que hace que la escena tarde más en descargarse para los jugadores que entren en tu escena. El tamaño del video también podría hacer que superes las [limitaciones de la escena](/docs/creator/content-creator-es/scenes-sdk7/optimizacion/scene-limitations/), ya que tienes un máximo de 15 MB por parcel para usar. Recomendamos comprimir el video todo lo posible, para que sea menos problemático.

## Streaming en vivo

Para streaming en vivo de extremo a extremo, consulta [Live Streaming](/docs/creator/content-creator-es/scene-editor/operar-en-vivo/live-streaming/).

### Streaming desde otras fuentes

También puedes transmitir videos usando otras infraestructuras de streaming. Para hacerlo, simplemente configura el smart item Video Player para usar la **URL de video** fuente de medios, y pega la URL del stream en el **Video Path o URL .m3u8** campo.

La fuente del streaming debe ser una *https* URL (*http* las URL no son compatibles), y la fuente debe tener [políticas CORS (Cross Origin Resource Sharing)](https://en.wikipedia.org/wiki/Cross-origin_resource_sharing) que permitan acceder a ella desde el exterior. Esto significa que no puedes transmitir un video desde YouTube o sitios similares, ya que estos solo permiten mostrar su contenido en su widget HTML con marca. Consulta [Acerca del streaming externo](/docs/creator/content-creator-es/scenes-sdk7/media/video-playing/#about-external-streaming) para ver opciones y consejos.

Hay varias opciones para transmitir video. La opción más simple es usar un proveedor de hosting administrado como [Vimeo](https://vimeo.com/), [Bunny](https://bunny.net), [Livepeer Studio](https://livepeer.studio/) o [Serraform](https://serraform.gitbook.io/streaming-docs/guides/decentraland-playback) donde pagas una tarifa al proveedor para que gestione toda la infraestructura de streaming.

Lee [Configurar OBS para una transmisión exitosa](/docs/creator/content-creator-es/scenes-sdk7/media/video-playing/#setting-up-obs-for-successful-streaming) para ver consejos sobre cómo transmitir contenido de la mejor manera a Decentraland.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scene-editor/interactividad/video-screen.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
