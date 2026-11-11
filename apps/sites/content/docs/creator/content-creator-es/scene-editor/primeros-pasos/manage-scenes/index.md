> Gestionando los proyectos de tu escena

# Gestionar escenas

El **escenas** Esta pestaña muestra cada una de tus escenas disponibles como una tarjeta. Abre la tarjeta para editar esa escena; desde ahí también puedes previsualizarla o publicarla.

![La pestaña Scenes de Creator Hub, mostrando una cuadrícula de tarjetas de escena con una tarjeta New Scene primero, los botones Import Scene y Templates encima de la cuadrícula, y una columna Tutorials a la derecha.](/docs/_remote/8a4ec6d91bfb06d4402cf305.webp)

## Crear una escena

Haz clic en la flecha junto al **Crear** botón y selecciona **Nueva escena** para crear una nueva escena. También puedes hacer clic en la **Nueva escena** tarjeta al comienzo de la lista de escenas.

![El Dropdown del botón Crear, abierto, con las opciones Nueva escena e Importar escena.](/docs/_remote/727cb68c3c3feea0e6221431.webp)

Se te pedirá que nombres tu escena y que elijas una ubicación para guardarla. La ubicación predeterminada es la carpeta Scenes configurada en las preferencias de la aplicación; consulta [Cambiar la carpeta de escenas](#change-the-scenes-folder).

![El cuadro de diálogo Create Project, con un campo Project Name, un campo Project Path con un selector de carpetas y los botones Cancel y Create.](/docs/_remote/b21de226270dbf1e41f5f2ca.webp)

Una vez que confirmes estos pasos, se creará el proyecto de la escena. Esto puede tardar uno o dos minutos, ya que descarga dependencias y configura una carpeta en tu máquina local con todo lo que necesita. Cuando termine, tu escena se abrirá en el [Scene Editor](/docs/creator/content-creator-es/scene-editor/primeros-pasos/scene-editor-essentials/).

### Empezar desde una plantilla

Para empezar desde una escena ya hecha en lugar de una vacía, haz clic en **Templates** en la **escenas** pestaña, o desplázate hasta la **Templates** fila en la **Home** pestaña. Las plantillas van desde una **Empty Scene** hasta escenas completas como una galería de arte o una discoteca. Puedes filtrarlas por dificultad. Haz clic en una tarjeta de plantilla para crear una nueva escena basada en ella; luego asígnale un nombre y elige una ubicación como harías con una escena vacía.

![La pantalla Choose a Template, con filtros de dificultad Easy, Medium y Hard y una cuadrícula de tarjetas de plantillas, incluidas Empty Scene, Art Gallery Template y Cozy House Template.](/docs/_remote/9f7cb7f9f5985517a1f16c0d.webp)

### Opciones de la tarjeta de escena

Haz clic en los tres puntos de la tarjeta de una escena para ver más opciones.

![Una tarjeta de escena con su menú de tres puntos abierto, que muestra Duplicate, Open Folder Location, Rename Folder, View Deployments y Delete from My Scenes.](/docs/_remote/def8b7efaebef1e28761a7ce.webp)

Haz clic **Duplicar** para hacer una copia de una escena existente.

Para cambiar el nombre visible de tu escena, ábrela y haz clic en el icono del lápiz para cambiar el **Nombre** campo y otras propiedades.

Para cambiar el nombre de la carpeta de la escena en el disco, haz clic en los tres puntos de la tarjeta de la escena y selecciona **Rename Folder**. Ingresa el nuevo nombre de la carpeta y confirma. El nombre de la carpeta debe ser válido para tu sistema operativo y no debe entrar en conflicto con una carpeta existente en la misma ubicación.

## Importar una escena

El gestor de escenas muestra las escenas que encuentra en la ruta predeterminada de tu máquina.

Para añadir una escena que esté en otra ubicación de tu disco local, haz clic en **Import scene** y busca la ruta a la carpeta del proyecto. La escena importada ahora estará disponible como una nueva tarjeta en la pantalla del gestor de escenas.

La escena importada no se mueve en tu disco local.


> [!WARNING]
> **📔 Nota**: No renombres ni muevas manualmente la carpeta de una escena importada directamente desde tu gestor de archivos. Scene Editor ya no podrá encontrar la escena importada en su nueva ruta.


Las escenas que creaste en el antiguo editor web se almacenan en la nube. Para trabajar en estas escenas desde Scene Editor de escritorio, debes exportar la escena desde Web Editor, descomprimirla en una carpeta y luego importarla en Scene Editor de escritorio. Consulta [Migrar desde el Web Editor](/docs/creator/content-creator-es/scene-editor/primeros-pasos/migrate-from-web/) para más detalles.

## Eliminar una escena

En la pantalla de selección de escenas, pulsa el *tres puntos* icono y selecciona *Delete from My Scenes*.

Esto elimina la escena de la pantalla de inicio de Scene Editor. De forma predeterminada, no borra los archivos de tu máquina, pero el cuadro de diálogo de confirmación incluye una casilla para **también eliminar los archivos de la escena de tu computadora**.

![El cuadro de diálogo de confirmación de eliminación, que pregunta si estás seguro de que quieres eliminar la escena de My Scenes, con una opción desmarcada para también eliminar los archivos de la escena de tu computadora.](/docs/_remote/710213c338c5781ee7ca3f8f.webp)

## Cambiar la carpeta de escenas

De forma predeterminada, los proyectos creados a través de Scene Editor se guardan dentro de una `escenas` carpeta en el directorio de datos de la aplicación de Creator Hub. Para cambiar esta ubicación:

1. Haz clic en el logotipo de Creator Hub en la esquina superior izquierda para abrir el menú principal y luego haz clic en **Configuración**.
2. En la **SCENES** pestaña, haz clic en el icono de carpeta junto a **Scenes Folder** y elige una nueva carpeta.

![El cuadro de diálogo App Preferences en la pestaña SCENES, que muestra la ruta de Scenes Folder con un selector de carpetas y las opciones de Scene Dependencies: Auto-update all my scenes, Notify me of new version updates y Do nothing.](/docs/_remote/f3a6d0025165ce6a10282622.webp)

La misma pestaña también controla qué ocurre cuando hay disponible una nueva versión de las dependencias de la escena: actualizar automáticamente todas tus escenas, recibir una notificación o no hacer nada.

Para ir a la carpeta de un proyecto, haz clic en los tres puntos de su tarjeta y selecciona **Open Folder Location**.

## Gestionar Worlds

Si eres propietario de un NAME de Decentraland o de un dominio ENS, puedes publicar escenas en tu [World de Decentraland](/docs/creator/content-creator-es/scenes-sdk7/publicacion/publishing-options/#decentraland-worlds). Los Worlds aparecen en Scene Editor igual que las escenas normales, y puedes publicar en ellos usando el mismo **Publicar** botón.

Un World tiene sus propios metadatos (nombre, descripción y miniatura), separados de los metadatos de cada escena publicada en él. En Worlds con una sola escena, se mantiene sincronizado automáticamente con los metadatos de la escena; en Worlds con varias escenas, solo se puede editar en la **Manage** pestaña. Consulta [Metadatos del World frente a metadatos de la escena](/docs/creator/content-creator-es/scene-editor/publicar/publish-scene/#world-metadata-vs-scene-metadata).

### Visualización del espacio de almacenamiento

Las escenas publicadas en Worlds cuentan para un presupuesto de almacenamiento compartido entre todos los Worlds que pertenecen a tu wallet. El presupuesto se calcula a partir de tus activos: cada NAME de Decentraland o parcela de LAND que poseas te concede 100 MB, y cada 2.000 MANA que tengas en tu wallet te concede 100 MB adicionales.

Puedes comprobar tu presupuesto de almacenamiento usado y restante en dos lugares:

* El **Manage** sección de Creator Hub muestra cuánto de tu presupuesto total está usado y tu capacidad total de almacenamiento. Haz clic en **View Details** para ver un desglose de cómo se suman tus activos de MANA, LAND y NAME.

<img src="/docs/_remote/6b3fe18d494c053a67e9cb4f.webp" alt="" width="300">

* El **Worlds** pestaña de la [Builder](https://decentraland.org/builder/worlds).

### Retirar escenas

Si necesitas liberar espacio de almacenamiento, puedes retirar escenas del World Content Server. Esto se puede hacer a través de la interfaz de Builder, que te permite retirar escenas fácilmente para liberar espacio de almacenamiento.

Para los poseedores de NAME de Decentraland, si superas el espacio de almacenamiento asignado (por ejemplo, debido a ventas de assets o transferencias a otra wallet), se te dará una ventana de 48 horas para solucionar la situación. Si no lo haces, tus Worlds quedarán inaccesibles después de este período de gracia.

Para recuperar el acceso a un World bloqueado, puedes:

* Adquirir más MANA, NAMEs de Decentraland o LANDs para aumentar tu capacidad de almacenamiento
* Retirar las escenas existentes del World Content Server para liberar espacio de almacenamiento

Consulta [Límites de tamaño de Worlds](/docs/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/kinds-of-project/#size-limits) para obtener información detallada sobre cómo se calcula la capacidad de almacenamiento.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scene-editor/primeros-pasos/manage-scenes.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
