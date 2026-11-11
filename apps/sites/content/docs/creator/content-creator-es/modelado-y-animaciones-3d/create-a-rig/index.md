> Aprende cómo crear un rig para personajes en Decentraland.

# Crear un rig

Este documento muestra cómo configurar un rig básico en Blender. Si quieres familiarizarte más con algunos conceptos de rigging, consulta [El rig: conceptos básicos](/docs/creator/wearables-and-emotes/emotes/avatar-rig/).


> [!NOTE]
> **💡 Consejo**: Instala el [Decentraland Tools Blender plugin](https://extensions.blender.org/add-ons/decentraland-tools/). Incluye varias funciones útiles para ayudarte a editar y exportar modelos 3D, wearables y emotes.


### Añadir la Armature

En primer lugar, tendrás que importar tu modelo 3D en Blender o, si ya lo hiciste en Blender, simplemente abre tu archivo .blend. En Object Mode, presiona `Shift+A` y selecciona ***Armature*** en el menú, como se muestra abajo. Luego, en ***Object Data Properties***, debajo de ***Viewport Display***, activa ***In Front***, para que puedas ver el bone a través de la mesh. La mesh y la Armature deben estar alineadas, así que asegúrate de que el modelo esté bien posicionado en el centro del mundo antes de añadir los bones. El origen de la Armature debe estar en 0,0,0 (X,Y,Z).

![](/docs/_remote/c2e0666e9dda208616b7bea9.webp)

*Añadir una armature y mostrarla a través de la mesh.*

### Editar la Armature

Después de añadir el objeto Armature, selecciónalo y ve a ***Edit Mode***. Aquí es donde añades los otros bones extruyendo el original (presiona `E`), duplicándolo (`Shift+D`) o simplemente añadiendo uno nuevo con `Shift+A`. El esqueleto debería seguir la forma del modelo, como un esqueleto real. Los bones pueden escalarse, moverse y rotarse, y tienen dos partes: la head y la tail. La head es el punto de pivote, lo que significa que las rotaciones y la escala comenzarán desde ahí. Eso también significa que es ahí donde ocurrirá la deformación de la mesh, así que coloca los bones tan centrados en la mesh como sea posible.

![](/docs/_remote/8512ec54c085799325f2d767.webp)

*Estructura del bone.*

![](/docs/_remote/edf9b89cb68b376886dd222e.webp)

*Bone girando desde su punto de pivote.*

No hace falta preocuparse por crear bones para el lado derecho e izquierdo por ahora. Concéntrate primero en crear solo un lado.

### Renombrar los Bones

Algo realmente importante que debes tener en cuenta es renombrar correctamente todos tus bones, según [la convención de nombres de Blender](https://docs.blender.org/manual/en/latest/animation/armatures/bones/editing/naming.html). Esto no solo mantendrá todo organizado, sino que también hará posible espejar poses y weight paint. Podrías simplemente renombrarlo con la parte del cuerpo que representa, como Spine\_01, o ser más específico y llamarlo DEF\_spine.001 (DEF significa deforming bone, así sabrás que este bone pertenece a la jerarquía de deformación).

### Orientación del Bone

Antes de continuar, debes comprobar la orientación de tu bone. Puedes hacerlo yendo a ***Object Data Properties*** y, debajo de ***Viewport Display***, activa ***Axes***. Esto hará visibles los axes del bone, para que puedas comprobar si van a rotar en la dirección correcta.

Los axes deben estar alineados con la dirección en la que quieres que roten los bones. Si notas que algo no encaja del todo, selecciona el bone y, en el ***Item Transform*** menú de la derecha del viewport, ajusta el ***Roll*** hasta que la orientación se vea correcta.

![](/docs/_remote/e985c26fabd10eaadd09c1d3.webp)

*Corrigiendo el bone roll.*

![](/docs/_remote/9737a17ebb3d4b75ab40ecbe.webp)

*Activa Axes para que puedas ver la dirección en la que rotará el bone.*

Corregir el bone roll es especialmente importante para los fingers, para que todas las joints se doblen en la misma dirección correcta. Sin embargo, no tienes que hacerlo para todos los bones de un finger, basta con que uno esté bien. Luego, selecciona primero los que quieres corregir y deja para el final el que tenga el roll correcto, ve a ***Armature > Bone Roll > Recalculate Roll > Active Bone*** y todos tendrán el mismo roll. O simplemente puedes presionar `Shift+N` > ***Active Bone***. Esto puede utilizarse en muchas situaciones en las que tienes una larga cadena de bones, como un tentáculo, una cola, una coleta larga o un brazo mecánico.

![](/docs/_remote/a781e753f3bb59cd494a2002.webp)

*Presiona `Shift+N` para recalcular el bone roll.*

### Espejar el rig

No tienes que crear bones para ambos lados. Haz primero un lado y, cuando hayas corregido todo el nombre y la orientación de los bones, selecciona todos los bones que quieres espejar, haz clic con el botón derecho del mouse y selecciona ***Symmetrize***. De ese modo, todos los bones se espejarán correctamente, con las orientaciones adecuadas. Recuerda que, para que esta herramienta funcione, los bones deben estar nombrados según la convención de nombres de Blender. Si hay una “L”, “R”, “left” o “right” en minúsculas o mayúsculas, Blender maneja correctamente el correspondiente.

Algunos ejemplos de convención de nombres correcta:

* Hand\_L –> Hand\_R
* Hand.L –>Hand.R
* HandLeft –> HandRight

**Todos los ejemplos anteriores son válidos, pero elige solo uno y manténlo consistente en todo el rig.**

![](/docs/_remote/de49fec9cde1c419a8de093f.webp)

*Usa la opción Symmetrize para espejar tus bones.*


> [!NOTE]
> **⚠️ ¡Atención!** Symmetrize solo funcionará si los bones están renombrados correctamente. Si no se espeja bien, quizá quieras revisar de nuevo los nombres de los bones.


### Skinning

Skinning es el proceso de vincular la mesh a la Armature. Para hacerlo, en ***Object Mode***, selecciona la mesh, luego la Armature y presiona `CTRL+P` > ***Armature Deform > With Automatic Weights.*** Luego, ve a ***Pose Mode*** y prueba diferentes poses para probar la deformación de la mesh. La mayoría de las veces necesitará algunos ajustes, como se ve a continuación.

![](/docs/_remote/6b45f045f5eacbbaf6f518ce.webp)

*Para el Skinning, haz que la mesh sea hija de la Armature presionando `CTRL+P` y pruébalo en Pose Mode.*

Si sientes que un bone se ha colocado mal o que el punto de pivote no es preciso, siempre puedes volver al ***Edit Mode*** y ajustarlo. Esto no afectará en absoluto al Skinning y siempre puedes usar ***Symmetrize*** para espejar los cambios.

Para corregir deformaciones extrañas, ve a ***Object Mode***, selecciona la mesh y ve a ***Weight Paint***. En ***Object Data Properties*** encontrarás los ***Vertex Groups***.

![](/docs/_remote/a2dbfb8ab2995b61fc35c031.webp)

*Edita la influencia de un bone en Weight Paint.*

Antes de empezar a editar, para que tu weight paint se espeje automáticamente mientras pintas, activa ***Mirror Vertex Groups*** y selecciona ***X*** en ***Mirror*** en ***Symmetry*** (en Blender 3.4.0) Para editar las influencias, selecciona el vertex group que quieras y, en el ***Tool*** menú, debajo de ***Blend***, alterna entre ***Add*** y ***Subtract*** según tus necesidades. También puedes cambiar el tamaño, el Weight y la Strength del brush.

![](/docs/_remote/3a6194f2b0653962efb783c3.webp)

*Cambia el Blend para añadir o quitar influencia. Asegúrate de que la Symmetry esté correcta.*

![](/docs/_remote/7787147101d32c50c282e5c6.webp)

Usa la ***Blur*** herramienta Blur del lado izquierdo de la pantalla para suavizar el weight paint y hacerlo más uniforme.

![](/docs/_remote/94e217fd4c7d693055d4cc02.webp)

*Herramienta Blur.*

### Configurar el IK

El IK es esencial cuando quieres que algo se mantenga en su lugar. El mejor ejemplo de uso es en las piernas, pero puede aplicarse en diversas situaciones, como en los ejemplos de abajo:

![](/docs/_remote/ae34643f4671eb5037f61506.webp)

*Uso del IK en un brazo mecánico.*

![](/docs/_remote/af572bd577d62ebeb0784242.webp)

*Uso del IK en cables.*

Como cambiará la jerarquía de los bones, es mejor mantenerlo como una configuración separada. Así que lo primero que necesitas hacer es duplicar la cadena de bones que será afectada por el IK. Tomemos la pierna como ejemplo. Selecciona los bones y presiona `Shift+D` para duplicarlos y moverlos a otra layer presionando `M` y seleccionando otro slot para ello.

Recuerda renombrar los bones duplicados, añadiendo IK al nombre para saber que forman parte de la configuración del IK. Y como no deberían deformar la mesh, selecciónalos todos y en ***Bone Property***, mantén presionado `Alt` y desmarca ***Deform***.

![](/docs/_remote/3b93713349f793f06d7f6bcb.webp)

*Duplica los bones y muévelos a otra layer.*

![](/docs/_remote/fb47b25da4bfe5bf1f79c139.webp)

*Selecciona todos los bones del IK y, mientras mantienes presionado `Alt`, desmarca Deform.*

Después, necesitarás crear un bone que controlará la cadena IK. Selecciona la head del foot bone y extrúyela en Y. Luego, presiona `Alt+P` para quitarle el parent, porque el IK bone no puede formar parte de la cadena ni puede estar conectado a otros bones. Tendrás que hacer que el foot sea hijo del IK, así que selecciona primero el foot bone y el IK bone al final y presiona `CTRL+P` > ***Keep Offset***.

![](/docs/_remote/2538a5aac86c9d53322d27e3.webp)

*Creando un IK bone.*

En ***Pose Mode***, haz clic en el shin bone y presiona `CTRL+Shift+C` y selecciona ***Inverse Kinematics***. Se verá todo desordenado, pero no te preocupes, se arreglará una vez que cambies algunos settings. Con el shin seleccionado, ve a las ***Bone Constraint Properties*** como se muestra abajo.

![](/docs/_remote/25208beb5685cd2437384baa.webp)

*En Bone Constraint Properties puedes editar los settings del IK.*

Para el ***Target***, selecciona ***Armature***. Una vez hecho eso, aparecerá una opción llamada ***Bone*** debajo de Target. Para eso, selecciona el IK bone que creaste. En ***Chain Length*** establecerás el número de bones que deben verse afectados por el IK. Como estamos haciendo una pierna, queremos que afecte al shin y al thigh, así que configúralo en ***2***.

### Pole Target

El Pole Target te permite controlar la dirección en la que se doblarán los bones. Estos son los pasos para añadir correctamente un pole target (o pole vector) a la cadena IK:

* En ***Edit Mode***, duplica el thigh bone y colócalo en cualquier lugar.
* Selecciona la knee joint (o la tail del thigh bone) y presiona `Shift+S` > ***Cursor to Selected.***
* Selecciona el thigh bone duplicado y presiona `Shift+S` > ***Selection to Cursor***.
* En ***Transform Orientation***, cámbialo a ***Normal*** y en ***Tansform Pivot Point*** cámbialo a ***Active Element***.
* Rota el bone en X -90 (o 90, según la orientación que hayas configurado) para que apunte hacia delante y muévelo en Y hasta que tenga una buena posición frente a la pierna. También puedes reducirlo un poco.

![](/docs/_remote/9eb9391b9e9f16ea7b21d708.webp)

*Creando un Pole Target.*

De vuelta en ***Pose Mode***, selecciona de nuevo el shin y ve a las ***Bone Constraint Properties***. Para ***Pole Target*** selecciona ***Armature*** y para ***Bone***, selecciona el pole target que acabas de crear. Si muestras los deform bones, verás que el pole target rotó un poco el IK.

![](/docs/_remote/a7dfd226eea1cbdac586bafa.webp)

*La cadena IK y los deform bones ya no están alineados debido al pole target.*

![](/docs/_remote/4c1a015b3348c877c0b7412e.webp)

*La rotación puede corregirse cambiando el Pole Angle.*

Eso puede arreglarse fácilmente cambiando el ***Pole Angle***. Normalmente -90° bastará, pero siempre puedes ajustarlo manualmente para asegurarte de que estén perfectamente alineados.

Por último, haz que tanto el IK bone como el pole target sean child del root bone seleccionándolos a ambos y luego el root; presiona `CTRL+P` > ***Keep Offset***.

Mueve el thigh y el shin a otra layer, ya que no los necesitarás para la animación; solo forman parte de la configuración del IK.

### Vincular Bones de deformación a los no deforming

La cadena IK ya está configurada, pero debería controlar los deform bones y ahora mismo eso no está ocurriendo; sin embargo, puedes usar constraints para arreglarlo. En ***Pose Mode***, selecciona primero un bone de la cadena IK y al final el bone deforming correspondiente, presiona `CTRL+Shift+C` y selecciona ***Copy Transforms***. Haz eso para todos los bones, que en el ejemplo son thigh, shin, foot y toes. Los deform bones tendrán un color verde, lo que significa que tienen un constraint. Si haces clic en ***Bone Constraint Properties***, podrás ver qué constraint se está usando y qué bone lo controla.

![](/docs/_remote/6b79927002f5fe0563c8468a.webp)

*Los bones verdes tienen constraints. Puedes comprobarlos en Bone Constraints Properties.*


> [!NOTE]
> **💡 Consejo**: No tienes que configurar el IK desde cero para el otro lado. En *Edit Mode*, simplemente elimina todos los bones del lado que no tiene el IK, luego selecciona todos los bones deforming y la cadena IK que quieras espejar, haz clic derecho con el mouse y selecciona *Symmetrize*. No solo espejará los bones, ¡sino también todos los constraints!


![](/docs/_remote/657c90b8853090a842847bce.webp)

*¡Usa la opción Symmetrize para espejar también los constraints!*

### Skeleton no deforming y controles\*\*

Es una buena práctica no animar directamente los bones deforming, ya que podrías acabar rompiendo el rig, y añadir constraints a la Armature deforming afectará la jerarquía y el comportamiento de los bones al exportar.

La solución es crear un skeleton no deforming que controle al deforming mediante constraints, y que pueda animarse con seguridad sin riesgo de romper nada. También puedes personalizar su forma para que sea más fácil identificar la función de un bone. Serán los controls de tu rig.

El proceso para esto es prácticamente el mismo que el de la configuración del IK. Duplica todos los bones (excepto la configuración del IK) con `Shift+D` y muévelos a otra layer presionando `M` y eligiendo otro slot. Renómbralos todos añadiendo **Control\_** o **CTRL\_** como sufijo para saber que forman parte de la jerarquía de controls. Presiona `A` para seleccionarlos todos y en ***Bone Property***, mantén presionado `Alt` y desmarca ***Deform***.

Ahora tendrás que añadir constraints para vincular los dos skeletons. Para facilitar este proceso, puedes cambiar la forma de los bones haciendo clic en ***Object Data Properties*** y, debajo de ***Viewport Display***, cambia ***Display As*** de ***Octahedral*** en ***B-Bone***. Luego, presiona `CTRL+Alt+S` para escalar un poco los bones y que sean más grandes que los originales.

![](/docs/_remote/99d8f893af1a418a6af220a9.webp)

*Cambia la forma de los bones y escálalos para que sea más fácil diferenciar entre los dos skeletons.*

Para los constraints, selecciona primero un control bone y al final el bone deforming correspondiente, presiona `CTRL+Shift+C` y selecciona ***Copy Transforms***. Haz eso para todos los control bones.

Puedes separar los control bones en diferentes groups y asignarles colores. Ve a ***Pose Mode*** > ***Object Data Properties*** > ***Bone Groups***. Haz clic en el ***+*** icono para añadir un nuevo group, renómbralo como quieras y selecciona un color para él. Luego, selecciona los bones que quieres que formen parte de ese group y haz clic en ***Assign***. Puedes crear tantos groups como quieras para mantener todo organizado. También puedes mover diferentes bone groups a diferentes layers.

![](/docs/_remote/d70aff06bf4143e4711b47e3.webp)

*Los bone groups te ayudarán a mantener tu rig organizado y más intuitivo.*

Otra forma de mejorar tu rig es personalizar la forma de los bones. Para ello, ve a ***Object Mode***, pulsa `Shift+A` para añadir una mesh, como un círculo, por ejemplo. Para mantener todo organizado, crea una collection para tus shapes y mueve allí el círculo que acabas de crear. Vuelve a Pose Mode y, en ***Object Data Properties*** > ***Viewport Display*** check ***Shapes***. Selecciona el bone que quieras y luego, en ***Bone Properties***, haz clic en ***Viewport Display*** > ***Custom Shape***. En ***Custom object***, selecciona el círculo en el outliner.

![](/docs/_remote/a712323c80495b4deba01b97.webp)

*Personalizando la forma del bone.*

Si la forma no se ve bien, siempre puedes editarla en ***Edit Mode***, como se muestra abajo. Asegúrate de que la shape tenga la misma orientación que el bone.

![](/docs/_remote/6ea2f3c66b5674c66248f2bc.webp)

*Edita la shape en Edit Mode.*

Crea diferentes shapes para distintos controls según su función, para que tu rig sea más intuitivo.

### Spring Bones

Además del skeleton deforming estándar, los wearables pueden incluir **spring bones** — bones adicionales cuyos transforms están controlados por una simulación física en lugar de por animación. Los spring bones hacen que el hair, los earrings, las capes y otros accesorios se muevan dinámicamente en respuesta al movimiento del avatar y la gravedad.

Para añadir spring bones a un rig:

1. En **Edit Mode**, extruye nuevos bones desde el parent bone apropiado (por ejemplo, `Avatar_Head` para el hair).
2. Nombra cada bone incluyendo `springbone` en el nombre (sin importar mayúsculas o minúsculas), por ejemplo: `SpringBone_hair_left`, `springbone_hair_left_01`, `Hair_SpringBone_left_tip` son nombres válidos, siempre que incluyan `springbone`.
3. formar **linear chains** de al menos **2 bones** (una root y una tip). Un solo spring bone por sí mismo no producirá ningún movimiento: la simulación necesita una cadena con un root bone que impulse la física y un tip bone que defina el extremo. Cada bone de la cadena debería tener como máximo un spring bone child.
4. Aplica Skin weight a la mesh para los spring bones como de costumbre.
5. Exporta como `.glb` — los spring bones se incluirán en la Armature.

Los parámetros de physics (stiffness, gravity, drag) se configuran en Builder después de subirlo, no en Blender. Para la guía completa, consulta [Spring Bones](/docs/creator/wearables-and-emotes/wearables/spring-bones/).

¡Y eso es todo! Con toda esta información, ya estás listo para hacer un rig básico!


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/modelado-y-animaciones-3d/create-a-rig.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
