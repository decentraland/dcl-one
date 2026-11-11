> Soluciones para problemas comunes

# Solución de problemas

## Depura con un asistente de IA

Antes de revisar los problemas de abajo, considera pasarle el problema a un asistente de IA, como el chat de Cursor, GitHub Copilot o Claude Code. Pega el mensaje de error de la consola, o describe qué no se está comportando como esperas, y por lo general podrá encontrar el problema en el código de tu escena y arreglarlo por ti.

Para obtener buenos resultados, asegúrate de que la IA tenga instaladas las skills de Decentraland SDK. Las skills le enseñan los patrones y las restricciones del SDK, para que no adivine basándose en información genérica o desactualizada. Instálalas ejecutando este comando en el proyecto de tu escena:

```bash
npx skills add decentraland/sdk-skills
```

Consulta [Vibe Coding con IA](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/vibe-coding/) para más detalles sobre la configuración y sobre cómo darle prompts a los asistentes de IA.

También puedes dejar que la IA depure la escena *mientras se está ejecutando*. El cliente de escritorio de Decentraland puede exponer un servidor MCP que permite a un agente tomar sus propias capturas de pantalla, leer la salida de la consola de la escena, mover al jugador por el entorno y hacer clic en objetos — así que, en lugar de que tú reproduzcas el error y pegues el fallo, el agente lo reproduce por sí mismo, ve qué ocurre e itera hasta que quede resuelto. Inicia la escena con `npm run start -- --mcp` y conecta tu agente a él.

Consulta [Deja que la IA vea tu escena en el mundo](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/vibe-coding/#let-the-ai-see-your-scene-in-world) para ver la configuración completa, e instala el `unity-explorer-mcp` skill para que el agente conozca el flujo de trabajo:

```bash
npx skills add decentraland/sdk-skills --skill unity-explorer-mcp
```

## Problemas al ejecutar preview

#### Problema: No se puede ejecutar ningún preview de la escena, el mensaje de error menciona **Permisos denegados** o **EACCES**

Tu sistema operativo no te permite editar permisos en la carpeta donde quieres ejecutar el proyecto. Al ejecutar la escena, algunas dependencias necesitan instalarse, pero está prohibido. Debes configurar los permisos de la carpeta para permitir que tu cuenta de usuario de Windows/Mac/Linux pueda editar los archivos dentro de ella.

Recursos útiles:

* [docs.npmjs](https://docs.npmjs.com/resolving-eacces-permissions-errors-when-installing-packages-globally)
* [letscodepare](https://letscodepare.com/blog/npm-resolving-eacces-permissions-denied)

#### Problema: No se puede ejecutar un preview concreto de la escena, el error dice **Error: Error durante la compilación del proyecto**

Si estás ejecutando una escena que compartieron contigo, asegúrate de que esta escena no se haya compartido incluyendo una `node_modules` o `bin` carpeta, o un `package-lock.json` archivo. Estos archivos contienen dependencias que usan versiones específicas de tu sistema operativo y de tu máquina; deberían generarse al ejecutar la escena por primera vez. Elimina estas carpetas y el archivo manualmente, y luego ejecuta `npm run start` de nuevo.

#### Problema: Al ejecutar `npm run start` se ejecuta, no aparece ningún mensaje de error, pero no se abre ninguna ventana del navegador y no hay ninguna URL en la salida para abrir el preview

Asegúrate de que tu versión de Node esté actualizada. Debe ser 20 o superior.

#### Problema: Al ejecutar `npm run start` abre una pestaña del navegador, pero la pantalla de carga nunca termina de cargar, o veo un banner de error rojo que dice "critical error".

* Asegúrate de tener instalada en tu proyecto la última versión de Decentraland SDK. Ejecuta:

  `npm i @dcl/sdk@latest`

#### Problema: La escena se ejecuta, en la consola veo `Dependencias cíclicas` advertencias.

Esto se refiere a archivos de tu escena que se referencian mutuamente entre sí. Esto no es necesariamente un problema, pero no es un patrón recomendable para escribir software, ya que puede provocar condiciones de carrera difíciles de depurar y otros problemas. Es probable que tu escena funcione bien a pesar de estas advertencias.

Lo ideal es que la carga del código en tu escena siga un orden secuencial claro. El código que tiene dependencias cíclicas puede sufrir un problema de la gallina y el huevo, en el que el compilador no sabe cuál iniciar primero. A menudo esto se resuelve sin problemas, pero es una buena práctica evitarlo.

Para corregir estas dependencias, a menudo debes recurrir a llamar funciones o constructores de objetos pasando referencias a entidades/objetos ya instanciados en los argumentos de la función; en lugar de codificar de forma fija las referencias a estas entidades/objetos en la función, que puede que ya estén instanciadas o no.

## Problemas al hacer deploy

#### Problema: No tienes permisos para hacer deploy en estos parcels

* Asegúrate de que el `scene.json` archivo de tu escena enumera correctamente las coordenadas donde quieres hacer deploy.
* Asegúrate de que Metamask esté correctamente configurado para usar la wallet adecuada para firmar la transacción. Puede ser la wallet que posee los tokens LAND, o una que tenga permisos de operador otorgados por el propietario.

#### Problema: Al ejecutar `npm run deploy` falla

* Revisa los spawn points de tu escena: las tres coordenadas x, y, z de un spawn point deben ser un número o un rango. O las tres son números o las tres son rangos. No se admite tener rangos para algunas y números para otras.

  Por ejemplo, esto no se admite:

  `"position": {"x": [1,4], "y": 0, "z": [1,4]}`

  Esto sí se admite:

  `"position": {"x": [1,4], "y": [0,0], "z": [1,4]}`
* El servidor catalyst predeterminado al que se te asigna desplegar podría estar caído o tener problemas. Puedes forzar el `npm run deploy` comando para desplegar en un servidor catalyst específico en su lugar. Para desplegar en un servidor específico en el Decentraland Editor:

  1. Abre tu escena y haz clic en **Publicar**
  2. Selecciona la opción **Publicar en un servidor diferente** en la parte inferior.
  3. En el desplegable, selecciona **Servidor personalizado**
  4. Introduce la dirección del servidor, por ejemplo `peer-ec1.decentraland.org`
  5. Haz clic **Publicar en servidor personalizado**
  6. Aprueba la transacción como en un despliegue normal.

  Para hacerlo a través de la CLI:

  `npm run deploy -- --target-content <server-name>`

  Por ejemplo:

  `npm run deploy -- --target-content peer-ec1.decentraland.org`

  Consulta [catalyst-monitor](https://decentraland.github.io/catalyst-monitor/) para comprobar el estado de todos los servidores de la red catalyst. También puedes copiar las direcciones de cada uno desde la parte superior de cada tarjeta.
* Revisa el `package.json`. Un problema común es que hay un `bundleDependencies` y también un `bundledDependencies` (sección con una d extra). Esto a veces puede deberse a ejecutar distintas versiones de Node en el mismo proyecto en distintos momentos, o a compartir el proyecto entre personas que lo ejecutaron con diferentes versiones de Node instaladas. Elimina `bundleDependencies`, que se relaciona con versiones antiguas de Node.

Asegúrate también de tener tu versión de Node actualizada, al menos la versión 20.

#### Problema: Al ejecutar `npm run deploy` o `npm run build` reporta errores de tipos

Tu escena podría tener errores de tipo reportados por TypeScript, por ejemplo indicando que cierta variable podría ser de tipo `cualquier` o que `undefined` o `null` no están permitidos. Al ejecutar `npm run deploy`, también ejecuta `npm run build`, que es un poco más estricto con estas comprobaciones que `npm run start`.

A diferencia de JavaScript, TypeScript impone una tipificación estricta de todas las variables. Aunque tu escena está escrita de tal manera que, por ejemplo, cierto valor nunca será `undefined`, TypeScript necesita saber qué ocurriría en ese escenario, o necesitas aclarar explícitamente que el valor solo puede ser, por ejemplo, una cadena de texto.

Como alternativa, puedes ejecutar `npm run deploy -- --skip-build` para omitir la ejecución de `npm run build`, y evitar que se ejecuten estas comprobaciones.

#### Problema: Desplegué mi escena pero no veo los cambios cuando entro en Decentraland

* Ten en cuenta que puede tardar unos minutos si la escena contiene assets pesados.
* Mientras los modelos 3D de la nueva versión se convierten en asset bundles, a los jugadores se les sirve deliberadamente la última versión completamente funcional de tu escena. Esto normalmente solo tarda unos segundos, pero puede tardar más en escenas muy grandes o cuando los servidores están ocupados. Puedes [comprobar el estado de la conversión](/docs/creator/content-creator-es/scene-editor/publicar/publish-scene/#check-the-conversion-status) de tu escena directamente.
* Recargar la escena no es suficiente para tomar una nueva versión: una recarga reinicia el código de la escena, pero no obtiene la versión recién publicada. Una vez que la conversión esté completa, cierra completamente y vuelve a abrir Decentraland, y luego entra de nuevo a la escena mediante un enlace de salto o el `/goto` comando de chat.

#### Problema: Algunos jugadores ven la nueva versión de mi escena, otros siguen viendo la antigua

La conversión de los modelos 3D de tu escena termina en distintos momentos para cada plataforma (Windows y Mac), y el cliente de cada jugador también mantiene una caché local. Para comprobar si la conversión ya terminó en todas las plataformas, [comprobar el estado de la conversión](/docs/creator/content-creator-es/scene-editor/publicar/publish-scene/#check-the-conversion-status) y compara los `Windows` y `Mac` valores bajo `assetBundles`. Una vez que ambos estén `completos`, pídeles a los jugadores afectados que cierren completamente y vuelvan a abrir Decentraland.

#### Problema: La publicación se queda atascada en la etapa Converting y el botón Jump In nunca aparece

Tu escena está en cola detrás de otras escenas que se están convirtiendo, o la conversión falló.

* Abrir `https://asset-bundle-registry.decentraland.org/queues/status` y busca el entity ID de tu escena. Puedes encontrar ese ID en el `entityId` campo cuando [comprobar el estado de la conversión](/docs/creator/content-creator-es/scene-editor/publicar/publish-scene/#check-the-conversion-status) usas las coordenadas de tu escena. Si tu ID aparece en la cola, la escena está en espera y solo tienes que aguardar.
* Si no está en la cola, [comprobar el estado de la conversión](/docs/creator/content-creator-es/scene-editor/publicar/publish-scene/#check-the-conversion-status) de tu escena. Si una plataforma muestra `fallida`, vuelve a publicar la escena. Si falla una segunda vez, [informa del problema](/docs/creator/content-creator-es/scenes-sdk7/depuracion/report-bug/) e incluye el entity ID.

#### Problema: Una vez desplegada, faltan algunos modelos 3D, aparecen en negro o sin texturas

* Si acabas de publicar, puede que la conversión de tus modelos a asset bundles aún no haya terminado. Escribe `/detectabs` en la ventana de chat: los modelos teñidos de rojo aún no se han convertido. [Comprueba el estado de la conversión](/docs/creator/content-creator-es/scene-editor/publicar/publish-scene/#check-the-conversion-status) y espera a que termine.
* Asegúrate de que todos los modelos 3D estén dentro de los límites de la escena, incluidas sus bounding boxes. Si alguna parte de tus modelos se extiende más allá de estos límites al ejecutar un preview, esas partes que sobresalen se recortarán y no se renderizarán, tanto en el preview como en la escena publicada.
* Si las texturas son el problema, ten en cuenta que las texturas de los modelos 3D están limitadas a un tamaño máximo de 1024x1024 píxeles durante la conversión.

#### Problema: La escena se ve bien de cerca, pero está rota o falta contenido cuando se ve desde lejos

Las versiones de bajo Level of Detail (LOD) de tus assets, usadas para renderizar la escena desde lejos, se generan en la última etapa de la publicación y puede que aún no hayan terminado. Esto no bloquea probar tu escena de cerca. [Comprueba el estado de la conversión](/docs/creator/content-creator-es/scene-editor/publicar/publish-scene/#check-the-conversion-status) y mira los valores bajo `lods`, o simplemente espera y vuelve a comprobarlo más tarde.

#### Problema: Una vez desplegados, mis modelos 3D se ven diferentes

* Si las texturas se ven diferentes, ten en cuenta que las texturas de los modelos 3D están limitadas a un tamaño máximo de 1024x1024 píxeles. Esta conversión se realiza para garantizar que Decentraland funcione sin problemas para todos.
* Si los modelos se ven diferentes, podría haber un problema con la conversión de los modelos a asset bundles. Lee más sobre la compresión de asset bundles [aquí](/docs/creator/content-creator-es/scenes-sdk7/optimizacion/performance-optimization/#asset-bundle-conversion).

  Para validar esto, prueba a ejecutar la escena con el parámetro de URL `&DISABLE_ASSET_BUNDLES`. Si los modelos se ven bien con esta bandera, el problema debe estar relacionado con un bug en la conversión del modelo. En ese caso, [informa del problema](/docs/creator/content-creator-es/scenes-sdk7/depuracion/report-bug/) e incluye el entity ID de tu despliegue.

  Ten en cuenta que la generación de versiones comprimidas de asset bundles de tus modelos lleva un poco de tiempo a los servidores (normalmente unos minutos, más para escenas muy grandes o cuando los servidores están ocupados). Puedes comprobar si los modelos se están cargando como asset bundles comprimidos o no escribiendo el siguiente comando en la ventana de chat `/detectabs`. Los modelos comprimidos se tiñen de verde, los no comprimidos se tiñen de rojo.

  También puedes reproducir esta conversión localmente antes de publicar, habilitando [assets optimizados en el preview](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/preview-scene/#preview-with-optimized-assets). Esto te permite detectar y depurar estos problemas sin tener que desplegar la escena.

#### Problema: Mi escena desapareció por completo

* Si la escena está en un World: puede que hayas superado el presupuesto de almacenamiento de Worlds, por ejemplo después de vender o transferir NAMEs, LAND o MANA. Tienes 48 horas para liberar espacio o aumentar tu presupuesto antes de que tus Worlds se vuelvan inaccesibles. Comprueba tu presupuesto en la **Manage** pestaña de Creator Hub o en el **Worlds** pestaña de la [Builder](https://decentraland.org/builder/worlds), y luego vuelve a publicar. Consulta [Límites de tamaño de Worlds](/docs/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/kinds-of-project/#size-limits).
* Si la escena está en LAND: alguien con permisos de deploy puede haber publicado una nueva escena sobre tus parcels, lo que borra el contenido anterior. Consulta [sobrescritura de escenas](/docs/creator/content-creator-es/scenes-sdk7/publicacion/publishing/#scene-overwriting).
* En casos raros, el contenido que viola la política de contenido de Decentraland puede ser incluido en una denylist de los servidores de contenido. Si crees que esto ocurrió por error, ponte en contacto a través de la [Discord de Decentraland](https://decentraland.org/discord).

#### Problema: Mi escena tiene un FPS bajo en producción, aunque se ejecuta sin problemas en preview.

El rendimiento de tu escena podría verse afectado por escenas vecinas que siguen malas prácticas, ya que también se ejecutan en paralelo. Puedes validar si este es el caso abriendo los ajustes y configurando la línea de visión al mínimo, de modo que solo se cargue 1 parcel alrededor de tu escena actual.

Puedes reducir la línea de visión aún más ejecutando tu escena con el parámetro `&LOS=0`, para no cargar ninguna escena circundante en absoluto.

Si acabas de desplegar tu escena, la carga al abrir la escena también podría reducirse una vez que los servidores conviertan los modelos 3D de la escena en asset bundles comprimidos. Puedes comprobar si los modelos se están cargando como asset bundles comprimidos o no escribiendo el siguiente comando en la ventana de chat `/detectabs`. Los modelos comprimidos se tiñen de verde, los no comprimidos se tiñen de rojo.

### Reportar un error

Si encuentras un problema que no sea con tu escena, sino con el SDK de Decentraland en general, consulta [Reportar un error](/docs/creator/content-creator-es/scenes-sdk7/depuracion/report-bug/).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/depuracion/troubleshooting.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
