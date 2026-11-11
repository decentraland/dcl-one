# Visão geral

Decentraland é uma plataforma de mundo virtual descentralizado construída sobre uma arquitetura híbrida que combina propriedade em blockchain, entrega descentralizada de conteúdo e serviços centralizados para uma experiência de utilizador melhorada.

## Princípios Arquitetónicos

### Descentralização onde importa

* **Propriedade** - LAND, wearables e nomes são NFTs em Ethereum
* **Governança** - A DAO controla smart contracts críticos e o tesouro
* **Conteúdo** - Distribuído entre nós Catalyst
* **Identidade** - Autenticação baseada em carteira (sem palavras-passe)

### Centralização para desempenho

* **Serviços em tempo real** - Comunicações WebSocket, chat de voz
* **Descoberta** - Pesquisa, eventos, conteúdo em destaque
* **Funcionalidades sociais** - Amigos, comunidades, notificações
* **Otimização** - Processamento de assets, geração de imagens

## Camadas do sistema

### Clients

Os jogadores interagem através de vários clients:

* **Unity Explorer** - Client principal para desktop (Windows, Mac, Linux)
* **Web Explorer** - Client baseado no browser
* **CLI** - Ferramentas para developers para criação e deployment de scenes
* **Clients da comunidade** (mobile, VR, etc) - Implementações alternativas do client de referência usando diferentes tecnologias de rendering.

Todos os clients autenticam-se através de carteiras de criptomoedas e seguem os mesmos protocolos.

### Descoberta (Realm Provider)

Ponto de entrada único que fornece aos clients uma descrição de realm:

* Nós Catalyst disponíveis (localização, capacidade, estado de saúde)
* Endereços de serviços de backend

Permite a seleção inteligente do servidor com base na geografia e na carga.

### Conteúdo Descentralizado (Catalyst Network)

Rede sem permissões de nós Catalyst que armazenam e servem:

* Definições de scenes e assets
* Perfis de avatar e wearables

Cada nó valida e armazena conteúdo de forma independente. Os nós sincronizam deployments, mas operam autonomamente. Ver [Catalyst Network](/docs/contributor/contributor-pt/arquitetura/catalyst/) para detalhes.

### Servidores de Funcionalidades Centralizadas

Os microserviços de backend fornecem funcionalidades melhoradas:

* **Descoberta de serviços** - Seleção de realm e monitorização do estado de saúde
* **Comunicações** - Clustering de WebSocket, controlo de acesso a voz/vídeo
* **Funcionalidades de conteúdo** - Worlds, events, places, dados do atlas
* **Funcionalidades sociais** - Amigos, comunidades, notificações
* **Funcionalidades da plataforma** - Conquistas, recompensas, créditos virtuais
* **Processamento de assets** - Otimização, miniaturas, traduções

Veja [Serviços de backend](/docs/contributor/contributor-pt/arquitetura/services/) para documentação abrangente dos serviços.

### Infraestrutura

Sistemas de suporte e serviços de terceiros:

* **Message Broker** - NATS para comunicação entre serviços
* **Media Server** - LiveKit para voz e vídeo
* **Indexadores de blockchain** - Consultas rápidas para dados de NFT
* **Serviço de email** - SendGrid para notificações

Veja [Infraestrutura](/docs/contributor/contributor-pt/arquitetura/infrastructure/) para detalhes.

### Blockchain

Smart contracts em Ethereum e Polygon gerem:

* **Propriedade** - LAND, wearables, nomes, estates
* **Marketplace** - Trading e royalties
* **DAO** - Governança e tesouro
* **Moeda** - Token MANA

## Modelo de segurança

### Identidade e autenticação

* **Assinaturas da carteira** - Sem palavras-passe, autenticação através de assinaturas de Ethereum
* **Servidor de autenticação** - Emite tokens JWT após a verificação da assinatura
* **Fetch assinado** - As APIs autenticam pedidos usando assinaturas da carteira

### Validação de conteúdo

* **Verificação de assinatura** - Todos os deployments são assinados pela carteira
* **Propriedade do Pointer** - Apenas o proprietário do LAND pode fazer deployment nas coordenadas
* **Limites de tamanho** - O tamanho do conteúdo é restringido pelo número de parcels

### Controlo de acesso

* **Tokens com tempo limitado** - Os tokens JWT e LiveKit expiram
* **Limitação de taxa** - Limites de taxa da API por IP/utilizador
* **Proteção contra DDoS** - Rede Catalyst distribuída

## Abordagem de escalabilidade

### Escalabilidade horizontal

* **Nós Catalyst** - Adicionar nós à medida que o conteúdo cresce
* **Serviços de backend** - Microserviços sem estado escalam de forma independente
* **Ilhas** - As comunicações são divididas em grupos baseados na proximidade

### Otimização

* **Pacotes de assets** - Assets otimizados para a plataforma (Unity bundles)
* **CDN** - Assets estáticos servidos através de CDN
* **Cache** - Cache agressiva em todas as camadas
* **Indexadores** - Dados da blockchain em cache em bases de dados rápidas

## Próximos Passos

Aprofunde-se em áreas arquitetónicas específicas:

* [**Serviços de backend**](/docs/contributor/contributor-pt/arquitetura/services/) - Especificações detalhadas de serviços e integrações
* [**Catalyst Network**](/docs/contributor/contributor-pt/arquitetura/catalyst/) - Entrega e deployment de conteúdo
* [**Infraestrutura**](/docs/contributor/contributor-pt/arquitetura/infrastructure/) - Sistemas de suporte e serviços de terceiros

## Recursos Adicionais

* [Referência da API](https://github.com/decentraland/docs/tree/main/apis/README.md) - Documentação completa da API
* [Guias do Contribuinte](/docs/contributor/contributor-pt/guias-para-colaboradores/contributor-guides/) - Normas de desenvolvimento
* [Organização no GitHub](https://github.com/decentraland) - Repositórios de código-fonte


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/arquitetura/architecture.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
