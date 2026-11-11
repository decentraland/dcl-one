# Infraestrutura

Esta página cobre a infraestrutura de suporte e os serviços externos que alimentam a Decentraland.

## Serviços Externos (SaaS)

### LiveKit

**Propósito**: servidor de media de alta qualidade para voz e vídeo

[LiveKit](https://livekit.io/) fornece:

* roteamento de mídia WebRTC
* chat de voz
* videoconferência
* partilha de ecrã
* áudio espacial (em desenvolvimento)

**Controlo de acesso**: [Comms Gatekeeper](https://github.com/decentraland/docs/tree/main/apis/apis/comms-gatekeeper/README.md) gera tokens com tempo limitado

**Usado por**:

* Archipelago Workers - voz padrão
* Worlds - canais de voz por World
* Events - canais específicos por evento

### SendGrid

**Propósito**: serviço de entrega de emails

Envia emails de notificação:

* Pedidos de amizade
* Lembretes de eventos
* Notificações do sistema

**Acionado por**: [sistema de notificações](https://github.com/decentraland/docs/tree/main/apis/apis/notifications-workers/README.md)

***

## Indexadores de blockchain

Vários serviços de indexação fornecem consultas rápidas em blockchain:

### Squids

**Propósito**: indexador blockchain personalizado

[Subsquid](https://www.sqd.ai/) baseados em:

* Modelos de dados personalizados
* Tempos de sincronização rápidos
* GraphQL API

**Índices**:

* Parcelas de terra (NFTs LAND)
* Wearables e coleções
* atividade do Marketplace
* nomes ENS

### Satsuma

**Propósito**: fornecedor hospedado de GraphQL

Hospedagem gerida de subgraphs:

* Alta disponibilidade
* Escalonamento automático
* Monitorização incluída

### The Graph

**Propósito**: protocolo de indexação descentralizado

[The Graph](https://thegraph.com/) rede:

* indexadores descentralizados
* Taxas de consulta em GRT
* Operado pela comunidade

***

## Message Broker

### NATS

**Propósito**: roteamento de mensagens pub/sub

[NATS](https://nats.io/) fornece:

* Entrega de mensagens em tempo real
* Padrões pub/sub
* Padrões request/reply
* Suporte a clustering

**Usado por**:

| Serviço        | Propósito               |
| -------------- | ----------------------- |
| Archipelago    | Atualizações de posição |
| Serviço social | Estado de amizade       |
| Worlds         | Eventos de World        |

***

## Processamento de media

### Serviço de conversão de imagens (Metamorph)

**Propósito**: conversão e otimização de formatos de imagem

Converte imagens entre formatos:

* Otimização de WebP
* Geração de miniaturas
* Conversão de formato

**GitHub**: [metamorph](https://github.com/decentraland/metamorph)

### Sistema de imagens de perfil

**Propósito**: geração de miniaturas de Avatar

Gera imagens de pré-visualização de Avatar:

* Fotos de rosto
* Fotos de corpo inteiro
* Múltiplas resoluções

**Componentes**:

* **Consumidor de imagens de perfil** - Renderiza avatars
* **Produtor de perfil** - Escuta alterações de avatar

***

## Workers de processamento de assets

### Asset Bundle Converter

**Propósito**: otimização de assets específica da plataforma

Converte assets implantados em bundles otimizados para todas as plataformas.

**Regista bundles em**: [Registo de Asset Bundle](https://github.com/decentraland/docs/tree/main/apis/apis/asset-bundle-registry/README.md)

### Gerador de LODs

**Propósito**: variantes de nível de detalhe

Gera versões com menos polígonos de modelos 3D para melhor desempenho.

***

## Sistema de recompensas

### API de recompensas

**Propósito**: distribuição de recompensas NFT

Concede recompensas NFT por ações:

* Conclusão do onboarding
* Participação em eventos
* Indicações

***

## Próximos Passos

* Revisar [Serviços de backend](/docs/contributor/contributor-pt/arquitetura/services/) para especificações de serviço
* Verifique [Catalyst Network](/docs/contributor/contributor-pt/arquitetura/catalyst/) para infraestrutura de conteúdo
* Veja [Visão Geral da Arquitetura](/docs/contributor/contributor-pt/arquitetura/architecture/) para o panorama completo


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/arquitetura/infrastructure.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
