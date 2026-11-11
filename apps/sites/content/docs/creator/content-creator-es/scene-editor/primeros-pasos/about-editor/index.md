> El Scene Editor es una herramienta visual sencilla que te permite crear y publicar escenas de Decentraland.

# Editando escenas

Creator Hub incluye un potente Scene Editor que combina una sencilla interfaz sin código con la posibilidad de escribir código para personalizar aún más tus escenas.

![Creator Hub](/docs/_remote/9100f2b541affb1c6fecfe89.webp)

Consulta [Instalación de Creator Hub](/docs/creator/content-creator-es/scene-editor/primeros-pasos/editor-installation/) para comenzar.



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/tK5-fyBVnK0" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=tK5-fyBVnK0>">Watch on YouTube</a></p>



## Crear una escena

Para crear una nueva escena, abre Creator Hub y haz clic en la flecha junto al botón **Crear** botón. En el Dropdown, selecciona **Nueva escena**.

![El Dropdown del botón Crear, abierto, con las opciones Nueva escena e Importar escena.](/docs/_remote/727cb68c3c3feea0e6221431.webp)

A continuación, se te pedirá que nombres tu escena y elijas una ubicación para guardarla. Para comenzar con un proyecto que tenga contenido inicial en lugar de una escena vacía, haz clic en la pestaña **Templates** en la **escenas** y elige una.

Consulta [Administrar escenas](/docs/creator/content-creator-es/scene-editor/primeros-pasos/manage-scenes/) para más detalles.

Una vez que se abra la escena, verás el Entity tree a la izquierda, el Canvas en el centro, las propiedades del elemento seleccionado a la derecha y los Asset packs en la parte inferior.

![El Scene Editor con una pequeña escena abierta: el Entity tree de la izquierda enumera Player, Camera, Scene y algunos elementos; el Canvas muestra un Ground de césped con una barbacoa y una fuente con pájaros, y el Properties panel de la derecha muestra los Components Transform y GLTF de la fuente con pájaros seleccionada.](/docs/_remote/e758b583939953f86e7202c8.webp)

Consulta [Aspectos esenciales del Scene Editor](/docs/creator/content-creator-es/scene-editor/primeros-pasos/scene-editor-essentials/) para ver un recorrido por cada sección.

## Desplazarse

Para desplazarte por el Scene Editor:

* Usa **W** y **S** para acercarte o alejarte. También puedes usar la rueda de desplazamiento del mouse o las teclas **+** y **-** teclas
* Usa **A** y **D** para moverte lateralmente.
* Usa **Q** y **E** para subir y bajar.
* Usa la **Botón izquierdo del mouse** para hacer clic y seleccionar elementos, y para moverlos.
* Usa la **Botón derecho del mouse** y arrastra para rotar la cámara.


> [!NOTE]
> **💡 Consejo**: También puedes rotar la cámara presionando **Alt** en Windows o **Option** en Mac mientras arrastras. Esto resulta especialmente útil al usar un trackpad en lugar de un mouse.


* Presiona **Barra espaciadora** para restablecer la cámara a la posición predeterminada

## Añadir items

Navega por las categorías temáticas de Asset packs en el menú de la parte inferior para encontrar distintos elementos que puedes colocar en tu escena.

![](/docs/_remote/c5121b56db02ac73b992d99e.webp)

Para colocar un elemento, haz clic y arrástralo desde el menú de Asset packs hasta una ubicación de tu escena en el Canvas.

![](/docs/_remote/b20b9f38d71f91b4c4270633.webp)

Haz clic y arrastra un elemento seleccionado para moverlo libremente por la escena a nivel del suelo. Consulta [Aspectos esenciales del Scene Editor](/docs/creator/content-creator-es/scene-editor/primeros-pasos/scene-editor-essentials/#position-items) para más detalles.


> [!NOTE]
> **💡 Consejo**: Algunos elementos son **Los smart items**, estos incluyen comportamientos interactivos integrados. Consulta [Los smart items](/docs/creator/content-creator-es/scene-editor/interactividad/smart-items/) para más detalles.


![](/docs/_remote/4401edd6b0ae269ede4c9467.webp)

Para que la vista 3D ocupe toda la altura de la ventana, contrae el Panel de Assets con el botón de chevrón situado en el extremo derecho de su barra de Tabs. El Panel se reduce a solo sus Tabs, y el mismo botón lo restaura a la altura que tenía. Al hacer clic en cualquier Tab mientras está contraído, también se vuelve a abrir.

### Avatar animations

El **Animaciones del avatar** pack contiene animaciones para el Avatar del jugador, en lugar de elementos para colocar: poses para sentarse, pesca, cargar una caja, baile y más. Están ordenadas en los mismos grupos **Actions**, **Sentado** y **Expresiones** que el resto.

Pasa el cursor sobre una tarjeta para ver cómo se reproduce la animación. Arrastra una a la vista 3D para copiar su File a los Local Assets de tu escena. No se añade ninguna Entity, porque una animación no es algo que se sitúe en la escena: se reproduce desde código. Consulta [Animaciones del avatar](/docs/creator/content-creator-es/scenes-sdk7/interactividad/avatares/avatar-animations/).

Las tarjetas marcadas con una insignia **Built-in** funcionan de manera diferente. Esas animaciones ya se incluyen con Decentraland, por lo que cualquier escena puede reproducirlas sin añadir ningún File. No puedes arrastrarlas. En su lugar, haz clic derecho en una tarjeta y elige **Copiar nombre** para obtener el nombre que debes usar en tu código.

En la **Local Assets** pestaña, el filtro **Animaciones del avatar** reúne los Files de animación de tu proyecto, junto con una carpeta de solo lectura **Animaciones integradas** que enumera las que no requieren ningún File.

## Preview

Para probar tu escena y experimentarla como un jugador, haz clic en el **Reproducir** botón en la esquina superior derecha. Esto abrirá una nueva ventana con el Explorer de escritorio de Decentraland, ejecutando solo tu escena. Allí podrás moverte por la escena e interactuar con elementos interactivos.


> [!WARNING]
> **📔 Nota**: Si no lo tienes instalado en tu equipo, descarga el **Decentraland Launcher** de [Decentraland.org](https://decentraland.org).


![](/docs/_remote/ca7e6550cdb1bb947ce74b28.webp)

Haz clic en la flecha junto al **Reproducir** botón para abrir el **Opciones de reproducción** menú, donde eliges si hacer Preview en el Client de escritorio, en un navegador o en un teléfono. Consulta [Previsualiza tu scene](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/preview-scene/) para ver una lista completa de todas las opciones disponibles.

## Renderizador de scene

El Scene Editor utiliza **Babylon** como su Renderer predeterminado para el Canvas de edición. Puedes cambiar a **Bevy (experimental)**.

Para cambiar el Renderer:

1. Haz clic en el logotipo de Creator Hub en la esquina superior izquierda y selecciona **Configuración**.
2. Ve a la pestaña **EXPERIMENTAL** pestaña.
3. Activa **Habilitar el Renderer de escenas Bevy**.

![El cuadro de diálogo Preferencias de la aplicación en la pestaña EXPERIMENTAL, con el interruptor Habilitar el Renderer de escenas Bevy.](/docs/_remote/ef0ed302a5aa995c466b03ac.webp)

El Renderer Bevy es un Engine alternativo para el Canvas de edición. Afecta al aspecto de tu escena durante la edición, no al aspecto que tendrá para los jugadores después de publicarla.

Al usar el Renderer Bevy, la barra de herramientas incorpora un Dropdown de modo de cámara, botones de Play y Stop para ejecutar la escena en el Canvas, un Toggle **Interact** (el icono de gamepad) y un Toggle **Silenciar el audio de la escena** (el icono de altavoz).

![La barra de herramientas del editor con el Renderer Bevy: deshacer y rehacer, las herramientas Gizmo, un Dropdown de cámara con la opción Free, botones de Play y Stop, el Toggle Interact con icono de gamepad, el engranaje de Preferences y el botón Inspector.](/docs/_remote/851c27272919db1fa6fcf53e.webp)

Usa la **Interact** para probar una escena en ejecución directamente en el Canvas del editor sin abrir una ventana de Preview independiente. Esto te permite comprobar rápidamente las interacciones sin detener la escena. Mientras interactúas, los Smart items muestran sus Hints al pasar el cursor, como la tecla que debes presionar, directamente en el Canvas.

Haz clic en el botón de altavoz, etiquetado como **Silenciar el audio de la escena**, para silenciar la escena mientras trabajas. Silencia todos los [Audio Source](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/sounds/) y los Streams de Audio de la escena, tanto mientras editas como mientras la escena se ejecuta en el Canvas, y la escena permanece silenciada si se recarga. Vuelve a hacer clic, ahora etiquetado como **Activar el audio de la escena**, para recuperar el sonido. El botón solo aparece con el Renderer Bevy, porque el Renderer Babylon no reproduce el Audio de la escena.

El editor Bevy también carga [elementos personalizados](/docs/creator/content-creator-es/scene-editor/interactividad/custom-items/), muestra marcadores de posición para Assets dañados o faltantes y proporciona mensajes de error claros con acciones de corrección cuando las dependencias del SDK de tu escena están desactualizadas. El Toggle de cámara en la barra de herramientas muestra **Free** o **Player**, según el modo en el que te encuentres.


> [!WARNING]
> **📔 Nota**: La pestaña de métricas de la escena no está disponible con Bevy. Ejecuta un Preview de la escena en el Client de escritorio o móvil y consulta las estadísticas desde allí.



> [!WARNING]
> **Nota:** El Renderer Bevy es experimental y es posible que no admita todas las funciones de edición que sí admite el Renderer Babylon predeterminado.


## Configuración de la escena

Haz clic en el **Icono de lápiz** en la esquina superior derecha de la pantalla. Esto abre una serie de propiedades de nivel de escena para editar, incluidos el nombre, la miniatura, el tamaño de la escena y más.

![](/docs/_remote/a64bc0ae30e4d852f97ccd78.webp)

Consulta [Configuración de la escena](/docs/creator/content-creator-es/scene-editor/configurar/scene-settings/) para más detalles.

## Publica tu scene

Cuando estés satisfecho con tu escena, presiona el botón **Publicar** botón.

![](/docs/_remote/75db93f7a2fdd17bec3b4907.webp)

Consulta [Publicar escena](/docs/creator/content-creator-es/scene-editor/publicar/publish-scene/) para más detalles.

## Ver también

* Consulta [Aspectos esenciales del Scene Editor](/docs/creator/content-creator-es/scene-editor/primeros-pasos/scene-editor-essentials/) para obtener más información sobre la interfaz del Scene Editor.
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
GET https://docs.decentraland.org/creator/content-creator-es/scene-editor/primeros-pasos/about-editor.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
