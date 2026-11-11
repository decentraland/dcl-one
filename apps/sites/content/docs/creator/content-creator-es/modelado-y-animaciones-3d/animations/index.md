> Aprende cómo crear animaciones que se puedan incrustar en modelos 3D importados a Decentraland.

# Animaciones

La animación es el arte de dar vida a las cosas. Y no hay mejor manera de hacer que tu escena sea más viva que añadiendo algunas animaciones a tus modelos 3D.


> [!NOTE]
> **💡 Consejo**: Instala el [Decentraland Tools Blender plugin](https://extensions.blender.org/add-ons/decentraland-tools/). Incluye varias funciones útiles para ayudarte a editar y exportar modelos 3D, wearables y emotes.


Hay varias maneras de hacerlo: mediante **animación de objetos**, mediante un **rig (animación esquelética)**, o usando **shape keys (animación de morph target)**.

**Animación de objetos** es la mejor para modelos simples, como una pelota que rebota, un globo giratorio o una silla flotante, y no necesita una armature. Es importante mencionar que la animación de objetos es diferente de **la animación de vértices**. En la animación de objetos, el modelo se animará como un todo, mientras que en la animación de vértices cada vértice del objeto puede animarse por separado.

**Shape keys (también conocidas como morph targets o blend shapes)** son una forma de animación de vértices que **son compatibles con el engine de Decentraland**. Son especialmente útiles para expresiones faciales, lip sync o cualquier deformación que sea difícil de lograr solo con bones. Las animaciones de shape keys incrustadas en un archivo glTF/GLB se reproducirán in-world igual que las animaciones esqueléticas o de objetos.

Si tienes un modelo más complejo, como una persona, una criatura o maquinaria, entonces necesitarás un **rig**. Un rig no es más que un esqueleto digital que moverá y deformará el mesh. El proceso de vincular el mesh al esqueleto se llama Skinning, donde defines qué bone afectará a cada grupo de vértices y qué tan fuerte será esa influencia, asegurándote de que el modelo se deforme de la mejor manera posible.

Para cualquiera de los dos métodos, sin embargo, todas las animaciones de un modelo 3D deben estar incrustadas dentro de su *archivo glTF* porque no puedes referenciar animaciones en archivos separados.

## Creación de una animación

Así es como creas animaciones usando Blender.

### Animación de objetos

* Asegúrate de tener la ***Dope Sheet*** > ***Action Editor*** pestaña abierta y haz clic en ***New*** para añadir un clip de animación.
* Renómbralo como te parezca y asegúrate de activar ***Fake User*** (el icono del escudo) para que tu animación se guarde.
* Establece un keyframe en el primer frame seleccionando el objeto y presionando ***I***. Luego mueve el objeto, gíralo o escálalo y, en otro frame de la línea de tiempo, presiona ***I*** de nuevo para establecer otro keyframe con las transformaciones actuales.
* El frame final de la animación debe ser igual al primero, así que simplemente copia el primer frame y pégalo en el último.

![](/docs/_remote/da0a0df92981981477fc8dbd.webp)

*Creando un clip de animación y añadiendo keyframes.*

### Animación de rig

Para una animación de rig, necesitarás una armature. Si quieres hacerlo tú mismo, consulta [Crear un Rig](https://www.notion.so/686e6f59a1604585b059f990a36b2d55) para obtener instrucciones sobre cómo configurar una armature consistente.

Cuando el rigging está hecho y el skinning produce buenas deformaciones, ¡ya estás listo para empezar tu animación! El proceso es muy similar al de la Animación de objetos, pero en lugar de hacerlo en ***Object Mode***, la animarás en ***Pose Mode***.

* Ve a ***Pose Mode*** y asegúrate de tener la ***Dope Sheet*** > ***Action Editor*** pestaña abierta y haz clic en ***New*** para añadir un clip de animación.
* Renómbralo como te parezca y asegúrate de activar ***Fake User*** (el icono del escudo) para que tu animación se guarde.
* Con el ratón sobre el ***Viewport***, presiona ***A*** para seleccionar todo y luego ***I*** para establecer un keyframe en toda la armature.
* Ve a otro frame en la línea de tiempo y manipula el bone como quieras para obtener la pose que deseas. Puedes cambiar la ubicación, la rotación y la escala de un bone dependiendo de cómo hayas configurado tu rig.
* Cuando tengas un buen resultado, establece otro keyframe para toda la armature. Sigue así hasta terminar tu animación. Recuerda que el primer y el último frame deben ser iguales si la animación va a repetirse en bucle.

![](/docs/_remote/aa4ecd75ce5fa54f9c60066e.webp)

Si eres nuevo en el proceso de animación, mira este tutorial con buenos consejos para principiantes:

[![Video Preview](/docs/_remote/629c65271f6a14386ffd42f7.webp)](https://youtu.be/-iWslh4uQIk)

## Animaciones skinned vs animaciones de Transform

**Las animaciones de Transform suelen ser mejores que las animaciones de skinned mesh en términos de rendimiento porque implican menos sobrecarga en cuanto a recursos computacionales necesarios para mostrar la animación.**

La animación de skinned mesh implica usar un mesh con un skeleton de bones que están ponderados a los vértices del mesh. El skeleton se anima, lo que a su vez anima el mesh. Este proceso puede ser bastante costoso a nivel computacional, especialmente cuando hay una gran cantidad de vértices que animar.

Las animaciones de Transform, por otro lado, implican animar directamente las propiedades de transform de un objeto (como su posición, rotación y escala). Esto puede hacerse con menos recursos computacionales que la animación de skinned mesh, ya que normalmente hay menos propiedades de transform que animar que vértices en un mesh.

Además, las animaciones de Transform a menudo pueden precalcularse y almacenarse en un formato más compacto, como datos de keyframes, que pueden accederse y reproducirse rápidamente sin necesidad de cálculos intensivos en tiempo real.

En general, aunque la animación de skinned mesh puede producir animaciones más detalladas y realistas, las animaciones de Transform suelen preferirse en aplicaciones críticas para el rendimiento, como los videojuegos.

## Creación y exportación de múltiples clips de animación

Puedes tener tantos clips de animación como quieras para tu modelo. Puedes consultar cómo crear, explorar y eliminar animaciones en [esta sección aquí](https://www.notion.so/5e962e5d54a24bcb9b906748007eb4cc).

En el video de abajo, aprenderás a exportar múltiples animaciones desde Blender en un solo archivo GLB.

[![Video Preview](/docs/_remote/eb23ea78b0ffa9a0778e60d6.webp)](https://www.youtube.com/watch?v=YxAB4bujO_w&ab_channel=Decentraland)

## Exportando muchas animaciones como una sola

A diferencia de una animación esquelética, donde puedes riggear múltiples objetos y combinar sus animaciones en un solo clip de animación, en la animación de objetos no puedes hacer eso. Las animaciones se exportarán por separado y solo un objeto se reproducirá a la vez. Una solución para eso es fusionar muchas animaciones en una sola, de modo que tengas un clip de animación que tenga todos los objetos moviéndose al mismo tiempo.

Para hacerlo, envía hacia abajo al ***Nonlinear Animation Track*** la animación de cada objeto, como se muestra abajo.

![](/docs/_remote/03413441d7a33b6e5298b18f.webp)

*Envía la animación de cada objeto al NLA Track.*

Una vez que todas las animaciones estén listadas en el NLA Editor, selecciona todos los objetos, ve a **File > Export > glTF2.0**. Expande Include y marca Selected Objects. Expande Animation, vuelve a expandir Animation y desmarca **Group by NLA Track**. Puedes renombrar el clip de animación en Merged Animation Name y luego solo presiona **Export glTF 2.0**.

![](/docs/_remote/9ba538ff6074491ba9311c8c.webp)

*para exportar varias animaciones como una sola.*

Ten en cuenta que esto solo funcionará si los objetos tienen una sola animación cada uno. Si los objetos tienen múltiples clips de animación, es mejor exportarlos por separado.


> [!NOTE]
> **💡 Consejo**: en lugar de crear tus propias animaciones, también puedes descargar animaciones genéricas y aplicarlas a tu modelo. Por ejemplo, para personajes 3D con características humanas, puedes descargar animaciones gratuitas o de pago desde [Mixamo](https://www.mixamo.com/#/).


## Muestreo

A veces tu archivo de animación puede terminar siendo demasiado pesado debido a la cantidad de animaciones o a la combinación de animación+modelo.

El muestreo es una buena forma de optimizar la animación. La frecuencia de muestreo definirá con qué frecuencia se horneará un keyframe en la animación. Por ejemplo, si la frecuencia de muestreo se establece en 2, eso significa que se creará un keyframe cada dos frames. Una frecuencia de muestreo de 3 horneará un keyframe cada tres frames, y así sucesivamente. Cuanto mayor sea la frecuencia de muestreo, más ligero será el archivo.

Sin embargo, la desventaja es que la animación empezará a ser cada vez menos fluida, ya que pierde algunos keyframes importantes (se distribuyen por la animación de forma desigual). También es importante notar que **el muestreo NO consiste en dividir el número de frames de la animación por la tasa de muestreo**.

Normalmente, una **frecuencia de muestreo de 2 o 3** hará el truco. Esos números pueden optimizar la animación sin comprometer la calidad.

Puedes encontrar ***Frecuencia de muestreo*** en la configuración de exportación, en ***Animation***.

![](/docs/_remote/942221d28977584ca4c825da.webp)


> [!NOTE]
> **💡 Consejo**: si el número de frames de la animación puede dividirse por la tasa de muestreo, ¡eso es bueno! Significa que el frame final quedará horneado, preservando la transición del final al inicio de la animación.


## Implementación de animaciones

Este documento cubre cómo añadir animaciones a un modelo 3D. Consulta [**manejar animaciones**](/docs/creator/sdk7/3d-essentials/3d-model-animations/) para obtener instrucciones sobre cómo activar y manejar animaciones en una escena.

## Buenas prácticas para las animaciones

* Mantén la armature lo más simple posible, crea solo bones para las partes del modelo que quieras animar. Los bones pueden afectar el rendimiento, así que asegúrate de añadir solo lo que realmente sea necesario.
* Si la animación va a repetirse en bucle en tu escena, asegúrate de que la pose final sea idéntica a la inicial para una mejor transición.
* Nunca dejes a un personaje sin animación, incluso si en realidad no está haciendo nada. Crea una animación "idle" para cuando el personaje esté quieto. La idle puede incluir movimientos sutiles como respirar y quizá miradas ocasionales.
* No dejes bones sin posar, como manos, dedos, cabeza o cuello. Los detalles son realmente importantes en una buena animación y un movimiento rígido solo la hará menos creíble.
* Evita demasiados keyframes, a menos que sea extremadamente necesario. Cuantos más keyframes añadas, mayores serán las posibilidades de obtener una mala interpolación y una animación más pesada.
* Selecciona todos tus bones (incluidos los deformadores) y establece un keyframe en el primer y el último frame de tu animación. Esto evitará tener bones sin información, haciendo que una animación afecte a la otra sin querer.
* Asegúrate de que tu archivo solo tenga una armature al exportarlo. Al importar animaciones, también se importará una armature con ellas. Todas las animaciones deben ser realizadas por la misma armature base, así que borra todo lo que no necesites.
* Renombra siempre tus bones y animaciones. ¡Mantén todo organizado!
* Al exportar el *archivo glTF* modelo, confirma que estás exportando todos los objetos y animaciones. Algunos exportadores solo exportarán lo ***actualmente seleccionado*** por defecto.
* Después de exportar el modelo, revísalo en [Babylon Sandbox](https://sandbox.babylonjs.com/) y comprueba si todas las animaciones funcionan y tienen los nombres esperados.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/modelado-y-animaciones-3d/animations.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
