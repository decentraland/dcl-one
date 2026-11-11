> Cómo usa Decentraland la blockchain de Ethereum.

# Acerca de la Blockchain

Todas las blockchains son, en esencia, bases de datos descentralizadas que se distribuyen entre las máquinas de una red. Las transacciones se agrupan en "bloques" y se procesan secuencialmente para formar una *cadena* de eventos.

Ethereum es una de las blockchains más populares. Lo que la distingue de otras, como Bitcoin, es que usa la blockchain como almacenamiento para algo más que un registro de transacciones de moneda. Ethereum puede almacenar información más compleja para distinguir distintos tipos de tokens o incluso manejar tokens únicos con características específicas. La blockchain de Ethereum también ejecuta Smart Contracts, estos permiten ejecutar transacciones más complejas que también pueden depender de eventos acordados.

Decentraland usa la blockchain de Ethereum para registrar la propiedad de los activos digitales y otros elementos comerciables que una escena 3D puede leer y a los que puede reaccionar.

La blockchain no se usa para almacenar el estado de la escena, la posición del jugador ni nada que necesite cambiar en tiempo real mientras un jugador interactúa con una escena; todo eso se almacena localmente en la máquina de cada jugador, o en un servidor privado propiedad del dueño de la escena. Los desarrolladores de cada escena deben elegir qué información vale la pena almacenar en la blockchain y qué almacenar en un servidor privado.

## Wallets

Los tokens de Ethereum se guardan en Wallets. Un Wallet de Ethereum puede contener varios tokens, incluidos Ether, MANA, LAND y otros tokens que pueden ser usados por juegos o experiencias en Decentraland.

Hay muchos proveedores de Wallet donde puedes guardar tokens de Decentraland. Para usar el Marketplace, o para entrar en Decentraland, debes usar un Wallet integrado con tu navegador web, así que recomendamos que uses:

* [Metamask](https://metamask.io/)
* [Trezor](https://trezor.io/)/[Ledger](https://www.ledger.com/) wallets de hardware

Cada wallet tiene una clave pública y una privada. El hash de tu clave pública es la dirección única de tu wallet, usada para enrutar transacciones e identificar a un jugador. Tu clave privada es usada por tu wallet para firmar cada transacción que envías a la red y certificar que realmente fue enviada por ti. Tu clave privada también se usa para restaurar tu wallet en caso de que olvides tu contraseña, así que guárdala en un lugar seguro y no la compartas con nadie.

En Decentraland, las identidades de los jugadores se construyen alrededor de wallets. Dado que las claves públicas de las wallets son únicas, tu escena puede usarlas para identificar a un usuario de Decentraland de forma persistente. Las wallets también pueden contener diferentes tokens que pueden darle a un jugador un avatar único, un wearable, permisos para entrar en escenas que eligen restringir el acceso, un arma especial para usar en un juego, etc.

## Transactions

Las transacciones realizan cambios en la información que se almacena en la blockchain. Las transacciones típicas implican que los tokens cambian de dueño; por ejemplo, que el usuario A le entregue su token LAND al usuario B a cambio de una cantidad de tokens MANA. En la red de Ethereum, sin embargo, una transacción también puede significar cambiar la información que se almacena sobre un token sin cambiar su dueño. Por ejemplo, cambiar la descripción de una parcela, o fusionar varias parcelas en un Estate.

Todas las transacciones que ocurren en la cadena principal de Ethereum tienen un costo que se paga en tokens Ether. Esta tarifa se denomina tarifa de gas, y se paga al usuario de la red que "mina" la transacción.

Cuando solicitas que ocurra una transacción, estableces el precio de gas que estás dispuesto a pagar para que la transacción sea minada. Las transacciones que ofrecen precios más altos se minan más rápido, ya que los mineros les dan prioridad. Los precios de mercado de estas transacciones oscilan regularmente; tienden a ser más caros cuando hay un mayor uso de la red. Asegúrate de que lo que ofreces no esté por debajo del precio de mercado, de lo contrario tu transacción podría permanecer indefinidamente en un pool sin procesar.

Todas las transacciones deben ser firmadas por una dirección de Ethereum, usando la clave privada de la dirección. Esto certifica que la transacción fue realizada por esa dirección.

#### Validación de transacciones

Las transacciones de blockchain no son inmediatas, requieren tiempo para ser "minadas" por uno de los nodos de la red, y luego para propagarse por el resto de las máquinas. Cuantas más transacciones solicite la red, más tiempo tardan en validarse.

En términos breves, así es como se valida una transacción:

1. Ocurre una nueva transacción, y pasa a un pool de transacciones no confirmadas.
2. Una de las máquinas de la red resuelve con éxito un algoritmo para minar un nuevo "bloque" que contiene un puñado de transacciones de este pool, incluida esta. Adjunta este nuevo bloque al final de la cadena.
3. El bloque se comparte con otras máquinas de la red. Cada máquina verifica que cada transacción del bloque sea válida y comprueba el hash del bloque para asegurarse de que es legítimo; luego lo añade a su propia versión de la cadena.
4. El nuevo bloque se propaga por toda la red. Hay una comprensión compartida universalmente de que esta transacción ha ocurrido.

#### Sidechains

Decentraland está asociándose con [Matic](https://matic.network/) para crear una *sidechain* (un tipo especial de blockchain) que podrá manejar transacciones más rápido y a menor costo que la red principal de Ethereum. Esta sidechain será ideal para transacciones dentro del juego, ya que los cambios pueden ocurrir más cerca del tiempo real y a un costo muy bajo. Para transacciones que impliquen objetos valiosos, seguiremos recomendando la cadena principal de Ethereum, ya que será más segura.

Cada desarrollador que trabaje en una escena podrá elegir si usar la cadena principal, la sidechain o una combinación de ambas para distintas transacciones.

La sidechain se mantendrá interoperable con la cadena principal de Ethereum. Podrás cargar tokens desde la cadena principal a la sidechain y viceversa. Las transacciones que tienen lugar en la sidechain finalmente se reflejan en la cadena principal cuando los tokens "salen" de vuelta a la cadena principal.

#### Activar transacciones desde una escena

El código de tu escena puede activar transacciones, tanto en la cadena principal de Ethereum como en la sidechain de Decentraland. Podrías tener una tienda en tu escena que venda tokens (como NFTs), o tener un juego que otorgue objetos del juego a los jugadores que alcancen ciertas metas.

El usuario siempre debe aprobar explícitamente estas transacciones en su cliente de Ethereum. Por ejemplo, al usar Metamask, Metamask le pide al usuario que acepte cada transacción antes de que se procese.

## Tipos de tokens

Se pueden manejar diferentes tipos de tokens en la red de Ethereum. Han surgido algunos estándares que agrupan tokens que comparten las mismas características.

En Decentraland, puedes usar tokens para representar objetos relacionados con tu juego o experiencia, como un arma o un trofeo. Como los tokens se guardan en la wallet de un jugador, acompañan al jugador de una escena a otra, por lo que cada escena puede elegir si quiere reaccionar y cómo a cada tipo de token existente.

Leer [Qué son los NFTs](https://decentraland.org/blog/technology/what-are-nfts/) en nuestro blog para una mirada más profunda al surgimiento y la evolución de los tokens no fungibles.

#### Tokens fungibles

Si un objeto es fungible, entonces puede ser sustituido o intercambiado por cualquier objeto similar. Las monedas fiat, como el dólar estadounidense, son fungibles. Un billete de un dólar puede intercambiarse por cualquier otro billete de un dólar.

Los tokens de criptomonedas como Bitcoin, Ethereum y MANA son todos fungibles porque una unidad de token puede intercambiarse por cualquier otra unidad de token.

También podrías crear tokens fungibles personalizados para usarlos en escenas de Decentraland y utilizarlos para representar objetos que son todos iguales y no tienen propiedades distintivas ni personalizables entre sí. Podrías, por ejemplo, crear un juego que gire en torno a recolectar una gran cantidad de objetos idénticos y representarlos mediante un token fungible. También podrías usar un token fungible para representar un boleto dorado que dé a los jugadores que lo posean acceso a una región o servicio específico.

*ERC20* es el estándar más aceptado para los tokens fungibles en la red de Ethereum. MANA se basa en este estándar.

#### Tokens no fungibles

Los tokens no fungibles (o NFTs) tienen características que hacen que cada unidad sea objetivamente diferente de las demás. Las parcelas de LAND en Decentraland son NFTs, ya que la ubicación de cada parcela es única. La adyacencia a otras parcelas, carreteras o distritos hace que estas ubicaciones sean relevantes para los propietarios del token.

En Decentraland, puedes usar NFTs para representar objetos dentro del juego, como avatares, wearables, armas y otros elementos del inventario. Podrías, por ejemplo, usar un solo tipo de NFT para representar todas las armas de tu juego y diferenciarlas estableciendo diferentes propiedades en estos NFTs.

Los NFTs se pueden usar para proporcionar bienes digitales verificablemente escasos. Gracias a la escasez legítima que hace posible la blockchain, los compradores pueden estar seguros de que el arte que adquieren es, de hecho, raro. Esto le da al arte digital un valor real que nunca habíamos visto antes.

Los objetos del juego tendrán un historial que se almacena en la blockchain. Este historial podría hacer que un objeto tenga más valor, por ejemplo si se usó para lograr grandes hazañas o fue usado por alguien admirado.

Dependiendo del contrato que describa el token, cada NFT podría ser inmutable, o podrías permitir que los jugadores personalicen y cambien ciertas características si así lo desean.

*ERC721* es el estándar más aceptado para los tokens no fungibles en la red de Ethereum. Los tokens LAND siguen el estándar ERC721.

## Smart Contracts

Un contrato consta tanto de código (sus métodos) como de datos (su estado) que residen en una dirección específica en la blockchain de Ethereum.

Los métodos de un contrato siempre se llaman mediante una transacción que tiene el *to* campo establecido en la dirección del contrato. El código que ejecuta el método del contrato puede incluir llamadas a otros contratos; estas activan más transacciones que tienen el *from* campo establecido en la dirección del contrato.

Un contrato no puede activar ninguna acción por sí solo ni basarse en un evento temporal. Todas las acciones que realiza un smart contract siempre surgen de una transacción que llama a una de las funciones del contrato.

Puedes usar smart contracts para condicionar transacciones según condiciones personalizadas. Por ejemplo, los jugadores podrían apostar al resultado de un juego, y los pagos correspondientes ocurrirían en cuanto el resultado del juego se le informe al contrato.

El código completo de un smart contract es público para quien quiera leerlo. Esto permite a los desarrolladores crear reglas verificables públicamente.

Todos los Tokens están definidos por un smart contract que especifica sus características y lo que se puede hacer con ellos. Decentraland ha escrito y mantiene varios smart contracts. Los propios tokens LAND y MANA están definidos por los *LANDregistry* y *MANAtoken* contratos, respectivamente.

Puedes encontrar la dirección de cada contrato creado por Decentraland en [Smart contracts de Decentraland](https://contracts.decentraland.org/addresses.json).

Puedes leer el código completo de cada uno de esos contratos, ya que es información pública en la blockchain. Puedes encontrar el contrato por nombre en [Etherscan](https://etherscan.io/contractsVerified) y leer su contenido allí.

## dApps

*dApps* (aplicaciones descentralizadas) son aplicaciones que se construyen sobre smart contracts y la blockchain.

Una dApp puede ser tan simple como algo que valida que tu wallet tenga cierto token y te permita usar un servicio. O puede ser una aplicación totalmente desarrollada con su propia UI, como el Decentraland Marketplace.

## Red de pruebas de Sepolia

Antes de desplegar un smart contract, crear un nuevo tipo de token o una escena de Decentraland que dependa de transacciones en la red de Ethereum, necesitas asegurarte de que no tenga errores ni fallos que usuarios maliciosos puedan explotar.

La red de pruebas de Sepolia es una versión alternativa de Ethereum hecha específicamente para ejecutar pruebas.

Los tokens en la red de Sepolia no tienen valor real, así que puedes permitirte cometer errores sin correr ningún riesgo real. Puedes reponer cualquier token perdido gratis usando un faucet:

* faucet de Ether de Sepolia (<https://www.alchemy.com/faucets/ethereum-sepolia/>)

Si estás desarrollando una escena que activa transacciones, probar estas transacciones en la red de Sepolia es gratis, ya que los tokens que envías no tienen valor. En mainnet, de lo contrario tendrías que pagar al menos una tarifa real de gas en Ether por cada transacción de prueba que realices.

Una vez que estés seguro de que tu código funciona como se espera y no puede ser explotado, puedes desplegar en la mainnet de Ethereum.

## Reorgs de blockchain

De vez en cuando, varias máquinas crearán nuevos bloques alternativos aproximadamente al mismo tiempo. Esto es un problema, porque bifurca la cadena en dos versiones divergentes que potencialmente podrían contradecirse entre sí. Cuando ocurre una bifurcación, Ethereum lo resuelve dando siempre prioridad a la cadena más larga y descartando cualquier cadena más corta. Aunque es posible que dos cadenas rivales existan al mismo tiempo, pronto una de las dos añadirá otro bloque y superará a la otra. Debido al tiempo que lleva resolver los algoritmos de minado, cada vez es más difícil que las cadenas rivales sigan creciendo en perfecta sincronía entre sí. Tarde o temprano, una prevalecerá sobre la otra.

Cuando una cadena supera a la otra y la disputa se resuelve, las máquinas que habían adoptado la cadena más corta necesitan hacer ajustes. Esto es lo que se conoce como un "reorg". Necesitan revertir todas las transacciones incluidas en los bloques de la rama en la que están hasta llegar al punto en que ocurrió la bifurcación. Luego necesitan agregar los nuevos bloques de la rama más larga que se considera legítima.

Las transacciones revertidas pueden volver al pool de transacciones pendientes hasta que un minero las retome (o se descarten). Cualquier tarifa de gas pagada por estas transacciones también se revierte.

Los bloques que acaban de añadirse al final de una cadena tienen una probabilidad considerable de revertirse debido a los mecanismos explicados arriba. A medida que se añaden bloques posteriores al final de la cadena, cada vez es menos probable que los bloques que están más atrás en la blockchain puedan revertirse, porque eso requeriría un reorg mayor. Por ello, cada nuevo bloque que se añade al final de la cadena después de una transacción se denomina una confirmación para esa transacción.

Al crear aplicaciones (o escenas) que usan información de fuera de la blockchain, debes tener en cuenta la ocurrencia de reorgs. Tal vez quieras considerar que las transacciones solo están verificadas cuando ha ocurrido un cierto número de confirmaciones y la transacción ya no está al final de la cadena.

Usar varias confirmaciones hará que la información sea muy estable, pero las transacciones tardarán mucho en reflejarse.

Usar pocas confirmaciones hará que los cambios se reflejen más rápido, pero a veces habrá contratiempos que parezcan deshacer transacciones cuando ocurran reorgs. Si estas transacciones tienen consecuencias fuera de la blockchain en tu escena, entonces puede que necesites revertir de algún modo esas consecuencias también.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/player/player-es/integracion-con-blockchain/ethereum-essentials.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
