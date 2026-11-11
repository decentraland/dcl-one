> Conceptos básicos sobre el rig de Avatar.

# Rig de Avatar

Un rig es un esqueleto virtual que permite que un modelo se mueva. Consiste en una jerarquía de huesos individuales, muy parecido a un esqueleto de la vida real, y funciona bajo una relación padre/hijo. Este documento cubrirá algunos conceptos básicos de rigging, como la posición de los huesos, la orientación de los huesos, los huesos deformantes y no deformantes, la diferencia entre IK y FK y sus propósitos. La estructura del rig de un avatar, los atributos personalizados y la configuración para animar se pueden encontrar en [características del rig](/docs/creator/content-creator-es/wearables-y-emotes/emotes/rig-features/).


> [!NOTE]
> **💡 Consejo**: Instala el [plugin Blender de Decentraland Tools](https://extensions.blender.org/add-ons/decentraland-tools/). Incluye varias funciones útiles para ayudarte a editar y exportar modelos 3D, wearables y emotes.


## **Lo básico**

#### Posición de los huesos o puntos de pivote

Aunque un rig no es una réplica exacta del esqueleto humano, es una buena práctica seguir la posición de los huesos de la vida real al colocar los digitales. Esto se hace para garantizar una deformación creíble y fluida. La posición de los huesos es importante porque establece el punto de pivote (desde donde comenzará el movimiento). Un hueso mal colocado causará una mala deformación de la malla. Las imágenes de abajo muestran la diferencia en la posición de los huesos.

![head\_pivot\_rig\_1.0.gif](/docs/_remote/19b528a3a24a1ffcdeaf8d5f.webp)

![head\_pivot\_rig\_2.0.gif](/docs/_remote/95fc62ca479c3f614187167d.webp)

#### Orientación de los huesos

La orientación de los huesos definirá en qué dirección rotarán, como X positivo, Z negativo, etc. Pueden configurarse de muchas maneras diferentes siempre que se mantenga la coherencia en todo el rig. Por ejemplo, si se elige el eje X para el movimiento de flexión hacia adelante de la columna, tiene sentido usar la misma configuración para las piernas. La orientación también es importante porque afectará el comportamiento de espejado. Eso significa que, por lo general, la orientación del lado derecho es la versión espejada de la del lado izquierdo y viceversa.

![](/docs/_remote/4d39fc26ef0dd267cbd54630.webp)

*Activa los ejes para mostrar la orientación de los huesos.*

![](/docs/_remote/8278804deb8abeeac7cba8ee.webp)

*Los ejes son las direcciones en las que rotará el hueso.*

#### Huesos deformantes y no deformantes

Los huesos deformantes son los que deformarán la malla; son responsables de la forma en que se mueve el modelo. Ese es el esqueleto base y **no debe editarse en absoluto**. Cambiarlo de cualquier manera puede romper el rig y la animación no funcionará al exportarla. Por esta razón, este rig base se movió a su propia capa (la última, en la parte inferior). Estos huesos son el armature que se exporta junto con la malla.

Los huesos no deformantes son los que no deformarán la malla, pero siguen siendo necesarios en un rig robusto y se utilizan para la configuración de [IKs, FKs](#whats-fk-and-ik-in-a-rig) y otras propiedades personalizadas. Ejemplos de huesos no deformantes son los controles, los huesos de IK y FK, y los huesos de configuración de los pies. Estos no deberían exportarse; solo se usan con fines de animación.

![](/docs/_remote/4587f7ead5c1973d96247db4.webp)

*Huesos deformantes.*

![](/docs/_remote/8c4d5b63d6775310aef4709b.webp)

*Huesos no deformantes.*


> [!WARNING]
> ⚠️ **¡Atención!** **¡No edites el esqueleto base en absoluto!**


![](/docs/_remote/7856fc2ef29a54224be410db.webp)

*El esqueleto base.*

#### Controles

Como buena práctica, un rig no debería animarse manipulando los huesos deformantes porque podría hacer que el rig se rompa. En su lugar, **las animaciones deberían hacerse manipulando los controles.**

Los controles son básicamente huesos no deformantes, lo que significa que no afectarán a la malla, haciéndolos completamente seguros para ser manipulados y animados. Su función es controlar el esqueleto base mediante restricciones y drivers, sin tocarlo directamente. Por lo general, tienen diferentes formas y colores como una señal visual de sus funciones y propósitos, lo que facilita al animador distinguir los huesos. Algunos de ellos también tendrán algo más que transformaciones de posición, rotación y escala, porque es posible añadirles propiedades personalizadas, como un cambio IK/FK.

También es importante tener en cuenta que no es posible usar la configuración de controles en un software diferente de aquel en el que se hizo originalmente. Cada software tiene su propia lógica y no es posible exportar las restricciones.

![](/docs/_remote/66b97ad8bec99d92c8bba7a2.webp) \_Controles y sus diferentes formas y colores.\_


> [!WARNING]
> ⚠️ **Advertencia**: El rig tiene que animarse en el mismo software en el que fue creado. No es posible usar una configuración de Blender en, por ejemplo, Maya y viceversa.


#### ¿Qué son FK e IK en un rig?

Un rig puede usar dos configuraciones diferentes que influirán en cómo se mueve: FK e IK.

#### FK - Cinemática directa

En la cinemática directa, o FK, el padre en la jerarquía mueve todos los huesos hijos que están debajo de él. Tomemos el brazo como ejemplo: cuando el hombro gira, el resto del brazo también girará; cuando el brazo gira, el antebrazo seguirá su comportamiento. Al animar en FK, cada hueso tiene que rotarse individualmente. Esta configuración ofrece mucho control sobre el movimiento y es ideal para movimientos en arco, que son esenciales para una animación fluida.

*Dirección del movimiento en la jerarquía.*

![](/docs/_remote/c8dc3dc16dcc58581a238041.webp)

*En FK, cada hueso tiene que rotarse individualmente.*

#### IK - Cinemática inversa

En la cinemática inversa, o IK, el hijo en la jerarquía puede influir en el movimiento de sus padres. En este caso, tomando nuevamente el brazo como ejemplo, cuando se mueve la mano, el resto del brazo seguirá el movimiento. También significa que, sin importar cómo se mueva el hombro, la mano mantendrá su posición. En esta configuración, un vector de polo / objetivo de polo controlará en qué dirección se doblarán los huesos. Las piernas suelen estar en IK y eso es esencial para que los pies se mantengan pegados al nivel del suelo mientras se anima.

![](/docs/_remote/2137414a0b2253ea260c8f89.webp)

*Dirección del movimiento en la jerarquía.*

![](/docs/_remote/9de7bae6abe4d41fe6467d0e.webp)

*En IK, la mano moverá todo el brazo y también mantendrá su posición. El objetivo de polo dirige la dirección en la que se dobla el codo.*


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/wearables-y-emotes/emotes/avatar-rig.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
