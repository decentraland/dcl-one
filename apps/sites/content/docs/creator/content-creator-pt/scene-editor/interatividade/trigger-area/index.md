> Reaja à posição do jogador

# Área de gatilho

Para fazer algo acontecer quando o jogador entra ou sai de uma área, use um Trigger Area [smart item](/docs/creator/content-creator-pt/scene-editor/interatividade/smart-items/).

![](/docs/_remote/9dcff76b0c7616640d8295ac.webp)

O cubo laranja que você vê enquanto edita sua scene só fica visível no Scene Editor; ele fica invisível quando você executa uma Preview da scene. Você pode ajustar e redimensionar facilmente o cubo laranja para cobrir exatamente a área de que precisa.

Arraste-o para a sua scene e redimensione-o com o gizmo de escala. A caixa ou esfera que você vê só é desenhada no editor; os players nunca a veem.

Se qualquer parte do avatar do jogador se sobrepuser a este cubo laranja, o evento atribuído será chamado. As Trigger Areas reagem apenas ao avatar do jogador na máquina local, não aos avatares de outros players — cada player dispara o trigger na sua própria instância da scene.


> [!NOTE]
> **💡 Dica**: Se as Trigger Areas da sua scene começarem a atrapalhar a edição de outro conteúdo, lembre-se de que você sempre pode bloqueá-las e/ou ocultá-las da [Entity Tree](/docs/creator/content-creator-pt/scene-editor/primeiros-passos/scene-editor-essentials/#the-entity-tree).
> 
> <img src="/docs/_remote/c1baca1d0d660705a098e478.webp" alt="" data-size="original">


Várias Trigger Areas podem se sobrepor e não afetam umas às outras.

Você descreve o que deve acontecer e o [AI Assistant](/docs/creator/content-creator-pt/scene-editor/expandir-com-codigo/ai-assistant/) escreve um pequeno script para isso.

### Definições

Do item **Script** componente tem dois campos:

| Campo           | O que faz                              | Opções                                                                                                                        | Padrão        |
| --------------- | -------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- | ------------- |
| **Ativado por** | O que dispara a área.                  | `meu jogador`, `todos os jogadores`, `qualquer collider`, `clicável`, `personalizado 1`, `personalizado 2`, `personalizado 3` | `meu jogador` |
| **Forma**       | Se o volume é uma caixa ou uma esfera. | `box`, `sphere`                                                                                                               | `box`         |

`meu jogador` reage apenas ao avatar do jogador que está executando a scene. `todos os jogadores` reage a qualquer avatar, incluindo outras pessoas na scene. As opções restantes correspondem a outras [camadas de colisão](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/colliders/), então você pode fazer a área reagir a objetos físicos, a objetos clicáveis ou às suas próprias camadas personalizadas.

Alternar **Forma** alterna o placeholder do editor entre uma caixa e uma esfera, então o que você vê ao editar é o volume que você obtém.

### Descreva o que deve acontecer

A área dispara dois eventos: **entrar** e **sair**. Na **Reactions** seção do componente Script há um botão para cada um:

* **Quando um jogador entra…**
* **Quando um jogador sai…**

Clique em um e o Creator Hub escreve o início de uma frase na caixa de mensagens do AI Assistant, nomeando o item, por exemplo `Quando um jogador entra em "Trigger Area",` . O prompt usa o nome do item que você vê na entity tree. Termine a frase com o que você quer que aconteça e envie-a. O assistente escreve um script separado e o anexa à mesma entity, junto com o detector.

Os três chips ao lado dos botões (**Tocar um som**, **Mostrar uma mensagem**, **Marcar pontos**) preenchem uma frase completa para você, então você pode enviá-las como estão.


> [!NOTE]
> **💡 Dica**: As Reactions precisam do [AI Assistant](/docs/creator/content-creator-pt/scene-editor/expandir-com-codigo/ai-assistant/) para ser ativado. Se não estiver, o Creator Hub informa onde ativá-lo em vez de abrir o chat.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scene-editor/interatividade/trigger-area.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
