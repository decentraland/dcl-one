> Comportamentos de NPC reutilizáveis e UI de diálogo para cenas SDK7.

# NPC Toolkit

Use comportamentos de NPC reutilizáveis e UI de diálogo para cenas SDK7.

## Informações do projeto

**Mantido por:** Decentraland

[Ver código-fonte](https://github.com/decentraland-scenes/dcl-npc-toolkit)

**Licença:** [Apache-2.0](https://github.com/decentraland-scenes/dcl-npc-toolkit/blob/main/LICENSE). Verifique os termos upstream antes de reutilizar; o código e os assets podem ter termos diferentes.

## Explorar o código

Estes links apontam para um snapshot específico da source para que as explicações continuem úteis à medida que o desenvolvimento upstream avança. São referências de aprendizagem, não uma avaliação de segurança.

### Separe a revelação do diálogo do seu avanço

`handleDialogClick`, `rushText`e `confirmText` primeiro revele os caracteres restantes se o texto ainda estiver sendo digitado. A entrada subsequente avança o script ou seleciona uma ramificação numérica do diálogo, com callbacks para ações do jogo.

**Contexto do projeto:** Leia isto em conjunto com o `Dialog` dados e `npcDialogComponent` estado para entender como a entrada da UI conduz uma conversa.

**Referências de origem:**

* [`handleDialogClick`, `rushText`e `confirmText`](https://github.com/decentraland-scenes/dcl-npc-toolkit/blob/60e327c67a126d92e47ecf8e7eda342ce1191076/src/dialog.ts#L489-L549)

### Conecte a entrada de proximidade e do Pointer a uma interação com um NPC

`addClickReactions` e `addTriggerArea` configure callbacks de Pointer e um trigger de esfera filho, com definições para ativação apenas por clique e para continuar o diálogo quando o jogador se afasta. `ativar` e `endInteraction` coordene o NPC ativo, o cooldown, o diálogo e o comportamento opcional voltado ao jogador.

**Contexto do projeto:** Estas funções dependem dos mapas de callback e do sistema de rastreamento do toolkit.

**Referências de origem:**

* [`addClickReactions` e `addTriggerArea`](https://github.com/decentraland-scenes/dcl-npc-toolkit/blob/60e327c67a126d92e47ecf8e7eda342ce1191076/src/npc.ts#L410-L531)
* [`ativar` e `endInteraction`](https://github.com/decentraland-scenes/dcl-npc-toolkit/blob/60e327c67a126d92e47ecf8e7eda342ce1191076/src/npc.ts#L769-L823)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/tutoriais-e-exemplos/ui-camera-player/npc-toolkit.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
