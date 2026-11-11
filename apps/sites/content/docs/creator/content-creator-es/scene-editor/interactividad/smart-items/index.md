> Usa smart items en tu escena para añadir interactividad.

# Smart Items - Básico

Algunos de los elementos del catálogo del Scene Editor son **Smart Items**. Los jugadores pueden interactuar con ellos, tienen propiedades configurables y pueden activar acciones en otros smart items. Por ejemplo: puertas que se pueden abrir y cerrar, plataformas que suben y bajan, o botones y palancas que pueden activar otros elementos.



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/z7HF4GR01hE" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=z7HF4GR01hE>">Watch on YouTube</a></p>



Puedes reconocer estos elementos en el explorador del asset pack porque tienen un icono de rayo y un fondo de color diferente.

![](/docs/_remote/4401edd6b0ae269ede4c9467.webp)

Puedes reconocer qué elementos de tu scene son smart porque tienen el icono de rayo junto a ellos en el entity tree.

![](/docs/_remote/d3ff99754b5e412ff6b047ee.webp)

## Uso de elementos

Para usar un smart item, arrástralo a la escena como cualquier otro objeto. Todos los objetos incluyen un comportamiento predeterminado; ejecuta una vista previa de la escena para probarlo.

Aquí hay algunos elementos comunes y sus comportamientos predeterminados:

* **Puertas**: Las puertas se abren o se cierran al hacer clic. Puedes cambiar este comportamiento para que se abran mediante botones, áreas de activación, etc.
* **Botones**: Al hacer clic, reproducen un sonido y una animación como retroalimentación. Añade más acciones a sus eventos de activación para activar otros smart items.
* **Palancas**: Al hacer clic, alternan entre dos estados. Haz que cada posición de la palanca realice acciones diferentes sobre otros smart items.
* **Cofres**: Se comportan como puertas; por defecto se abren o cierran al hacer clic. Puedes colocar objetos más pequeños dentro.
* **Plataformas**: Se mueven entre dos posiciones. Usa sus acciones de Tween para controlar hacia dónde se mueven, su velocidad, etc.
* **Área de activación**: Un elemento invisible que puede activar otros smart items cuando el jugador entra en su área. Véase [Acerca de las áreas de activación](#trigger-areas).
* **Avatar Modifier Area**: Un área invisible que cambia cómo se comportan o aparecen los avatares dentro de ella; por ejemplo, ocultando avatares o deshabilitando ventanas emergentes del pasaporte. Véase [Modifier Areas](/docs/creator/content-creator-es/scene-editor/interactividad/modifier-areas/).
* **Camera Modifier Area**: Un área invisible que fuerza la cámara del jugador a primera o tercera persona mientras está dentro. Véase [Modifier Areas](/docs/creator/content-creator-es/scene-editor/interactividad/modifier-areas/).
* **Video Player**: Una pantalla para mostrar videos o streams en vivo. Véase [Reproducción de videos](#playing-videos).
* **Stream de audio**: Reproduce audio desde un stream en vivo. Véase [Reproducción de streams de audio](#playing-audio-streams)
* **NFT**: Muestra la imagen de un NFT como un retrato. Véase [Visualización de NFTs](#displaying-nfts)
* **Botón de presión de suelo**: Una placa redonda que se hunde mientras un jugador está sobre ella y vuelve a subir cuando se baja. Usa sus **On Press** y **On Release** hooks para ejecutar acciones, por ejemplo para mantener una puerta abierta mientras alguien está sobre ella.
* **Plano de agua** y **Cilindro de agua**: Superficies de agua que salpican cuando un jugador camina o salta dentro de ellas. Véase [Agua](#water).

Todos los smart items se pueden configurar para que se comporten de forma personalizada. Por ejemplo, hasta dónde se mueve una plataforma o qué activa un botón.

## Configurar un objeto

Selecciona un objeto en el Scene Editor para ver todas sus propiedades a la derecha.

La mayoría de los smart items guardan su comportamiento en un [Script component](/docs/creator/content-creator-es/scene-editor/extender-con-codigo/script-component/). Sus ajustes aparecen en el **Script** panel, un campo por parámetro, cada uno con un tooltip que explica qué hace. No necesitas escribir ningún código para usarlos.

Algunos campos típicos que puedes encontrar en muchos objetos son:

* **Texto al pasar el cursor**: Qué texto se muestra en la UI como pista cuando el jugador pasa el cursor sobre el objeto. Por ejemplo, una puerta podría decir "Open"
* **Interacción**: ¿Con qué botón se activa el objeto? En un teclado típico:
  * **Primario** es **E**
  * **Secundario** es **F**
  * **Puntero** es **Botón izquierdo del mouse**
  * **Action3** es la tecla **1**
  * **Action4** es la tecla **2**
  * **Action5** es la tecla **3**
  * **Action6** es la tecla **4**
* **Hooks que reaccionan al objeto**: parámetros donde eliges una acción para ejecutar en un momento determinado. Cuáles tiene un objeto depende de lo que haga. Un botón tiene **onClick**, una palanca tiene **onActivate** y **onDeactivate**, una plataforma móvil tiene **onReachStart** y **onReachEnd**, una campana tiene **onRing**. Deja un hook vacío y no ocurrirá nada en ese momento.

Cada objeto tiene sus propios ajustes específicos, que pueden variar de un objeto a otro.

Algunos objetos conservan el antiguo **Config** panel en su lugar. Las luces, por ejemplo, siguen configurando allí su color e intensidad.

Todos los objetos tienen un **Modo avanzado** que te permite configurar casi cualquier cosa sobre ellos. Esto incluye cosas como qué sonidos se reproducen o en qué dirección se mueve una plataforma. También puedes añadir acciones personalizadas que incluyen todo tipo de cosas, como teletransportar al jugador, reproducir animaciones del avatar, adjuntar un objeto a las manos del jugador, etc. También puedes añadir lógica condicional para activar algo solo en ciertos escenarios. Véase [Smart Items - Advanced](/docs/creator/content-creator-es/scene-editor/interactividad/smart-items-advanced/).

![](/docs/_remote/ff0b8ab0126b0a18e07ec387.webp)

## Llamar una acción en otro objeto

Los smart items pueden activar acciones en otros smart items, para que sucedan cada vez que el objeto se active. Solo selecciona el objeto al que quieres llamar, desde una lista de todos los objetos de la escena, y luego selecciona una acción. Distintos objetos exponen acciones diferentes.

Por ejemplo, aquí hay un botón que abre o cierra una puerta. Cada vez que se presiona el botón, la puerta se abrirá o cerrará.

![](/docs/_remote/564f4fe0ae016962310472b6.webp)

Aquí hay una palanca que abre una puerta cuando se activa y la cierra cuando se desactiva.

![](/docs/_remote/dc29efe9b4e6df7d2b39a714.webp)

Puedes añadir tantas acciones distintas de distintos objetos para que se desencadenen juntas. Solo haz clic **+ Asignar acción**.

Elimina acciones haciendo clic en los tres puntos junto a una acción y seleccionando *Eliminar acción*.

Las acciones que expone un objeto conservan sus nombres familiares, como **Abrir**, **Cerrar**, **Sentarse aquí**, **Encender**, o **Teletransportar**, así que la conexión entre objetos funciona igual que siempre.

También puedes encadenar acciones. Por ejemplo, si el **onActivate** abre una puerta, y el propio hook de esa puerta activa otra cosa, la palanca desencadena toda la cadena de forma indirecta.

Si usas el [modo avanzado](/docs/creator/content-creator-es/scene-editor/interactividad/smart-items-advanced/) también puedes añadir lógica condicional a este tipo de acciones.

## Smart items especiales

Algunos smart items tienen características únicas que los hacen muy útiles para escenarios comunes:

### Áreas de trigger

Usa el smart item Trigger Area para activar una acción cuando el jugador entra en un área.

![](/docs/_remote/9dcff76b0c7616640d8295ac.webp)

Arrástralo, cambia el tamaño de la caja o esfera para cubrir el área que quieras y luego usa los **Cuando un jugador entra…** y **Cuando un jugador sale…** botones en su **Reactions** sección para describir qué debería suceder. El [AI Assistant](/docs/creator/content-creator-es/scene-editor/extender-con-codigo/ai-assistant/) escribe el script por ti. Véase [Área de activación](/docs/creator/content-creator-es/scene-editor/interactividad/trigger-area/) para ver la guía completa.

![](/docs/_remote/83142e91c462787a36ca8b35.webp)

Consulta [Área de activación](/docs/creator/content-creator-es/scene-editor/interactividad/trigger-area/) para más información.

### Paredes invisibles

Una colección de formas invisibles que pueden impedir que los jugadores atraviesen un área o hagan clic a través de ella.

Estas paredes invisibles pueden ser útiles al importar un modelo 3D que no tiene un collider mesh, o cuando quieres crear una pared que no sea visible para el jugador.

Consulta [Colliders](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/colliders/) para más información.

### Área de clic

Un cubo invisible en el que los jugadores pueden hacer clic para activar acciones sobre cualquier otro smart item. Este objeto puede ser habilitado o deshabilitado por cualquier otro smart item; cuando está deshabilitado no se puede hacer clic en él. También puedes establecer el texto que los jugadores ven cuando apuntan su cursor hacia él.

![](/docs/_remote/43e0c40165045ebe199d3264.webp)

### Asientos

Sillas, bancos y otros asientos permiten que un jugador se siente al hacer clic en ellos.

* Un asiento solo se libera cuando el jugador que lo ocupó **se aleja** de él, a más de 1,5 metros.
* En un objeto con varios sitios, como un banco, al hacer clic te sientas en el **sitio libre más cercano**.
* Cuando todos los sitios están ocupados, el texto al pasar el cursor dice **El asiento está ocupado** y hacer clic no hace nada.

### Agua

Dos objetos de la **Materials** categoría te dan agua en la que el jugador puede chapotear:

* **Plano de agua**: una superficie plana de 16 x 16 m. Escálala para cubrir un estanque, un canal o un mar.
* **Cilindro de agua**: una piscina redonda, de 8 m de ancho y 2 m de profundidad.

Ambas salpican cuando un jugador camina o salta dentro de ellas. Puedes configurar:

* **Activado por**: `mi jugador` para que solo tú veas tus propias salpicaduras, o `todos los jugadores` para que todos los avatares salpiquen.
* **Profundidad**: hasta qué profundidad bajo la superficie llega el área de salpicaduras, en metros. Nunca llega más allá del fondo de una piscina.
* **Tamaño de la salpicadura**: el tamaño del efecto de salpicadura, en metros.
* **Sonido**: el nombre de un archivo de audio dentro de la carpeta del objeto, que se reproduce en cada salpicadura. Déjalo vacío para que el agua sea silenciosa.

También exponen **entrada** y **salida** eventos, y una **Salpicadura** acción que crea una salpicadura en el centro del agua bajo demanda.


> [!WARNING]
> **📔 Nota**: Cada capa de agua necesita su normal map. Una capa sin uno ya no se anima y, si no queda ninguna, el área de salpicaduras se reduce a 1 x 1 m.


### Teletransportes

El **Teletransportar** el objeto envía al jugador a otro lugar cuando interactúa con él.

Configura los **x** y **y** parámetros para enviarlos a un parcel en Genesis City. Para enviarlos a un [World](/docs/creator/content-creator-es/scenes-sdk7/publicacion/publishing-options/#decentraland-worlds) en su lugar, completa el **world** parámetro world con el nombre del World, como `myname.dcl.eth`, y deja las coordenadas vacías.


> [!WARNING]
> **📔 Nota**: Antes existía un **World Teleport** objeto. Ha sido eliminado. Usa el **Teletransportar** objeto con su **world** parámetro en su lugar.


### Configuración de locomoción

El **Ajustes de locomoción** el objeto cambia qué tan rápido se mueven los jugadores y qué tan alto saltan mientras están dentro de tu escena. Úsalo para darle una sensación flotante a una escena, ralentizar a los jugadores en un museo o acelerarlos en un mapa grande.

Arrástralo a cualquier lugar de tu escena. Aplica sus valores cuando la escena comienza, y su cubo de marcador de posición solo es visible en el editor, nunca para los jugadores.

Cada campo es un deslizador, preconfigurado con el valor predeterminado de Decentraland:

| Campo                 | Predeterminado | Rango  |
| --------------------- | -------------- | ------ |
| `walkSpeed`           | 1.5 m/s        | 0 a 10 |
| `jogSpeed`            | 8 m/s          | 0 a 30 |
| `runSpeed`            | 10 m/s         | 0 a 50 |
| `jumpHeight`          | 1 m            | 0 a 10 |
| `runJumpHeight`       | 1.5 m          | 0 a 10 |
| `doubleJumpHeight`    | 2 m            | 0 a 15 |
| `glidingSpeed`        | 6 m/s          | 0 a 30 |
| `glidingFallingSpeed` | 1 m/s          | 0 a 10 |
| `hardLandingCooldown` | 0.75 s         | 0 a 5  |

El trote es la velocidad a la que los jugadores se mueven por defecto. Caminar y correr es lo que obtienen al mantener pulsadas las teclas Control o Shift.

El objeto también expone dos acciones que otros smart items pueden llamar:

* `aplicar`: vuelve a aplicar al jugador los valores actuales de los campos, por ejemplo después de un Trigger Area o un botón.
* `restoreDefaults`: borra tus valores, para que el jugador vuelva a los valores predeterminados de Decentraland.

Consulta [Ajustes y restricciones de locomoción](/docs/creator/content-creator-es/scenes-sdk7/interactividad/avatares/locomotion/) para ver en detalle qué hace cada ajuste y cómo cambiar estos valores desde código.

### Reproducción de videos

Reproduce videos desde cualquiera de estos orígenes:

* **Archivos locales**
* **Emitir desde una URL**
* **Transmitir en vivo desde** [**Decentraland Cast**](/docs/creator/content-creator-es/scene-editor/operar-en-vivo/live-streaming/#dcl-cast-easy)
* **Transmitir en vivo desde** [**Software RTMP**](/docs/creator/content-creator-es/scene-editor/operar-en-vivo/live-streaming/#stream-advanced) **(OBS, XSplit, StreamYard, etc.)**


> [!WARNING]
> **📔 Nota**: Reproducir videos exige mucho rendimiento, así que mantén bajo el número de videos que se reproducen al mismo tiempo. El engine limita cuántos videos pueden reproducirse simultáneamente según los ajustes de calidad de cada jugador (1 en baja, 5 en media, 10 en alta), y pausa cualquier video que supere ese límite. Véase [Reproducir videos](/docs/creator/content-creator-es/scene-editor/interactividad/video-screen/#play-videos).


Consulta [Reproducir videos](/docs/creator/content-creator-es/scene-editor/interactividad/video-screen/) para más información.

### Reproducción de streams de audio

Reproduce un stream de audio desde una URL usando el **Stream de audio** smart item.


> [!NOTE]
> **📔 Nota**: No todos los servicios de streaming permiten reproducir su audio fuera de su sitio. Los siguientes son algunos ejemplos que funcionan en Decentraland:
> 
> ```ts
> GRAFFITI = 'https://n07.radiojar.com/2qm1fc5kb.m4a?1617129761=&rj-tok=AAABeIR7VqwAilDFeUM39SDjmw&rj-ttl=5'
> SIGNS = 'https://edge.singsingmusic.net/MC2.mp3'
> DELTA = 'https://cdn.instream.audio/:9069/stream?_=171cd6c2b6e'
> JAZZ = 'https://live.vegascity.fm/radio/8010/the_flamingos.mp3'
> ```
> 


Puedes ajustar el volumen de tu stream. Ten en cuenta que el audio del stream no es posicional; se escucha con un volumen uniforme en toda tu escena.

### Visualización de NFTs

Para mostrar un NFT en un marco de fotos, usa el **NFT** smart item. Debes proporcionar los siguientes campos:

* Red


> [!NOTE]
> **📔 Nota**: Actualmente **ethereum** es la única red compatible.


* NFT Collection Contract: el smart contract de la colección de NFTs.
* Token ID: el ID del token de este NFT en particular.

<img src="/docs/_remote/9bae5521b60927140f632d59.webp" alt="Forma del NFT" width="400">

Puedes obtener esta información en [OpenSea](https://opensea.io), consultando la **Detalles** pestaña debajo de la imagen del NFT.

<img src="/docs/_remote/2ef80788a2ffef0f4c7b360b.webp" alt="Detalles de OpenSea" width="400">


> [!NOTE]
> **📔 Nota**: También puedes obtener esta información a partir de la URL de OpenSea. Por ejemplo, si la URL del NFT es la siguiente:
> 
> > `https://opensea.io/assets/ethereum/0x32b7495895264ac9d0b12d32afd435453458b1c6/1956`
> 
> Puedes completar los campos con lo siguiente:
> 
> * Red: ethereum
> * Contrato: 0x32b7495895264ac9d0b12d32afd435453458b1c6
> * Token: 1956


También puedes configurar un color de fondo; esto es especialmente útil para NFTs con un fondo transparente.

También puedes elegir un **estilo de marco**, para enmarcar el NFT con una variedad de estilos distintos, clásicos y modernos.

Consulta [NFTs](/docs/creator/content-creator-es/scene-editor/construir/nfts/) para más detalles.

### Barras de salud

![](/docs/_remote/4b7b9bba4ad2a27559774acf.webp)

El **Barra de salud** smart item es una excelente base para varias mecánicas de juego. Se puede usar de varias maneras:

* Anídalo debajo del **Player** para mostrar la salud del jugador sobre el avatar

  ![](/docs/_remote/7ec4155f634538c1243f6333.webp)
* Anídalo debajo del **Camera** para mostrarla fijada en la UI

  ![](/docs/_remote/8f666b7f68658ee843a4304c.webp)
* Anídalo debajo de literalmente cualquier objeto de la escena para hacer un seguimiento de la salud de ese objeto

  ![](/docs/_remote/1bc82b628c58c582bc684876.webp)

Otros objetos pueden interactuar con la barra de salud para añadirle o restarle salud.

* Objetos como **Spikes** o **Enemigo robot** pueden reducir la salud

  ![](/docs/_remote/8a3aa562c2ae8a2e3b67d7c6.webp)
* objetos como **Primeros auxilios** o el **Plataforma de curación** pueden restaurarla.

  ![](/docs/_remote/4b71eac34757d0b4ca625eca.webp)

Debes configurar la Barra de salud para definir qué sucederá cuando la salud sea 0. Podrías reaparecer al jugador en la posición de una **Plataforma de reaparición** smart item, reiniciar el contador de su puntuación, reaparecer a cualquier enemigo, mostrar un texto en la UI o cualquier otra cosa que tenga sentido en la lógica de tu juego.

También puedes activar acciones cuando la salud sea inferior a cierto valor; por ejemplo, reproducir música especial o mostrar una pista en la UI cuando la salud sea menor que 3.

Las barras de salud se pueden configurar para afectar a cualquier cosa. Por ejemplo, añade una barra de salud anidada debajo de la **Puerta de madera** smart item. Esta barra puede ver reducida su salud por el jugador usando el **Espada** smart item, pero también por la explosión de un **Barril** o el ataque del **Enemigo robot**. Para que esto funcione, configura la barra de salud para que ejecute una acción sobre su objeto padre cuando su valor sea 0.

![](/docs/_remote/03f177d0727bf4a6bb82243f.webp)

Armas como la **Espada** pueden ser recogidas por el jugador y luego usarse para causar daño a cualquier otro objeto con una barra de salud que esté cerca del jugador al realizar la acción.

## Multiplayer

Casi todos los smart items tienen comportamiento multijugador, de modo que todos los jugadores de la escena comparten la misma experiencia a medida que los objetos cambian de estado. Si el jugador A abre una puerta, el jugador B también ve esa puerta abierta. Si luego el jugador C entra en la escena mientras los otros jugadores siguen allí, ella también verá la puerta ya abierta.

Sin embargo, si no hay jugadores cerca de la escena, esta vuelve a su estado predeterminado. Así que, si todos los jugadores se van, pero luego el jugador A regresa, encontrará la puerta cerrada (si ese era el estado predeterminado de la puerta).

Asegúrate de diseñar tu escena de forma que las acciones de un jugador no arruinen la escena para otros que lleguen después. Por ejemplo, si la escena es un juego de rompecabezas, puedes usar una *retraso* acción en un *tools* smart item para hacer que todos los objetos de la escena se restablezcan a su estado inicial unos segundos después de que se resuelva el rompecabezas.

También puedes deshabilitar el comportamiento multijugador de un objeto; véase [Smart Items - Advanced](/docs/creator/content-creator-es/scene-editor/interactividad/smart-items-advanced/).

## Solución de problemas

* *Un objeto de mi escena debería poder hacerse clic, pero no se puede hacer clic en él*.

Asegúrate de que no esté siendo obstruido por otra cosa. No puedes hacer clic a través de otros objetos. Algunos objetos tienen un *collider mesh* que tiene una geometría simplificada que puede estar obstruyendo tu objeto, aunque su forma visible no parezca hacerlo. Intenta mover el objeto para ver qué pasa.

## Ver también

* [Smart items - Advanced](/docs/creator/content-creator-es/scene-editor/interactividad/smart-items-advanced/)
* [Estados y condiciones](/docs/creator/content-creator-es/scene-editor/interactividad/states-and-conditions/)
* [Hacer que cualquier objeto sea smart](/docs/creator/content-creator-es/scene-editor/interactividad/make-any-item-smart/)
* [Combinar con código](/docs/creator/content-creator-es/scene-editor/extender-con-codigo/overview/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scene-editor/interactividad/smart-items.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
