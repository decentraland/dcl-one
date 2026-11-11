> Guías para añadir props y sonidos a los emotes.

# Props y sonidos

Para llevar tus Decentraland Emotes al siguiente nivel, puedes añadir props (geometría 3D) y/o sonidos a ellos, ¡haciendo que los emotes sean mucho más divertidos y atractivos! En esta guía encontrarás todo lo que necesitas saber para exportarlos correctamente!


> [!NOTE]
> **💡 Consejo**: Instala el [Decentraland Tools Blender plugin](https://extensions.blender.org/add-ons/decentraland-tools/). Incluye varias funciones útiles para ayudarte a editar y exportar modelos 3D, wearables y emotes.


## **Lo básico y las limitaciones**

Para empezar a añadir los props a tus emotes, es importante usar el [Decentraland Template File](https://raw.githubusercontent.com/decentraland/documentation-creators/main/images/emotes/BaseMale_Rig_1.0.blend) que tendrá el rig para el Avatar y también el Ground Reference para mantener tu trabajo dentro de los límites permitidos del espacio.

**Actualmente, las animaciones de los props solo funcionan con Armature/Bones Animations, lo que significa que las animaciones \_transform**\_\*\* no están permitidas.\*\*

El emote con sus props debe exportarse todo junto en un único *.glb* archivo (Avatar\_Armature + Props\_Armature con sus animations).

* No más de 3 MB en total.
* No más de 3k tris en total para props.
* No más de 2 materials y 2 textures para props.
* No más de 62 bones para la armature del prop.
* El emote debe tener una animation para el Avatar y una animation para el prop. *Actualmente no se permiten múltiples animations.*
* Ambas animations (Avatar y Prop) deben tener la misma longitud de keyframes.
* Las animations no pueden exceder 300 frames o 10 segundos.
* Los límites del espacio son 4 metros cuadrados. Los props y las particles deben mantenerse dentro del cubo de referencia proporcionado en el Avatar File. Para el movimiento del Avatar, consulta [Ground Reference and Animation Area](/docs/creator/content-creator-es/wearables-y-emotes/emotes/creating-emotes/#ground-reference-and-animation-area). sección.

## **Convenciones de nombres:**

¡Las convenciones de nombres deben seguirse estrictamente para que los emotes funcionen! De lo contrario, no se reproducirán correctamente ni en el Builder ni en World.

#### Convenciones de nombres de Armatures:

**Para Avatar:**

`Armature`

**Para props:**

`Armature_Prop`

#### Convenciones de nombres de Animations:

**Para Avatar:**

`AnimationName_Avatar`

* Ejemplo: `TennisServe_Avatar`, `GunShoot_Avatar`

**Para props:**

`AnimationName_Prop`

* Ejemplo: `TennisServe_Prop`, `GunShoot_Prop`

## **Cómo empezar**

Antes de empezar tu animation, tendrás que crear un rig para el prop. Si no estás familiarizado con el proceso, consulta [Crear un Rig](/docs/creator/content-creator-es/modelado-y-animaciones-3d/create-a-rig/) para obtener más información sobre cómo hacerlo.

Asegúrate de que el objeto del prop y la armature tengan sus origins ubicados en la posición 0,0 dentro de Blender. Además, aplica transformations al objeto del prop y a la armature, asegurando que queden congeladas con una escala de 1,1,1. Esto es crucial para evitar posibles problemas con el comportamiento del prop cuando se utilice dentro de World o durante las animations.

#### Hacer que el Prop siga el rig del Avatar

Algunos props pueden tener que estar sujetos a ciertas partes del cuerpo, como una raqueta de tenis a la mano. Eso se puede hacer simplemente añadiendo un constraint. Para hacerlo, en ***Pose Mode***, selecciona el bone del prop (el de la raqueta de tenis, por ejemplo), pulsa ***CTRL + Shift + C*** en tu teclado y selecciona ***Child of*** o simplemente haz clic en la ***Bone Constraint Properties*** pestaña y, en el menú desplegable, selecciona ***Child of***.

*Añade un constraint pulsando `Ctrl + Shift + C` en tu teclado.*

Luego, en ***Target***, selecciona la armature del Avatar y en ***Bone*** selecciona el bone que quieres que siga el prop. Para mantener la posición original del prop, haz clic en ***Set Inverse*** una vez que añadas el constraint. Si el influence es 1, el prop seguirá por completo el bone seleccionado; si es 0, el constraint se desactivará. Puedes establecer keyframes en el influence para activarlo y desactivarlo a lo largo de la animation. Para hacerlo, solo pulsa I mientras el cursor esté sobre ***Influence***.

***Child of** menú de constraint. Haz keyframe del influence para activarlo y desactivarlo.*


> [!NOTE]
> **💡Consejo de animación!** Si usas el deslizador para desactivar el Influence, el prop no mantendrá su posición anterior, lo que dificulta mantener la animation fluida. Para evitar tener que corregir manualmente la posición, en lugar de usar el deslizador, haz clic en la X junto a Influence, establece un keyframe en ella y otro en todos los atributos de transform. Así, el prop conservará la misma posición que cuando el Influence estaba activado!



> [!NOTE]
> **💡Consejo de animación**
> 
> ¡No dejes el prop visible desde el principio! Para evitar revelar lo que está a punto de suceder y una transición abrupta, empieza la animation con el prop escalado a 0.001 y cámbialo a 1 solo cuando quieras que aparezca. Recuerda volver a escalarlo a 0 al final de la acción. ¡Esto hará que las transiciones sean mucho más fluidas y geniales!


### Animation Slots

Blender 4.4 introdujo una nueva función: animation slots. Según la documentación de Blender, “el propósito de los slots es permitir que un action almacene datos de animation distintos para múltiples data-blocks”. En pocas palabras, los slots hacen posible almacenar la animation de varias cosas en el mismo Action. ¿Cómo afecta esto a los emotes 2.0?

Nueva función de Blender 4.4: animation slots.

Aunque es posible que tanto el Avatar como el prop compartan el mismo action clip, debido a la convención de nombres y al número de animation clips involucrados en Emotes 2.0, no funcionará. Así que el flujo para esto sería:

1. Crea un animation clip para el Avatar, o renombra el que se proporciona (***Starting\_Pose***). Ya tiene un animation slot, pero siéntete libre de usarlo (***Avatar\_Animation***) o crear uno nuevo.
2. Renombra el animation clip ***AnimationName\_Avatar***
3. Crea un animation clip para el prop y renómbralo ***AnimationName\_Prop***
4. Haz clic en ***New*** botón para crearle un animation slot (recibirá un nombre automático: ***Armature\_Prop***)
5. Anima como lo harías en versiones anteriores de Blender.

Creando un action clip y un slot para la animation del prop.

## **NLA Tracks**

Para que todas las animations se exporten, los clips deben añadirse a las NLA Tracks. Asegúrate de que solo haya un animation clip para el Avatar y otro para el prop, **deben tener exactamente el mismo número de frames.**

En ***Object Mode***, selecciona la armature del Avatar, ve a ***Pose Mode***, selecciona el animation clip correspondiente en el menú Browse Action, haz clic en ***Action*** y luego en ***Push Down*** opción.

Luego, vuelve a ***Object Mode***, selecciona la armature del prop, ve a ***Pose mode***, selecciona el animation clip correspondiente en el menú Browse Action, haz clic en ***Action*** y luego en ***Push Down*** opción.

Enviando actions a las NLA tracks.


> [!WARNING]
> ⚠️ Ten cuidado al enviar actions hacia abajo. Asegúrate de seleccionar la armature deseada con la animation correspondiente. No cambies solo la animation y la envíes hacia abajo antes de seleccionar la otra armature, o de lo contrario asignarás dos actions a una armature y ninguna a la otra.


Las NLA tracks deberían verse así: una animation para cada armature.


> [!NOTE]
> **🔥 Consejo de optimización**
> 
> **Antes de este paso, asegúrate de hacer una copia de seguridad de tu proyecto.**
> 
> Si tienes distintos objetos para tus props, puedes unirlos en una sola mesh. Puedes hacerlo simplemente seleccionando los objetos y pulsando el atajo ctrl + J.
> 
> Esto ayudaría a reducir los draw calls en el juego, haciendo que el emote rinda mejor.
> 
> Ten en cuenta que esto no funcionará para las partículas, sin embargo.
> 
> *Selecciona los objetos y pulsa `Ctrl+J` para unirlos.*


## **Exportación**

Los Emotes 2.0 se exportan de la misma manera que los emotes comunes. Asegúrate de que solo sean visibles la armature del Avatar, la armature del prop y los prop meshes, y oculta todo lo demás.

Ten visibles solo la armature del Avatar, la armature del prop y el prop mesh para exportar.

Para exportar, ve a File > Export > glTF2.0 (.glb, .gltf)

Para los ajustes de exportación, despliega Include y en Limit to activa Visible Objects. Luego, despliega la pestaña Data, despliega Armature y activa Export Deformation Bones Only.

|   |   |
| - | - |

¡Pulsa Export y listo!

## **Añadir audio a los emotes**

### Formato y limitaciones para los clips de audio

* El formato correcto para exportar sonidos para tus emotes es `.mp3` y `.ogg`.
* El audio clip debe tener la misma duración que el emote.
* Aunque no hay limitación de tamaño para el audio, el emote con props y sonidos no puede superar los 3 MB.


> [!NOTE]
> **📔 Nota**: Si el emote tiene sonido (mp3 u ogg), debe comprimirse en un .zip junto con el .glb. Después de eso, solo arrastra y suelta el .zip en el Builder. Más detalles se pueden encontrar aquí: [Subir emote con sonido](/docs/creator/content-creator-es/wearables-y-emotes/gestionar-colecciones/uploading-emotes/#uploading-emotes-using-a-zip-file)



> [!NOTE]
> **💡 ¡Atención!** Ten en cuenta que los audio clips usados en el emote deben ser IP original (Propiedad Intelectual), tener los derechos de reproducción y cumplir con los [Política de contenido](https://decentraland.org/content/)criterios.


### Editar sonidos

Para añadir sonidos a tus emotes puedes hacerlo de distintas maneras:

1. **Edita tus sonidos directamente en Blender**

Una forma de añadir sonidos a tus emotes es usando el editor Video Sequencer que ofrece Blender.

Para empezar a añadir sonidos, ve a *Editor Type > Video Sequencer.*

Arrastra y suelta tus sonidos en la interfaz de canales.

Pulsa el atajo `N` para ver más opciones para gestionar tus sonidos, como mostrar la forma de onda, poner tus sonidos en Mono o cambiar el volumen.


> [!NOTE]
> Si quieres hacer un fade in y fade out, puedes hacerlo simplemente añadiendo keyframes de 0 a 1 y viceversa a la propiedad volume.


Una vez que hayas terminado de editar tus sonidos, puedes exportarlos yendo a *Render > Render Audio*. En la opción de exportación necesitas seleccionar `.mp3` o `.ogg` el formato en la *Container* sección y luego *Mixdown*. **Solo se exportará el audio dentro del rango de frames.**

2. **Renderiza la animation y añade sonido con un software de edición de sonido**

Aunque editar sonidos directamente en Blender puede ser conveniente, no es muy flexible porque el software no está enfocado principalmente en la edición de sonido. Las herramientas disponibles son muy básicas. Si quieres darle un toque más profesional a tus sonidos, te recomendamos usar un software de edición de sonido dedicado de tu elección.

Hay varias opciones de software que puedes usar, como [Audacity](https://www.audacityteam.org/) (Gratis y de código abierto), Adobe Audition, Ableton Live o ProTools. Usar un software de edición de sonido dedicado te proporcionará una gama más amplia de herramientas, funcionalidades y efectos de sonido, lo que te permitirá mejorar tus sonidos y darles un toque más profesional.

Para renderizar tu emote, simplemente puedes añadir una cámara a tu escena de Blender y posicionarla de manera que puedas ver todos los elementos lo más claramente posible para luego tener una buena referencia para añadir sonidos.

Al renderizar un emote, es importante incluir solo el frame range de tu emote y no más. Elige una relación de aspecto que se adapte a tus necesidades y selecciona la carpeta de salida donde quieras que se guarde el video o la secuencia de imágenes.


> [!NOTE]
> **¡Pista!**
> 
> *Antes de renderizar, asegúrate de hacer un render con bajo sampling para ahorrar tiempo en tu render!*


Una vez completado este paso, usa tu video como referencia para crear los sonidos correspondientes usando tu software de edición de sonido preferido. **Asegúrate de que la secuencia de video coincida con el framerate de la animation de 30 frames por segundo (fps)**


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/wearables-y-emotes/emotes/props-and-sounds.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
