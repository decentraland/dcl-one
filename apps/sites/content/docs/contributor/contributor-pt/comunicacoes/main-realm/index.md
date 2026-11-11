# MAIN Realm

Decentraland tem tantos realms quantos forem os Catalyst com serviços de comunicação, mas existe um realm distinto conhecido como MAIN que existe fora da rede de Catalyst. O realm MAIN foi projetado especificamente para simplificar o processo de conexão do usuário, tornando mais fácil para os usuários interagir e se encontrar. Ele é construído com um conjunto de serviços capaz de substituir o serviço de comunicação de um nó Catalyst. Este realm serve como o destino padrão para os usuários ao fazer login no Decentraland.

<img src="/docs/_remote/d01dbf731e2a22a53eeb9e6a.webp" alt="" width="800">

O [provedor de realm](https://github.com/decentraland/realm-provider) services expõe um [/about](https://realm-provider.decentraland.org/main/about) endpoint com uma descrição de realm de acordo com o [ADR-110: Realm description](https://adr.decentraland.org/adr/ADR-110). Isso fornece as informações para estabelecer uma conexão com a rede de gerenciamento de conteúdo, conhecida como um dos serviços do nó Catalyst, e também fornece os detalhes de conexão para os serviços de comunicação do realm MAIN.

O serviço de comunicação MAIN é acessado por meio do [Conector WebSocket](https://github.com/decentraland/archipelago-workers). Este componente leve permite a interação com os demais componentes de backend e pode ser facilmente escalado com instâncias adicionais para acomodar uma carga de usuários em crescimento, gerenciando de forma eficiente um número maior de conexões ativas. Por meio da conexão WebSocket, o cliente obtém os dados necessários para estabelecer uma conexão com o LiveKit Cloud, juntamente com o `islandId` atribuído pelo Archipelago Service. O Archipelago Service tem responsabilidades idênticas tanto no realm MAIN quanto no serviço de comunicação dos nós Catalyst. Ele gerencia efetivamente o processamento das posições dos usuários e sua subsequente alocação em ilhas distintas, cada uma das quais corresponde a uma sala do Livekit, organizando assim eficientemente grupos de usuários com base em suas localizações espaciais dentro do mundo virtual. Para mais informações sobre o Archipelago Service, você pode consultar a [Archipelago](/docs/contributor/contributor-pt/comunicacoes/archipelago/) página.

[NATS](https://nats.io/) funciona como um sistema de broadcast, facilitando a comunicação contínua entre vários serviços. Ele pode suportar múltiplas instâncias do WebSocket Connector comunicando-se com o mesmo Archipelago Service. Além disso, ele permite que o serviço de Stats reúna estatísticas de usuários do Archipelago, como posições e agrupamentos de ilhas, tornando esses dados acessíveis para uso externo.

O serviço de estatísticas do Archipelago compartilha a mesma implementação de API que o serviço Archipelago no nó Catalyst. Você pode explorar a API neste link: [Especificações da API do Catalyst](https://decentraland.github.io/catalyst-api-specs/#tag/Archipelago). O serviço está acessível por meio do seguinte URL `https://archipelago-stats.decentraland.org/` e os endpoints disponíveis estão em `/comms/` conforme descrito na Especificação da API, por exemplo: `https://archipelago-stats.decentraland.org/comms/peers`.

O [LiveKit](/docs/contributor/contributor-pt/comunicacoes/tipos-de-transporte/livekit/) A plataforma cloud gerencia a tarefa complexa de trocar as informações dos usuários por meio de WebRTC e é a principal plataforma cloud compartilhada pela maioria dos nós Catalyst para o mesmo propósito.

O conceito fundamental derivado do realm MAIN é que diferentes usuários têm flexibilidade para se conectar a qualquer nó Catalyst para fins de gerenciamento de conteúdo (baixar scenes, wearables, emotes ou enviar atualizações de perfil), mas, apesar da seleção de um nó de gerenciamento de conteúdo específico, a conexão com o serviço de comunicação **pode** ser a mesma. Como resultado, mesmo que os usuários estejam utilizando diferentes nós Catalyst como servidores de conteúdo, mas se encontrem em coordenadas próximas da scene, eles terão a capacidade de interagir entre si.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/comunicacoes/main-realm.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
