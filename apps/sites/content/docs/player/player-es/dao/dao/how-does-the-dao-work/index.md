> Resumen de las diferentes plataformas y entidades que conforman la DAO de Decentraland.

# Cómo funciona la DAO

Para eludir las tarifas de gas muy altas asociadas con la gobernanza totalmente on-chain, la DAO de Decentraland utiliza una combinación de votación gratuita off-chain para la comunidad y una wallet multisig controlada por el "Comité de la DAO" para ejecutar esas decisiones off-chain en la blockchain de Ethereum. Esto permite que todos los que tengan MANA, NAMES o LAND participen en la DAO sin tener que pagar ninguna tarifa cada vez que quieran votar o abrir una propuesta.

El uso de una wallet multisig controlada por un comité de personas de confianza garantiza la seguridad de la DAO, además de asegurar que las propuestas aprobadas resulten en una acción on-chain. Una segunda multisig propiedad del SAB proporciona una segunda capa de protección además del Comité de la DAO.

La DAO es un sistema complejo creado a partir de varias capas de entidades y plataformas, cada una de las cuales se enumera y describe a continuación:

## Governance dApp

La interfaz principal para la DAO de Decentraland se encuentra en [governance.decentraland.org](https://governance.decentraland.org). Aquí es donde los usuarios inician sesión, crean propuestas y votan.

*Actualmente esto es mantenido por un equipo de desarrollo financiado por una* [*subvención de la DAO*](https://governance.decentraland.org/proposal/?id=ed53e850-5e70-11ec-8188-4352ce3d30e7)*.*

## Snapshot

[Snapshot](https://snapshot.org/#/) es una plataforma de votación off-chain que proporciona una forma gratuita para que los poseedores de tokens voten sobre propuestas. Al almacenar tanto las propuestas como los votos en IPFS como mensajes firmados criptográficamente, Snapshot permite obtener resultados seguros y fácilmente impugnables.

La DAO de Decentraland utiliza Snapshot para alojar las propuestas y votos generados por la comunidad.

Cada vez que se abre una propuesta en governance.decentraland.org, también se crea automáticamente en [el espacio de Snapshot de Decentraland](https://snapshot.org/#/snapshot.dcl.eth). Esto permite a la DAO registrar y almacenar propuestas, votos y resultados de manera segura y descentralizada, y los resultados se muestran de vuelta en governance.decentraland.org. Desde el espacio de Snapshot los usuarios también pueden [delegar su VP](https://snapshot.org/#/delegate/snapshot.dcl.eth) a otros miembros de la comunidad.

## Aragon

[Aragon](https://aragon.org/) es una plataforma segura para crear y administrar la colección de smart contracts necesarios para ejecutar una DAO. El backend de la DAO de Decentraland está construido usando Aragon, así que cada vez que la comunidad aprueba en Snapshot una propuesta que resulta en una acción vinculante, el Comité de la DAO la registra en la mainnet de Ethereum en Aragon.

## Comité de la DAO

El Comité de la DAO es un grupo de tres personas de confianza que han sido seleccionadas por la comunidad para tener las claves en una wallet multisig. Esta multisig es responsable de ejecutar cualquier voto aprobado con una acción vinculante, como financiar una subvención, prohibir un NAME, añadir o quitar un POI, implementar una propuesta de Gobernanza o añadir un nodo Catalyst.

El Comité de la DAO está supervisado por el SAB, que tiene la capacidad de pausar y cancelar cualquier acción iniciada por el Comité.

Toda transacción on-chain iniciada por el Comité de la DAO tiene un retraso automático de 24 horas antes de completarse, lo que permite al SAB o al Comité revocar la transacción.

![Comité de la DAO](/docs/_remote/063ee68d732926eeb99173e9.webp)

## Security Advisory Board

El Security Advisory Board actúa como garante de la seguridad de los smart contracts de Decentraland, y se encarga de supervisar el trabajo del Comité de la DAO y de responder a los informes de vulnerabilidades y errores en cualquiera de los contratos de Decentraland.

El SAB incluye a 5 expertos en Solidity que han sido seleccionados inicialmente por el equipo de desarrollo de Decentraland.

Cada vez que se vaya a realizar una modificación en los contratos de LAND o Estate, la actualización debe contar con el apoyo unánime de la multisig del SAB. Se requieren al menos tres firmantes y ningún voto en contra para poder hacer cualquier cambio en los contratos de LAND o Estate.

El SAB tiene la capacidad de pausar, reanudar o cancelar cualquier acción realizada por el Comité de la DAO.

Iniciar la adición o eliminación de un miembro del SAB puede hacerse poniendo en marcha un proceso de propuesta de Gobernanza en [governance.decentraland.org](https://governance.decentraland.org).

![Security Advisory Board](/docs/_remote/199ca9caded989d293a0f36d.webp)

## Comité de Curación de Wearables

El Comité de Curación de Wearables es responsable de revisar y aprobar los wearables y emotes enviados por la comunidad de Decentraland.

![Comité de Curación de Wearables](/docs/_remote/dc4a02e026b26163106c05ac.webp)

## Comité de Revocaciones

El Comité de Revocaciones es responsable de revisar los casos presentados por la comunidad con respecto al programa de Grants y tiene la responsabilidad de ofrecer una resolución, que puede ser revocar un contrato de vesting como acción final.

[Comité de Revocaciones](https://github.com/lordlikedao/documentation/blob/main/static/images/DAO%20Organizational%20Chart/Revocation.png?raw=true)

## Equipo de Gobernanza

El Equipo de Gobernanza es responsable de desarrollar, mejorar y mantener la infraestructura tecnológica y las herramientas de toma de decisiones de la DAO.

![Equipo de Gobernanza](/docs/_remote/f6663bc09324c4879744664c.webp)

## Equipo de Facilitación

El Equipo de Facilitación es responsable de gestionar las comunicaciones y alinear a las distintas partes interesadas, fomentando la toma de decisiones colectiva y operaciones de gobernanza eficientes.

![Equipo de Facilitación](/docs/_remote/058f90f95aed65ab975d706d.webp)

## Equipo de Soporte de Grants

El Equipo de Soporte de Grants tiene la responsabilidad de mantener un diálogo constante con la comunidad de beneficiarios de Grants, para entender sus necesidades y brindar apoyo a sus bloqueos y solicitudes.

![Equipo de Soporte de Grants](/docs/_remote/28cc4e3f4a37dfd9819f791d.webp)

## El Forum

La [Decentraland Forum](https://forum.decentraland.org) es el centro de comunicación de la DAO. Cada vez que se crea una nueva propuesta, también se abre automáticamente un hilo acompañante en el Forum bajo la [sección de Gobernanza](https://forum.decentraland.org/c/governance/). Estos temas de propuesta se pueden encontrar ya sea explorando el Forum, o haciendo clic en el botón "Discutir en el Forum" en cualquier página de detalles de una propuesta en la UI de Gobernanza. Estos hilos son un espacio donde los votantes pueden debatir los diversos impactos y problemas que podrían derivarse de una propuesta.

## Servidores de Discord

La DAO opera en dos servidores de Discord:

* [canal #dao](https://discord.com/channels/417796904760639509/538106198419832844) en el servidor de Discord de Decentraland para anuncios generales, debates y preguntas.
* Un [servidor de Discord dedicado](https://dcl.gg/daodiscord) solo a las operaciones de la DAO, con debates más en profundidad, canales específicos para grupos de trabajo que abordan cuestiones complejas y espacios para que los beneficiarios de Grants de la comunidad publiquen actualizaciones sobre su proyecto. En este canal, la DAO también organiza su Town Hall mensual.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/player/player-es/dao/dao/how-does-the-dao-work.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
