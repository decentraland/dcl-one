> Características sobre el rig de Avatar y el archivo descargable.

# Características del rig

Esta documentación explica la configuración de Rig 1.0, sus controles y sus funciones.


> [!NOTE]
> **💡 Consejo**: Instala el [plugin Decentraland Tools Blender](https://extensions.blender.org/add-ons/decentraland-tools/). Incluye varias functions útiles para ayudarte a editar y exportar modelos 3D, wearables y emotes.


### Transformaciones del Armature

Estas son las transformaciones del armature en Object Mode con la configuración de los controles. **No edites esto de ninguna manera**. El rig solo debe manipularse en Pose Mode. Para evitar una edición no deseada, las transformaciones se han bloqueado en Object Mode.

*Transformaciones de Rig 1.0.*


> [!WARNING]
> ⚠️ **Advertencia**: **Nunca edites el rig en Object Mode.**


## Orientación de los bones

Esta es la orientación de los bones para Rig 1.0.

*Ejes para la orientación de los bones.*

*Comportamiento al espejar poses.*

### Espejar una pose

Los bones izquierdo y derecho del rig no están dispuestos de la misma manera, así que un solo truco de espejado no funciona para todo el cuerpo. Empieza con **Pose > Copy Pose**, luego **Paste X-Flipped**, y termina la parte superior del cuerpo a mano.

**Las piernas y los pies se hacen después de pegar.** **Paste X-Flipped** da el resultado correcto en `CTRL_FK_Avatar_UpLeg`, `CTRL_FK_Avatar_Leg`, `CTRL_FK_Avatar_Foot`, `CTRL_FK_Avatar_ToeBase`, `CTRL_IK_Foot`, `CTRL_IK_Avatar_ToeBase` y `CTRL_Avatar_Knee`. Es exacto para las piernas IK, y queda dentro de unos 3 cm en el pie para las piernas FK, porque los bones de descanso izquierdo y derecho tienen rolls ligeramente diferentes. Ajusta el pie FK a ojo si necesitas que sea exacto.

**Los hombros, los brazos, las manos y los dedos necesitan un paso más.** **Paste X-Flipped** solo pone la otra mano entre 40 y 70 cm lejos de donde la quieres. Después de pegar, pasa por cada uno de estos controles y **cambia el signo de los valores X y Z, manteniendo Y**:

* `CTRL_Avatar_Shoulder`
* `CTRL_FK_Avatar_Arm`, `CTRL_FK_Avatar_ForeArm`, `CTRL_FK_Avatar_Hand`
* `CTRL_IK_Avatar_Hand`, `CTRL_IK_Avatar_Elbow`
* Todos los 30 `CTRL_Avatar_Hand*` controles de los dedos

Aplícalo a la rotación, tanto si trabajas con quaternions como con ángulos de Euler. Los signos son los mismos en cualquier caso. En los controles IK de la mano y el codo, aplícalo también a la ubicación.

Por ejemplo, un brazo pegado que lea X -0.5, Z 0.1 pasa a X 0.5, Z -0.1.

Esto deja toda la cadena del brazo exactamente correcta.

**Por qué la parte superior del cuerpo es diferente:** los frames del bone de la pierna derecha son un simple reflejo de los de la izquierda, que es lo que **Paste X-Flipped** asume. Los frames del brazo derecho reflejan los del izquierdo con el **Z** local invertido en lugar de X, así que el pegado espejado sale rotado en la dirección incorrecta. Cambiar el signo de X y Z corrige exactamente eso.

#### Establecer los valores directamente, en su lugar

Si estableces los valores del lado derecho a partir de los valores en bruto del lado izquierdo en lugar de pegarlos, usa estos cambios de signo:

| Controles                 | Rotación `(w, x, y, z)` | Ubicación    |
| ------------------------- | ----------------------- | ------------ |
| Parte superior del cuerpo | `(w, -x, -y, z)`        | `(x, y, -z)` |
| Piernas                   | `(w, x, -y, -z)`        | `(-x, y, z)` |

Esta es la ruta aproximada: dentro de aproximadamente 1 mm para los brazos, pero con un error de 5 a 10 cm en las piernas FK. **Paste X-Flipped** también corrige las pequeñas diferencias de roll entre los bones de descanso izquierdo y derecho, y un cambio de signo en bruto no lo hace. Prefiere el método de pegar y luego corregir de arriba.


> [!WARNING]
> **📔 Nota**: La pose inicial no es perfectamente simétrica desde el principio, así que espejar una pose de un lado al otro sobrescribe esa pequeña asimetría. Comprueba el resultado a ojo antes de convertirlo en keyframe.


### Rotar los brazos

El control del brazo superior, `CTRL_FK_Avatar_Arm`, tiene un eje X local inclinado. Rotar alrededor de él barre el brazo a través del pecho, que es la razón más común por la que los brazos terminan cruzados cuando querías levantarlos.

* Para **apuntar** el brazo, rota alrededor de los ejes del mundo en su lugar: el X mundial lo levanta hacia delante, el Z mundial lo mueve hacia dentro y hacia fuera.
* Para **girar** el brazo, de modo que la palma mire hacia arriba o hacia abajo y el codo apunte hacia dentro o hacia fuera, rota alrededor del Y local del propio bone.

### En qué eje se dobla cada articulación

Los codos y las rodillas se doblan en su X local. Las muñecas y los dedos no: se curvan en su Z local. Rotar un dedo alrededor de su X local lo desplaza hacia un lado, que es la razón más común por la que los dedos se doblan al revés en un emote terminado.

| Control                                                     | Dóblalo en                                      | Otros ejes                                                                 |
| ----------------------------------------------------------- | ----------------------------------------------- | -------------------------------------------------------------------------- |
| Codo `CTRL_FK_Avatar_ForeArm`, rodilla `CTRL_FK_Avatar_Leg` | X local                                         | Y y Z están bloqueados                                                     |
| Muñeca `CTRL_FK_Avatar_Hand`                                | Z local flexiona la mano hacia la palma         | X local la inclina hacia un lado                                           |
| Dedos `CTRL_Avatar_Hand{Index,Middle,Ring,Pinky}{1,2,3}`    | Z local curva el dedo hacia la palma            | X local lo desplaza lateralmente en el plano de la palma. Y está bloqueado |
| Base del pulgar `CTRL_Avatar_HandThumb1`                    | X local desplaza el pulgar a través de la palma | Y está bloqueado                                                           |
| Pulgar `CTRL_Avatar_HandThumb2`, `CTRL_Avatar_HandThumb3`   | Z local curva el pulgar                         | Y está bloqueado                                                           |

Las curvaturas van en la misma dirección en ambas manos. La flexión del codo, la inclinación de la muñeca y el desplazamiento del pulgar a través de la palma van en la dirección opuesta en los lados izquierdo y derecho, así que un valor que funciona en un lado necesita que se le cambie el signo en el otro.

### Colecciones de bones

Para evitar cualquier accidente y facilitar la identificación de los controles, este rig está organizado en colecciones de bones que se pueden acceder en la *Data Properties* pestaña en Blender. La visibilidad de estas colecciones se puede activar y desactivar haciendo clic en el *Eye Icon.* Por defecto, todas son visibles, excepto las colecciones DON'T TOUCH.

pestaña Armature Data Properties.

Así se separaron los bones en las colecciones:

* Global/Switch: controles globales, como los de root y spine, así como los hombros. Los controles con cualquier atributo personalizado también están en esta colección.
* FK Upper: todos los controles de configuración FK de la parte superior del cuerpo.
* FK Lower: todos los controles de configuración FK de la parte inferior del cuerpo.
* IK Upper: todos los controles de configuración IK de la parte superior del cuerpo.
* IK Lower: todos los controles de configuración IK de la parte inferior del cuerpo.
* Fingers: controles para los dedos de ambas manos.
* Deformation Bones: aquí se almacenan los bones de deformación.


> [!WARNING]
> **⚠️ ¡Atención!**
> 
> Las colecciones DON'T TOUCH contienen las configuraciones de IK y otras restricciones del rig y deben permanecer ocultas. Editar estos bones podría romper la funcionalidad del rig.


## Controles y agrupación

Los controles son bones no deformantes que controlan el esqueleto base. Tienen diferentes colores según su categoría:

* Amarillo: controles globales y controles con atributos personalizados
* Verde: hip (más fácil de identificar en la jerarquía de la spine)
* Azul: controles con comportamiento FK
* Rojo: controles IK
* Rosa: controles del lado izquierdo
* Naranja: controles del lado derecho

*Todos los controles y sus colores.*

## Atributos personalizados y configuración

### Mezcla FK/IK

Aunque los brazos suelen configurarse como FK y las piernas como IK, hay ciertas situaciones que requerirán una configuración diferente. Si la mano tiene que mantener una posición determinada, como durante flexiones o al escalar, IK será la mejor opción. En cuanto a las piernas, mientras están en el aire, nadando o rodando, FK funciona mejor. Para una mayor flexibilidad y libertad en la animación, este rig tiene una mezcla FK/IK en el control UpperBody, siendo 0 completamente FK y 1 completamente IK. Cualquier otro valor intermedio será una mezcla de ambas.

![Mezcla FK/IK para ambos brazos y piernas.](/docs/_remote/efb0c0633809e34cc3499adb.webp)

*Mezcla FK/IK para ambos brazos y piernas.*

![Cómo funciona el cambio FK > IK.](/docs/_remote/b7bd249527c142b1c626e967.webp)

*Cómo funciona el cambio FK > IK.*

Una regla simple cubre la mayoría de los casos: si una extremidad mantiene contacto con algo, mantenla en IK; si pierde el contacto y se mueve libremente, cámbiala a FK. Para una pose sentada:

* **Ambos pies en el suelo**: mantén las piernas en IK. Mueve los pies con `CTRL_IK_Foot`, luego baja `CTRL_Avatar_UpperBody` para bajar el cuerpo. Los pies se quedan apoyados. Apunta las rodillas con `CTRL_Avatar_Knee`.
* **Una pierna cruzada sobre la otra**: cambia solo la pierna levantada a FK.
* **Pies colgando**: cambia ambas piernas a FK.

Usa `CTRL_Avatar_UpperBody` para mover todo el cuerpo hacia arriba y hacia abajo. Es el centro de gravedad. `CTRL_Avatar_Hips` mueve solo la pelvis y las piernas, lo que estira el torso, así que es el control incorrecto para sentarse.


> [!WARNING]
> **📔 Nota**: Evita cambiar una extremidad entre FK e IK en mitad de un clip. A menos que las dos cadenas coincidan exactamente en ese frame, la extremidad hace un salto. Si realmente necesitas cambiar, keyframea la propia propiedad FK > IK para que el cambio ocurra donde quieres.


#### Toca la cara o un objetivo con IK

Para hacer que una mano toque algo, como la cara, un prop o la otra mano, cambia ese brazo a IK. Rotar las articulaciones FK no puede colocar de forma fiable la punta de un dedo en un objetivo. Ten en cuenta que el control de la mano IK está en la muñeca, así que colocar el control sobre el objetivo pone la muñeca allí, no la punta del dedo. Mueve el control hasta que la punta del dedo caiga donde quieres.

La propiedad FK > IK se mezcla gradualmente entre sus keyframes. En los keyframes en los que la cambias, coloca el brazo FK cerca de la pose IK. De lo contrario, un brazo FK dejado colgando a un lado atraviesa el pecho mientras se ejecuta la mezcla.

Usa estas medidas de la cabeza del avatar predeterminado, tomadas con el avatar de pie en su pose idle, para colocar una mano en la cara:

| Punto de referencia         | Altura sobre el suelo                  | Posición relativa al `Avatar_Head` bone                                   |
| --------------------------- | -------------------------------------- | ------------------------------------------------------------------------- |
| `Avatar_Head` bone          | 1.72 m                                 | Este es el centro de la cara, no la base del cráneo                       |
| Punta de la nariz           | 1.66 m (1.64 m en el cuerpo masculino) | 0.19 m hacia delante y 0.06 m hacia abajo (0.08 m en el cuerpo masculino) |
| Barbilla                    | 1.60 m                                 | 0.12 m hacia abajo                                                        |
| Parte superior de la cabeza | 1.90 m                                 | 0.18 m hacia arriba                                                       |

La cabeza mide aproximadamente 0.30 m de ancho. Una mano apuntada 0.12 m por encima del bone de la cabeza cae en la frente. Si la cabeza está rotada, aplica estos desplazamientos en el marco rotado de la cabeza.

### Mezcla FK de rotación aislada

Otro atributo personalizado en el control UpperBody es la rotación aislada, que te permite elegir si el bone heredará o no la rotación de su padre (mientras está en FK). Cuando está en 0, el bone no heredará la rotación, mientras que en 1 seguirá por completo el comportamiento del padre. Cualquier valor intermedio será una mezcla de ambos. Esta es una herramienta interesante porque hace que el bone FK mantenga su posición, comportándose un poco como un IK.

![Atributo de rotación aislada para los brazos.](/docs/_remote/627c80a7f1680d2c4d02c6d4.webp)

*Atributo de rotación aislada para los brazos.*

![Cómo funciona el atributo IsoRot para los brazos.](/docs/_remote/517ca0821f2dd011b2c186f1.webp)

*Cómo funciona el atributo IsoRot para los brazos.*

El control Head también tiene este atributo. Es muy útil para los walk cycles, por ejemplo, ya que la cabeza mantendrá su rotación aunque el torso esté girando, asegurándose de mirar siempre hacia delante. Sin esta opción, el animador tendría que rotar manualmente la cabeza cada vez que el torso gira para que quede recta y mire hacia delante.

![Atributo de rotación aislada para la cabeza.](/docs/_remote/4afc891b59733db2550998de.webp)

*Atributo de rotación aislada para la cabeza.*

![Cómo funciona el atributo IsoRot para la cabeza.](/docs/_remote/4c87d322918f5fad671253d0.webp)

*Cómo funciona el atributo IsoRot para la cabeza.*


> [!WARNING]
> ⚠️ **Advertencia**: En versiones antiguas de Blender, aunque se hayan seleccionado y keyframeado todos los controles, estos atributos personalizados no se keyframearán automáticamente. Asegúrate de insertar manualmente un keyframe en cada atributo para no perder la pose o el movimiento que creaste. En Blender 4.4, al pulsar I, se establece un keyframe en todos los atributos y custom properties.


![En versiones anteriores de Blender, asegúrate de keyframear todos los controles y atributos personalizados.](/docs/_remote/a66e6f888ab4d3128c2395ee.webp)

*En versiones anteriores de Blender, asegúrate de keyframear todos los controles y atributos personalizados.*

*En Blender 4.4, pulsa I para establecer automáticamente un keyframe en Location, Rotation, Scale y Custom Properties..*

Otra solución para keyframear custom properties es seleccionar ***Keying*** en la pestaña Timeline y en ***Active Keying Set*** seleccionar Location, Rotation, Scale y Custom Properties, como se muestra en el gif de abajo. De esa manera, cada vez que pulses I, se creará un keyframe sin el menú emergente. Como a algunos animadores les gusta el menú, por defecto esa opción no está habilitada. Pero siéntete libre de elegir el método que mejor te convenga.

![Keyframing con la opción Keying.](/docs/_remote/296a3a5045569233102e0836.webp)

*Keyframing con la opción Keying.*

### Configuración inversa del pie IK

Animar un pie FK es bastante sencillo: solo tienes que coger cualquiera de los controles y rotarlo. Como hay un control para el pie y otro para los dedos del pie, el animador tiene control total sobre los movimientos. Sin embargo, con IK no es tan simple. El pie tiene que permanecer pegado al suelo, y además poder rotar sobre la parte delantera y los talones, y de lado a lado.

Este rig se configuró de forma que le da al animador libertad de movimiento del pie sin perder las ventajas del sistema IK. Consiste en cuatro controles:

* Foot roll: este control rota el pie hacia delante y hacia atrás y de lado a lado. Para evitar doblarlo demasiado sobre el talón o demasiado sobre la parte delantera, se estableció un límite para que el rig del pie no se rompa. Cuando alcanza este límite, el pie deja de rotar.

![Foot roll: rota en X y se mueve hacia delante y hacia atrás; rota en Z y se mueve de lado a lado.](/docs/_remote/28fab31b129fde23a5c5f90d.webp)

*Foot roll: rota en X y se mueve hacia delante y hacia atrás; rota en Z y se mueve de lado a lado.*

* Toe tip roll: rota el pie desde la punta de los dedos. Solo rota hacia delante.

![El toe tip roll solo rota en X positivo.](/docs/_remote/68e7725b2019cbf56dd0f028.webp)

*El toe tip roll solo rota en X positivo.*

* Toes control: rota los dedos del pie desde la parte delantera.

![Los dedos del pie se pueden rotar en cualquier dirección.](/docs/_remote/9c173333785f1d51960159db.webp)

*Los dedos del pie se pueden rotar en cualquier dirección.*

* Foot control: este es un control global que mueve el pie como un todo. Como es el padre de todos los demás controles del pie, mantendrá cualquier transformación y además se podrá coger y rotar.

![Cómo funciona el control del pie.](/docs/_remote/aa913f0950d1fe1e5d55b393.webp)

*Cómo funciona el control del pie.*

### Transforms bloqueadas

Algunos controles pueden tener un símbolo de candado junto a los parámetros de transforms, lo que significa que esos valores no se pueden cambiar. Esto se hizo en los controles que solo deben comportarse de una cierta manera y para evitar cualquier transformación no deseada. Por ejemplo, los codos y las rodillas están pensados para rotar en un solo eje, que en este caso es X, así que los otros ejes se han bloqueado. Otros ejemplos de controles con atributos bloqueados son los codos y rodillas IK, los dedos, foot roll y toe tip.

Se recomienda mantenerlos bloqueados, pero en caso de que quieras más libertad de movimiento, solo tienes que hacer clic en el icono de candado para desbloquearlos.

Los bloqueos solo se aplican a las herramientas interactivas de Blender. Un script de Python que escriba directamente la rotación de un bone los ignora, así que si poseas el rig con un script, mantén tú mismo cada rotación en los ejes libres de la articulación.

![Transforms bloqueadas en un control.](/docs/_remote/52bbc9e002370ab201046aeb.webp)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/wearables-y-emotes/emotes/rig-features.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
