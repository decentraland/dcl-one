> Saiba como tornar UIs dinâmicas, respondendo a alterações nos dados.

# UI Dinâmica

Você pode definir uma UI que inclua elementos dinâmicos, que são atualizados a cada tick. Você só precisa lidar com a atualização da variável que representa esses dados, e a UI se adaptará em resposta aos novos valores.

Isso é muito útil para incluir elementos como um timer, a pontuação de um jogador etc. Mas você pode até dar um passo além e definir estruturas inteiras de UI com base no state.

## Variáveis de referência

Você pode simplesmente referenciar uma variável em qualquer propriedade de um dos Components em uma uiEntity. À medida que a variável muda de valor, a UI se adaptará em conformidade.

O exemplo abaixo define uma variável `playerCurrentPosition` e a referencia como parte de uma string em um `uiText` component. Um System então atualiza o valor dessa variável a cada tick, usando a posição atual do jogador. À medida que o valor da variável muda, a UI é atualizada em conformidade, sem nunca precisar modificar explicitamente a UI.

***arquivo ui.tsx:***

```tsx
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { playerCurrentPosition } from './index'
import { Color4 } from '@dcl/sdk/math'

// desenhar UI
export const uiMenu = () => (
  <UiEntity
    uiTransform={{
			width: '100%',
			height: '100px',
			justifyContent: 'center',
			alignItems: 'center',
    }}
    uiText={{ value: `Player: `+  playerCurrentPosition, fontSize: 40 }}
    uiBackground={{ color: Color4.create(0.5, 0.8, 0.1, 0.6) }}
  />
)
```

***arquivo index.ts:***

```ts
import { ReactEcsRenderer } from '@dcl/sdk/react-ecs'
import { engine, Transform } from '@dcl/sdk/ecs'
import { uiMenu } from './ui'

export function main() {
    ReactEcsRenderer.setUiRenderer(uiMenu, { virtualWidth: 1920, virtualHeight: 1080 })
}

// definir variável
export let playerCurrentPosition: string = ""

// System para atualizar a variável
engine.addSystem(() => {
  const playerPosition = Transform.getOrNull(engine.PlayerEntity)
  if (!playerPosition) return
  const { x, y, z } = playerPosition.position
  playerCurrentPosition =  `{x: ${x.toFixed(2)}, y: ${y.toFixed(2)}, z: ${z.toFixed(2)} }`
})
```

No exemplo acima, você também poderia incluir a variável como parte da string, envolvendo a variável em `${ }`.

```ts
uiText={{
	value: `Player: ${playerCurrentPosition}`,
	fontSize: 40
}}
```

## Chamar funções de dentro de uma UI

Você também pode chamar uma função de dentro de uma definição JSX, retornando um valor para usar em uma propriedade da UI. Funções chamadas de dentro dessa definição JSX são chamadas repetidamente, a cada tick do loop do jogo.

No exemplo abaixo, uma `uiText` componente chama a `getPlayerPosition()` função para definir parte da string a ser exibida.

Este exemplo é semelhante ao da seção anterior, mas ao chamar uma função de dentro da definição de UI evitamos declarar uma variável separada e definir um System para alterar essa variável. Observe que `getPlayerPosition()` é chamada a cada tick do loop do jogo, sem precisar declarar explicitamente um System.

***arquivo ui.tsx:***

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
    uiText={{ value: `Player: `+  getPlayerPosition(), fontSize: 40 }}
    uiBackground={{ color: Color4.create(0.5, 0.8, 0.1, 0.6) }}
  />
)

function getPlayerPosition(){
  const playerPosition = Transform.getOrNull(engine.PlayerEntity)
  if (!playerPosition) return 'loading...'
  const { x, y, z } = playerPosition.position
  return `{x: ${x.toFixed(2)}, y: ${y.toFixed(2)}, z: ${z.toFixed(2)} }`
}
```

***arquivo index.ts:***

```ts
import { ReactEcsRenderer } from '@dcl/sdk/react-ecs'
import { uiMenu } from './ui'

export function main() {
    ReactEcsRenderer.setUiRenderer(uiMenu, { virtualWidth: 1920, virtualHeight: 1080 })
}
```

## Alternar uma UI entre ligada e desligada

A maneira mais fácil de alternar uma UI entre ligada e desligada é usar uma variável para o valor da `display` propriedade em uma Entity `uiTransform`. A `display` propriedade torna uma uiEntity e todos os seus filhos invisíveis se definida como `none`.

O exemplo a seguir usa uma variável para definir o `display` campo de uma parte da UI. O valor dessa variável pode ser alternado clicando em outro elemento da UI.

***arquivo ui.tsx:***

```tsx
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

// Variável para refletir o estado atual da visibilidade do menu
var isMenuVisible: boolean = false

// Função para alternar o estado do menu
function toggleMenuVisibility() {
  isMenuVisible = !isMenuVisible
}

export const uiMenu = () => (
   // pai
   <UiEntity>
      {/* Menu */}
      <UiEntity
       uiTransform={{
          width: '80%',
          height: '100px',
          alignContent: 'center',
          justifyContent: 'center',
          display: isMenuVisible ? 'flex': 'none'
        }}
         uiText={{
          value: "Menu",
          fontSize: 30
        }}
        uiBackground={{ color: Color4.Green() }}
      />
      {/* button */}
      <UiEntity
        uiTransform={{
          width: 100,
          height: 30,
          margin: { top: '35px', left: '500px' }
        }}
        uiText={{
          value: "Toggle Menu",
          fontSize: 40
        }}
        uiBackground={{ color: Color4.Red() }}
        onMouseDown={toggleMenuVisibility}
      />
   </UiEntity>
)
```

***arquivo index.ts:***

```ts
import { ReactEcsRenderer } from '@dcl/sdk/react-ecs'
import { uiMenu } from './ui'

export function main() {
    ReactEcsRenderer.setUiRenderer(uiMenu, { virtualWidth: 1920, virtualHeight: 1080 })
}
```

## UI Entities dinâmicas

Os exemplos nas seções acima mostram como alterar dinamicamente uma única propriedade em uma Entity, mas você também pode definir estruturas inteiras de Entities que podem escalar com base em dados que mudam dinamicamente. Esse tipo de padrão é comum no desenvolvimento web ao usar libraries como React, e é extremamente poderoso. Com isso, você pode definir aplicações de UI extremamente flexíveis e escaláveis.

O exemplo a seguir lista os ids de todas as Entities na scene que têm um `MeshRenderer` e `Transform`. Ele cria um `uiText` para cada uma. À medida que o conteúdo da scene muda, a lista de UI entities também se adapta a cada tick.

***arquivo ui.tsx:***

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

***arquivo index.ts:***

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
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/ui-2d/dynamic-ui.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
