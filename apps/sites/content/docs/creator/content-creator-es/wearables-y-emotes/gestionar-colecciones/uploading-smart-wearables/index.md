> Guías para subir Smart Wearables al Editor

# Subiendo Smart Wearables

Después de generar tu Smart Wearable usando el [SDK7](https://github.com/decentraland/docs/blob/main/creator/sdk7/projects/smart-wearables.md), el siguiente paso es subirlo al Builder. Este documento explica cómo subir, publicar y poner tus Smart Wearables a la venta.

## Subir tu archivo

Recuerda que necesitas crear una colección antes de subir tu archivo. Si no sabes cómo hacerlo, consulta [Crear una colección](https://github.com/decentraland/docs/blob/main/creator/wearables-and-emotes/manage-collections/creating-a-collection.md). Para subir tu Smart Wearable, arrastra y suelta el archivo en la ***Nuevo ítem*** ventana o busca en tu computadora. Detectará automáticamente si el archivo es un Smart Wearable. **Recuerda que el tamaño máximo de archivo de la colección es de 3 MB**.

![](/docs/_remote/48697c84cb446eeea05d89d3.webp)

Cuando subas el archivo, se te pedirá que subas un video de presentación.

## Subiendo tu video de presentación

Este video de presentación se mostrará en el Marketplace para que los compradores vean cómo funciona el Smart Wearable.

Para subir tu video de presentación, arrastra y suelta un **.mp4** archivo de video en la ***Sube un video para tu Smart Wearable*** ventana o busca en tu computadora. **Recuerda que el tamaño máximo del archivo de video es de 4 MB y la duración máxima es de 15 segundos**.

![](/docs/_remote/763eb735dd9262151b2d4576.webp)

Cuando subas el video de presentación, se te pedirá que ingreses un nombre y definas la rareza y la categoría. También puedes añadir una miniatura para el Smart Wearable si quieres.

### Rareza

Selecciona la rareza de tu ítem.

![](/docs/_remote/5aa59b7826603f649de1aade.webp)

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

### Categoría

Los Wearables se organizan en diferentes categorías, según la parte del avatar que modifiquen. En este paso, debes seleccionar la categoría de ítem adecuada.

![](/docs/_remote/805bd4e946bcf6044e5f79e7.webp)

### Miniaturas personalizadas

Puedes añadir tu miniatura haciendo clic en el ícono de la cámara y seleccionando la imagen que quieras de tu computadora. **Asegúrate de que la miniatura sea un archivo .png cuadrado de 256 px, con fondo transparente.** Ten en cuenta que el Comité de Curación no aprobará colecciones que tengan miniaturas sin fondos transparentes.

![](/docs/_remote/d45b65391e8cc81a9f4c7a8c.webp)


> [!WARNING]
> ⚠️ Tener un buen render de tu Wearable es crucial para hacerlo más atractivo para los posibles usuarios del Marketplace. **Es importante evitar agregar gráficos que no sean el Wearable en sí, porque esto podría hacer que el Comité de Curación lo rechace.**
> 
> <img src="/docs/_remote/b8bdd9d193e7703e63def953.webp" alt="" data-size="original">


### Actualizar video de presentación

Para actualizar tu video de presentación, simplemente haz clic en el ícono de la cámara y busca el video en tu computadora. **Ten en cuenta que el tamaño máximo de archivo permitido es de 4 MB y la duración no debe superar los 15 segundos**.

![](/docs/_remote/3375e0716ee39e30f5ccb5aa.webp)

### Propiedades

Junto a la miniatura, encontrarás las propiedades de tu wearable, como la cantidad de triangles de tu modelo, la cantidad de materials y las textures.

![](/docs/_remote/41d92d3af0ff7eebd17ea3cf.webp)

## Subida de boca, ojos y cejas

La categoría de boca, ojos y cejas tiene un comportamiento diferente en el editor porque solo son archivos .png. Para subirlos, simplemente arrastra y suelta el archivo png como una imagen transparente (256x256 píxeles). La boca se teñirá automáticamente con el color de la piel, igual que las cejas con el color del cabello.


> [!WARNING]
> Si quieres que el asset esté enmascarado, para que una parte de la boca o las cejas no se vea afectada por el teñido, sube un archivo zip con los archivos png y mask. Recuerda que el archivo mask debe tener el sufijo "\_mask" para que funcione.


![](/docs/_remote/ba0ba79e9d5699b2b18f666a.webp)

Después de subir tus Wearables, terminarás con una pantalla como esta, que muestra los ítems en tu colección.

![](/docs/_remote/83043b56afafb5b81fe2be0f.webp)

## Establecer el precio de tu Smart Wearable

Puedes establecer el precio de tu Wearable haciendo clic en ***Establecer precio***. Esto se puede editar en cualquier momento, así que no te preocupes si quieres cambiarlo más adelante. Los precios se fijan en MANA. Recuerda que, cuando haces mint de Wearables, se mintean directamente en Matic/Polygon. De hecho, cuando un usuario compra tu ítem, la transacción se realizará en Matic/Polygon MANA.

También podrías ***Hacerlo gratis***, lo que significa que el precio será 0 MANA y la dirección del beneficiario será null. Ten en cuenta que ofrecer un producto gratis como venta primaria no impide que se venda a cualquier precio como venta secundaria.

No olvides establecer la dirección beneficiaria, que es la que recibirá el MANA de tus ventas. Puedes usar cualquier dirección de Ethereum que quieras. Para rellenar automáticamente la dirección con la que has iniciado sesión, haz clic en ***Soy el beneficiario.***

![](/docs/_remote/44c1580896b6f7d4ff6a5349.webp)

Guarda el precio y volverás a la lista de Wearables de tu colección. Cuando hagas clic en el ítem, obtendrás su información general. Haz clic en el botón ***Vista previa en el Editor*** para verlo en el Editor.

![](/docs/_remote/8170f9434918babf3565876f.webp)

## El Editor

Una vez que hagas clic en ***Vista previa en el Editor***, tendrás el Editor abierto. Puedes editar la información de tu Smart Wearable, incluida la descripción, las tags y los overrides.

![](/docs/_remote/cabb347fa07def0f5e919e48.webp)

## Descripción

Esta es una breve descripción de tu ítem que se mostrará en el Marketplace.

## Overrides

Los overrides determinan qué categorías de Wearable o partes del cuerpo del avatar ocultará tu ítem. Por ejemplo, un sombrero con cabello adjunto podría necesitar ocultar la *Hair* categoría Hair. Un casco de buzo de aguas profundas puede requerir ocultar accesorios para la cabeza como pendientes, eyewear, tiaras, etc., que no serían visibles. Se pueden seleccionar varias opciones para cada override.

* **Base Body**: Esto se refiere a partes básicas del avatar como la *cabeza* y *manos*. Por ejemplo, si estás creando un **Handwear** ítem como una mano mecánica de robot, probablemente tendrás que ocultar *manos* para evitar solapamientos y clipping.
* **Wearables**: Esto incluye otras categorías de Wearables. Puedes ocultar varias categorías. Echa un vistazo a [**Crear Wearables**](https://github.com/decentraland/docs/blob/main/creator/wearables-and-emotes/wearables/creating-wearables.md) para obtener más detalles sobre cada categoría y cómo interactúan los ítems.


> [!WARNING]
> Nota: los overrides que selecciones serán la configuración predeterminada sugerida para tu Wearable. Sin embargo, los usuarios pueden personalizar qué Wearables están ocultos o visibles desde el Backpack.


## Etiquetas

¡Los Tags son palabras descriptivas que los usuarios pueden usar para competencias o eventos!

## Preview

Para previsualizar tu Smart Wearable, pasa el mouse sobre el elemento de Wearable en la parte superior izquierda y haz clic en el símbolo del ojo.

![](/docs/_remote/38a5124d67833aaa252218c1.webp)

Al hacer clic en el ícono de la parte inferior izquierda, puedes editar el avatar. Esto es útil si tienes una versión masculina y femenina de tu Wearable, así que revisa cómo se ven ambas versiones en el Editor, probando diferentes emotes para identificar problemas de skinning y combinando otros Wearables para ver cómo encaja con otras prendas. Cuando termines de editar tu Wearable, haz clic en ***Guardar***.

![](/docs/_remote/2d3fb93f880674b537bb022c.webp)

## Probándolo en Decentraland

Incluso después de probar el Smart Wearable en el Editor, es importante comprobar cómo se verá y comportará en Decentraland. Para probarlo allí, ve a la pestaña Collections. Selecciona la colección deseada y haz clic en el botón ***Ver en Decentraland***.

![](/docs/_remote/f98021fb76ec1a330e1a9f4a.webp)

Después de hacer clic en el ***Ver en Decentraland*** botón, aparecerá la siguiente ventana emergente. Al seleccionar ***Parcelas vacías*** te teleportará a un lugar sin demasiado contenido, que cargará más rápido. Al seleccionar ***Genesis Plaza*** te llevará a la plaza principal.

![](/docs/_remote/fa3231abc7a9d5db5a275577.webp)

Una vez que selecciones la ubicación a la que teletransportarte, se abrirá una nueva pestaña en tu navegador y verás este mensaje.

![](/docs/_remote/61eed76d8ed92cce5d514ea9.webp)

Haz clic en ***Cambiar a Sepolia***, y aparecerá una ventana emergente de tu Wallet pidiéndote que cambies la red. Haz clic en Switch Network y la nueva pestaña se actualizará automáticamente. Para probar tu Smart Wearable, puedes ir a la mochila y seleccionarlo.


> [!WARNING]
> Si no has habilitado las redes de testnet de Ethereum, puedes seguir este [Artículo de Metamask](https://support.metamask.io/hc/en-us/articles/13946422437147-How-to-view-testnets-in-MetaMask).


![](/docs/_remote/91716ea1b880be3f761e1a9c.webp)

## Antes de publicar

Asegúrate de establecer el precio correctamente, añade una buena descripción y comprueba dos veces que toda la información y la configuración sean correctas. Si has completado toda la información necesaria, verás ***Hecho*** como el estado de tu ítem.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/wearables-y-emotes/gestionar-colecciones/uploading-smart-wearables.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
