> Aprende qué ofrece el SDK para realizar operaciones con la blockchain de Ethereum

# Operaciones Blockchain de la escena

Una escena de Decentraland puede interactuar con la blockchain de Ethereum. Esto puede servir para obtener datos sobre la wallet del usuario y los tokens que contiene, o para desencadenar transacciones que podrían involucrar cualquier token de Ethereum, fungible o no fungible. Esto se puede usar de muchas maneras; por ejemplo, para vender tokens, para recompensar tokens como parte de una mecánica de juego, para cambiar cómo un jugador interactúa con una escena si posee ciertos tokens, etc.

Ten en cuenta que todas las transacciones en la red principal de Ethereum que sean desencadenadas por una escena requerirán que un jugador las apruebe y pague una gas fee.

Todas las operaciones de blockchain también deben ejecutarse como [funciones asíncronas](/docs/creator/content-creator-es/scenes-sdk7/patrones-de-programacion/async-functions/), ya que el tiempo depende de eventos externos.

## Obtener la cuenta Ethereum de un jugador

Para obtener la cuenta Ethereum de un jugador, usa la `getPlayer()` función.

```ts
import { getPlayer } from "@dcl/sdk/src/players";

export function main() {
  let userData = getPlayer();
  if (!userData) return;
  if (!userData.isGuest) {
    console.log(userData.userId);
  } else {
    console.log("El jugador no está conectado con Web3");
  }
}
```

Ten en cuenta que si un jugador ha entrado en Decentraland como invitado, no tendrá una wallet Ethereum conectada. Si está conectado como invitado, el `isGuest` campo en la respuesta de `getPlayer()` será true. De lo contrario, puedes obtener la dirección de la wallet del jugador desde el campo `userId` . Aprende más sobre los datos que puedes obtener de un jugador en [obtener datos del jugador](/docs/creator/content-creator-es/scenes-sdk7/interactividad/user-data/#get-player-data)


> [!WARNING]
> **📔 Nota**: Aunque la dirección eth puede contener caracteres en mayúscula, algunos navegadores convierten automáticamente la cadena devuelta a minúsculas. Si deseas comparar valores de direcciones y que funcione en todos los navegadores, usa el `.toLowerCase()` método para convertir el valor a minúsculas.


## Comprobar el precio del gas

Después de importar la biblioteca `eth-connect` , debes instanciar un proveedor web3 y un request manager, lo que te permitirá conectarte vía web3 a Metamask en el navegador del jugador.

La función de abajo obtiene el precio actual del gas en la red principal de Ethereum y lo imprime.

```ts
import { RequestManager } from "eth-connect";
import { createEthereumProvider } from "@dcl/sdk/ethereum-provider";

executeTask(async function () {
  // crear una instancia del proveedor web3 para interactuar con Metamask
  const provider = createEthereumProvider();
  // Crear el objeto que se encargará de enviar y recibir mensajes RPC
  const requestManager = new RequestManager(provider);
  // Comprobar el precio actual del gas en la red Ethereum
  const gasPrice = await requestManager.eth_gasPrice();
  // registrar la respuesta
  console.log({ gasPrice });
});
```


> [!NOTE]
> **💡 Consejo**: Ten en cuenta que las funciones manejadas por el `requestManager` deben llamarse usando `await`, ya que dependen de obtener datos externos y pueden tardar algo de tiempo en completarse.


## Importar un ABI de contrato

Un ABI (Application Binary Interface) describe cómo interactuar con un contrato Ethereum, determinando qué funciones están disponibles, qué entradas toman y qué devuelven. Cada contrato Ethereum tiene su propio ABI; debes importar los ABI de todos los contratos que quieras usar en tu proyecto.

Por ejemplo, aquí tienes un ejemplo de una función en el ABI de MANA:

```ts
{
  anonymous: false,
  inputs: [
    {
      indexed: true,
      name: 'burner',
      type: 'address'
    },
    {
      indexed: false,
      name: 'value',
      type: 'uint256'
    }
  ],
  name: 'Burn',
  type: 'event'
}
```

Las definiciones de ABI pueden ser bastante largas, ya que a menudo incluyen muchas funciones, por lo que recomendamos pegar el contenido JSON de un archivo ABI en un `.ts` separado e importarlo desde allí en otros archivos de la escena. También recomendamos guardar todos los archivos ABI en una carpeta separada de tu escena, llamada `/contracts`.

```ts
import { abi } from "../contracts/mana";
```

Aquí tienes enlaces a diferentes contratos de Decentraland. Obtén el ABI de cada contrato haciendo clic en *Export ABI* y eligiendo *JSON Format*.

* [ABI del token MANA](https://etherscan.io/address/0x0f5d2fb29fb7d3cfee444a200298f468908cc942#code)
* [Marketplace de Decentraland](https://etherscan.io/address/0x19a8ed4860007a66805782ed7e0bed4e44fc6717#code)
* [ABI de LAND](https://etherscan.io/address/0xf87e31492faf9a91b02ee0deaad50d51d56d5d4d#code)
* [ABI de Estate](https://etherscan.io/address/0x959e104e1a4db6317fa58f8295f586e1a978c297#code)
* [ABI de AvatarNameRegistry](https://etherscan.io/address/0x894b883905bfEe2CC448880F1b59f4A762E67566)
* [ABI de Catalyst](https://etherscan.io/address/0xcc054fab08127c19f621ab83ade5962cd10584ec)

Estos son los contratos de las distintas colecciones de Wearables: (cada colección se emitió como un contrato separado)

* [ABI de ExclusiveMasksCollection](https://etherscan.io/address/0xc04528c14c8ffd84c7c1fb6719b4a89853035cdd)
* [ABI de Halloween2019Collection](https://etherscan.io/address/0xc1f4b0eea2bd6690930e6c66efd3e197d620b9c2)
* [ABI de Halloween2019CollectionFactory](https://etherscan.io/address/0x07ccfd0fbada4ac3c22ecd38037ca5e5c0ad8cfa)
* [ABI de Xmas2019Collection](https://etherscan.io/address/0xc3af02c0fd486c8e9da5788b915d6fff3f049866)
* [ABI de MCHCollection](https://etherscan.io/address/0xf64dc33a192e056bb5f0e5049356a0498b502d50)
* [ABI de CommunityContestCollection](https://etherscan.io/address/0x32b7495895264ac9d0b12d32afd435453458b1c6)
* [ABI de DCLLaunchCollection](https://etherscan.io/address/0xd35147be6401dcb20811f2104c33de8e97ed6818)
* [ABI de DCGCollection](https://etherscan.io/address/0x3163d2cfee3183f9874e2869942cc62649eeb004)


> [!NOTE]
> **💡 Consejo**: Para ver claramente las funciones expuestas por un contrato, ábrelo en [abitopic.io](https://abitopic.io). Solo pega allí la dirección del contrato y abre la pestaña *functions* para ver la lista completa de funciones compatibles y sus argumentos. Incluso puedes probar a llamar a las funciones con diferentes parámetros a través de la página web.


Configurar TypeScript para poder importar desde un archivo JSON tiene sus dificultades. La solución alternativa más fácil recomendada es cambiar la extensión del archivo `ABI.JSON` a `.ts` y modificarlo ligeramente para que su contenido empiece con `export default`.

Por ejemplo, si el contenido del archivo ABI empieza con `[{"constant":true,"inputs":[{"internalType":"bytes4" ...etc`, modifícalo para que empiece con `export default [{"constant":true,"inputs":[{"internalType":"bytes4" ...etc`.

### Instanciar un contrato

Después de importar la biblioteca `eth-connect` biblioteca y un *abi*, debes instanciar varios objetos que te permitirán usar las funciones del contrato y conectarte a Metamask en el navegador del jugador.

También debes importar el proveedor web3. Esto se debe a que Metamask en el navegador del jugador usa web3, así que necesitamos una forma de interactuar con eso.

```ts
import { RequestManager, ContractFactory } from "eth-connect";
import { createEthereumProvider } from "@dcl/sdk/ethereum-provider";
import { abi } from "../contracts/mana";

executeTask(async () => {
  // crear una instancia del proveedor web3 para interactuar con Metamask
  const provider = createEthereumProvider();
  // Crear el objeto que se encargará de enviar y recibir mensajes RPC
  const requestManager = new RequestManager(provider);
  // Crear un objeto factory basado en el abi
  const factory = new ContractFactory(requestManager, abi);
  // Usar el objeto factory para instanciar un objeto `contract`, haciendo referencia a un contrato específico
  const contract = (await factory.at(
    "0x2a8fd99c19271f4f04b1b7b9c4f7cf264b626edb"
  )) as any;
});
```


> [!NOTE]
> **💡 Consejo**: Para contratos que siguen un mismo estándar, como ERC20 o ERC721, puedes importar un único ABI genérico para todos. Luego generas un único objeto `ContractFactory` con ese ABI y usas ese mismo factory para instanciar interfaces para cada contrato.


### Llamar a los métodos en un contrato

Una vez que has creado un objeto `contract` , puedes llamar fácilmente a las funciones definidas en su ABI, pasándole los parámetros de entrada especificados.

```ts
import { getPlayer } from "@dcl/sdk/src/players";
import { createEthereumProvider } from "@dcl/sdk/ethereum-provider";
import { RequestManager, ContractFactory } from "eth-connect";
import { abi } from "../contracts/mana";

executeTask(async () => {
  try {
    // Pasos de configuración explicados en la sección anterior
    const provider = createEthereumProvider();
    const requestManager = new RequestManager(provider);
    const factory = new ContractFactory(requestManager, abi);
    const contract = (await factory.at(
      "0x2a8fd99c19271f4f04b1b7b9c4f7cf264b626edb"
    )) as any;
    let userData = getPlayer();
    if (!userData || userData.isGuest) {
      return;
    }

    // Ejecutar una función del contrato
    const res = await contract.setBalance(
      "0xaFA48Fad27C7cAB28dC6E970E4BFda7F7c8D60Fb",
      100,
      {
        from: userData.userId,
      }
    );
    // Registrar la respuesta
    console.log(res);
  } catch (error: any) {
    console.log(error.toString());
  }
});
```

El ejemplo anterior usa el abi de un *falso MANA* contrato de prueba, y llama a su método `setBalance` para otorgar 100 *falso MANA* a una cuenta. Al adaptar este ejemplo, reemplaza la dirección y el abi por los del contrato que quieras llamar, y llama a los métodos que defina su abi.

### Otras funciones

La biblioteca eth-connect incluye varias otras utilidades que puedes usar. Por ejemplo, para:

* Obtener un precio estimado del gas
* Obtener el saldo de una dirección dada
* Obtener un recibo de transacción
* Obtener el número de transacciones enviadas desde una dirección
* Convertir entre varios formatos, incluidos hexadecimal, binario, utf8, etc.

## Usar la red de prueba de Ethereum

Mientras pruebas tu escena, para evitar transferir MANA real u otras monedas, puedes usar la *red de prueba Ethereum Sepolia* y transferir en su lugar MANA falso de la testnet.

Para usar la red de prueba debes configurar tu extensión Chrome de Metamask para usar la *red de prueba Sepolia* en lugar de *red principal*.

Debes adquirir Ether de Sepolia, que puedes obtener gratis de varios faucets externos como [este](https://www.alchemy.com/faucets/ethereum-sepolia/).


> [!NOTE]
> **💡 Consejo**: Para ejecutar la transacción de transferir MANA de Sepolia a tu wallet, deberás pagar una gas fee en Ether de Sepolia.


Para previsualizar tu escena usando la red de prueba, pega la siguiente URL en una pestaña del navegador. Esto abrirá la escena en el cliente de escritorio de Decentraland:

`decentraland://realm=http://127.0.0.1:8000&local-scene=true&debug=true&dclenv=zone&position=0,0`


> [!NOTE]
> **💡 Consejo**: Cambia el parámetro position a las coordenadas de tu escena para cargar directamente en tu escena..


Cualquier transacción que aceptes mientras ves la escena en este modo solo ocurrirá en la red de prueba y no afectará el saldo de MANA en tu wallet real.

Si necesitas probar transacciones en la Polygon Testnet y necesitas tener MANA en esa testnet, tendrás que hacer swap de MANA a esa red después de obtenerlo en Sepolia. Para puentear MANA de Sepolia a la Polygon Testnet, visita tu [página de cuenta de Decentraland en Sepolia](https://account.decentraland.zone/) y haz clic en ‘swap’ en el lado de Ethereum MANA.

Al ejecutar una vista previa en el cliente web heredado, de una escena que usa una de las bibliotecas de ethereum, debes abrir la vista previa en una ventana del navegador separada, tener Metamask abierta en tu navegador e incluir manualmente la cadena `&ENABLE_WEB3`.

## Enviar mensajes RPC personalizados

Usa la función `sendAsync()` para enviar mensajes a través del [protocolo RPC](https://en.wikipedia.org/wiki/Remote_procedure_call).

```ts
import { sendAsync } from "~system/EthereumController";

// enviar un mensaje
await sendAsync({
  id: 1,
  method: "myMethod",
  jsonParams: "{ myParam: myValue }",
});
```

## smart contracts de Decentraland

En el siguiente enlace puedes encontrar una lista de smart contracts de Ethereum relevantes para el ecosistema de Decentraland. La lista incluye los contratos de la red principal, así como de otras redes de prueba de Ethereum.

[contracts.decentraland.org](https://contracts.decentraland.org/links)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/blockchain/scene-blockchain-operations.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
