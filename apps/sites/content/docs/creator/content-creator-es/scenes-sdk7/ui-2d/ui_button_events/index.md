> Gestiona eventos de Button en entidades de UI.

# Eventos de Button de UI

Para hacer un Button en tu UI, crea un `Button` elemento de UI con las siguientes propiedades:

* `valor`: Una cadena con el texto que se mostrará en el botón.
* `onMouseDown`: Una función de callback que se ejecuta cada vez que el usuario presiona el botón del puntero sobre la entidad.
* `uiTransform`: Propiedades de posicionamiento del elemento de UI.

El siguiente ejemplo muestra cómo crear un botón de UI clicable.

***archivo ui.tsx:***

```tsx
import { Button } from '@dcl/sdk/react-ecs'

export const uiMenu = () => (
	<Button
		value="Click me"
		uiTransform={{ width: 100, height: 100 }}
		onMouseDown={() => {
			console.log('Clicked on the UI')
		}}
	/>
)
```

***archivo index.ts:***

```ts
import { ReactEcsRenderer } from '@dcl/sdk/react-ecs'
import { uiMenu } from './ui'

export function main() {
    ReactEcsRenderer.setUiRenderer(uiMenu)
}
```


> [!WARNING]
> **📔 Nota**: Todos los siguientes fragmentos en esta página asumen que tienes un `.ts` similar al anterior, ejecutando la función `ReactEcsRenderer.setUiRenderer()` .


También puedes escribir la función que se ejecuta al hacer clic fuera de la definición de la UI, y referenciarla por nombre. Esto ayuda a mantener el código de la UI más legible, y también es útil si varias entidades de UI clicables necesitan llamar a la misma función.

```tsx
import { Button } from '@dcl/sdk/react-ecs'

function handleClick() {
	// Hacer algo en onClick
	console.log('Clicked on the UI')
}
export const uiMenu = () => (
	<Button
		value="Click me"
		uiTransform={{ width: 100 }}
		onMouseDown={handleClick}
	/>
)
```

Se pueden añadir los siguientes campos a un `Button` elemento de UI:

* `onMouseDown`: Una función de callback que se ejecuta cada vez que el usuario presiona el botón del puntero sobre la entidad.
* `onMouseUp`: Una función de callback que se ejecuta cada vez que se levanta el botón del puntero mientras se apunta a la entidad.
* `onMouseEnter`: Una función de callback que se ejecuta cada vez que el puntero empieza a pasar por encima del botón.
* `onMouseLeave`: Una función de callback que se ejecuta cada vez que el puntero deja de pasar por encima del botón.
* `color`: Color del texto del botón.
* `font`: Fuente del texto del botón.
* `textAlign`: Alineación del texto dentro del botón
* `uiTransform`: Propiedades de posicionamiento del elemento de UI.
* `uiBackground`: Establece el color o la textura del elemento de UI.
* `variant`: Usa esta propiedad para establecer el estilo del botón como una de las opciones predeterminadas. `primary` y `secondary` están disponibles.
* `disabled`: Booleano para desactivar un botón. Cuando `disabled` se establece en *true*, las `onMouseDown` y `onMouseUp` acciones ya no se llaman, y el botón deja de anunciar cualquier interacción del puntero. El botón también se dibuja "atenuado": tanto el texto como el fondo se renderizan con la mitad de su valor de `alpha` . Este es solo un cambio visual. Los valores de `Color4` que pasa tu escena nunca se modifican, así que puedes reutilizar con seguridad un objeto de paleta compartido entre muchos elementos.

## Estilo de Button

Establece el variant en `primary` o `secondary` para aprovechar las opciones de estilo predeterminadas para botones. `primary` hace que tu botón sea rojo con texto blanco, `secondary` hace que tu botón sea blanco con texto rojo.

```tsx
import { UiEntity, Button, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
	<UiEntity
		uiTransform={{
			width: 500,
			height: 230,
			margin: '16px 0 8px 270px',
			padding: 4,
			alignSelf: 'center',
		}}
		uiBackground={{ color: Color4.Gray() }}
	>
		<Button
			value="Click Me"
			variant="primary"
			uiTransform={{ width: 80, height: 20, margin: 4 }}
			onMouseDown={() => {
				console.log('Clicked on the UI')
			}}
		/>
		<Button
			value="Click Me"
			variant="secondary"
			uiTransform={{ width: 80, height: 20, margin: 4 }}
			onMouseDown={() => {
				console.log('Clicked on the UI')
			}}
		/>
	</UiEntity>
)
```

También eres libre de usar todas las propiedades del fondo libremente. También puedes establecer un variant y luego sobrescribir algunas de sus propiedades. Este ejemplo usa el `primary` variant, pero sobrescribe el color para que sea verde:

```tsx
import { Button } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
	<Button
		value="My Button!"
		variant="primary"
		uiTransform={{ width: 100, height: 100 }}
		onMouseDown={() => {
			console.log('Clicked on My Button!')
		}}
		uiBackground={{
			color: Color4.Green(),
		}}
	/>
)
```

## Botones conmutable

Un caso de uso común es hacer que un botón alterna entre dos estados, como un interruptor. El siguiente ejemplo cambia entre dos colores cada vez que se pulsa el botón:

```tsx
import { Button } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

let buttonEnabled = false

export const uiMenu = () => (
	<Button
		value="My Button"
		variant="primary"
		uiTransform={{ width: 100, height: 100 }}
		onMouseDown={() => {
			console.log('Clicked on My Button!')
			buttonEnabled = !buttonEnabled
			if (buttonEnabled) {
				// hacer algo
			} else {
				// hacer algo diferente
			}
		}}
		uiBackground={{
			color: buttonEnabled ? Color4.Green() : Color4.Red(),
		}}
	/>
)
```

Ten en cuenta que en el ejemplo anterior, el color depende de una `buttonEnabled` variable. Siempre que cambie el valor de esta variable, afecta inmediatamente al color del fondo.

## Retroalimentación al pasar el cursor

Otro caso de uso común es mostrar algún tipo de indicación visual al pasar el cursor sobre un botón, para aclarar que es interactivo, o incluso mostrar una pista de hover que explique qué hace este botón. Usa los `onMouseEnter` y `onMouseLeave` callbacks para detectar cuándo el cursor del jugador está sobre el botón y reaccionar en consecuencia.

```tsx
import { Button } from '@dcl/sdk/react-ecs'

let buttonEnabled = false

export const uiMenu = () => (
	<Button
		value="My Button"
		uiTransform={{ width: 100, height: 100 }}
		onMouseDown={() => {
			// función del botón
		}}
		onMouseEnter={() => {
			// mostrar pista
		}}
		onMouseLeave={() => {
			// ocultar pista
		}}
	/>
)
```

## Hacer clicables otros elementos

Cualquier elemento de la UI puede hacerse clicable añadiendo una propiedad `onMouseDown` a él; funciona igual que un botón. El siguiente ejemplo añade propiedades `onMouseDown` a las imágenes de fondo y al texto.

```tsx
import { UiEntity, Label, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'
import { engine, Transform } from '@dcl/sdk/ecs'

export const uiMenu = () => (
	<UiEntity
		onMouseDown={() => {
			console.log('¡Se hizo clic en el fondo!')
		}}
		uiTransform={{
			width: 400,
			height: 230,
		}}
		uiBackground={{ color: Color4.create(0.5, 0.8, 0.1, 0.6) }}
	>
		<Label
			onMouseDown={() => {
				console.log('¡Se hizo clic en la etiqueta!')
			}}
			value={`Player: ${getPlayerPosition()}`}`
			fontSize={18}
			uiTransform={{ width: '100%', height: 30 }}
		/>
	</UiEntity>
)

function getPlayerPosition() {
	const playerPosition = Transform.getOrNull(engine.PlayerEntity)
	if (!playerPosition) return 'unknown'
	const { x, y, z } = playerPosition.position
	return `{x: ${x.toFixed(2)}, y: ${y.toFixed(2)}, z: ${z.toFixed(2)} }`
}
```

## Bloqueo del puntero

Todas las entidades de UI no bloquean el puntero por defecto, lo que significa que los clics de los jugadores las atravesarán e interactuarán con los objetos en el espacio del mundo 3D que está detrás de ellas. Si una entidad tiene un `onMouseDown` callback, entonces se convierte en bloqueadora del puntero, así que los clics del jugador no afectan a lo que está detrás de esa entidad de UI.

Puedes cambiar este comportamiento predeterminado cambiando el valor de la propiedad `pointerFilter` en el `uiTransform` componente de cualquier entidad de UI. Por ejemplo, para configurar una entidad que no tiene `onMouseDown` para que bloquee el puntero.

Los valores compatibles para `pointerFilter` son:

* `block`: El elemento de UI bloquea el puntero; los jugadores no pueden hacer clic en nada detrás de este elemento de UI.
* `none`: El elemento de UI no bloquea el puntero. El elemento no es clicable y se puede hacer clic en cualquier cosa detrás de él.

A continuación hay una UI simple que no tiene un `onMouseDown`, pero que anula el comportamiento predeterminado de no bloquear el puntero estableciendo `pointerFilter` en `block`.

```tsx
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

// dibujar UI
export const uiMenu = () => (
	<UiEntity
		uiTransform={{
			width: '100%',
			height: '100px',
			pointerFilter: `block`,
		}}
		uiText={{ value: `Este elemento bloquea el puntero`, fontSize: 40 }}
		uiBackground={{ color: Color4.create(0.5, 0.8, 0.1, 0.6) }}
	/>
)
```

### El bloqueo sigue el cuadro de diseño, no los píxeles visibles

Un elemento bloqueador captura los clics en todo su **rectángulo completo**, haya o no algo dibujado allí. Un fondo totalmente transparente no marca ninguna diferencia.


> [!CAUTION]
> **Advertencia:** Nunca pongas un manejador de puntero o `pointerFilter: 'block'` en un contenedor de pantalla completa dimensionado `100%` por `100%`.
> 
> Su rectángulo es toda la pantalla, así que un solo `onMouseDown` erróneo en la raíz de tu diseño hace que todos los demás elementos de UI y todo en el mundo 3D no puedan hacerse clic. La UI sigue viéndose perfectamente correcta, porque el panel que puedes ver solo cubre una pequeña parte de la pantalla, lo que hace que esto sea muy difícil de detectar.


Adjunta los manejadores al elemento más pequeño que los necesite: el panel, el botón, la fila. Los contenedores de diseño se quedan sin manejadores.

Hay dos casos en los que un elemento bloqueador de pantalla completa es lo correcto, y ambos son deliberados:

* Un **fondo modal**, que está pensado para absorber los clics, y que solo renderizas mientras el modal está abierto.
* Un **capturador de liberación de arrastre**, que solo existe mientras hay un arrastre en curso. Consulta [Interacciones de arrastre](#drag-interactions) abajo.

Si los clics dejan de funcionar en cualquier parte de tu escena, esto es lo primero que debes comprobar.

## Interacciones de arrastre

Los manejadores de puntero de UI (`onMouseDown`, `onMouseUp`, `onMouseEnter`, `onMouseLeave`) no toman parámetros. Se activan como simples callbacks `() => void` sin datos de posición o coordenadas. No existe un manejador `onMouseDrag` o `onMouseMove` en el sistema de UI.

Para construir UI basada en arrastre (deslizadores, barras de búsqueda, tiradores de arrastre), usa `PrimaryPointerInfo.screenDelta` de `@dcl/sdk/ecs`. Esto proporciona el movimiento del ratón en píxeles desde el último frame, actualizado en cada frame independientemente de lo que haya debajo del cursor.

El patrón funciona así:

1. `onMouseDown` en el objetivo del arrastre inicia el arrastre y registra el valor inicial.
2. Un sistema lee `screenDelta` en cada frame y lo acumula en el valor mientras el arrastre está activo.
3. Una superposición invisible a pantalla completa con `pointerFilter: 'block'` captura la liberación del ratón, de modo que soltarlo fuera del objetivo estrecho aún termina el arrastre.

```ts
import { engine, PrimaryPointerInfo, UiCanvasInformation } from '@dcl/sdk/ecs'

let dragging = false
let sliderValue = 0.5

// Llama a este sistema cada frame para acumular el movimiento del arrastre
engine.addSystem(() => {
	if (!dragging) return

	const delta = PrimaryPointerInfo.getOrNull(engine.RootEntity)?.screenDelta
	if (!delta || delta.x === 0) return

	// Convierte los píxeles de pantalla a un rango de 0–1
	// Divide por el factor de escala de la UI para que la velocidad del arrastre coincida con el cursor
	const canvas = UiCanvasInformation.getOrNull(engine.RootEntity)
	const scale = canvas ? Math.min(canvas.width / 1920, canvas.height / 1080) : 1
	const trackWidth = 200 // ancho en px virtuales de la pista del deslizador

	sliderValue = Math.max(0, Math.min(1, sliderValue + delta.x / scale / trackWidth))
})
```


> [!WARNING]
> **Nota:** En móvil, `screenDelta` siempre informa 0 (no hay cursor que se mueva libremente). Para deslizadores compatibles con móvil, añade botones incrementadores (`-` / `+`) junto a la pista de arrastre. Usa [`isMobile()`](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/detect-platform/) de `@dcl/sdk/platform` para ramificar tu UI.



> [!NOTE]
> **Consejo:** Divide siempre `screenDelta` por el factor de escala de la UI, o el arrastre se pasará o se quedará corto en pantallas cuya resolución difiera de tu tamaño virtual.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/ui-2d/ui_button_events.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
