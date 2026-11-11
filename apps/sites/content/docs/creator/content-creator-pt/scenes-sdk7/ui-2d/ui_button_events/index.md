> Lide com eventos de botão em UI entities.

# Eventos de Botão da UI

Para criar um Button na sua UI, crie um `Button` elemento de UI com as seguintes propriedades:

* `value`: Uma string com o texto a exibir no botão.
* `onMouseDown`: Uma função de callback que é executada sempre que o usuário pressiona o botão do ponteiro na entity.
* `uiTransform`: Propriedades de posicionamento do elemento de UI.

O exemplo a seguir mostra como criar um Button de UI clicável.

***arquivo ui.tsx:***

```tsx
import { Button } from '@dcl/sdk/react-ecs'

export const uiMenu = () => (
	<Button
		value="Clique em mim"
		uiTransform={{ width: 100, height: 100 }}
		onMouseDown={() => {
			console.log('Clicou na UI')
		}}
	/>
)
```

***arquivo index.ts:***

```ts
import { ReactEcsRenderer } from '@dcl/sdk/react-ecs'
import { uiMenu } from './ui'

export function main() {
    ReactEcsRenderer.setUiRenderer(uiMenu)
}
```


> [!WARNING]
> **📔 Nota**: Todos os trechos a seguir nesta página presumem que você tenha um `.ts` semelhante ao acima, executando a função `ReactEcsRenderer.setUiRenderer()` .


Você também pode escrever a função executada pelo clique fora da definição da UI e referenciá-la pelo nome. Isso ajuda a manter o código da UI mais legível e também é útil se várias entities de UI clicáveis precisarem chamar a mesma função.

```tsx
import { Button } from '@dcl/sdk/react-ecs'

function handleClick() {
	// Faça algo em onClick
	console.log('Clicou na UI')
}
export const uiMenu = () => (
	<Button
		value="Clique em mim"
		uiTransform={{ width: 100 }}
		onMouseDown={handleClick}
	/>
)
```

Os seguintes campos podem ser adicionados a um `Button` elemento de UI:

* `onMouseDown`: Uma função de callback que é executada sempre que o usuário pressiona o botão do ponteiro na entity.
* `onMouseUp`: Uma função de callback que é executada sempre que o botão do ponteiro é liberado enquanto se aponta para a entity.
* `onMouseEnter`: Uma função de callback que é executada sempre que o ponteiro começa a passar sobre o botão.
* `onMouseLeave`: Uma função de callback que é executada sempre que o ponteiro para de passar sobre o botão.
* `color`: Cor do texto no botão.
* `font`: Fonte do texto no botão.
* `textAlign`: Alinhamento do texto dentro do botão
* `uiTransform`: Propriedades de posicionamento do elemento de UI.
* `uiBackground`: Define a cor ou textura do elemento de UI.
* `variant`: Use esta propriedade para definir o estilo do botão como um dos estilos padrão. `primary` e `secondary` estão disponíveis.
* `disabled`: Booleano para desativar um botão. Quando `disabled` está definido como *true*, as `onMouseDown` e `onMouseUp` ações deixam de ser chamadas, e o botão deixa de oferecer qualquer interação de ponteiro. O botão também é desenhado "acinzentado": tanto o texto quanto o fundo são renderizados com metade do seu `valor de alpha` . Esta é apenas uma mudança de exibição. Os `Color4` valores que sua scene passa nunca são modificados, então você pode reutilizar com segurança um objeto de paleta compartilhado entre muitos elementos.

## Estilização do Button

Defina a variant como `primary` ou `secondary` para aproveitar as opções de estilo padrão para botões. `primary` torna seu botão vermelho com texto branco, `secondary` torna seu botão branco com texto vermelho.

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
			value="Clique em mim"
			variant="primary"
			uiTransform={{ width: 80, height: 20, margin: 4 }}
			onMouseDown={() => {
				console.log('Clicou na UI')
			}}
		/>
		<Button
			value="Clique em mim"
			variant="secondary"
			uiTransform={{ width: 80, height: 20, margin: 4 }}
			onMouseDown={() => {
				console.log('Clicou na UI')
			}}
		/>
	</UiEntity>
)
```

Você também pode usar livremente todas as propriedades do background. Também pode definir uma variant e depois sobrescrever algumas de suas propriedades. Este exemplo usa a `primary` variant, mas sobrescreve a cor para verde:

```tsx
import { Button } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
	<Button
		value="Meu Botão!"
		variant="primary"
		uiTransform={{ width: 100, height: 100 }}
		onMouseDown={() => {
			console.log('Clicou em Meu Botão!')
		}}
		uiBackground={{
			color: Color4.Green(),
		}}
	/>
)
```

## Botões alternáveis

Um caso de uso comum é fazer um botão alternar entre dois estados, como um interruptor. O exemplo abaixo alterna entre duas cores cada vez que o botão é pressionado:

```tsx
import { Button } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

let buttonEnabled = false

export const uiMenu = () => (
	<Button
		value="Meu Botão"
		variant="primary"
		uiTransform={{ width: 100, height: 100 }}
		onMouseDown={() => {
			console.log('Clicou em Meu Botão!')
			buttonEnabled = !buttonEnabled
			if (buttonEnabled) {
				// faça algo
			} else {
				// faça outra coisa
			}
		}}
		uiBackground={{
			color: buttonEnabled ? Color4.Green() : Color4.Red(),
		}}
	/>
)
```

Observe que no exemplo acima, a cor depende de uma `buttonEnabled` variável. Sempre que o valor dessa variável muda, isso afeta imediatamente a cor do fundo.

## Feedback ao passar o cursor

Outro caso de uso comum é exibir algum tipo de dica visual ao passar o cursor sobre um botão, para deixar claro que ele é interativo, ou até mesmo exibir uma dica de hover explicando o que esse botão faz. Use os `onMouseEnter` e `onMouseLeave` callbacks para detectar quando o cursor do jogador está sobre o botão e reagir de acordo.

```tsx
import { Button } from '@dcl/sdk/react-ecs'

let buttonEnabled = false

export const uiMenu = () => (
	<Button
		value="Meu Botão"
		uiTransform={{ width: 100, height: 100 }}
		onMouseDown={() => {
			// função do botão
		}}
		onMouseEnter={() => {
			// mostrar dica
		}}
		onMouseLeave={() => {
			// ocultar dica
		}}
	/>
)
```

## Tornar outros elementos clicáveis

Qualquer elemento na UI pode ser tornado clicável adicionando uma `onMouseDown` propriedade a ele; ela funciona de forma idêntica a um botão. O exemplo a seguir adiciona `onMouseDown` propriedades a imagens de fundo e texto.

```tsx
import { UiEntity, Label, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'
import { engine, Transform } from '@dcl/sdk/ecs'

export const uiMenu = () => (
	<UiEntity
		onMouseDown={() => {
			console.log('Fundo clicado!')
		}}
		uiTransform={{
			width: 400,
			height: 230,
		}}
		uiBackground={{ color: Color4.create(0.5, 0.8, 0.1, 0.6) }}
	>
		<Label
			onMouseDown={() => {
				console.log('Label clicada!')
			}}
			value={`Jogador: ${getPlayerPosition()}`}
			fontSize={18}
			uiTransform={{ width: '100%', height: 30 }}
		/>
	</UiEntity>
)

function getPlayerPosition() {
	const playerPosition = Transform.getOrNull(engine.PlayerEntity)
	if (!playerPosition) return 'desconhecido'
	const { x, y, z } = playerPosition.position
	return `{x: ${x.toFixed(2)}, y: ${y.toFixed(2)}, z: ${z.toFixed(2)} }`
}
```

## Bloqueio de ponteiro

Todas as entities de UI são, por padrão, não bloqueadoras de ponteiro, o que significa que os cliques dos jogadores as atravessam e interagem com objetos no espaço 3D do mundo atrás delas. Se uma entity tiver um `onMouseDown` callback, então ela se torna bloqueadora de ponteiro, de modo que os cliques do jogador não afetam o que está atrás dessa entity de UI.

Você pode alterar esse comportamento padrão mudando o valor da `pointerFilter` propriedade no `uiTransform` component em qualquer entity de UI. Por exemplo, para definir uma entity que não tenha nenhum `onMouseDown` para ser bloqueadora de ponteiro.

Os valores suportados para `pointerFilter` são:

* `block`: O elemento de UI é bloqueador de ponteiro, os jogadores não podem clicar em nada atrás deste elemento de UI.
* `none`: O elemento de UI não é bloqueador de ponteiro. O elemento não é clicável e qualquer coisa atrás dele pode ser clicada.

Abaixo há uma UI simples que não tem um `onMouseDown`, mas que substitui o comportamento padrão de não bloquear o ponteiro ao definir `pointerFilter` como `block`.

```tsx
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

// desenhar UI
export const uiMenu = () => (
	<UiEntity
		uiTransform={{
			width: '100%',
			height: '100px',
			pointerFilter: `block`,
		}}
		uiText={{ value: `Este elemento bloqueia o ponteiro`, fontSize: 40 }}
		uiBackground={{ color: Color4.create(0.5, 0.8, 0.1, 0.6) }}
	/>
)
```

### O bloqueio segue a caixa de layout, não os pixels visíveis

Um elemento bloqueador captura cliques em todo o seu **retângulo inteiro**, quer haja algo desenhado ali ou não. Um fundo totalmente transparente não faz diferença.


> [!CAUTION]
> **Aviso:** Nunca coloque um handler de ponteiro ou `pointerFilter: 'block'` em um wrapper de tela inteira dimensionado `100%` por `100%`.
> 
> O seu retângulo é a tela inteira, então um único handler perdido `onMouseDown` na raiz do seu layout torna todos os outros elementos de UI e tudo no mundo 3D impossíveis de clicar. A UI ainda parece perfeitamente correta, porque o painel que você vê cobre apenas uma pequena parte da tela, o que torna isso muito difícil de perceber.


Anexe handlers ao menor elemento que precisar deles: o painel, o botão, a linha. Os wrappers de layout permanecem sem handlers.

Há dois casos em que um elemento bloqueador em tela inteira é a coisa certa, e ambos são intencionais:

* Um **fundo modal**, que existe para absorver cliques, e que você só renderiza enquanto o modal está aberto.
* Um **capturador de liberação do arrasto**, que só existe enquanto um arrasto está em andamento. Veja [Interações de arraste](#drag-interactions) abaixo.

Se os cliques pararem de funcionar em qualquer lugar da sua scene, esta é a primeira coisa a verificar.

## Interações de arraste

Handlers de ponteiro da UI (`onMouseDown`, `onMouseUp`, `onMouseEnter`, `onMouseLeave`) não recebem parâmetros. Eles disparam como simples `() => void` callbacks sem dados de posição ou coordenadas. Não há `onMouseDrag` ou `onMouseMove` handler no sistema de UI.

Para criar UI baseada em arrasto (sliders, barras de busca, alças de arraste), use `PrimaryPointerInfo.screenDelta` de `@dcl/sdk/ecs`. Isso fornece o movimento do mouse em pixels desde o último frame, atualizado a cada frame independentemente de sobre o que o cursor esteja.

O padrão funciona da seguinte forma:

1. `onMouseDown` no alvo de arraste inicia o arrasto e registra o valor inicial.
2. Um system lê `screenDelta` a cada frame e o acumula no valor enquanto o arrasto está ativo.
3. Uma sobreposição invisível em tela inteira com `pointerFilter: 'block'` captura a liberação do mouse, então soltar fora do alvo estreito ainda encerra o arrasto.

```ts
import { engine, PrimaryPointerInfo, UiCanvasInformation } from '@dcl/sdk/ecs'

let dragging = false
let sliderValue = 0.5

// Chame este system a cada frame para acumular o movimento do arrasto
engine.addSystem(() => {
	if (!dragging) return

	const delta = PrimaryPointerInfo.getOrNull(engine.RootEntity)?.screenDelta
	if (!delta || delta.x === 0) return

	// Converter pixels da tela para uma faixa de 0–1
	// Divida pelo fator de escala da UI para que a velocidade do arrasto corresponda ao cursor
	const canvas = UiCanvasInformation.getOrNull(engine.RootEntity)
	const scale = canvas ? Math.min(canvas.width / 1920, canvas.height / 1080) : 1
	const trackWidth = 200 // largura virtual em px da trilha do slider

	sliderValue = Math.max(0, Math.min(1, sliderValue + delta.x / scale / trackWidth))
})
```


> [!WARNING]
> **Observação:** Em dispositivos móveis, `screenDelta` screenDelta sempre retorna 0 (não há cursor livre para mover). Para sliders compatíveis com mobile, adicione botões de ajuste (`-` / `+`) ao lado da trilha de arraste. Use [`isMobile()`](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/detect-platform/) de `@dcl/sdk/platform` para ramificar sua UI.



> [!NOTE]
> **Dica:** Divida sempre `screenDelta` pelo fator de escala da UI, ou o arrasto irá passar do ponto ou ficar aquém nas telas cuja resolução difere do seu tamanho virtual.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/ui-2d/ui_button_events.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
