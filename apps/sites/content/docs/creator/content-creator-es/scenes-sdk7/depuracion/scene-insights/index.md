> Buscar logs persistentes del Multiplayer Server, seguir logs en vivo e inspeccionar las métricas de rendimiento de la escena.

# Scene Insights

Scene Insights te ayuda a investigar una [Multiplayer Server](/docs/creator/content-creator-es/scenes-sdk7/networking/authoritative-servers/) escena. Busca registros históricos del servidor, sigue nuevos mensajes en vivo y recupera CPU, memoria y otras métricas desde tu terminal o asistente de IA.

**Los logs y métricas se conservan durante 7 días**, a través de reinicios del servidor y redeploys de la escena. Esto cubre la ejecución del lado del servidor; usa [Depurar en producción](/docs/creator/content-creator-es/scenes-sdk7/depuracion/debug-in-prod/) para problemas del lado del cliente y de renderizado.

## Actualiza tu proyecto

Ejecuta estos comandos desde tu proyecto de escena para instalar la última `auth-server` SDK y runtime:

```bash
npm install @dcl/sdk@auth-server @dcl/js-runtime@auth-server
```

Si tu proyecto también incluye `@dcl/sdk-commands` directamente en `package.json`, actualízalo a `@auth-server` también. Confirma que el CLI instalado incluye `login`, `logs`, `tail`, `metrics`, y `executions`:

```bash
npx sdk-commands scene-insights --help
```

Para depuración asistida por IA, también instala o actualiza las [SDK Skills](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/vibe-coding/):

```bash
npx skills add decentraland/sdk-skills --all
```

Puedes pedirle a tu compañero de IA que haga la configuración:

> Actualiza esta escena a la última versión del SDK y runtime de auth-server, y actualiza las Decentraland SDK Skills. Ayúdame a iniciar sesión en Scene Insights y luego investiga las últimas dos horas de errores del servidor y el uso de CPU y memoria.

Leer telemetría de una escena ya desplegada no requiere volver a desplegarla. Los cambios de permisos sí.

## ¿Quién puede leer los datos?

Inicia sesión con una wallet que cumpla una de estas condiciones:

* Aparece en el nivel raíz de la escena desplegada `logsPermissions` array.
* Es propietaria del World.
* Es un colaborador de despliegue de World cuyo permiso cubre todo el World o cada parcela desplegada de la escena.
* Para Genesis City, es propietaria de la **parcela base**, o tiene permiso delegado de gestión de LAND sobre esa parcela: `operator`, `updateOperator`, `updateManager`, o `approvedForAll`.

Los propietarios y gestores elegibles no necesitan una `logsPermissions` entrada ni volver a desplegar la escena. Para otorgar acceso explícitamente a otra wallet, añádela a `scene.json` y vuelve a desplegar:

```json
{
  "logsPermissions": ["0xYourWalletAddress"]
}
```

Para Genesis City, el permiso se comprueba contra la parcela base de la escena desplegada, incluso si consultas otra parcela dentro de la escena. Solo los derechos de propiedad o gestión sobre otra parcela no otorgan acceso a la escena.

Los mismos permisos cubren los logs históricos, logs en vivo, métricas e historial de ejecuciones. El acceso sigue **los permisos actuales**: cualquiera que esté autorizado actualmente puede leer todo el historial retenido bajo la misma clave de escena durante hasta 7 días, incluidos los logs escritos mientras otra persona desplegaba la escena. Esto incluye nuevos propietarios, operadores, inquilinos elegibles y wallets añadidas recientemente a `logsPermissions`. Ten en cuenta ese historial antes de otorgar acceso.

Los cambios de permisos no son inmediatos. De forma predeterminada, las cachés de permisos duran 60 segundos y los flujos en vivo abiertos vuelven a comprobar el acceso cada 5 minutos. Deja pasar aproximadamente 6 minutos para que un flujo abierto observe la revocación. Estos son tiempos predeterminados, no una fecha límite estricta: la configuración y los periodos de gracia acotados durante fallos temporales de capacidad pueden ampliar el retraso. Iniciar sesión no concede permiso para leer otras escenas.

## Iniciar sesión

```bash
npx sdk-commands scene-insights login
```

Aprueba el inicio de sesión en tu navegador con una wallet normal o una wallet de smart contract. Esto crea una sesión de Insights limitada a leer logs, métricas y ejecuciones. Dura hasta **12 horas**, limitada por la expiración de la sesión principal. Los comandos posteriores la reutilizan sin otra aprobación.

Comprueba la dirección autenticada y la expiración con:

```bash
npx sdk-commands scene-insights session --json
```

Cuando la sesión expire, ejecuta `login` de nuevo. Si un asistente de IA está investigando por ti, aprueba tú mismo el inicio de sesión de la wallet. No compartas claves privadas ni el archivo de sesión en caché con él.

### Mantén privado el archivo de sesión


> [!WARNING]
> **Nunca hagas commit, deploy ni compartas tu archivo de sesión de inicio de sesión.** De forma predeterminada, se guarda en `~/.dcl/scene-insights-session.json`. Contiene una clave privada temporal de firma y la delegación de solo lectura de tu wallet. Cualquiera con una copia puede usar esas credenciales para leer logs, métricas e historial de ejecuciones de escenas a las que tu wallet pueda acceder hasta que la sesión expire. No la envíes a compañeros de equipo, asistentes de IA, chats de soporte ni informes públicos de incidencias, ni la incluyas en archivos del proyecto.


El SDK la protege automáticamente:

* Guarda el archivo en tu directorio personal de forma predeterminada, fuera del proyecto de la escena. Si configuras `DCL_SCENE_INSIGHTS_SESSION_DIR`, usa el `.dcl` folder en su lugar.
* Añade `*` a la `.gitignore` y se niega a guardar o usar un archivo de sesión que Git ya controla.
* Escribe el archivo de sesión con permisos solo para el propietario (`0600`) y crea su carpeta con `0700` permisos en los sistemas que los admiten.
* El recopilador estándar de archivos de despliegue del SDK excluye los archivos y directorios que comienzan con punto, incluidos `.dcl`, independientemente de `.dclignore`.

Ten presentes estas comprobaciones:

* Revisa los archivos staged antes de hacer commit. No añadas a la fuerza el archivo de sesión; `.gitignore` no protege los archivos ya controlados por Git.
* Si personalizas la ubicación de la sesión, mantenla fuera de tu proyecto de escena siempre que sea posible. Nunca copies ni renombres la sesión dentro de assets de la escena ni de otra carpeta desplegable.
* Si usas scripts personalizados de deployment, copia de seguridad o ZIP, excluye explícitamente el archivo de sesión y `.dcl` las credenciales; esas herramientas pueden no aplicar las exclusiones del SDK.
* Comparte solo la salida de estado segura de `npx sdk-commands scene-insights session --json` cuando sea necesario, nunca el archivo de sesión en sí. Los compañeros de equipo deben iniciar sesión con sus propias wallets autorizadas.

`npx sdk-commands scene-insights logout` elimina la sesión guardada localmente. No **no** revoca las copias ya compartidas: esas credenciales pueden seguir siendo utilizables hasta su expiración. Si el archivo se expuso, elimínalo de los artefactos compartidos y del historial de Git según corresponda; borrar el archivo local o iniciar sesión de nuevo no invalida la copia expuesta.

## Elige una escena

Desde el proyecto de la escena, los comandos infieren el World a partir de `scene.json`'s `worldConfiguration.name`, o usa `scene.base` para Genesis City. También puedes elegir explícitamente:

```bash
# Un World
npx sdk-commands scene-insights logs --world myworld.dcl.eth

# Una escena en un World con varias escenas
npx sdk-commands scene-insights logs --world myworld.dcl.eth --position 10,10

# Una escena de Genesis City: usa = cuando las coordenadas sean negativas
npx sdk-commands scene-insights logs --position=-125,-96
```

Los comandos usan producción por defecto (`.org`). Para una escena en la red de desarrollo, añade `--target https://multiplayer-scenes-insights-service.decentraland.zone` para el inicio de sesión, las comprobaciones de sesión y las consultas. Usar el mismo target en todo momento también garantiza que las wallets de smart contract se validen en la red correcta.

## Buscar logs y ejecuciones

```bash
# Los últimos 200 errores en las últimas 6 horas
npx sdk-commands scene-insights logs --from 6h --level error --limit 200

# Encontrar texto en una ventana temporal UTC fija
npx sdk-commands scene-insights logs --from 2026-11-01T14:00:00Z --to 2026-11-01T14:30:00Z --query "player joined"

# Listar ejecuciones del servidor del último día
npx sdk-commands scene-insights executions --from 1d

# Logs de una ejecución: reemplaza EXECUTION_ID con un ID de executions
npx sdk-commands scene-insights logs --execution EXECUTION_ID --from 1d
```

`logs` usa por defecto la última hora y las 100 líneas coincidentes más recientes, mostradas cronológicamente. El CLI acepta `--limit` entre 1 y 10,000 y recupera varias páginas de la API según sea necesario. `--level` es una severidad mínima (`warn` incluye errores); `--query` busca texto literal con coincidencia ASCII sin distinguir mayúsculas y minúsculas. Usa `--source scene,engine,stderr` para seleccionar los orígenes de los logs, o pasa un subconjunto de estas fuentes.

`executionId` identifica una ejecución del servidor; `entityId` identifica la versión desplegada de la escena. Una nueva ejecución puede significar una parada/inicio en inactividad, un rollout o un fallo. Revisa los logs circundantes antes de concluir que se ha caído.

Añade `--json` para una salida legible por máquina. El CLI devuelve logs históricos como un solo documento JSON; su `more: true` campo significa que existen coincidencias adicionales más allá del límite devuelto. Reduce la ventana o aumenta el límite. Los clientes directos de la API reciben hasta 1,000 filas por página y usan un cursor para la paginación.

## Seguir logs en vivo

```bash
npx sdk-commands scene-insights tail
```

Pulsa **Ctrl+C** para detenerlo. Para una captura acotada, incluso cuando pidas a un asistente de IA que investigue:

```bash
npx sdk-commands scene-insights tail --for 30s --lines 20 --json
```

Esto reproduce las 20 líneas más recientes y luego sigue las nuevas durante 30 segundos. La salida JSON contiene una entrada de log por línea. El CLI se reconecta y reanuda durante un tail en ejecución. Los logs normalmente llegan unos segundos después de que la escena los escribe.

`tail` admite filtros de nivel y texto. Para ventanas temporales históricas o filtros de ejecución/fuente, usa `logs`.

Si usas `npm run server-logs`, comprueba que el de tu proyecto `package.json` contiene:

```json
{
  "scripts": {
    "server-logs": "sdk-commands scene-insights tail"
  }
}
```

Las compilaciones completas del proyecto con el SDK actualizado crean o migran el script predeterminado generado por el SDK. Los scripts personalizados se conservan; las vistas previas con `--skip-build` y las compilaciones de un solo archivo no lo actualizan. Puedes editar el script directamente sin recompilar solo para inspeccionar logs.

## Inspeccionar métricas de rendimiento

Primero lista las métricas que tienen datos y luego consulta una serie:

```bash
npx sdk-commands scene-insights metrics --json
npx sdk-commands scene-insights metrics cpu.percent --from 2h --step 60 --json
npx sdk-commands scene-insights metrics mem.heap_used --from 2h --step 60 --json
```

`--step` establece el tamaño del bucket en segundos. Las métricas disponibles dependen de la versión del engine y de la actividad de la escena. El descubrimiento de métricas y `executions` usan por defecto las últimas 24 horas. Usa `--execution EXECUTION_ID` en una serie de métricas para aislar una ejecución.

| Métrica                                                 | Qué te indica                                                                                                                       |
| ------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| `cpu.percent`                                           | CPU del hilo de ejecución como porcentaje de un solo núcleo; valores sostenidos cercanos al 100% sugieren que ese hilo está ocupado |
| `cpu.run_ms`                                            | Tiempo de CPU empleado en `onStart`/`onUpdate`, no latencia de reloj real                                                           |
| `mem.heap_used`, `mem.heap_limit`, `mem.external_bytes` | uso del heap de JavaScript, límite del heap y memoria externa de V8                                                                 |
| `logs.lines`, `logs.bytes`                              | salida de consola antes de los límites de tasa; útil para detectar un registro excesivo                                             |
| `fetch.failed`, `storage.failed`                        | errores al enviar solicitudes para llamadas HTTP salientes o de almacenamiento; correlaciónalos con los logs de error               |
| `fetch.status_4xx`, `fetch.status_5xx`                  | respuestas HTTP de error; estas no se cuentan como `fetch.failed`                                                                   |
| `logs.dropped`                                          | líneas descartadas por el presupuesto de logs por tick del engine                                                                   |
| `comms.msgs_out`, `comms.bytes_out`                     | Mensajes y bytes enviados a los jugadores                                                                                           |

Las métricas de CPU no miden todo el servidor, y las métricas de memoria no representan la memoria total del proceso ni el límite del contenedor. Compara la misma ejecución, ventana temporal y carga de trabajo antes de diagnosticar una fuga o un problema de rendimiento.

Los gauges devuelven `promedio` y `máximo` por bucket. Las sumas devuelven `suma` y una `tasa`. Un `nulo` punto significa que no hay una muestra almacenada, no que el uso sea cero; las escenas inactivas, los retrasos y los fallos de recolección pueden causar huecos.

## Retención y límites

| Límite                                  | Predeterminado                                                                                   |
| --------------------------------------- | ------------------------------------------------------------------------------------------------ |
| Retención de logs y métricas            | 7 días                                                                                           |
| Rango máximo de tiempo de consulta      | 7 días                                                                                           |
| Resultados de logs por consulta del CLI | 100 de forma predeterminada; hasta 10,000 mediante paginación                                    |
| Serie de métricas                       | Hasta 1,500 buckets; aumenta `--step` para ventanas más largas                                   |
| Registro sostenido de la escena         | 50 líneas/segundo y 64 KiB/segundo                                                               |
| Margen de ráfaga de logs                | 500 líneas y 512 KiB                                                                             |
| Cuota diaria de logs de la escena       | Límite blando de 500,000 líneas o 128 MiB por proceso de servidor; se restablece a las 00:00 UTC |

Los contadores de la cuota diaria se mantienen en memoria. Un reinicio del proceso de servidor o que una escena se mueva a otra réplica también restablece sus contadores, por lo que la cuota diaria es un límite aproximado a través de esos cambios.

Los límites de tasa y las cuotas diarias descartan los logs excedentes; las líneas descartadas no pueden recuperarse más tarde. Los mensajes de advertencia del engine informan de descartes y restablecimientos de cuota. Alcanzar la cuota diaria detiene la persistencia adicional de logs hasta el restablecimiento; la escena sigue ejecutándose. Reduce el registro repetido por frame para que los mensajes útiles quepan dentro del presupuesto.

El engine también tiene un presupuesto de logs separado por tick (1,000 líneas por defecto); las líneas excedentes se descartan antes de exportarse y se contabilizan en `logs.dropped`.

Los mensajes grandes pueden truncarse (la API usa 8,192 caracteres por mensaje de forma predeterminada). La recolección es de mejor esfuerzo: las interrupciones, los fallos de exportación y los límites de tasa pueden dejar huecos. Solo está disponible la telemetría recolectada; instalar el nuevo CLI no puede recuperar datos antiguos o descartados. Los resultados vacíos por sí solos no prueban que una escena esté sana.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/depuracion/scene-insights.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
