> Rodadas multijogador de captura de pacotes com classificações persistentes, projéteis em pool e posicionamento de pickups semeado a partir do estado da rodada.

# Sky Chaser

![Pré-visualização de Sky Chaser](/docs/_remote/3033913f0028e180a375c5bf.webp)

Um jogo multijogador em que os players competem para apanhar pacotes em fuga em cada rodada. A scene inclui tabelas de classificação semanais e de todos os tempos.

* [Jogue no Decentraland](https://decentraland.org/jump?realm=skychaser.dcl.eth)

## Informações do projeto

**Mantido por:** Comunidade · stom66

[Ver fonte upstream](https://github.com/stom66/dcl-sky-chaser)

**Licença:** [CC BY-NC-ND 4.0](https://github.com/stom66/dcl-sky-chaser/blob/main/LICENSE.md). Verifique os termos upstream antes de reutilizar; o código e os assets podem ter termos diferentes.


> [!WARNING]
> **Código mantido pela comunidade — reveja antes de usar**
> 
> Este projeto da comunidade é mantido fora das organizações GitHub da Decentraland Foundation e da DAO. O seu código-fonte e a sua implementação em produção podem mudar de forma independente.
> 
> A inclusão neste catálogo não implica uma auditoria de segurança nem uma garantia de segurança. Reveja o código, as dependências e os scripts de instalação antes de o executar ou reutilizar.
> 
> [Reportar uma preocupação no catálogo](https://github.com/decentraland/docs/issues/new). Para vulnerabilidades suspeitas, siga a política de segurança do projeto upstream. Não inclua detalhes da exploração nem credenciais em issues públicas.


## Principais funcionalidades

* Servidor multiplayer
* Jogabilidade sincronizada via RNG Mulberry32 sincronizado
* API de armazenamento para tabelas de classificação
* Ajustes de locomoção do player e forças físicas para booster/trampolines/explosões
* Sistemas de partículas (rastros de boost, explosões, pickups)

## Explorar o código

Estes links apontam para um snapshot específico da source para que as explicações continuem úteis à medida que o desenvolvimento upstream avança. São referências de aprendizagem, não uma avaliação de segurança.

### Reutilizar entidades de projéteis durante o jogo

`fireProjectile` e `getIdleProjectile` reutilizar projéteis inativos, criar novos quando o pool se esgota e atualizar apenas os projéteis ativos.

**Contexto do projeto:** Siga o projeto `Projétil` classe e handlers de eventos para movimento, respostas a colisões e limpeza ao final da rodada.

**Referências de origem:**

* [`fireProjectile` e `getIdleProjectile`](https://github.com/stom66/dcl-sky-chaser/blob/e546854d2a2634ca5e28635c8b9be124a173f6dc/dcl/src/client/projectileManager.ts#L76-L124)

### Semear o posicionamento dos pickups a partir do estado da rodada

`onGameStart` e `getRandomPosition` use o horário de início da rodada para inicializar o gerador aleatório do projeto e, em seguida, amostrar as distâncias e alturas dos pickups.

**Contexto do projeto:** Leia a configuração dos pickups e o component store juntamente com ela: limites móveis e a atividade posterior dos pickups afetam a sequência de chamadas aleatórias, então uma seed partilhada por si só não garante layouts idênticos em cada client.

**Referências de origem:**

* [`onGameStart` e `getRandomPosition`](https://github.com/stom66/dcl-sky-chaser/blob/e546854d2a2634ca5e28635c8b9be124a173f6dc/dcl/src/client/spawners/pickupSpawner.ts#L115-L164)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/tutoriais-e-exemplos/published-games/sky-chaser.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
