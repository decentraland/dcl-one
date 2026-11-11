> Los administradores de la escena tienen control especial sobre lo que ocurre en la escena en tiempo real.

# Administración de la escena

Otorga a ciertos jugadores el rol especial de **admin** en tu escena.

Durante un evento en vivo, un admin puede controlar espontáneamente lo que sucede en la escena desde dentro de Decentraland, sin necesidad de programar acciones con antelación ni depender de un servicio de terceros. Empieza a reproducir la música cuando se haya reunido suficiente gente, lanza confeti o haz aparecer una nave espacial cuando el momento sea adecuado.



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/efjJN7Jr7Qo" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=efjJN7Jr7Qo>">Watch on YouTube</a></p>



Cuando un admin de la escena visita tu escena, ve un panel de admin especial en la esquina superior derecha que solo él puede ver. El panel usa pestañas con iconos en la parte superior para cambiar entre secciones (Video, Announcements, Smart Items, Permissions). A través de esta UI pueden reproducir videos o transmisiones en vivo, enviar anuncios, banear jugadores o activar cualquier smart item que esté configurado para activarse así. Estas acciones las ven todos los demás jugadores de la escena que estén conectados a la misma isla de comms que el admin.

![](/docs/_remote/4a423dfb13080da0e3f0dd27.webp)

## Configuración de admins

Para asignar admins, debes agregar el **Admin Tools** smart item a tu escena.

![](/docs/_remote/360f0190e175f35ce4c17c72.webp)


> [!WARNING]
> **📔 Nota**: Actualiza tu escena para usar las dependencias más recientes.
> 
> <img src="/docs/_remote/83f4693cc61cdbab26246a7d.webp" alt="" data-size="original">


Mientras desarrollas la escena y la pruebas localmente, siempre eres admin. Una vez que la escena se publica, cualquiera con permisos de publicación sobre la escena también es automáticamente admin. Esto incluye:

* El propietario de los parcels de LAND o del World NAME donde se publica la escena
* Cualquiera a quien se le conceda **derechos de operador** en estas parcelas o nombre. Consulta [Conceder permisos](/docs/player/marketplace/land-manager/#give-permissions).
* Cualquier usuario que alquile ese terreno. Consulta [Alquileres](/docs/player/marketplace/rentals/).

Para asignar admins adicionales que no tengan permiso de publicación pero sí puedan hacer live-ops en la escena:

1. Publica la escena y visita la versión en vivo como admin
2. Abre la **Permisos** pestaña (a través de las pestañas con iconos en la parte superior del panel de admin).

   ![](/docs/_remote/ed908a8f1e3f969689af965f.webp)
3. Escribe la dirección de la wallet de la persona que quieres agregar junto a **Agregar un Admin** y haz clic en **Add**.

Puedes ver quién es admin en la escena haciendo clic en el **Ver lista de admins** botón. Desde esta pantalla también puedes **Remove** personas de la lista de admins.

![](/docs/_remote/983d682c49d98b1ce3fb7227.webp)


> [!WARNING]
> **📔 Nota**: Solo es posible quitar el rol de admin a los jugadores que fueron añadidos manualmente a la lista a través de la **Permissions & Moderation** pestaña. Los jugadores que son propietarios, operadores o arrendatarios de la escena se muestran en esta lista, pero no se les puede quitar el rol de admin desde esta UI. Para quitar el rol de admin a un operador, primero debes quitar su rol de operador.


Cada vez que un jugador admin está en la escena, verá una UI especial en la esquina superior derecha. Los jugadores que no son admin no ven esta UI.

![](/docs/_remote/4a423dfb13080da0e3f0dd27.webp)

### Comprobar el estado de admin mediante código

También es posible saber si un Player es Admin de una escena usando código. Esto permite que otros comportamientos estén disponibles (o no) para un Player, por ejemplo, permitir interacciones con una Entity específica de la escena.

```ts
import { isAdmin } from "@dcl/asset-packs/dist/admin";

async function onPlayerSpawn() {
  const isAdminUser = await isAdmin();
  if (isAdminUser) {
    // Mostrar UI solo para admin, teletransportar al escenario, mostrar Entity, etc.
  }
}
```


> [!NOTE]
> **💡 Consejo**: Para más información sobre las funciones async, consulta [Funciones async](/docs/creator/content-creator-es/scenes-sdk7/patrones-de-programacion/async-functions/).


## Video playing

Una de las acciones más comunes que realizan los admins es reproducir videos. El panel de admin incluye una sección de reproductor de video donde pueden controlar todo lo relacionado con los videos.

Para habilitar esto, necesitas agregar un **Video Player** smart item a tu escena y vincularlo con el smart item Admin Tools.

1. Añade un **Video Player** smart item a tu escena

   ![](/docs/_remote/bb8266cdd73eaeb440e3fe4d.webp)

   Consulta [Pantalla de video](/docs/creator/content-creator-es/scene-editor/interactividad/video-screen/) para más detalles sobre cómo puedes configurar la fuente de medios predeterminada y otros ajustes del smart item Video Player. La mayoría de estas configuraciones pueden ser sobrescritas por el admin una vez que la escena está en ejecución.

   <div data-gb-custom-block data-tag="hint" data-style="warning" class="hint hint-warning"><p><strong>📔 Nota</strong>: Un admin solo puede administrar videos que se reproducen en el smart item Video Screen, no en pantallas añadidas mediante código SDK.</p><p>Puedes incluir tantas pantallas de video como quieras. En general, evita tener más de un video diferente reproduciéndose al mismo tiempo, ya que eso perjudica mucho el rendimiento.</p></div>
2. Abre el Smart Item Admin Tools y asegúrate de que la **Video Screens** casilla de verificación esté habilitada para que se muestre esta sección. Luego selecciona la pantalla de una lista desplegable y dale un nombre amigable para mostrarlo en la UI de admin. Puedes agregar tantas pantallas de video como quieras; cada pantalla se controla de forma independiente.

   <img src="/docs/_remote/34ed9d039629babf7b485b20.webp" alt="Nombre de la escena" width="300">

Una vez configurado lo anterior, los usuarios admin de tu escena pueden abrir el panel de admin y seleccionar la pestaña de video (a través de las pestañas con iconos en la parte superior) para controlar estas pantallas de video.

<img src="/docs/_remote/48468cefefacba6545b47b45.webp" alt="Nombre de la escena" width="300">

Si tu escena tiene varias pantallas de video independientes, el **Pantalla** desplegable te permite elegir qué pantalla de video controlar. La lista muestra los nombres que le asignaste a cada pantalla de video en la configuración del smart item Admin Tools.


> [!NOTE]
> **💡 Consejo**: Para mostrar el mismo video en varias pantallas que puedan controlarse como una sola, consulta [Múltiples Video Screens](/docs/creator/content-creator-es/scene-editor/interactividad/video-screen/#multiple-video-screens).


## Fuentes de medios

Un control segmentado en la parte superior de la sección de video te permite alternar entre tres fuentes de medios. La fuente actualmente activa se indica con una etiqueta tipo pill. Cambiar de pestaña solo muestra los ajustes de esa fuente; no interrumpe lo que se está reproduciendo actualmente hasta que hagas clic en **Activate**.

* **URL de video**: Reproduce un video desde una URL. Pega una URL de video en el campo y haz clic en **Activate**. El video comenzará a reproducirse en la pantalla seleccionada para todos los jugadores. También puedes detenerlo, pausarlo, reiniciarlo y ajustar el volumen.

  <img src="/docs/_remote/77179ebe47d00636f626ed06.webp" alt="Nombre de la escena" width="300">

  <div data-gb-custom-block data-tag="hint" data-style="warning" class="hint hint-warning"><p><strong>📔 Nota</strong>: No cualquier URL de video funcionará. Los videos de algunos sitios tienen políticas estrictas sobre su contenido y bloquearán el acceso a ellos desde Decentraland. Consulta <a href="/docs/creator/content-creator-es/scene-editor/interactividad/video-screen/#streaming-from-other-sources">Streaming desde otras fuentes</a> para más información sobre lo que puedes y no puedes reproducir en Decentraland.</p></div>
* **DCL Cast**: Usa la aplicación web gratuita de streaming de Decentraland para compartir fácilmente tu cámara o pantalla con otros jugadores de la escena, sin necesidad de configurar software de streaming. Los controles de presentación (Previous, Next, Play video, Stop) se muestran en línea cuando una presentación está activa.

  <img src="/docs/_remote/0d1340c42e272abcc25bf603.webp" alt="DCL Cast" width="300">
* **Stream**: Reproduce una transmisión en vivo usando la infraestructura gratuita de streaming de Decentraland y un software de streaming como OBS o StreamYard.

  <img src="/docs/_remote/79d537ec46382a7153d0fb2f.webp" alt="Nombre de la escena" width="300">

  Consulta [Streaming en vivo](/docs/creator/content-creator-es/scene-editor/operar-en-vivo/live-streaming/) para más información sobre cómo configurar una transmisión en vivo.

Cada sección de fuente de medios incluye un **control deslizante de volumen** que te permite ajustar el volumen del 0% (silenciado) al 100%.

![](/docs/_remote/6fa0aca94ae7997a1988bcad.webp)

## Anuncios

En la **Anuncios** pestaña del panel de admin, los admins pueden escribir mensajes que serán vistos por todos los jugadores en la escena. Mensajes como este solo pueden ser enviados por admins, por lo que otros jugadores los percibirán como más legítimos que un mensaje en el chat de alguien que afirma ser admin.

Selecciona la pestaña Anuncios del panel de admin. Escribe un mensaje y haz clic en **Compartir**. El mensaje puede tener hasta 90 caracteres. El campo de entrada se vacía automáticamente después de enviarlo, así que puedes escribir un mensaje nuevo enseguida.

![](/docs/_remote/a7a34d2c60e405a1f61bb7f6.webp)

## Banear jugadores

Puedes banear jugadores de tu escena seleccionando la **Permisos** pestaña del panel de admin, escribiendo el nombre o la dirección de la wallet del jugador que quieres banear y haciendo clic en el **Banear** botón.

![](/docs/_remote/ed908a8f1e3f969689af965f.webp)


> [!NOTE]
> **💡 Consejo**: Para obtener la dirección de la wallet de un jugador, haz clic en su avatar para abrir su perfil y luego haz clic en el **Copiar al portapapeles** botón junto a la dirección de la wallet.


Los jugadores baneados no podrán cargar tu escena ni interactuar con ninguno de sus contenidos. Los demás jugadores no los verán en la escena ni leerán ninguno de sus mensajes de chat.


> [!WARNING]
> **📔 Nota**: Los efectos de tu ban son inmediatos y permanentes. Una vez que un jugador es baneado, permanecerá baneado hasta que se levante el ban. Mientras está baneado, un jugador no puede ver ni interactuar con tu escena, no puede chatear en el canal Nearby mientras esté en ella, y los demás jugadores en la escena no pueden verlo.
> 
> Si un jugador sale de los límites de tu escena, ya no se verá afectado por las reglas de ban de tu escena y volverá a ver a los jugadores baneados.


Haz clic **Ver lista de baneados** para ver la lista de jugadores baneados actualmente. Desde esta lista también puedes **Desbanear** jugadores.

## Activar smart items

Para activar una acción de cualquier smart item de la escena:

* Agrega un smart item a tu escena
* Abre la configuración del **Admin Tools** Smart Item en Creator Hub
* En la **Acciones del smart item** sección, agrega el smart item desde la lista desplegable, asígnale un nombre personalizado y selecciona una acción predeterminada

Una vez configurado lo anterior, los admins pueden activar la acción abriendo la **Smart Items** pestaña del panel de admin y seleccionando un elemento de la lista desplegable. Luego pueden elegir una de las acciones del elemento desde el **Actions** desplegable y hacer clic en **Ejecutar acción** para activarla. También hay un botón para **Ocultar/Mostrar** el elemento seleccionado.

<img src="/docs/_remote/078360766f2efe8ad3431ea0.webp" alt="Nombre de la escena" width="300">

También puedes mostrar u ocultar cualquier smart item de esta lista, incluso si no incluye una acción para hacerlo.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scene-editor/operar-en-vivo/scene-admin.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
