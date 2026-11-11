> Vincule ações de entrada aos seus próprios elementos de UI para que eles acionem a entrada do jogador.

# Vinculação de Entrada da UI

`UiInputBinding` transforma qualquer elemento de UI num botão de controlo. Associe um elemento a uma ou mais [`InputAction`](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/click-events/#pointer-buttons)s, e enquanto estiver pressionado — por toque ou pointer — essas ações são mantidas, controlando tanto a entrada do jogador local (movimento, salto) como quaisquer listeners da cena [`InputAction`](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/click-events/#pointer-buttons) exatamente como os botões nativos no ecrã.

Use-o para construir os seus próprios controlos táteis. Normalmente é usado em conjunto com [Controlos no ecrã](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/touch-screen-controls/): esconda os botões nativos e depois coloque os seus no lugar deles.

<figure><img src="/docs/_remote/19dc4254d88476f6abb9fbcf.webp" alt="A retro-style gamepad assembled from custom UI elements, each wired to an input action"><figcaption><p>Um gamepad totalmente personalizado construído a partir de elementos de UI — cada botão é uma entidade de UI ligada a uma ação de input</p></figcaption></figure>

## Associe uma ação a um elemento de UI

Adicione um `uiInputBinding` prop a qualquer elemento na sua `.tsx` UI e liste as ações a manter premidas enquanto estiver pressionado. A prop está disponível em todos os elementos de UI ([`UiEntity`](/docs/creator/content-creator-pt/scenes-sdk7/ui-2d/onscreen-ui/#ui-entities), [`Button`](/docs/creator/content-creator-pt/scenes-sdk7/ui-2d/onscreen-ui/#ui-entities), [`Label`](/docs/creator/content-creator-pt/scenes-sdk7/ui-2d/onscreen-ui/#ui-entities), …), tal como `uiTransform` e `uiBackground`.

**Um botão que move o jogador para a frente enquanto estiver premido:**

```tsx
import { InputAction } from '@dcl/sdk/ecs'
import { Button } from '@dcl/sdk/react-ecs'

export const forwardButton = () => (
	<Button
		value="▲"
		uiTransform={{ width: 100, height: 100 }}
		uiInputBinding={{ actions: [InputAction.IA_FORWARD] }}
	/>
)
```

## Crie um conjunto de controlos personalizado

Combine vários elementos vinculados para montar um esquema de controlo completo.

**Um botão de movimento mais um botão de ação que aciona `IA_PRIMARY`:**

```tsx
import { InputAction } from '@dcl/sdk/ecs'
import { Button, UiEntity } from '@dcl/sdk/react-ecs'

export const customControls = () => (
	<UiEntity uiTransform={{ width: 240, height: 120 }}>
		<Button
			value="▲"
			uiTransform={{ width: 100, height: 100 }}
			uiInputBinding={{ actions: [InputAction.IA_FORWARD] }}
		/>
		<Button
			value="Usar"
			uiTransform={{ width: 100, height: 100 }}
			uiInputBinding={{ actions: [InputAction.IA_PRIMARY] }}
		/>
	</UiEntity>
)
```

As ações vinculadas comportam-se exatamente como os botões nativos: `IA_FORWARD` / `IA_BACKWARD` / `IA_LEFT` / `IA_RIGHT` movem o avatar, e qualquer ação pode ser lida pelos listeners da sua cena. [`InputAction`](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/click-events/#pointer-buttons) Ao remover a prop (ou o componente), a vinculação é libertada.

## Propriedades

| Propriedade | Tipo                                                                                                                              | Descrição                                                                                                                                                                                                                                                                                                                                                 |
| ----------- | --------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `actions`   | array de [`InputAction`](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/click-events/#pointer-buttons) | As ações de input mantidas premidas enquanto este elemento está pressionado. Veja [Eventos de clique](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/click-events/) para a lista completa de [`InputAction`](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/click-events/#pointer-buttons) valores. |


> [!NOTE]
> Combine isto com [`TouchScreenControls`](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/touch-screen-controls/) para ocultar os controlos nativos e substituí-los pela sua própria UI tátil — a forma recomendada de publicar um esquema de controlo totalmente personalizado.


## Relacionado

* [Controlos no ecrã](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/touch-screen-controls/)
* [Eventos de botão da UI](/docs/creator/content-creator-pt/scenes-sdk7/ui-2d/ui_button_events/)
* [Input em dispositivos móveis](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/input-on-mobile/)
* [Eventos de clique](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/click-events/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/ui-2d/ui_input_binding.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
