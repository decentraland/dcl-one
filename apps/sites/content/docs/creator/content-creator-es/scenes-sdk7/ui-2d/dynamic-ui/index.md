> Aprende cómo hacer que las UIs sean dinámicas y respondan a cambios en los datos.

# UI dinámica

Puedes definir una UI que incluya elementos dinámicos, que se actualizan en cada tick. Solo necesitas encargarte de actualizar la variable que representa estos datos, y la UI se adaptará en respuesta a los nuevos valores.

Esto es muy útil para incluir elementos como un temporizador, la puntuación de un jugador, etc. Pero incluso puedes dar un paso más y definir estructuras completas de UI basadas en el estado.

## Variables de referencia

Puedes simplemente hacer referencia a una variable en cualquier propiedad de uno de los components en un uiEntity. A medida que la variable cambia de valor, la UI se adaptará en consecuencia.

El siguiente ejemplo define una variable `playerCurrentPosition` y la hace referencia como parte de una cadena en un `uiText` component. Luego, un system actualiza el valor de esta variable en cada tick, usando la posición actual del player. A medida que el valor de la variable cambia, la UI se actualiza en consecuencia, sin necesidad de modificar explícitamente la UI.

***Archivo ui.tsx:***

```tsx
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { playerCurrentPosition } from './index'
import { Color4 } from '@dcl/sdk/math'

// dibujar UI
export const uiMenu = () => (
  <UiEntity
    uiTransform={{
			width: '100%',
			height: '100px',
			justifyContent: 'center',
			alignItems: 'center',
    }}
    uiText={{ value: `Jugador: `+  playerCurrentPosition, fontSize: 40 }}
    uiBackground={{ color: Color4.create(0.5, 0.8, 0.1, 0.6) }}
  />
)
```

***Archivo index.ts:***

```ts
import { ReactEcsRenderer } from '@dcl/sdk/react-ecs'
import { engine, Transform } from '@dcl/sdk/ecs'
import { uiMenu } from './ui'

export function main() {
    ReactEcsRenderer.setUiRenderer(uiMenu, { virtualWidth: 1920, virtualHeight: 1080 })
}

// definir variable
export let playerCurrentPosition: string = ""

// system para actualizar la variable
engine.addSystem(() => {
  const playerPosition = Transform.getOrNull(engine.PlayerEntity)
  if (!playerPosition) return
  const { x, y, z } = playerPosition.position
  playerCurrentPosition =  `{x: ${x.toFixed(2)}, y: ${y.toFixed(2)}, z: ${z.toFixed(2)} }`
})
```

En el ejemplo anterior, también podrías incluir la variable como parte de la cadena, envolviendo la variable en `${ }`.

```ts
uiText={{
	value: `Jugador: ${playerCurrentPosition}`,
	fontSize: 40
}}
```

## Llamar funciones desde dentro de una UI

También puedes llamar a una function desde dentro de una definición de JSX, devolviendo un valor para usar en una propiedad de la UI. Las functions que se llaman desde dentro de esta definición de JSX se llaman recurrentemente, en cada tick del game loop.

En el ejemplo siguiente, un `uiText` component llama a la `getPlayerPosition()` function para definir parte de la cadena que se mostrará.

Este ejemplo es similar al de la sección anterior, pero al llamar a una function desde dentro de la definición de la UI evitamos declarar una variable separada y definir un system para alterar esa variable. Ten en cuenta que `getPlayerPosition()` se llama en cada tick del game loop, sin necesidad de declarar explícitamente un system.

***Archivo ui.tsx:***

```tsx
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'
import { engine, Transform } from "@dcl/sdk/ecs";

export const uiMenu = () => (
  <UiEntity
    uiTransform={{
      width: '100%',
      height: '100px',
      justifyContent: 'center',
      alignItems: 'center',
    }}
    uiText={{ value: `Jugador: `+  getPlayerPosition(), fontSize: 40 }}
    uiBackground={{ color: Color4.create(0.5, 0.8, 0.1, 0.6) }}
  />
)

function getPlayerPosition(){
  const playerPosition = Transform.getOrNull(engine.PlayerEntity)
  if (!playerPosition) return 'cargando...'
  const { x, y, z } = playerPosition.position
  return `{x: ${x.toFixed(2)}, y: ${y.toFixed(2)}, z: ${z.toFixed(2)} }`
}
```

***Archivo index.ts:***

```ts
import { ReactEcsRenderer } from '@dcl/sdk/react-ecs'
import { uiMenu } from './ui'

export function main() {
    ReactEcsRenderer.setUiRenderer(uiMenu, { virtualWidth: 1920, virtualHeight: 1080 })
}
```

## Alternar una UI activándola y desactivándola

La forma más sencilla de activar y desactivar una UI es usar una variable para el valor de la `display` property en la `uiTransform`. El `display` property hace que un uiEntity y todos sus hijos sean invisibles si se establece en `none`.

El siguiente ejemplo usa una variable para establecer el `display` field de una parte de la UI. El valor de esta variable se puede alternar haciendo clic en otro elemento de la UI.

***Archivo ui.tsx:***

```tsx
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

// Variable para reflejar el estado actual de la visibilidad del menú
var isMenuVisible: boolean = false

// Function para alternar el estado del menú
function toggleMenuVisibility() {
  isMenuVisible = !isMenuVisible
}

export const uiMenu = () => (
   // padre
   <UiEntity>
      {/* Menú */}
      <UiEntity
       uiTransform={{
          width: '80%',
          height: '100px',
          alignContent: 'center',
          justifyContent: 'center',
          display: isMenuVisible ? 'flex': 'none'
        }}
         uiText={{
          value: "Menú",
          fontSize: 30
        }}
        uiBackground={{ color: Color4.Green() }}
      />
      {/* botón */}
      <UiEntity
        uiTransform={{
          width: 100,
          height: 30,
          margin: { top: '35px', left: '500px' }
        }}
        uiText={{
          value: "Alternar menú",
          fontSize: 40
        }}
        uiBackground={{ color: Color4.Red() }}
        onMouseDown={toggleMenuVisibility}
      />
   </UiEntity>
)
```

***Archivo index.ts:***

```ts
import { ReactEcsRenderer } from '@dcl/sdk/react-ecs'
import { uiMenu } from './ui'

export function main() {
    ReactEcsRenderer.setUiRenderer(uiMenu, { virtualWidth: 1920, virtualHeight: 1080 })
}
```

## Entities de UI dinámicas

Los ejemplos de las secciones anteriores muestran cómo cambiar dinámicamente una sola property en una entity, pero también puedes definir estructuras completas de entities que pueden escalar en función de datos que cambian dinámicamente. Este tipo de patrón es común en el desarrollo web cuando se usan libraries como React, y es extremadamente potente. Con esto puedes definir aplicaciones de UI extremadamente flexibles y escalables.

El siguiente ejemplo enumera los ids de todas las entities en la scene que tienen una `MeshRenderer` y `Transform`. Crea un `uiText` para cada una. A medida que el contenido de la scene cambia, la lista de UI entities también se adapta en cada tick.

***Archivo ui.tsx:***

```tsx
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'
import { engine } from '@dcl/sdk/ecs'
import { MeshRenderer } from '@dcl/sdk/ecs'
import { Transform } from '@dcl/sdk/ecs'

export const uiMenu = () => (
  <UiEntity
    uiTransform={{
      width: '100%',
      height: '300px',
      justifyContent: 'center',
      alignItems: 'center',
    }}
    uiBackground={{ color: Color4.create(0.5, 0.8, 0.1, 0.6) }}
  >
    <UiEntity>
      {generateText()}
    </UiEntity>
  </UiEntity>
)


function generateText(){
  return Array.from(engine.getEntitiesWith(
    MeshRenderer,
    Transform
  )).map(([entity]) => <TextComponent value={entity.toString()} key={entity} /> )
}


function TextComponent(props: { value: string; key: string | number }) {
  return <UiEntity
    key={props.key}
    uiTransform={{ width: 80, height: 20 }}
    uiText={{ value: props.value, textAlign: 'middle-center', fontSize: 12 }}
    uiBackground={{ color: { r: 1, g: 0.176, b: 0.333, a: 1 } }}
  />
}
```

***Archivo index.ts:***

```ts
import { ReactEcsRenderer } from '@dcl/sdk/react-ecs'
import { uiMenu } from './ui'

export function main() {
    ReactEcsRenderer.setUiRenderer(uiMenu, { virtualWidth: 1920, virtualHeight: 1080 })
}
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/ui-2d/dynamic-ui.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
