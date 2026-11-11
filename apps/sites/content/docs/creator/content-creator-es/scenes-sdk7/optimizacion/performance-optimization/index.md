> Optimiza tu escena para que cargue rápido y funcione sin problemas para todos los jugadores.

# Optimización del rendimiento

Hay varios aspectos que puedes optimizar en tus escenas para garantizar la mejor experiencia posible para los jugadores que las visitan. Este documento cubre algunas prácticas recomendadas que pueden marcar una gran diferencia en la velocidad con la que carga tu escena y en lo fluido que se ejecuta para los jugadores que están en ella o en escenas vecinas.

Ten en cuenta que muchos jugadores pueden estar visitando Decentraland usando hardware que no está diseñado para gaming, a través del navegador, o desde el [mobile app](/docs/creator/content-creator-es/desarrollar-para-movil/cliente-movil/overview/) en un teléfono — todo lo cual limita cuánta capacidad de procesamiento está disponible para tu escena. La experiencia de visitar tu escena debería ser fluida para todos.



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/tc1PwYKW1Kc" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=tc1PwYKW1Kc>">Watch on YouTube</a></p>



Consulta [Recursos útiles](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/useful-resources/) para herramientas que pueden ayudar, como Decentraland Scene Optimizer, que extrae, deduplica y comprime texturas de los modelos 3D de tu escena.


> [!NOTE]
> **📱 Móvil**: Los dispositivos móviles suelen ser el cliente con más restricciones de recursos. Si tu escena está dirigida a jugadores móviles, consulta también [Compilación para móviles](/docs/creator/content-creator-es/desarrollar-para-movil/cliente-movil/overview/) para obtener orientación específica para móviles.


El Explorer de Decentraland aplica muchas optimizaciones a nivel de Engine. Estas optimizaciones marcan una gran diferencia, pero el desafío de renderizar simultáneamente múltiples experiencias generadas por usuarios en un navegador es enorme. Necesitamos tu ayuda para que todo funcione sin problemas.

## Tiempo

### Video playing

Reproducir videos es una de las cosas más costosas para el Engine. Si tu escena incluye videos, asegúrate de que solo *UN* VideoTexture esté en uso a la vez. Puedes tener docenas de planos compartiendo el mismo VideoTexture sin un impacto significativo en el rendimiento, pero en cuanto añades un segundo VideoTexture, sus efectos en la tasa de fotogramas se vuelven muy notorios.

También deberías evitar tener videos reproduciéndose en regiones donde no se puedan ver. Por ejemplo, si tienes una pantalla en interiores, activa o desactiva el video usando una área de activación según cuándo el jugador entra y sale.


> [!NOTE]
> **💡 Consejo**: Un truco que han usado varias escenas es transmitir un único video con múltiples regiones que se asignan de forma diferente a distintos planos. Cada pantalla de video usa [mapeo UV](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/materials/#using-textures) para mostrar solo una parte distinta del VideoTexture. Gracias a esto, puede parecer que se están reproduciendo videos separados sin el costo de múltiples VideoTextures.



> [!NOTE]
> **💡 Consejo**: Cuando los jugadores están fuera de tu escena, los VideoTextures no se actualizan en cada fotograma. Esto ayuda a reducir el impacto en las escenas vecinas. Aun así, lo ideal es activar la reproducción de cualquier video solo cuando los jugadores [entren en tu escena](/docs/creator/content-creator-es/scenes-sdk7/interactividad/event-listeners/#player-enters-or-leaves-scene) .


### carga diferida

Si tu escena es grande, o tiene áreas interiores que no siempre son visibles, puedes elegir no cargar todo el conjunto de entities desde el principio. En su lugar, carga el contenido por región a medida que el jugador visita distintas partes de la escena. Esto puede reducir significativamente el tiempo de carga de la escena, y también la cantidad de texturas y contenido 3D que el Engine tiene que manejar en cada fotograma.

Por ejemplo, el edificio principal de un museo podría cargarse desde el inicio, pero los cuadros de cada piso solo se cargarían para cada jugador a medida que visita cada piso.

Consulta [esta escena de ejemplo](https://github.com/decentraland-scenes/lazy-loading) para ver cómo podría funcionar.

Para obtener el mejor resultado en términos de evitar tirones, oculta las entities cambiando la propiedad `visible` de su shape a false. Con este enfoque, las agregas al Engine al crearlas, pero simplemente no haces visibles sus modelos.

Otra alternativa es no añadir las entities al Engine hasta que sean necesarias. Esto puede provocar algunos tirones cuando las entities aparecen por primera vez, y también podrían tardar un par de segundos en hacerse visibles. La ventaja de este enfoque es que es una forma válida de sortear las [limitaciones de la escena](/docs/creator/content-creator-es/scenes-sdk7/optimizacion/scene-limitations/)limitaciones. Ten en cuenta que el conteo de límites de la escena corresponde al contenido que se está renderizando en la escena en un momento dado, no al contenido total que podría renderizarse. Cargar y descargar partes de la escena debería permitirte sortear esas limitaciones.


> [!WARNING]
> **📔 Nota**: Las entities que no son visibles pero están añadidas al Engine sí cuentan para las limitaciones de la escena.


También puedes activar o desactivar las animaciones para entities que estén lejos u ocultas. Por ejemplo, para un NPC que reproduce una animación idle muy sutil, podrías hacer que solo reproduzca esa animación cuando el jugador esté a menos de 20 metros. Usa un área de activación alrededor del NPC y activa o desactiva sus animaciones según corresponda.


> [!NOTE]
> **💡 Consejo**: Cuando una entity está lejos y es lo suficientemente pequeña, el Engine la descarta. Este descarte ayuda a nivel de draw call, pero eliminar entities del Engine siempre es mejor. Este descarte tampoco tiene en cuenta la oclusión por otras entities, así que las entities que no son tan pequeñas pero están ocultas por una pared siguen renderizándose.


### Bloques asíncronos

Bloques de [código asíncrono](/docs/creator/content-creator-es/scenes-sdk7/patrones-de-programacion/async-functions/) no bloquean el progreso de todo lo demás mientras esperan: la escena sigue ejecutándose, y el bloque asíncrono se reanuda cuando llega la respuesta que esperaba. Ten en cuenta que las escenas se ejecutan en un solo hilo, así que esto no es procesamiento en paralelo; solo evita que la escena se detenga mientras espera respuestas externas.

Cualquier proceso que dependa de respuestas de servicios asíncronos, como `getPlayerData()` o `getRealm()` debe ejecutarse siempre en bloques asíncronos, ya que de lo contrario bloquea el resto de la carga de la escena mientras espera una respuesta. Lo mismo aplica a cualquier llamada a servidores de terceros.

Ten en cuenta que la escena se considerará completamente cargada cuando todo lo que no sea asíncrono haya terminado. Es posible que los procesos asíncronos sigan ejecutándose cuando el jugador entre en la escena. Evita situaciones en las que un proceso asíncrono resulte en la carga de una entity que potencialmente pueda dejar al jugador atrapado dentro de su geometría.

### Confía en Events

Intenta que la lógica de la escena dependa de escuchar [events](/docs/creator/content-creator-es/scenes-sdk7/interactividad/event-listeners/) tantos como sea posible, en lugar de ejecutar comprobaciones en cada fotograma.

El `update()` function en un [system](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/systems/) se ejecuta en cada fotograma, 30 veces por segundo (idealmente). Evita hacer comprobaciones recurrentes si en su lugar puedes suscribirte a un evento.

Por ejemplo, en lugar de comprobar constantemente los wearables del jugador, puedes suscribirte al `onProfileChanged` evento, y comprobar los wearables del jugador solo cuando hayan cambiado.

Si debes usar un system, evita hacer comprobaciones o ajustes en cada fotograma. Puedes incluir un temporizador como parte de la función update y ejecutar la comprobación solo una vez por cada segundo completo, o por el período que tenga sentido.

## Optimizar modelos 3D

Hay varias formas en que tus modelos 3D pueden optimizarse para ser más ligeros.

Cuando trabajas con el [Creator Hub](/docs/creator/content-creator-es/scene-editor/primeros-pasos/editor-installation/), puedes ver estadísticas sobre los recursos usados por los modelos 3D en tu escena, y si superan alguno de los [limitaciones de la escena](/docs/creator/content-creator-es/scenes-sdk7/optimizacion/scene-limitations/).

![](/docs/_remote/ab87e37107598ee524831639.webp)

Puedes expandir este menú para ver los detalles.

![](/docs/_remote/8d4619b1778683c8d72d3c61.webp)

Aquí tienes algunos consejos para mejorar estas métricas:

* Cuando sea posible, comparte texturas entre modelos 3D. Una buena práctica es usar una sola textura como atlas, compartida por todos los modelos de la escena. Es mejor tener una única textura compartida grande de 1024x1024 píxeles que varias pequeñas.

  > Nota: Evita usar el mismo archivo de imagen tanto para la textura albedo como para el normal map o el emissive map de un material. Usa archivos separados, incluso si son idénticos. Asignar el mismo archivo de imagen a distintos tipos de propiedades de textura puede introducir artefactos visuales no deseados cuando se comprime a asset bundles.
* *.glb* es un formato comprimido, siempre pesará menos que un *.gltf*. Por otro lado, con *.gltf* es fácil compartir imágenes de textura exportando las texturas como un archivo aparte. Puedes obtener lo mejor de ambos mundos usando el [siguiente pipeline](https://github.com/AnalyticalGraphicsInc/gltf-pipeline), que te permite tener *.glb* modelos con archivos de textura externos.
* Evita usar transparencias combinadas. Las transparencias combinadas tienen que saltarse bastantes de las optimizaciones de renderizado. Si es posible, favorece la geometría opaca o con alpha test.
* Evita las skinned meshes. Pueden degradar significativamente el rendimiento.


> [!NOTE]
> **💡 Consejo**: Lee más sobre las mejores prácticas para modelos 3D en la \[sección de Modelado 3D]\(/creator/3d-modeling/3d-models


### Rescata un modelo pesado en Blender

Cuando las estadísticas señalan un modelo específico, normalmente es más rápido arreglar el modelo que sortearlo en código. Las modificaciones más útiles, en Blender o en una herramienta similar:

* **Reducir el conteo de triángulos** con el modificador Decimate. Esta es una herramienta de rescate para modelos que ya tienes. Los modelos nuevos es mejor modelarlos con low-poly desde el inicio.
* **Redimensionar texturas** a una potencia de dos, 1024x1024 o menos, y volver a empaquetarlas en el *.glb*.
* **Eliminar faces que el jugador nunca ve**, como las partes inferiores y las espaldas de props. Activa [back-face culling](#backface-culling) en lugar de duplicar la geometría.
* **Fusionar materiales** en uno, usando una sola textura atlas en lugar de una textura por parte.
* **Quita luces, cámaras y materiales de `_collider` meshes.** El Engine las ignora, así que son peso puro.

Comprueba el conteo de triángulos y el conteo de materiales antes y después, luego vuelve a exportar sobre el mismo archivo. Si hay un preview en ejecución, se recarga con tus cambios.

### Reutilizar el mismo modelo muchas veces

Las escenas suelen estar llenas de contenido repetido: postes de luz a lo largo de una calle, sillas en una habitación, árboles en un parque. La mejor manera de construirlos es la más simple — **darle a cada copia su propia entity, y apuntarlas todas al mismo&#x20;*****.glb*****&#x20;.**

```ts
// BIEN: un archivo, muchas entities
for (const position of lampPostPositions) {
  const lampPost = engine.addEntity()
  Transform.create(lampPost, { position })
  GltfContainer.create(lampPost, { src: 'assets/scene/lampPost.glb' })
}
```

El Engine reconoce que estas entities comparten una fuente. El archivo se descarga una vez, se convierte a asset bundle una vez, y sus meshes y textures se mantienen en memoria una vez — el vigésimo poste de luz cuesta casi nada más allá de su propia posición en el mundo. Esto también funciona entre escenas: si una escena vecina usa el mismo archivo, ya está en memoria. Exportar veinte archivos casi idénticos en su lugar significa veinte descargas y veinte copias en memoria. *.glb* archivos en su lugar significa veinte descargas y veinte copias en memoria.

Lo que esto **no** ahorra son las draw calls. Veinte postes de luz son veinte objetos que dibujar, tanto si provienen de veinte entities como de un solo *.glb* que contiene veinte postes de luz modelados en él. La única forma de reducir ese conteo es unir las meshes en tu herramienta de modelado, lo cual es un verdadero intercambio — consulta [Instancing vs Duplicating vs Merging](/docs/creator/content-creator-es/modelado-y-animaciones-3d/meshes/#instancing-objects-vs-duplicating-objects) para saber cuándo vale la pena.

Algunos consejos relacionados:

* **Agrupa en clústeres en lugar de usar un modelo gigante.** Si tienes muchos props dispersos, un buen punto intermedio es uno *.glb* por grupo — un bloque de una calle, el mobiliario de una habitación — en lugar de un modelo por prop o un modelo para toda la escena. Reduces la cantidad de objetos y mantienes cada clúster lo bastante pequeño como para que el culling siga siendo útil.
* **¿Vas a crear muchas copias a la vez?** Precarga primero el modelo con el [`AssetLoad` component](/docs/creator/content-creator-es/scenes-sdk7/optimizacion/pre-load-resources/). Las copias se crean entonces a partir de un modelo que ya está en memoria, en lugar de que cada una espere la misma descarga. Construir cada copia sigue requiriendo trabajo, así que una ráfaga grande puede costarte un fotograma de todas formas — la precarga elimina la espera del archivo, no el costo de crear las copias.
* **Un modelo muy grande es más exigente para el Engine que muchos pequeños.** El Engine reparte el trabajo de construir tu escena a lo largo de varios fotogramas para evitar tirones, pero no puede dividir un único modelo enorme — eso cae en un solo fotograma. Muchas piezas pequeñas cargan sin problemas; una pieza monolítica es más propensa a causar un tirón visible.
* **Repetir un modelo no introduce nuevas texturas ni geometría.** Veinte postes de luz construidos a partir de un modelo comparten las meshes y textures de ese modelo en lugar de añadirlas al conjunto — el vigésimo no cuesta nada en memoria de texturas. Los materiales son la excepción: el Engine crea una instancia de material por objeto renderizado para aplicar el recorte de límites de tu escena, así que el conteo de materiales sigue el número de objetos que renderizas, no el número de modelos que usas. Esas instancias siguen compartiendo las mismas texturas y variantes de shader, así que cuestan un poco de memoria en lugar de tiempo de fotograma. Lo que acerca una escena al [límite](/docs/creator/content-creator-es/scenes-sdk7/optimizacion/scene-limitations/) es tener muchos *diferentes* modelos, cada uno con su propio conjunto hecho a medida — así que consolidar tu escena alrededor de una biblioteca más pequeña de modelos reutilizados ayuda en eso.

### Backface Culling

Para optimizar el rendimiento, Backface Culling se establecerá en **On** en **todos** de los materiales del modelo una vez renderizados en el Engine, independientemente de sus configuraciones.

Si esperas ver la cara posterior o el interior de tus modelos, duplica las faces e invierte las normals.

#### Solución de problemas

Para verificar si una escena tiene problemas de Backface Culling en los materiales, sigue estos pasos:

1. Abre el `debug` panel en la escena.

* Si la escena está publicada, escribe `/debug` comando en el chat.
* Si estás en modo Preview en la escena, haz clic en el icono de Bug (

  <img src="/docs/_remote/3a2d6a1437dc36383f2065db.webp" alt="icono de debug" width="32">

  ) ubicado en la esquina superior derecha de la pantalla.

2. El panel de debug aparecerá en la esquina inferior derecha de la pantalla.
3. En **Escena actual**, haz clic en el **Depurador de Backface** botón.

![](/docs/_remote/1e0c76604bd561268516ee00.webp)

4. Alternar **Forzar Backface Culling**: Muestra los materiales renderizados con Backface Culling activado. Este es el renderizado real una vez que la Optimización está en producción. Activa y desactiva para detectar materiales que necesitan corrección.
5. Alterna el **Depurador de Backface** para detectar fácilmente los materiales que tienen Backface Culling desactivado. Resalta:

* **Rojo**: Materiales que no tienen Backface Culling establecido en **On**.
* **Verde**: Materiales con Backface Culling **On**.

<p align="center"><img src="/docs/_remote/0c934b5889f4f369d893365d.webp" alt=""> <img src="/docs/_remote/d2530a38414758e20fb9ebd0.webp" alt=""><br><em>Material con Backface Culling desactivado (izquierda) vs. Backface Culling activado (derecha)</em></p>

### Conversión a Asset Bundle

Cada vez que publicas una escena, los servidores de contenido de Decentraland comprimen cada *.gltf* y *.glb* modelo que contiene al formato asset bundle. Este formato es *significativamente* más ligero, lo que hace que las escenas carguen mucho más rápido y se ejecuten de manera más fluida. La conversión comienza inmediatamente después de cada deployment. Por lo general solo tarda unos segundos, aunque puede tardar más en escenas muy grandes o cuando los servidores están ocupados, ya que la conversión se ejecuta por separado para cada plataforma y puede quedar en cola detrás de otras escenas. Puedes [comprobar el estado de la conversión](/docs/creator/content-creator-es/scene-editor/publicar/publish-scene/#check-the-conversion-status) de tu escena directamente.

También puedes ejecutar esta misma conversión localmente mientras desarrollas tu escena. Esto hace que tu preview sea más fluido y te permite comprobar si hay algún problema con los modelos comprimidos antes de publicar. Consulta [Previsualizar con assets optimizados](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/preview-scene/#preview-with-optimized-assets).


> [!NOTE]
> **💡 Consejo**: Cuando planifiques un evento en Decentraland, publica tu versión final al menos una hora antes, para que para entonces todos los modelos ya estén convertidos a asset bundles y tengas tiempo de corregir cualquier cosa inesperada. Si no quieres arruinar la sorpresa antes del evento, puedes desplegar una versión de tu escena que incluya todos los modelos 3D finales en la carpeta del proyecto, pero donde estos no sean visibles o donde su tamaño esté establecido en 0.



> [!WARNING]
> **📔 Nota**: Si haces *cualquier* cualquier cambio en un archivo de modelo 3D, incluso si solo es un cambio de nombre, se considerará un archivo nuevo y deberá convertirse de nuevo al formato asset bundle.


## Conectividad

Si tu escena se conecta a algún servidor de terceros o usa el [messagebus](/docs/creator/content-creator-es/scenes-sdk7/networking/serverless-multiplayer/#send-explicit-messagebus-messages) para enviar mensajes entre jugadores, también hay algunas cosas que quizá quieras tener en cuenta.

* Tu escena solo debería tener una conexión WebSockets activa a la vez.
* Las llamadas HTTP son canalizadas por el Engine de modo que solo una se gestiona a la vez. Cualquier solicitud adicional se pone en cola internamente y debe esperar hasta que se completen las otras solicitudes. Este proceso de cola se maneja automáticamente, no necesitas hacer nada.
* Cuando usas el [messagebus](/docs/creator/content-creator-es/scenes-sdk7/networking/serverless-multiplayer/#send-explicit-messagebus-messages) para enviar mensajes entre jugadores, ten en cuenta que todos los mensajes se envían a todos los demás jugadores de la isla del servidor. Evita situaciones en las que un mensaje entrante resulte directamente en el envío de otro mensaje, ya que la cantidad de mensajes puede crecer rápidamente de forma exponencial cuando hay una multitud en la escena.

## UI de la escena

Las UIs de la escena pueden volverse costosas de renderizar cuando están compuestas por muchos elementos individuales. Ten en cuenta que cada elemento de UI requiere una draw call separada en el Engine.


> [!NOTE]
> **💡 Consejo**: Intenta fusionar varios elementos en una sola imagen. Por ejemplo, si tienes un menú con varios elementos de texto, lo ideal es tener el texto de los tiles y cualquier imagen adicional incrustados en la imagen de fondo. Eso evita que el Engine haga una draw call adicional por fotograma por cada elemento de texto.


Evita hacer ajustes a la UI en cada fotograma; eso es especialmente costoso y puede acabar quedando en cola. Por ejemplo, si hay una barra de salud en tu UI que debería reducirse con el paso del tiempo, probablemente los jugadores no notarían la diferencia entre que se actualice a 10 FPS en lugar de a 30 FPS (en cada fotograma). El system que actualiza esa barra puede usar un temporizador breve que cuente 100 milisegundos y solo afectar a la UI cuando ese temporizador llegue a 0.

Evita tener muchos elementos de UI ocultos; estos también afectan al rendimiento aunque no se estén renderizando. Cuando sea posible, intenta crear componentes de UI bajo demanda.

## El terreno del paisaje en Worlds

Las scenes publicadas en un [World de Decentraland](/docs/creator/content-creator-es/scenes-sdk7/publicacion/publishing-options/#decentraland-worlds) están rodeados por un paisaje autogenerado de praderas, árboles y mar. Renderizar este paisaje consume parte del presupuesto de renderizado del jugador. Si tu escena no lo necesita, puedes [desactivar el terreno del paisaje](/docs/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/scene-metadata/#landscape-terrain) en tu `scene.json` para liberar esos recursos para el contenido propio de tu escena.

## Monitorear el rendimiento

La mejor métrica para saber qué tan bien está funcionando una escena es el FPS (Frames Per Second). En preview, puedes ver el FPS actual de la escena en el panel de debug. Debes procurar tener siempre 30 FPS o más.

En la escena desplegada, puedes alternar el panel que muestra estas métricas escribiendo `/showfps` en la ventana del chat.

### Panel de Scene Stats

Cuando ejecutas una escena en preview en el cliente de escritorio, puedes abrir un panel que muestra estadísticas en vivo sobre el contenido de tu escena. Haz clic en el icono de estadísticas en la esquina superior derecha de la pantalla, junto a los iconos de console y debug.

<img src="/docs/_remote/96e07557afec5b2ade6528d5.webp" alt="Panel de Scene Stats" width="400">

El panel cuenta los triángulos, entities, bodies y texturas de la escena frente a sus límites, mostrando qué porcentaje de cada presupuesto se está usando. Un valor se vuelve naranja cuando alcanza el 80% de su límite. También muestra conteos simples de materials, geometries, colliders y videos o audios externos. Pasa el cursor sobre el icono de información junto a cada métrica para ver una explicación de lo que cuenta y por qué importa.

Los números se actualizan en tiempo real a medida que el contenido se carga y se descarga, lo que hace que el panel sea útil para comprobar cómo estrategias como [carga diferida](#lazy-loading) afectan a lo que el Engine está manejando realmente en cada momento. Consulta [limitaciones de la escena](/docs/creator/content-creator-es/scenes-sdk7/optimizacion/scene-limitations/) para obtener detalles sobre cada límite. Ten en cuenta que estos son límites suaves: superar uno no bloqueará tu escena, pero puede degradar el rendimiento para los jugadores.

### Mensajes entre la escena y el Engine

Uno de los principales cuellos de botella en el rendimiento de una escena suele ser el envío de mensajes entre el código de la escena y el Engine.

Cuando ejecutas una escena en preview, nota que en la esquina superior derecha dice “Y = Toggle Panel”. Pulsa Y en el teclado para abrir un panel con información útil que se actualiza en tiempo real.

A medida que interactúas con cosas que implican mensajes entre el SDK y el Engine, verás que aumenta el número de “Processed Messages”. Debes vigilar de cerca el número de “Pending on Queue”; debería estar siempre en 0 o cerca de 0. Esto te indica cuántos de esos mensajes no llegaron a procesarse y se pusieron en una cola. Si el conteo de “Pending on Queue” empieza a crecer, entonces has entrado en la zona de peligro y deberías pensar en hacer más optimizaciones a tu escena.


> [!WARNING]
> **📔 Nota**: No mantengas el panel abierto cuando no lo estés usando, ya que tiene un impacto negativo en el rendimiento.


Ten en cuenta que el rendimiento que experimentas en preview puede diferir del de producción:

* Las escenas vecinas alrededor pueden tener un impacto negativo
* La compresión de los modelos 3D de las escenas a asset bundles puede tener un impacto positivo
* Algunos jugadores que visitan tu escena pueden estar ejecutándose en hardware menos potente

Siempre es una buena práctica intentar desplegar primero tu escena en un [World de Decentraland](/docs/creator/content-creator-es/scenes-sdk7/publicacion/publishing-options/#decentraland-worlds) para hacer pruebas más exhaustivas.

Pide siempre feedback a los jugadores. Nunca des por sentado que la forma en que tú experimentas la escena es la misma para todos los demás.

## Optimiza con IA

Un agente de IA puede encargarse de gran parte de la medición y las correcciones descritas en esta página por ti. Decentraland proporciona [habilidades de IA](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/vibe-coding/#install-skills-for-any-ai-agent) que enseñan a agentes de codificación (como Claude Code o Cursor) cómo optimizar escenas, y el cliente de escritorio incluye un [MCP server](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/vibe-coding/#let-the-ai-see-your-scene-in-world) que permite al agente inspeccionar y medir directamente tu escena en ejecución.

Con los `optimize-scene` y `unity-explorer-mcp` skills instalados y el servidor MCP conectado, un agente puede:

* **Comprobar la escena frente a sus presupuestos de contenido**: lee las mismas estadísticas en vivo que el panel Scene Stats y las compara con los [limitaciones de la escena](/docs/creator/content-creator-es/scenes-sdk7/optimizacion/scene-limitations/), así que puede decirte qué presupuesto está más ajustado.
* **Encontrar los assets responsables**: clasifica los modelos 3D de tu escena según cuántos triángulos, materials y draw calls contribuye cada uno, incluyendo lo que es visible desde un punto de vista específico, para que sepa exactamente qué modelos optimizar primero.
* **Medir la tasa de fotogramas real**: coloca al jugador en cualquier punto de la escena y muestrea el FPS que un jugador realmente experimenta allí, incluyendo los tirones momentáneos.
* **Verificar su propio trabajo**: después de cada cambio puede volver a medir, de modo que sus recomendaciones y correcciones estén respaldadas por números reales en lugar de suposiciones.

Esto convierte la optimización en algo que simplemente puedes pedir:

> "Comprueba si mi escena está dentro de los límites, encuentra lo que esté arrastrando hacia abajo la tasa de fotogramas cerca del punto de aparición, corrígelo y muéstrame los números antes y después."

### Combínalo con el servidor MCP de Blender

Las herramientas de Decentraland le dicen al agente cuáles son los modelos más pesados, pero no editan los modelos en sí. Para ir más allá, puedes intentar combinarlas con el [servidor MCP de Blender](https://www.blender.org/lab/mcp-server/), que le da al agente acceso a las herramientas de edición de Blender. Esto permite al agente abrir un modelo pesado en Blender, reducir su conteo de triángulos, fusionar sus materiales o reducir sus texturas, y exportar el resultado de vuelta a la escena como un `.glb` archivo, donde luego puede volver a medir la tasa de fotogramas en el mundo.

Ten en cuenta que las ediciones automatizadas de meshes pueden cambiar visualmente cómo se ve un modelo, así que revisa siempre los resultados y conserva copias de seguridad de tus archivos originales.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/optimizacion/performance-optimization.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
