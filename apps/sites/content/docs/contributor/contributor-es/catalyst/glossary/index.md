# Glosario

#### Catalyst

[Catalyst](https://github.com/decentraland/catalyst) es el nombre dado al **Paquete de servicios de Decentraland**, cada Catalyst [nodo](https://decentraland.github.io/catalyst-monitor) expone un conjunto de servicios que funcionan como la base de la plataforma y también expone una [API](https://decentraland.github.io/catalyst-api-specs/).

#### Bootstrap

Cuando el Catalyst se inicia, el nodo tendrá un `Bootstrapping` estado mientras obtiene contenido de otros catalysts hasta ponerse al día con los demás. Luego transiciona al siguiente estado `Syncing` en el que está listo para servir contenido.

#### Cliente de Catalyst

[Biblioteca](https://github.com/decentraland/catalyst-client) que puede usarse para interactuar con los servidores de Catalyst. Permite tanto obtener datos como desplegar nuevas Entities en el servidor especificado.

#### Entity

Tipo de dato, definido con [schemas](https://github.com/decentraland/common-schemas), que no puede almacenarse en la blockchain debido a su tamaño. Algunos de los [tipos admitidos](https://github.com/decentraland/common-schemas/blob/main/src/platform/entity.ts#L13) actualmente son los siguientes:

* [Wearable](https://github.com/decentraland/common-schemas/blob/main/src/platform/item/wearable/wearable.ts)
* [Emote](https://github.com/decentraland/common-schemas/blob/main/src/platform/item/emote/emote.ts)
* [Scene](https://github.com/decentraland/common-schemas/blob/main/src/platform/scene/scene.ts)
* [Perfil](https://github.com/decentraland/common-schemas/blob/main/src/platform/profile/profile.ts)
* [Stores](https://github.com/decentraland/common-schemas/blob/main/src/dapps/store.ts)
* [Outfits](https://github.com/decentraland/common-schemas/blob/main/src/platform/outfits/outfits.ts)

#### Despliegue

Instancias de Entities creadas en el Content Server e identificadas con un [EntityId](https://github.com/decentraland/common-schemas/blob/main/src/platform/entity.ts#L31). Los deployments pueden ser de cualquier [tipo de Entity](https://github.com/decentraland/common-schemas/blob/main/src/platform/entity.ts#L13).

#### Deployment fallido

Instancia de una Entity desplegada en un Content Server que no logró pasar las [validaciones de contenido](https://adr.decentraland.org/adr/ADR-51). Las validaciones se implementan en el [validador de contenido](https://github.com/decentraland/content-validator) biblioteca.

#### Entity activa

Una Entity que actualmente está desplegada en un puntero; por lo tanto, todas las Entities activas representan el contenido en vivo de Decentraland: escenas, perfiles, wearables, etc.

#### Puntero

Los punteros son los ids o ubicaciones a los que pertenece un Deployment. Su formato difiere según el `tipo de Entity`, por ejemplo:

* **Wearables**: usa URN como punteros `urn:decentraland:ethereum:collections-v1:dg_atari_dillon_francis:gerald_patchwork_knit_sweater`
* **Scenes**: usa parcels o un array de parcels como punteros, por ejemplo `--10,20` o `[(0,0), (0,1), (0,-1)]`
* **Profiles**: usa una dirección de Wallet como puntero `0xb01ddb6d4c89314f74a32dfc2f3bd0f8311a6d75`
* **Stores**: usa una URN como puntero `urn:decentraland:off-chain:marketplace-stores:{wallet}`
* **Outfits**: usa una dirección de Wallet como puntero seguida de `:outfits`, `0xb01ddb6d4c89314f74a32dfc2f3bd0f8311a6d75:outfits`

#### URN

[URN](https://en.wikipedia.org/wiki/Uniform_Resource_Name) significa Uniform Resource Name. Se usan como identificadores únicos globales para activos en todos los proyectos del Metaverse.

* Resolutor de URN: <https://github.com/decentraland/urn-resolver>
* Espacio de nombres de URN: <https://github.com/common-metaverse/urn-namespaces>

#### Servidor de contenido

El [Servidor de contenido](https://github.com/decentraland/catalyst/tree/main/content) es una parte del nodo Catalyst encargada de gestionar el almacenamiento descentralizado de despliegues de Entities. Actualmente, todo el contenido almacenado en un Content Server se sincroniza con el resto de los Catalysts del DAO.

#### Lambdas

El [Lambdas](https://github.com/decentraland/catalyst/tree/main/lambdas) Service es una parte del nodo Catalyst que funciona como capa de reconciliación entre los Content Servers y la Blockchain, saneando el contenido, validando la propiedad y ayudando a recuperar información para los clientes de Catalyst.

#### Sincronización

Los Content Servers ofrecen la posibilidad de desplegar contenido en ellos. También garantizan que cualquier contenido desplegado en los otros Catalysts del DAO esté disponible allí también. Resolver esto se denomina **sincronización**.

#### SNAPSHOT de contenido

Un conjunto de todas las Entities activas de un tipo específico en un [Content Server](#content-server). Estos snapshots de contenido se usan para sincronizar contenido entre nodos Catalyst.

#### Realm

Cuando te conectas a Decentraland, solo podrás interactuar con Peers conectados al mismo Realm. Como nombres amigables, los Realms utilizan nombres de dioses griegos como Hela o Loki. Cada realm está determinado por un único [Nodo Catalyst](https://decentraland.github.io/catalyst-monitor/).

#### Peer

Un usuario conectado a Decentraland.

#### Lighthouse

El [Lighthouse](https://github.com/decentraland/lighthouse) es el nombre usado para referirse a la versión anterior del Communications Service alojado en los nodos Catalyst y usado para agrupar usuarios en una red P2P. Sus responsabilidades principales eran determinar cuáles son los candidatos para una conexión P2P y realizar la señalización WebRTC para establecer la conexión. La mayor parte de esta lógica se hacía a través de dos componentes externos: el PeerJS Server (conecta [WebRTC](https://webrtc.org/) Peers) y Archipelago (recibe las posiciones de los usuarios y los agrupa en islas).

#### Backend for Frontend (también llamado BFF)

El [BFF](https://github.com/decentraland/explorer-bff) Service fue creado para resolver las necesidades del cliente y permitir un desarrollo más rápido de nuevas funcionalidades sin romper las APIs existentes. En el contexto del Catalyst Communication Service, se encargará de gestionar la señalización P2P y así reemplazar a Lighthouse.

#### Propietario de Catalyst

Un [Propietario de Catalyst](https://github.com/decentraland/catalyst-owner) es una persona o una organización que ejecuta un nodo Catalyst. Para añadir un nodo Catalyst a la red del DAO se [propuesta](https://governance.decentraland.org/?type=catalyst) debe presentarse y los nodos del DAO representan la red de confianza de Decentraland.

#### Servicio de Archipelago

El [Servicio de Archipelago](https://github.com/decentraland/archipelago-service) anteriormente era una biblioteca utilizada por Lighthouse. Este servicio tendrá la misma responsabilidad que tenía la biblioteca: agrupar a los Peers en clústeres para que puedan comunicarse eficientemente entre sí. Cada clúster se llama una isla. Por otro lado, el servicio también necesita poder equilibrar islas usando los transportes de comunicación disponibles y siguiendo un conjunto de [Propietario de Catalyst](https://github.com/decentraland/catalyst-owner) reglas definidas, con el fin de, por ejemplo, usar LiveKit para una isla en la escena Casino y P2P en una Genesis Plaza.

#### Isla

Clúster de Peers conectados.

#### Transporte de comunicación

Transport es la tecnología utilizada para compartir actualizaciones entre Peers conectados a una Isla.

#### Transporte P2P

Usa [WebRTC](https://webrtc.org/) conexiones entre Peers, no se necesita infraestructura adicional para escalar, ya que la carga recae en el lado del cliente y los Servers solo gestionan la señalización. La latencia aumenta a medida que se añaden más Peers a una Isla. Hay un límite en la cantidad de conexiones WebRTC que puede tener un cliente, por lo que, para tener islas más grandes, se necesitan más conexiones indirectas (saltos) entre Peers.

#### Transporte LiveKit

[LiveKit](https://livekit.io/) es un proyecto de código abierto que proporciona conferencias escalables para múltiples usuarios sobre WebRTC. En lugar de hacer una red P2P, los Peers se conectan a una Selective Forwarding Unit (SFU) encargada de gestionar el reenvío de mensajes y distintos aspectos de calidad de la comunicación. Las SFU son la infraestructura adicional necesaria para proporcionar comunicaciones de alto rendimiento y alta calidad entre multitudes en escenas designadas.

#### Transporte WebSocket

El [Transporte WebSocket](https://github.com/decentraland/ws-room-service) es más ligero que LiveKit; también requiere un Server que gestione el reenvío de mensajes a través de salas WebSocket. Las salas son equivalentes a las islas. Este transporte ofrece menos latencia y mejor rendimiento que las islas P2P, pero también requiere la infraestructura adicional.

#### NATS

[NATS](https://nats.io/) es un broker de mensajes que permite el intercambio de datos y la comunicación entre servicios. En el contexto del Catalyst Server se utiliza para comunicar algunos de los servicios internos.

#### DAO

DAO significa "Decentralized Autonomous Organization". El DAO de Decentraland posee los smart contracts y activos más importantes que conforman Decentraland: el LAND Contract, el Estates Contract, Wearables, Content Servers y el Marketplace. También posee una importante reserva de MANA que le permite ser realmente autónomo y subvencionar diversas operaciones e iniciativas en todo Decentraland.

<https://dao.decentraland.org/en/>


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/catalyst/glossary.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
