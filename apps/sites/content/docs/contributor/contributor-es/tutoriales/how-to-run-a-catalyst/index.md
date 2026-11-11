# Cómo ejecutar un nodo de Catalyst

¿Alguna vez has sentido curiosidad por saber cómo funcionan los nodos Catalyst de Decentraland? ¿Alguna vez has querido ejecutar tu propio nodo y te has sentido abrumado sin saber por dónde empezar? ¿Estás creando contenido para Decentraland y te resulta difícil hacer todo el ciclo de desarrollo y pruebas en servidores de producción?

Si eres así (o has pasado por eso), no temas más. Este tutorial te ayudará en el proceso de planificación, a averiguar qué necesitas decidir y luego te guiará paso a paso para poner en marcha el servidor. Además, por si algo no sale según lo previsto, al final hay una sección de solución de problemas con algunos de los problemas más comunes y cómo solucionarlos.

¿Listo? Manos a la obra.

### Requisitos de hardware

Los servidores de la Decentraland Foundation están desplegados en Amazon AWS en `t2.xlarge` instancias. Así que, aunque esos servidores están pensados para uso público, el tuyo podría funcionar bien con especificaciones mucho menores, dependiendo de cuál sea el uso previsto.

Un AWS EC2 `t2.xlarge` tiene el siguiente hardware:

* 4 vCPU.
* 16 GB de RAM

Y para almacenamiento de archivos + almacenamiento de la DB, un SSD / HDD con 2 TB de capacidad debería ser más que suficiente. En el momento de escribir esto, los servidores de la foundation están usando un poco más de 1 TB para todo el almacenamiento.

### Prerrequisitos de software

Se requiere lo siguiente para ejecutar catalyst-owner:

* Sistema operativo Linux / MacOS
* Docker / docker-compose
* Git
* cluster LiveKit

### Algunas opciones a considerar antes de empezar

* ¿Quieres que tu nodo esté expuesto públicamente en Internet? Si es así, necesitarás una instancia expuesta a Internet y una URL pública para conectarte a ella.
* ¿El nodo se va a usar para el desarrollo de escenas, wearables, etc.? En este caso, las especificaciones de hardware pueden ser mucho menores, ya que probablemente lo accederán una o dos personas, no cientos de usuarios.
* ¿Quieres sincronizar todos los tipos de entidades? Si vas a usar esto para desarrollar escenas, probablemente quieras omitir la sincronización de profiles, ya que consumen mucho tiempo, ancho de banda y, sobre todo, espacio en disco, y no te beneficiarán para tu objetivo.

### LiveKit

El servicio Comms del Catalyst necesita un cluster externo de LiveKit para orquestar las comunicaciones entre jugadores. Para ello, hay dos opciones: usar una cuenta de LiveKit Cloud o ejecutar tu propio cluster de LiveKit.

La recomendación es configurar [una cuenta de LiveKit Cloud](https://cloud.livekit.io/) que incluye un plan gratuito que puede gestionar 100 usuarios, o pagar el servicio según el tráfico. Este enfoque es más sencillo, ya que no requiere aprovisionar infraestructura adicional y el servicio puede gestionar el escalado. De lo contrario, habrá que aprovisionar un cluster de LiveKit. LiveKit hace un muy buen trabajo con su documentación:

* [Despliegue](https://docs.livekit.io/oss/deployment/)
* [Configuración distribuida](https://docs.livekit.io/oss/deployment/distributed/)

*Si quieres profundizar en los detalles técnicos necesarios para soportar comunicaciones basadas en transporte de LiveKit, consulta la* [*ADR-70 Nueva arquitectura de comunicaciones*](https://adr.decentraland.org/adr/ADR-70)*.*

### Guía paso a paso

Lo primero que debes hacer es clonar el [catalyst-owner](https://github.com/decentraland/catalyst-owner) repositorio desde GitHub

```bash
> git clone https://github.com/decentraland/catalyst-owner.git
Cloning into 'catalyst-owner'...
remote: Enumerating objects: 2392, done.
remote: Counting objects: 100% (602/602), done.
remote: Compressing objects: 100% (168/168), done.
remote: Total 2392 (delta 506), reused 481 (delta 430), pack-reused 1790
Receiving objects: 100% (2392/2392), 926.05 KiB | 1.09 MiB/s, done.
Resolving deltas: 100% (1257/1257), done.
> cd catalyst-owner
```

Una vez hecho esto, es hora de configurar las variables de entorno para el nodo y hacer los cambios que correspondan.

```bash
# En la carpeta /catalyst-owner
> cp .env.example .env
> cp .env-advanced.example .env-advanced
```

Ahora, usando tu editor de texto favorito, haz los cambios necesarios en estos archivos. Por ejemplo, configurar la `EMAIL` variable de entorno con un correo electrónico válido es necesario para que puedas recibir actualizaciones sobre la expiración del certificado de Certbot.

También puede ser necesario configurar tu valor particular para `CATALYST_URL` especialmente si tu servidor va a estar expuesto públicamente en Internet. Para usarlo en tu máquina local, el valor predeterminado de `http://localhost` servirá.

**Servicio Comms**

Una vez que el cluster de LiveKit esté disponible, tendrás que configurar las variables específicas de LiveKit para que el servicio Comms funcione, por ejemplo:

```
LIVEKIT_HOST=wss://livekit-1.mydomain.org
LIVEKIT_API_KEY=API-JjHuvM
LIVEKIT_API_SECRET=J7YSHmNzkNCEfT2
ROOM_PREFIX=my-prefix
```

El `La variable ROOM_PREFIX` es opcional y se puede configurar para identificar las rooms de LiveKit creadas por el Catalyst. Esto es útil si más de un Catalyst está usando el mismo cluster de LiveKit.

**Servidor de contenido**

Hay una variable llamada `CONTENT_SERVER_STORAGE` que define la carpeta local que el content server usará para el almacenamiento. Por defecto, esto es `CONTENT_SERVER_STORAGE=./storage`. Puedes cambiar este valor para almacenar archivos en otro lugar, o al menos asegurarte de que la carpeta referenciada exista. Puede que tengas que crearla así:

```bash
# En la carpeta /catalyst-owner o dondequiera que apunte CONTENT_SERVER_STORAGE
> mkdir storage
```

Hay otra variable interesante `SYNC_IGNORED_ENTITY_TYPES` que permite ignorar ciertos tipos de entidades durante el proceso de sincronización. Si vas a usar el servidor Catalyst para desarrollo, quizá no necesites que se sincronice todo el tipo de contenido. Puedes configurar la variable de entorno `SYNC_IGNORED_ENTITY_TYPES="profile,store"` para que solo se traigan escenas y wearables desde los servidores de la DAO. Esto ahorrará mucho tiempo, ancho de banda y almacenamiento en disco en tu servidor.

> 💡 Para una lista más exhaustiva de todas las variables de entorno compatibles, échale un vistazo a la [sección de Variables de entorno](#environment-variables) más abajo.

#### Iniciar el nodo Catalyst

Una vez configuradas todas las variables de entorno, es hora de iniciar el nodo. Eso debería ser tan fácil como ejecutar el `init.sh` script en la carpeta raíz.

```bash
# En la carpeta /catalyst-owner

> ./init.sh
## Cargando variables de entorno... [ OK ]
[ OK ]
## Comprobando si el correo electrónico está configurado... [ OK ]
## Comprobando si el almacenamiento está configurado... [ OK ]
## Comprobando si la URL de Catalyst está configurada... [ OK ]
 ADVERTENCIA: No estás ejecutando la última imagen de los nodos Content y Lambdas de Catalyst.
 ADVERTENCIA: No estás ejecutando la última imagen del nodo Archipelago de Catalyst.
 ADVERTENCIA: No estás ejecutando la última imagen del nodo Explorer BFF de Catalyst.
 - DOCKER_TAG:               45a5154f11b53d55aadfdf7f958e2fce6a964824
 - LIGHTHOUSE_DOCKER_TAG:    latest
 - CATALYST_URL:             http://localhost
 - CONTENT_SERVER_STORAGE:   ./storage
 - EMAIL:                    a@a.com
 - ETH_NETWORK:              mainnet
 - REGENERATE:               0

Iniciando en 5 segundos...
45a5154f11b53d55aadfdf7f958e2fce6a964824: Descargando de decentraland/catalyst-content
Digest: sha256:d6c2981c57cb9367fdb3228d15b07e4d26e62c902944115937de24ecd3ab6aac
Estado: la imagen está actualizada para decentraland/catalyst-content:45a5154f11b53d55aadfdf7f958e2fce6a964824
docker.io/decentraland/catalyst-content:45a5154f11b53d55aadfdf7f958e2fce6a964824
45a5154f11b53d55aadfdf7f958e2fce6a964824: Descargando de decentraland/catalyst-lambdas
Digest: sha256:5605812dd29316afc32e561d2a2ce1311f164f07ef3cd99a1cd60727518636e5
Estado: la imagen está actualizada para decentraland/catalyst-lambdas:45a5154f11b53d55aadfdf7f958e2fce6a964824
docker.io/decentraland/catalyst-lambdas:45a5154f11b53d55aadfdf7f958e2fce6a964824
latest: Descargando de decentraland/catalyst-lighthouse
Digest: sha256:fedc10b714823909f0a1c17955eed2f22a02e4f784090848d3253ef734e410d4
Estado: la imagen está actualizada para decentraland/catalyst-lighthouse:latest
docker.io/decentraland/catalyst-lighthouse:latest
latest: Descargando de decentraland/archipelago-service
Digest: sha256:e2c1d6fa96a5cfbbf6fb37e3840c0724fb5abeef8c366025a28fab3ad33d6480
Estado: la imagen está actualizada para quay.io/decentraland/archipelago-service:latest
quay.io/decentraland/archipelago-service:latest
latest: Descargando de decentraland/explorer-bff
Digest: sha256:fa9e305c44972a8613b01f4cd573d6fb84b0fb03ce953a506fbb06053202913c
Estado: la imagen está actualizada para quay.io/decentraland/explorer-bff:latest
quay.io/decentraland/explorer-bff:latest
Deteniendo nginx ... hecho
## Usando HTTP porque CATALYST_URL está configurada en http://localhost
## Reemplazando el valor $katalyst_host en el archivo del servidor nginx... [ OK ]
## Reiniciando contenedores...
Creando red "catalyst-owner_default" con el controlador predeterminado
Creando catalyst-owner_comms-server_1 ... hecho
Creando node-exporter                   ... hecho
Creando catalyst-owner_lambdas_1      ... hecho
Creando postgres                      ... hecho
Creando catalyst-owner_certbot_1        ... hecho
Creando nats                            ... hecho
Creando postgres-exporter               ... hecho
Creando catalyst-owner_content-server_1 ... hecho
Creando catalyst-owner_archipelago_1    ... hecho
Creando catalyst-owner_explorer-bff_1   ... hecho
Creando nats-exporter                   ... hecho
Creando cadvisor                        ... hecho
Creando nginx                           ... hecho
## El servidor Catalyst está funcionando en http://localhost
```

Si todo salió bien, ahora tenemos un nodo completo de Decentraland en funcionamiento. Ahora puedes ir al navegador y escribir la URL. Si usas la predeterminada, debería ser [`http://localhost`](http://localhost/).

Podemos comprobar los logs del servidor de contenido con este comando de docker:

```bash
> docker logs catalyst-owner_content-server_1
```

Aunque el servidor ya está en funcionamiento, todavía no está 100% listo para operar. Necesita sincronizar el contenido de los otros [servidores de la DAO](https://decentraland.github.io/catalyst-monitor) para poder ofrecer la misma experiencia a los usuarios conectados a él. La sincronización puede llevar mucho tiempo. Con una buena conexión a Internet, 6 horas debería ser algo bastante común. Así que... es hora de tomar un café, una siesta, un buen descanso nocturno y volver mañana.

Una forma de saber si la sincronización ha terminado consiste en comprobar los resultados del endpoint de estado del contenido: <http://localhost/content/status> (usa tu URL aquí).

```json
{
  "synchronizationStatus": {
    "lastSyncWithDAO": 1658153514917,
    "synchronizationState": "Bootstrapping"
  },
  "snapshot": {
    "entities": {
      "profile": 1271331,
      "scene": 23477,
      "wearable": 17351,
      "store": 890
    },
    "lastUpdatedTime": 1658153414530
  },
  "version": "v3",
  "commitHash": "9a2e0d5d05d02646df2e1e5d00436d3166a07aa1",
  "catalystVersion": "4.8.6",
  "ethNetwork": "mainnet"
}
```

Mientras `synchronizationState` es `Bootstrapping`, el nodo suspenderá temporalmente la aceptación de nuevos despliegues. Esta medida garantiza que no se desplieguen nuevas entidades hasta que el nodo se ponga al día dentro de la red DAO. Una vez que el estado cambie a `Syncing`, indica que el nodo se ha puesto al día correctamente y ahora recibe continuamente las últimas actualizaciones. Este es el estado saludable en el que el nodo funciona completamente y acepta nuevos despliegues.

```json
{
  "synchronizationStatus": {
    "lastSyncWithDAO": 1658153872301,
    "synchronizationState": "Syncing"
  },
  "snapshot": {
    "entities": {
      "profile": 1271317,
      "scene": 23477,
      "store": 890,
      "wearable": 17350
    },
    "lastUpdatedTime": 1658152050030
  },
  "version": "v3",
  "commitHash": "9a2e0d5d05d02646df2e1e5d00436d3166a07aa1",
  "catalystVersion": "4.8.6",
  "ethNetwork": "mainnet"
}
```

Si eres más de usar la CLI, puedes filtrar los logs de la imagen Docker del servidor de contenido para este mensaje: `Starting to sync entities from servers pointer changes`. Una vez que veas ese texto, el servidor de contenido estará completamente sincronizado y listo para usarse por completo.

### sección de Variables de entorno

Lo siguiente es una lista completa de todas las variables de entorno del servidor de contenido con sus valores predeterminados, tal como aparecen registradas en los logs del servidor de contenido durante el inicio:

```jsx
STORAGE_ROOT_FOLDER: "/app/storage/content_server/"
DENYLIST_FILE_NAME: "denylist.txt"
DENYLIST_URLS: "https://config.decentraland.org/denylist"
SYNC_IGNORED_ENTITY_TYPES: ""
FOLDER_MIGRATION_MAX_CONCURRENCY: 1000
SERVER_PORT: 6969
LOG_REQUESTS: false
UPDATE_FROM_DAO_INTERVAL: 1800000
SYNC_WITH_SERVERS_INTERVAL: 45000
CHECK_SYNC_RANGE: 1200000
DECENTRALAND_ADDRESS: "0x1337e0507eb4ab47e08a179573ed4533d9e22a7b"
DEPLOYMENTS_DEFAULT_RATE_LIMIT_TTL: 60
DEPLOYMENTS_DEFAULT_RATE_LIMIT_MAX: 300
ETH_NETWORK: "mainnet"
LOG_LEVEL: "debug"
FETCH_REQUEST_TIMEOUT: "2m"
USE_COMPRESSION_MIDDLEWARE: false
BOOTSTRAP_FROM_SCRATCH: false
REQUEST_TTL_BACKWARDS: 1200000
LAND_MANAGER_SUBGRAPH_URL: "https://api.thegraph.com/subgraphs/name/decentraland/land-manager"
COLLECTIONS_L1_SUBGRAPH_URL: "https://api.thegraph.com/subgraphs/name/decentraland/collections-ethereum-mainnet"
COLLECTIONS_L2_SUBGRAPH_URL: "https://api.thegraph.com/subgraphs/name/decentraland/collections-matic-mainnet"
THIRD_PARTY_REGISTRY_L2_SUBGRAPH_URL: "https://api.thegraph.com/subgraphs/name/decentraland/tpr-matic-mainnet"
BLOCKS_L1_SUBGRAPH_URL: "https://api.thegraph.com/subgraphs/name/decentraland/blocks-ethereum-mainnet"
BLOCKS_L2_SUBGRAPH_URL: "https://api.thegraph.com/subgraphs/name/decentraland/blocks-matic-mainnet"
PSQL_DATABASE: "content"
PSQL_HOST: "postgres"
PSQL_SCHEMA: "public"
PSQL_PORT: "5432"
GARBAGE_COLLECTION: true
GARBAGE_COLLECTION_INTERVAL: 21600000
PG_IDLE_TIMEOUT: 30000
PG_QUERY_TIMEOUT: 60000
PG_STREAM_QUERY_TIMEOUT: 600000
SNAPSHOT_FREQUENCY_IN_MILLISECONDS: 21600000
CUSTOM_DAO: undefined
DISABLE_SYNCHRONIZATION: false
SYNC_STREAM_TIMEOUT: "10m"
CONTENT_SERVER_ADDRESS: "http://localhost/content/"
REPOSITORY_QUEUE_MAX_CONCURRENCY: 50
REPOSITORY_QUEUE_MAX_QUEUED: 300
REPOSITORY_QUEUE_TIMEOUT: "1m"
ENTITIES_CACHE_SIZE: 150000
DEPLOYMENT_RATE_LIMIT_MAX: {}
DEPLOYMENT_RATE_LIMIT_TTL: {}
VALIDATE_API: false
RETRY_FAILED_DEPLOYMENTS_DELAY_TIME: 900000
```

### Usar tu nodo para producción

Si quieres ejecutar tu propio servidor y ayudar a escalar la red, ante todo eso es genial; la comunidad y la foundation realmente agradecen que lo hagas.

En este caso, tendrás que pasar por el proceso de solicitar la aprobación de la DAO para unirte a la red visitando [este enlace](https://governance.decentraland.org/submit/catalyst/). También puedes solicitar una subvención de MANA para cubrir los gastos de infraestructura y mantenimiento.

Es importante que las especificaciones de tu hardware se acerquen más a lo sugerido arriba en los requisitos de hardware, ya que el servidor será utilizado por cualquier miembro de la comunidad para entrar en Decentraland.

### Especificaciones de la API

Para más detalles sobre las APIs de content, lambdas y comms servers, puedes consultar las [especificaciones de la API](https://decentraland.github.io/catalyst-api-specs).

### Solución de problemas/Preguntas frecuentes

Pueden salir mal algunas cosas al iniciar tu servidor. Aquí tienes algunos de los problemas más comunes encontrados con el tiempo y cómo solucionarlos.

#### El puerto 5432 está siendo usado por el postgres local

Si ejecutas el nodo en una máquina usada para desarrollo, o si ya está alojando otros servicios, es probable que ya haya un servidor de base de datos postgres ejecutándose en esa máquina. Así que el puerto 5432 ya estará ocupado.

La forma más fácil de solucionarlo es detener el postgres que ya está ejecutándose localmente y luego reiniciar el contenedor Docker de postgres usando `docker start postgres`.

Si tienes conocimientos de docker compose, puedes empezar a modificar `docker-compose.yml` y las variables de entorno para intentar que funcione en un puerto diferente. Para mantener las cosas simples aquí, evitaremos tomar ese camino en este tutorial.

#### El puerto 80 está siendo usado por nginx local

Lo mismo puede ocurrir con el puerto 80 (e incluso con el 443). Si estás ejecutando nginx en la máquina local, es probable que los puertos 80/443 ya estén vinculados a ese servicio, por lo que el contenedor Docker de Catalyst no podrá usarlos.

De nuevo, el enfoque más sencillo sería detener el servicio local de nginx y luego reiniciar el contenedor Docker usando `docker start nginx`. O, alternativamente, necesitas aprovechar `docker-compose.yml` y `.env` para hacerlo funcionar en un puerto diferente.

### Documentación adicional de referencia

Si quieres entender mejor qué hace un servidor Catalyst, qué servicios incluye, etc., puedes consultar el repositorio de arquitectura <https://github.com/decentraland/architecture> que explica las partes en detalle.

Si te gustaría contribuir al código de los servidores Catalyst, asegúrate de revisar la [guía de contribución](https://github.com/decentraland/catalyst/blob/main/docs/CONTRIBUTING.md) para aprender sobre nuestro proceso de desarrollo, cómo proponer correcciones de errores y mejoras, y cómo construir y probar tus cambios.

Y por último, si necesitas contactar al equipo, puedes hacerlo a través de [Discord](https://discord.com/channels/417796904760639509/948230185457696820) o enviando [issues en GitHub](https://github.com/decentraland/catalyst/issues).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/tutoriales/how-to-run-a-catalyst.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
