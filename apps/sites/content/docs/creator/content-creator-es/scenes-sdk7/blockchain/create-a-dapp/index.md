> Referencia para crear tu propia Aplicación Descentralizada

# Crear una dApp

Puedes crear tus propias aplicaciones descentralizadas (dApps) para interactuar con los smart contracts de Decentraland y exponer su funcionalidad de formas más elaboradas y amigables.

## ¿Qué es una dApp?

Una aplicación descentralizada, o dApp, es la que se ejecuta en una red distribuida peer to peer en lugar de hacerlo desde un servidor central.

En el contexto de blockchain, una dApp usa smart contracts y posiblemente una red P2P, en lugar de un servicio de API web. Una dApp también puede exponer un front end y almacenar en caché información de la blockchain temporalmente, pero su resultado se refleja en última instancia on-chain.

Consulta [este sitio](https://blockchainhub.net/decentralized-applications-dapps/) para una visión más completa sobre las dApps.

## smart contracts de Decentraland

Decentraland ha escrito y mantiene una serie de smart contracts que interactúan con los tokens LAND y MANA.

Los propios tokens LAND y MANA están definidos por los *LANDregistry* y *MANAtoken* contratos, respectivamente. La lista también incluye contratos más específicos, como crear un estate a partir de varios parcels o hipotecar parcels.

* [smart contracts de Decentraland](https://contracts.decentraland.org/addresses.json): Puedes encontrar una lista completa de cada uno de nuestros contratos y sus direcciones.

Ten en cuenta que cada contrato tiene una versión de producción en *mainnet* y una versión de prueba en *sepolia* y que cada una tiene una dirección diferente.

Puedes leer el código completo de cada contrato, ya que es información pública en la blockchain. Por ejemplo, puedes encontrar el contrato por nombre en [Etherscan](https://etherscan.io/contractsVerified) para leer su contenido.

## código base de dApp

* [Código base](https://github.com/decentraland/dapp-boilerplate): Este código base puede ser un punto de partida para construir tu propia dApp. Ten en cuenta que este repositorio está archivado y ya no se mantiene, así que trátalo como una referencia y no como una plantilla actualizada.

## Bibliotecas auxiliares

Mientras construíamos nuestras propias dApps internamente, reunimos algunas bibliotecas auxiliares que también podrían resultarte útiles.

* [Decentraland-dapps](https://github.com/decentraland/decentraland-dapps): Esta es una biblioteca de nivel superior con módulos comunes para dApps, utilizada activamente por las propias aplicaciones de Decentraland.
* [Decentraland UI](https://ui.decentraland.org/): Esta biblioteca contiene una selección de elementos de UI reutilizables que están incluidos en los proyectos de Decentraland.
* [Decentraland-eth](https://github.com/decentraland/decentraland-eth): Esta es una biblioteca de bajo nivel con funciones de utilidad para trabajar con la blockchain de Ethereum. Ten en cuenta que este repositorio está archivado y ya no se mantiene.

## dApps de ejemplo

A continuación tienes enlaces al código completo de varias dApps que construimos alrededor de Decentraland; esto puede ayudarte a crear la tuya:

* [Gate](https://github.com/decentraland/gate): Esta dApp básica crea un NFT de invitación que se usaba cuando Decentraland estaba en una etapa de beta cerrada. Al principio, los jugadores de la lista blanca que poseían este NFT eran los únicos que podían entrar al metaverso.
* [Marketplace](https://github.com/decentraland/marketplace): Esta es la aplicación completa que ejecuta Decentraland [Marketplace](https://market.decentraland.org/). Para que funcione rápido, requiere una base de datos y un servidor backend conectado a la red de Ethereum para crear índices con información sobre LAND.

## marco de pruebas de dApp

Antes de lanzar tu dApp a producción, te recomendamos probarla primero.

* [dAppeteer](https://github.com/decentraland/dappeteer): Creamos este framework para ayudarte a ejecutar pruebas en tu dApp. Ten en cuenta que este repositorio está archivado y ya no se mantiene.

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
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/blockchain/create-a-dapp.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
