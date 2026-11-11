# Glossário

#### Catalyst

[Catalyst](https://github.com/decentraland/catalyst) é o nome dado ao **Pacote de Serviços do Decentraland**, cada Catalyst [nó](https://decentraland.github.io/catalyst-monitor) expõe um conjunto de serviços que funcionam como a espinha dorsal da plataforma e também expõe uma pública [API](https://decentraland.github.io/catalyst-api-specs/).

#### Bootstrap

Quando o Catalyst é iniciado, o nó terá um `Inicializando` estado enquanto obtém conteúdo de outros catalysts até se atualizar em relação aos restantes. E depois transita para o estado seguinte `Sincronizando` no qual está pronto para servir conteúdo.

#### Cliente Catalyst

[Biblioteca](https://github.com/decentraland/catalyst-client) que pode ser usada para interagir com os servidores Catalyst. Permite tanto obter dados como implementar novas Entities no servidor especificado.

#### Entity

Tipo de Dados, definido com [esquemas](https://github.com/decentraland/common-schemas), que não pode ser armazenado na blockchain devido ao seu tamanho. Alguns dos atuais [tipos suportados](https://github.com/decentraland/common-schemas/blob/main/src/platform/entity.ts#L13) são os seguintes:

* [Wearable](https://github.com/decentraland/common-schemas/blob/main/src/platform/item/wearable/wearable.ts)
* [Emote](https://github.com/decentraland/common-schemas/blob/main/src/platform/item/emote/emote.ts)
* [Cena](https://github.com/decentraland/common-schemas/blob/main/src/platform/scene/scene.ts)
* [Perfil](https://github.com/decentraland/common-schemas/blob/main/src/platform/profile/profile.ts)
* [Stores](https://github.com/decentraland/common-schemas/blob/main/src/dapps/store.ts)
* [Outfits](https://github.com/decentraland/common-schemas/blob/main/src/platform/outfits/outfits.ts)

#### Implantação

Instâncias de Entities criadas no Content Server e identificadas com um identificador único [EntityId](https://github.com/decentraland/common-schemas/blob/main/src/platform/entity.ts#L31). As implementações podem ser de qualquer [Entity Type](https://github.com/decentraland/common-schemas/blob/main/src/platform/entity.ts#L13).

#### Implementação falhada

Instância de uma Entity implementada num Content Server que não passou nas [Validações de conteúdo](https://adr.decentraland.org/adr/ADR-51). As validações são implementadas no [validador de conteúdo](https://github.com/decentraland/content-validator) biblioteca.

#### Entity ativa

Uma Entity que está atualmente implementada num ponteiro, pelo que todas as Entities ativas representam o conteúdo em tempo real de Decentraland: scenes, perfis, wearables, etc.

#### Ponteiro

Os ponteiros são os ids ou localizações às quais um Deployment pertence. O seu formato é diferente consoante o `Entity Type`, por exemplo:

* **Wearables**: Usa URN como ponteiros `urn:decentraland:ethereum:collections-v1:dg_atari_dillon_francis:gerald_patchwork_knit_sweater`
* **Scenes**: Usa parcels ou um array de parcels como ponteiros, por exemplo `--10,20` ou `[(0,0), (0,1), (0,-1)]`
* **Profiles**: Usa um endereço de Wallet como ponteiro `0xb01ddb6d4c89314f74a32dfc2f3bd0f8311a6d75`
* **Stores**: Usa uma URN como ponteiro `urn:decentraland:off-chain:marketplace-stores:{wallet}`
* **Outfits**: Usa um endereço de Wallet como ponteiro com o sufixo de `:outfits`, `0xb01ddb6d4c89314f74a32dfc2f3bd0f8311a6d75:outfits`

#### URN

[URN](https://en.wikipedia.org/wiki/Uniform_Resource_Name) significa Uniform Resource Name. São usados como identificadores únicos globais para ativos em todos os projetos do Metaverso.

* URN Resolver: <https://github.com/decentraland/urn-resolver>
* URN Namespace: <https://github.com/common-metaverse/urn-namespaces>

#### Servidor de conteúdo

O [Servidor de conteúdo](https://github.com/decentraland/catalyst/tree/main/content) faz parte do nó Catalyst responsável por gerir o armazenamento descentralizado de Entity Deployments. Atualmente, todo o conteúdo armazenado num Content Server é sincronizado com o resto dos Catalysts da DAO.

#### Lambdas

O [Lambdas](https://github.com/decentraland/catalyst/tree/main/lambdas) Service faz parte do nó Catalyst que funciona como uma camada de reconciliação entre os Content Servers e o Blockchain, sanitizando o conteúdo, validando a propriedade e ajudando a recuperar informação para os Catalyst clients.

#### Sincronização

Os Content Servers oferecem a capacidade de implementar conteúdo neles. Também garantem que qualquer conteúdo implementado nos outros Catalysts da DAO esteja também disponível lá. Resolver isto chama-se **sincronização**.

#### SNAPSHOT de conteúdo

Um conjunto de todas as Active Entities de um tipo específico num [Content Server](#content-server). Estes snapshots de conteúdo são usados para sincronizar conteúdo entre nós Catalyst.

#### Realm

Quando estiver ligado ao Decentraland, só poderá interagir com Peers ligados ao mesmo Realm. Como nomes amigáveis, os Realms fazem uso de nomes de deuses gregos como Hela ou Loki. Cada realm é determinado por um único [Nó Catalyst](https://decentraland.github.io/catalyst-monitor/).

#### Peer

Um utilizador ligado ao Decentraland.

#### Lighthouse

O [Lighthouse](https://github.com/decentraland/lighthouse) é o nome usado para referir a versão anterior do Communications Service alojado nos nós Catalyst e usado para agrupar utilizadores numa rede P2P. As principais responsabilidades eram determinar quais eram os candidatos para uma ligação P2P e fazer a sinalização WebRTC para estabelecer a ligação. A maior parte desta lógica era feita através de dois componentes externos: o PeerJS Server (conecta [WebRTC](https://webrtc.org/) peers) e Archipelago (recebe as posições dos utilizadores e agrupa-os em ilhas).

#### Backend for Frontend (também conhecido como BFF)

O [BFF](https://github.com/decentraland/explorer-bff) Service foi criado para resolver as necessidades dos clientes e permitir um desenvolvimento mais rápido de novas funcionalidades sem quebrar as APIs existentes. No contexto do Catalyst Communication Service, ficará encarregue de gerir a sinalização P2P e, assim, substituir o Lighthouse.

#### Proprietário do Catalyst

Um [Proprietário do Catalyst](https://github.com/decentraland/catalyst-owner) é um indivíduo ou organização que executa um nó Catalyst. Para adicionar um nó Catalyst à rede DAO, uma [proposta](https://governance.decentraland.org/?type=catalyst) precisa de ser submetida e os nós DAO representam a rede Decentraland de confiança.

#### Serviço Archipelago

O [Serviço Archipelago](https://github.com/decentraland/archipelago-service) anteriormente era uma biblioteca usada pelo Lighthouse. Este serviço terá a mesma responsabilidade que a biblioteca tinha: agrupar peers em clusters para que possam comunicar eficientemente entre si. Cada cluster chama-se uma ilha. Por outro lado, o serviço também precisa de conseguir balancear ilhas usando os transportes de comunicação disponíveis e seguindo um conjunto de [Proprietário do Catalyst](https://github.com/decentraland/catalyst-owner) regras definidas, de modo a, por exemplo, usar LiveKit para uma ilha na scene Casino e P2P numa Genesis Plaza.

#### Ilha

Cluster de peers ligados.

#### Transporte de Comunicação

Transport é a tecnologia usada para partilhar atualizações entre Peers ligados a uma Ilha.

#### Transporte P2P

Usa [WebRTC](https://webrtc.org/) ligações entre Peers, sem necessidade de infraestrutura extra para escalar, uma vez que a carga fica do lado do cliente e os Servers apenas gerem a sinalização. A latência aumenta à medida que mais Peers são adicionados a uma Ilha. Existe um limite para a quantidade de ligações WebRTC que um cliente pode ter; por isso, para ter ilhas maiores, são necessárias mais ligações indiretas (saltos) entre peers.

#### Transporte LiveKit

[LiveKit](https://livekit.io/) é um projeto de código aberto que fornece conferência escalável para vários utilizadores sobre WebRTC. Em vez de fazer uma rede P2P, os Peers são ligados a uma Selective Forwarding Unit (SFU) encarregue de gerir o encaminhamento de mensagens e diferentes aspetos de qualidade da comunicação. As SFUs são a infraestrutura adicional necessária para fornecer comunicações de alto desempenho / alta qualidade entre multidões em scenes designadas.

#### Transporte WebSocket

O [Transporte WebSocket](https://github.com/decentraland/ws-room-service) é mais leve do que LiveKit, também requer um servidor que gere o encaminhamento de mensagens através de WebSocket rooms. Rooms são equivalentes a ilhas. Este transporte oferece menos latência e melhor desempenho do que ilhas P2P, mas também requer a infraestrutura adicional.

#### NATS

[NATS](https://nats.io/) é um broker de mensagens que permite a troca de dados e a comunicação entre serviços. No contexto do Catalyst Server, é usado para comunicar alguns dos serviços internos.

#### DAO

DAO significa "Decentralized Autonomous Organization". O Decentraland DAO é dono dos Smart Contracts e ativos mais importantes que compõem o Decentraland – o LAND Contract, o Estates Contract, Wearables, Content Servers e o Marketplace. Também é proprietário de uma reserva substancial de MANA, o que lhe permite ser verdadeiramente autónomo, bem como subsidiar várias operações e iniciativas em todo o Decentraland.

<https://dao.decentraland.org/en/>


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/catalyst/glossary.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
