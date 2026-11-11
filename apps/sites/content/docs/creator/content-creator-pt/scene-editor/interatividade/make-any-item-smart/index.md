> Configure qualquer item para se comportar como um smart item.

# Torne qualquer item smart



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/wnnEU8GCLjc" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=wnnEU8GCLjc>">Watch on YouTube</a></p>



Smart items são apenas itens normais com um **Action** e/ou **Trigger** component. Você pode adicionar estes components a qualquer item na sua scene. Você também pode importar os seus próprios modelos 3D personalizados e adicionar o mesmo a eles.

Para adicionar components a um item, clique no **Plus Icon** ao lado do nome do item e selecione qual component adicionar na lista suspensa.

![O menu de adicionar component abre-se numa entity, listando components agrupados em Conteúdo 3D e Interação, incluindo Actions e Triggers.](/docs/_remote/e0e52c0c060b63e97e2466dd.webp)

Isto permite uma enorme quantidade de possibilidades criativas. Transforme uma vela numa alavanca que abre uma passagem secreta atrás de uma estante de livros, reproduza sons misteriosos vindos de dentro de um poço, transforme diamantes em itens colecionáveis que encolhem até 0 quando clicados. Há toneladas de formas imaginativas de combinar estas mecânicas!

## Interatividade

Você pode fazer um item reagir a diferentes ações do jogador.


> [!NOTE]
> **💡 Dica**: Quando um jogador interage com um item, você deve mostrar algum tipo de feedback para deixar essa interação clara. Se o modelo não tiver animações, considere pelo menos reproduzir um som. Em alguns casos, pode funcionar fazer o item realizar um ligeiro tween de escala e depois retornar à sua escala original, como uma forma de feedback.


Adicione um **Trigger** component, para detetar diferentes ações do jogador:

* **Pointer events**: Quando o jogador clica ou pressiona uma tecla enquanto aponta o cursor para o item.
* **Eventos globais de botão** Quando o jogador pressiona uma tecla, em qualquer ponto da scene.
* **Proximidade do jogador**: Quando o jogador entra na posição do item.

A **Trigger** component pode ser configurado para reconhecer qualquer um destes tipos de triggers. Sempre que um trigger acontece, pode chamar Actions do seu próprio **Actions** component, ou a partir dos **Actions** components de outros itens na scene. Veja [Smart items - Avançado](/docs/creator/content-creator-pt/scene-editor/interatividade/smart-items-advanced/).


> [!NOTE]
> **💡 Dica**: Também pode combinar estes triggers com [lógica condicional](/docs/creator/content-creator-pt/scene-editor/interatividade/states-and-conditions/), para que as actions não sejam chamadas sempre que o trigger acontece, apenas se as condições forem verdadeiras.
> 
> Por exemplo, poderia adicionar um **Pointer Events** trigger a uma porta, para que ela abra quando clicada, mas inclua lógica condicional para que só abra se estiver destrancada.


### Pointer events

Adicione um **Trigger** component com **On Click** ou **On Input Action** Eventos de Trigger.

* **On Click** reage sempre que o jogador clica no botão esquerdo do rato enquanto aponta para o item.
* **On Input Action** reage sempre que o jogador pressiona o Primary Button (E) enquanto aponta para o item.

![](/docs/_remote/342cba6acbd2ae9ce5e8ce5f.webp)

**Colliders**

É importante que, para um item ser clicável, ele tenha um **Collider**. Caso contrário, os seus cliques irão atravessar o modelo e tentar interagir com o que estiver atrás. Os modelos 3D nos Asset Packs predefinidos devem todos ter colliders, mas, se criar o seu próprio modelo ou o obter de outro lugar, pode descobrir que lhe falta um.

Se o seu modelo não tiver colliders, qualquer uma das seguintes opções deverá resolver isso:

* Adicione um **Mesh Collider** component. Isto criará um collider com uma [forma primitiva](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/shape-components/#primitive-shapes) (cube, plane, cylinder, sphere).
* Altere as propriedades do **Collisions** secção na **GLTF** component. O **Visible layer** deve ser atribuído a **Pointer**.
* Edite o modelo 3D no Blender para incluir uma geometria de collider invisível (qualquer mesh com um nome que termina em `_collider`). Veja [colliders](/docs/creator/content-creator-pt/modelacao-3d-e-animacoes/colliders/).


> [!NOTE]
> **💡 Dica**: Se usou o **Mesh Renderer** component para dar ao seu modelo uma forma primitiva, isso por si só não lhe dará um collider. Também deve atribuir-lhe um **Mesh Collider** component.


**Personalizar pointer events**

Pode substituir as definições predefinidas que são usadas quando um item tem um **On Click** ou um **On Input Action** Trigger Action.

* **Texto de hover**: Altere a dica que os jogadores veem junto do cursor quando passam o rato sobre o item. Isto pode ser muito útil para clarificar o que o seu item faz.
* **Distância máxima**: A que distância pode o jogador estar ao interagir com o seu item.
* **Mostrar feedback**: Se não estiver selecionado, o item não tem dica de hover quando o jogador passa o cursor sobre ele.
* **Button**: Se usar o **On Input Action** Trigger Action, pode reatribuir o padrão **Primary (E)** para outra tecla. A dica de hover incluirá um ícone para clarificar que tecla usar. Pode usar **Secondary (F)**, ou **Actions 3 a 6** (teclas numéricas 1 a 4).

### Eventos globais de botão

Adicione um **Trigger** component com **On Global Click**, **On Global Primary** ou **On Global Secondary** Eventos de Triggers.

* **On Global Click** reage sempre que o jogador clica no botão esquerdo do rato, em qualquer ponto da scene.
* **On Global Primary** reage sempre que o jogador pressiona o Primary Button (E), em qualquer ponto da scene.
* **On Global Secondary** reage sempre que o jogador pressiona o Secondary Button (F), em qualquer ponto da scene.


> [!NOTE]
> **💡 Dica**: Muitas vezes faz sentido combinar isto com [Estados e condições](/docs/creator/content-creator-pt/scene-editor/interatividade/states-and-conditions/), para que os items só reajam ao evento do botão se o jogador estiver na sala, ou noutra condição.


### Posição do jogador

Adicione um **Trigger** component com **Jogador Entra na Área**, **Jogador Sai da Área** Eventos de Triggers.

Isto reagirá quando o jogador entrar ou sair de uma área com tamanho predefinido de 1x1x1, posicionada no centro do item.


> [!NOTE]
> **💡 Dica**: Muitas vezes é melhor usar o [**Trigger Area**](/docs/creator/content-creator-pt/scene-editor/interatividade/smart-items/#trigger-areas) smart item, uma vez que as dimensões deste item podem ser claramente visualizadas no Scene Editor.


## Ver também

* [Smart items - Noções básicas](/docs/creator/content-creator-pt/scene-editor/interatividade/smart-items/)
* [Smart items - Avançado](/docs/creator/content-creator-pt/scene-editor/interatividade/smart-items-advanced/)
* [Estados e condições](/docs/creator/content-creator-pt/scene-editor/interatividade/states-and-conditions/)
* [Combine com código](/docs/creator/content-creator-pt/scene-editor/expandir-com-codigo/overview/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scene-editor/interatividade/make-any-item-smart.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
