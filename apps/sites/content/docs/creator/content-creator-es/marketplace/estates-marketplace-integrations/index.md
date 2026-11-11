> Cosas importantes a tener en cuenta si quieres integrar el Estate de Decentraland en tu marketplace

# Integraciones de Estates Marketplace

## Integrar el Estate de Decentraland en tu Marketplace

El Estate de Decentraland es un NFT compatible con ERC721 y funciona en Ethereum Mainnet. Por lo tanto, muchos marketplaces de terceros pueden comerciarlo. Para hacerlo, este marketplace debe seguir ciertas reglas para mantener seguras las operaciones para los usuarios.

Como quizá sepas, cada [Estate](https://docs.decentraland.org/decentraland/faq/#what-is-an-estate) es un grupo de [LANDs](https://docs.decentraland.org/decentraland/faq/#what-is-land) y tiene un *bytes* hash asociado al grupo que se llama *`fingerprint`*. Cada vez que el Estate cambia al agregar o quitar un LAND, su *`fingerprint`* también cambia.

Para los marketplaces, especialmente los que no tienen un sistema de escrow, se recomienda al 100% tener un registro del *`fingerprint`* del Estate cuando alguien lo pone a la venta o hace una oferta. De esa manera, cuando la orden/oferta se ejecuta correctamente, el propietario actual no puede cambiar el estate intentando hacer front-run de la ejecución de la orden.

No se recomienda listar Estates vacíos. El smart contract de Estate tiene un método `getEstateSize` que devuelve la cantidad de LANDs en el Estate. Si el resultado es `0`, te recomendamos no listar ese Estate. Si aun así quieres listarlo, verás una imagen como esta:

![](/docs/_remote/3ea9308d2dc5bf30fe069b53.webp)

## Ejemplo

### No usar el fingerprint del Estate

* Bob tiene el Estate1 con LAND (1,1) y (1,2). fingerprint de Estate1: hash1
* Bob agrega el LAND (1,3) a Estate1. Estate1 tiene los LANDs: (1,1), (1,2) y (1,3). fingerprint de Estate1: hash2 (El fingerprint cambió)
* Bob quita el LAND (1,1) de Estate1. Estate1 tiene los LANDs: (1,2) y (1,3). fingerprint de Estate1: hash3 (El fingerprint cambió)
* Bob pone a la venta el Estate1. La lista se crea onchain en Ethereum Mainnet con la dirección del smart contract de Estate, el id de Estate, el precio y la fecha de expiración.
* Alice envía una transacción para comprar el Estate especificando el id del estate y el precio esperado a pagar.
* Bob detecta que alguien está intentando comprar su Estate1 y envía una transacción con gas fees más altos que los de Alice para quitar los LANDs (1,2) y (1,3) de Estate1.
* La transacción de Bob se mina primero. Estate1 tiene 0 LANDs. fingerprint de Estate1: hash4 (El fingerprint cambió)
* La transacción de Alice se mina después. Alice compró el Estate1 con 0 LANDs en él. Esto significa que Alice fue objeto de front-run (y estafada/robada) por Bob.

### Usar el fingerprint del Estate

* Bob tiene el Estate1 con LAND (1,1) y (1,2). fingerprint de Estate1: hash1
* Bob agrega el LAND (1,3) a Estate1. Estate1 tiene los LANDs: (1,1), (1,2) y (1,3). fingerprint de Estate1: hash2 (El fingerprint cambió)
* Bob quita el LAND (1,1) de Estate1. Estate1 tiene los LANDs: (1,2) y (1,3). fingerprint de Estate1: hash3 (El fingerprint cambió)
* Bob pone a la venta el Estate1. La lista se crea onchain en Ethereum Mainnet con la dirección del smart contract de Estate, el id de Estate, el precio y la fecha de expiración.
* Alice envía una transacción para comprar el Estate especificando el id del estate, el precio esperado a pagar y **el fingerprint que vio (hash3)**.
* Bob detecta que alguien está intentando comprar su Estate1 y envía una transacción con gas fees más altos que los de Alice para quitar los LANDs (1,2) y (1,3) de Estate1.
* La transacción de Bob se mina primero. Estate1 tiene 0 LANDs. fingerprint de Estate1: hash4 (El fingerprint cambió)
* La transacción de Alice se revierte porque el smart contract comprobó que el fingerprint en el parámetro que envió Alice no coincidía con el fingerprint actual de Estate1 (hash3 != hash4). Esta comprobación evitó que Alice comprara un Estate no deseado.

## Interfaz de fingerprint del Smart Contract de Estate

El smart contract de Estate es compatible con una [interfaz de fingerprint](https://github.com/decentraland/land/blob/master/contracts/estate/EstateStorage.sol#L19). Cuando un Estate se pone a la venta o recibe una oferta, registra su fingerprint llamando a *`getFingerprintV2(uint256 estateId)`* y guarda el valor que devuelve. Para comprobar si esa orden/oferta sigue siendo válida en el momento de la ejecución, llama a *`verifyFingerprint(uint256 estateId, bytes fingerprint)`* con el valor almacenado.

Guarda siempre el valor devuelto por *`getFingerprintV2`*. La versión anterior, *`getFingerprint`* , se mantiene solo por compatibilidad con versiones anteriores y se está retirando gradualmente: a partir del 26 de noviembre de 2026, *`verifyFingerprint`* dejará de aceptar valores producidos por ella. Las nuevas integraciones deberían usar *`getFingerprintV2`*.

Puedes consultar un ejemplo de producción funcional [aquí](https://github.com/decentraland/marketplace-contracts/blob/master/contracts/marketplace/MarketplaceV2.sol#L382).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/marketplace/estates-marketplace-integrations.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
