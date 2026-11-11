> Qué ha cambiado en el SDK y en Creator Hub durante el último año, filtrado a los cambios que afectan a los creadores.

# Notas de la versión

Esta página enumera los cambios en el [SDK](#sdk) y en el [Creator Hub](#creator-hub) del último año que afectan a cómo construyes escenas. Se omite el trabajo interno, como las herramientas de build, las pruebas, el seguimiento y los refactors. Cada elemento enlaza a su pull request en GitHub, donde puedes encontrar más detalles.

Para ver las notas completas y sin filtrar, consulta las [SDK releases](https://github.com/decentraland/js-sdk-toolchain/releases) y en el [Creator Hub releases](https://github.com/decentraland/creator-hub/releases) en GitHub.


> [!NOTE]
> Las Creator Hub releases agrupan los cambios de `@dcl/inspector` (el scene editor) y `@dcl/asset-packs` (smart items). Esos cambios se enumeran bajo la versión de Creator Hub en la que se publicaron por primera vez.


## SDK

### 7.31.0 (22 de oct. de 2026)

* **Fuentes personalizadas**: la nueva `fontSrc` propiedad renderiza texto con un `.ttf` archivo de fuente de tu escena, en `TextShape`, `UiText`, `UiInput` y `UiDropdown` y en React ECS `Label`, `Button`, `Input` y `Dropdown`. Consulta [Fuentes personalizadas](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/text/#custom-fonts) y [Fuentes personalizadas en la UI](/docs/creator/content-creator-es/scenes-sdk7/ui-2d/ui_text/#custom-fonts). [#1656](https://github.com/decentraland/js-sdk-toolchain/pull/1656)
* **Publicar escenas grandes**: `npm run deploy` sube escenas de 50 MiB o más en lotes, por lo que se pueden publicar escenas de más de unos 200 MB, y la página de deploy en el navegador muestra el progreso de la subida y te permite cancelarla. Ver [Publicar escenas grandes](/docs/creator/content-creator-es/scenes-sdk7/publicacion/publishing/#publish-large-scenes). [#1635](https://github.com/decentraland/js-sdk-toolchain/pull/1635)
* **Correcciones**: la vista previa local sirve cada entity de la escena una sola vez, por lo que las escenas que abarcan muchos parcels cargan en la vista previa [#1650](https://github.com/decentraland/js-sdk-toolchain/pull/1650).

### 7.30.0 y 7.30.1 (8 de oct. de 2026)

* **Idioma del jugador**: `getPlayerLanguage()` devuelve el idioma al que está configurado el cliente del jugador, como una etiqueta BCP 47, y `onPlayerLanguageChanged` informa de los cambios realizados a mitad de sesión. Ver [Obtener el idioma del jugador](/docs/creator/content-creator-es/scenes-sdk7/interactividad/runtime-data/#get-player-language). [#1622](https://github.com/decentraland/js-sdk-toolchain/pull/1622)
* **Esperar a que se cierre un panel del explorer**: la nueva `openExplorerUiAndWait()` helper en `@dcl/sdk/explorer-ui` abre un panel y se resuelve una vez que el jugador lo cierra, con un límite de tiempo opcional. Ver [Esperar hasta que el jugador cierre un panel](/docs/creator/content-creator-es/scenes-sdk7/interactividad/event-listeners/#wait-until-the-player-closes-a-panel). [#1543](https://github.com/decentraland/js-sdk-toolchain/pull/1543)
* **Correcciones**: el bundle watcher ignora `node_modules` [#1640](https://github.com/decentraland/js-sdk-toolchain/pull/1640); la vista previa vuelve a comprobar la escena del editor en cada reconstrucción de watch [#1644](https://github.com/decentraland/js-sdk-toolchain/pull/1644).

### 7.29.0 (15 de sep. de 2026)

* **Orden de apilado entre módulos de UI**: una opción de renderer `zIndex` decide qué módulo de UI se dibuja delante de cuál. Sin ella, los módulos se apilan en el orden en que aparecen por primera vez, con la UI principal al fondo. Ver [Orden de apilado entre módulos de UI](/docs/creator/content-creator-es/scenes-sdk7/ui-2d/onscreen-ui/#stacking-order-between-ui-modules). [#1607](https://github.com/decentraland/js-sdk-toolchain/pull/1607)
* **Correcciones**: quitar un manejador de eventos de puntero ahora elimina la entrada que creó, por lo que un botón de UI deshabilitado deja de anunciar un clic que ya no puede manejar, y volver a registrar manejadores ya no acumula entradas duplicadas. [#1577](https://github.com/decentraland/js-sdk-toolchain/pull/1577)

### 7.28.0 (10 de sep. de 2026)

* **Los eventos de puntero usan la distancia de la cámara**: `maxCameraDistance` reemplaza a `maxPlayerDistance` en los eventos de puntero. El campo anterior sigue funcionando, pero está obsoleto. Los hints y los clics ahora dependen de a qué distancia está la cámara del item, lo que coincide con lo que ve el jugador. [#1560](https://github.com/decentraland/js-sdk-toolchain/pull/1560)
* **Componente AvatarNametag**: personaliza la etiqueta con el nombre que se muestra sobre los avatars. [#1600](https://github.com/decentraland/js-sdk-toolchain/pull/1600)
* **Teletransportarse a un realm**: `teleportTo()` puede apuntar a un realm específico. [#1601](https://github.com/decentraland/js-sdk-toolchain/pull/1601)
* **Correcciones**: una UI deshabilitada `Button` ya no atenúa los colores de la escena [#1578](https://github.com/decentraland/js-sdk-toolchain/pull/1578); el input de puntero de una entity ya no oculta el de otra en el mismo frame [#1576](https://github.com/decentraland/js-sdk-toolchain/pull/1576); los `ActionCallback` parámetros en los smart items Script se resuelven como `undefined` en lugar de fallar [#1605](https://github.com/decentraland/js-sdk-toolchain/pull/1605); los composites se instancian correctamente en la RootEntity [#1587](https://github.com/decentraland/js-sdk-toolchain/pull/1587); los campos opcionales del componente establecidos en `0` or `false` ahora se guardan [#1582](https://github.com/decentraland/js-sdk-toolchain/pull/1582); los componentes definidos a partir de un esquema primitivo o de array se pueden leer de vuelta [#1593](https://github.com/decentraland/js-sdk-toolchain/pull/1593); varias correcciones de robustez de serialización y networking [#1592](https://github.com/decentraland/js-sdk-toolchain/pull/1592), [#1595](https://github.com/decentraland/js-sdk-toolchain/pull/1595), [#1568](https://github.com/decentraland/js-sdk-toolchain/pull/1568), [#1570](https://github.com/decentraland/js-sdk-toolchain/pull/1570).

### 7.27.0 (27 de ago. de 2026)

* **Mejoras en los helpers de eventos**: los sistemas de helpers de eventos de puntero, trigger, audio y otros ahora tienen nombres únicos y un comportamiento consistente. [#1536](https://github.com/decentraland/js-sdk-toolchain/pull/1536)
* **Estado de la pantalla de carga**: `EngineInfo` te indica si la pantalla de carga sigue visible. [#1555](https://github.com/decentraland/js-sdk-toolchain/pull/1555)
* **Builds más rápidas**: los archivos del proyecto se procesan en paralelo al hacer build y deploy. [#1481](https://github.com/decentraland/js-sdk-toolchain/pull/1481)
* **Correcciones**: los eventos de click se detectan para todos los jugadores, no solo para el primero [#1468](https://github.com/decentraland/js-sdk-toolchain/pull/1468); los timers siguen funcionando después de que una callback lanza una excepción [#1470](https://github.com/decentraland/js-sdk-toolchain/pull/1470); las entitys de UI de React ECS desmontadas se liberan, corrigiendo una fuga de memoria [#1471](https://github.com/decentraland/js-sdk-toolchain/pull/1471); las suscripciones observables ya no interfieren entre sí [#1467](https://github.com/decentraland/js-sdk-toolchain/pull/1467); el paso de inicio de sesión al hacer deploy reenvía los headers correctamente [#1551](https://github.com/decentraland/js-sdk-toolchain/pull/1551); los IDs de entity reservados por el renderer nunca se generan ni se reciclan [#1544](https://github.com/decentraland/js-sdk-toolchain/pull/1544), [#1460](https://github.com/decentraland/js-sdk-toolchain/pull/1460).

### 7.26.0 (13 de ago. de 2026)

* **Componente TouchScreenControls**: muestra, oculta y personaliza los controles en pantalla en móviles. Ver [Controles en pantalla](/docs/creator/content-creator-es/scenes-sdk7/interactividad/touch-screen-controls/). [#1441](https://github.com/decentraland/js-sdk-toolchain/pull/1441), [#1510](https://github.com/decentraland/js-sdk-toolchain/pull/1510)
* **Abrir la UI de Explorer desde tu escena**: la `openExplorerUi()` acción restringida abre el mapa, la mochila, la configuración y otras pantallas del explorer, y `ExplorerUiEventsResult` informa cuando se cierran. [#1503](https://github.com/decentraland/js-sdk-toolchain/pull/1503), [#1511](https://github.com/decentraland/js-sdk-toolchain/pull/1511)
* **Eventos de audio y estado de emote**: la nueva `audioEventSystem` reacciona a los eventos de reproducción de audio, y `AvatarEmoteCommand` expone el emote `state`. [#1530](https://github.com/decentraland/js-sdk-toolchain/pull/1530)
* **Valores predeterminados de la pantalla de UI**: la UI usa una pantalla virtual predeterminada y un área de inset de pantalla, y la relación de píxeles del dispositivo ya no afecta al layout. [#1489](https://github.com/decentraland/js-sdk-toolchain/pull/1489)
* **Vista previa de Bevy Web**: la vista previa del navegador usa el Explorer de Bevy Web en lugar del Explorer web heredado. [#1502](https://github.com/decentraland/js-sdk-toolchain/pull/1502)
* **Assets optimizados en la vista previa**: la vista previa puede cargar tus modelos como paquetes de assets, coincidiendo con cómo se ven una vez publicados. Ver [Vista previa con assets optimizados](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/preview-scene/#preview-with-optimized-assets). [#1539](https://github.com/decentraland/js-sdk-toolchain/pull/1539)
* **Correcciones**: `npm run start` sigue activo cuando falla la build inicial, así puedes corregir el error y se recompila [#1501](https://github.com/decentraland/js-sdk-toolchain/pull/1501); los puertos ocupados se detectan correctamente [#1542](https://github.com/decentraland/js-sdk-toolchain/pull/1542).

### 7.24.5 (15 de jul. de 2026)

* **Objetivo de Billboard**: una opción de renderer `Billboard` puede orientarse hacia una entity objetivo en lugar de hacia la cámara. [#1445](https://github.com/decentraland/js-sdk-toolchain/pull/1445)
* **Bundles de escena más pequeños**: una segunda copia de `@dcl/ecs` ya no se incluye en cada escena. [#1450](https://github.com/decentraland/js-sdk-toolchain/pull/1450)
* **Corrección**: `MessageBus` los mensajes se envían en orden. [#1462](https://github.com/decentraland/js-sdk-toolchain/pull/1462)

### 7.24.4 (3 de jul. de 2026)

* **InteractableArea para React ECS**: nuevo componente de UI para áreas interactuables en pantalla. [#1411](https://github.com/decentraland/js-sdk-toolchain/pull/1411)

### 7.24.3 (1 de jul. de 2026)

* **Se acabó el límite de parcela 45x45** para escenas. [#1429](https://github.com/decentraland/js-sdk-toolchain/pull/1429)
* **Doble salto y planeo**: `AvatarLocomotionSettings` tiene campos para activar y desactivar el doble salto y el planeo. [#1402](https://github.com/decentraland/js-sdk-toolchain/pull/1402)
* **Corrección**: la pantalla virtual de UI tiene en cuenta la relación de píxeles del dispositivo. [#1433](https://github.com/decentraland/js-sdk-toolchain/pull/1433)

### 7.24.2 (25 de jun. de 2026)

* **Correcciones**: un rearmado de `setTimeout` ya no bloquea la escena después de un frame largo [#1420](https://github.com/decentraland/js-sdk-toolchain/pull/1420); React ECS mantiene una prop opcional que se establece por primera vez [#1422](https://github.com/decentraland/js-sdk-toolchain/pull/1422); los errores de "Premature close" durante el deploy y otras solicitudes de CLI han desaparecido [#1428](https://github.com/decentraland/js-sdk-toolchain/pull/1428).

### 7.24.1 (12 de jun. de 2026)

* **Capa de colisión del jugador**: una nueva capa de colisión para el jugador principal. [#1414](https://github.com/decentraland/js-sdk-toolchain/pull/1414)
* **Corrección**: al volver a disparar un `AudioSource` ya no reproduce sonidos duplicados. [#1382](https://github.com/decentraland/js-sdk-toolchain/pull/1382)

### 7.24.0 (4 de jun. de 2026)

* **ScreenInsetArea**: componente de React ECS que mantiene la UI dentro del área segura en móviles. Ver [Área segura móvil](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/safe-area/). [#1386](https://github.com/decentraland/js-sdk-toolchain/pull/1386)
* **Proveedor de composite**: `getCompositeProvider()` y `setCompositeProvider()` en el engine, para un control avanzado de cómo se cargan los composites. [#1333](https://github.com/decentraland/js-sdk-toolchain/pull/1333)

### 7.23.2 (13 de may. de 2026)

* **Corrección**: las trigger areas ya no saturan de mensajes de red mientras un jugador permanece dentro. [#1397](https://github.com/decentraland/js-sdk-toolchain/pull/1397)

### 7.23.1 (5 de may. de 2026)

* **Análisis de audio**: leer datos en vivo de audio sources en reproducción para impulsar efectos. [#1256](https://github.com/decentraland/js-sdk-toolchain/pull/1256)

### 7.23.0 (5 de may. de 2026)

* **Componente AvatarLocomotionSettings**: controla los ajustes de caminar, correr y saltar del jugador. [#1389](https://github.com/decentraland/js-sdk-toolchain/pull/1389)
* **Acción de input Modifier**: `IA_MODIFIER` está disponible en la lista de comandos de input. [#1390](https://github.com/decentraland/js-sdk-toolchain/pull/1390)

### 7.22.5 (23 de abr. de 2026)

* **Bundles más pequeños para escenas solo de código**: la biblioteca de smart items solo se incluye cuando la escena usa un composite. [#1381](https://github.com/decentraland/js-sdk-toolchain/pull/1381)

### 7.22.4 (16 de abr. de 2026)

* **Sourcemaps excluidos de los deployments** mediante `.dclignore`. [#1373](https://github.com/decentraland/js-sdk-toolchain/pull/1373)
* **Ráfagas del sistema de partículas** propiedad actualizada. [#1375](https://github.com/decentraland/js-sdk-toolchain/pull/1375)

### 7.22.3 (8 de abr. de 2026)

* **Componente ParticleSystem**: emite partículas desde una entity. Ver [Sistema de Partículas](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/particle-system/). [#1369](https://github.com/decentraland/js-sdk-toolchain/pull/1369)
* **Menos tráfico de red**: escribir un valor sin cambios en un componente mutable ya no envía un mensaje. [#1359](https://github.com/decentraland/js-sdk-toolchain/pull/1359)
* **Errores de deploy más claros** cuando el World no existe, no tienes permisos o un catalyst no está disponible. [#1361](https://github.com/decentraland/js-sdk-toolchain/pull/1361), [#1370](https://github.com/decentraland/js-sdk-toolchain/pull/1370)

### 7.22.2 (26 de mar. de 2026)

* **Interacciones de proximidad**: reacciona cuando el jugador se acerca a una entity. Ver [Eventos de proximidad](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/proximity-events/). [#1356](https://github.com/decentraland/js-sdk-toolchain/pull/1356)

### 7.22.1 (26 de mar. de 2026)

* **Página de inicio de sesión actualizada** usada al desplegar desde la CLI. [#1362](https://github.com/decentraland/js-sdk-toolchain/pull/1362)

### 7.22.0 (25 de mar. de 2026)

* **Fuerza e impulso físicos**: empuja al jugador con fuerzas e impulsos. Ver [Player Physics](/docs/creator/content-creator-es/scenes-sdk7/interactividad/player-physics/). [#1338](https://github.com/decentraland/js-sdk-toolchain/pull/1338)
* **Worlds multiscene**: despliega varias escenas en el mismo World. [#1355](https://github.com/decentraland/js-sdk-toolchain/pull/1355)
* **Deploy desde CI** pipelines. [#1357](https://github.com/decentraland/js-sdk-toolchain/pull/1357)
* **Correcciones**: reordenar listas con clave en React ECS [#1351](https://github.com/decentraland/js-sdk-toolchain/pull/1351); los tipos de acción se excluyen de la sincronización de red [#1354](https://github.com/decentraland/js-sdk-toolchain/pull/1354).

### 7.21.0 (10 de mar. de 2026)

* **Desactiva el doble salto y el planeo** en tu escena. [#1323](https://github.com/decentraland/js-sdk-toolchain/pull/1323)
* **Modo Tween MoveRotateScale**: anima posición, rotación y escala en un solo tween. [#1339](https://github.com/decentraland/js-sdk-toolchain/pull/1339)
* **Detección de plataforma**: sabe si la escena se ejecuta en escritorio, web o móvil. Ver [Detectar la plataforma](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/detect-platform/). [#1340](https://github.com/decentraland/js-sdk-toolchain/pull/1340)

### 7.20.4 (3 de mar. de 2026)

* **Vista previa con varias instancias**: ejecuta varias instancias de jugador contra una sola vista previa para probar el multiplayer. [#1336](https://github.com/decentraland/js-sdk-toolchain/pull/1336)
* **Correcciones**: las colisiones de IDs de entity [#1332](https://github.com/decentraland/js-sdk-toolchain/pull/1332); `UiInput` ya no devuelve su valor [#1331](https://github.com/decentraland/js-sdk-toolchain/pull/1331).

### 7.20.2 y 7.20.3 (23 de feb. de 2026)

* **El QR móvil incluye la posición** en la que aparecer. [#1328](https://github.com/decentraland/js-sdk-toolchain/pull/1328)
* **Correcciones**: los valores con marcas de tiempo idénticas (por ejemplo, resultados de eventos de puntero) se ordenan correctamente [#1322](https://github.com/decentraland/js-sdk-toolchain/pull/1322); se corrigió el endpoint de la vista previa móvil ([release 7.20.3](https://github.com/decentraland/js-sdk-toolchain/releases/tag/7.20.3)).

### 7.20.1 (16 de feb. de 2026)

* **Mejor flujo al desplegar a un World multiscene.** [#1317](https://github.com/decentraland/js-sdk-toolchain/pull/1317)

### 7.20.0 (5 de feb. de 2026)

* **Precarga de assets**: marca assets para cargarlos antes de que se necesiten. [#1287](https://github.com/decentraland/js-sdk-toolchain/pull/1287)

### 7.19.0 (29 de ene. de 2026)

* **Funciones auxiliares del SDK**: atajos para tareas comunes como retrasar una instrucción. [#1275](https://github.com/decentraland/js-sdk-toolchain/pull/1275)
* **Distancia máxima del jugador para eventos de puntero**: limita a qué distancia puede estar un jugador para interactuar. [#1293](https://github.com/decentraland/js-sdk-toolchain/pull/1293)

### 7.18.0 (26 de ene. de 2026)

* **Múltiples renderers de UI**: `addUiRenderer()` y `removeUiRenderer()` en el renderer de React ECS. [#1285](https://github.com/decentraland/js-sdk-toolchain/pull/1285)
* **Factor de escala automático de la UI** en React ECS. [#1290](https://github.com/decentraland/js-sdk-toolchain/pull/1290)
* **Instancias de Script desde código**: llama a los componentes Script de tus smart items desde cualquier parte de tu código, y activa acciones configuradas en el editor con `ActionCallback`. [#1284](https://github.com/decentraland/js-sdk-toolchain/pull/1284)
* **Los archivos Markdown se excluyen de los deployments.** [#1282](https://github.com/decentraland/js-sdk-toolchain/pull/1282)

### 7.17.0 (5 de ene. de 2026)

* **Los errores de build muestran la salida de type-check.** [#1263](https://github.com/decentraland/js-sdk-toolchain/pull/1263)
* **La vista previa móvil imprime coordenadas y un deep link.** [#1268](https://github.com/decentraland/js-sdk-toolchain/pull/1268)

### 7.16.0 (30 de dic. de 2025)

* **La visibilidad se propaga a los hijos**: ocultar una entity con `VisibilityComponent` también oculta a sus hijos. [#1262](https://github.com/decentraland/js-sdk-toolchain/pull/1262)
* **Vista previa en móvil**: ejecuta la vista previa en la app móvil. [#1261](https://github.com/decentraland/js-sdk-toolchain/pull/1261)
* **Vista previa del navegador Bevy Web** compatible. [#1241](https://github.com/decentraland/js-sdk-toolchain/pull/1241)
* **`npm run start` abre el nuevo Explorer de escritorio por defecto.** [#1264](https://github.com/decentraland/js-sdk-toolchain/pull/1264)
* **Corrección**: la descarga de la plantilla del proyecto de wearable inteligente vuelve a funcionar. [#1255](https://github.com/decentraland/js-sdk-toolchain/pull/1255)

### 7.15.0 y 7.15.1 (8 de dic. de 2025)

* **Audio espacial** para `AudioSource` y `VideoPlayer`. [#1243](https://github.com/decentraland/js-sdk-toolchain/pull/1243)
* **Soporte para el componente Script** para smart items con código. [#1245](https://github.com/decentraland/js-sdk-toolchain/pull/1245)
* **Correcciones**: Windows muestra un aviso para descargar el Desktop Client cuando falta [#1244](https://github.com/decentraland/js-sdk-toolchain/pull/1244); los smart items se resuelven correctamente desde estructuras anidadas `node_modules` [#1258](https://github.com/decentraland/js-sdk-toolchain/pull/1258).

### 7.12.1 (3 de nov. de 2025)

* **`code-to-composite` command**: convierte el código de la escena en un composite que el editor puede abrir. [#1232](https://github.com/decentraland/js-sdk-toolchain/pull/1232)

### 7.12.0 (30 de oct. de 2025)

* **Acción restringida CopyToClipboard**. [#1234](https://github.com/decentraland/js-sdk-toolchain/pull/1234)

### 7.11.0 (15 de oct. de 2025)

* **Errores más claros del content server** al desplegar. [#1217](https://github.com/decentraland/js-sdk-toolchain/pull/1217)
* **`dclcontext` y las carpetas thumbnails se excluyen de los deployments.** [#1227](https://github.com/decentraland/js-sdk-toolchain/pull/1227), [#1229](https://github.com/decentraland/js-sdk-toolchain/pull/1229)

### 7.10.6 (9 de oct. de 2025)

* **Modos continuos de Tween**: mantén la rotación o el movimiento indefinidamente. [#1221](https://github.com/decentraland/js-sdk-toolchain/pull/1221)

### 7.10.5 (6 de oct. de 2025)

* **`get-context-files` command**: descarga los documentos de contexto de IA en tu escena para que los asistentes de programación conozcan el SDK. [#1220](https://github.com/decentraland/js-sdk-toolchain/pull/1220)

### 7.10.3 (29 de sep. de 2025)

* **Componentes de trigger area**: detecta a los jugadores al entrar, permanecer y salir de una región. Ver [Trigger Areas](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/trigger-areas/). [#1210](https://github.com/decentraland/js-sdk-toolchain/pull/1210)
* **Componente Tags**: etiqueta entities y consulta por tag. [#1216](https://github.com/decentraland/js-sdk-toolchain/pull/1216)

## Creator Hub

### 0.52.0 (20 de oct. de 2026)

* **Asset pack de Avatar Animations**: 45 animaciones de avatar listas para usar, además de las 32 integradas que cada escena ya puede reproducir. Pasa el cursor sobre una tarjeta para verla, arrástrala para copiar su archivo a tu escena y luego reprodúcela desde código. Ver [Animaciones de avatar](/docs/creator/content-creator-es/scene-editor/primeros-pasos/about-editor/#avatar-animations). [#1689](https://github.com/decentraland/creator-hub/pull/1689)
* **Menciona assets en un prompt de AI Assistant**: haz doble clic en un asset en **Local Assets** para mencionarlo en tu siguiente mensaje, junto con lo que tengas seleccionado. Ver [Apunta a un asset en tu prompt](/docs/creator/content-creator-es/scene-editor/extender-con-codigo/ai-assistant/#point-at-an-asset-in-your-prompt). [#1689](https://github.com/decentraland/creator-hub/pull/1689)
* **Más de 400 assets nuevos, y categorías Buildings y Materials**: modelos de Genesis Plaza y de las escenas plantilla de Decentraland, además de los smart items Water Plane y Water Cylinder que salpican, y un Floor Pressure Button que se presiona mientras un jugador está encima. Ver [Water](/docs/creator/content-creator-es/scene-editor/interactividad/smart-items/#water). [#1696](https://github.com/decentraland/creator-hub/pull/1696)
* **Panel de assets plegable**: un chevron a la derecha de la barra de pestañas del panel lo reduce solo a sus pestañas, dando a la vista 3D toda la altura de la ventana. Ver [Agregar items](/docs/creator/content-creator-es/scene-editor/primeros-pasos/about-editor/#add-items). [#1691](https://github.com/decentraland/creator-hub/pull/1691)
* **Configuración de AI Assistant desde una terminal**: Claude y Cursor ahora se instalan e inician sesión desde una terminal, igual que Gemini. Cada pantalla de configuración muestra el comando de instalación del proveedor para tu sistema operativo y enlaza a su guía de instalación, y **Volver a comprobar** encuentra una instalación nueva sin reiniciar. Ver [Conecta una CLI de programación](/docs/creator/content-creator-es/scene-editor/extender-con-codigo/ai-assistant/#connect-a-coding-cli). [#1684](https://github.com/decentraland/creator-hub/pull/1684)
* **Un enlace compartible después de publicar**: la pantalla de éxito muestra y copia un enlace web que cualquiera puede abrir en un navegador, en lugar de uno que requiere el cliente de escritorio. Ver [Publica tu escena](/docs/creator/content-creator-es/scene-editor/publicar/publish-scene/). [#1688](https://github.com/decentraland/creator-hub/pull/1688)
* **La tarjeta Collections abre la app de creadores**: los wearables y emotes **Collections** tarjeta en la **Más** ahora abre la nueva app de creadores en lugar del Builder heredado. [#1687](https://github.com/decentraland/creator-hub/pull/1687)
* **Correcciones**: un fallo de publicación que informaba que la miniatura de la escena "debe ser un archivo incluido en la implementación", lo que hacía que las escenas afectadas se corrigieran solas al guardar la siguiente miniatura [#1657](https://github.com/decentraland/creator-hub/pull/1657); un diseño redimensionado en la pestaña Layout se mantiene, así que publicar ya no restablece la escena a un solo parcel bajo el renderizador Bevy [#1682](https://github.com/decentraland/creator-hub/pull/1682); fiabilidad de AI Assistant, con un turno silencioso que ahora muestra un error, las preguntas caducadas que se cierran, **Detener** realmente finaliza un turno en Windows, y un mensaje enviado a mitad de turno vuelve al compositor en lugar de perderse [#1681](https://github.com/decentraland/creator-hub/pull/1681); guía de inicio de sesión cuando un agente informa que no has iniciado sesión [#1685](https://github.com/decentraland/creator-hub/pull/1685).

### 0.51.0 (8 oct 2026)

* **Adjunta archivos a los prompts de AI Assistant**: arrastra archivos al cuadro de mensaje, selecciónalos con el botón del clip, o pega una imagen desde el portapapeles. Se admiten imágenes, modelos 3D, audio y cualquier otro archivo, hasta 8 por mensaje. Ver [Adjunta archivos a un prompt](/docs/creator/content-creator-es/scene-editor/extender-con-codigo/ai-assistant/#attach-files-to-a-prompt). [#1670](https://github.com/decentraland/creator-hub/pull/1670)
* **Items rápidos al soltar medios**: arrastrar una imagen, sonido o video desde **Local Assets** sobre el lienzo ahora crea un item listo para usar que ya apunta a ese archivo. Ver [Importar assets personalizados](/docs/creator/content-creator-es/scene-editor/construir/import-items/). [#1616](https://github.com/decentraland/creator-hub/pull/1616)
* **El botón de la barra de herramientas está etiquetado como Reproducir de nuevo**, y su menú se divide en un panel principal que elige dónde previsualizar, además de un **Cliente de escritorio** submenú para las opciones que solo aplican allí. La consola de depuración ahora se abre por defecto, así que ya no es una opción de menú. Ver [Previsualiza tu escena](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/preview-scene/). [#1671](https://github.com/decentraland/creator-hub/pull/1671), [#1656](https://github.com/decentraland/creator-hub/pull/1656)
* **Escribe un nivel de zoom exacto en el UI Editor**: el porcentaje de zoom ahora es un campo. Haz clic en él, escribe un valor de 10 a 200 y presiona Enter. Ver [UI Editor](/docs/creator/content-creator-es/scene-editor/construir/ui-editor/). [#1659](https://github.com/decentraland/creator-hub/pull/1659)
* **Silenciar el audio de la escena mientras editas**: un botón de altavoz en la barra de herramientas del editor silencia todas las fuentes de audio y stream, bajo el renderizador Bevy. Ver [Renderizador de la escena](/docs/creator/content-creator-es/scene-editor/primeros-pasos/about-editor/#scene-renderer). [#1669](https://github.com/decentraland/creator-hub/pull/1669)
* **Opción Scene Inset renombrada**: "Gameplay Safe Area" ahora es "Interactable Safe Area" en el UI Editor. Escribe el mismo `screenInset: 'interactable'` valor que antes. Ver [UI Editor](/docs/creator/content-creator-es/scene-editor/construir/ui-editor/). [#1673](https://github.com/decentraland/creator-hub/pull/1673)
* **Correcciones**: duplicación de Trigger Area y pérdida del campo de edición de la forma [#1658](https://github.com/decentraland/creator-hub/pull/1658); la previsualización de MobileHUD se enmarca contra la pantalla móvil en lugar de la GUI activa [#1648](https://github.com/decentraland/creator-hub/pull/1648); cambiar el nombre de una escena ya no pierde pulsaciones de teclas, y el encabezado del editor se actualiza [#1651](https://github.com/decentraland/creator-hub/pull/1651); la consola de depuración ya no desaparece después de **Recargar escena desde el disco** [#1652](https://github.com/decentraland/creator-hub/pull/1652); limpieza de deshacer del editor Bevy y aviso de dependencia [#1669](https://github.com/decentraland/creator-hub/pull/1669); el logo de Decentraland se renderiza en el tooltip del smart item [#1677](https://github.com/decentraland/creator-hub/pull/1677); las entities eliminadas siguen pudiendo restaurarse con deshacer [#1679](https://github.com/decentraland/creator-hub/pull/1679); las etiquetas largas de las casillas se ajustan [#1676](https://github.com/decentraland/creator-hub/pull/1676).

### 0.50.0 (23 sep 2026)

* **Los assets soltados en la carpeta del proyecto aparecen por sí solos**: los archivos copiados en la `carpeta de assets` del proyecto desde fuera del editor aparecen en **Local Assets** sin presionar **Actualizar**. Consulta [Importar assets personalizados](/docs/creator/content-creator-es/scene-editor/construir/import-items/). [#1606](https://github.com/decentraland/creator-hub/pull/1606)
* **El UI Editor es estable**: ya no está detrás de un ajuste experimental. Ábrelo desde el conmutador 2D/3D. Tu escena necesita `@dcl/sdk` 7.26.0 o superior. Ver [UI Editor](/docs/creator/content-creator-es/scene-editor/construir/ui-editor/). [#1643](https://github.com/decentraland/creator-hub/pull/1643)
* **AI Assistant es estable** y activado por defecto, con su propia **AI** pestaña en la configuración. Ver [AI Assistant](/docs/creator/content-creator-es/scene-editor/extender-con-codigo/ai-assistant/). [#1646](https://github.com/decentraland/creator-hub/pull/1646)
* **Optimiza modelos 3D desde el editor**: una **Optimizar** acción en la barra de título y en el **Local Assets** panel comprime tus modelos, con un **Revertir** para volver a ponerlos. Ver [Optimizar modelos](https://github.com/decentraland/docs/tree/main/creator/scene-editor/build/optimize-models.md). [#1566](https://github.com/decentraland/creator-hub/pull/1566)
* **Smart item Trigger Area**: coloca una caja invisible, elige si reacciona solo al jugador o a todo el mundo, y describe con palabras sencillas qué debe pasar cuando alguien entra o sale. Ver [Trigger Area](/docs/creator/content-creator-es/scene-editor/interactividad/trigger-area/). [#1604](https://github.com/decentraland/creator-hub/pull/1604)
* **MobileHUD personalizable en el UI Editor**: elige el botón principal, muestra u oculta los demás botones, el joystick y la mira, y asígnales iconos personalizados, con una previsualización en vivo en el lienzo. Ver [Personaliza los controles móviles](/docs/creator/content-creator-es/scene-editor/construir/ui-editor/#customize-the-mobile-controls). [#1615](https://github.com/decentraland/creator-hub/pull/1615)
* **Miniaturas de escena 16:9**: la previsualización de la miniatura ocupa todo el ancho, las imágenes con otra relación de aspecto se rechazan y la miniatura autogenerada se captura en 16:9. Ver [Miniatura de la escena](/docs/creator/content-creator-es/scene-editor/configurar/scene-settings/#scene-thumbnail). [#1601](https://github.com/decentraland/creator-hub/pull/1601)
* **La configuración de Components se pone al día con el SDK**: el inspector ahora expone la velocidad de reproducción de video, la posición de inicio y el audio espacial, el ajuste de texto y el tamaño de la caja, el tono de audio y los modos de mover y rotar del material, además de una forma de borrar colores y texturas. Las propiedades establecidas desde código ya no se pierden cuando editas un Component. Ver [Components](/docs/creator/content-creator-es/scene-editor/construir/components/). [#1470](https://github.com/decentraland/creator-hub/pull/1470)
* **Consola de depuración desmontable**: saca la consola a su propia ventana y mantenla en un segundo monitor. Ver [La consola de Creator Hub](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/preview-scene/#the-creator-hub-console). [#1642](https://github.com/decentraland/creator-hub/pull/1642)
* **AI Assistant puede usar las herramientas de escena de Creator Hub en Cursor y Gemini**, no solo Claude y Codex. Ver [AI Assistant](/docs/creator/content-creator-es/scene-editor/extender-con-codigo/ai-assistant/). [#1614](https://github.com/decentraland/creator-hub/pull/1614)
* **Correcciones**: **Mostrar código QR para móvil** funciona sea cual sea el cliente de previsualización seleccionado, y la previsualización de Bevy (Web) reutiliza su pestaña [#1608](https://github.com/decentraland/creator-hub/pull/1608); publicar ya no espera a la generación de LOD, así que el **Optimizando** paso desaparece [#1603](https://github.com/decentraland/creator-hub/pull/1603); el UI Editor ya no deja un huérfano `./ui` importación en escenas antiguas [#1609](https://github.com/decentraland/creator-hub/pull/1609); el renderizador Bevy ya no muestra el aviso de previsualización de "code elements" [#1607](https://github.com/decentraland/creator-hub/pull/1607); el Admin Toolkit ya no cambia el orden z de la UI del resto de la escena [#1612](https://github.com/decentraland/creator-hub/pull/1612); la superposición de atajos de teclado funciona bajo Bevy, el desplazamiento se restablece al navegar y los conmutadores de iconos del entity tree se comportan [#1617](https://github.com/decentraland/creator-hub/pull/1617); pulido del compositor de AI Assistant, del desplegable de chat y de las notificaciones [#1632](https://github.com/decentraland/creator-hub/pull/1632), [#1621](https://github.com/decentraland/creator-hub/pull/1621).

### 0.49.0 (16 sep 2026)

* **Smart item Locomotion Settings**: cambia qué tan rápido se mueven los jugadores y qué tan alto saltan, con deslizadores en lugar de código. Ver [Ajustes de locomoción](/docs/creator/content-creator-es/scene-editor/interactividad/smart-items/#locomotion-settings). [#1584](https://github.com/decentraland/creator-hub/pull/1584)
* **AI Assistant admite Cursor y Gemini**, junto con Claude y Codex. Ver [AI Assistant](/docs/creator/content-creator-es/scene-editor/extender-con-codigo/ai-assistant/). [#1591](https://github.com/decentraland/creator-hub/pull/1591)
* **Escribe cantidades exactas de transformación**: pulsa una tecla de herramienta, un eje y un número para mover, rotar o escalar una cantidad precisa en lugar de arrastrar. **F** ya no es un atajo para el gizmo libre. Ver [Escribe una cantidad exacta](/docs/creator/content-creator-es/scene-editor/primeros-pasos/scene-editor-essentials/#type-an-exact-amount). [#1599](https://github.com/decentraland/creator-hub/pull/1599)
* **Correcciones**: el UI Editor ya no nubla la vista con contenedores de pantalla completa, y los valores anidados de la previsualización de GUI y la colocación libre funcionan [#1590](https://github.com/decentraland/creator-hub/pull/1590); el componente Animator se muestra debajo del editor Bevy [#1587](https://github.com/decentraland/creator-hub/pull/1587); la recarga en caliente del editor Bevy se basa en eventos reales de compilación [#1592](https://github.com/decentraland/creator-hub/pull/1592); los turnos de seguimiento de AI Assistant vuelven a funcionar con Codex [#1600](https://github.com/decentraland/creator-hub/pull/1600); los menús desplegables y kebab ya no se recortan [#1595](https://github.com/decentraland/creator-hub/pull/1595); crear escena con un clic, los nombres de las entities conservan los espacios consecutivos, el menú de la tarjeta de la escena ya no refluye la cuadrícula, los tooltips de assets se ubican correctamente en los resultados de búsqueda y puedes arrastrar desde el árbol de Local Assets [#1593](https://github.com/decentraland/creator-hub/pull/1593).

### 0.48.0 (10 sep 2026)

* **Previsualización móvil del UI Editor**: un marco de teléfono real con un área segura precisa, usando los mismos preajustes de dispositivo que el cliente, además de guías opcionales del HUD (joystick, botones, chat) que puedes activar o desactivar. [#1579](https://github.com/decentraland/creator-hub/pull/1579)
* **Rediseño de AI Assistant**: panel más limpio, botón de cierre fiable, mensaje de introducción persistente y mejores ajustes. Ver [AI Assistant](/docs/creator/content-creator-es/scene-editor/extender-con-codigo/ai-assistant/). [#1586](https://github.com/decentraland/creator-hub/pull/1586)
* **Analytics muestra el tiempo de juego mediano** en lugar del promedio. [#1564](https://github.com/decentraland/creator-hub/pull/1564)
* **Cambios**: el renderizador Bevy está etiquetado como "experimental" [#1583](https://github.com/decentraland/creator-hub/pull/1583); el campo de configuración de la escena ahora es "Creator name" [#1561](https://github.com/decentraland/creator-hub/pull/1561); el aviso de retraso del asset-bundle en la pantalla de publicación desaparece [#1582](https://github.com/decentraland/creator-hub/pull/1582).
* **Corrección**: en Windows, los turnos de AI Assistant ya no fallan cuando la línea de comandos se hace demasiado larga. [#1588](https://github.com/decentraland/creator-hub/pull/1588)

### 0.47.0 (4 sep 2026)

* **Scene Agent**: un asistente de IA integrado en el editor. Lee tu escena, escribe código de SDK7, crea y edita entities, components, smart items y scripts en vivo en el viewport, y puede ejecutar la previsualización para comprobar su trabajo. Controla tu CLI instalada de Claude o Codex, así que no hay ninguna clave API que configurar. [#1499](https://github.com/decentraland/creator-hub/pull/1499)
* **Los smart items se ejecutan sobre Script components**: el comportamiento de cada smart item ahora vive como código en un `.ts` archivo dentro de la carpeta del item, con sus funciones principales expuestas como acciones. Puedes leerlo y ajustarlo. [#1354](https://github.com/decentraland/creator-hub/pull/1354)
* **Entradas deslizantes para smart items de Script**: un parámetro numérico puede declarar un rango y se muestra como un deslizador. Ver [Usar el Script Component](/docs/creator/content-creator-es/scene-editor/extender-con-codigo/script-component/). [#1513](https://github.com/decentraland/creator-hub/pull/1513)
* **UI Editor**: la posición propia de un node (fijo o en flujo) es independiente de cómo organiza a sus children, y los nodes tienen iconos al estilo Figma. [#1552](https://github.com/decentraland/creator-hub/pull/1552)
* **Una sola lista de tutoriales compartida** en la barra lateral Scenes y en la página Learn. [#1551](https://github.com/decentraland/creator-hub/pull/1551)
* **Correcciones**: los Mac Intel vuelven a iniciar. [#1545](https://github.com/decentraland/creator-hub/pull/1545); el UI Editor recuerda el modo 2D o 3D [#1532](https://github.com/decentraland/creator-hub/pull/1532); las animaciones importadas tienen por defecto un peso de 1 [#1557](https://github.com/decentraland/creator-hub/pull/1557); el componente Script aparece justo después de GLTF en el panel de entities [#1556](https://github.com/decentraland/creator-hub/pull/1556); los items personalizados cargan bajo el renderizador Bevy [#1555](https://github.com/decentraland/creator-hub/pull/1555).

### 0.46.0 (27 ago 2026)

* **Contenedor de pantalla completa** en el UI Editor: un contenedor que llena toda la pantalla con un solo clic. [#1540](https://github.com/decentraland/creator-hub/pull/1540)

### 0.45.0 (27 ago 2026)

* **UI Editor (experimental)**: diseña visualmente la interfaz en pantalla de tu escena en un canvas 2D con un árbol de nodos y un panel de propiedades. Tu `@dcl/react-ecs` código sigue siendo la única fuente de verdad. Actívalo en Settings > Experimental. Requiere SDK 7.26.0 o posterior. Ver [UI Editor](/docs/creator/content-creator-es/scene-editor/construir/ui-editor/). [#1523](https://github.com/decentraland/creator-hub/pull/1523), [#1522](https://github.com/decentraland/creator-hub/pull/1522), [#1521](https://github.com/decentraland/creator-hub/pull/1521), [#1520](https://github.com/decentraland/creator-hub/pull/1520), [#1524](https://github.com/decentraland/creator-hub/pull/1524)
* **Áreas modificadoras de Avatar y Camera**: cambia cómo aparecen los avatars y cómo se comporta la cámara dentro de una región. Añádelas como components o como smart items. Ver [Modifier Areas](/docs/creator/content-creator-es/scene-editor/interactividad/modifier-areas/). [#1471](https://github.com/decentraland/creator-hub/pull/1471)
* **Talleres de Friendzone Buildathon 2026** en Learn > Videos. [#1510](https://github.com/decentraland/creator-hub/pull/1510)
* **Correcciones**: publicar en LAND ya no se queda colgado en "Publicando…" [#1496](https://github.com/decentraland/creator-hub/pull/1496); el botón de confirmación funciona al publicar en un World desde el editor Bevy [#1493](https://github.com/decentraland/creator-hub/pull/1493); Analytics muestra todas las escenas desplegadas [#1492](https://github.com/decentraland/creator-hub/pull/1492); los nuevos NAMEs y permisos de World aparecen sin reiniciar [#1509](https://github.com/decentraland/creator-hub/pull/1509); desaparece el cuadro de diálogo de actualización de Windows "Creator Hub no se puede cerrar" [#1497](https://github.com/decentraland/creator-hub/pull/1497); la pestaña More elimina la tarjeta Legacy Web Editor y enlaza Names con el marketplace [#1495](https://github.com/decentraland/creator-hub/pull/1495).

### 0.44.2 (14 ago 2026)

* **Analytics**: visitantes, retención, visitas e interacción para cada escena de World y Genesis City que poseas o en la que colabores, divididos por escritorio y móvil. Ver [Scene Analytics](/docs/creator/content-creator-es/scenes-sdk7/otros/scene-analytics/). [#1447](https://github.com/decentraland/creator-hub/pull/1447)
* **Pantalla de inicio rediseñada.** [#1481](https://github.com/decentraland/creator-hub/pull/1481)
* **Correcciones**: las importaciones de GLB ya no se quedan colgadas en un indicador giratorio [#1487](https://github.com/decentraland/creator-hub/pull/1487); el servidor local multijugador vuelve a iniciarse en las previsualizaciones de escena [#1455](https://github.com/decentraland/creator-hub/pull/1455).

### 0.44.0 (4 ago 2026)

* **Renderizador Bevy (experimental)**: un renderizador alternativo para el viewport del editor. [#1330](https://github.com/decentraland/creator-hub/pull/1330), [#1363](https://github.com/decentraland/creator-hub/pull/1363)
* **Opción de previsualización Optimize Assets**: previsualiza con asset bundles para que los modelos se vean como se verán una vez publicados. Ver [Vista previa con assets optimizados](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/preview-scene/#preview-with-optimized-assets). [#1396](https://github.com/decentraland/creator-hub/pull/1396)
* **Conmutador Explorer MCP** en las opciones de previsualización. [#1440](https://github.com/decentraland/creator-hub/pull/1440)
* **Búsqueda en el entity tree.** [#1385](https://github.com/decentraland/creator-hub/pull/1385)
* **Renombra la carpeta de un proyecto** desde la vista Scenes. [#1357](https://github.com/decentraland/creator-hub/pull/1357)
* **Ajuste de escena "Hide Landscape Terrain".** [#1383](https://github.com/decentraland/creator-hub/pull/1383)
* **Herramientas de administración rediseñadas** según el nuevo diseño de Decentraland. [#1360](https://github.com/decentraland/creator-hub/pull/1360)
* **Se eliminó la opción de publicación en Test Server.** [#1441](https://github.com/decentraland/creator-hub/pull/1441)
* **Correcciones**: la previsualización móvil de QR ya no abre el cliente de escritorio [#1452](https://github.com/decentraland/creator-hub/pull/1452); los conmutadores de bloquear y ocultar en el entity tree se pueden volver a desactivar [#1429](https://github.com/decentraland/creator-hub/pull/1429); los Custom Items incluyen assets de componente Placeholder [#1287](https://github.com/decentraland/creator-hub/pull/1287); el panel de administración de DCL Cast se abre automáticamente, conserva el estado de las diapositivas y se replica para quienes se unen tarde [#1358](https://github.com/decentraland/creator-hub/pull/1358); las herramientas de administración omiten la inicialización en el servidor autoritativo [#1381](https://github.com/decentraland/creator-hub/pull/1381).

### 0.43.0 (6 jul 2026)

* **Inicio de sesión más simple**: una sola confirmación en el navegador en lugar de escribir un código. [#1338](https://github.com/decentraland/creator-hub/pull/1338)
* **Correcciones**: error claro cuando una implementación agota el tiempo de espera [#1355](https://github.com/decentraland/creator-hub/pull/1355); conteo correcto de parcels para escenas que abarcan coordenadas negativas y positivas [#1352](https://github.com/decentraland/creator-hub/pull/1352); mensaje de "La escena ya se importó" al volver a importar [#1350](https://github.com/decentraland/creator-hub/pull/1350).

### 0.42.0 (26 jun 2026)

* **componente Particle System** en el editor. Ver [Sistema de Partículas](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/particle-system/). [#1311](https://github.com/decentraland/creator-hub/pull/1311)
* **Acción Spawn Entity**: genera un smart item desde una cadena de acciones, en lugar de colocar copias ocultas de antemano. [#1322](https://github.com/decentraland/creator-hub/pull/1322)
* **Copiar y pegar valores de componentes** desde el menú de tres puntos de un componente. [#1321](https://github.com/decentraland/creator-hub/pull/1321)
* **Hasta 100 colaboradores** por World. [#1310](https://github.com/decentraland/creator-hub/pull/1310)
* **Los colaboradores pueden anular la publicación de escenas de World.** [#1209](https://github.com/decentraland/creator-hub/pull/1209)
* **Cambios**: se elimina la clasificación de edad Teen [#1329](https://github.com/decentraland/creator-hub/pull/1329); un aviso explica que la transmisión en vivo no es compatible en el modo para un jugador [#1336](https://github.com/decentraland/creator-hub/pull/1336).
* **Correcciones**: Wind Area e Impulse Area reaccionan solo al jugador actual [#1323](https://github.com/decentraland/creator-hub/pull/1323); la UI de las herramientas de administración se ajusta a pantallas móviles [#1256](https://github.com/decentraland/creator-hub/pull/1256); controles para varios components [#1282](https://github.com/decentraland/creator-hub/pull/1282); errores de inicio del inspector [#1334](https://github.com/decentraland/creator-hub/pull/1334).

### 0.41.1 (29 abr 2026)

* **Botón Actualizar** en la barra superior reinicia el editor, útil después de editar la escena con herramientas de IA. [#1312](https://github.com/decentraland/creator-hub/pull/1312)
* **La consola se desplaza automáticamente** y permite copiar texto. [#1305](https://github.com/decentraland/creator-hub/pull/1305)

### 0.41.0 (27 abr 2026)

* **Sesión de depuración móvil**: vincúlate con el cliente móvil mediante QR y transmite el estado de las entities, los logs de la consola y los datos de rendimiento al editor, con scrubbing de ticks y comandos de pausa, reanudación y recarga. [#1306](https://github.com/decentraland/creator-hub/pull/1306)
* **Speaker Showcase y panel de presentación para DCL Cast**: administra oradores en vivo y diapositivas desde Admin Tools. [#1213](https://github.com/decentraland/creator-hub/pull/1213)
* **Errores a nivel de entidad** en el entity tree: un icono rojo marca las entities con valores de componentes inválidos. [#1258](https://github.com/decentraland/creator-hub/pull/1258)
* **"Desactivar el chat de voz cercano"** restricción de escena. [#1289](https://github.com/decentraland/creator-hub/pull/1289)
* **Botón "Ver almacenamiento"** para Worlds, para ver y editar datos del servidor autoritativo. [#1307](https://github.com/decentraland/creator-hub/pull/1307)
* **Smart items Wind Area e Impulse Area** que empujan al jugador. [#1296](https://github.com/decentraland/creator-hub/pull/1296), [#1308](https://github.com/decentraland/creator-hub/pull/1308)
* **Smart items de recompensa**: Balloons, dos dispensadores de Wearables y un dispensador de reclamo de Clap que entregan wearables de una campaña de recompensas. [#1124](https://github.com/decentraland/creator-hub/pull/1124)
* **Correcciones**: los valores de rotación ya no se desvían al salir del campo [#1266](https://github.com/decentraland/creator-hub/pull/1266); la casilla "Fixed Skybox" ya no se invierte con otras casillas [#1254](https://github.com/decentraland/creator-hub/pull/1254); desplegar en un World solo anula el despliegue de escenas cuyos parcels se superponen [#1271](https://github.com/decentraland/creator-hub/pull/1271); los duplicados aparecen junto al original en el árbol [#1150](https://github.com/decentraland/creator-hub/pull/1150); modelo de Trigger Area actualizado [#1285](https://github.com/decentraland/creator-hub/pull/1285).

### 0.39.1 (30 mar 2026)

* **"Mostrar en Places"** casilla en la configuración de World. [#1264](https://github.com/decentraland/creator-hub/pull/1264)
* **Correcciones**: la previsualización muestra el error cuando el código de la escena está roto en lugar de fallar en silencio [#1238](https://github.com/decentraland/creator-hub/pull/1238); la marca de multi-scene validada en el archivo de despliegue [#1270](https://github.com/decentraland/creator-hub/pull/1270).

### 0.39.0 (26 mar 2026)

* **Correcciones**: actualización del instalador de Windows [#1252](https://github.com/decentraland/creator-hub/pull/1252); despliegue multi-scene [#1262](https://github.com/decentraland/creator-hub/pull/1262); smart items que no funcionaban [#1260](https://github.com/decentraland/creator-hub/pull/1260); las áreas de trigger y click muestran una malla de pointer [#1255](https://github.com/decentraland/creator-hub/pull/1255).

### 0.38.0 (20 mar 2026)

* **Correcciones**: los asset packs vuelven a desplazarse [#1239](https://github.com/decentraland/creator-hub/pull/1239); se conservan los archivos de audio al generar un smart item personalizado en una escena nueva [#1234](https://github.com/decentraland/creator-hub/pull/1234).

### 0.37.0 (16 mar 2026)

* **Consola de depuración incrustada** como una pestaña en el panel de Assets. [#1200](https://github.com/decentraland/creator-hub/pull/1200)
* **Correcciones**: los items personalizados conservan sus archivos de textura [#1217](https://github.com/decentraland/creator-hub/pull/1217); los gizmos funcionan en la primera selección [#1218](https://github.com/decentraland/creator-hub/pull/1218); la acción Show Image usa por defecto alineación centrada [#1221](https://github.com/decentraland/creator-hub/pull/1221); la copia del origen de video ya no menciona solo Vimeo [#1216](https://github.com/decentraland/creator-hub/pull/1216).

### 0.36.0 (10 mar 2026)

* **Correcciones**: selección del spawn point en el entity tree [#1199](https://github.com/decentraland/creator-hub/pull/1199); "Restricted" se elimina de las clasificaciones de contenido de World [#1205](https://github.com/decentraland/creator-hub/pull/1205); Manage Published solo enumera Worlds con escenas publicadas [#1208](https://github.com/decentraland/creator-hub/pull/1208); error de despliegue cuando se supera el límite de World [#1210](https://github.com/decentraland/creator-hub/pull/1210).

### 0.35.0 (5 mar 2026)

* **Visualización de spawn points** en el editor. Ver [Spawn Areas](/docs/creator/content-creator-es/scene-editor/construir/spawn-areas/). [#1129](https://github.com/decentraland/creator-hub/pull/1129)
* **Catálogo dinámico de assets**: los items más recientes se cargan en tiempo de ejecución, con una comprobación de compatibilidad con SDK. [#1172](https://github.com/decentraland/creator-hub/pull/1172)
* **Vista previa con varias instancias** conmutador para probar el multiplayer. Requiere SDK 7.20.4 o posterior. [#1187](https://github.com/decentraland/creator-hub/pull/1187)
* **Pestaña Access de permisos de World**, y los ajustes de Multiplayer y Skybox de vuelta en World Settings. [#1186](https://github.com/decentraland/creator-hub/pull/1186), [#1189](https://github.com/decentraland/creator-hub/pull/1189)
* **Actualización de la UI** de colores, estilos de componentes y el inspector de entities. [#1178](https://github.com/decentraland/creator-hub/pull/1178)
* **Correcciones**: TextShape se inicializa correctamente [#1194](https://github.com/decentraland/creator-hub/pull/1194) y renderiza sin recorte [#1185](https://github.com/decentraland/creator-hub/pull/1185); baldosas de suelo duplicadas [#1179](https://github.com/decentraland/creator-hub/pull/1179); las actualizaciones del layout de la escena en modo avanzado [#1181](https://github.com/decentraland/creator-hub/pull/1181); el resaltado de selección ya no se retrasa ni pone los objetos en negro [#1196](https://github.com/decentraland/creator-hub/pull/1196); los proyectos administrados se cargan [#1182](https://github.com/decentraland/creator-hub/pull/1182); la UI de Scene Admin se adapta a todas las pantallas [#1190](https://github.com/decentraland/creator-hub/pull/1190).

### 0.34.0 (25 feb 2026)

* **Worlds multiscene**: publica varias escenas en un solo World. [#1089](https://github.com/decentraland/creator-hub/pull/1089), [#1163](https://github.com/decentraland/creator-hub/pull/1163)
* **Manage Worlds**: ajustes de World y una página de administración. [#1132](https://github.com/decentraland/creator-hub/pull/1132), [#1049](https://github.com/decentraland/creator-hub/pull/1049), [#1112](https://github.com/decentraland/creator-hub/pull/1112)
* **Permisos de colaborador** para Worlds. [#1128](https://github.com/decentraland/creator-hub/pull/1128), [#1083](https://github.com/decentraland/creator-hub/pull/1083), [#1125](https://github.com/decentraland/creator-hub/pull/1125)
* **Anula la publicación de una escena desde un World.** [#1123](https://github.com/decentraland/creator-hub/pull/1123)
* **Placeholder component**: entities invisibles para sit spots, trigger areas y similares, sin afectar las métricas de la escena. [#1154](https://github.com/decentraland/creator-hub/pull/1154)
* **Acciones Log y Delete** para smart items. [#1131](https://github.com/decentraland/creator-hub/pull/1131)
* **Correcciones**: las áreas de trigger solo se activan para tu propio jugador [#1142](https://github.com/decentraland/creator-hub/pull/1142); DevTools para el tráfico web de la escena vuelven a funcionar [#1109](https://github.com/decentraland/creator-hub/pull/1109); duración de Virtual Camera y autoselección en "Change camera" [#1160](https://github.com/decentraland/creator-hub/pull/1160); gizmo de escala [#1082](https://github.com/decentraland/creator-hub/pull/1082); smart item Photo Wall [#1110](https://github.com/decentraland/creator-hub/pull/1110); la advertencia "Custom Code" es más precisa [#1171](https://github.com/decentraland/creator-hub/pull/1171); problemas de acceso a World [#1133](https://github.com/decentraland/creator-hub/pull/1133); miniaturas de la escena [#1168](https://github.com/decentraland/creator-hub/pull/1168); se eliminaron el icono Save sin usar y la advertencia "Slow connection" [#1152](https://github.com/decentraland/creator-hub/pull/1152), [#1169](https://github.com/decentraland/creator-hub/pull/1169).

### 0.33.1 (18 feb. 2026)

* **Vista previa en la app móvil**, configurable en los ajustes de vista previa. [#1121](https://github.com/decentraland/creator-hub/pull/1121)
* **Tooltips del componente Script** de los comentarios en el archivo del script, y una plantilla mejorada. [#1130](https://github.com/decentraland/creator-hub/pull/1130), [#1019](https://github.com/decentraland/creator-hub/pull/1019)
* **Selección de fuente** en el componente TextShape. [#1096](https://github.com/decentraland/creator-hub/pull/1096)
* **Correcciones**: gizmo de rotación [#1081](https://github.com/decentraland/creator-hub/pull/1081); edición de smart items de luz [#1139](https://github.com/decentraland/creator-hub/pull/1139); error falso "signal is aborted" al publicar [#1144](https://github.com/decentraland/creator-hub/pull/1144); el gizmo ya no cambia a movimiento libre después de duplicar [#1088](https://github.com/decentraland/creator-hub/pull/1088); smart items Clap Meter y Photo Wall [#1101](https://github.com/decentraland/creator-hub/pull/1101).

### 0.33.0 (4 feb. 2026)

* **Métodos de Script como acciones**: el desplegable de tipo de acción incluye métodos de los componentes Script. [#1026](https://github.com/decentraland/creator-hub/pull/1026), [#1071](https://github.com/decentraland/creator-hub/pull/1071)
* **`isAdmin` función** para saber si el jugador es un administrador de la escena. [#1037](https://github.com/decentraland/creator-hub/pull/1037)
* **Página de almacenamiento de Worlds.** [#1036](https://github.com/decentraland/creator-hub/pull/1036)
* **Modo sin conexión** con una página de "Sin conexión". [#991](https://github.com/decentraland/creator-hub/pull/991)
* **Correcciones**: la búsqueda de assets considera tags y descripciones [#1022](https://github.com/decentraland/creator-hub/pull/1022); el gizmo de escala responde al arrastre de izquierda a derecha y no puede establecer el tamaño en 0 [#1065](https://github.com/decentraland/creator-hub/pull/1065); escalado proporcional con valores inferiores a 1 [#1076](https://github.com/decentraland/creator-hub/pull/1076); el estado de bloqueo no se hereda por los hijos [#1085](https://github.com/decentraland/creator-hub/pull/1085); Photo Mural admite imágenes de Worlds [#1074](https://github.com/decentraland/creator-hub/pull/1074); los custom items solo de imagen crean su carpeta [#1033](https://github.com/decentraland/creator-hub/pull/1033); valores decimales en los sliders [#1075](https://github.com/decentraland/creator-hub/pull/1075).

### 0.32.1 (21 ene. 2026)

* **Nuevas acciones**: Freeze Player, Unfreeze Player, Face Item, Change Collisions, Change Skybox, Reset Skybox, además de un smart item Camera para "Change Camera". [#997](https://github.com/decentraland/creator-hub/pull/997), [#989](https://github.com/decentraland/creator-hub/pull/989)
* **Cambiar varios States a la vez** seleccionando varios items. [#975](https://github.com/decentraland/creator-hub/pull/975)
* **Atajo para republicar**: volver a desplegar al mismo World o coordenadas sin pasar por los menús de configuración. [#1003](https://github.com/decentraland/creator-hub/pull/1003)
* **Nuevos smart items con script**: Clap Meter, Wearables Scanner, Zombie y Photo Mural. Padlock puede activar "Open" en otra entidad. [#1008](https://github.com/decentraland/creator-hub/pull/1008)
* **`ActionRef` y `ActionCallback` tipos de entrada** para Scripts, para gestionar las acciones de otro item desde el código. [#1054](https://github.com/decentraland/creator-hub/pull/1054)
* **Tooltips** en componentes, acciones y smart items. [#1009](https://github.com/decentraland/creator-hub/pull/1009)
* **Página Administrar proyectos.** [#1032](https://github.com/decentraland/creator-hub/pull/1032)
* **Correcciones**: los modelos de pantalla de video son de doble cara [#1007](https://github.com/decentraland/creator-hub/pull/1007); las métricas de la escena cuentan correctamente triangles, materials y textures [#1020](https://github.com/decentraland/creator-hub/pull/1020).

### 0.32.0 (5 ene. 2026)

* **Editar un componente en varias entidades a la vez**: Tag, Material, AudioSource, AudioStream, MeshCollider, TextShape, Visibility y LightSource. [#949](https://github.com/decentraland/creator-hub/pull/949), [#950](https://github.com/decentraland/creator-hub/pull/950), [#958](https://github.com/decentraland/creator-hub/pull/958), [#959](https://github.com/decentraland/creator-hub/pull/959), [#960](https://github.com/decentraland/creator-hub/pull/960), [#963](https://github.com/decentraland/creator-hub/pull/963), [#964](https://github.com/decentraland/creator-hub/pull/964)
* **Campo Creator** en el inspector de entidades. [#944](https://github.com/decentraland/creator-hub/pull/944)
* **Nueva UI de ajustes de la app.** [#988](https://github.com/decentraland/creator-hub/pull/988)
* **Clean Unused Assets**: seleccionar todo, estado intermedio y deshacer. [#993](https://github.com/decentraland/creator-hub/pull/993)
* **Más componentes** disponibles en el editor. [#999](https://github.com/decentraland/creator-hub/pull/999)
* **Correcciones**: los items ocultos siguen invisibles [#974](https://github.com/decentraland/creator-hub/pull/974); materiales transparentes [#976](https://github.com/decentraland/creator-hub/pull/976); el indicador de ejes muestra el norte [#978](https://github.com/decentraland/creator-hub/pull/978); desplegar con una identidad caducada [#981](https://github.com/decentraland/creator-hub/pull/981); el modal de publicación ya no se cierra al hacer clic en el fondo [#982](https://github.com/decentraland/creator-hub/pull/982); visualización de errores de despliegue [#985](https://github.com/decentraland/creator-hub/pull/985); superposición del desplegable de smart items [#995](https://github.com/decentraland/creator-hub/pull/995); deshacer ya no crea una plantilla que no se puede eliminar [#998](https://github.com/decentraland/creator-hub/pull/998); rendimiento del gizmo de posición [#972](https://github.com/decentraland/creator-hub/pull/972).

### 0.31.0 y 0.31.1 (16 dic. 2025)

* **Gizmos de posición plana.** [#943](https://github.com/decentraland/creator-hub/pull/943)
* **Paneo de cámara** y un **ayudante de ejes** en el viewport. [#952](https://github.com/decentraland/creator-hub/pull/952), [#951](https://github.com/decentraland/creator-hub/pull/951)
* **DCL Cast** smart item para hacer streaming de video en una escena. [#957](https://github.com/decentraland/creator-hub/pull/957)
* **Nuevas acciones** en smart items. [#955](https://github.com/decentraland/creator-hub/pull/955)
* **Deseleccionar haciendo clic en un área vacía.** [#961](https://github.com/decentraland/creator-hub/pull/961)
* **Retroalimentación "Link copied"** en el botón de compartir. [#965](https://github.com/decentraland/creator-hub/pull/965)
* **Correcciones**: importar varios archivos [#931](https://github.com/decentraland/creator-hub/pull/931); posición final al escalar varios objetos [#962](https://github.com/decentraland/creator-hub/pull/962).

### 0.30.0 (9 dic. 2025)

* **Componente Script**: adjuntar código personalizado a cualquier item. Ver [Usar el Script Component](/docs/creator/content-creator-es/scene-editor/extender-con-codigo/script-component/). [#886](https://github.com/decentraland/creator-hub/pull/886)
* **Pulsa F** para centrar la cámara en la entidad seleccionada. [#948](https://github.com/decentraland/creator-hub/pull/948)
* **Advertencia cuando una escena tiene código personalizado.** [#903](https://github.com/decentraland/creator-hub/pull/903)
* **La pestaña Custom assets muestra un tutorial** cuando está vacía. [#925](https://github.com/decentraland/creator-hub/pull/925)
* **Correcciones**: rotar varios items juntos [#946](https://github.com/decentraland/creator-hub/pull/946); Add Component oculto en las entidades de escena, cámara y player [#923](https://github.com/decentraland/creator-hub/pull/923); errores más claros cuando una escena no se carga [#921](https://github.com/decentraland/creator-hub/pull/921); Windows solicita descargar el Launcher [#899](https://github.com/decentraland/creator-hub/pull/899); todos los duplicados permanecen seleccionados [#929](https://github.com/decentraland/creator-hub/pull/929); orientación del marcador de posición del video player [#909](https://github.com/decentraland/creator-hub/pull/909); numeración de duplicados [#933](https://github.com/decentraland/creator-hub/pull/933); la carpeta Custom Items se crea cuando falta [#940](https://github.com/decentraland/creator-hub/pull/940); Clean Unused Assets elimina las carpetas vacías [#924](https://github.com/decentraland/creator-hub/pull/924); publicar escenas con código personalizado [#941](https://github.com/decentraland/creator-hub/pull/941); campos de casilla de verificación en la vista básica del smart item [#922](https://github.com/decentraland/creator-hub/pull/922).

### 0.29.3 (20 nov. 2025)

* **Correcciones**: las escenas recién importadas ya no se quedan atascadas cargando [#908](https://github.com/decentraland/creator-hub/pull/908); ya no hay varias ventanas del Finder o Explorer [#906](https://github.com/decentraland/creator-hub/pull/906); flujo de despliegue [#896](https://github.com/decentraland/creator-hub/pull/896); smart items de pantalla de video [#898](https://github.com/decentraland/creator-hub/pull/898).

### 0.29.1 y 0.29.2 (13 nov. 2025)

* **Correcciones**: renderizado de Video Player y su entrada en la lista de componentes, con arrastrar y soltar [#872](https://github.com/decentraland/creator-hub/pull/872), [#873](https://github.com/decentraland/creator-hub/pull/873); smart item NFT [#885](https://github.com/decentraland/creator-hub/pull/885); Clean Unused Assets ya no rompe modelos ni miniaturas [#887](https://github.com/decentraland/creator-hub/pull/887), [#882](https://github.com/decentraland/creator-hub/pull/882); el editor se pone gris mientras se actualizan las dependencias [#891](https://github.com/decentraland/creator-hub/pull/891).

### 0.29.0 (5 nov. 2025)

* **Clean Unused Assets** herramienta. [#857](https://github.com/decentraland/creator-hub/pull/857)
* **Panel de ayuda de la escena**: el archivo de ayuda markdown de una escena se muestra en el editor, útil para plantillas. [#833](https://github.com/decentraland/creator-hub/pull/833)
* **Atajo para abrir la carpeta de la escena** en Finder o Explorer. [#853](https://github.com/decentraland/creator-hub/pull/853)
* **Correcciones**: descargas de plantillas [#845](https://github.com/decentraland/creator-hub/pull/845); campo de contrato del smart item NFT [#839](https://github.com/decentraland/creator-hub/pull/839); las entidades de cámara y player no pueden tener un padre [#855](https://github.com/decentraland/creator-hub/pull/855).

### 0.28.0 (23 oct. 2025)

* **Luces y modificadores de nodos** en el editor. Ver [Luces](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/lights/). [#813](https://github.com/decentraland/creator-hub/pull/813)
* **Corrección**: los items duplicados se pueden mover. [#815](https://github.com/decentraland/creator-hub/pull/815)

### 0.27.5 (15 oct. 2025)

* **Etiquetado de entidades.** [#790](https://github.com/decentraland/creator-hub/pull/790)
* **Archivos de contexto de IA** se descargan en cada escena cuando se crea o actualiza, para que los asistentes de programación conozcan el SDK. [#817](https://github.com/decentraland/creator-hub/pull/817)
* **Enviar un evento** tarjeta en la sección Más. [#811](https://github.com/decentraland/creator-hub/pull/811)
* **Correcciones**: el peso de la animación acepta decimales [#826](https://github.com/decentraland/creator-hub/pull/826); miniaturas de sonido y modelo [#822](https://github.com/decentraland/creator-hub/pull/822), [#804](https://github.com/decentraland/creator-hub/pull/804); depurador de vista previa en blanco [#821](https://github.com/decentraland/creator-hub/pull/821); los assets se actualizan después de importar [#809](https://github.com/decentraland/creator-hub/pull/809); despliegues a través del servidor linker [#812](https://github.com/decentraland/creator-hub/pull/812).

### 0.25.3 (6 oct. 2025)

* **Chrome DevTools** disponibles desde Creator Hub para depurar escenas. [#766](https://github.com/decentraland/creator-hub/pull/766)
* **Corrección**: crear nuevas escenas. [#794](https://github.com/decentraland/creator-hub/pull/794)

### 0.24.1 (17 sep. 2025)

* **Ajuste predeterminado del editor de código**: elige VS Code, Cursor u otro editor instalado. [#751](https://github.com/decentraland/creator-hub/pull/751)
* **Eliminar archivos del disco** al eliminar un proyecto, como opción. [#787](https://github.com/decentraland/creator-hub/pull/787)
* **Correcciones**: las textures se pueden eliminar y cargar desde URLs [#772](https://github.com/decentraland/creator-hub/pull/772), [#779](https://github.com/decentraland/creator-hub/pull/779); redimensionamiento del canvas al eliminar una entidad [#770](https://github.com/decentraland/creator-hub/pull/770).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/referencia-de-api/releases.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
