> Guías para subir Wearables al Editor

# Subiendo Wearables

Una vez que exportes tu Wearable, tendrás que subirlo al Builder. Este documento cubrirá el proceso de subir Wearables.

## Subir tu archivo

Recuerda que necesitas crear una colección antes de poder subir tu archivo. Si no sabes cómo hacerlo, consulta [Crear una colección](/docs/creator/wearables-and-emotes/manage-collections/creating-a-collection/). Para subir tu Wearable, simplemente arrastra y suelta el archivo en la ***Nuevo ítem*** ventana o navega por tu computadora. Detectará automáticamente si el archivo es un emote o un Wearable. **Recuerda que el tamaño máximo de archivo de la colección es de 3 MB**.

![](/docs/_remote/3db6fae4f154b1b73fd50dc4.webp)

Cuando subas el archivo, se te pedirá seleccionar una body shape, ingresar un nombre y definir la rareza y la categoría. También puedes añadir la miniatura del Wearable.

### **Body Shape**

Para asegurarte de que tus Wearables puedan ser usados por los avatares previstos, necesitas subir archivos GLB separados para cada body shape. Si tienes dos versiones separadas del Wearable, una para masculino y otra para femenino, puedes añadir una de las representaciones durante el proceso de subida y luego añadir la otra más tarde usando el editor. Si tu Wearable está pensado para ser unisex, asegúrate de subir un único archivo GLB diseñado para ajustarse tanto a la versión masculina como a la femenina.

![](/docs/_remote/9c4ad1dbfd25f7f986e2b0d5.webp)

### **Rareza**

Selecciona la rareza de tu ítem.

![](/docs/_remote/a57649e01874c1a4789f0456.webp)

| Rareza     | Número de ítems |
| ---------- | --------------- |
| Único      | 1               |
| Mítico     | 10              |
| Exótico    | 50              |
| Legendario | 100             |
| Épico      | 1,000           |
| Raro       | 5,000           |
| Poco común | 10,000          |
| Común      | 100,000         |

### **Categoría**

Los Wearables se organizan en diferentes categorías, dependiendo de qué parte de un avatar modifican. Selecciona la categoría adecuada para tu ítem.

![](/docs/_remote/ac233f1f8c3b34463031b787.webp)

### **Miniaturas personalizadas**

Puedes añadir tu propia miniatura personalizada haciendo clic en el icono de la cámara y navegando por tu computadora. **La miniatura debe ser un archivo .png cuadrado de 256 px con fondo transparente.** Las colecciones que contengan miniaturas sin fondos transparentes no serán aceptadas por el Comité de Curación.

![](/docs/_remote/5a563c26949da7b96f8c2028.webp)


> [!WARNING]
> ⚠️ Tener un buen render de tu Wearable es crucial para hacerlo más atractivo para los usuarios potenciales en el Marketplace. **Es importante evitar añadir gráficos que no sean el propio Wearable, porque esto puede hacer que el comité de curación lo rechace.**
> 
> <img src="/docs/_remote/b8bdd9d193e7703e63def953.webp" alt="" data-size="original">


### **Spring Bones**

Si tu Wearable contiene bones con `springbone` en su nombre, el Builder los detectará automáticamente y mostrará un **Spring Bones** panel de configuración. Desde este panel puedes ajustar parámetros físicos como la rigidez, la gravedad y el arrastre para cada cadena de spring bones. Consulta [Spring Bones](/docs/creator/wearables-and-emotes/wearables/spring-bones/) para obtener detalles sobre cómo configurar spring bones en tu modelo.

### **Propiedades**

Debajo de la miniatura encontrarás las propiedades de tu Wearable, el número de triángulos de tu modelo, el número de materiales y texturas.

![](/docs/_remote/c1c0778d6b438f3e33e8c6cf.webp)

## **Subida de boca, ojos y cejas**

La categoría de boca, ojos y cejas tiene un comportamiento diferente en el editor porque solo son archivos .png. Para subirlos, simplemente arrastra y suelta el archivo png como una imagen transparente (256x256 píxeles). La boca se teñirá automáticamente con el color de la piel, igual que las cejas con el color del cabello.


> [!WARNING]
> Si quieres que el asset esté enmascarado, para que una parte de la boca o las cejas no se vea afectada por el teñido, sube un archivo zip con los archivos png y mask. Recuerda que el archivo mask debe tener el sufijo "\_mask" para que funcione.


![](/docs/_remote/ba0ba79e9d5699b2b18f666a.webp)

Después de subir tus Wearables, terminarás con una pantalla como esta, que muestra los ítems de tu colección.

![](/docs/_remote/b1d31fcc4dddce1716af7bc8.webp)

## **Establecer el precio de tus Wearables**

Una vez que publiques tu colección y sea aprobada, podrás habilitar las ventas en tu colección.

![](/docs/_remote/dd9d2d46268767048dd15f53.webp)

Puedes establecer el precio de tu Wearable haciendo clic en ***Put up for sale***. Esto se puede editar en cualquier momento, así que no te preocupes si quieres cambiarlo más adelante. Los precios se establecen en MANA. Recuerda que cuando mintas Wearables, se mintean directamente en Matic/Polygon. Cuando un usuario compra tu ítem, la transacción se realizará en MANA de Matic/Polygon.

También podrías ***Hacerlo gratis***, lo que significa que el precio se establecerá en 0 MANA y la dirección beneficiaria será nula. Ten en cuenta que hacerlo gratis (venta primaria) no impide que se venda a cualquier precio como venta secundaria.

No olvides establecer la dirección beneficiaria, que es la que recibirá el MANA de tus ventas. Puedes usar cualquier dirección de Ethereum que quieras. Para rellenar automáticamente la dirección con la que has iniciado sesión, haz clic en ***Soy el beneficiario.***

![](/docs/_remote/f25c9cc68be82373f6fe392e.webp)

Pon el ítem a la venta y volverás a la lista de Wearables de tu colección. Cuando hagas clic en el ítem, obtendrás su información general. Haz clic en el botón ***Preview*** para verlo en el editor.

![](/docs/_remote/528700c2d810068ed39ccdb2.webp)

## El Editor

Una vez que hagas clic en ***Preview***, tendrás el editor abierto. Puedes editar toda la información de tu Wearable, así como añadir nuevas, como descripción, etiquetas y overrides. Aquí también es donde añades otra representación de body shape a tu Wearable.

![](/docs/_remote/c21cb9360544651f12054903.webp)

## **Descripción**

Esta es una breve descripción de tu ítem que se mostrará en el Marketplace.

## **Overrides**

Los overrides determinan qué categorías de Wearable o partes del cuerpo del avatar ocultará tu ítem. Por ejemplo, un sombrero con cabello adjunto podría necesitar ocultar la *Hair* categoría Hair. Un casco de buzo de aguas profundas puede requerir ocultar accesorios para la cabeza como pendientes, eyewear, tiaras, etc., que no serían visibles. Se pueden seleccionar varias opciones para cada override.

* **Base Body**: Esto se refiere a partes básicas del avatar como la *cabeza* y *manos*. Por ejemplo, si estás creando un **Handwear** ítem como una mano mecánica de robot, probablemente tendrás que ocultar *manos* para evitar solapamientos y clipping.
* **Wearables**: Esto incluye otras categorías de Wearable. Puedes ocultar varias categorías. Para más detalles sobre cada categoría y cómo interactúan los ítems, consulta [**Crear Wearables**](/docs/creator/wearables-and-emotes/wearables/creating-wearables/).


> [!WARNING]
> Nota: los overrides que selecciones serán la configuración predeterminada sugerida para tu Wearable. Sin embargo, los usuarios pueden personalizar qué Wearables están ocultos o visibles desde el Backpack.


## **Etiquetas**

Las etiquetas son simplemente palabras descriptivas que los usuarios pueden usar al buscar o filtrar ítems. ¡Son útiles para competiciones o eventos!

## **Permiso de exportación VRM**

Cuando esta propiedad está habilitada, permitirá a los propietarios de tu ítem incluirlo en VRM Avatar Exports para que puedan mostrarlo fuera de Decentraland.

## **Contorno**

El interruptor de Contorno controla si tu Wearable es compatible con el sistema de renderizado de contorno de Decentraland (usado en los ajustes de calidad Media/Alta). De forma predeterminada, esto está habilitado para todos los nuevos Wearables.

Debes desactivar esta opción si tu Wearable:

* Tiene contornos creados manualmente en el modelo
* Usa normales invertidas
* Muestra artefactos visuales cuando se aplica el efecto de contorno

La mayoría de los Wearables funcionan bien con la configuración predeterminada (habilitada). Solo desactívala si notas problemas visuales relacionados con el contorno en tu Wearable. Una vez que desactives el contorno, tendrás que guardar los cambios haciendo clic en ***Guardar***.

![](/docs/_remote/69f438abb59b36e0dc7bcc84.webp)

## **Añadir otra representación**

Si tu Wearable tiene una representación diferente para masculino y femenino, necesitarás subir otro archivo. Hasta ahora, solo tienes uno subido. En el ejemplo, era la versión femenina del Krampus Sweater. Para añadir la otra representación, haz clic en los tres puntos (*…*) en la esquina superior derecha, junto a ***Propiedades*** y selecciona ***Añadir masculino/femenino*** representación. En el ejemplo siguiente, necesitábamos la versión masculina.

![](/docs/_remote/21001d89e30650778f2bd9b8.webp)

Una vez que hagas clic en ello, se abrirá la ventana para añadir representación masculina/femenina; arrastra y suelta el archivo de la otra representación para subirlo. Una vez subido, aparecerá otra ventana que mostrará el Wearable y, si todo está bien, solo haz clic en Guardar.

![](/docs/_remote/983512d9a30c68af5edb426d.webp) ![](/docs/_remote/7debe500c2fea74349f7fa23.webp)

## **Preview**

Para previsualizar tu Wearable, pasa el ratón por encima del icono del Wearable en la parte superior izquierda y haz clic en el símbolo del ojo.

![](/docs/_remote/4634ace580b72aed8b781aed.webp)

Al hacer clic en el icono de la parte inferior izquierda podrás editar el avatar. Esto es bastante útil si tienes una versión masculina y femenina de tu Wearable, así que asegúrate de comprobar cómo se ven ambas versiones en el editor, probando diferentes emotes para identificar si hay problemas de skinning y combinando otros Wearables para ver cómo encaja con diferentes prendas. Cuando termines de editar tu Wearable, haz clic en ***Guardar***.

![](/docs/_remote/cec3d31a2ab318de454d52a0.webp)

## **Probando en World**

Incluso después de probar el Wearable en el editor, es importante comprobar cómo se verá y comportará realmente en Decentraland. Para probarlo en World, ve a la pestaña Collections. Selecciona la colección deseada y haz clic en el botón ***Ver en World***.

![](/docs/_remote/4a233b404952959b105fa854.webp)

Después de hacer clic, aparecerá el siguiente pop-up. Al seleccionar ***Parcelas vacías*** te teleportará a un lugar sin demasiado contenido, que cargará más rápido. Al seleccionar ***Genesis Plaza*** te llevará a la plaza principal.

![](/docs/_remote/fa3231abc7a9d5db5a275577.webp)

Una vez que selecciones Ver en World, se abrirá una nueva pestaña en tu navegador y recibirás este mensaje.

![](/docs/_remote/a95471fb4beb838e7547a9bf.webp)

Haz clic en ***TRUST PEER-TESTING.DECENTRALAND.ORG*** y aparecerá un pop-up. Simplemente haz clic en Abrir Decentraland. Para probar tu Wearable, ve al Backpack y équipalo.

![](/docs/_remote/bd6c470a523472534ab4282f.webp)

## **Antes de publicar**

Asegúrate de establecer el precio correctamente, añade una buena descripción y comprueba dos veces que toda la información y la configuración sean correctas. Si has completado toda la información necesaria, verás ***Hecho*** como el estado de tu ítem.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/wearables-y-emotes/gestionar-colecciones/uploading-wearables.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
