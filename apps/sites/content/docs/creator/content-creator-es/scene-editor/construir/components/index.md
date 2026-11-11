> Comprende cómo funcionan los components de un ítem

# Entities y Components

Selecciona un elemento haciendo clic sobre él en el canvas o en el Entity tree. Luego verás sus Components mostrados en el properties panel, a la derecha de la pantalla. Distintos elementos tienen diferentes Components que muestran ajustes específicos.

![](/docs/_remote/8c1945559d13f38d8a485e7a.webp)

La mayoría de los elementos no interactivos tienen los siguientes Components:

* **Transform**: Establece la posición, la rotación y la escala del elemento. Si el elemento es hijo de otro elemento en el [Entity Tree](/docs/creator/content-creator-es/scene-editor/primeros-pasos/scene-editor-essentials/#the-entity-tree), estos valores son relativos a los del padre.
* **GLTF**: Qué modelo 3D cargar. Incluye la ruta local al archivo de este modelo 3D. También incluye algunas propiedades para configurar [colliders](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/colliders/#colliders-on-3d-models) en el modelo.

Los elementos de tu scene son todos **Entities**. Todo en una scene es una Entity; son los bloques de construcción básicos de las scenes. Los elementos son Entities que tienen al menos una posición y una forma visible.

## Añadir Components

Para añadir Components a cualquier Entity, haz clic en el **+** signo en la parte superior de la pestaña de propiedades y selecciona el Component de la lista. Consulta [Haz que cualquier elemento sea smart](/docs/creator/content-creator-es/scene-editor/interactividad/make-any-item-smart/)

![El menú para añadir Components está abierto sobre una Entity, mostrando Components agrupados en 3D Content e Interaction.](/docs/_remote/e0e52c0c060b63e97e2466dd.webp)

Puedes eliminar cualquier Component de una Entity haciendo clic en el icono de tres puntos a su derecha y seleccionando **Delete Component**.

## Crear una Entity desde cero

Para crear una Entity nueva desde cero, haz clic derecho sobre la raíz **Scene** Entity en el Entity tree, o sobre cualquier otra Entity, y selecciona **Add Child**

![](/docs/_remote/a6edc747060b504b764ee35b.webp)

Esto crea una Entity vacía con solo un **Transform** Component. La nueva Entity es hija de la Entity padre en la que hiciste clic. Luego puedes añadirle cualquier otro Components que quieras para darle la forma que desees.

## Components disponibles

Los siguientes Components se pueden añadir a cualquier Entity mediante la UI de Scene Editor:

* **Mesh Renderer**: Le da a la Entity una forma visible basada en una forma primitiva (cubo, plano, cilindro o esfera).
* **Mesh Collider**: Le da a la Entity una geometría collider invisible. Esto puede impedir que el jugador atraviese el elemento y/o hacer que sea clicable. Consulta [collider](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/colliders/).
* **Material**: Define el color, la textura y otras propiedades de una Entity que tiene un **Mesh Renderer** Component. Consulta [materials](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/materials/).

  <div data-gb-custom-block data-tag="hint" data-style="warning" class="hint hint-warning"><p>**📔 Nota**: El elemento debe tener un **Mesh Renderer** Component. No afecta a los elementos con una forma visible **GLTF**.</p></div>
* **Visibilidad**: Define si una Entity es invisible.
* **Fuente de luz**: Añade una luz a la Entity.
* **Cambiar material**: Intercambia el material de una Entity que tiene un **GLTF** component. Si el modelo 3D tiene varios meshes, puedes intercambiar el material de cada mesh de forma individual.
* **Fuente de audio**: Reproduce un sonido desde un archivo de sonido en la ubicación de la Entity. Consulta [Sonidos](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/sounds/).
* **Forma de texto**: Muestra texto en el espacio 3D. Consulta [Texto](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/text/).
* **Eventos de puntero**: Marca una Entity como clicable, mostrando una pista al pasar el cursor.


> [!WARNING]
> **📔 Nota**: El **Eventos de puntero** Component solo proporciona feedback. Para realizar acciones cuando se interactúa con una Entity, consulta [Haz que cualquier elemento sea smart](/docs/creator/content-creator-es/scene-editor/interactividad/make-any-item-smart/)


* **Multiplayer**: Comparte cualquier cambio que ocurra en la Entity para que todos los jugadores de la scene también lo vean. Se puede configurar para compartir solo cambios en ciertos Components. Consulta [Serverless Multiplayer](/docs/creator/content-creator-es/scenes-sdk7/networking/serverless-multiplayer/#mark-an-entity-as-synced) para más detalles.
* **Animador**: Controla la reproducción de animaciones en una Entity con un **GLTF** modelo 3D. Consulta [animaciones de modelos 3D](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/3d-model-animations/).
* **Tween**: Hace que la Entity se mueva, rote o escale gradualmente durante un periodo de tiempo. Consulta [Mover Entities](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/move-entities/).
* **Billboard**: Hace que la Entity rote siempre para mirar al jugador. Consulta [Mirar al usuario](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/entity-positioning/#face-the-player).
* **Avatar Attach**: Adjunta la Entity al avatar del jugador, para que lo siga. Consulta [Adjuntar una Entity a un avatar](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/entity-positioning/#attach-an-entity-to-an-avatar).
* **Video Player**: Reproduce un video dentro de la scene, que puede mostrarse en la Entity mediante su **Material**. Consulta [Reproducción de video](/docs/creator/content-creator-es/scenes-sdk7/media/video-playing/).
* **NFT Shape**: Muestra la imagen de un NFT dentro de un marco. Consulta [Mostrar un NFT](/docs/creator/content-creator-es/scene-editor/construir/nfts/).
* **Particle System**: Emite partículas desde la ubicación de la Entity, para efectos como humo, fuego o destellos. Consulta [sistemas de partículas](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/particle-system/).
* **Virtual Camera**: Define un ángulo de cámara personalizado al que la scene puede cambiar la vista del jugador. Consulta [Camera](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/camera/).
* **Avatar Modifier Area**: Cambia cómo se comportan o aparecen los avatars para los jugadores dentro de una región centrada en esta entity. La escala de la entity establece el tamaño de la región. Consulta [Modifier Areas](/docs/creator/content-creator-es/scene-editor/interactividad/modifier-areas/).
* **Camera Modifier Area**: Fuerza la cámara del jugador a primera o tercera persona dentro de una región centrada en esta entity. La escala de la entity establece el tamaño de la región. Consulta [Modifier Areas](/docs/creator/content-creator-es/scene-editor/interactividad/modifier-areas/).
* **Script**: Adjunta un módulo de código personalizado a la Entity, con parámetros que puedes configurar desde la UI. Consulta [Script component](/docs/creator/content-creator-es/scene-editor/extender-con-codigo/script-component/).


> [!WARNING]
> **📔 Nota**: Existen otros Components en el SDK que actualmente solo se pueden usar mediante código. También puedes crear tus propios [Custom Components](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/custom-components/) mediante código; estos no tendrán una representación en la UI, pero se pueden añadir y editar mediante código.
> 
> Consulta [Combinar con código](/docs/creator/content-creator-es/scene-editor/extender-con-codigo/overview/) para saber cómo editar el código de tu scene.
> 
> Ten en cuenta también que una Entity solo puede contener **uno** de cada Component. No es posible asignar una segunda instancia de un Component que ya existe en la entity. Por ejemplo, no puedes añadir dos **Actions** components a la misma Entity.


## Ajustes que conviene conocer

La mayoría de los campos de los components se corresponden uno a uno con la página del SDK enlazada arriba. Algunos son fáciles de pasar por alto, o se comportan de una manera que no es obvia a partir del nombre del campo.

### Fuente de audio

**Pitch** multiplica la velocidad de reproducción y desplaza el pitch con ella. `1` reproduce el sonido original, `2` lo reproduce al doble de velocidad (una octava más arriba), `0.5` a la mitad de velocidad (una octava más abajo). Debe ser mayor que 0 y su valor predeterminado es `1`.

### Video Player

* **Velocidad de reproducción**: la misma idea que Pitch, para video. `1` es velocidad normal. Déjalo en blanco para usar el valor predeterminado.
* **Posición de inicio (segundos)**: dónde comienza la reproducción cuando empieza el video. Déjalo en blanco para empezar desde el inicio.
* **Spatial Audio**: activa **Spatial**  para hacer que el sonido del video sea posicional, de modo que se atenúe a medida que el jugador se aleja. **Spatial Distance**  luego establece el rango: el audio se reproduce a volumen máximo hasta **Min** metros y se desvanece hasta el silencio en **Max** metros. Los valores predeterminados son 0 y 60.

Con Spatial **Spatial**  desactivado, el audio se reproduce al mismo volumen en toda la scene.

### Forma de texto

* **Tamaño**: el ancho (**W**) y la altura (**H**) del cuadro de texto, en metros. La alineación y el padding se miden con respecto a este cuadro.
* **Text Wrapping**: rompe el texto en nuevas líneas cuando alcanza el ancho establecido en **Tamaño**. Con esto desactivado, el texto se mantiene en una sola línea.

### Material

* **Alfa del color**: la opacidad del Color principal, de 0 a 1. No tiene efecto mientras **Color**, de 0 a 1. No tiene efecto mientras **Modo de transparencia** es **Opaco**.
* **Prueba alfa**: el punto de corte en el que un píxel se vuelve invisible. Solo aparece cuando **Modo de transparencia** es **Prueba alfa** o **Prueba alfa y mezcla**, ya que esos son los únicos modos que lo usan.
* **Ninguno**: los dropdowns de color ahora ofrecen una **Ninguno** entrada que borra el color, y el dropdown Type de la textura **Type** ofrece una opción **Ninguno** para quitar una textura.

### Particle System

La sección **Rotation**  controla cómo se orientan las partículas y cómo giran:

* **Billboard**: cada partícula siempre mira hacia la cámara. Activado por defecto. Desactívalo para partículas que deban girar libremente en 3D.
* **Face Travel Direction**: cada partícula empieza apuntando en la dirección en la que se mueve, como una flecha.
* **Initial Rotation (deg)**: cómo se rota cada partícula cuando nace, por eje.
* **Rotation Over Time (deg/sec)**: qué tan rápido gira, por eje. Los valores están limitados a 180 por eje.

Cuando **Use Texture**  está activado, aparecen dos dropdowns más:

* **Modo de ajuste**: cómo se comporta la textura más allá de sus bordes. **Clamp** (el valor predeterminado) estira los píxeles del borde, **Repeat** la repite en mosaico, **Mirror** la repite en mosaico con inversiones alternadas.
* **Modo de filtrado**: cómo se muestrea la textura. **Bilinear** (el valor predeterminado) es suave, **Point** es pixelado y va bien para pixel art, **Trilinear** suaviza entre niveles de mipmap.

### Tween

**Tipo de Tween** ofrece cinco modos:

| Modo                                                         | Qué hace                                                                     |
| ------------------------------------------------------------ | ---------------------------------------------------------------------------- |
| **Mover elemento**, **Rotar elemento**, **Escalar elemento** | Anima desde un valor inicial hasta un valor final durante el **Duración**.   |
| **Movimiento continuo**                                      | Sigue moviéndose en una dirección fija, para siempre, a velocidad constante. |
| **Rotación continua**                                        | Sigue girando alrededor de un eje fijo, para siempre, a velocidad constante. |

Los dos modos continuos no tienen valor final, así que **Inicio**, **Fin**, **Duración**, **Función de suavizado**  y **Loop**  están ocultos para ellos. En su lugar obtienes:

* **Movimiento continuo**: un vector **de dirección**  y una **Velocidad (metros/seg)**. Usa una dirección de longitud 1 para que la velocidad sea exactamente en metros por segundo. Una velocidad negativa mueve en la dirección opuesta.
* **Rotación continua**: un vector **Eje de rotación (grados)**  y una **Velocidad (grados/seg)**. Solo importa la dirección del eje, no el tamaño de los ángulos.

Al cambiar **Tipo de Tween** se restablecen estos valores a valores predeterminados razonables para el nuevo modo, porque los números significan cosas diferentes en cada uno.

La sección **Reproduciendo** casilla (antes llamada "Auto start") decide si el tween se ejecuta en cuanto carga la scene. Con ella desactivada, el tween se crea en pausa en su valor inicial y solo se ejecuta cuando tu código lo pone a reproducirse.


> [!NOTE]
> **💡 Consejo**: Algunas propiedades de los components solo se pueden establecer desde código. Editar una Entity en el inspector ya no las descarta, así que puedes establecer un valor en el código de tu scene y luego seguir usando el editor sobre esa Entity sin perderlo.


## Los Smart items

[Los Smart items](/docs/creator/content-creator-es/scene-editor/interactividad/smart-items/) también pueden incluir Components especiales que controlan la interactividad de la Entity. Normalmente son:

* **Actions**: enumera todas las acciones posibles que el elemento puede realizar.
* **Triggers**: determina cuándo se llevan a cabo las acciones del Component Actions.
* **States**: lleva un registro del estado actual del elemento. El estado se puede usar para lógica condicional, para activar solo ciertas acciones si el elemento está en cierto estado.
* **Counter**: lleva un registro de un contador. El contador se puede usar para lógica condicional, para activar solo ciertas acciones si el valor del contador es igual/mayor/menor que un valor dado.

Consulta [Smart items avanzados](/docs/creator/content-creator-es/scene-editor/interactividad/smart-items-advanced/) para más detalles.

## Sobre Entities y Components

Todo en una scene es una Entity. Todos los elementos y Smart items de la scene son Entities.

Todos los rasgos de una Entity están determinados por sus components. Definen qué es la Entity, dónde está, cómo suena y cómo se comporta. Por ejemplo, un **Transform** Component almacena las coordenadas, rotación y escala de la Entity. Un **MeshRenderer** Component le da a la Entity una forma visible (como un cubo o una esfera), y un **Material** Component le da a la Entity un color o una textura.

Los valores de los components pueden cambiar con el tiempo. En el Scene Editor configuras los valores iniciales de estos components. Pero una vez que tu scene está en ejecución, las acciones del jugador o el paso del tiempo pueden cambiar esos valores.

Por ejemplo, un Smart Item de plataforma móvil tiene una posición inicial que estableces mediante su **Transform** Component, pero después de que las actions de este item lo hagan moverse, su **Transform** tendrá valores diferentes.

Consulta [Entities y Components](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/entities-components/) para una visión en profundidad de este concepto y de cómo lo usan las scenes de Decentraland.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scene-editor/construir/components.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
