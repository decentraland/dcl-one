> Corrida em três faixas com cenário reciclado, fantasmas de recorde baseados em distância e dados persistentes de leaderboard.

# Prime Drive

![Pré-visualização do Prime Drive](/docs/_remote/bba6ea4b5469647296b97b46.webp)

Um jogo de corrida de bicicletas com três faixas, em que os jogadores desviam de obstáculos, coletam moedas e perseguem o fantasma do recorde da pista ao longo de um percurso de 10 km.

* [Jogue no Decentraland](https://decentraland.org/jump?realm=primedrive.dcl.eth)

## Informações do projeto

**Mantido por:** Comunidade · carlosmu

[Ver fonte upstream](https://github.com/carlosmu/dcl-prime-drive)

**Licença:** Não identificado neste catálogo. Consulte o repositório upstream e o mantenedor para os termos aplicáveis antes de reutilizar.


> [!WARNING]
> **Código mantido pela comunidade — reveja antes de usar**
> 
> Este projeto da comunidade é mantido fora das organizações GitHub da Decentraland Foundation e da DAO. O seu código-fonte e a sua implementação em produção podem mudar de forma independente.
> 
> A inclusão neste catálogo não implica uma auditoria de segurança nem uma garantia de segurança. Reveja o código, as dependências e os scripts de instalação antes de o executar ou reutilizar.
> 
> [Reportar uma preocupação no catálogo](https://github.com/decentraland/docs/issues/new). Para vulnerabilidades suspeitas, siga a política de segurança do projeto upstream. Não inclua detalhes da exploração nem credenciais em issues públicas.


## Explorar o código

Estes links apontam para um snapshot específico da source para que as explicações continuem úteis à medida que o desenvolvimento upstream avança. São referências de aprendizagem, não uma avaliação de segurança.

### Crie uma corrida longa com cenário reciclado

`setWorldOffset` e `scrollTrack` move um shared world root lateralmente para mudanças de faixa e reposicione as decorações da estrada adiante depois que passarem pelo jogador. A bicicleta permanece no lugar enquanto o cenário transmite a sensação de movimento.

**Contexto do projeto:** Leia os ring builders e shared `PISTA`/`SPAWN` configurações para entender o espaçamento e as distâncias de repetição.

**Referências de origem:**

* [`setWorldOffset` e `scrollTrack`](https://github.com/carlosmu/dcl-prime-drive/blob/d3767141bc39ccef031e56ff9640592ecb349645/src/client/game/track.ts#L43-L92)

### Mostre fantasmas do recorde em relação ao jogador

`updateGhosts` coloca as bicicletas-fantasma conforme sua diferença de distância, oculta os fantasmas fora do alcance de visualização e os move suavemente para faixas afastadas do jogador.

**Contexto do projeto:** Este módulo consome `GhostRunView` dados; ele exibe o progresso fornecido e não grava uma repetição por conta própria.

**Referências de origem:**

* [`updateGhosts`](https://github.com/carlosmu/dcl-prime-drive/blob/d3767141bc39ccef031e56ff9640592ecb349645/src/client/game/ghost.ts#L129-L183)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/tutoriais-e-exemplos/published-games/prime-drive.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
