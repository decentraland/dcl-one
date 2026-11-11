> Limpeza cooperativa de uma boate com polling de components compartilhados e props anexados ao avatar que refletem os itens carregados.

# Clean The Club

![Pré-visualização de Clean The Club](/docs/_remote/a8f103a70354f2dfa74d73d7.webp)

Um jogo cooperativo de limpeza em que os jogadores competem para restaurar uma boate antes da próxima festa.

* [Jogue no Decentraland](https://decentraland.org/jump?realm=cleantheclub.dcl.eth)

## Informações do projeto

**Mantido por:** Community · CleanTheClub

[Ver código-fonte upstream](https://github.com/CleanTheClub/CleanTheClub)

**Licença:** [Apache-2.0](https://github.com/CleanTheClub/CleanTheClub/blob/main/LICENSE). Verifique os termos upstream antes de reutilizar; o código e os assets podem ter termos diferentes.


> [!WARNING]
> **Código mantido pela comunidade — revise antes de usar**
> 
> Este projeto da comunidade é mantido fora das organizações GitHub da Decentraland Foundation e da DAO. Seu código-fonte e a implantação ao vivo podem mudar de forma independente.
> 
> A inclusão neste catálogo não implica uma auditoria de segurança nem garante a segurança. Revise o código, as dependências e os scripts de instalação antes de executar ou reutilizá-lo.
> 
> [Reportar uma preocupação no catálogo](https://github.com/decentraland/docs/issues/new). Para vulnerabilidades suspeitas, siga a política de segurança do projeto upstream. Não inclua detalhes da exploração nem credenciais em issues públicas.


## Explorar o código

Estes links apontam para um instantâneo específico do código-fonte para que as explicações continuem úteis à medida que o desenvolvimento upstream avança. Eles são referências de aprendizado, não uma avaliação de segurança.

### Compartilhe uma única varredura de Component entre vários Systems

onClutterPoll coleta entradas do ClutterSync uma vez por intervalo de sondagem, cria uma tabela de consulta por ID do item e passa o mesmo snapshot aos subscribers. Cada consumidor pode fazer sua própria filtragem e detecção de alterações.

**Contexto do projeto:** Isso é útil para estudar como evitar varreduras repetidas de ECS; depende do schema compartilhado e do intervalo de sondagem do phase-gate, e as subscriptions devem ficar na inicialização do client.

**Referências do código-fonte:**

* [onClutterPoll](https://github.com/CleanTheClub/CleanTheClub/blob/8ffc6c1fa24ff0da22a1d82ecdf2f9ce72f47f73/src/client/clutterWatcher.ts#L15-L54)

### Mostrar o equipamento carregado de outro jogador

updateRemoteCarry anexa uma hierarquia de prop à mão esquerda do outro avatar, seleciona um model a partir do gear atual ou do estado de transporte e dimensiona a bolsa pela sua fração de preenchimento. Também remove a hierarquia quando não há nada a exibir.

**Contexto do projeto:** Leia isto em conjunto com os handlers de mensagens de carry, a consulta aos models do tema e os offsets de ajuste do model: eles fornecem o estado e o posicionamento específico do asset.

**Referências do código-fonte:**

* [updateRemoteCarry](https://github.com/CleanTheClub/CleanTheClub/blob/8ffc6c1fa24ff0da22a1d82ecdf2f9ce72f47f73/src/client/carrySystem.ts#L805-L846)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/tutoriais-e-exemplos/published-games/clean-the-club.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
