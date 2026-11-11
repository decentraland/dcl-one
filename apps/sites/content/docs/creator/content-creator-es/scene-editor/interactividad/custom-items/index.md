> Crea tus propios ítems personalizados para reutilizarlos en cualquiera de tus escenas.

# Ítems personalizados

Define un Custom Item para reutilizarlo fácilmente en cualquiera de tus escenas, o compártelo con otros creadores de escenas. Los Custom Items pueden constar de una sola entidad, o de tantas entidades como quieras. Los Custom Items pueden ser variaciones de Smart Items existentes, o completamente personalizados, con sus propios modelos y funcionalidades adaptados.



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/7cGLu8P7dso" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=7cGLu8P7dso>">Watch on YouTube</a></p>



## Cómo definir un Custom Item

Haz clic derecho en una entidad en el [Entity Tree](/docs/creator/content-creator-es/scene-editor/primeros-pasos/scene-editor-essentials/#the-entity-tree), o selecciona varias entidades y luego haz clic derecho sobre ellas. Selecciona **Create Custom Item**.

![](/docs/_remote/52a9b492b9f5042f38574e21.webp)

En la sección inferior de la pantalla, se te pide que le des un nombre a tu nuevo Custom Item.

![](/docs/_remote/eb32f501e5b7c6a70d610e46.webp)

El elemento ahora aparece en la pestaña **Custom Items** , en la parte inferior de la pantalla. Esta pestaña solo se muestra si existe al menos un Custom Item en tu workspace.

![](/docs/_remote/a66e7b1f2ae4e2d83577aeb6.webp)

Al definir un Custom Item, puedes seleccionar varias entidades en el mismo nivel jerárquico, pero no entidades de padres separados si no comparten un ancestro común. Cualquier entidad anidada se incluye automáticamente como parte del Custom Item; no hace falta seleccionarla.

Las entidades originales de tu escena no se ven afectadas por la acción de definir un Custom Item.


> [!WARNING]
> **📔 Nota**: Al definir un Custom Item, tomas una instantánea del estado de cada componente de las entidades seleccionadas (excepto la posición, rotación y escala de la entidad raíz). Esto incluye **Actions**, **Triggers**, **Multiplayer**, **Scripts**, **Visibility**, y cualquier otro componente.
> 
> Cualquier acción o trigger de Smart Item se autorreferenciará a su propia copia. Por ejemplo, si defines un Custom Item que incluye un ascensor y botones, los botones de cada copia del ascensor controlarán la copia del ascensor a la que pertenecen, no la copia original del ascensor.


### Uso de Custom Items

Simplemente arrastra el elemento desde la **Custom Items** pestaña a tu escena.

Una vez añadido, puedes modificar cualquier propiedad de un Custom Item; los cambios que hagas solo afectarán a *esa copia* del Custom Item. También puedes editar o eliminar los elementos originales a partir de los cuales se definió el Custom Item; esto no afectará a las copias existentes ni a las futuras.

Ten en cuenta que los Custom Items se muestran con un icono diferente en el Entity Tree. En la parte superior del menú de propiedades del Item, a la derecha, también verás una referencia al Custom Item a partir del cual se crearon.

Para eliminar una definición de Custom Item, haz clic derecho sobre el elemento en el **Custom Items** menú y selecciona **Delete**. Esta acción no afecta a ninguna copia existente del elemento en tus escenas; los Custom Items huérfanos permanecen en tu escena sin cambios. Eliminar una definición de Custom Item solo la quita de la pestaña Custom Items.

Para renombrar una definición de Custom Item, haz clic derecho sobre la definición de Custom Item en la **Custom Items** pestaña y selecciona **Rename**.

No puedes modificar la definición de un Custom Item que ya se ha creado; debes crear una nueva definición y eliminar la original.

### Generando Custom Items dinámicamente

Cada Custom Item se almacena como un composite, un archivo que describe un árbol de entidades y componentes. Esto significa que puedes generar una nueva copia de un Custom Item mientras la escena se está ejecutando, en lugar de colocar cada copia manualmente.

Puedes generar un Custom Item de dos maneras:

* **Sin código**: Usa la **Spawn Entity** acción en el Scene Editor. Consulta [About spawning entities](/docs/creator/content-creator-es/scene-editor/interactividad/smart-items-advanced/#about-spawning-entities).
* **Con código**: Usa `Composite.instance()` en el SDK. Consulta [Composites](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/composites/).

### Compartir Custom Items

Puedes compartir tus Custom Items con otros creadores, para que puedan usarlos en sus propias escenas.

Cada Custom Item se guarda en una carpeta independiente en tu equipo local. Abre esa carpeta haciendo clic en el icono de carpeta <img src="/docs/_remote/ad0c0dd6dced1bedf2b986b9.webp" alt="Code" data-size="line"> en la esquina superior derecha de la pestaña Custom Items.

También puedes encontrar manualmente esta carpeta en tu equipo:

* En Windows: *User/AppData/Roaming/creator-hub/Custom Items*
* En Mac: *Users/username/Library/Application Support/creator-hub/Custom Items*


> [!WARNING]
> **📔 Nota**: La *Library* está oculta en Mac de forma predeterminada. La forma más sencilla de acceder es abrir Go > Go to Folder y escribir *application support/creator-hub*


Para compartirla con otra persona, simplemente abre la carpeta Custom Items y copia la carpeta completa del elemento. Puede que quieras comprimir la carpeta en un zip para facilitar su transferencia. Esta carpeta contiene todo lo necesario para usar tu Custom Item.

La persona que use tu Custom Item deberá descomprimir luego la carpeta del elemento en su propia carpeta Custom Items en su equipo. Es posible que tenga que hacer clic en el botón Refresh <img src="/docs/_remote/29e0f054f9a96c7fb0c004cd.webp" alt="Refresh" data-size="line"> para que el elemento aparezca en su **Custom Items** pestaña.

Cualquier **Assets** utilizado por tu Custom Item también se almacena en la carpeta del Custom Item. Esto incluye cualquier modelo 3D, imagen, script, sonido y video referenciados por el elemento.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scene-editor/interactividad/custom-items.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
