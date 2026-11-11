> Cómo exportar partículas en tus emotes usando Armature

# Partículas en emotes

Los Emotes 2.0 pueden tener sonidos y props, pero ¿sabías que también es posible usar partículas? ¡Pueden ser muy divertidas y añadir ese extra visual! Y además también pueden acelerar el proceso de animación.


> [!NOTE]
> **💡 Consejo**: Instala el [Decentraland Tools Blender plugin](https://extensions.blender.org/add-ons/decentraland-tools/). Incluye varias funciones útiles para ayudarte a editar y exportar modelos 3D, wearables y emotes.


Pero dado que la animación de objetos y los sistemas de partículas no son compatibles con Emotes 2.0, necesitamos convertir estas partículas en un armature (que sí es compatible) para poder agregarlas a los emotes. Usaremos un add-on de Blender desarrollado por el Content Team de la Foundation para este proceso, que se puede descargar desde el enlace de abajo.

Usa la [Particles to Bones Add-on para Blender](https://github.com/decentraland/docs/blob/main/creator/images/emotes/particle_system_to_bone_animation.py) ¡para empezar!

![](/docs/_remote/8a0401cd85e912317d2b0e9e.webp) ![](/docs/_remote/86f127e163391ec5c9cc99a0.webp)

El proceso de agregar partículas a tu emote implicará dos pasos; puedes seguir las indicaciones junto con estos videos:

* **Convirtiendo partículas en armature**

[![Video Preview](/docs/_remote/26ae9b1db5276e225497fc37.webp)](https://vz-8a0704eb-552.b-cdn.net/bb51a195-4a80-48d3-badd-0bfe6cc9e3b3/playlist.m3u8)

* **Fusionando el armature de partículas con el armature del prop**

[![Video Preview](/docs/_remote/5a12aec98b02dff46c6ea7a5.webp)](https://vz-8a0704eb-552.b-cdn.net/9f512c34-ae83-42fe-a858-165ccea6d0c5/playlist.m3u8)


> [!NOTE]
> **¡Atención!**
> 
> ⚠️ La creación de un sistema de partículas no se cubrirá en esta documentación. El objetivo es ayudar a implementarlo en tu emote.


## Convertir Partículas en Armature

Como se mencionó anteriormente, Emotes 2.0 no admite la animación de objetos ni ningún tipo de sistema de partículas. Por eso necesitamos convertir las partículas en un armature.

Para empezar, necesitarás un sistema de partículas funcional y el add-on. El sistema de partículas que se muestra abajo es muy simple; es solo un ejemplo para mostrar cómo funciona el add-on. Asegúrate de que tu sistema de partículas coincida con las animaciones de tu emote para obtener un mejor resultado.

![](/docs/_remote/221d5be3906962eace9e413c.webp)

*Sistema de partículas simple.*

A continuación, descarga e instala el add-on. Para hacerlo, ve a Edit > Preferences > Add-ons y haz clic en Install para buscar el archivo en tu computadora.

![](/docs/_remote/7494e551d315ebd81e317174.webp)

*Haz clic en Edit > Preferences.*

![](/docs/_remote/455940b9766bf96f12df39e3.webp)

*Selecciona Add-ons a la derecha y haz clic en Install.*

Una vez hecho eso, haz clic en la casilla para habilitarlo. Aparecerá una pestaña llamada Converter en el lado derecho del Viewport.

![](/docs/_remote/4a04786944a55dc6ea9f1b36.webp)

*Haz clic en la casilla para habilitar el add-on.*

![](/docs/_remote/c89bb19680957280f6fbecbf.webp)

*Encontrarás la pestaña Converter a la derecha del Viewport.*

A continuación, selecciona el objeto, ve a la pestaña Converter, renombra la Output Collection como prefieras, define los frames de inicio y fin y haz clic en Particle System to Armature Converter, como se muestra abajo.

![](/docs/_remote/e7e1f6344eb6ec89ec8c469f.webp)

Notarás que se creó una collection en el Outliner, con el armature de partículas. También se creó un clip de animación, al hornear todo el movimiento de cada objeto.

![](/docs/_remote/d4067e87e6fe33342f19de70.webp)

*Armature generado por el add-on.*

![](/docs/_remote/b2b2e68b9a2d4d5dc47d2c10.webp)

*Clip de animación generado por el add-on.*

¡Y eso es todo para convertir tus partículas en un armature! Recuerda guardar tu archivo con un nombre diferente (mantén el original como respaldo), lo necesitaremos para el siguiente paso.


> [!NOTE]
> **¡Atención!**
> 
> ⚠️ Las partículas deben ser el último elemento que se agregue a tu emote. Cuando el emote esté listo, tendrás la duración exacta de la animación, lo que facilitará sincronizar las partículas con la acción realizada.


## Fusionando Partículas Armature con Prop Armature

Los Emotes 2.0 solo pueden tener dos armatures y dos clips de animación por archivo. Si simplemente importamos las partículas, terminaremos con tres armatures y tres clips de animación. La forma de solucionarlo es fusionar el armature de partículas dentro del armature del prop. La razón para fusionarlo con el prop y no con el armature del avatar es muy simple: el rig del avatar no se puede editar de ninguna manera añadiendo o eliminando bones. Eso deja al armature del prop como la única opción.

## Fusionando Partículas Armature con Prop Armature

Los Emotes 2.0 solo pueden tener dos armatures y dos clips de animación por archivo. Si simplemente importamos las partículas, terminaremos con tres armatures y tres clips de animación. La forma de solucionarlo es fusionar el armature de partículas dentro del armature del prop. La razón para fusionarlo con el prop y no con el armature del avatar es muy simple: el rig del avatar no se puede editar de ninguna manera añadiendo o eliminando bones. Eso deja al armature del prop como la única opción.

Lo primero que tenemos que hacer es importar o anexar la collection generada por el add-on. Siempre podrías exportar el archivo glb para ello e importarlo en tu archivo de emote. O simplemente podrías anexar la collection, lo cual es mucho más fácil y rápido, yendo a ***File*** > ***Append***, busca el archivo de partículas en tu computadora, selecciona la carpeta ***Collection*** y elige la collection deseada.

![](/docs/_remote/9ca230b3c18bf89586ff9347.webp)

*Para anexar una collection, ve a File > Append.*

![](/docs/_remote/1f0417578e1b9a2dcae76f3e.webp)

*Una vez que busques el archivo de partículas, selecciona la carpeta Collection.*

![](/docs/_remote/539369b8db786f772b98368b.webp)

*Elige la collection generada por el add-on (que pudiste renombrar). Particles\_Out es simplemente cómo se renombró para este ejemplo.*

A continuación, vamos a fusionar los armatures. En Object Mode, selecciona primero el armature de partículas, luego mantén presionada Shift para seleccionar el armature del prop y presiona Ctrl+J para unirlos/fusionarlos, como se muestra abajo.

![](/docs/_remote/c6adaaf3d6e33639b4ce3f4e.webp)

Una vez que hagas eso, las partículas pueden verse desordenadas, pero no te preocupes, eso forma parte del proceso. Para corregirlo, selecciona cada objeto de partícula en el Outliner uno por uno, ve a Object Properties (icono del cuadrado naranja) y, en Relations, verifica si el parent es Armature\_Prop. Haz esto con todos los objetos de partícula. Esto significa que todos los objetos son hijos de Armature\_Prop.

![](/docs/_remote/bed704b9a320a1befd208977.webp)

*Asegúrate de que Armature\_Pop sea el parent de cada objeto de partícula.*

Después de hacer eso con todos los objetos de partícula, selecciona el primer objeto de partícula en el Outliner, ve a Modifiers (el icono de la llave inglesa azul) y el campo Object estará vacío. Como esta es la pestaña Modifiers, eso significa que aquí elegirás qué armature controlará el objeto. Así que haz clic en Object y selecciona Armature\_Prop, y repite esto para todos los objetos de partícula. A medida que lo hagas, notarás que los objetos de partículas están volviendo a sus tamaños originales, corrigiendo todo ese desorden creado cuando se fusionaron los armatures.

![](/docs/_remote/39489288235ee5b1b5748410.webp)

*Selecciona el armature adecuado para controlar los objetos.*

Ahora que el armature está fusionado y configurado correctamente, queda un último paso para que esto funcione. Si intentas reproducir la animación, notarás que las partículas no se están moviendo. Y eso es porque ahora forman parte de Armature\_Prop, y la animación que se reproduce con él es la del prop. ¿Recuerdas que las partículas tenían su propio clip de animación? Lo que necesitamos hacer ahora es copiar los keyframes del clip de animación de partículas y pegarlos en el clip de animación del prop.

Para hacerlo, ve a ***Pose Mode*** y en el Outliner, expande ***Armature\_Prop*** y luego ***Pose***. Selecciona todos los bones que pertenecen a las partículas.

![](/docs/_remote/10184a7f4c04c033592867a1.webp)

*Seleccionando todos los bones de las partículas.*

Con los bones seleccionados, en el Action Editor, selecciona el clip de animación original de las partículas que tiene todos los keyframes. Selecciona todos los keyframes presionando A, haz clic derecho y copia los keyframes. Luego, selecciona la animación del prop, establece un keyframe en el primer frame y pega los keyframes del clip de animación de partículas.

![](/docs/_remote/b16528d9ab25ee97d3d824e0.webp)

*Copiando frames de la animación de partículas a la animación del prop.*

¡Y listo! Guarda el archivo, asegúrate de eliminar el clip de animación original de las partículas y exporta como lo harías normalmente.

Si necesitas más información sobre cómo hacerlo, consulta la ***Exportación*** sección en la [Agregar props y sonidos a tus Emotes](/docs/creator/wearables-and-emotes/emotes/props-and-sounds/) documentación.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/wearables-y-emotes/emotes/particles-in-emotes.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
