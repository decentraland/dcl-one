> Cómo usar el Scene Editor

# Conceptos esenciales del Scene Editor

La UI del Scene Editor se divide en varias secciones diferentes, con distintos propósitos.

![](/docs/_remote/ee9a6732ea7a70cc625e7341.webp)

* **Canvas**: Manipula items directamente y ve cómo se ve tu scene.
* **Entity tree**: Contiene una lista de todos los items de la scene y su jerarquía.
* **Propiedades**: Muestra detalles sobre el item actualmente seleccionado.
* **Recursos**: Muestra recursos que están disponibles para usar.

## Desplazarse

Para desplazarte por el Scene Editor:

* Usa **W** y **S** para acercarte o alejarte. También puedes usar la rueda de desplazamiento del mouse o las teclas **+** y **-** teclas
* Usa **A** y **D** para moverte lateralmente.
* Usa **Q** y **E** para subir y bajar.
* Haz clic en el **Botón derecho del mouse** y arrastra para rotar la cámara.
* Presiona **Barra espaciadora** para restablecer la cámara a la posición predeterminada
* Usa **Botón izquierdo del mouse** para hacer clic y seleccionar elementos, y para moverlos.

## Previsualizar en móvil

El **Opciones de reproducción** menú junto al **Reproducir** botón tiene una **Mostrar código QR para móvil** opción. Escanea el código QR con un teléfono en la misma red Wi‑Fi para abrir tu scene en el [aplicación móvil de Decentraland](/docs/creator/content-creator-es/desarrollar-para-movil/cliente-movil/overview/). Esta es la forma recomendada de validar la UI, los controles y el rendimiento para jugadores móviles. Consulta [Previsualizar en móvil](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/preview-on-mobile/) para la guía completa.

## Configurar el Ground

El Ground de la scene puede usar varias texturas diferentes. Puedes encontrarlas en los diferentes asset packs temáticos en el menú de items.

Los items de tipo **Ground** tienen un icono de cubeta de pintura. Si arrastras uno de estos a tu scene, cubrirá todo el Ground de tu scene con copias de este item.

![](/docs/_remote/e120bb99677fe9cb301458fb.webp)

También puedes añadir una sola copia del item manteniendo pulsada **Shift** mientras arrastras el Ground a la scene.

![](/docs/_remote/df027a655d783fcc8276005c.webp)

La colección de items de Ground aparece en el [entity tree](#the-entity-tree) dentro de una carpeta. Cada uno de ellos está bloqueado para evitar seleccionarlo accidentalmente. [Desactivar](#lock-or-hide-items) los items para moverlos o editarlos.

## Añadir items

Navega por las categorías temáticas de Asset packs en el menú de la parte inferior para encontrar distintos elementos que puedes colocar en tu escena.

![](/docs/_remote/c5121b56db02ac73b992d99e.webp)

También puedes usar la caja de búsqueda. Ten en cuenta que cuando estás dentro de un asset pack, la búsqueda solo se realiza dentro de ese asset pack.

Para colocar un elemento, haz clic y arrástralo desde el menú de Asset packs hasta una ubicación de tu escena en el Canvas.

![](/docs/_remote/b20b9f38d71f91b4c4270633.webp)


> [!NOTE]
> **💡 Consejo**: Tus cambios se guardan automáticamente cada vez que añades, mueves o editas propiedades de cualquiera de los items de tu scene.


Para duplicar un item, selecciónalo y pulsa **Ctrl + C** y luego **Ctrl + V**. También puedes encontrar el item en el [entity tree](#the-entity-tree) para hacer clic derecho y seleccionar la opción **Duplicar**. El nuevo item quedará perfectamente superpuesto al original.

Para eliminar un item de la scene, selecciónalo y pulsa la *Eliminar* tecla.

Consulta [Importar items](/docs/creator/content-creator-es/scene-editor/construir/import-items/) para añadir tus propios modelos 3D personalizados desde el disco.


> [!WARNING]
> **📔 Nota**: Una vez que arrastras un modelo 3D a tu scene, se descarga en la carpeta de tu proyecto y permanece allí incluso si lo eliminas. Estos modelos sin usar pueden aumentar el tamaño de tu scene.
> 
> Abre la **Local Assets** pestaña para eliminar cualquier modelo sin usar.


## Posicionar items



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/cNl02PFPdcQ" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=cNl02PFPdcQ>">Watch on YouTube</a></p>



Haz clic con el **Botón izquierdo del mouse** y arrastra un item seleccionado para moverlo libremente por la scene a nivel del Ground.

También puedes usar las herramientas del menú superior:

![](/docs/_remote/c6834cebdebce2e0fb6608ba.webp)

* **Herramienta de mover**: Haz clic y arrastra cada flecha para mover el item en un solo eje a la vez. Con esta herramienta también puedes colocar cosas por encima del nivel del Ground.
* **Herramienta de rotar**: Haz clic y arrastra cada uno de los aros alrededor del item para rotarlo en un eje a la vez.
* **Herramienta de escalar**: Haz clic en el centro del gizmo y arrastra hacia dentro o hacia fuera para agrandarlo. Esta herramienta también te permite estirar un item en un solo eje para cambiar sus proporciones; para hacerlo, haz clic en uno de los ejes del gizmo y arrastra.

![](/docs/_remote/11bce750b05bad2b59a8c3b8.webp)

Para tener mayor precisión al mover, rotar o escalar un item, mantén pulsada la *Shift* tecla mientras haces ajustes. Esto evitará que se ajuste a la rejilla.

Para cambiar la granularidad del movimiento y otros ajustes, haz clic en la flecha hacia abajo a la derecha de las herramientas. Están disponibles los siguientes ajustes:

* **Snap**: Activa o desactiva la rejilla. Cuando está desactivada, el comportamiento de **Shift** se invierte: no sigues la rejilla por defecto; sí lo haces si mantienes pulsada **Shift**.
  * **Posición**: El tamaño de los incrementos de movimiento en metros cuando **Snap** está activado.
  * **Rotación**: El tamaño de los incrementos de rotación en grados cuando **Snap** está activado.
  * **Escala**: El tamaño de los incrementos de escala cuando **Snap** está activado.
* **Alinear al mundo**: Una sola casilla de verificación que se refiere al eje usado por los gizmos. Cuando está marcada, el eje de la herramienta Move y Rotate siempre se alinea con el mundo y no cambia con la orientación del objeto. Cuando no está marcada, se alinea con la orientación del objeto.

Para seleccionar varios items al mismo tiempo, mantén pulsada la *Control* tecla mientras los seleccionas. Luego podrás moverlos, rotarlos, escalarlos, duplicarlos o eliminarlos todos en una sola acción.

### Escribe una cantidad exacta

Cuando arrastrar no es lo suficientemente preciso, escribe la cantidad en su lugar:

1. Pulsa una tecla de herramienta: **M** o **G** para mover, **R** para rotar, **X** para escalar.
2. Presiona **X**, **Y** o **Z** para elegir un eje. Scale no necesita uno: sin un eje, escala todos los ejes por el mismo factor.
3. Escribe el número. Usa **.** para los decimales, y **-** para alternar entre positivo y negativo.
4. Presiona **Enter** para aplicar, o **Esc** para cancelar. **Backspace** borra lo que escribiste.

Un chip en la parte inferior del Canvas muestra la entrada mientras la escribes, te indica qué falta todavía y confirma lo que se aplicó.

Algunos ejemplos:

* **R** **X** **1** **5** **Enter** rota la selección 15 grados alrededor de X.
* **X** **2** **Enter** hace que la selección sea el doble de grande.
* **M** **Y** **-** **3** **Enter** la mueve 3 metros hacia abajo.

Algunas cosas que conviene saber:

* La cantidad siempre es relativa a dónde está la selección ahora, no a una coordenada absoluta.
* Los valores escritos ignoran la rejilla de snap.
* Con varios items seleccionados, todos se mueven, rotan o escalan alrededor de su centro compartido, como un solo paso de deshacer.
* Al pulsar **M**, **G** o **R** una segunda vez, también desactiva su gizmo. **X** no lo hace, porque mientras escribes significa el eje X.


> [!NOTE]
> **💡 Consejo**: El icono del teclado en la esquina inferior derecha del Canvas abre la lista completa de atajos.


## Los smart items

Los Smart items son items especiales que incluyen comportamientos interactivos integrados. Consulta [Los smart items](/docs/creator/content-creator-es/scene-editor/interactividad/smart-items/) para más detalles.

![](/docs/_remote/4401edd6b0ae269ede4c9467.webp)

## El entity tree

En el margen izquierdo verás una estructura en árbol con todas las entities de la scene. Esto incluye todos los items que añades, así como algunas entities predeterminadas.


> [!NOTE]
> **💡 Consejo**: Todo en una scene es una Entity; son los bloques básicos de las scenes. Los items son Entities que tienen al menos una posición y una forma visible.


En lugar de seleccionar un item haciendo clic sobre él desde la vista 3D de la scene, puedes seleccionarlo desde la vista en árbol. Haz clic con el botón derecho del ratón sobre una entity para mostrar más opciones: puedes renombrarla, eliminarla o duplicarla; también crear una entity hija, convertirla en un custom item o añadir un component a la entity.

![El entity tree con el menú contextual abierto sobre la entity Bird Fountain, mostrando Rename, Add child, Duplicate, Delete, Create Custom Item y Add component.](/docs/_remote/42bfc23b5b7dbda3a5127c7a.webp)

### Buscando en el entity tree

Un cuadro de búsqueda en la parte superior del entity tree te permite encontrar entities por nombre. Escribe parte del nombre de una entity y el árbol se filtra para mostrar solo las entities que coinciden y su jerarquía padre, para que puedas ver dónde está cada coincidencia dentro del árbol. La búsqueda no distingue mayúsculas de minúsculas. Las entities padre de las coincidencias se expanden automáticamente para que cada resultado sea visible.

Presiona **Escape** o haz clic en el icono de borrar para eliminar el filtro y volver al árbol completo. Si hay una entity seleccionada, al borrar la búsqueda se desplaza de nuevo a la entity seleccionada para que no pierdas tu lugar.

![El entity tree filtrado por el término de búsqueda fount: solo siguen visibles las entities Camera, Scene y Bird Fountain, con el padre Scene expandido para mostrar la coincidencia.](/docs/_remote/462138368861e2d4fd3aba3d.webp)

### Jerarquía de entities

Las entities siguen una jerarquía que puede tener tantos niveles como quieras. Establece una relación padre-hijo entre dos entities arrastrando un item sobre otro en el árbol. Una entity hija hereda la posición de la entidad padre, así que cuando la entidad padre se mueve, arrastra consigo a cualquier hijo. Esto puede ser práctico al construir una scene; por ejemplo, puedes establecer unas gafas y unos platos como hijos de una mesa y luego mover la mesa sin necesidad de reajustar nada más. También puede ser importante al interactuar con la scene, para que los items se muevan juntos.

![](/docs/_remote/2ba2f0d9e5656796ad572f00.webp)

También puedes minimizar o expandir los hijos de una entity para mantener la vista simple; esta acción no tiene ningún efecto en la scene.

### Entities especiales

La scene incluye un par de entities especiales que puedes ver en el entity tree.

* **Scene**: Esto se refiere a la RootEntity; todo lo que añades en la scene es hijo de esta entity. Puedes abrirla para ver [los ajustes de la scene](#scene-settings).
* **Player**: El avatar del jugador. Puedes añadir components especiales a esta entity que pueden cambiar las mecánicas de juego. También puedes arrastrar otras entities para que sean hijas del avatar. Si una entity es hija del avatar, su posición quedará fijada al jugador. Úsalo, por ejemplo, para añadir un marcador flotante sobre la cabeza del jugador, que lo siga mientras se mueve.
* **Camera**: La cámara del jugador. Puedes arrastrar otras entities para que sean hijas de la cámara. Si una entity es hija de la cámara, su posición quedará fija en pantalla. Úsalo, por ejemplo, para mostrar un arma en un juego de disparos, que siempre esté a la vista incluso si el jugador mira hacia arriba o hacia abajo.

### Bloquear u ocultar items

A veces puede resultarte útil bloquear un item para evitar seleccionarlo y moverlo accidentalmente. Esto es especialmente útil para items de fondo, como el Ground o un edificio. Para bloquear un item, búscalo en el entity tree de la izquierda, pasa el cursor sobre él y selecciona el icono del candado. Puedes activar y desactivar este comportamiento desde ese mismo icono.

También puede convenirte ocultar un item que pueda obstruir tu vista mientras colocas otros. Esto es especialmente útil para ocultar el techo o un edificio mientras trabajas en los interiores. Los items ocultos solo se ocultan en la ventana Canvas del Scene Editor, no para los jugadores que entren en la scene. Para ocultar un item, búscalo en el entity tree de la izquierda, pasa el cursor sobre él y selecciona el icono del ojo. Puedes activar y desactivar este comportamiento desde ese mismo icono.

![](/docs/_remote/1360d2763c98c1ac75eca0d4.webp)

## Panel de propiedades

Selecciona un item haciendo clic sobre él en el Canvas o en el entity tree. Entonces verás sus components mostrados en el panel de propiedades, a la derecha de la pantalla. Diferentes items tienen diferentes components que muestran ajustes específicos.

![](/docs/_remote/8c1945559d13f38d8a485e7a.webp)

La mayoría de los ítems no interactivos tienen los siguientes Components:

* **Transform**: Establece la posición, rotación y escala del item.
* **GLTF**: Qué modelo 3D cargar.

[Los smart items](/docs/creator/content-creator-es/scene-editor/interactividad/smart-items/) puede incluir otros components.

Consulta [Components](/docs/creator/content-creator-es/scene-editor/construir/components/) para saber más.

## Limitaciones de la escena

Las scenes de Decentraland deben seguir ciertas limitaciones para poder ejecutarse una junto a otra. Existe un número máximo de materials, textures, triangles, etc., que es proporcional al número de parcels en la scene. Consulta [limitaciones de la escena](/docs/creator/content-creator-es/scenes-sdk7/optimizacion/scene-limitations/) para más detalles.

Si el contenido de tu scene supera alguno de estos límites, el Scene Editor te lo notificará en la esquina inferior izquierda.

![](/docs/_remote/ab87e37107598ee524831639.webp)

Puedes expandir este menú para ver los detalles.

![](/docs/_remote/8d4619b1778683c8d72d3c61.webp)


> [!NOTE]
> **💡 Consejo**: Si estás creando un Decentraland World, siempre puedes aumentar el [tamaño de la scene](/docs/creator/content-creator-es/scene-editor/configurar/scene-settings/#layout) para aumentar tus límites.


El contenido de una scene de Decentraland también debe evitar desbordarse hacia parcels vecinas. Si alguna parte de los modelos de tu scene se extiende más allá de los límites, cuando abras la preview de la scene verás que esas partes quedan recortadas. El Scene Editor marcará todo el modelo en rojo, pero realmente solo debes preocuparte por las partes del modelo que se extienden más allá de los límites de la scene.

![](/docs/_remote/8dcb8f74f41494a20d11ba4d.webp)


> [!NOTE]
> **💡 Consejo**: Si los modelos que quieres mostrar no caben, quizá quieras aumentar el tamaño de tu scene. Consulta [tamaño de la scene](/docs/creator/content-creator-es/scene-editor/configurar/scene-settings/#layout) para ampliar tu scene.


Incluso si toda la geometría del modelo 3D cabe en tu scene, un modelo podría marcarse en rojo si el Bounding Box del modelo se extiende más allá del área. Si este es el caso, puedes ignorar las advertencias, ya que todo el modelo se mostrará correctamente. Aprende más sobre [Bounding Boxes](/docs/creator/content-creator-es/modelado-y-animaciones-3d/meshes/#bounding-boxes).

## Limpiar assets

Mantén tu proyecto ordenado eliminando los assets que ya no se usan. Abre la **Local Assets** pestaña en la parte inferior de la pantalla y haz clic en el icono del pincel.

![](/docs/_remote/98ff3e388967f6b98ae40ab1.webp)

Se abre un diálogo que enumera todos los assets de tu scene y resalta los que no están referenciados por ningún item.

![](/docs/_remote/a20d113518b454ef45385873.webp)

Selecciona los assets que quieres eliminar usando las casillas de verificación y luego haz clic en **Eliminar seleccionados** para eliminarlos permanentemente.

Eliminar un item de la scene no borra sus archivos. Los modelos, textures o sounds importados permanecen en tu proyecto hasta que los limpies, así que revisa periódicamente los assets sin usar.


> [!WARNING]
> **Importante:** Si tu scene contiene código que referencia assets, algunos assets en uso pueden aparecer como sin usar. Este diálogo solo detecta assets referenciados por components en la UI de Creator Hub. Después de hacer clic en **Eliminar seleccionados**, los archivos se eliminan de la carpeta del proyecto; esta acción es permanente y no se puede deshacer.


## Configuración de la escena

Haz clic en el **Icono de lápiz** en la esquina superior derecha de la pantalla. Esto abre una serie de propiedades a nivel de scene para editar.

![](/docs/_remote/a64bc0ae30e4d852f97ccd78.webp)

Aquí puedes configurar varias propiedades, incluyendo el título y la miniatura, el tamaño de la scene, la categoría de la scene y la clasificación por edad, las ubicaciones de spawn del player y los toggles de funciones.

Consulta [Configuración de la escena](/docs/creator/content-creator-es/scene-editor/configurar/scene-settings/).

## Ver también

* Consulta [Los smart items](/docs/creator/content-creator-es/scene-editor/interactividad/smart-items/) para saber cómo añadir interactividad sencilla a tu escena.
* Consulta [Combinar con código](/docs/creator/content-creator-es/scene-editor/extender-con-codigo/overview/) para saber cómo editar el código de tu escena.
* Consulta [Publicar escena](/docs/creator/content-creator-es/scene-editor/publicar/publish-scene/) para saber cómo publicar tu escena en Decentraland.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scene-editor/primeros-pasos/scene-editor-essentials.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
