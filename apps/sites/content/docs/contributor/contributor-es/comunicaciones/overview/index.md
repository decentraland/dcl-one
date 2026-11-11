# Resumen

El sistema de comunicaciones de Decentraland, o simplemente *comms*, es el protocolo de mensajería en tiempo real que maneja la interacción entre players en un Realm.

Algunas de estas interacciones son iniciadas por players, otras son gestionadas automáticamente por los clients internamente. Algunos ejemplos:

* Chat de texto y voz
* Actualizaciones de posición a medida que los players se mueven por el entorno
* Actualizaciones de Avatar cuando los players cambian su apariencia


> [!NOTE]
> Puedes ver el protocolo de comms en acción y experimentar con él usando el de código abierto [Comms Station](https://decentraland.github.io/comms-station/).


Dado que la mayor parte de esta funcionalidad requiere transmitir mensajes a todos los players cercanos, se agrupan automáticamente en clústeres basados en proximidad llamados *islas*. A cada player se le asigna una sola isla a la vez, lo que cambia a medida que viaja por el mundo y se mueve en relación con los demás.

El servicio Realm que gestiona y asigna players a las islas se llama *Archipelago*. Se encarga de crear islas a medida que se necesitan, mantener su población en un número razonable y reasignar dinámicamente a los players en respuesta a sus movimientos.

```mermaid
flowchart LR
    subgraph Realm
        Players["Jugadores<br/>⭐⭐⭐⭐⭐⭐⭐⭐⭐"]
        Archipelago["Archipelago"]
        
        subgraph Islas
            Island1["Isla 1<br/>⭐⭐⭐⭐"]
            Island2["Isla 2<br/>⭐⭐⚪⚪"]
            IslandN["Isla n<br/>⭐⭐⭐⚪"]
        fin
        
        Players --> Archipelago
        Archipelago --> Island1
        Archipelago --> Island2
        Archipelago --> IslandN
    fin
```

Cuando se asigna a un island, a los clients se les proporciona un URI específico de la isla para conectarse al backend real que retransmitirá mensajes entre ellos. Esta conexión dura hasta que Archipelago reasigna al client a una isla diferente.

Esto significa que, además del [Archipelago](/docs/contributor/contributor-es/comunicaciones/archipelago/) protocol, los clients deben implementar una serie de *transports*, cada uno envolviendo uno de los backends compatibles en una interfaz unificada.

### Ciclo de vida del client <a href="#lifecycle" id="lifecycle"></a>

El ciclo de vida de un client de comms se puede resumir en unos pocos pasos:

1. **Seleccionar un Realm**: obtener un URI para el [Archipelago](/docs/contributor/contributor-es/comunicaciones/archipelago/) servicio.
2. **Unirse a Archipelago**: abrir una conexión persistente con el servicio.
3. **Obtener una (re)asignación de island**: informar la posición actual y obtener un URI específico de la isla.
4. **Conectar un transport**: abrir una segunda conexión con el backend específico de la isla.
5. **Repetir**: continuar pasando por los pasos 3 y 4, informando periódicamente nuevas posiciones.

Cuando el client finaliza su sesión, simplemente se desconecta del servicio Archipelago. Será eliminado automáticamente de su isla actual.

```mermaid
sequenceDiagram
    participant Archipelago
    participant Client
    participant IslandBackend as Island Backend
    
    Nota sobre Client: Conectarse a Archipelago
    Client->>Archipelago: Autenticar
    Archipelago->>Client: Aceptar
    
    Nota sobre Client: Informar posición
    Client->>Archipelago: Informar posición
    Archipelago->>Client: Asignar isla
    
    Nota sobre Client: Conectarse a la isla
    Client->>IslandBackend: Conectar transport
    Client->>IslandBackend: Autenticar
    IslandBackend->>Client: Aceptar
    
    Nota sobre Client,IslandBackend: Intercambiar mensajes
    Client->>IslandBackend: Mensajes del player…
    IslandBackend->>Client: Mensajes del player…
    
    Nota sobre Client: Actualizar posición
    Client->>Archipelago: Actualizar posición
    Archipelago->>Client: Reasignar isla
    
    Nota sobre Client: Desconectarse de la isla anterior
    Client->>IslandBackend: Desconectar transport
    
    Nota sobre Archipelago,IslandBackend: Repetir ciclo
```

### Conexiones

Las conexiones de comms se basan principalmente en websocket, aunque algunos transports pueden usar otras estrategias.

Ve a las secciones de [Transports](/docs/contributor/contributor-es/comunicaciones/transports/) o [Archipelago](/docs/contributor/contributor-es/comunicaciones/archipelago/) para obtener más información, o a la página de un transport específico para conocer sus detalles.

### Autenticación

Las conexiones a comms se autentican haciendo que los clients firmen un desafío proporcionado por el server, utilizando el esquema descrito en la sección de [cadena de autenticación](/docs/contributor/contributor-es/autenticacion/authchain/) .

Dirígete a la [Archipelago](/docs/contributor/contributor-es/comunicaciones/archipelago/) página o consulta un transport específico para obtener más información sobre los flujos de autenticación.

### Islas

Las islas son grupos de players altamente dinámicos que pueden transmitir mensajes entre sí, creados y mantenidos por [Archipelago](/docs/contributor/contributor-es/comunicaciones/archipelago/) en respuesta a sus movimientos en el mundo.

No hay un área predefinida ni un punto central para una island. No son características geográficas estables, solo asociaciones temporales de players cercanos. Si se puede decir que una island cubre una región, es solo porque sus miembros están actualmente distribuidos en esa zona.

Puede haber cero islas en una región sin players y varias islas superpuestas en áreas densamente pobladas, donde asignar a todos al mismo grupo haría imposible la difusión en tiempo real.

Cada server de la red de Decentraland puede configurar la población máxima de una island y la distancia a la que los players se consideran cercanos. De forma predeterminada, las islas pueden alojar hasta 100 players dentro de 100 metros entre sí.

El flujo para ser asignado a una island y unirse a ella se detalla en la [Archipelago](/docs/contributor/contributor-es/comunicaciones/archipelago/) .

### Mensajes

Los mensajes en el protocolo de comms son blobs binarios serializados usando [protocol buffers](https://github.com/protocolbuffers/protobuf), envueltos en una estructura [Packet](/docs/contributor/contributor-es/comunicaciones/messages/#packet) .


> [!NOTE]
> Cuando usamos la palabra *message* en el contexto de comms, siempre nos estaremos refiriendo al protocolo de mensajes binarios, no a los mensajes de chat intercambiados entre players.


Hay varios tipos diferentes de mensajes, para una variedad de flujos de interacción en tiempo real. Dirígete a la sección de [mensajes](/docs/contributor/contributor-es/comunicaciones/messages/) para obtener más información.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/comunicaciones/overview.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
