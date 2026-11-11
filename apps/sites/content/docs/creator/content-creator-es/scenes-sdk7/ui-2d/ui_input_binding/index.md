> Vincula acciones de entrada a tus propios elementos de UI para que controlen la entrada del jugador.

# Vinculación de entrada de UI

`UiInputBinding` convierte cualquier elemento de UI en un botón de control. Vincula un elemento a una o más [`InputAction`](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/click-events/#pointer-buttons)s, y mientras se mantiene pulsado — con el tacto o el puntero — esas acciones se mantienen activas, impulsando tanto la entrada del jugador local (movimiento, salto) como cualquier escena [`InputAction`](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/click-events/#pointer-buttons) listeners, exactamente igual que los botones nativos en pantalla.

Úsalo para construir tus propios controles táctiles. Normalmente se combina con [Controles en pantalla](/docs/creator/content-creator-es/scenes-sdk7/interactividad/touch-screen-controls/): oculta los botones nativos y luego coloca los tuyos en su lugar.

<figure><img src="/docs/_remote/19dc4254d88476f6abb9fbcf.webp" alt="A retro-style gamepad assembled from custom UI elements, each wired to an input action"><figcaption><p>Un gamepad totalmente personalizado construido con elementos de UI: cada botón es una entidad de UI vinculada a una acción de entrada</p></figcaption></figure>

## Vincula una acción a un elemento de UI

Añade un `uiInputBinding` prop a cualquier elemento de tu `.tsx` UI y enumera las acciones que se mantendrán activas mientras esté pulsado. La prop está disponible en todos los elementos de UI ([`UiEntity`](/docs/creator/content-creator-es/scenes-sdk7/ui-2d/onscreen-ui/#ui-entities), [`Button`](/docs/creator/content-creator-es/scenes-sdk7/ui-2d/onscreen-ui/#ui-entities), [`Label`](/docs/creator/content-creator-es/scenes-sdk7/ui-2d/onscreen-ui/#ui-entities), …), al igual que `uiTransform` y `uiBackground`.

**Un botón que mueve al jugador hacia adelante mientras se mantiene pulsado:**

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

## Construye un conjunto de controles personalizado

Combina varios elementos vinculados para montar un esquema de control completo.

**Un botón de movimiento más un botón de acción que dispara `IA_PRIMARY`:**

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
			value="Use"
			uiTransform={{ width: 100, height: 100 }}
			uiInputBinding={{ actions: [InputAction.IA_PRIMARY] }}
		/>
	</UiEntity>
)
```

Las acciones vinculadas se comportan igual que los botones nativos: `IA_FORWARD` / `IA_BACKWARD` / `IA_LEFT` / `IA_RIGHT` mueven al avatar, y cualquier acción puede ser leída por los [`InputAction`](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/click-events/#pointer-buttons) listeners de tu escena. Si eliminas la prop (o el componente), se libera la vinculación.

## Propiedades

| Propiedad | Tipo                                                                                                                               | Descripción                                                                                                                                                                                                                                                                                                                                                            |
| --------- | ---------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `actions` | array de [`InputAction`](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/click-events/#pointer-buttons) | Las acciones de entrada que se mantienen pulsadas mientras este elemento está presionado. Ver [Eventos de clic](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/click-events/) para la lista completa de [`InputAction`](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/click-events/#pointer-buttons) valores. |


> [!NOTE]
> Combínalo con [`TouchScreenControls`](/docs/creator/content-creator-es/scenes-sdk7/interactividad/touch-screen-controls/) para ocultar los controles nativos y reemplazarlos por tu propia UI táctil — la forma recomendada de publicar un esquema de control totalmente personalizado.


## Relacionado

* [Controles en pantalla](/docs/creator/content-creator-es/scenes-sdk7/interactividad/touch-screen-controls/)
* [Eventos de botones de UI](/docs/creator/content-creator-es/scenes-sdk7/ui-2d/ui_button_events/)
* [Entrada en móvil](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/input-on-mobile/)
* [Eventos de clic](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/click-events/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/ui-2d/ui_input_binding.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
