> Configura una campaña de Decentraland Rewards: créala, añade stock de wearable o emote, gestiona fondos y activa dispensadores.

# Primeros pasos

Esta página te guiará a través del proceso de usar el sistema de Rewards para incentivar la participación de los usuarios con Decentraland Wearables y Emotes. Aprenderás cómo configurar y administrar una campaña de rewards, incluyendo la creación de una campaña, la adición de items al stock de la campaña, la gestión de fondos y el uso de dispensers para entregar rewards. Además, encontrarás instrucciones para activar tu campaña y gestionar las comisiones de transacción.

## Requisitos previos

Antes de comenzar a usar el sistema de Rewards, asegúrate de tener una colección [aprobada y lista para minting](/docs/creator/content-creator-es/wearables-y-emotes/publicacion-de-colecciones/publishing-collections/). Esta colección debe contener los items que planeas ofrecer como rewards en tu campaña.

## Crea tu primera campaña

Una campaña se refiere a una iniciativa de incentivos diseñada para aumentar la participación de los usuarios mediante ofertas de rewards. En esta sección, descubrirás los detalles de configuración de la campaña, junto con los requisitos previos para iniciar la distribución de rewards.

1. Accede a la [Rewards dApp](https://decentraland.org/rewards)
2. Conecta tu Wallet
3. Haz clic en el **Campañas** pestaña
4. Haz clic en el **Crear campaña** botón. Serás redirigido al formulario de creación de la campaña.

![](/docs/_remote/6288b97cb788a168a929488f.webp)

5. Completa los siguientes campos:

* **Nombre de la campaña**: Elige un nombre para tu campaña. Este nombre será visible para los usuarios en su página de rewards.
* **Red de la campaña**: Selecciona la red donde se acuñarán tus wearables. Ten en cuenta que esta elección es definitiva y no se puede cambiar una vez creada la campaña.
* **Precio máximo de gas**: Establece el precio máximo de gas que estás dispuesto a pagar por transacción. Si el precio de gas de una transacción supera este límite, se pospondrá hasta que el precio baje. La UI proporcionará una estimación del costo de acuñar cada item según esta configuración.

6. Haz clic en el **Crear campaña** botón. Serás redirigido a la página de la campaña.

![](/docs/_remote/2c39dab8808bf032e1c51198.webp)

Después de crear tu campaña, quedan algunos pasos más por completar antes de que puedas empezar a entregar rewards:

* **Añadir un suministro de tokens MATIC**: Asegúrate de tener suficientes tokens MATIC en tu cuenta para cubrir las comisiones de transacción.
* **Añadir items al stock de la campaña**: Llena tu campaña con los items que planeas ofrecer como rewards.
* **Crear un dispenser**: Configura un dispenser que se encargará de distribuir los rewards a los usuarios.
* **Activar tu campaña**: Finalmente, activa tu campaña para ponerla en vivo y empezar a entregar rewards.

### Añadir MATIC para pagar las comisiones de transacción

Para acuñar items, deberás cubrir las comisiones de transacción añadiendo una cantidad de tokens MATIC a la dirección de tu campaña. Puedes encontrar esta dirección debajo del nombre de la campaña.

Para transferir MATIC desde tu Wallet, primero asegúrate de estar conectado a la misma red que la campaña. Luego, copia la dirección de la campaña y envíale la cantidad deseada de MATIC. Si no tienes MATIC en tu Wallet, puedes seguir [esta guía](https://github.com/decentraland/docs/tree/main/player/blockchain-integration/transactions-in-polygon.md#where-can-i-get-matic-to-pay-for-transaction-fees) para conseguir algunos.


> [!NOTE]
> **💡 Consejo**: Para campaigns que se ejecutan en la red de prueba **AMOY** , puedes obtener algunos MATIC de [este faucet](https://faucet.polygon.technology/).


Para enviar MATIC desde un exchange solo necesitas retirar el token MATIC a la Polygon Network (normalmente la Amoy Network no está disponible en los exchanges).

### Añadir items al stock de la campaña

Cuando creas una nueva campaña, comienza con un inventario vacío. Para empezar a entregar rewards, necesitarás añadir items a la campaña:

![](/docs/_remote/ec2d216749ea170e1a5f766a.webp)

* **Otorgar permisos de minting**: Primero, autoriza a la campaña a acuñar items. Sigue [esta guía](/docs/creator/content-creator-es/wearables-y-emotes/publicacion-de-colecciones/publishing-collections/#adding-minters-to-the-collection) y usa la dirección de la campaña (ubicada debajo del nombre de la campaña) como la dirección del minter. Después de enviar, espera a que la transacción se confirme.
* **Añadir items al stock de la campaña**: Una vez otorgados los permisos de minting, puedes empezar a añadir items al inventario de la campaña.

  a) Haz clic en el **Añadir suministro** botón para ver todas las collections para las que la campaña tiene permisos de minting.

![](/docs/_remote/0e60e96b9eeeac0ec6fcde02.webp)

```
b) Selecciona el item de la colección que quieres añadir al stock, luego haz clic en el botón **Añadir suministro**.
```

![](/docs/_remote/f1349ec843ec2ae7fa0e04a0.webp)

```
c) Selecciona la cantidad de items que quieres añadir al stock de la campaña y haz clic en el botón **Añadir suministro**.
```

![](/docs/_remote/1281e9ab96332e98d58a7e9a.webp)


> [!NOTE]
> Los campos Priority y Group son funciones avanzadas que se tratarán en una sección dedicada.


### Crear un dispenser para entregar los rewards

Los dispensers son las herramientas utilizadas para enviar o reclamar rewards desde tu stock. Cada dispenser está asociado con una clave única, que se requiere para la autenticación con la HTTP API. Aunque la clave no se muestra en la UI, puedes copiarla haciendo clic en el botón Copy.

Cuando creas una nueva campaña, se proporciona automáticamente un dispenser predeterminado con una master key solo para fines de prueba. Para configurar un nuevo dispenser, haz clic en el **Añadir dispenser** botón.

![](/docs/_remote/80ef5f28ed3dbc20c04f5aaa.webp)

Selecciona el grupo de items del que quieres tomar items y haz clic en el **Guardar** botón.


> [!NOTE]
> Los dispensers tienen algunas opciones de configuración que se tratarán en una sección dedicada.


### Activar tu campaña

Por defecto, tu campaña está inactiva. Esto evita que el servicio de rewards asigne nuevos rewards. Para activar tu campaña, haz clic en el **Activar campaña** botón.


> [!WARNING]
> **📔 Nota**: Si una campaña activa se establece como inactiva, los rewards pendientes seguirán enviándose. Sin embargo, se impedirán los nuevos reclamos o los reclamos que necesiten reintentarse por algún motivo.


**Ahora estás listo para empezar a acuñar wearables/emotes usando la** [**API**](/docs/creator/content-creator-es/recompensas/integrations/)

### Gestionar fondos

Una vez completada la campaña, si quedan fondos restantes de las comisiones de transacción, el propietario puede elegir recuperar los fondos o reasignarlos a otra campaña. Para recuperar los fondos, simplemente haz clic en el botón Manage Funds en la página de detalles de la campaña.

![](/docs/_remote/3ff911494b211a0d72e187c5.webp)


> [!WARNING]
> **📔 Nota**: Si todos los fondos se eliminan de una campaña, cualquier transacción pendiente o nueva relacionada con esa campaña será bloqueada hasta que se asignen más fondos.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/recompensas/getting-started.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
