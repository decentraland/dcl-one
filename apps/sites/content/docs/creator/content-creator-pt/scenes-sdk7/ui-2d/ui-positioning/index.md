> Defina a posição, escala, padding e outras propriedades de UI entities.

# Posicionamento da UI

Para todos os tipos de conteúdo de UI, use o `uiTransform` componente para definir o tamanho, a posição e outras propriedades relacionadas ao alinhamento da Entity.

A `uiTransform` componente funciona no espaço 2D da tela de forma muito parecida com o `Transform` componente funciona no espaço 3D da cena.

***arquivo ui.tsx:***

```ts
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
	<UiEntity
		uiTransform={{
			width: '200px',
			height: '100px',
			justifyContent: 'center',
			alignItems: 'center',
		}}
		uiBackground={{ color: Color4.Green() }}
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
> **📔 Nota**: Todos os snippets a seguir nesta página assumem que você tem um `.ts` semelhante ao acima, a correr o `ReactEcsRenderer.setUiRenderer()` função.


## Propriedades de posicionamento

O alinhamento das entidades de UI é baseado no modelo de alinhamento Flexbox. Este é um modelo muito poderoso para organizar dinamicamente entidades aninhadas dentro de modais que podem variar de tamanho.


> [!NOTE]
> **💡 Dica**: A implementação de UI do Decentraland é baseada na do [Yoga](https://yogalayout.com/docs/). Leia [este artigo](https://www.joshwcomeau.com/css/interactive-guide-to-flexbox/) para uma cobertura muito acessível e aprofundada das propriedades disponíveis no Flexbox.


### Tamanho da Entity

Usa `width` e `height` para definir o tamanho da Entity. Os seguintes tipos de valores são suportados:

* `auto`: O tamanho se adapta para caber no conteúdo interno. Isso é muito conveniente para textos que podem variar de comprimento. Escreva o valor como "auto".
* **Porcentagem**: Como uma porcentagem das medidas do pai. Escreva o valor como uma string que termine em "%", por exemplo `10 %`.
* **Pixels**: Escreva o valor como um número.
* **Largura ou altura da tela**: vw (view width) e vh (view height) podem ser usados para indicar uma fração do tamanho total da janela em que o Decentraland está sendo executado. Por exemplo `10vw` se refere a 10% da largura da janela, `25vh` a 25% da altura da janela.

Observe que essas propriedades afetam o **default** tamanho desse item, o tamanho do item antes que quaisquer cálculos de flex grow e flex shrink sejam realizados. O tamanho final pode ser interpretado de forma diferente com base no tamanho da entidade pai e nas propriedades Flexbox definidas.


> [!WARNING]
> **📔 Nota**: Em propriedades que suportam tanto números quanto strings, para definir o valor em pixels, escreva um número. Para definir esses campos como uma porcentagem das medidas do pai, escreva o valor como uma string que termine em "%", por exemplo `10 %`. Você também pode definir um valor em pixels como uma string terminando a string em `px`, por exemplo `200px`.
> 
> * Quando os valores são expressos como porcentagem, eles estão sempre em relação ao contêiner pai. A entity de nível superior da sua UI também tem um pai: o renderizador a coloca dentro da área selecionada por [`screenInset`](/docs/creator/content-creator-pt/scenes-sdk7/ui-2d/onscreen-ui/#screen-inset-area), que, por padrão, é a área segura do dispositivo. Portanto, uma entity de nível raiz `100%` é 100% dessa área — em um telefone isso é mais estreito que a tela, no desktop os dois coincidem. Passe `screenInset: 'none'` se precisar de porcentagens medidas em relação à tela inteira.
> * Se os valores forem expressos em pixels, eles não são afetados pela escala do pai, mas também não são **não** pixels brutos da tela: eles são multiplicados pelo fator de escala da UI derivado da [tela virtual](/docs/creator/content-creator-pt/scenes-sdk7/ui-2d/onscreen-ui/#screen-virtual-scale), que está ativa por padrão. Um `width: 200` significa "200 px em uma tela do tamanho da tela virtual" e cresce ou encolhe proporcionalmente em qualquer outra.
> * Se os valores forem expressos em `vh` ou `vw`, eles são uma porcentagem da janela inteira, não afetados pela escala do pai, pela tela virtual ou por `screenInset`.
> 
> Para o `auto` width/height funcionarem, aplicam-se as seguintes regras:
> 
> * O UiTransform que usa width/height como “auto” deve ter `alignSelf`: `“center”`/`“flex-start”`/`“flex-end”` OU `positionType: “absolute”`
> * Se o UiTransform de um filho usar `positionType: “absolute”`, o pai não se adaptará ao seu tamanho/posição
> * Se o UiTransform de um filho usar qualquer sobrescrita de posição, o pai não se adaptará ao seu tamanho/posição


Essas outras propriedades também estão disponíveis para ajustar o tamanho de uma forma mais avançada:

* `maxWidth` e `maxHeight`: *number* ou string (como height e width). O tamanho máximo que a Entity pode ter.
* `minWidth` e `minHeight`: *number* ou string (como height e width). O tamanho mínimo que a Entity pode ter. Se o pai for pequeno demais para acomodar o tamanho mínimo das entidades, elas transbordarão para fora do pai.
* `flexBasis`: Esta é uma forma independente de eixo de fornecer o tamanho padrão de um item ao longo do eixo principal. Definir o flex basis de um filho é semelhante a definir a width desse filho se o pai for um contêiner com flex direction: row ou a definir a height de um filho se o pai for um contêiner com flex direction: column.

```ts
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
	<UiEntity
		uiTransform={{
			alignSelf: 'center',
			alignContent: 'center',
			width: '80%',
			height: '30%',
			minWidth: 300,
			maxWidth: 2500,
			margin: { left: '10%', right: '10%' },
		}}
		uiBackground={{ color: Color4.Green() }}
	/>
)
```

### Organizando entidades filhas

Por padrão, as entidades filhas são posicionadas em relação ao canto superior esquerdo do pai. Você pode usar propriedades como `justifyContent` e `alignItems` para mudar esse comportamento.


> [!NOTE]
> **💡 Dica**: Quaisquer propriedades que se refiram a *content* se referem a entidades ao longo do eixo principal (determinado por `flexDirection`). Quaisquer propriedades que se refiram


* `flexDirection`: A direção flex controla a direção em que os filhos de um node são organizados. Isso também é chamado de eixo principal. O eixo principal é a direção na qual os filhos são organizados. O eixo cruzado é o eixo perpendicular ao eixo principal, ou o eixo no qual as linhas de quebra são organizadas. Ele recebe seu valor do `FlexDirectionType` tipo. As seguintes opções estão disponíveis:
  * `row` (PADRÃO)
  * `row-reverse`
  * `column`
  * `column-reverse`
* `justifyContent`: Esta propriedade descreve como alinhar os filhos dentro do eixo principal do contêiner. Por exemplo, você pode usar esta propriedade para centralizar um filho horizontalmente dentro de um contêiner com `flexDirection` definido como row ou verticalmente dentro de um contêiner com `flexDirection` definido como column. O valor desta propriedade deve ser do `JustifyType` tipo. Os valores possíveis são:
  * `flex-start` (PADRÃO): Alinha os filhos de um contêiner ao início do eixo principal do contêiner.
  * `flex-end`: Alinha os filhos de um contêiner ao final do eixo principal do contêiner.
  * `center`: Alinha os filhos de um contêiner no centro do eixo principal do contêiner.
  * `space-between`: Distribui os filhos uniformemente ao longo do eixo principal do contêiner, distribuindo o espaço restante entre os filhos.
  * `space-around`: Distribui os filhos uniformemente ao longo do eixo principal do contêiner, distribuindo o espaço restante ao redor dos filhos. Em comparação com space-between, usar space-around resultará no espaço sendo distribuído ao início do primeiro filho e ao fim do último filho.
  * `space-evenly`: Distribui os filhos uniformemente ao longo do eixo principal do contêiner, distribuindo o espaço restante para que os espaços entre os filhos e entre os filhos e as bordas do contêiner sejam todos iguais.
* `alignItems`: Descreve como alinhar os filhos ao longo do eixo cruzado do contêiner. Align items é muito semelhante a justify content, mas em vez de se aplicar ao eixo principal, align items se aplica ao eixo cruzado. Esta propriedade requer um valor do `AlignType` tipo. As seguintes opções estão disponíveis:
  * `stretch`: (PADRÃO) Estica os filhos de um contêiner para corresponder à altura do eixo cruzado do contêiner.
  * `flex-start`: Alinha os filhos de um contêiner ao início do eixo cruzado do contêiner.
  * `flex-end`: Alinha os filhos de um contêiner ao final do eixo cruzado do contêiner.
  * `center`: Alinha os filhos de um contêiner no centro do eixo cruzado do contêiner.
  * `baseline`: Alinha os filhos de um contêiner ao longo de uma baseline comum. Filhos individuais podem ser definidos como a baseline de referência para seus pais.
* `alignSelf`: Align self tem as mesmas opções e efeito que `alignItems` mas, em vez de afetar os filhos dentro de um contêiner, você pode aplicar esta propriedade a um único filho para mudar seu alinhamento dentro do pai. align self substitui qualquer opção definida pelo pai com align items. Ele recebe seu valor de `AlignType`, vê `alignItems` acima para ver detalhes sobre essas opções.
* `alignContent`: Align content define a distribuição das linhas ao longo do eixo cruzado. Isso só tem efeito quando os itens são quebrados em várias linhas usando `flexWrap`. Ele recebe seu valor do `AlignType` tipo. As seguintes opções estão disponíveis:
  * `flex-start`: (PADRÃO) Alinha as linhas quebradas ao início do eixo cruzado do contêiner.
  * `flex-end`: Alinha as linhas quebradas ao final do eixo cruzado do contêiner.
  * `stretch`: Estica as linhas quebradas para corresponder à altura do eixo cruzado do contêiner.
  * `center`: Alinha as linhas quebradas no centro do eixo cruzado do contêiner.
  * `space-between`: Distribui uniformemente as linhas quebradas ao longo do eixo principal do contêiner, distribuindo o espaço restante entre as linhas.
  * `space-around`: Distribui uniformemente as linhas quebradas ao longo do eixo principal do contêiner, distribuindo o espaço restante ao redor das linhas. Em comparação com space-between, usar space-around resultará no espaço sendo distribuído para o início das primeiras linhas e ao fim da última linha.
* `flexGrow`: Isso descreve como qualquer espaço dentro de um contêiner deve ser distribuído entre seus filhos ao longo do eixo principal. Depois de organizar seus filhos, um contêiner distribuirá qualquer espaço restante de acordo com os valores de flex grow especificados por seus filhos. Flex grow aceita qualquer valor de ponto flutuante >= 0, sendo 0 o valor padrão. Um contêiner distribuirá qualquer espaço restante entre seus filhos ponderado pelo valor de flex grow do filho.
* `flexShrink`: Descreve como encolher os filhos ao longo do eixo principal no caso de o tamanho total dos filhos ultrapassar o tamanho do contêiner no eixo principal. flex shrink é muito semelhante a flex grow e pode ser pensado da mesma forma se qualquer tamanho excedente for considerado como espaço restante negativo. Essas duas propriedades também funcionam bem juntas, permitindo que os filhos cresçam e encolham conforme necessário. Flex shrink aceita qualquer valor de ponto flutuante >= 0, sendo 1 o valor padrão. Um contêiner encolherá seus filhos ponderado pelo valor de flex shrink do filho.
* `overflow`: Determina o que acontece se o tamanho dos filhos de uma entity ultrapassar o pai. Ele usa valores do `OverflowType` tipo.
  * `hidden`: As entidades que transbordam tornam-se invisíveis.
  * `visible`: As entidades que transbordam saem das margens do pai.
  * `scroll`: A área torna-se rolável, permitindo que o jogador role pelo conteúdo excedente. Veja [Contêineres roláveis](#scrollable-containers) para detalhes.
* `flexWrap`: A propriedade flex wrap é definida em contêineres e controla o que acontece quando os filhos ultrapassam o tamanho do contêiner ao longo do eixo principal. Por padrão, os filhos são quebrados em várias linhas ao longo do eixo principal, se necessário. Se a quebra estiver desativada com `nowrap`, os filhos são forçados para uma única linha (o que pode encolher entidades). wrap reverse se comporta da mesma forma que wrap, mas a ordem das linhas é invertida. Esta propriedade recebe seu valor do `FlexWrapType` tipo.
  * `wrap`
  * `nowrap`
  * `wrap-reverse`

### Margins e padding

* `margin`: Esta propriedade afeta o espaçamento ao redor da parte externa de um node. Um node com margin se deslocará em relação aos limites do pai, mas também deslocará a posição de quaisquer irmãos. A margin de um node contribui para o tamanho total do pai se o pai tiver tamanho automático. Defina o espaço entre a Entity e as margens do pai. O valor esperado é um objeto que contém as propriedades `top`, `left`, `bottom`, e `right`.
* `padding`: Esta propriedade afeta o tamanho do node ao qual ela é aplicada. O padding em Yoga age como se box-sizing: border-box; estivesse definido. Ou seja, o padding não acrescentará ao tamanho total de uma Entity se ela tiver um tamanho explícito definido. Para nodes com tamanho automático, o padding aumentará o tamanho do node, bem como deslocará a posição de quaisquer filhos. O valor esperado é um objeto que contém as propriedades `top`, `left`, `bottom`, e `right`.

### Ajustar a posição

No Flexbox, as posições das entities são determinadas principalmente por como elas são parentadas e quais propriedades de arranjo são definidas no pai e no filho. Muitas vezes você nem precisa definir a `position` propriedade. Mas, se quiser ajustar isso, ou substituir completamente o fluxo normal do Flexbox e definir uma posição absoluta, aqui estão as propriedades relevantes:

* `positionType`: Define como as entities são posicionadas. Ele usa um valor do `PositionType` enum.
  * `relative`: (PADRÃO) Por padrão, uma entity é posicionada de forma relativa. Isso significa que uma entity é posicionada de acordo com o fluxo normal do layout e depois deslocada em relação a essa posição com base nos valores de `top`, `right`, `bottom`, e `left`. O deslocamento não afeta a posição de nenhuma entidade irmã ou pai.
  * `absolute`: Quando posicionada de forma absoluta, uma entity não participa do fluxo normal do layout. Em vez disso, ela é disposta de forma independente de suas irmãs. A posição é determinada com base no `top`, `right`, `bottom`, e `left` valores.
* `position`: Os valores de posição `top`, `right`, `bottom`, e `left` se comportam de forma diferente dependendo do `positionType`. Para uma entity relativa, eles deslocam a posição da entity na direção especificada. Para uma entity absoluta, porém, essas propriedades especificam o deslocamento do lado da entity em relação ao mesmo lado no pai. O valor esperado é um objeto que contém as propriedades `top`, `left`, `bottom`, e `right`.


> [!WARNING]
> **📔 Nota** : Um valor positivo para `top` ou `left` mede a distância para dentro a partir dessa mesma borda do pai. Exemplo: para posicionar um componente deixando uma margem de 20 pixels em relação ao pai nos lados superior e esquerdo, defina `position` para `{ top: 20, left: 20 }`.


### Visibilidade

* `display`: Determina se uma entity está visível ou não. Para tornar uma entity invisível, defina `display` para `none`.

### Z Index

A `zIndex` propriedade de um `UiEntity` determina a ordem em que as entities são renderizadas. As entities com um `zIndex` maior são renderizadas acima das entities com um `zIndex`. O `zIndex` padrão é 0.

```ts
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
	<UiEntity
		uiTransform={{
			zIndex: 4
		}}
		uiBackground={{ color: Color4.Green() }}
	/>
)
```


> [!WARNING]
> **📔 Nota** : O `zIndex` propriedade só ordenará elementos em relação a irmãos diretos; ela não pode ser usada para renderizar uma entity sobre outras partes da árvore de layout. Em termos de html/CSS, cada elemento de DCL UI cria um novo [contexto de empilhamento](https://web.dev/learn/css/z-index#stacking_context). Para ordenar módulos de UI independentes adicionados com `addUiRenderer()` entre si, use as `zIndex` de [opções do renderizador](/docs/creator/content-creator-pt/scenes-sdk7/ui-2d/onscreen-ui/#stacking-order-between-ui-modules) em vez disso.
> 
> A UI padrão do Decentraland, incluindo o mapa, chat etc., é sempre renderizada acima de todos os outros elementos de UI.


## Contêineres roláveis

Quando uma entity de UI tem mais conteúdo do que cabe no tamanho atribuído, você pode tornar a área rolável definindo `overflow` para `scroll` na `uiTransform`propriedade overflow da entity. O jogador então pode rolar pelo conteúdo arrastando ou usando a roda do mouse.

Para criar um contêiner rolável, a entity pai deve ter um tamanho fixo (usando `width` e `height`), e os filhos devem exceder esse tamanho.

```ts
import { UiEntity, Label, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const scrollableMenu = () => (
	<UiEntity
		uiTransform={{
			width: 300,
			height: 400,
			overflow: 'scroll',
			flexDirection: 'column',
		}}
		uiBackground={{ color: Color4.fromHexString('#1a1a1a') }}
	>
		{/* Esses filhos excedem a altura de 400px do pai, tornando a área rolável */}
		<Label value="Item 1" fontSize={18} uiTransform={{ width: '100%', height: 80 }} />
		<Label value="Item 2" fontSize={18} uiTransform={{ width: '100%', height: 80 }} />
		<Label value="Item 3" fontSize={18} uiTransform={{ width: '100%', height: 80 }} />
		<Label value="Item 4" fontSize={18} uiTransform={{ width: '100%', height: 80 }} />
		<Label value="Item 5" fontSize={18} uiTransform={{ width: '100%', height: 80 }} />
		<Label value="Item 6" fontSize={18} uiTransform={{ width: '100%', height: 80 }} />
		<Label value="Item 7" fontSize={18} uiTransform={{ width: '100%', height: 80 }} />
	</UiEntity>
)
```

Isso é útil para criar listas longas, inventários, logs de chat, rankings ou qualquer painel em que o conteúdo possa crescer além do que cabe na tela.

Você também pode aninhar contêineres roláveis dentro de outros layouts de UI. Por exemplo, um modal de diálogo com um cabeçalho fixo e um corpo rolável:

```ts
import { UiEntity, Label, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const dialogWithScroll = () => (
	<UiEntity
		uiTransform={{
			width: 400,
			height: 500,
			flexDirection: 'column',
		}}
		uiBackground={{ color: Color4.fromHexString('#2a2a2a') }}
	>
		{/* Cabeçalho fixo */}
		<Label
			value="Classificação"
			fontSize={22}
			uiTransform={{ width: '100%', height: 60 }}
		/>

		{/* Corpo rolável */}
		<UiEntity
			uiTransform={{
				width: '100%',
				flexGrow: 1,
				overflow: 'scroll',
				flexDirection: 'column',
			}}
		>
			<Label value="1. Alice - 9500" fontSize={16} uiTransform={{ width: '100%', height: 50 }} />
			<Label value="2. Bob - 8200" fontSize={16} uiTransform={{ width: '100%', height: 50 }} />
			<Label value="3. Charlie - 7800" fontSize={16} uiTransform={{ width: '100%', height: 50 }} />
			<Label value="4. Diana - 6100" fontSize={16} uiTransform={{ width: '100%', height: 50 }} />
			<Label value="5. Eve - 5500" fontSize={16} uiTransform={{ width: '100%', height: 50 }} />
			<Label value="6. Frank - 4900" fontSize={16} uiTransform={{ width: '100%', height: 50 }} />
			<Label value="7. Grace - 4200" fontSize={16} uiTransform={{ width: '100%', height: 50 }} />
			<Label value="8. Hank - 3800" fontSize={16} uiTransform={{ width: '100%', height: 50 }} />
			<Label value="9. Ivy - 3100" fontSize={16} uiTransform={{ width: '100%', height: 50 }} />
			<Label value="10. Jack - 2700" fontSize={16} uiTransform={{ width: '100%', height: 50 }} />
		</UiEntity>
	</UiEntity>
)
```


> [!NOTE]
> **💡 Dica**: Use `flexGrow: 1` na entidade rolável para fazer com que ela preencha o espaço restante no pai, assim ela se adapta se outros irmãos (como um cabeçalho ou rodapé) mudarem de tamanho.


## Tamanho responsivo da UI

Players com tamanhos de tela diferentes podem ver o layout da sua UI de forma diferente. Os valores em pixels são escalados em relação à [tela virtual](/docs/creator/content-creator-pt/scenes-sdk7/ui-2d/onscreen-ui/#screen-virtual-scale) para você, então a mesma UI mantém suas proporções entre resoluções — **você não precisa calcular um fator de escala sozinho**, e fazer isso aplica a escala duas vezes.


> [!WARNING]
> **📔 Nota**: `devicePixelRatio` não participa do layout da UI. É uma dica de densidade de exibição — útil para escolher entre uma versão 1x, 2x ou 3x de uma textura — e nada mais. Se sua cena foi dimensionada em uma versão anterior do SDK, espere que a UI em tamanho de pixel seja renderizada até 2–3 vezes maior em telas de alta densidade (retina e mobile), e revise novamente qualquer coisa que tenha sido ajustada manualmente.



> [!CAUTION]
> **📔 Remova o seu próprio fator de escala.** Se a sua cena multiplica seus tamanhos por um fator que ela calcula a partir de `UiCanvasInformation` — normalmente `Math.min(width / 1920, height / 1080)` — remova esse multiplicador. É o mesmo fator que o SDK agora aplica por padrão, então manter os dois faz sua UI crescer quadraticamente com o tamanho da tela. Se você preferir manter seu próprio fator como o único, desative a tela virtual com `setUiRenderer(ui, { virtualWidth: 0, virtualHeight: 0 })`.


`UiCanvasInformation`, adicionado por padrão à entidade raiz da cena, ainda é a ferramenta certa para as decisões de layout que o dimensionamento não consegue expressar — um arranjo de diálogo diferente em uma tela estreita, escolher uma resolução de textura de `devicePixelRatio`, ou ler as áreas internas por conta própria. Não é a ferramenta certa para dimensionamento.

A `UiCanvasInformation` A component holds the following information:

* `height`: Altura da Canvas em pixels
* `width`: Largura da Canvas em pixels
* `devicePixelRatio`: A relação entre a resolução em pixels físicos no dispositivo e os pixels na canvas. Útil como dica de densidade de exibição, por exemplo para escolher entre uma versão 1x, 2x ou 3x de uma textura.
* `interactableArea`: Um `BorderRect` object, detalhando a área designada para elementos de UI da cena. Este objeto contém valores para `top`, `bottom`, `left` e `right`, cada um deles é o número de pixels naquela margem da tela ocupada pela UI do Explorer.
* `screenInsetArea`: Um `BorderRect` object, detalhando a área de recuo da tela (margens seguras) reservada pela UI do dispositivo ou da plataforma, por exemplo o notch, a barra de status, o indicador inicial ou cantos arredondados no mobile. Este objeto contém valores para `top`, `bottom`, `left` e `right`, cada um deles é o número de pixels reservados naquela borda da tela. No desktop isso normalmente é `0` em todos os lados.


> [!WARNING]
> **📔 Nota** : Diferentes explorers do Decentraland terão valores diferentes para isso, pois as UIs globais da plataforma podem diferir, e os valores podem mudar dinamicamente à medida que o usuário expande ou oculta diferentes menus globais de UI.


```ts
import { engine, UiCanvasInformation } from "@dcl/sdk/ecs"

export function Main() {
  const canvas = UiCanvasInformation.getOrNull(engine.RootEntity)
  if (!canvas) return
  console.log("DIMENSÕES DA CANVAS: ", canvas.width, canvas.height)
}
```

Algumas outras práticas recomendadas sobre tamanhos de UI:

* Se a largura ou a altura de qualquer elemento de UI for dinâmica, também é bom usar os `maxWidth`, `minWidth`, `maxHeight`, e `minHeight` parâmetros para garantir que elas permaneçam dentro de valores razoáveis.
* Um tamanho de fonte numérico é um valor de pixel virtual, escalado como qualquer outro. Se você quiser um tamanho medido em relação à canvas em vez disso, para que ele se mantenha independentemente da tela virtual, passe uma `vw`/`vh` string — veja [Tamanho de texto responsivo](/docs/creator/content-creator-pt/scenes-sdk7/ui-2d/ui_text/#responsive-text-size)


> [!NOTE]
> **💡 Dica**: Para exemplos práticos de dimensionamento de UI, veja a [`81,-2-ui-screen-inset-area`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/81,-2-ui-screen-inset-area) scene de teste, que executa todos os três `screenInset` modos (`'none'`, `'device'`, `'interactable'`) como três renderizadores coexistentes e imprime os valores `screenInsetArea` e `interactableArea` ao vivo, e [`76,-10-UiCanvasInformation`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/76,-10-UiCanvasInformation), que lê `UiCanvasInformation` a cada frame para dimensionar a UI de forma responsiva.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/ui-2d/ui-positioning.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
