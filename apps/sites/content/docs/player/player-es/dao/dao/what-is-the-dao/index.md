> La DAO es la plataforma de toma de decisiones de Decentraland.

# Qué es la DAO


> [!NOTE]
> **Misión de la DAO:** Garantizar la sostenibilidad del ecosistema de Decentraland mediante una gobernanza representativa, una gestión eficiente de los recursos colectivos y la innovación técnica. Lea la [misión, visión y roles](/docs/player/player-es/dao/dao/mission-vision-and-roles/) de la DAO, el DAO Council, DCL Regenesis Labs y la Decentraland Foundation.


La Decentraland DAO es la herramienta de toma de decisiones para los titulares de MANA, NAMES y LAND en el mundo virtual de Decentraland. Mediante votos en la DAO, la comunidad puede emitir grants y hacer cambios en las listas de nombres prohibidos, POIs y nodos catalyst. La DAO también controla los smart contracts de LAND y Estate.

La emisión de grants y los cambios en los registros y contratos propiedad de la DAO solo pueden realizarse usando propuestas predefinidas accesibles en [governance.decentraland.org](https://governance.decentraland.org).

Estas propuestas, los votos enviados y los resultados finales se almacenan todos en IPFS a través de Snapshot, un cliente de votación sin gas. Las propuestas aprobadas con acciones vinculantes son ejecutadas en la blockchain de Ethereum por un comité mediante una wallet multi-sig. Este comité está supervisado por el Security Advisory Board (SAB), otra multisig con titulares de claves de confianza. Este Comité fue elegido por la comunidad en la versión anterior de la DAO. [La propuesta original se puede encontrar aquí](https://forum.decentraland.org/t/proposal-for-a-more-accessible-and-affordable-dao/450).

El resto de este documento explica con más detalle qué es la DAO, cómo funciona y para qué puede usarse.

Para un tutorial detallado sobre cómo usar la Decentraland DAO, visita la [Guía de usuario de DAO](/docs/player/player-es/dao/dao-userguide/).

## La DAO funciona con smart contracts

Todas las DAO, u organizaciones autónomas descentralizadas, forman parte de un nuevo enfoque de gestión organizacional y toma de decisiones hecho posible por Ethereum.

Ethereum amplió lo que era posible con las blockchains al añadir la capacidad de descentralizar el manejo de datos más complejos que simples registros de propiedad de tokens. Ethereum hizo esto permitiendo que las personas pusieran smart contracts en una blockchain.

### ¿Qué es un smart contract?

Un smart contract es un programa informático que se ejecuta en la blockchain de Ethereum. Puede almacenar tanto funciones (fragmentos de código que hacen cosas) como datos (información). Los smart contracts a menudo se comparan con las máquinas expendedoras. Si introduces entradas específicas, obtienes salidas específicas. Si me acerco a una máquina expendedora, inserto 1 $ y presiono el botón "orange soda", entonces obtendré una orange soda si queda alguna en la máquina. Si ya no quedan orange sodas, me devolverán mi dólar.

Los smart contracts funcionan de la misma manera: las personas pueden interactuar con ellos enviando información con la expectativa de recibir resultados o información específicos. Al igual que la máquina expendedora no tiene a una personita dentro repartiendo refrescos, los smart contracts son automáticos (atrevámonos a decir, autónomos).

Si quieres aprender más sobre los smart contracts de Ethereum, la [documentación de Ethereum](https://ethereum.org/en/developers/docs/smart-contracts/) es el mejor lugar para empezar.

### La DAO controla los smart contracts críticos de Decentraland

La segunda cualidad importante de los smart contracts es su **capacidad de poseer otros smart contracts**.

Así es, cada smart contract tiene su propia dirección (igual que la dirección de tu wallet de Ethereum), lo que le permite poseer otros smart contracts y criptomonedas.

Entonces, en términos ligeramente más técnicos, una DAO es uno o más smart contracts que pueden realizar tareas específicas y predefinidas y mantener la propiedad de criptomonedas. Las DAO se construyen de tal manera que solo realizarán sus tareas bajo condiciones específicas, como la aprobación de una propuesta votada por un grupo de personas que posee un determinado token (como MANA, NAMES o LAND). Todo esto se hace en una blockchain. De ahí el nombre, "organización autónoma descentralizada".

La DAO de Decentraland también posee una suma de MANA y otros tokens junto con los smart contracts de LAND y Estate. [Este fondo](https://governance.decentraland.org/transparency/) se ha reservado para ayudar a patrocinar subvenciones de la comunidad y ayudar a hacer crecer la plataforma de Decentraland de acuerdo con las decisiones y direcciones votadas por la comunidad.


> [!WARNING]
> **📔 Nota** La DAO no posee, y por lo tanto no puede modificar, el [smart contract de MANA](https://etherscan.io/address/0x0f5d2fb29fb7d3cfee444a200298f468908cc942#readContract).
> 
> El propietario del smart contract de MANA es el [contrato TokenSale](https://etherscan.io/address/0xa66d83716c7cfe425b44d0f7ef92de263468fb3d#readContract). El propietario del contrato TokenSale es un contrato independiente que se autodestruyó al desplegarse ([como puedes ver aquí en Etherscan](https://etherscan.io/address/0xdf861993edbe95bafbfa7760838f8ebbd5afda9f)). Esto significa que no existe ningún otro contrato o wallet con los permisos para modificar o pausar la oferta de MANA.


También hay otra información que la DAO controla, como la lista de nombres dañinos u ofensivos que no están permitidos en Decentraland, una lista de ubicaciones destacadas (POIs o Points of Interest) que se promueven a nuevos usuarios, y la lista de servidores gestionados por la comunidad que alojan el mundo virtual de Decentraland.

Transferir cualquiera de los MANA de la DAO, modificar los smart contracts de LAND o Estate, o modificar cualquiera de la otra información listada controlada por la DAO **solo puede hacerse** con la aprobación de los titulares de MANA, NAMES y LAND.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/player/player-es/dao/dao/what-is-the-dao.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
