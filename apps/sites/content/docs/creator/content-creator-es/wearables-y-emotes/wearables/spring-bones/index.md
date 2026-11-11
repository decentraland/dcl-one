> Añade física dinámica a los Wearables con Spring Bones

# Spring Bones

### ¿Qué son los Spring Bones?

Los Spring Bones (también conocidos como jiggle bones o physics bones) son bones adicionales que se añaden a un wearable y se mueven dinámicamente en respuesta al movimiento del avatar y a la gravedad, en lugar de estar impulsados por animation clips. Dan vida a elementos como el cabello, los pendientes, las capas, los cinturones, las coletas y otros accesorios colgantes, haciéndolos balancearse, rebotar y asentarse de forma natural mientras el avatar camina, corre o gira.

Los Spring Bones son **no** parte de la base avatar Armature. Son bones adicionales que añades al esqueleto base del avatar en Blender (o en cualquier software 3D), y su comportamiento físico se configura en el Builder después de la carga.

Esta implementación sigue el [estándar VRM Spring Bone](https://vrm.dev/en/vrm1/springbone/), una convención ampliamente adoptada para la física de avatar en formatos como VRM y MMD.

![](/docs/_remote/34cee41d461f2df0b8a03bc9.webp)

### Cómo funcionan los Spring Bones

Un **spring chain** es una secuencia de bones que simula la física en conjunto y debe tener al menos dos bones: el spring bone real y un end bone. Un solo spring bone por sí solo no producirá ningún movimiento: la simulación necesita la head del spring bone para impulsar la física y la head del end bone definirá el extremo geométrico. Las chains más largas (3+ bones) tendrán un movimiento más suave y natural, ideal para long hairs o capes.

1. **Spring bone parent** — El primer bone de la chain. Posee la configuración física (stiffness, gravity, drag, etc.). Se identifica por tener `springbone` en cualquier parte del nombre del bone (sin distinguir mayúsculas y minúsculas).
2. **Child spring bone** — Es el child del bone anterior en la jerarquía. Hereda los parámetros físicos de su parent y forma la chain.
3. **Spring bone end** — El último bone de la chain. Define el extremo geométrico de la chain, pero no se ve afectado por la simulación y no deforma ninguna mesh.

La chain, o jerarquía, para los Spring Bones debe ser **lineal**, lo que significa que cada bone solo puede tener un child. Los bones con dos o más children pueden tener un comportamiento inesperado.

![Una bonita spring chain lineal.](/docs/_remote/69a822cfcb94b03d0ba7ebc5.webp)

![Los bones con 2 o más children tendrán un comportamiento inesperado.](/docs/_remote/9ef1ec407222b18595a504e1.webp)

### La jerarquía

Para que los Spring Bones funcionen bien, deben estar parented a uno de los bones originales del avatar. Para el cabello y los pendientes, por ejemplo, el bone en la parte superior de la jerarquía debe estar parented a ***Avatar\_Head***. Para las bufandas, parentar la chain a ***Avatar\_Neck*** es una buena idea. Para las faldas, quizá ***Avatar\_Hips*** o ***Avatar\_LeftUpLeg/Avatar\_RightUpLeg*** podría funcionar.

Es importante notar cómo funciona la estructura de esta jerarquía.

* *Hair\_springBone\_1* (spring bone parent): este es el bone en la parte superior de la chain, tiene configuración física
* *Hair\_springBone\_2* (child spring bone): este es el child del bone anterior en la jerarquía y hereda la configuración.
* *Hair\_springBone\_end* (spring bone end): es el último bone de la chain y solo sirve como endpoint.

### Convención de nombres

Todos los Spring Bones **deben** contener la subcadena `springbone` (sin distinguir mayúsculas y minúsculas) en su nombre. La subcadena puede aparecer en cualquier posición:

* *SpringBone\_hair\_left*
* *hair\_springbone\_l*
* *springbone\_earring.R*

Para mantener organizado tu flujo de trabajo, se sugiere usar este formato: **BodyPart/WearableName\_springBone**

Ejemplos: *Hair\_springBone\_1*, *Earring\_springBone.L* etc… Siéntete libre de usar el formato que mejor te convenga, siempre que siga la convención de nombres de Blender (o del software que prefieras) para izquierda y derecha.


> [!WARNING]
> ¡Atención!
> 
> Los Spring Bones no funcionarán sin `springbone` en el nombre del bone. Debe usarse, incluso para el end bone.


### Límites

Cada wearable debe mantenerse dentro de los siguientes límites de Spring Bones:

* Máximo **6** spring chains por wearable
* Máximo **12** total spring bones (suma de todos los bones en todas las chains)
* Maximum chain depth of **6** bones

## Creating Spring Bones in Blender

Los Spring Bones son bones adicionales que creas en Blender para la base avatar Armature. Los parámetros físicos se configuran más tarde en el Builder — ahora solo necesitas preparar la jerarquía de bones.

Para crear un nuevo bone, selecciona la avatar Armature y, en **Edit Mode**, asegúrate de tener el cursor donde quieres que se cree el bone y presiona **Shift+A**. Otra forma de hacerlo sería duplicar un bone existente presionando **Shift+D**. Una vez que tengas el primer bone de la chain, presiona **E** para extruirlo y crear el resto de la chain.

![Presiona Shift+A para crear un bone donde está el cursor.](/docs/_remote/ba60206570eff70b8cf1e047.webp)

Es importante notar que una spring chain no tiene que estar conectada al parent del skeleton, así que puedes colocarla en cualquier parte de la mesh. Sin embargo, la spring chain **tiene** que estar conectada; no puedes desplazar ninguno de los spring bones.

Una vez que crees los bones, asígnales un nuevo nombre siguiendo la convención mencionada arriba y asegúrate de parentar la chain al bone adecuado. Las líneas punteadas mostrarán el parent de la chain. Si no hay ninguno, significa que la chain no tiene parent. En **Edit Mode**, selecciona primero el child, luego selecciona el parent y presiona **Ctrl+P** > **Keep Offset**.

![Parenta los bones seleccionando primero el child, luego el parent y presiona Ctrl+P.](/docs/_remote/c74a20db130a6672bd12b248.webp)

Para renombrar un bone, selecciónalo en Edit Mode o Pose Mode, ve a la pestaña Bone Properties y asígnale un nuevo nombre de acuerdo con la convención de nombres. Haz esto para todos los bones de la chain.

![](/docs/_remote/ce5dc94e935f8d3e30031022.webp)


> [!NOTE]
> ¡Consejo!
> 
> Para asegurarte de que el bone esté colocado correctamente, selecciona la mesh en **Object Mode** y, en **Edit Mode**, selecciona los vertices del área donde quieres colocar el bone (puede ser un loop o un group de vertices) > presiona **Shift+S** > **Cursor to Selected.**
> 
> Vuelve a **Object Mode**Object Mode, selecciona la armature > ve a **Edit Mode** > selecciona el bone deseado > **Shift+S** > **Selection to cursor**.


![Usa Shift+S para colocar el bone en el lugar correcto de la mesh.](/docs/_remote/f67fcda5bdba4aa2338aeafe.webp)

### Skinning the Mesh

El skinning es el proceso de vincular la mesh a la armature, para que se muevan juntas. Para ello, definimos cuánto influirá (weight) cada bone sobre los vertices. Cuanto más weight, más deformará el bone la mesh. Para hacerlo, ve a **Object Mode**, selecciona la mesh primero, presiona **Shift** y selecciona la armature, presiona **Ctrl+P**. Hay dos formas de hacerlo: selecciona **With Empty Groups** o **With Automatic Weights**.

![](/docs/_remote/af0366e9dd4fcda637585505.webp)

#### With Automatic Weights

Como su nombre indica, con este método, Blender intentará establecer los skin weights automáticamente creando vertex groups para cada bone en la armature y asignando a cada uno weights automáticos. Puede funcionar en algunos casos, pero también puede requerir algunos ajustes. Puedes revisar los groups creados haciendo clic en la **Data** pestaña.

Si sabes con certeza que no necesitarás que ciertos bones afecten la mesh, puedes simplemente hacer clic en el **lock icon** para bloquear los groups que quieras, haz clic en el dropdown menu y **Delete All Unlocked Groups** para eliminarlos del object. Por ejemplo, si estás trabajando en hair, tener groups para feet y hands no tiene sentido. En ese caso, elimina todo lo que no sea la head o los Spring Bones, como en el ejemplo de abajo. Asegúrate también de eliminar el group para el end bone de la spring chain.

![Deleting vertex groups.](/docs/_remote/7241732fd5c95ea8e45c2fc5.webp)

#### With Empty Groups

En este método, Blender creará todos los vertex groups para cada bone en la armature, pero tendrán un weight de cero por defecto. Tendrás que asignar manualmente los weights en Edit Mode o pintarlos en Weight Paint mode. Esto te da más control sobre lo que afecta cada group y puede ser extra útil para superficies rígidas u objetos que necesiten asignarse por completo a un group determinado. En cualquier caso, una vez que hayas asignado los weights, tendrás que probarlos en Pose Mode y luego ajustarlos en Weight Paint.

![Parenting with empty groups.](/docs/_remote/7117bb961a31e9863ce1f2b6.webp)

#### Painting Weights

Para probar la mesh skinned, selecciona la armature y ve a **Pose Mode**, configura un keyframe, luego rota los bones para crear otra pose y configura otro keyframe. Así podrás comprobar cómo se deforma la mesh con el movimiento. Una vez que tengas las poses listas, vuelve a **Object Mode**, selecciona la mesh y ve a **Weight Paint**.

![](/docs/_remote/b65b3d58d5501afc7e9c1613.webp)

En **Weight Paint**, en la pestaña Tools encontrarás diferentes brushes para ajustar los skin weights. Selecciona el Vertex Group que quieras editar (si están bloqueados, simplemente desbloquéalos para poder editar la influencia) y usa el brush para añadir o quitar influencia. Negro significa cero influencia, mientras que rojo significa que ese group influye por completo en la mesh. Activar la wireframe en **Overlays** facilitará ver lo que estás pintando.

![](/docs/_remote/90b7445921156a708238f07c.webp)

Usa los brushes de add, subtract o smooth para obtener el resultado deseado. Prueba también distintas poses extremas para ver cómo se deforma la mesh y si no queda ningún vertex sin weights. Si estás satisfecho con el resultado, ¡es hora de exportarlo!

### Exporting

Antes de exportar, asegúrate de eliminar cualquier animation clip creado al probar las poses. Ve a Pose Mode, selecciona todos los bones presionando **Un** y presiona **Ctrl+R**, luego **Ctrl+S** y finalmente **Ctrl+G** para eliminar cualquier transform en tu armature. Luego vuelve al object mode, cambia el Display Mode de View Layer a Blender File, expande Actions y haz clic derecho y elimina el animation file.

![](/docs/_remote/d7f2ca8abd2e46afcdc74627.webp)

Si tienes otros objects en tu scene, como la mesh del avatar, desactiva su visibility haciendo clic en el eye icon en el Outliner. Luego ve a **File** > **Export** > **gltf 2.0 (.glb, .gltf)**. Para los export settings, expande Include y en **Limit to** activa **Visible Objects**. Haz clic en Export y ¡estarás listo para subir tu archivo al Builder!

![](/docs/_remote/22840326793df160d616c22f.webp)

## Configuring Spring Bones in the Builder

Después de subir un wearable que contiene bones con nombre `springbone`-, el Builder los detecta automáticamente y muestra el panel de configuración de **Spring Bones** .

![](/docs/_remote/6198c56189e1f79689bb9bd2.webp)

Desde este panel puedes configurar los parámetros físicos para cada spring root bone. La vista previa del avatar en el Builder reflejará tus cambios al instante, de modo que puedes ajustar el comportamiento en tiempo real.


> [!NOTE]
> No puedes añadir nuevos spring bones desde el Builder. Los bones ya deben existir en el archivo `.glb` subido con la convención de nombres correcta. El Builder solo te permite configurar los parámetros físicos para los spring bones detectados.


### Referencia de parámetros

#### Stiffness Force

|             |        |
| ----------- | ------ |
| **Range**   | 0 to 4 |
| **Default** | `2.0`  |

Controla con qué fuerza intenta el bone volver a su rest pose. Esta es la restoring force — piensa en ella como la "firmness" del spring.

* **0**: El bone no volverá en absoluto al rest; simplemente colgará flojo bajo la gravedad.
* **Low values** (e.g., 0–1): El bone está suelto y caído, se balancea libremente. Bueno para long flowing hair o lightweight hanging accessories.
* **High values** (e.g., 3–4): El bone es rígido, se mantiene cerca de su posición original y vuelve rápidamente a ella. Bueno para short hair o rigid accessories.

#### Gravity Power

|             |        |
| ----------- | ------ |
| **Range**   | 0 to 2 |
| **Default** | `0`    |

Controla la magnitud de la gravity force que tira del bone en cada frame.

* **0**: Sin efecto de gravity; el bone solo se ve afectado por la inercia del movement y la stiffness.
* **Low values** (e.g., 0.3–0.8): Atracción de gravity sutil, buena para la mayoría de los accessories.
* **High values** (e.g., 1.5–2.0): Fuerte atracción de gravity, que hace que el bone caiga mucho.

#### Gravity Direction

|             |                                |
| ----------- | ------------------------------ |
| **Format**  | X, Y, Z vector                 |
| **Default** | `X: 0, Y: -1, Z: 0` (downward) |

Sets la direction de la gravity force en world space. By default, gravity tira hacia abajo (Y = -1), simulando natural gravity.

| X  | Y  | Z  | Effect                                |
| -- | -- | -- | ------------------------------------- |
| 0  | -1 | 0  | Downward (natural gravity, default)   |
| 0  | 1  | 0  | Upward (floating/supernatural effect) |
| 1  | 0  | 0  | Leftward                              |
| -1 | 0  | 0  | Rightward                             |
| 0  | 0  | 1  | Forward                               |
| 0  | 0  | -1 | Backward                              |

Puedes combinar axes (e.g., `X: 0.5, Y: -0.5, Z: 0`) para diagonal directions. Esto es útil para simular wind-like effects o crear una floating appearance para supernatural characters.

#### Drag Force

|             |       |
| ----------- | ----- |
| **Range**   | 0 – 1 |
| **Default** | `0.5` |

Controla con qué rapidez el bone pierde momentum y se asienta. Piensa en ello como air resistance o damping.

* **Low values** (e.g., 0–0.2): El bone se balancea libremente durante mucho tiempo antes de asentarse, como un pendulum con poca friction.
* **High values** (e.g., 0.7–1.0): El bone se asienta casi al instante después del movement, dando una sensación pesada o dampened.
* **1**: El bone apenas se mueve: maximum deceleration.

#### Center (Optional)

|             |                      |
| ----------- | -------------------- |
| **Format**  | Bone name (dropdown) |
| **Default** | None                 |

Un optional reference bone usado para calcular el movement del spring bone en relación con un punto del avatar, en lugar de hacerlo con respecto al world space. Esto evita que la spring chain se balancee en exceso cuando el avatar se mueve (walking, running).

**Without a center bone**, springs calculan inercia en world space: cada paso que da el avatar hace que los bones reaccionen como si se hubiera movido todo el mundo, lo que provoca exagerated swinging durante la locomotion.

**With a center bone**, la simulation usa el space de ese bone como el reference frame, por lo que solo los *local* movements (head turning, body bending) activan la spring reaction.

Choose the center bone based on where the wearable is located on the body:

| Wearable location                     | Recommended center bone |
| ------------------------------------- | ----------------------- |
| Head (hair, earrings, tiaras)         | `Avatar_Head`           |
| Upper body (capes, necklaces)         | `Avatar_Spine`          |
| Lower body (belts, skirt accessories) | `Avatar_Hips`           |


> [!WARNING]
> The center bone **must not** be part of any spring chain. It should be a bone from the base avatar Armature.


## Casos de uso comunes y valores recomendados

These are suggested starting points. Adjust to taste using the Builder's real-time preview.

| Use case                | Stiffness | Gravity Power | Gravity Dir | Drag      | Center         | Notas                                                 |
| ----------------------- | --------- | ------------- | ----------- | --------- | -------------- | ----------------------------------------------------- |
| Long hair / ponytail    | 1.0 – 2.0 | 0.3 – 0.8     | 0, -1, 0    | 0.3 – 0.5 | `Avatar_Head`  | More bones in the chain = smoother movement           |
| Short hair              | 3.0 – 4.0 | 0 – 0.3       | 0, -1, 0    | 0.4 – 0.6 | `Avatar_Head`  | Higher stiffness keeps hair close to the head         |
| Earrings                | 0.5 – 1.5 | 0.5 – 1.0     | 0, -1, 0    | 0.4 – 0.6 | `Avatar_Head`  | Short chain (2–3 bones), lower stiffness for dangling |
| Cape / cloak            | 1.0 – 2.0 | 0.5 – 1.0     | 0, -1, 0    | 0.3 – 0.5 | `Avatar_Spine` | Multiple parallel chains for width                    |
| Belt / hanging ornament | 1.5 – 2.5 | 0.3 – 0.8     | 0, -1, 0    | 0.5 – 0.7 | `Avatar_Hips`  | Higher drag for heavier items                         |
| Floating / ghost effect | 1.0 – 2.0 | 0.5 – 1.0     | 0, 1, 0     | 0.3 – 0.5 | `Avatar_Spine` | Upward gravity for supernatural look                  |

## Limitaciones

Ten en cuenta las siguientes limitaciones al trabajar con Spring Bones:

* **No colliders**: Los Spring Bones no colisionan con el body del avatar ni con otros bones. Las chains pueden atravesar la mesh del body en poses extremas. Versiones futuras podrían añadir soporte para colliders.
* **No cross-wearable interactions**: Las spring chains de cada wearable son independientes. Las chains de diferentes wearables no se afectan entre sí.
* **No global wind**: No existe una wind force a nivel de scene. Puedes aproximar un wind effect por wearable ajustando `gravityDir` a una diagonal direction.
* **Performance on remote avatars**: La spring bone simulation se desactiva para avatars que están lejos del local player para ahorrar performance. Los avatars cercanos mostrarán la spring bone physics normalmente.
* **Alternative clients compatibility**: Los wearables con Spring Bones son backward-compatible con alternative clients y versiones antiguas de Explorer, pero los spring bone elements permanecerán static. En algunos casos, puede haber pequeños visual issues.

## Referencia técnica

La spring bone physics usa dos artefactos separados que trabajan juntos:

1. **El `.glb` archivo** contiene solo los nombres de los bones (con el `springbone` token) y su jerarquía. No se almacenan parámetros físicos en el archivo del modelo.
2. **Los metadatos del item del wearable** (`wearable.data.springBones`) contienen todos los parámetros físicos como JSON.

Cuando configuras parámetros en el Builder, se guardan en los metadatos del item del wearable — el `.glb` archivo nunca se modifica mediante ediciones de parámetros.

No necesitas interactuar directamente con este formato — el Builder se encarga de leerlo y escribirlo. Esta sección se proporciona como referencia.

### `.glb` Jerarquía de nodos (ejemplo)

El archivo del modelo contiene solo los nombres de los bones y las relaciones padre-hijo:

```json
{
  "asset": {"version": "2.0"},
  "nodes": [
    {"name": "Avatar_Head", "children": [1, 2]},
    {"name": "SpringBone_earring_r", "children": [3]},
    {"name": "SpringBone_hair_left", "children": [4]},
    {"name": "springbone_earring_r_tip"},
    {"name": "SpringBone_hair_left_tip"}
  ]
}
```

### Item Metadata (`wearable.data.springBones`)

Los parámetros físicos se almacenan en los metadatos del wearable, indexados por el GLB content hash:

```json
{
  "version": 1,
  "models": {
    "bafkreialsvt77jvpy673cnugp5ggnxfaalfncufweayuk3jbxskh3pelkm": {
      "SpringBone_earring_r": {
        "stiffness": 0.5,
        "gravityPower": 1.0,
        "gravityDir": [0, -1, 0],
        "drag": 0.6,
        "isRoot": true,
        "center": "Avatar_Hips"
      },
      "SpringBone_hair_left": {
        "stiffness": 2.0,
        "gravityPower": 0.8,
        "gravityDir": [0, -1, 0],
        "drag": 0.4,
        "isRoot": true
      }
    }
  }
}
```

* `models` se indexa por el GLB content hash. Los wearables cuyas representaciones masculina y femenina comparten el mismo `.glb` tienen una sola entrada.
* Cada entrada de bone usa exactamente el nombre del nodo del `.glb` (sensitivo a mayúsculas y minúsculas).
* tip bones (por ejemplo, `springbone_earring_r_tip`) no necesitan una entrada de metadata — solo sirven como extremos geométricos.

### Parámetros de metadata

| Parámetro      | Tipo    | Default      | Descripción                                                                                    |
| -------------- | ------- | ------------ | ---------------------------------------------------------------------------------------------- |
| `stiffness`    | float   | `2.0`        | Fuerza de restauración hacia la pose de reposo. Rango: 0–4.                                    |
| `gravityPower` | float   | `0`          | Magnitud de la fuerza de gravedad. Rango: 0–2.                                                 |
| `gravityDir`   | vec3    | `[0, -1, 0]` | Dirección de la gravedad en el espacio mundial.                                                |
| `drag`         | float   | `0.5`        | Factor de amortiguación / desaceleración. Rango: 0–1.                                          |
| `isRoot`       | boolean | —            | Si este nodo es la raíz de una cadena de resortes. Se requiere que sea `true` para root bones. |
| `center`       | string  | —            | Opcional. Nombre de un bone de referencia para el cálculo de inercia relativa.                 |


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/wearables-y-emotes/wearables/spring-bones.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
