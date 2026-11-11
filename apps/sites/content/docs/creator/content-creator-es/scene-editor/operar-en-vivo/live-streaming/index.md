> Transmite video en vivo a tu escena usando la pantalla de video y las herramientas de administración.

# Streaming en vivo

Usa la **Pantalla de video** smart item junto con el **Admin Tools** smart item para transmitir video en vivo en tu escena.

Decentraland ofrece distintas formas de transmitir video en vivo en tu escena:

* **DCL Cast** *(Modo fácil)*: Usa la aplicación web gratuita de streaming de Decentraland para compartir fácilmente tu cámara o pantalla con otros jugadores en la escena, sin necesidad de configurar un software de streaming. Este modo tiene la menor latencia y es el más fácil de configurar.
* **Stream** *(Modo avanzado)*: Usa un software de streaming como [OBS](https://obsproject.com/) para transmitir a través de la infraestructura de streaming de Decentraland. Este modo te permite tener más control sobre la transmisión, como el diseño de la pantalla y las fuentes de audio.
* **URL de video** *(Modo avanzado)*: Apunta a tu propia infraestructura de streaming, pegando la URL en el **URL de video** campo.

<img src="/docs/_remote/9fe1dc304a4f62e468f108b3.webp" alt="Métodos de streaming" width="400">

El streaming funciona en Worlds y Genesis City, sin límites de audiencia del lado de la escena.

## Configura la escena

Los siguientes pasos son comunes tanto para DCL Cast como para los métodos de Stream:

1. Añade un **Pantalla de video** smart item a tu escena.

   ![](/docs/_remote/bb8266cdd73eaeb440e3fe4d.webp)
2. Añade un **Admin Tools** smart item y habilita la sección **Video Screens** . Selecciona cada pantalla del menú desplegable y asígnale un nombre amigable para la UI de administración.

   ![](/docs/_remote/34ed9d039629babf7b485b20.webp)
3. Publica tu escena (World o Genesis City) e ingresa como un usuario con permisos de administrador.

   ![](/docs/_remote/522b72b559e9bf32de19bcab.webp)

Una vez que tu escena esté publicada, puedes entrar como usuario con permisos de administrador y configurar los ajustes de streaming.


> [!NOTE]
> **💡 Consejo**: Si agregas múltiples Video Screens para mostrar el mismo video, configura la fuente de todas excepto una para que apunte al mismo reproductor de video, ver [Múltiples Video Screens](/docs/creator/content-creator-es/scene-editor/interactividad/video-screen/#multiple-video-screens) para más detalles.


## DCL Cast (fácil)

### Compartir acceso a la app

Entra en tu escena publicada como usuario administrador y abre el panel de administración. Selecciona la pestaña **Video** y luego selecciona la funcionalidad **DCL Cast** .

<img src="/docs/_remote/0d1340c42e272abcc25bf603.webp" alt="DCL Cast" width="400">

Verás dos enlaces que puedes copiar y compartir con otras personas.

* **Cast Speakers**: Este enlace es para que los speakers lo usen para transmitir su video a la escena.

  <div data-gb-custom-block data-tag="hint" data-style="danger" class="hint hint-danger"><p><strong>❗Advertencia</strong>: Trata el enlace de streaming como un secreto; compártelo solo con personas en las que confíes. Reinicia el enlace entre presentadores si es necesario.</p><p>Cuando termines de transmitir, cierra la pestaña del navegador de DCL Cast para liberar el canal.</p></div>
* **Viewers**: Este enlace es para que la audiencia vea el video desde un navegador o móvil. Esto es útil para jugadores que no están dentro de Decentraland en ese momento.

Haz clic en el **Botón de copiar enlace** para copiar los enlaces al portapapeles.

Cuando estés listo para transmitir, haz clic en el **Activate** botón para hacer visible la transmisión para la audiencia en la escena.

<img src="/docs/_remote/6fa0aca94ae7997a1988bcad.webp" alt="Activar transmisión" width="150">

Si por alguna razón necesitas reiniciar la sala, haz clic en el **Reset Room** botón para generar una nueva. Cualquiera que esté transmitiendo en ese momento se desconectará.

<img src="/docs/_remote/5daf35dba544baf2c6504619.webp" alt="Reiniciar sala" width="150">

### Usar la app DCL Cast

Cuando alguien pega el enlace del speaker en un navegador, verá una pantalla como esta:

<img src="/docs/_remote/8b74b9e4f571aec33f619bc5.webp" alt="app DCL Cast" width="400">

El navegador te pedirá permiso para compartir tu cámara y micrófono. También puedes configurar los distintos dispositivos de entrada que se usarán para la transmisión.


> [!NOTE]
> **📔 Consejo**: Usa Google Chrome o un navegador basado en el motor de Chrome. Estos navegadores ofrecen la funcionalidad para compartir fácilmente tanto video como audio directamente desde una pestaña del navegador.


Los usuarios pueden introducir un nombre (no tiene que coincidir con su nombre de usuario de Decentraland) y hacer clic en el **Join Now** botón para empezar a transmitir.

Una vez en transmisión, la app es similar a varias aplicaciones conocidas de videoconferencia, con botones para silenciar/reactivar el audio, compartir cámara y pantalla, y una interfaz de chat.

El chat es de solo lectura y escucha todos los mensajes enviados por los jugadores dentro de la escena en Decentraland. Esto es excelente para mantenerse en contacto con la audiencia, incluso si estás transmitiendo desde un dispositivo diferente.

<img src="/docs/_remote/66b24803959f06d37231b10c.webp" alt="app DCL Cast" width="700">

En la **Pestaña Participants** puedes ver tres listas:

* **Speakers**: Las personas que están transmitiendo actualmente a la escena.
* **Viewers**: Las personas que están viendo la transmisión desde un navegador.
* **Participantes dentro del mundo**: Los jugadores que están actualmente dentro de la escena, viendo la transmisión in-world.

<img src="/docs/_remote/1ff69ac3627fa7cbbad58fca.webp" alt="Pestaña Participants" width="200">

Si hay varios speakers presentes en una sesión de DCL Cast, los jugadores in-world escucharán las voces de todos los speakers, y la vista de video cambiará automáticamente para mostrar a quien esté emitiendo sonido en ese momento, para mostrar siempre quién está hablando.

Para anular este comportamiento predeterminado:

* Haz clic en el **Speakers** botón en el panel de administración de la escena

<img src="/docs/_remote/2e5e3dd8ae5ce493e7c7cc13.webp" alt="Pestaña Participants" width="200">

* Elige uno de los speakers y selecciona una fuente para mostrar (ya sea la cámara o la pantalla de ese speaker)

<img src="/docs/_remote/7e4c6bd675830ab1e698bcfc.webp" alt="Pestaña Participants" width="200">

Esto forzará a que esta fuente se muestre siempre en pantalla, sin importar si otros speakers están hablando.


> [!NOTE]
> **📔 Consejo**: Si tú también estás in-world viendo la transmisión, puede resultarte molesto escuchar eco del audio repetido tanto en la app DCL Cast como en la escena de Decentraland. Puedes silenciar fácilmente todo el audio de la app DCL Cast alternando el icono de speaker en la esquina inferior izquierda de la pantalla
> 
> <img src="/docs/_remote/f1c102f46a10285862b60d5a.webp" alt="Participants tab" data-size="original">
> 
> De lo contrario, puedes silenciar el audio en la configuración de Decentraland.


### Compartir presentaciones

También puedes compartir el contenido de una presentación de diapositivas como una fuente alternativa de imágenes.

* Desde in-world, haz clic en el **Compartir presentación** botón in-world en la pestaña DCL Cast. Desde la app DCL Cast, haz clic en el menú desplegable junto al botón **Share Screen** y selecciona **Compartir presentación**.

<img src="/docs/_remote/021196a0a453b918bd70cad9.webp" alt="Pestaña Participants" width="200">

* Pega un enlace de Google Slides, un enlace a un .pdf alojado en Drive o una fuente similar, o sube un archivo .pdf.

La presentación ahora será una fuente que se puede seleccionar para mostrar en pantalla, mientras las voces de todos los speakers siguen escuchándose.


> [!WARNING]
> **📔 Nota**: Los archivos de presentación deben ser de menos de 100 MB. Las presentaciones de Google Slides deben configurarse como *públicas*.
> 
> Solo puede haber una presentación activa a la vez en una sesión de DCL Cast.


Luego puedes cambiar de diapositiva, o incluso reproducir y pausar cualquier video incrustado en estas diapositivas pulsando botones que existen tanto en la app DCL Cast como in-world en la UI de Admin Tools.

## Stream (avanzado)

Para usar la función de Live Streaming en tu escena, necesitarás instalar un software de streaming que pueda enviar a un endpoint RTMP (por ejemplo, [OBS](https://obsproject.com/), [XSplit](https://www.xsplit.com/), [StreamYard](https://streamyard.com/)).


> [!WARNING]
> **Advertencia**: El live streaming no es compatible en mundos de Single Player. Si tu mundo tiene **Single Player** habilitado en World Settings (o `fixedAdapter` establecido en `"offline:offline"` en `scene.json`), desactívalo antes de configurar el streaming. La función de streaming depende de la capa de comunicaciones, que está deshabilitada en el modo Single Player.


### Obtener credenciales de streaming

1. Abre la UI de administración en la escena (icono superior derecho).

   ![](/docs/_remote/3a78785f1b44cd50b0963098.webp)
2. En la **Video** pestaña, cambia a **Stream** y haz clic en **Get Stream Key**.

   ![](/docs/_remote/548a347a2432d40b40d20705.webp)
3. Copia el **RTMP Server** y **Stream Key** en tu software de streaming.

   ![OBS configuration](/docs/_remote/683eec84be8a03a78e4ed799.webp)


> [!CAUTION]
> **❗Advertencia**: Solo una persona puede transmitir a una escena a la vez. Cuando termines de transmitir, haz clic en **Stop Streaming** en tu software para liberar el canal.


### Iniciar y controlar la transmisión

1. Empieza a transmitir desde tu software.
2. En la UI de administración, haz clic en **Activate** para mostrar la transmisión en la escena.

   <img src="/docs/_remote/6fa0aca94ae7997a1988bcad.webp" alt="Activar transmisión" width="100">

### Claves de transmisión

Las claves de transmisión se generan por escena y son válidas durante 4 días (96 horas). Una sola sesión en vivo puede ejecutarse hasta 4 horas continuas.

![](/docs/_remote/8fdba6b9787070f6aacf6827.webp)

* Haz clic **Reset Stream Key** para revocar la clave actual y emitir una nueva. Las transmisiones en curso se detendrán.
* Cada escena tiene su propia dirección de streaming y su propia clave. Los administradores pueden compartir la clave con streamers externos.
* Solo puede haber una transmisión activa por escena a la vez; iniciar una nueva sobrescribirá la transmisión actual.


> [!CAUTION]
> **❗Advertencia**: Trata las claves de transmisión como secretos. Reinicia la clave entre presentadores si es necesario.


## Streaming desde URL (avanzado)

También puedes transmitir configurando Video Screen para usar la opción **URL de video** y pegando una URL de streaming.

Deberías poder pegar una URL que apunte a un video de la mayoría de los sitios populares de streaming de video. Ten en cuenta los términos de servicio de estas plataformas.

Para transmitir desde un archivo de video que tienes en tu máquina local, la forma más sencilla es subir este video a un Google Drive público y pegar el enlace.

* La URL debe ser `https`. Consulta [Acerca del streaming externo](/docs/creator/content-creator-es/scenes-sdk7/media/video-playing/#about-external-streaming).
* Los proveedores recomendados incluyen [Vimeo](https://vimeo.com/), [Bunny](https://bunny.net), [Livepeer Studio](https://livepeer.studio/) y [Serraform](https://serraform.gitbook.io/streaming-docs/guides/decentraland-playback).
* Consejos para la configuración del codificador: [Configuración de OBS para un streaming exitoso](/docs/creator/content-creator-es/scenes-sdk7/media/video-playing/#setting-up-obs-for-successful-streaming).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scene-editor/operar-en-vivo/live-streaming.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
