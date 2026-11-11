> Boliche multijogador com física simulada pelo servidor, progressão persistente, atualizações de estado em lote e efeitos de partículas dimensionados para mobile.

# Fastlane Bowling

![Pré-visualização de Fastlane Bowling](/docs/_remote/926145e7ae23468dd849d344.webp)

Um jogo de bowling multijogador com física realista fornecida por Cannon-ES. Vários jogos decorrem em pistas separadas num espaço partilhado.

* [Jogue no Decentraland](https://decentraland.org/jump?realm=stom.dcl.eth)

## Informações do projeto

**Mantido por:** Comunidade · stom66

[Ver fonte upstream](https://github.com/stom66/dcl-bowling)

**Licença:** [CC BY-NC-ND 4.0](https://github.com/stom66/dcl-bowling/blob/main/LICENSE.md). Verifique os termos upstream antes de reutilizar; o código e os assets podem ter termos diferentes.


> [!WARNING]
> **Código mantido pela comunidade — reveja antes de usar**
> 
> Este projeto da comunidade é mantido fora das organizações GitHub da Decentraland Foundation e da DAO. O seu código-fonte e a sua implementação em produção podem mudar de forma independente.
> 
> A inclusão neste catálogo não implica uma auditoria de segurança nem uma garantia de segurança. Reveja o código, as dependências e os scripts de instalação antes de o executar ou reutilizar.
> 
> [Reportar uma preocupação no catálogo](https://github.com/decentraland/docs/issues/new). Para vulnerabilidades suspeitas, siga a política de segurança do projeto upstream. Não inclua detalhes da exploração nem credenciais em issues públicas.


## Principais funcionalidades

* [**Cannon-ES**](https://github.com/pmndrs/cannon-es) **física** no Multiplayer Server, com keyframes comprimidos reproduzidos pelos clients.
* **Armazenamento** para progressão do jogador, estatísticas, preferências, inventário e tabelas classificativas semanais/de todos os tempos.
* Dinâmico **luzes** com máscaras de sombra rotativas e holofotes que seguem a bola.
* **Partículas** e rastos de desbloqueio, com contagens de partículas reduzidas em mobile.
* **Câmeras virtuais** para o fly-in de chegada, a mira e a reprodução da rolagem.
* **`GltfNodeModifiers`** para deslocar os UVs da textura neon.
* controlado pelo SDK **animações glTF** para os bumpers da pista, contagens decrescentes e efeitos de pontuação.

## Explorar o código

Estes links apontam para um snapshot específico da source para que as explicações continuem úteis à medida que o desenvolvimento upstream avança. São referências de aprendizagem, não uma avaliação de segurança.

### Agrupar alterações de estado sincronizadas em eventos do client

Em `LaneWatcher.onLaneChanged`, um conjunto recolhe as pistas alteradas e agenda um único flush. `flush` lê o snapshot de cada pista e emite eventos de estado e de transição de fase. Siga este fluxo para entender como a cena liga componentes sincronizados às reações do client.

**Contexto do projeto:** Leia-o juntamente com as definições do projeto `LaneStore` e as definições do event-bus.

**Referências de origem:**

* [`LaneWatcher.onLaneChanged`](https://github.com/stom66/dcl-bowling/blob/25c5a7f9152835c190d9d20fa18b6010d0c6ce50/dcl/src/client/laneWatcher.ts#L79-L90)
* [`flush`](https://github.com/stom66/dcl-bowling/blob/25c5a7f9152835c190d9d20fa18b6010d0c6ce50/dcl/src/client/laneWatcher.ts#L132-L153)

### Dimensionar efeitos de partículas para dispositivos móveis

`getParticleRatio` e `scaleConfig` ajustar as taxas de emissão, os limites de partículas e as contagens de rajada usando um rácio para dispositivos móveis, preservando a configuração original. `gerar` aplica essa configuração a um novo emissor, com parenting opcional e limpeza temporizada.

**Contexto do projeto:** Isto é útil ao estudar como uma configuração de efeito pode servir diferentes dispositivos.

**Referências de origem:**

* [`getParticleRatio` e `scaleConfig`](https://github.com/stom66/dcl-bowling/blob/25c5a7f9152835c190d9d20fa18b6010d0c6ce50/dcl/src/client/particleSpawner.ts#L176-L218)
* [`gerar`](https://github.com/stom66/dcl-bowling/blob/25c5a7f9152835c190d9d20fa18b6010d0c6ce50/dcl/src/client/particleSpawner.ts#L91-L121)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/tutoriais-e-exemplos/published-games/fastlane-bowling.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
