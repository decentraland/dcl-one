> Cómo publicar tu escena en LAND o en un NAME.

# Publicar una escena

## Antes de comenzar

Asegúrate de lo siguiente:

* Tu scene cumple con todas las [limitaciones de la scene](/docs/creator/content-creator-es/scenes-sdk7/optimizacion/scene-limitations/). La mayoría de estas se validan cada vez que ejecutas una preview de tu scene.
* Tienes una [Metamask](https://metamask.io/) cuenta, con tus parcels de LAND o NAME asignados a ella.
* Posees la cantidad necesaria de parcels de LAND adyacentes o un NAME de Decentraland. De lo contrario, puedes comprar LAND en el [Marketplace](https://decentraland.org/marketplace/) o un NAME en el [Marketplace](https://decentraland.org/shop/items?category=names).


> [!WARNING]
> **📔 Nota**: Las scenes de múltiples parcels solo pueden desplegarse en parcels adyacentes.


Revisa los [detalles de tu scene](/docs/creator/content-creator-es/scene-editor/configurar/scene-settings/#scene-details), asegúrate de proporcionar un nombre atractivo, descripción, miniatura, categorías, etc.


> [!CAUTION]
> **❗Advertencia**: Cuando planifiques eventos en vivo, no hagas cambios de último minuto en la scene justo antes del evento. Publica tu versión final al menos una hora antes de que el evento comience.
> 
> Los jugadores que ya habían cargado la scene en la sesión actual antes de tus cambios seguirán viendo una versión en caché de la scene. Para ver la versión renovada, deben cerrar y volver a entrar a Decentraland.


## Publica tu scene

Para publicar tu scene:

1. Abre tu scene en el Scene Editor y haz clic en **Publicar**. Esto abre una ventana que muestra detalles sobre la publicación.
2. Selecciona si quieres publicar en LAND o en un WORLD. Consulta [Tipos de proyectos](/docs/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/kinds-of-project/) para comprender mejor las diferentes opciones.

![](/docs/_remote/75db93f7a2fdd17bec3b4907.webp)

3. Si publicas en LAND, selecciona la ubicación en el mapa. Verás tus parcels elegibles marcados en rojo. Si publicas en un WORLD, verás tus NAMEs elegibles en un dropdown.


> [!NOTE]
> **💡 Consejo**: Si no ves tus parcels o NAMEs, asegúrate de estar conectado a Creator Hub con la cuenta de usuario correcta. De lo contrario, sal del proyecto y haz clic en el icono de configuración de usuario en la esquina superior derecha, luego selecciona **Cerrar sesión** y vuelve a iniciar sesión.


4. La siguiente pantalla muestra todos los archivos que estás subiendo actualmente y sus tamaños; confirma la operación.
5. Entonces comenzará el proceso de publicación. Tiene dos etapas, y una vez que ambas terminen aparece un **Entrar** botón.


> [!NOTE]
> **💡 Consejo**: La pantalla de éxito también muestra un enlace web a tu scene, con un botón para copiarlo. Cualquiera puede abrir ese enlace en un navegador, tenga o no instalado el cliente de escritorio, así que es el que debes pegar en un mensaje de Discord o en una página de evento. Tiene este aspecto `https://decentraland.org/jump?realm=<your-world-name>` para un World, o `https://decentraland.org/jump?position=<x>,<y>` para una scene en Genesis City.
> 
> El **Entrar** botón es para ti y no para tu audiencia: abre la scene en tu propio cliente de escritorio para que puedas revisar tu trabajo.


![](/docs/_remote/a76ebe12a85e91368d25d90d.webp)


> [!NOTE]
> **📔 Nota**: Las dos etapas del deployment son:
> 
> * **1. Subida**: Subida de los archivos a los servidores. Las scenes de 50 MiB o más se suben por lotes; consulta [Publicar scenes grandes](/docs/creator/content-creator-es/scenes-sdk7/publicacion/publishing/#publish-large-scenes).
> * **2. Conversión**: Los modelos 3D de la scene se comprimen en Asset Bundles para un rendering más rápido. Esto suele tomar segundos. Puede tardar más en scenes muy grandes.
> 
> Los servidores de Decentraland también generan versiones de Low Level of Detail (LOD) de tus modelos, que se usan para dibujar tu scene desde lejos. Eso se ejecuta en segundo plano una vez que tu scene ya está en vivo, así que la publicación nunca espera por ello.


## ¿Cuánto tarda la publicación?

* **Hasta que puedas probarlo**: de unos segundos a un par de minutos. Escala con el tamaño total de tu scene y tu ancho de banda de subida. El **Entrar** botón aparece en cuanto la scene es jugable, una vez que ambas etapas terminan.
* **Antes de un evento en vivo**: publica tu versión final al menos una hora antes de que el evento comience, para dejar margen para cualquier imprevisto.

Los siguientes factores pueden hacer que una publicación sea más lenta:

* **Tamaño total de la scene**: afecta tanto a la subida como a la conversión.
* **La cantidad y complejidad de modelos 3D distintos**: los modelos se convierten uno por uno.
* **Qué tan ocupados estén los servidores de conversión**: tu scene puede quedar en cola detrás de otras scenes publicadas aproximadamente al mismo tiempo.

## Gestión de Worlds

Creator Hub permite la gestión de Worlds a través de la **Manage** pestaña de su panel principal. La **Manage** pestaña permite hacer seguimiento y edición de Worlds. Desde aquí, puedes editar World Settings, Permissions y Scenes.

### Presupuesto de almacenamiento del World

Las scenes publicadas en Worlds cuentan contra un presupuesto de almacenamiento compartido entre todos los Worlds que posee tu wallet. El presupuesto aumenta con tus posesiones: cada NAME de Decentraland o parcel de LAND que posees te concede 100 MB, y cada 2,000 MANA que mantienes en tu wallet te concede 100 MB adicionales. Los Worlds publicados en dominios ENS tienen, en cambio, un límite fijo de 36 MB.

El **Manage** pestaña muestra cuánto de tu presupuesto de almacenamiento estás usando. Haz clic en **Ver detalles** para ver cómo se suman tus posesiones de MANA, LAND y NAME.

<img src="/docs/_remote/6b3fe18d494c053a67e9cb4f.webp" alt="" width="300">

También puedes comprobar el presupuesto restante en la **Worlds** pestaña de la [Builder](https://decentraland.org/builder/worlds).

Consulta [Límites de tamaño de Worlds](/docs/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/kinds-of-project/#size-limits) para ver detalles sobre cómo se calcula el presupuesto y qué ocurre si lo excedes.

### Configuración del World

Un propietario del World puede editar su configuración entrando en el World deseado **Configuración** en el **Manage** panel, o accediendo a ella durante el proceso de publicación haciendo clic en **Configuración** si **Multi-Scene World (Avanzado)** está activado.

<img src="/docs/_remote/c29f510065b494f433726156.webp" alt="" width="600">

* **Detalles**: Información general del World:
  * Título del World
  * Descripción
  * Clasificación de contenido
  * Categorías

La información añadida en **Detalles** se mostrará en Decentraland Places y en la información del World dentro del mundo una vez publicado. Ten en cuenta que, si el World contiene una sola scene, estos detalles se sobrescriben con los metadatos de la scene en cada publicación. Consulta [Metadatos del World vs metadatos de la scene](#world-metadata-vs-scene-metadata) a continuación.

* **Layout**: Solo accesible en Worlds Multi-Scene. Contiene información sobre todas las scenes publicadas del World.
  * Elimina scenes individuales haciendo clic en los tres puntos y seleccionando **Eliminar del World**.
  * **Mapa del World** muestra el diseño del World e identifica los parcels con contenido y los parcels libres restantes.

<img src="/docs/_remote/f965a7cd250c7251c2c60acb.webp" alt="" width="600">

* **Varios**: Otras configuraciones útiles del World:
  * Coordenada de aparición del World: esto configura el Parcel (X,Y) en el que el usuario aparecerá dentro del World. La scene ubicada en ese Parcel determina la posición exacta en la que aparecerá el usuario (por ejemplo, el Parcel 1,1 es el Spawn del World, y la scene en 1,1 tiene un Spawn point de 1,0,1 **dentro de esa scene**).
  * Configuración del Skybox


> [!NOTE]
> **📔 Nota**: World Settings solo son accesibles para el propietario del World (la dirección que acuñó el NAME). Para más detalles sobre cómo obtener un NAME, consulta la [sección NAMEs de Marketplace](https://decentraland.org/marketplace/names/claim).


### Metadatos del World vs metadatos de la scene

Un World y cada scene publicada en él tienen dos conjuntos de metadatos separados. Ambos incluyen un nombre, una descripción y una miniatura:

* **Metadatos de la scene**: Pertenece al proyecto de la scene, almacenado en su `scene.json` archivo. Lo editas a través de la [configuración de la scene](/docs/creator/content-creator-es/scene-editor/configurar/scene-settings/) en el Scene Editor, y se sube junto con la scene cada vez que publicas.
* **Metadatos del World**: Pertenece al propio World. Lo editas en la **Configuración**, en la **Manage** pestaña de Creator Hub. Esta es la información que se muestra en Decentraland Places y en la información del World dentro del mundo.

Cómo se relacionan estos dos depende de cuántas scenes contiene el World:

* **World vacío**: Cuando publicas una scene en un World vacío, los metadatos del World se completan a partir de los metadatos de la scene.
* **World de una sola scene**: Cada vez que publicas la scene, los metadatos del World se actualizan para coincidir con los metadatos de la scene.
* **World multi-scene**: Los metadatos del World son totalmente independientes de los metadatos de cada una de sus scenes. Publicar una scene nunca cambia los metadatos del World. La única forma de modificarlos es a través de la **Configuración** en la **Manage** pestaña.

En la práctica, si tu World contiene una sola scene, no necesitas pensar en los metadatos del World como algo separado: solo edita la configuración de tu scene y publica, y el World siempre los reflejará.


> [!WARNING]
> **📔 Nota**: En un World de una sola scene, cualquier cambio que hagas en la **Configuración** en la **Manage** pestaña del World se sobrescribirá con los metadatos de la scene la próxima vez que publiques la scene. Para que esos cambios permanezcan, edita la configuración de la scene en su lugar.


En un World Multi-Scene, la **Configuración** en la **Manage** pestaña es el único lugar donde editar los metadatos del World.

### Worlds Multi-Scene

Un World puede tener múltiples scenes, publicadas por el propietario del World o por otros creadores. Esto permite un entorno colaborativo donde cada parcel puede ser gestionado por distintos colaboradores.

#### Hacer un World Multi-Scene

Un propietario del World puede elegir convertir el World en Multi-Scene activando **Multi-Scene World (Avanzado)** al publicar en un World de una sola scene.

<img src="/docs/_remote/9f653359d5337efaf125df54.webp" alt="" width="600">

Una vez publicado el World Multi-Scene, el propietario del World puede publicar escenas adicionales o añadir colaboradores para publicar dentro del World.


> [!NOTE]
> **📔 Nota**: El tamaño de un World Multi-Scene se adapta automáticamente para contener todas las scenes publicadas, creciendo y reduciéndose dinámicamente en cada publicación. El espacio que queda entre las distintas scenes en el Multi-World se llena con entorno.


#### Agregar colaboradores a un World Multi-Scene

En la **Manage** panel, un World Owner puede acceder a los **Permissions** haciendo clic en los tres puntos. El World Owner puede gestionar colaboradores en la **colaboradores** pestaña.

<img src="/docs/_remote/2586c8bab86636ee8a2193fe.webp" alt="" width="600">

Un colaborador puede tener permisos de deployment para All Parcels o para Custom Coordinates específicas. Las Custom Coordinates pueden seleccionarse y confirmarse mediante un mapa interactivo del World, similar al de World Settings.

<img src="/docs/_remote/ef6b0e88960c90ce2923d1fe.webp" alt="" width="600">

#### Desplegar en un World Multi-Scene como colaborador

Los colaboradores del World no pueden editar su configuración ni sus permisos. En la **Manage** pestaña, un creador puede ver el World en el que es colaborador pero no puede acceder a **Configuración** ni a **Permissions**.

<img src="/docs/_remote/f15119b09e3098fe4efe4dbe.webp" alt="" width="600">

Al pasar por el proceso de publicación, el creador puede elegir publicar solo en los parcels en los que es colaborador (según lo establecido por el World Owner).

En la **colaboradores** sección, si el World Owner estableció **Custom Coordinates** para el creador, solo los parcels asignados estarán disponibles para publicar. Si el acceso se configuró como **All Parcels**, el creador podrá seleccionar cualquier parcel en el World para publicar su scene.


> [!WARNING]
> **📔 Nota**: Los colaboradores con **All Parcels** acceso de publicación pueden sobrescribir cualquier scene del world, incluso si fue publicada por el propietario o por otros colaboradores.


<img src="/docs/_remote/4a5bbf90cf2c56710137c17e.webp" alt="" width="600">

### Worlds privados

Un WORLD puede tener diferentes **Acceso** configuraciones. Puede ser accesible para cualquiera o estar restringido de diferentes maneras.

#### Configurar el acceso de un WORLD

En la **Manage** panel, un World Owner puede acceder a los **Permissions** haciendo clic en los tres puntos. El World Owner puede gestionar las restricciones de acceso en la **Acceso** pestaña.

**Tipos de acceso**

Un World Owner puede elegir entre tres tipos de **Acceso al World**:

**Público**

Cualquiera puede acceder al World. Esta es la configuración predeterminada de un World.

**Protegido por contraseña**

Solo los usuarios con la contraseña pueden entrar al World.

Las contraseñas deben tener al menos 8 caracteres y contener al menos 2 números. Una vez creada, la contraseña no será accesible, así que asegúrate de guardar una copia.

**Solo por invitación**

Solo las direcciones y Communities añadidas en la **Direcciones aprobadas** pueden acceder al World.

Para añadir nuevas direcciones o communities a la **Direcciones aprobadas**, sigue estos pasos:

1. Haz clic en el **+ Nueva invitación** botón.
2. Puedes añadir direcciones de tres maneras diferentes:

* **Dirección de wallet**: Añade wallets individuales, una a la vez.
* **Community**: Busca y añade cualquier Public Community. Esto añade **todas las direcciones de Community** a la **Direcciones aprobadas**.

  <img src="/docs/_remote/cd2c518c4a9642b70cf0c060.webp" alt="" width="600">
* **Importar CSV**: Usa un CSV existente con una lista de direcciones o IDs de community para añadir a **Direcciones aprobadas**. La estructura es una wallet por línea, por ejemplo:

```
0x3bA7fD92eC4a1F6B8d2E9c5A7b1D3f6C8e4A2d9F
0xA1c9E4b7D2f6C8a3B5e9F1d4A7c2E6b8D3f9C5a1
```

Una vez importado, hace seguimiento de cada dirección de forma individual, como se muestra en la imagen.

<img src="/docs/_remote/554211db6a545dd6cf7ee848.webp" alt="" width="600">

3. Después de confirmar, la/s dirección/es están en la **Direcciones aprobadas**.
4. Con una nueva **+ Nueva invitación**, las direcciones se añaden a la lista existente, ayudando al World Owner a gestionar y ampliar la lista si es necesario.

<img src="/docs/_remote/c316240a3038949beab28744.webp" alt="" width="600">

5. Las direcciones individuales o conjuntos de direcciones (en el caso de una Community) se pueden eliminar seleccionando **Eliminar** en los tres puntos de la **Direcciones aprobadas** sección.


> [!WARNING]
> **📔 Nota**: Si cambias el **Acceso** tipo de **Solo por invitación**, tu **Direcciones aprobadas** lista se eliminará. Asegúrate de tener una copia por si la necesitas en el futuro.


#### Entrar en Worlds privados

Hay diferentes escenarios si un usuario entra en un World que no tiene **Público** acceso:

* Su dirección en la **Direcciones aprobadas**: Podrá entrar con normalidad. Si no, recibirá información de que el World está **Solo por invitación**.

<img src="/docs/_remote/ac4dca8cd154a2b6ce27d48d.webp" alt="" width="300">

* El World está **Protegido por contraseña**: Los usuarios podrán escribir la contraseña. El límite máximo es de diez (10) intentos.

<img src="/docs/_remote/65d948411619e496f8eed2f7.webp" alt="" width="300">

## Publicar desde una wallet de hardware

En lugar de almacenar tus tokens de LAND en una cuenta de Metamask, puede resultarte más seguro guardarlos en un dispositivo de wallet de hardware, como un [Ledger](https://www.ledger.com/) o un [Trezor](https://trezor.io/), que está conectado físicamente a tu computadora.

Si estás usando uno de estos dispositivos, puedes vincular la wallet de hardware a Metamask para habilitar la firma de mensajes, manteniendo los tokens más seguros. Consulta [este artículo de Metamask](https://support.metamask.io/more-web3/wallets/how-to-connect-a-trezor-or-ledger-hardware-wallet/) para obtener instrucciones para conectar tu cuenta.

Una vez que tu wallet de hardware pueda usarse a través de Metamask, podrás desplegar siguiendo los mismos pasos que si tus tokens estuvieran en una cuenta de Metamask.

## Sobrescritura de scene

Cuando se despliega una nueva scene, sobrescribe el contenido anterior que existía en los parcels que ocupa.

La publicación normalmente comienza de inmediato. Si estás a punto de reemplazar contenido que ya existe en un World, Creator Hub te pide confirmación primero y ofrece una **No mostrar de nuevo** opción.

Si una scene que ocupa varios parcels solo es sobrescrita parcialmente por otra, todos sus parcels quedan sobrescritos o borrados.

Supón que desplegaste tu scene *A* sobre dos parcels *\[100, 100]* y *\[100, 101]*. Luego vendes el parcel *\[100, 101]* a un usuario que posee land adyacente y que despliega una scene grande (*B*) en varios parcels, incluyendo *\[100, 101]*.

Tu scene *A* no puede renderizarse parcialmente en un solo parcel, así que *\[100, 100]* no mostrará ningún contenido. Debes crear una nueva versión de la scene *A* que solo ocupe un parcel y desplegarla únicamente en el parcel *\[100, 100]*.

## Publicar en land concedido

Si estás publicando en land propiedad de la Decentraland Foundation que te fue concedido mediante una grant, haz clic en el **Publicar** botón normalmente y luego selecciona **Publicar en un servidor diferente** en la parte inferior. Luego selecciona **Servidor personalizado** en el dropdown e introduce la siguiente dirección del servidor: `https://linker-server.decentraland.org`.


> [!WARNING]
> **📔 Nota**: Primero debes establecer manualmente las coordenadas de tu scene en la pestaña avanzada de la configuración de Layout. Consulta [Configuración de la scene](/docs/creator/content-creator-es/scene-editor/configurar/scene-settings/#layout) para obtener más información.


## Servidores personalizados

Puedes desplegar contenido en un servidor personalizado que no pertenece a la red oficial de servidores catalyst mantenida por el DAO. Para hacerlo, no necesitas poseer tokens de LAND ni de NAME, ya que puedes configurar el servidor para usar la lógica de validación que prefieras y controlar quién puede desplegar dónde. Los servidores personalizados pueden elegir tener contenido de los servidores oficiales, que puedes sobrescribir, o empezar desde cero y publicar contenido completamente nuevo.

Para publicar en un servidor personalizado, haz clic en el **Publicar** botón normalmente y luego selecciona **Publicar en un servidor diferente** en la parte inferior. Luego selecciona **Servidor personalizado** en el dropdown e introduce la dirección del servidor.

Consulta [Cómo ejecutar tu propio nodo Catalyst](/docs/contributor/tutorials/how-to-run-a-catalyst/) para obtener más información sobre lo que puedes hacer con tu propio servidor y cómo configurarlo.


> [!WARNING]
> **📔 Nota**: Los jugadores tendrán que escribir manualmente una URL para acceder a tu servidor personalizado. Algunas validaciones de servicios como el [servidor de recompensas](/docs/creator/content-creator-es/recompensas/getting-started/) podrían fallar en estos contextos, ya que a menudo estos servicios requieren que la solicitud provenga de un servidor oficial.


Nunca se dirige a los jugadores a este servidor; la única forma de acceder a él es escribir explícitamente la URL para conectarse.

## Verificar que el despliegue fue exitoso

Una vez que despliegues tu scene, estos cambios pueden tardar unos minutos en subirse y procesarse. Si entras a Decentraland justo después de desplegar, podrías seguir viendo brevemente la versión anterior de tu contenido.

Creator Hub muestra el progreso de la publicación a medida que pasa por las **Subida** y **Conversión** etapas, y muestra un **Entrar** botón en cuanto la scene sea jugable.

### Comprueba el estado de la conversión

Mientras los modelos 3D de una nueva versión se están convirtiendo, a los jugadores se les sirve deliberadamente la última versión completamente funcional de tu scene. Las conversiones no deberían tardar más de un par de minutos; si tardan más, podría ser señal de que algo falló.

Puedes consultar el estado de la conversión directamente en un navegador:

* `https://asset-bundle-registry.decentraland.org/entities/status/<pointer>`: reemplaza `<pointer>` con una de las coordenadas de tu scene (por ejemplo `20,-34`) `assetBundles`, y el estado de los LODs bajo `lods`. Cuando `complete` es `true`, la scene está completamente convertida para todas las plataformas. Para una scene en un World, añade el nombre del World como parámetro de consulta, por ejemplo `.../entities/status/0,0?world_name=myname.dcl.eth`.
* `https://asset-bundle-registry.decentraland.org/queues/status`: lista los IDs de todas las scenes que actualmente esperan ser convertidas, por plataforma. Si tu entity ID aparece aquí, tu scene está en cola detrás de otras conversiones y solo necesitas esperar un poco más.


> [!WARNING]
> **📔 Nota**: Si tienes una sesión abierta del Explorer de Decentraland donde ya cargaste tu scene, tendrás que cerrar y volver a abrir la aplicación. De lo contrario, seguirás viendo la versión en caché.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scene-editor/publicar/publish-scene.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
