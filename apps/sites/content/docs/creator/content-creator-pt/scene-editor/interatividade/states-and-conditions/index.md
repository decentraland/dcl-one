> Gerenciando estados de itens e lógica condicional

# Estados e condições



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/wm8ZD2kSyKA" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=wm8ZD2kSyKA>">Watch on YouTube</a></p>



## Lógica condicional

Adicione condições a um trigger, para que a ação só ocorra se essas condições forem cumpridas. Por exemplo, clicar numa porta só ativa a ação "Open" se ela ainda não estiver aberta.

Para adicionar uma condição, clique no ícone dos três pontos ao lado de **evento do trigger** e selecione **Add Trigger Condition**.

![](/docs/_remote/39d3afce89ca9fad61d21f61.webp)

Um único trigger pode incluir várias condições. Clique no **+** ícone para adicionar mais condições. Quando existir mais do que uma condição, podes selecionar uma destas opções:

* **All Conditions should be met (AND)**: O trigger só acontece se todas as condições forem verdadeiras.
* **Any Condition can be met (OR)** O trigger acontece se pelo menos uma das condições for verdadeira.

![](/docs/_remote/95e6cb8369815d94d5f26f5a.webp)

### Estados

A **Estados** component está incluído em vários smart items. Lista os possíveis estados em que o smart item pode estar. A qualquer momento, o smart item está num destes estados. Por exemplo, uma porta pode estar *Open* ou *Closed*. A ação Open define o estado para *Open*, a ação Close define o estado para *Closed*.

Podes fazer as seguintes coisas com estados:

1. Usa uma condição num trigger para verificar o estado de uma entidade. Desta forma, a ação só é executada se um estado específico estiver ativo.

![](/docs/_remote/39d3afce89ca9fad61d21f61.webp)

2. Alterar um estado através da **Set State** .

![](/docs/_remote/6f9bc751de8405306a6f58e0.webp)

3. Reagir a alterações de estado através do **On State Change** evento de trigger.

Para alternar entre duas ações, define dois triggers, cada um com uma condição que verifica um estado. Por exemplo, as portas têm um trigger que ativa a ação Open, com uma condição que primeiro verifica que o estado da porta é *Closed*, e outro trigger que ativa a ação Close, com uma condição que verifica que o estado da porta é *Open*. Apenas uma das duas é ativada cada vez que o jogador clica na porta.

![](/docs/_remote/819921eca04a4f69ab10b587.webp)

Podes adicionar tantos estados quantos quiseres a um smart item. Basta clicar no **Add New State** botão para adicionar outro à lista.

![](/docs/_remote/fb464b1832beb2b74eb0d3e9.webp)

Um dos estados é selecionado como o predefinido; o item começará sempre neste estado quando a scene for executada. Podes atribuir outro estado como predefinido clicando nos três pontos ao lado de outro dos estados e selecionando **Set as Default**.


> [!NOTE]
> **💡 Dica**: Mantém as interações entre items simples. Por exemplo, evita cenários como ter um botão que abre uma porta ao desencadear três ações: reproduzir a animação da porta, reproduzir o som da porta e alterar o estado da porta. Em vez disso, faz com que o botão altere o estado da porta. Depois usa um **On State Change** trigger para que a própria porta trate de reproduzir a animação e o som sempre que o estado mudar.


### Counter

Use o **Counter** component para acompanhar um número, que pode mudar à medida que o jogador realiza ações na scene. Podes usar os valores do contador na lógica condicional.

Quando uma entidade tem um componente Counter, podes executar as seguintes ações sobre ela:

* **Increment Counter**: Incrementa o valor do contador por uma **Quantidade**, por omissão 1.
* **Decrease Counter**: Diminui o valor do contador por uma **Quantidade**, por omissão 1.
* **Set Counter**: Define o valor do contador para um número específico, por exemplo para o voltar a 0.

Use o **On Counter Change** trigger para executar uma ação sempre que o valor do contador muda. Adiciona uma condição a este trigger para que só seja ativado depois de ultrapassar um determinado limite.

![](/docs/_remote/c4189a0d754c03116088f69e.webp)

Numa condição, podes verificar se o valor do contador é

* Maior do que um valor dado
* Menor do que um valor dado
* Igual a um valor dado


> [!NOTE]
> **💡 Dica**: Para verificar se é maior ou igual, podes adicionar duas condições ao evento do trigger, usando a opção AND.
> 
> Para fazer com que uma ação ocorra apenas uma vez ao ultrapassar um limite, e não se repita em cada incremento depois disso, combina o contador com um **State** component. Define o State como "Done" sempre que atingires o valor desejado, e adiciona uma condição para verificar este estado no evento do trigger.


### Ver também

* [Smart items - Noções básicas](/docs/creator/content-creator-pt/scene-editor/interatividade/smart-items/)
* [Smart items - Avançado](/docs/creator/content-creator-pt/scene-editor/interatividade/smart-items-advanced/)
* [Transformar qualquer item em smart item](/docs/creator/content-creator-pt/scene-editor/interatividade/make-any-item-smart/)
* [Combine com código](/docs/creator/content-creator-pt/scene-editor/expandir-com-codigo/overview/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scene-editor/interatividade/states-and-conditions.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
