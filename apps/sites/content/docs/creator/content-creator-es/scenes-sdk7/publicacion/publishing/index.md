> ¿Cómo publico mi proyecto?

# Publicación

## Antes de comenzar

Asegúrate de lo siguiente:

* Tu escena cumple con todas las [limitaciones de la escena](/docs/creator/content-creator-es/scenes-sdk7/optimizacion/scene-limitations/). La mayoría de estas se validan cada vez que ejecutas un preview de tu escena.
* Si quieres que tu escena funcione bien para jugadores móviles, primero haz preview y pruébala en un dispositivo real; consulta [Building for Mobile](/docs/creator/content-creator-es/desarrollar-para-movil/cliente-movil/overview/) y [Preview on mobile](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/preview-on-mobile/). Una vez publicada, se puede acceder a tu escena desde la aplicación móvil de Decentraland de la misma manera que desde escritorio.
* Tienes una cuenta de [Metamask](https://metamask.io/) , con tus parcelas LAND asignadas a ella.
* Posees la cantidad necesaria de parcelas LAND adyacentes o un NAME de Decentraland. De lo contrario, puedes comprar LAND en el [Market](https://market.decentraland.org) o un NAME en el [Marketplace](https://decentraland.org/shop/items?category=names).


> [!WARNING]
> \*\*📔 Nota\*\*: Las escenas de varias parcelas solo pueden desplegarse en parcelas adyacentes.



> [!CAUTION]
> **❗Advertencia**: Al planificar eventos en vivo, no hagas cambios de última hora en la escena justo antes del evento. Publica tu versión final al menos una hora antes de que comience el evento.
> 
> Después de cada publicación, un proceso interno optimiza todos los modelos 3D antes de que puedan renderizarse. Esto suele tardar solo unos segundos, pero puede tardar más en escenas muy grandes o cuando los servidores están ocupados. Hasta que termine, los jugadores reciben la versión anterior de tu escena.


## Verificar los datos de la escena

Al desplegar, la CLI lee información del archivo *scene.json* para determinar dónde desplegar tu escena.

Abre el archivo de tu escena *scene.json* y completa los siguientes datos:

* **title**: El título se muestra en la UI debajo del minimapa, cada vez que los jugadores entran a tu escena. También aparece en la ventana emergente de teleport.
* **description**: Una descripción de lo que los jugadores encontrarán en tu escena. Se muestra en la ventana emergente de teleport.
* **navmapThumbnail**: Una imagen que representa tu escena. Se muestra en la ventana emergente de teleport. La imagen debe ser una imagen *.png* o *.jpg* con una relación de aspecto de 16:9, con un tamaño recomendado de *1920x1080* píxeles. Mantén todo lo esencial dentro del cuadrado central de *1080x1080* , ya que algunas partes de la plataforma muestran un recorte cuadrado de ella. Consulta [miniatura de la escena](/docs/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/scene-metadata/#scene-thumbnail).
* **Parcels**: Las coordenadas de las parcelas que ocupará tu escena
* **Base**: Las coordenadas de la parcela que se considerará la coordenada \[0,0] de la escena. Si tu escena tiene varias parcelas, debe ser la parcela inferior izquierda (suroeste).
* **spawnPoints**: Un conjunto de coordenadas dentro de la escena (relativas a la parcela base de la escena) donde aparecen los jugadores. De forma predeterminada, los jugadores aparecen en la *0,0,0* ubicación de la escena (esquina inferior izquierda). Usa esto para iniciar a los jugadores en una ubicación específica o establece una región para evitar que los jugadores se superpongan entre sí cuando aparezcan por primera vez.
* **tags**: Un conjunto de tags que describen tu escena. Estos ayudan a los jugadores a explorar Decentraland al facilitar la búsqueda de contenido que les interesa. [Consulta la lista de categorías disponibles](/docs/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/scene-metadata/#categories).
* **rating**: Esto se usa para clasificar el contenido de tu escena. Decentraland es una plataforma para mayores de 18 años, por lo que el valor que debes establecer es `A` para adultos. Consulta [Age Rating](/docs/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/scene-metadata/#age-rating).


> [!WARNING]
> **📔 Nota**: Consulta [metadatos de la escena](/docs/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/scene-metadata/) para obtener más detalles sobre cómo establecer estos parámetros.


## Para publicar la escena

### Usar el Scene Editor en Creator Hub

El Scene Editor en Creator Hub proporciona una forma sencilla de publicar tus escenas. Asegúrate de haber [instalado Creator Hub](/docs/creator/content-creator-es/scene-editor/primeros-pasos/editor-installation/).

1. Abre el proyecto de tu escena.
2. Haz clic en el botón **Publish** en la esquina superior derecha.
3. Un mensaje te preguntará si quieres publicar en un **WORLD** o en **LAND**.
   * Selecciona **PUBLISH TO WORLD** para hacer que tu escena esté disponible en uno de tus [WORLDs](/docs/creator/content-creator-es/scenes-sdk7/publicacion/publishing-options/#decentraland-worlds). Luego selecciona en cuál de tus NAMEs o dominios ENS publicar.
   * Selecciona **PUBLISH TO LAND** si posees terreno o un propietario te ha dado permisos de despliegue. Luego selecciona en el mapa las parcelas donde quieres desplegarla. Las parcelas en las que tienes permitido desplegar se muestran en rosa.
   * Selecciona **Servidores alternativos** para publicar en un [servidor personalizado](#custom-servers).

![](/docs/_remote/75db93f7a2fdd17bec3b4907.webp)

4. Esto abre una nueva pestaña en tu navegador que muestra detalles sobre la carga. Firma el despliegue con tu Wallet. Esto es solo una firma para demostrar la propiedad; es gratis y no implica ninguna transacción de Blockchain.
   * Para LAND en una cuenta de navegador de Metamask, confirma el despliegue. Luego firma el mensaje en la extensión de navegador de Metamask.
   * Para LAND vinculado a una Wallet que puedes usar mediante Wallet Connect, haz clic en **Connect wallet**, luego escanea el código QR con tu dispositivo móvil y sigue los pasos en Wallet Connect.

### Mediante la CLI

1. Inicia sesión en tu cuenta de Metamask con la misma dirección pública asociada a tus parcelas en Decentraland.
2. Ejecuta `npm run deploy` desde la carpeta de la escena.


> [!NOTE]
> \*\*💡 Consejo\*\*: Si hay archivos en la carpeta de tu proyecto que no quieres desplegar, inclúyelos en el archivo \_.dclignore\_ antes de desplegar.


3. Se abrirá una pestaña del navegador que mostrará en qué parcelas estás desplegando. Haz clic en **Sign and Deploy**.
4. Metamask se abre y te notifica que se solicita tu firma. Haz clic en **Sign** para confirmar esta acción.


> [!NOTE]
> **💡 Consejo**: Si estás implementando un flujo de integración continua, en el que los cambios de tu escena se despliegan automáticamente, puedes establecer la variable de entorno `export DCL_PRIVATE_KEY` en la clave privada de una cuenta que tenga permisos de despliegue.



> [!NOTE]
> **💡 Consejo**: `npm run deploy` ejecuta un `npm run build`, que comprueba los errores de tipo de la escena de forma más estricta que al ejecutar `npm run start`. Si estos errores no se pueden evitar (por ejemplo, ocurren en una Library externa) y no afectan la escena, puedes usar `npm run deploy -- --skip-build` para omitir el paso de `npm run build` y desplegar la escena tal como está.


## Publicar en Worlds

Para publicar tu escena en un World de Decentraland, debes poseer un NAME de Decentraland o un dominio ENS. Consulta [Opciones de publicación](/docs/creator/content-creator-es/scenes-sdk7/publicacion/publishing-options/#decentraland-worlds) para obtener información sobre cómo conseguir uno.

### Configurar scene.json

Debes especificar bajo qué **nombre** debe realizarse tu despliegue. Agrega la siguiente sección en tu `scene.json`:

```json
{
	"worldConfiguration": {
		"name": "my-name.dcl.eth"
	}
}
```

El **nombre** especificado en el `scene.json` puede ser un NAME de Decentraland o un dominio ENS. Debe pertenecer a la Wallet que firma el despliegue, o a cualquier Wallet a la que se le haya dado permiso mediante Access Control Lists (ACL).

Ten en cuenta lo siguiente:

* La Wallet que firma el despliegue debe poseer el NAME especificado en el `scene.json` archivo
* La escena no tiene limitaciones de parcelas (desde enero de 2023)
* Todos los Worlds se incluyen automáticamente en la página Places, a menos que optes por excluirlos como se detalla a continuación

### Metadatos del World

Un World tiene su propio nombre, descripción y miniatura, separados de los metadatos de las escenas publicadas en él. Cuando publicas en un World vacío o en un World que contiene una sola escena, los metadatos del World se actualizan para coincidir con los metadatos de la escena en cada publicación. Si el World contiene más de una escena, sus metadatos son independientes: publicar una escena no los modifica, y solo pueden editarse en la sección **Manage** de Creator Hub. Consulta [Metadatos del World frente a metadatos de la escena](/docs/creator/content-creator-es/scene-editor/publicar/publish-scene/#world-metadata-vs-scene-metadata).

### Presupuesto de almacenamiento de Worlds

Las escenas desplegadas en Worlds cuentan para un presupuesto de almacenamiento compartido entre todos los Worlds que posee tu Wallet. El presupuesto aumenta según tus activos de Decentraland:

* Cada NAME de Decentraland que posees concede 100 MB.
* Cada parcela LAND que posees concede 100 MB adicionales.
* Cada 2000 MANA en tu Wallet concede 100 MB adicionales.

Puedes distribuir este presupuesto entre tus Worlds como quieras. Por ejemplo, con un presupuesto de 500 MB podrías desplegar una escena de 300 MB y una de 200 MB, o cinco escenas de 100 MB. Los Worlds publicados en dominios ENS no consumen este presupuesto; en su lugar, tienen un límite fijo de 36 MB por World.

Puedes comprobar cuánto de tu presupuesto se ha utilizado y cuánto queda en dos lugares:

* El **Manage** sección de Creator Hub. Haz clic en **View Details** para ver un desglose de cómo se suman tus activos de MANA, LAND y NAME.
* El **Worlds** pestaña del [Builder](https://decentraland.org/builder/worlds).

Si superas tu presupuesto (por ejemplo, después de vender o transferir activos), tienes 48 horas para liberar espacio o aumentar tu presupuesto antes de que tus Worlds se vuelvan inaccesibles. Consulta [Límites de tamaño de Worlds](/docs/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/kinds-of-project/#size-limits) para obtener más detalles.

### Worlds con varias escenas

De forma predeterminada, al desplegar en un World mediante la CLI, tu escena se tratará como un World de una sola escena. Si tu World ya contiene una escena, desplegar una nueva escena sobrescribirá la anterior, incluso si las coordenadas son diferentes.

Para desplegar como un World con varias escenas, agrega el parámetro `--multi-scene` al comando de despliegue:

`npm run deploy -- --multi-scene --target-content https://worlds-content-server.decentraland.org`

Al usar este parámetro, si la escena que publicas tiene coordenadas diferentes de las escenas existentes en ese World, todas estas escenas coexistirán.


> [!WARNING]
> \*\*📔 Nota\*\*: Si tienes derechos de operador en un World que no posees y recibes derechos solo sobre parcelas específicas de ese World, solo podrás publicar en esas parcelas y no podrás afectar el contenido de otras parcelas del World.


### Excluir de la lista de Places

Si deseas excluir tus Worlds de la indexación en Places, puedes agregar la siguiente sección en tu `scene.json`:

```json
{
	"worldConfiguration": {
		"name": "my-name.dcl.eth",
		"placesConfig": {
			"optOut": true
		}
	}
}
```

### Usar el Scene Editor en Creator Hub

1. Abre el proyecto de tu escena.
2. Haz clic en el botón **Publish** en la esquina superior derecha.
3. Selecciona **PUBLISH TO WORLD**.
4. Selecciona en cuál de tus NAMEs o dominios ENS publicar.

### Mediante la CLI

Usa el siguiente comando:

```bash
npm run deploy -- --target-content https://worlds-content-server.decentraland.org
```

Una vez que ejecutes el comando, se te pedirá firmar el despliegue con tu Wallet y se ejecutará un conjunto de validaciones para permitir o rechazar la escena.


> [!WARNING]
> **📔 Nota**: Cuando tu `scene.json` tiene una `worldConfiguration` sección, `npm run deploy` necesita la flag `--target-content` (o `--target`). Sin ninguna de las dos, el comando se detiene con el error `DEPLOY_MISSING_TARGET_CONTENT`.


### Acceder a un World

Una vez que se carga una escena en el Server de Worlds, puedes acceder a ella mediante el Explorer de Decentraland con la siguiente URL: `decentraland://?realm=NAME.dcl.eth`, donde `NAME` debe reemplazarse por el NAME de Decentraland o el dominio ENS en el que desplegaste.

Con Decentraland ya abierto, también puedes ir a un World escribiendo el comando `/goto NAME.dcl.eth` en el cuadro de chat.

Consulta [Hacer que se pueda descubrir](/docs/creator/content-creator-es/scenes-sdk7/publicacion/make-discoverable/) para obtener más información sobre cómo hacer que tu World se pueda descubrir.

### Migrar un World a Genesis City

Si eres propietario de LAND y deseas desplegar una escena de World en Genesis City, puedes hacerlo volviendo a desplegar tu escena en la red descentralizada de Catalyst, que es el Server de contenido para Genesis City.

Cosas que recordar:

* Elimina la sección `worldConfiguration` de `scene.json`
* La limitación de tamaño para Worlds (dinámica según los activos) es diferente de la de las parcelas LAND (15 MB por parcela), así que asegúrate de que tu escena tenga el tamaño adecuado para los despliegues en Genesis City.

## Publicar escenas grandes

Las escenas grandes se cargan por lotes, por lo que puedes publicar escenas de más de unos 200 MB en Genesis City o en un World. Esto requiere `@dcl/sdk` 7.31.0 o posterior en tu proyecto. Creator Hub publica con el SDK instalado en tu proyecto, por lo que ocurre lo mismo cuando publicas desde Creator Hub.

Cómo funciona la carga:

* **Menos de 50 MiB** (aproximadamente 52 MB) de archivos en total: la escena se carga en una sola Request, como antes.
* **50 MiB o más**: los archivos se cargan en lotes de 32 MiB, tres lotes a la vez. Un archivo nunca se divide entre lotes. Si un archivo supera los 32 MiB, los lotes aumentan para que quepa.
* Después de cada lote, la CLI imprime una línea de progreso como `Se cargaron 96.0 MB de 268.8 MB (lote 3/9)`.

Los lotes no cambian los límites de tamaño. Cada archivo aún debe tener menos de 50 MB y toda la escena debe ajustarse al [límite de tamaño total de archivos](/docs/creator/content-creator-es/scenes-sdk7/optimizacion/scene-limitations/) de sus parcelas o de tu [Presupuesto de almacenamiento de Worlds](#worlds-storage-budget).


> [!WARNING]
> **📔 Nota**: Si el Server en el que despliegas no admite cargas por lotes, como puede suceder con un [servidor personalizado](#custom-servers), la CLI verifica qué archivos faltan en el Server. Si estos suman 200 MB o menos, muestra la advertencia `El Server de destino no admite cargas por lotes; se cargará en una sola Request.` y los carga en una Request. Si suman más, el despliegue falla con el error `DEPLOY_PARTIAL_NOT_SUPPORTED`. Una sola Request no puede superar los 200 MB porque la infraestructura frente a los Servers de contenido agota el tiempo de espera.


### Cancelar un despliegue

La pestaña del navegador que se abre cuando ejecutas `npm run deploy` muestra el progreso de la carga, con un botón **Cancel** . Cancelar detiene la carga. Si el Server ya publicó la escena cuando llega la cancelación, el despliegue termina como publicado.


> [!CAUTION]
> **❗Advertencia**: Cuando publicas en un World sin `--multi-scene`, y el World tiene escenas en otras parcelas, la CLI elimina esas escenas antes de cargar la tuya (consulta [Worlds con varias escenas](#multi-scene-worlds)). Si cancelas después de que se eliminaron, el World puede quedar vacío y la CLI termina con el error `DEPLOY_CANCELLED_AFTER_UNDEPLOY`. Despliega de nuevo para publicar la escena.


Si la CLI no puede confirmar si el Server publicó la escena, termina con el error `DEPLOY_CANCEL_OUTCOME_UNCERTAIN`. Verifica la escena antes de desplegar de nuevo.

Las herramientas que ejecutan el despliegue sin la pestaña del navegador pueden cancelarlo mediante la API de despliegue con un Token. La CLI crea un Token aleatorio en cada ejecución. Para establecer el tuyo, pasa `--deploy-token` con un valor de al menos 16 caracteres:

```bash
npm run deploy -- --deploy-token my-own-deploy-token-0001
```

Con `--programmatic`, el resultado del comando incluye este Token como `deployToken`. Ejecuta `npx sdk-commands deploy --help` para ver las rutas de la API de despliegue.

### Códigos de error de despliegue

| Código de error                   | Qué significa                                                                                                                                                       |
| --------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `DEPLOY_PARTIAL_NOT_SUPPORTED`    | El Server no admite cargas por lotes y los archivos que le faltan superan los 200 MB.                                                                               |
| `DEPLOY_CONTENT_CHECK_FAILED`     | La CLI no pudo comprobar qué archivos ya tiene el Server. Solo ejecuta esta comprobación cuando el Server no admite cargas por lotes y la escena supera los 200 MB. |
| `DEPLOY_CANCELLED`                | El despliegue se canceló y la escena no se publicó.                                                                                                                 |
| `DEPLOY_CANCELLED_AFTER_UNDEPLOY` | El despliegue se canceló después de que se eliminaron las escenas anteriores del World.                                                                             |
| `DEPLOY_CANCEL_OUTCOME_UNCERTAIN` | El despliegue se canceló, pero la CLI no pudo confirmar si el Server lo publicó.                                                                                    |
| `DEPLOY_MISSING_TARGET_CONTENT`   | La escena tiene una `worldConfiguration` pero el comando no tiene `--target-content` o `--target`.                                                                  |

## Publicar desde una hardware wallet

En lugar de almacenar tus Tokens LAND en una cuenta de Metamask, puede resultarte más seguro almacenarlos en un dispositivo de hardware wallet, como un [Ledger](https://www.ledger.com/) o un [Trezor](https://trezor.io/), conectado físicamente a tu computadora.

Si usas uno de estos dispositivos, puedes vincular la hardware wallet a Metamask para habilitar la firma de mensajes y mantener los Tokens más seguros. Consulta [este artículo de Metamask](https://metamask.zendesk.com/hc/en-us/articles/360020394612-How-to-connect-a-Trezor-or-Ledger-Hardware-Wallet) para obtener instrucciones para conectar tu cuenta.

Una vez que puedas usar tu hardware wallet mediante Metamask, puedes desplegar siguiendo los mismos pasos que si tus Tokens estuvieran en una cuenta de Metamask.

## Sobrescritura de escenas

Cuando se despliega una nueva escena, sobrescribe el contenido anterior que existía en las parcelas que ocupa.

Si una escena que ocupa varias parcelas es sobrescrita solo parcialmente por otra, todas sus parcelas se sobrescriben o se eliminan.

Supón que desplegaste tu escena *A* en dos parcelas *\[100, 100]* y *\[100, 101]*. Luego vendes la parcela *\[100, 101]* a un usuario que posee terreno adyacente y que despliega una escena grande (*B*) en varias parcelas, incluida *\[100, 101]*.

Tu escena *A* no puede renderizarse parcialmente en una sola parcela, por lo que *\[100, 100]* no mostrará ningún contenido. Debes crear una nueva versión de la escena *A* que ocupe solo una parcela y desplegarla únicamente en la parcela *\[100, 100]*.

## Qué son los Servers de contenido

Los Servers de contenido son una red de Servers propiedad de la comunidad con un sistema de archivos direccionado por contenido, lo que significa que cada archivo se identifica por su contenido, no por un nombre de archivo arbitrario.

Usamos los Servers de contenido para alojar y distribuir todo el contenido de las escenas de una forma similar a BitTorrent, manteniendo distribuida la red de Decentraland.

1. Los Servers de contenido almacenan y distribuyen todos los assets necesarios para renderizar tus escenas.
2. El `npm run deploy` El comando vincula estos assets a la parcela LAND especificada en tu **scene.json** archivo. Cada vez que vuelves a desplegar tu escena, los Servers de contenido actualizan sus registros para dirigir las parcelas al contenido más reciente. El despliegue no implica ninguna transacción de Blockchain; solo firmas la nueva Entity de la escena con tu Wallet para demostrar la propiedad.

La información en cada copia del Server es verificable, ya que cada escena está firmada por el hash del propietario de LAND. Esto significa que alguien que aloje una copia del Server no podrá manipular el contenido para mostrar algo ilegítimo. La comunidad también puede votar para aprobar o eliminar cualquiera de estos Servers mediante la DAO.

## Servidores personalizados

Puedes desplegar contenido en un Server personalizado que no pertenezca a la red oficial de Servers de Catalyst mantenida por la DAO. Para ello, no necesitas poseer ningún Token LAND o NAME, ya que puedes configurar el Server para usar cualquier lógica de validación que prefieras para controlar quién puede desplegar dónde. Los Servers personalizados pueden elegir incluir contenido de los Servers oficiales (que puedes sobrescribir) o empezar desde cero y publicar contenido completamente nuevo.

Consulta el [repositorio catalyst-owner](https://github.com/decentraland/catalyst-owner) para obtener más información sobre lo que puedes hacer con tu propio Server y cómo configurarlo.


> [!WARNING]
> **📔 Nota**: Los jugadores deberán escribir manualmente una URL para acceder a tu Server personalizado. Ciertas validaciones de servicios como el [Server de recompensas](/docs/creator/content-creator-es/recompensas/getting-started/) podrían fallar en estos contextos, ya que a menudo estos servicios requieren que la Request provenga de un Server oficial.


Los jugadores nunca son dirigidos a este Server; la única forma de acceder a él es escribir explícitamente la URL para conectarse.

### Mediante la CLI

Para desplegar en un Server personalizado, ejecuta:

`npm run deploy -- --target <CUSTOM SERVER DOMAIN>`

La URL para entrar a tu escena desplegada en tu Server personalizado dependerá del dominio en el que esté alojada.

## Verificar que el despliegue se realizó correctamente

Una vez que hayas desplegado tu escena, estos cambios pueden tardar unos minutos en propagarse por los diversos Servers de contenido de la red. Si entras a Decentraland justo después de desplegar, es posible que aún veas brevemente la versión anterior de tu contenido.

Si usas la CLI para desplegar, la dapp de firma comenzará a mostrar confirmaciones de que la nueva versión de tu contenido se ha propagado por todos los Servers de la red.

Verás una lista de cada uno de los Servers que conforman la red de contenido de Decentraland. Para cada Server, se especifica la marca de tiempo del último cambio cargado en esa parcela. Cada Server se refiere a un Realm diferente. Puedes ver cómo estos nombres de Server se asignan a nombres de Realm en la [pantalla de monitor de Catalyst](https://decentraland.github.io/catalyst-monitor/).

La propagación es solo una parte del proceso: después de la carga, los modelos 3D de la escena se convierten en paquetes de assets optimizados para cada plataforma (Windows y Mac). Hasta que termine esa conversión, los jugadores reciben deliberadamente la última versión completamente funcional de tu escena. Esto suele tardar solo unos segundos, pero puede tardar más en escenas muy grandes o cuando los Servers están ocupados. Puedes consultar directamente el progreso de la conversión; consulta [Comprueba el estado de la conversión](/docs/creator/content-creator-es/scene-editor/publicar/publish-scene/#check-the-conversion-status).

También puedes obtener la información de propagación en cualquier momento ejecutando el siguiente comando en la consola de línea de comandos:

`npx @dcl/opscli pointer-consistency --pointer 0,0`


> [!WARNING]
> **📔 Nota**: Usa las coordenadas de tu escena en lugar de `0,0`. Si tu escena tiene varios parcels, cualquiera de sus parcels producirá la misma salida. Si las coordenadas comienzan con un número negativo, añade una `\` al comienzo de las coordenadas para evitar que el `-` carácter sea interpretado erróneamente por la línea de comandos.


## Despliegues automáticos

Si realizas cambios con regularidad en el contenido de tu escena y quieres implementar un flujo de publicación optimizado, puedes automatizar el despliegue de tu escena mediante una GitHub action.

Para ello, primero debes almacenar tu escena en un proyecto de GitHub. Luego puedes configurar una [GitHub action](https://docs.github.com/en/actions) con el siguiente script, que se ejecuta cada vez que se realiza un merge en la `main` rama. El script instala cualquier dependencia, compila el proyecto y luego lo despliega en Decentraland.

```yaml
name: Deploy to DCL PROD

on:
  push:
    branches:
      - main

env:
  DCL_PRIVATE_KEY: ${{ secrets.DCL_PRIVATE_KEY }}

jobs:
  deploy:
    name: Deploy
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - name: Instalar paquetes npm
        run: |
          npm install
      - name: Compilar escena
        run: |
          npm run build
      - name: Desplegar escena
        run: |
          npm run deploy
```

> Importante: Para que este proceso se ejecute, debes establecer la clave privada de una wallet como variable de entorno en GitHub, que se usa para firmar el despliegue. Como siempre, ten mucho cuidado de mantener las claves privadas seguras. NO uses la clave privada de la cuenta que realmente posee los tokens de LAND, ya que eso supondría riesgos de seguridad importantes. En su lugar, delega los permisos de operador a una wallet desechable que no posea tokens valiosos. Si esta clave privada llegara a filtrarse, puedes revocar fácilmente esos permisos de operador de la cuenta y configurar una nueva wallet.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/publicacion/publishing.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
