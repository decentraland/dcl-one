> Proporciona a los usuarios transacciones sin coste

# Desplegando tu servidor de transacciones

El [transactions-server](https://github.com/decentraland/transactions-server/tree/1.8.0) es un proxy server que reenvía transacciones a [Gelato](https://www.gelato.network/relay). Recibe una transacción firmada del client y, a su vez, la envía a la red apropiada detrás de escena. Esto permite al dueño del server facilitar a sus usuarios transacciones sin costo

El transaction server se usa para ayudar con la UX de usar múltiples networks y para evitar que cambien de network provider sobre la marcha. Los usuarios pueden permanecer conectados a [Ethereum](https://ethereum.org/en/) y interactuar con [Polygon](https://polygon.technology/) [firmando solo transacciones](https://docs.decentraland.org/blockchain-integration/transactions-in-polygon/)

La Decentraland DAO ha configurado un server utilizado por nuestras dapps, cubriendo el costo hasta un cierto límite con unas pocas [restricciones](#restrictions). Este documento explica cómo puedes [desplegar este server](#running-the-server) para permitir a tus usuarios reenviar transacciones con las restricciones que necesites, si las hay.

## Restricciones

Todas las restricciones son por cada transacción que el usuario intenta enviar. Lo que, en la práctica, se traduce en una solicitud POST al server.

Las restricciones configurables que tiene el server son:

* Comprueba una cuota de máximo de transacciones por día. Consulta [la sección de transacciones](#transactions) para más información.
* Comprueba contratos en la whitelist, así que si una transacción intenta interactuar con un contrato que no está reconocido, fallará. Para ello usa
  * Los contratos y colecciones desplegados. Consulta [la sección de contratos y colecciones](#contracts-and-collections) para más información
* El precio de las ventas, restringiéndolo si está por debajo de un umbral. Consulta [la sección de valor mínimo de venta](#min-sale-value) para más información

## Configurando Gelato

[Gelato](https://www.gelato.network/relay) es un Multichain Relayer Protocol. Usamos su infraestructura para habilitar transacciones sin costo. Esto significa, en la práctica, que cuando vas a enviar una transacción, en su lugar estás firmando un mensaje y enviándolo a Gelato. El servicio se encargará de enviar la transacción por ti y devolverte una respuesta (hash de transacción) de vuelta.

Necesita un contrato para reenviar las transacciones, pero por suerte podemos reutilizar el que está siendo usado por Decentraland (ver abajo)

[Gelato](https://www.gelato.network/relay) funciona como una API para el server. [Para configurarlo](#configuring-the-server) primero necesitarás una API KEY que [usaremos más adelante](#gelato). Para obtenerlas:

* [Regístrate](https://app.gelato.network/relay) en el servicio
* Crea una nueva dapp para la red a la que pretendes apuntar. Para imitar la configuración de Decentraland:
  * Selecciona las `Mainnets` opciones
  * Establece un `nombre de app` apropiado para tu dApp
  * elige `Polygon` como la red para el `Smart Contract`
  * Habilita la `Any Contract` opción de alternancia
* Copia la API KEY de la `sección API Key` sección

Por último, tendrás que financiar tu dapp recién creada. Puedes hacerlo conectando tu wallet en la [1Balance](https://app.gelato.network/1balance) sección de la barra lateral izquierda. Una vez conectada, te permitirá depositar tus [USDC](https://polygonscan.com/token/0x3c499c542cef5e3811e1192ce70d8cc03d5c3359) para financiar las transacciones que enviarán tus usuarios. Si necesitas obtener MATIC, revisa esta [publicación](https://docs.decentraland.org/blockchain-integration/transactions-in-polygon/#where-can-i-get-matic-to-pay-for-transaction-fees).

### Red de pruebas

Si quieres probar tu app antes de salir en vivo y estás usando Polygon puedes hacerlo en `Polygon Amoy`, la testnet de Polygon.

Para hacerlo simplemente repite [el proceso](#configuring-gelato) pero eligiendo `Matic Testnet (Amoy)` en el campo network.

Tendrás que financiar tu dapp, pero puedes hacerlo fácilmente obteniendo tokens Sepolia ETH del [faucet](https://sepoliafaucet.com/).

## Descargando el transactions server

Lo primero que necesitarás es una copia del código transactions-server de Decentraland. Puedes encontrarlo [en GitHub](https://github.com/decentraland/transactions-server/tree/v1). Desde ahí, tienes dos opciones:

1. **Descargando el código**: Para descargar el código, primero tienes que hacer clic en el botón verde `Code` y luego cualquiera de las dos opciones

* Haz clic en `Download ZIP`
* Copia la URL debajo del título `Clone` y luego ejecuta `$ git clone THE_URL_HERE`

2. **Haciendo fork del código**: Puedes hacer clic en el `fork` botón en la parte superior derecha de la página. Una vez que el proceso esté completo, podrás descargar tu código de la misma manera que lo harías en la primera opción. Necesitarás una cuenta de GitHub para esto; para más información sobre cómo hacer fork de repositorios, consulta [aquí](https://docs.github.com/en/enterprise-server@3.5/get-started/quickstart/fork-a-repo)

## Configurando el server

El transactions server está escrito en [NodeJS](https://nodejs.org/en/) usando [Typescript](https://www.typescriptlang.org/). Antes de ejecutarlo, tendrás que configurar algunas variables de entorno.

Para hacer esto:

* Copia el `.env.defaults` archivo y pégalo renombrado como `.env`
* Abre el `.env` archivo. Verás que algunas variables tienen un valor predeterminado, como `HTTP_SERVER_PORT=5000` (en qué puerto ejecutar el server)
* Puedes dejar la mayoría de los valores como están, pero hay algunos valores importantes a tener en cuenta:
  * [Gelato](#gelato)
  * [Transacciones](#transactions)
  * [Contratos y colecciones](#contracts-and-collections)
  * [Valor mínimo de venta](#min-sale-value)

### Gelato

Usa la API KEY que obtuvimos cuando [configurando Gelato](#configuring-gelato).

```
GELATO_API_KEY=<YOUR_GELATO_API_KEY>
```

### Transacciones

Cuando llegue una nueva solicitud de transacción, comprobará la cantidad **una dirección** ha enviado ese día. Si supera el valor establecido, la transacción fallará.

```
MAX_TRANSACTIONS_PER_DAY=10
```

Para eliminar completamente esta comprobación, puedes ir al código y eliminar el

```ts
await checkQuota(components, transactionData)
```

de `async function checkData(transactionData: TransactionData): Promise<void> {` en `src/ports/transaction/component.ts`

### Contratos y colecciones

El server obtendrá la URL de las direcciones de contratos y las almacenará localmente, y consultará el subgraph. Cuando llegue una nueva solicitud de transacción, comprobará entonces si el contrato con el que interactúa la transacción pertenece a los contratos desplegados en la URL o a las colecciones desplegadas en el subgraph.

Si quieres proporcionar tus propios contratos, cambia la URL y mantén la misma estructura que la actual <https://contracts.decentraland.org/addresses.json> tiene. La red usada se determina por COLLECTIONS\_CHAIN\_ID, y el intervalo con el que se vuelve a obtener la caché es COLLECTIONS\_FETCH\_INTERVAL\_MS

Si tienes tus propias colecciones, también puedes cambiar la URL del subgraph.

Para eliminar completamente estas comprobaciones, puedes ir al código y eliminar el

```ts
await checkContractAddress(components, transactionData)
```

de `async function checkData(transactionData: TransactionData): Promise<void> {` en `src/ports/transaction/component.ts`

```
CONTRACT_ADDRESSES_URL=https://contracts.decentraland.org/addresses.json
COLLECTIONS_FETCH_INTERVAL_MS=3600000
COLLECTIONS_CHAIN_ID=80002

COLLECTIONS_SUBGRAPH_URL=https://subgraph.decentraland.org/decentraland/collections-matic-amoy
```

### Valor mínimo de venta

Cuando llegue una nueva solicitud de transacción, primero analizará los datos que intenta reenviar. Si detecta una venta (compra en Marketplace, oferta, etc.), comprobará el valor frente a MIN\_SALE\_VALUE\_IN\_WEI. Si es inferior, la transacción fallará.

```
MIN_SALE_VALUE_IN_WEI=1000000000000000000
```

Para comprobar los métodos de venta relevantes, puedes ver `src/ports/transaction/validation/checkSalePrice.ts` y para eliminar completamente esta comprobación, puedes ir al código y eliminar el

```ts
await checkSalePrice(components, transactionData)
```

de `async function checkData(transactionData: TransactionData): Promise<void> {` en `src/ports/transaction/component.ts`

## Ejecutando el server

Ahora que toda la configuración está hecha, lo que queda es ejecutar realmente el server. Puedes seguir [su README](/docs/creator/content-creator-es/scenes-sdk7/blockchain/deploying-your-own-transactions-server/) pero, en resumen, tendrás que:

* Tener [NodeJS](https://nodejs.org/en/) instalado
* Abre la terminal de tu preferencia
* Ejecuta los siguientes comandos:

```bash
npm install
npm run migrate # solo la primera vez
npm run start
```

Por supuesto, probablemente querrás desplegar esto en el servicio de tu elección, como [AWS](https://aws.amazon.com/) por ejemplo. Puedes usar el [Dockerfile](https://github.com/decentraland/transactions-server/blob/master/Dockerfile) del proyecto para hacerlo.

## Usando el server

Ahora que todo está configurado y en ejecución, es hora de usar realmente el server.

Para enviar realmente una transacción, necesitas hacer un POST a `/transactions`. El esquema requerido para la solicitud está definido por `transactionSchema` en `src/ports/transaction/types.ts`.

Si, en cambio, quieres usar nuestras librerías prehechas para facilitarte la vida, puedes probar [decentraland-transactions](https://github.com/decentraland/decentraland-transactions). Se usa a través de [decentraland-dapps](https://github.com/decentraland/decentraland-dapps) en nuestras dapps como el [Marketplace](https://market.decentraland.org), con las utils [`sendTransaction`](https://github.com/decentraland/decentraland-dapps/blob/master/src/modules/wallet/utils.ts#L104). Consulta [este código](https://github.com/decentraland/marketplace/blob/a2191515c6ae7ede54a685cc2dd9f9fafa35366b/webapp/src/modules/vendor/decentraland/OrderService.ts#L33) como ejemplo.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/blockchain/deploying-your-own-transactions-server.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
