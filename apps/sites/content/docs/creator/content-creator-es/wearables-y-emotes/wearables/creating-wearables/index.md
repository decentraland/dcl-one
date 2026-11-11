> Consejos y guías para crear Wearables de Decentraland

# Creando Wearables

![](/docs/_remote/a3778599667aedc8c1fc29ed.webp)

## Introducción

Esta guía presenta los conceptos básicos para crear modelos 3D personalizados para wearables de Decentraland. Explica cómo funciona el sistema de avatar de Decentraland e ilustra cómo modelar correctamente tus propios wearables.

*Nota: esta guía asume que ya tienes conocimientos básicos a intermedios de modelado 3D. Si eres nuevo en el modelado 3D,* [*empieza aquí*](/docs/creator/3d-modeling/3d-models/)*.*


> [!NOTE]
> **💡 Consejo**: Instala el [plugin de Blender Decentraland Tools](https://extensions.blender.org/add-ons/decentraland-tools/). Incluye varias funciones útiles para ayudarte a editar y exportar modelos 3D, wearables y emotes.


Antes de empezar, descarga los archivos de ejemplo para referencia de meshes y textures: [**Modelos de referencia de wearables**](https://drive.google.com/drive/u/1/folders/12hOVgZsLriBuutoqGkIYEByJF8bA-rAU)

## El sistema de avatar de Decentraland

El "sistema de avatar" de Decentraland es el amplio conjunto de diferentes componentes corporales y subcomponentes que pueden decorarse con wearables personalizados. Estos componentes son:

* Body shape
* Cabeza
  * Head shape
  * Eyebrows
  * Eyes
  * Mouth
* Upper body
* Lower body
* Handwear
* Feet
* Accesorios

### **Base Body Shape**

Después de descargar el archivo de ejemplo del avatar base, carga el modelo en tu editor 3D, como Blender.

Notarás que cada modelo contiene 8 meshes diferentes relacionados con un armature. Estas meshes representan la cabeza, Eyebrows, Eyes, Mouth, Upper body, Lower body, las manos y Feet. Puedes usar estos modelos de ejemplo como una referencia y un punto de partida para tu propio wearable personalizado.

Actualmente, hay dos body shapes: A o B. Shape A (`AvatarShape_A.blend`) es el **cuerpo femenino** body, usado para la `BaseFemale` Shape B (`AvatarShape_B.blend`) es el **cuerpo masculino** body, usado para `BaseMale`. Los nombres de archivo no dejan esto claro, así que compruébalo bien antes de construirlo: un wearable ajustado al cuerpo construido sobre el archivo incorrecto termina con sus dos representaciones intercambiadas.

![](/docs/_remote/15019e8948fa233ffdf5f7bb.webp)

### **Cabeza**

![](/docs/_remote/1108d14609f042d64adb1c53.webp)

*La cabeza base incluye diferentes meshes adjuntas que pueden personalizarse como wearables: Eyebrows, Eyes y Mouth funcionan como máscaras de transparencia renderizadas frente al rostro.*

### **Upper Body**

![](/docs/_remote/9ce8e60a3286071a888c6bd9.webp)

*La parte superior del cuerpo, o torso, de un avatar. No incluye las manos.*

### **Lower Body**

![](/docs/_remote/f12c5382a0abf31077a0dfda.webp)

*La parte inferior del cuerpo incluye la pelvis y las piernas de un avatar.*

### **Hands**

![](/docs/_remote/c61c01c445051846535b37fd.webp)

*Las Hands son iguales para Shape A y B, y comienzan en las muñecas de un avatar.*

### **Feet**

![](/docs/_remote/4e09b05d153e3455e46bc344.webp)

*Feet incluye los tobillos y el pie.*


> [!WARNING]
> **Importante: no modifiques los vértices "cuts/stitches" entre la cabeza, la parte superior e inferior del cuerpo.**


Cada parte del cuerpo tiene tapas, lo que las hace "estancas". Estas tapas existen para evitar fallos visuales antiestéticos si hay problemas de clipping de la animación debido a un mal skin weighting. Lo mejor es no eliminar estas tapas al editar el mesh.

![](/docs/_remote/6d31c3917522878384fcbbf7.webp)

## Creación de modelos 3D para wearables

#### **Límites de tris, materials y textures**

Para garantizar que Decentraland funcione sin problemas para todos los jugadores, es importante crear modelos de wearables sin usar demasiados triangles, materials y textures. El objetivo es mantener los modelos 3D lo más simples posible para que puedan renderizarse fácilmente, sin sacrificar demasiado detalle.

Hay límites para el número de triangles y textures que se pueden usar para cada wearable o accesorio:

* No más de 1.5K triangles por wearable slot: hat, helmet, upper body, lower body, feet y hair.
* No más de 500 triangles por categorías: mask, eyewear, earring, tiara, top\_head y facial hair.
* Para los hand accessories, el presupuesto es de 1k tris. Si el hand wear oculta la mano base del avatar, el presupuesto es de 1.5k tris.
* No más de 2 textures (a una resolución de 1024x1024px o inferior) por wearable. Todas las textures deben ser cuadradas con resolución de 72 pixel/inch.
* No más de 2 materials (sin contar el AvatarSkin\_MAT)
* En el caso del skin wearable, la cantidad de tris permitidos es de 5k y 5 textures.


> [!WARNING]
> **Combinador de tris de wearables:**
> 
> *Si el wearable oculta otros wearables, al creador se le permite combinar los tris por slot. Por ejemplo: si quieres hacer un mono podrías crearlo usando la categoría upper body ocultando lower body; en ese caso podrías tener 1.5K*2= 3K triangles.\*
> 
> En el caso del helmet, si ocultas todos los wearables de la cabeza (head, earrings, eyewear, tiara, hat, facial\_hair, hair y top\_head, puedes alcanzar los 4k tris, 2 materials y 2 textures)


#### Dimensión máxima de ancho, alto y profundidad de los wearables

Hay un límite de distancia para los wearables para garantizar que no obstruyan la visibilidad de las pantallas de otros jugadores ni invadan el espacio de la scene de manera excesiva.

La dimensión de los wearables no puede exceder:

> `Alto: 2.42 m`, `Ancho: 2,42 m`, `Profundidad: 1,4 m`

![](/docs/_remote/0c4ec0aad4c553469f608e3f.webp) ![](/docs/_remote/74d98bf9b871410fa2fac083.webp)

#### **Mapas**

Los wearables de Decentraland actualmente admiten 3 tipos de maps, que son:

1. **Base Color**: Esta es la textura principal con los colores y detalles de tu modelo.
2. **Emission**: Este mapa es para las partes que brillan en tu modelo. El emission map va en un material separado específicamente para emission.
3. **Alpha**: Este mapa sirve para manejar la transparencia. Usa el canal de opacidad de la texture. (*Siempre es preferible usar Alpha Clip en lugar de Alpha Blend, utilizando una textura en blanco y negro para el recorte*)

![](/docs/_remote/d9bad6330bf2adc9dd0f39cd.webp)

Como el cliente de referencia de Decentraland usa un Toon Shader para los materiales del avatar, algunos maps son **innecesarios** como:

* **Normal maps:** texturas usadas para simular detalles de alta resolución en modelos de baja cantidad de polígonos mediante la codificación de las normales de la superficie como valores RGB.
* **Roughness maps:** texturas usadas para definir la rugosidad de la superficie de objetos 3D.

Para usar estos maps en Decentraland, la solución es hornearlos en una sola texture. Puedes encontrar más información sobre cómo hornear textures aquí: <https://docs.blender.org/manual/en/latest/render/cycles/baking.html>

#### **Normales**

**El engine de Decentraland renderiza solo normales de un lado.** (Eso significa que un plano solo será visible desde un lado; el otro lado no se renderizará). Así que, para asegurarte de que tu 3D sea absolutamente correcto con las normales, podemos comprobarlo de dos maneras distintas.

La primera es activar "Backface culling" en los ajustes de propiedades de Material; esta es una buena práctica para detectar normales invertidas, como en esta imagen:

![](/docs/_remote/0c50bdc39625df77edb130ed.webp) ![](/docs/_remote/19bd475725e7d546031c764d.webp)

La segunda forma de comprobar si las normales son correctas es activando "Face orientation" en las opciones de superposición del viewport. Esto pondrá tu modelo azul, pero no te preocupes. Las caras azules son las correctas y las rojas son las que necesitan corregirse; puedes encontrar esta opción aquí:

![](/docs/_remote/05e89b0f38d24052c2543b41.webp)

#### **Armature**

Recuerda no cambiar ninguna de las especificaciones, convenciones de nomenclatura, jerarquía ni transforms del armature proporcionado. Cambiar cualquiera de estas hará que el wearable deje de funcionar en el client después de exportarlo.

![](/docs/_remote/8d4e69ae0e0d37a5a75dd0ec.webp)

Asegúrate de que el armature importado no tenga *end* o *neutral* bones; de lo contrario, el wearable no va a funcionar después de exportarlo a Builder. Si estás importando un .fbx que tiene este problema, puedes activar ***Ignore Leaf Bones*** al importar el armature.

![](/docs/_remote/9bb431a3abead38036195f21.webp)


> [!NOTE]
> **Spring Bones:** Los wearables pueden incluir bones adicionales más allá del armature base para física dinámica (por ejemplo, cabello que se balancea, pendientes colgantes). Estos bones deben contener `springbone` en su nombre. Consulta [Spring Bones](/docs/creator/wearables-and-emotes/wearables/spring-bones/) para ver todos los detalles.


#### **Eyebrows, Eyes and Mouth**

Estos meshes funcionan con un shader transparente, así que no tienes que hacer nada aparte de crear tu propia textura png para el nuevo estilo de Eyebrows, Eyes o Mouth que quieras y colocarla correctamente en el UV map. Estas textures deben ser de 256x256px y necesitan tener un canal alfa para la transparencia.

Aquí hay algunas texturas png de ejemplo:

![](/docs/_remote/5574fe5585ace1ba7b8fc724.webp) ![](/docs/_remote/65a2cf2f85f2d9c2eceaf72b.webp) ![](/docs/_remote/a7245593b361bbcdb12ba303.webp)

*Eyes y Eyebrows usan el mismo mesh y UV map.*

![](/docs/_remote/769020e9eaad71f8b782e641.webp) ![](/docs/_remote/2b2a15a2fadd5b4fa790102b.webp)

*Mesh y UV map de Mouth.*

Para visualizar el resultado final necesitarás usar estos nodes (en Blender):

![](/docs/_remote/477f9ad97ab6c5847c84ba3a.webp) ![](/docs/_remote/7205c2f5e0ce6b631e182dbf.webp)

**Masks:** El Avatar Editor tiene diferentes opciones de color que los jugadores pueden elegir para personalizar sus avatares.

![](/docs/_remote/689357b1313432170c554166.webp)

Estas opciones de color se aplican a una máscara específica en el wearable.

![](/docs/_remote/859d860ec5e8ae3859344474.webp) ![](/docs/_remote/bc69b3aeb4af2645c8f66ddd.webp)

La zona negra en la imagen de la izquierda (Eyes Mask) indica el área de la texture de la derecha (Eyes Base) que será coloreada. Es importante recordar que los iris siempre deben tener una escala de grises (si el iris es negro puro, el tint no va a funcionar. Por el contrario, si el iris es blanco puro quedará totalmente tintado por el color seleccionado usando el editor).

#### **Handwear**

Hay dos tipos de **Handwear** que puedes hacer dentro de la misma categoría.

1. **Reemplazo de manos:**

![](/docs/_remote/9affc36f5fddb68dcafbd2b3.webp)

Si quieres crear handwear que reemplace ambas manos, probablemente tendrás que sobrescribir la ***mano*** base mesh. Al hacer eso, el límite para el wearable de handwear será de 1500 tris.

2. **Accesorios de mano:**

![](/docs/_remote/f9dc813cad31b498851c90f6.webp)

También puedes crear accesorios de mano como relojes, pulseras, anillos, etc. En ese sentido, la mano no necesita ser sobrescrita y el límite es de 1000 tris por artículo.


> [!WARNING]
> Nota: la **handwear** categoría es para accesorios de mano, objetos sostenidos en la mano (como herramientas o props) y objetos que reemplazan las manos. Todos ellos deben seguir el armature del avatar con el skinning adecuado. Enviar un artículo que no siga estas especificaciones puede dar lugar al rechazo por parte del comité de curadores.


#### **Cabello y vello facial**


> [!TIP]
> **Consejo:** El cabello y otros accesorios colgantes ahora pueden usar **spring bones** para moverse dinámicamente en respuesta al movimiento del avatar y la gravedad. En lugar de cabello estático, puedes agregar cadenas de spring bone para hacer que colas de caballo, trenzas y cabello largo se balanceen de forma natural. Consulta [Spring Bones](/docs/creator/wearables-and-emotes/wearables/spring-bones/) para ver una guía completa.


Hay dos cosas importantes que recordar al crear wearables personalizados de cabello.

Primero, intenta seguir la forma de la cabeza. Siempre puedes consultar el mesh de la cabeza proporcionado en los archivos de ejemplo si necesitas un punto de partida.

![](/docs/_remote/370336fb19510d9325816b5a.webp)

Segundo, si quieres que los usuarios puedan cambiar el color del cabello o del vello facial usando el Avatar Editor, entonces necesitas pintar el cabello en escala de grises y usar "Hair" en el nombre del material (por ejemplo, "M\_Hair\_Short"). Si quieres incluir otro objeto que no esté influido por el tint, simplemente no añadas esa convención de nombres.

![](/docs/_remote/6c65a6133c0e05d828652d8e.webp) ![](/docs/_remote/cb8f9f3d94259277831a5b96.webp) ![](/docs/_remote/4b874f015973cb5a36ecc963.webp)

*Los tonos más oscuros de gris aparecerán más oscuros y los tonos más claros de gris aparecerán más brillantes, multiplicados por el color seleccionado por el usuario en el Avatar Editor.*

### **Materiales y texturas base**

Hay tres materiales básicos para los modelos de avatar. Uno es el material utilizado para el wearable en sí, otro se usa para la skin y otro para las eyebrows, eyes y mouth.

![](/docs/_remote/d83fc01227d8770cbf81986b.webp)

Cada mesh base viene con su propia skin texture.

![](/docs/_remote/930d22b040b6df838d2d26f2.webp) ![](/docs/_remote/8dfa78f550fa2b8b6f32dfd0.webp)

La skin texture está hecha en escala de grises, por lo que permite al motor de renderizado tintar la skin del avatar usando el editor según la preferencia del usuario. Para poder tintar el color de la skin usando el editor, el nombre del material debe ser *AvatarSkin\_MAT*.

![](/docs/_remote/51dce26a37033db0987f4cbe.webp)


> [!WARNING]
> Importante: conserva siempre el UV mapping de cualquier parte del cuerpo que quede expuesta por un wearable, como las piernas expuestas por los shorts o las faldas.


\src="../images/wearables-and-emotes/creating-wearables/19\_skin\_uv.png"width="600"/>

¡Puedes crear texturas personalizadas para tus wearables! Sin embargo, siempre es mejor usar un único archivo de textura muy pequeño para cada wearable. Usar la textura AvatarWearable\_MAT predeterminada proporcionada en los archivos de ejemplo garantizará que tus wearables tengan buen rendimiento.

![](/docs/_remote/d821fd219afb4cca8fe745d3.webp)


> [!WARNING]
> Para evitar activar shaders específicos de rasgos faciales, no incluyas '\_mouth,' '\_eyebrows,' ni '\_eyes' en el nombre de ningún mesh. Estos términos están reservados para los rasgos faciales, y usarlos incorrectamente aplicará el shader equivocado al mesh.


✨ Si quieres hacer tus propias textures para el modelo, recomendamos los siguientes addons para un mejor UV Unwrapping:

**UVPacker**

UvPacker es un addon gratuito que te ayuda a empacar y organizar tus uvs con solo unos pocos clics. Como Decentraland trabaja con textures de hasta 1024x para los wearables, esta herramienta es una gran ayuda para crear textures mejores y más organizadas.

Puedes descargarlo directamente desde el sitio web aquí:

[**https://www.uv-packer.com/download/**](https://www.uv-packer.com/download/)

**UVToolKit**

UvToolKit es un addon que te ayuda a ampliar tus ajustes y opciones de UV para crear excelentes UVs en Blender.

Aquí está el enlace de descarga: [**https://alexbel.gumroad.com/l/NbMya**](https://alexbel.gumroad.com/l/NbMya)

### **Skin Weighting**

Skin weighting es el proceso de determinar qué bones en el rigging del avatar afectan a qué wearables durante una animación.

Al hacer skin weighting de nuestros nuevos wearables, hay varias consideraciones que debemos tener en cuenta.

Cada asset debe estar weighted para el esqueleto completo. Por ejemplo, un asset de upper body se verá así al aplicar los skin weights:

![](/docs/_remote/18e214351b78092a3436da29.webp)

Los wearables que se unen en intersecciones entre partes del cuerpo deben estar completamente weighted al mismo bone. Por ejemplo, en estas dos zonas verdes, los vértices del cuello deben estar completamente weighted solo al bone "Neck".

![](/docs/_remote/e5d072fd17af31865e1e1c4a.webp)

#### **Bones clave**

Los bones "clave" que se usan al hacer skin weighting son:

**Bone de la cabeza:** para el cabello, los pendientes, las tiaras, los ojos, las cejas, la boca y cualquier accesorio que deba seguir el movimiento de la cabeza.

**Bone del cuello:** para los vértices de intersección entre la cabeza principal y la parte superior del cuerpo.

**Bone de las caderas:** para los vértices de intersección entre la parte superior e inferior del cuerpo.

**Bones de la pierna derecha y la pierna izquierda:** para los vértices de intersección entre la parte inferior del cuerpo y los pies.

**Antebrazo derecho y antebrazo izquierdo** para los vértices de intersección de las manos.


> [!WARNING]
> ⚠️ **Consejo**
> 
> * Recuerda que puedes usar cualquier bone para influir en los vértices de cualquier mesh. Por ejemplo, podrías crear un nuevo mesh de pie para un par de botas altas y asignar pesos de skin a la parte superior de la bota al bone "Leg Bones". O podrías crear cabello largo y usar los bones "Shoulder" o "Spine" para influir en el cabello cuando el avatar se mueve.
> * Siempre es recomendable mantener una simetría en ambos lados del rig; el lado izquierdo y el derecho deberían tener influencias óseas similares.
> * Como consejo general, un vértice no puede estar influido por más de 4 bones o joints.
> * Ten en cuenta exportar el armature exactamente como el que se proporciona en la documentación. Si tiene otros bones como *"\_end\_bones"* o similares, no va a funcionar en el client.


#### **Partes rígidas**

Para las partes que no deberían doblarse, como una mochila, un jetpack o placas de armadura, asigna a cada vértice de la parte un 100% a un solo bone. Por ejemplo, usa `Avatar_Spine2` para cualquier cosa en la espalda. Luego une la parte al mesh del wearable, de modo que el wearable siga siendo un único mesh skinned.

`Avatar_Spine2` está en el mismo lugar en ambas body shapes, así que una parte weighted a él puede usar la misma posición en ambas.

Una parte rígida en la espalda puede parecer que flota, porque la espalda se curva alejándose de una parte plana. Mide el espacio bajo toda la huella de la parte y coloca su cara frontal a unos 4 mm del punto más lejano de la espalda. Como alternativa, añade un pequeño bloque de montaje entre la parte y la espalda, y pondera los vértices frontales del montaje igual que los vértices del cuerpo con los que se tocan, para que el montaje se doble con la spine.

#### **Exportando wearables**

Al exportar wearables, asegúrate de que no haya otros bones fuera del Armature dado, **excepto los spring bones** (bones with `springbone` en su nombre, usados para física dinámica). Un problema común al importar armatures entre distintos software es la aparición de *"\_end"* o *"\_neutral"* bones. Asegúrate de eliminarlos antes de exportar. De lo contrario, es muy probable que los wearables no funcionen luego en el client.

![](/docs/_remote/c6477cfc83e060c6483735ac.webp)

Para exportar el wearable, selecciona el objeto y luego el armature. Asegúrate de no exportar nada más, como cameras, lights u objetos vacíos.

Un wearable válido hecho de un solo mesh tiene 64 nodes en su archivo exportado: el `Armature`, sus 62 bones y el mesh skinned. Cada spring bone añade un node.

Luego, exporta el wearable en *glTF2.0* formato. Asegúrate de exportar solo el wearable con sus propiedades de skinning, y sin otras características innecesarias como animación o shape keys.

![](/docs/_remote/f71be0c8670ef1406a96e0f4.webp)

## Buenas prácticas para modelar

### **Cambia tu mesh de T-Pose a A-Pose**

Cuando haces wearables, la mejor manera de visualizar el resultado final y facilitar cómo manejas la topología y la posición del wearable es trabajar con el modelo en A-Pose. Para hacerlo, debes seguir estos pasos simples:

1. Primero selecciona la parte superior del cuerpo, luego tienes que activar "**Edit mode**" y "**On cage**" en el modificador del armature.

![](/docs/_remote/9dd9e778254d10a239d518c8.webp)

2. Ahora, en pose mode puedes rotar los brazos 60°.

![](/docs/_remote/41758037910791f5efcffe51.webp)

3. Puedes editar tu mesh en A-Pose en lugar de T-Pose.

![](/docs/_remote/71276e7651ff3507da8ee9fe.webp)

4. Pero también es bueno tener en cuenta que puedes alternar fácilmente entre A-Pose y T-Pose simplemente volviendo a activar "Edit mode" y "On cage" en el modificador del armature.

![](/docs/_remote/a731a1c3b9056b0a3f73203e.webp)

### **Deformación de las articulaciones:**

Para obtener los mejores resultados en la topología del wearable cuando se trata de articulaciones (brazos o piernas, por ejemplo), es importante tener buenas prácticas al crear loops. Aquí podemos ver la diferencia en la deformación del mesh para diferentes loop cuts:

![](/docs/_remote/c18bb25e36a973c241938296.webp)

Una buena manera de asegurarte de que todo se deforma correctamente es hacer un weight paint como en el siguiente ejemplo:

![](/docs/_remote/0e4eb4a84b57c9c5a3f1d079.webp) ![](/docs/_remote/a6d3efa7b12801dd5b29ed8b.webp)

### **Faldas**

Un consejo útil y una buena práctica al modelar faldas/vestidos es añadir loopcuts adicionales en las intersecciones de los pliegues. Esto será muy útil cuando tengas que pintar los pesos del rig.

![](/docs/_remote/20d43358a97c94b9db32014e.webp)

Con estos loopcuts la influencia de los vértices se ve mucho más suave y te da mejores resultados cuando animas una falda/vestido.

Aquí tienes un ejemplo de cómo debería ser la influencia de los bones:

![](/docs/_remote/3a762bb515429e86217673cd.webp)

### **Sombreros**

Una buena práctica al crear sombreros es añadir un hair a la base mesh del sombrero y luego ocultar la categoría *hair* usando el editor. Hacer esto evitará que el sombrero haga clipping con otros hairs y reducirá resultados inesperados.

![](/docs/_remote/52f717e012928949c63883f0.webp)

### **Añadir recuento de polígonos**

Un consejo valioso es llevar siempre un control del polycount de tus modelos. Para hacerlo en Blender debes activar las estadísticas en el panel de superposiciones del viewport.

![](/docs/_remote/444dbdc043e94bf60c83af44.webp)

### Recursos

En esta carpeta compartida puedes encontrar modelos base, texturas y varios otros recursos, incluidos ejemplos de wearables completamente creados. Siéntete libre de aprovechar estos recursos al crear los tuyos propios.

[**Modelos de referencia de wearables**](https://drive.google.com/drive/u/1/folders/12hOVgZsLriBuutoqGkIYEByJF8bA-rAU)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/wearables-y-emotes/wearables/creating-wearables.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
