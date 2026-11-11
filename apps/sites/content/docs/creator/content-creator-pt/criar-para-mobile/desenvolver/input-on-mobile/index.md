> Como as ações de entrada mapeiam para toques no client mobile do Decentraland.

# Entrada no Celular

O sistema de entrada do Decentraland foi projetado para ser agnóstico em relação ao dispositivo. O mesmo `InputAction` enum que o SDK expõe no desktop também é encaminhado pelos controles na tela no mobile, então a maioria das cenas funciona sem nenhuma alteração. Há, no entanto, algumas regras e pegadinhas que vale a pena conhecer ao desenvolver para toque.

Para o modelo de entrada completo, veja [Eventos de clique](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/click-events/). Esta página foca no que é específico do mobile.

<figure><img src="/docs/_remote/d29f6b96a1d1db9049d689d6.webp" alt="The on-screen controls with the + menu expanded, showing every input button"><figcaption><p>Os controles na tela com o menu "+" expandido — todas as ações de entrada visíveis</p></figcaption></figure>

## A que o toque corresponde

Todo `InputAction` que existe no desktop também está disponível no client mobile — os controles na tela simplesmente encaminham o toque para as mesmas entradas do SDK. Isso significa que qualquer cena que dependa dos `InputAction` valores padrão funcionará no mobile sem precisar de ajustes.

Os controles na tela mapeiam-se da seguinte forma:

* **Joystick na tela** — controla `IA_FORWARD` / `IA_BACKWARD` / `IA_LEFT` / `IA_RIGHT` e o movimento do avatar.
* **Botão de interação (canto inferior direito)** — dispara `IA_POINTER` (o botão esquerdo do mouse no desktop) contra aquilo para o qual o jogador estiver mirando.
* **Botão E** — dispara `IA_PRIMARY` (a `E` tecla no desktop).
* **Botão F** — dispara `IA_SECONDARY` (a `F` tecla no desktop). Oculto por padrão (veja abaixo).
* **Botão de salto** — dispara `IA_JUMP` (a `Espaço` tecla no desktop). Este é o maior botão do HUD e é sempre fácil de alcançar.
* **Botões 1 / 2 / 3 / 4** — disparam `IA_ACTION_3` / `IA_ACTION_4` / `IA_ACTION_5` / `IA_ACTION_6` respectivamente. Ocultos por padrão (veja abaixo).
* **Arrastar a câmera** — gira a câmera; não é exposto como um `InputAction`.

## Personalização dos controles

Cenas podem reconfigurar os controles na tela — ocultar o joystick ou a mira, ocultar qualquer botão (incluindo o de salto), alterar o que o botão central faz, trocar os ícones dos botões ou substituir os controles nativos completamente por UI personalizada. Veja [Controles na tela](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/touch-screen-controls/) e [Vinculação de entrada da UI](/docs/creator/content-creator-pt/scenes-sdk7/ui-2d/ui_input_binding/).

<figure><img src="/docs/_remote/c911e3ac3460b2381dd7c1b5.webp" alt="Customized on-screen controls"><figcaption><p>Um HUD personalizado: um botão central com ícone alterado mapeado para uma ação diferente</p></figcaption></figure>

## Entradas a evitar para ações principais no mobile

Todo `InputAction` podem ser encaminhadas para um botão na tela, mas uma scene construída com SDK 7.32.0 ou posterior começa com apenas três delas visíveis: o botão de interação, o botão E e o botão de salto. `IA_SECONDARY` (F) e `IA_ACTION_3`–`IA_ACTION_6` (a `1`/`2`/`3`/`4` botões) estão **ocultos por padrão**, então uma ação de tecla vinculada a um deles não tem botão no telefone a menos que sua scene o mostre. Em SDKs mais antigos, todos os oito botões são exibidos, com os de menor prioridade atrás de um overflow "+" que não é facilmente acessível durante a jogabilidade.

A menos que sua scene os mostre explicitamente com [Controles na tela](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/touch-screen-controls/), evite vincular ações de tecla a:

* `IA_SECONDARY` — a `F` tecla no desktop / `F` botão no mobile
* `IA_ACTION_3` — a `1` tecla no desktop / `1` botão no mobile
* `IA_ACTION_4` — a `2` tecla no desktop / `2` botão no mobile
* `IA_ACTION_5` — a `3` tecla no desktop / `3` botão no mobile
* `IA_ACTION_6` — a `4` tecla no desktop / `4` botão no mobile

Prefira em vez disso:

* `IA_POINTER` para o botão principal de toque / interação
* `IA_PRIMARY` para a ação do botão E
* `IA_JUMP` quando uma ação se alinha naturalmente com pular (é o maior e mais acessível botão no HUD do mobile)
* Triggers baseados em proximidade (veja [Eventos de proximidade](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/proximity-events/)) quando uma ação deve ser disparada automaticamente à medida que o jogador se aproxima de uma Entity

## Bloqueio do cursor

O [`PointerLock` componente](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/click-events/#lock-or-unlock-the-cursor) é um conceito do client desktop (cursor do mouse bloqueado vs. desbloqueado). Não se aplica ao toque no mobile e é seguro deixá-lo na sua scene — ele não tem efeito lá.

## Relacionado

* [Controles na tela](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/touch-screen-controls/)
* [Vinculação de entrada da UI](/docs/creator/content-creator-pt/scenes-sdk7/ui-2d/ui_input_binding/)
* [Eventos de clique](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/click-events/)
* [Eventos de proximidade](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/proximity-events/)
* [Detectar a plataforma pelo código](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/detect-platform/)
* [Melhores práticas de UI para mobile](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/ui-best-practices/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/criar-para-mobile/desenvolver/input-on-mobile.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
