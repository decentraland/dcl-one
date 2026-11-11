> Lo que puedes ver en la vista previa de una escena

# Vista previa de tu escena

Una vez que hayas [creado una escena nueva](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/sdk-101/#create-your-first-scene) o descargado una [escena de ejemplo](/docs/creator/content-creator-es/tutoriales-y-ejemplos/examples/) puedes previsualizarla localmente.

## Usando Scene Editor en Creator Hub

Asegúrate de haber [instalado Creator Hub](/docs/creator/content-creator-es/scene-editor/primeros-pasos/editor-installation/).

1. Abre el proyecto de tu escena.
2. Haz clic en el **Reproducir** botón en la esquina superior derecha. Esto abrirá una nueva ventana con Decentraland Desktop Explorer, ejecutando solo tu escena. Allí podrás moverte por la escena e interactuar con los elementos interactivos.

![](/docs/_remote/ca7e6550cdb1bb947ce74b28.webp)

### Elige dónde previsualizar

Haz clic en la flecha junto al **Reproducir** botón para abrir el **Opciones de reproducción** menú. El panel principal es donde eliges qué ejecuta tu escena:

* **Cliente de escritorio**: Abre la vista previa en Decentraland Desktop Explorer. Esta es la opción predeterminada.
* **Web (Bevy)**: Abre la vista previa en tu navegador, usando el cliente web Bevy. Equivale a la `--web` bandera de CLI.
* **Mostrar código QR para móvil**: Muestra un código QR que abre la vista previa de tu escena en la [app móvil de Decentraland](/docs/creator/content-creator-es/desarrollar-para-movil/cliente-movil/overview/). Escanea el código con un teléfono conectado a la misma red Wi‑Fi que tu computadora. Haz clic en el área oscura alrededor del popup para cerrarlo. Consulta [Previsualizar en móvil](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/preview-on-mobile/) para más detalles.

### Opciones del cliente de escritorio

Selecciona **Cliente de escritorio** en el **Opciones de reproducción** menú; luego pasa el cursor sobre él para abrir un submenú con los ajustes que solo afectan al cliente de escritorio:

* **Activar terrenos del paisaje**: Activa o desactiva el paisaje alrededor de la escena. Está habilitado de forma predeterminada; desactívalo para reducir el consumo de memoria de la escena. Aparece atenuado cuando la propia configuración de la escena ya oculta el terreno.
* **Vista previa multiinstancia**: Permite que varias ventanas de Explorer se ejecuten al mismo tiempo, para que puedas [probar localmente una escena multijugador](#test-a-multiplayer-scene-locally). Con esto activado, cada ventana te pedirá iniciar sesión, para que puedas conectarte como un jugador diferente en cada una. Solo visible cuando tu proyecto usa SDK 7.20.4 o posterior.
* **Activar MCP Server**: Inicia Explorer con el servidor de automatización MCP habilitado, para que los agentes de IA puedan ver y controlar la vista previa en ejecución. Solo visible cuando tu proyecto usa SDK 7.25.0 o posterior. Consulta [Vibe Coding con IA](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/vibe-coding/#let-the-ai-see-your-scene-in-world) para ver el flujo completo.
* **Comprimir activos**: Muestra la escena con asset bundles generados localmente, igual que se renderiza en producción después de [la conversión a asset bundles](/docs/creator/content-creator-es/scenes-sdk7/optimizacion/performance-optimization/#asset-bundle-conversion). La primera ejecución convierte todos los activos, lo que puede tardar varios minutos en escenas grandes. Solo visible cuando tu plataforma y la versión de SDK de tu proyecto lo admiten. Consulta [Previsualizar con activos optimizados](#preview-with-optimized-assets).

La salida de consola de la escena no necesita una opción de menú. Creator Hub la abre por ti cada vez que ejecutas una vista previa. Consulta [La consola de Creator Hub](#the-creator-hub-console).


> [!NOTE]
> **Consejo:** También puedes previsualizar tu escena directamente en la app móvil de Decentraland. Usa la **Mostrar código QR para móvil** opción en Creator Hub, o ejecuta `npm run start -- --mobile` desde la CLI. Consulta [Previsualizar en móvil](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/preview-on-mobile/) para más detalles.


## Usar la CLI

Para previsualizar una escena, ejecuta el siguiente comando en la carpeta principal de la escena:

```bash
npm run start
```

Las dependencias que falten se instalarán y luego la CLI creará un servidor web local en tu sistema e iniciará la escena en el cliente de escritorio de Decentraland mediante un `decentraland://` deeplink. El cliente de escritorio es el destino de vista previa predeterminado.

Para previsualizar en una pestaña del navegador en su lugar, añade `-- --web` (o `-- --bevy-web`) para abrir la escena en el cliente web Bevy en `decentraland.org/bevy-web/`.

Cada vez que haces cambios en la escena, la vista previa se recarga y se actualiza automáticamente, así que no hace falta volver a ejecutar el comando.


> [!WARNING]
> **📔 Nota**: Algunas escenas dependen de comunicarse con un servidor externo para ejecutar lógica personalizada o almacenar y recuperar datos. Al previsualizar una de estas escenas, probablemente también tendrás que ejecutar el servidor localmente en otro puerto. Consulta el archivo readme de la escena para obtener instrucciones sobre cómo iniciar el servidor además de la escena.


### Parámetros del comando de vista previa

Puedes añadir las siguientes banderas al `npm run start` comando para cambiar su comportamiento:

* `-- --web` (alias `-- --bevy-web`) Abre la vista previa en el cliente web de Bevy en el navegador en `decentraland.org/bevy-web/` en lugar de Desktop Explorer. Los navegadores basados en Chromium (Chrome 142+) requieren el permiso de Acceso a la red local para que la página alojada pueda alcanzar tu servidor local de vista previa — cuando el navegador pida acceder a las aplicaciones de tu dispositivo, haz clic en "Permitir".
* `-- --mobile` (alias `-- -m`) Muestra un código QR en la terminal que abre tu escena en la [app móvil de Decentraland](/docs/creator/content-creator-es/desarrollar-para-movil/cliente-movil/overview/) en un teléfono conectado a la misma red Wi‑Fi. Consulta [Previsualizar en móvil](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/preview-on-mobile/).
* `-- --skip-build` Omite la compilación y sirve solo los archivos en modo de vista previa.
* `-- --port` (alias `-- -p`) para asignar un puerto específico para ejecutar la escena. De lo contrario, usará el puerto que esté disponible.
* `-- --no-browser` (alias `-- -b`) para evitar que la vista previa abra una nueva pestaña del navegador.
* `-- -w` o `-- --no-watch` para no vigilar cambios en el sistema de archivos y evitar la recarga en caliente cada vez que cambie el código de la escena.
* `-- --ci` Para ejecutar el visor de parcelas en un servidor Unix remoto.
* `-- --multi-instance` Permite ejecutar varias instancias de Explorer simultáneamente.
* `-- --asset-bundles` Previsualiza con asset bundles optimizados. Desktop Explorer convierte por sí mismo los activos de la escena en asset bundles durante la vista previa, igual que se renderiza la escena en producción. Equivale a la **Comprimir activos** opción en Creator Hub. Solo disponible con el cliente de escritorio (no con el cliente web Bevy). Consulta [Previsualizar con activos optimizados](#preview-with-optimized-assets).
* `-- --no-client` Suprime cualquier auto-inicio (deeplink de Explorer de escritorio, apertura del navegador, QR para móvil). El observador de archivos sigue notificando a un Explorer de escritorio si se conecta por su cuenta. Útil cuando una herramienta externa controla el proceso de Explorer.
* `-- --mcp` Activa el servidor MCP en Explorer (reenviado como un parámetro de deep link).
* `-- --mcp-port` Puerto para el servidor MCP en Explorer (reenviado como un parámetro de deep link). Por ejemplo: `npm run start -- --mcp --mcp-port 3001`.


> [!WARNING]
> **📔 Nota**: Los parámetros deben añadirse con dos series de guiones, por ejemplo `npm run start -- --web3`.


## Previsualizar con activos optimizados

Cuando publicas una escena, los servidores de Decentraland convierten todos sus modelos 3D a asset bundles, un formato optimizado que es mucho más ligero de cargar y renderizar. Consulta [Conversión de asset bundles](/docs/creator/content-creator-es/scenes-sdk7/optimizacion/performance-optimization/#asset-bundle-conversion). De forma predeterminada, las vistas previas locales omiten este paso y cargan los modelos sin procesar en su lugar.

Activa los activos optimizados para ejecutar esta misma conversión localmente cuando previsualices tu escena. Esto tiene dos beneficios:

* La vista previa carga más rápido y funciona con mayor fluidez, especialmente en escenas con modelos 3D pesados.
* Ves los modelos exactamente como los verán los jugadores después de publicar, así puedes detectar cualquier problema con la compresión de activos antes de publicar.

Esta opción solo afecta a tu vista previa local. La escena publicada siempre se convierte en los servidores, uses o no esta opción.

Para habilitarla:

* **En Creator Hub**: abre **Opciones de reproducción** junto al **Reproducir** botón, pasa el cursor sobre **Cliente de escritorio**, y marca **Comprimir activos**.
* **Desde la CLI**: ejecuta `npm run start -- --asset-bundles`.

![](/docs/_remote/b1d24b74a3fe9e049ed203e0.webp)

Con esta opción habilitada, Explorer de Decentraland convierte los modelos 3D de la escena en asset bundles en tu máquina antes de cargarlos. La conversión es casi instantánea para la mayoría de las escenas, pero la primera vista previa puede tardar más si la escena tiene activos muy pesados. Los modelos convertidos se almacenan en caché, así que en vistas previas posteriores solo hay que convertir los activos nuevos o modificados.


> [!NOTE]
> **💡 Consejo**: Si la conversión de un activo falla por cualquier motivo, la vista previa vuelve a cargar los modelos 3D sin procesar, igual que una vista previa normal.


## Sube una escena a Decentraland

Una vez que estés conforme con tu escena, puedes subirla y publicarla en Decentraland. Para ello debes poseer LAND, un NAME de Decentraland o un nombre ENS de ETH, o contar con permisos otorgados por alguien que los tenga. Consulta [publicación](/docs/creator/content-creator-es/scenes-sdk7/publicacion/publishing/) para ver instrucciones sobre cómo hacerlo.

## Previsualizar tamaño de la escena

El tamaño de la escena mostrado en la vista previa se basa en la configuración de la escena.

Edita esto en la segunda pestaña del menú de la escena en Scene Editor.

![](/docs/_remote/5bcc0f6dfa70093b896862ae.webp)

Usa los menús desplegables y haz clic en **Aplicar diseño** para cambiar las dimensiones de tu escena. También puedes hacer clic en cada parcela individual para desactivarla de tu diseño.

![](/docs/_remote/c2905f95052e96703e78829e.webp)

También puedes editar el *scene.json* archivo para listar varias parcelas en el campo "parcels". Consulta [configurar parcelas mediante la línea de comandos](/docs/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/scene-metadata/#scene-parcels) para más detalles.


> [!NOTE]
> **💡 Consejo**: Mientras ejecutas la vista previa, las coordenadas de las parcelas no necesitan coincidir con las que tu escena realmente usará, siempre que sean adyacentes y estén dispuestas en la misma forma. Tendrás que reemplazarlas más tarde por las coordenadas reales cuando [despliegues la escena](#upload-a-scene-to-decentraland).


## Ver la consola de la escena

Abre la consola haciendo clic en el ![](/docs/_remote/92e8eb247fcf289c2feff8f8.webp) icono en la esquina superior derecha. Aquí puedes ver cualquier mensaje de error y también cualquier texto que tu escena imprima en la consola mediante `console.log()`.

También puedes abrirla pulsando la **\`** tecla de tu teclado. También puedes pulsar Shift + **\`** para abrir la consola aún más ancha, por si necesitas ver más texto.

## La consola de Creator Hub

Creator Hub muestra la misma salida sin salir del editor, y lo hace de forma predeterminada. Ejecuta una vista previa y una **CONSOLA** pestaña aparece junto a **SISTEMA DE ARCHIVOS** y **PAQUETES DE ACTIVOS** en el panel de la parte inferior del editor.

Para evitar que la consola se abra, desmarca **Abrir consola de depuración** en **Opciones de reproducción** en la **EDITOR** pestaña de la configuración de Creator Hub.

Para mover la consola fuera del camino, haz clic en el icono emergente de su encabezado, etiquetado **Abrir consola en una ventana separada**. La consola se abre como una ventana independiente titulada **Consola**, que puedes mover a un segundo monitor y mantener visible mientras editas.

La ventana separada tiene un **Limpiar consola** botón, y un **Volver a acoplar en el editor** botón que devuelve la consola al panel inferior. Mientras tanto, la **CONSOLA** pestaña del editor dice **Consola abierta en una ventana separada** y ofrece un **Volver a acoplarla aquí** botón que hace lo mismo.


> [!NOTE]
> **💡 Consejo**: La consola sigue recopilando salida mientras está desacoplada, así que volver a acoplarla no hace que se pierda nada. Cuando no hay ninguna vista previa en ejecución, dice **Ejecuta una escena para ver la salida de depuración**.


## Probar localmente una escena multijugador

Si inicias una vista previa de la escena y la abres en dos (o más) ventanas diferentes de Explorer, cada ventana abierta se interpretará como un jugador separado, y un servidor simulado de comunicaciones mantendrá a estos jugadores sincronizados.

Interactúa con la escena en una ventana y luego cambia a la otra para ver que los efectos de esa interacción también son visibles allí.

Usando Creator Hub, activa **Vista previa multiinstancia** en el **Cliente de escritorio** submenú de **Opciones de reproducción**, luego haz clic en el **Reproducir** botón una segunda vez para abrir una segunda ventana de Explorer de Decentraland. Debes conectarte en ambas ventanas con direcciones diferentes. Las mismas sesiones permanecerán abiertas mientras la escena se recarga.

![](/docs/_remote/ca7e6550cdb1bb947ce74b28.webp)

Como alternativa, puedes abrir una segunda ventana de Explorer de Decentraland escribiendo lo siguiente en una URL del navegador:

> `decentraland://realm=http://127.0.0.1:8000&local-scene=true&debug=true&multi-instance=true`

### Avanzado: iteración rápida con asset bundles remotos


> [!NOTE]
> **💡 Consejo**: Para la mayoría de las escenas, la forma más sencilla de previsualizar con asset bundles es activar [activos optimizados](#preview-with-optimized-assets), lo que convierte tus modelos localmente y refleja siempre su versión más reciente. El modo de abajo es una alternativa que reutiliza los bundles ya publicados en los servidores.


En escenas pesadas con muchos modelos 3D, puedes acelerar la carga y recarga de la escena reutilizando los [asset bundles](/docs/creator/content-creator-es/scenes-sdk7/optimizacion/performance-optimization/#asset-bundle-conversion) que ya están publicados en los servidores de Decentraland, en lugar de cargar los modelos 3D sin optimizar. Esto es especialmente útil al iterar sobre cambios solo de código.

Para activar este modo, inicia el cliente de escritorio de Decentraland con los siguientes argumentos:

```bash
npm run start -- --realm http://127.0.0.1:8000/ --position 0,0 --local-scene true --debug --skip-version-check true --lsd-use-remote-ab <ab-source>
```

El `<ab-source>` argumento cambia según dónde esté ya publicada la escena:

* **En Genesis City**: `--lsd-remote-ab-server Genesis`
* **En un World**: `--lsd-remote-ab-world <world-name>.dcl.eth`

Por ejemplo, para previsualizar una copia local de una escena que ya está desplegada en un World:

```bash
npm run start -- --realm http://127.0.0.1:8000/ --position 0,0 --local-scene true --debug --skip-version-check true --lsd-use-remote-ab --lsd-remote-ab-world myworld.dcl.eth
```

En ambos casos, `--realm http://127.0.0.1:8000/` apunta el cliente a tu servidor local de vista previa (ejecútalo `npm run start` primero para iniciarlo), y `--local-scene true` le indica al cliente que cargue desde allí el código de la escena.


> [!WARNING]
> **📔 Importante**: Al usar este modo, se recomienda que **todos** sus elementos artísticos ya estén publicados, con sus asset bundles completamente procesados por los servidores de contenido. Si has añadido nuevos activos, no te beneficiarás de la carga optimizada, ya que se cargarán como archivos glTF sin procesar, tal como ocurre cuando ejecutas una vista previa normal. Pero si modificaste localmente un activo que ya estaba publicado, manteniendo el mismo nombre de archivo, entonces estarás viendo la versión publicada antigua de ese activo.
> 
> En ese caso, vuelve a desplegar primero la escena, espera unos minutos a que se generen los asset bundles (consulta [Conversión de asset bundles](/docs/creator/content-creator-es/scenes-sdk7/optimizacion/performance-optimization/#asset-bundle-conversion)), y luego retoma el uso de este modo para iterar solo en el código.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/primeros-pasos/preview-scene.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
