> Saiba como criar uma UI para os jogadores na sua cena. Isto é útil, por exemplo, para apresentar informações relacionadas com o jogo.

# UI no ecrã

Você pode criar uma UI para sua Scene, a ser exibida no espaço 2D fixo da tela, em vez de no espaço do mundo 3D.

Os elementos da UI só ficam visíveis quando o jogador está dentro dos Parcels de LAND da Scene, pois as Scenes vizinhas podem ter sua própria UI para exibir. Partes da UI também podem ser acionadas para abrir quando determinados Events ocorrem no espaço do mundo, por exemplo, se o jogador clicar em um local específico.

Crie uma UI definindo uma estrutura aninhada de `UiEntity` objetos em JSX. A sintaxe usada para UIs é muito semelhante à de [React](https://reactjs.org/) (uma biblioteca muito popular baseada em JavaScript para criar UIs web).


> [!WARNING]
> **📔 Nota**: Você só pode definir a sintaxe de UI em Files que tenham uma `.tsx` extensão. `.tsx` Files oferecem suporte a tudo que os `.ts` Files oferecem suporte, além da sintaxe de UI. Recomendamos criar um File `ui.tsx` e definir sua UI nele. Lembre-se de chamar o método de Render da sua UI a partir de `index.ts` com `ReactEcsRenderer.setUiRenderer(yourUiMethodName)`, veja o exemplo abaixo.


Uma UI simples com elementos estáticos pode se parecer muito com HTML, mas, quando você adiciona elementos dinâmicos que respondem a uma mudança de State, pode fazer coisas muito mais poderosas.

A UI padrão do Explorer do Decentraland inclui um widget de chat, um mapa e outros elementos. Esses elementos de UI são sempre exibidos na camada superior, acima de qualquer UI específica da Scene. Portanto, se sua Scene tiver elementos de UI que ocupem o mesmo espaço na tela que esses, eles serão ocultados.

Consulte [diretrizes de UX](/docs/creator/content-creator-pt/scenes-sdk7/conceber-a-experiencia/ux-ui-guide/) para obter dicas sobre como projetar a aparência e a experiência da sua UI.


> [!NOTE]
> **📱 Projetando para dispositivos móveis**: Os dispositivos reservam espaço na tela para o entalhe, a barra de status, o indicador de início e os cantos arredondados, e o Client desenha seus próprios controles sobre parte do Canvas. Você também não mede nenhum deles — escolha a área em que sua UI será posicionada com [`screenInset`](#screen-inset-area) abaixo. Por padrão, ela usa a área segura do dispositivo, portanto, as margens de hardware são evitadas sem nenhum trabalho da sua parte. Antes de publicar, revise a [Área segura para dispositivos móveis](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/safe-area/) e as [Boas práticas de UI para dispositivos móveis](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/ui-best-practices/).


Quando o jogador clica no botão *fechar UI* no canto inferior direito da tela, todos os elementos de UI são ocultados.

## Renderizar uma UI

Para exibir uma UI na sua Scene, use a Function `ReactEcsRenderer.setUiRenderer()` , passando uma estrutura válida de Entities, descrita em um `.tsx` File.

Cada Entity é definida como um nó semelhante a HTML, com propriedades para cada um de seus Components.

***File ui.tsx:***

```ts
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
  <UiEntity
    uiTransform={{
      width: 700,
      height: 400,
      margin: { top: '35px', left: '500px' },
    }}
    uiBackground={{ color: Color4.Red() }}
  />
)
```

***File index.ts:***

```ts
import { ReactEcsRenderer } from '@dcl/sdk/react-ecs'
import { uiMenu } from './ui'

export function main() {
    ReactEcsRenderer.setUiRenderer(uiMenu, { virtualWidth: 1920, virtualHeight: 1080 })
}
```

Você também pode definir uma estrutura de Entities e Renderizá-la, tudo em um único comando em um `.tsx` File.

***File ui.tsx:***

```tsx
import ReactEcs, { ReactEcsRenderer, UiEntity } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export function setupUI() {
  ReactEcsRenderer.setUiRenderer(() => (
    <UiEntity
      uiTransform={{
        width: 700,
        height: 400,
        margin: { top: '35px', left: '500px' },
      }}
      uiBackground={{ color: Color4.Red() }}
    />
  ), { virtualWidth: 1920, virtualHeight: 1080 })
}
```

***File index.ts:***

```ts
import { setupUI } from './ui'

export function main() {
    setupUI()
}
```


> [!WARNING]
> **📔 Nota**: Todos os seus elementos de UI precisam estar aninhados na mesma estrutura e ter um único elemento pai na raiz da estrutura. Você só pode chamar `ReactEcsRenderer.setUiRenderer()` uma vez na Scene.


## UI Entities

Cada elemento na UI deve ser definido como uma `UiEntity`separada, seja uma imagem, texto, plano de fundo, uma caixa de alinhamento invisível etc. Assim como no espaço 3D da Scene, cada `UiEntity` tem seus próprios Components para definir posição, cor etc.

A sintaxe semelhante a React permite especificar cada Component como uma propriedade dentro da `UiEntity`, tornando o código mais curto e legível.

Os Components usados em uma `UiEntity` são diferentes dos usados em Entities regulares. Você não pode aplicar um Component de UI a uma Entity regular, nem um Component regular a uma Entity de UI.

Os seguintes Components estão disponíveis para uso em uma `UiEntity`:

* `uiTransform`
* `uiBackground`
* `uiText`
* `onMouseDown`
* `onMouseUp`
* `onMouseEnter`
* `onMouseLeave`
* [`uiInputBinding`](/docs/creator/content-creator-pt/scenes-sdk7/ui-2d/ui_input_binding/) — mantenha ações de Input pressionadas enquanto o elemento estiver pressionado, para criar controles personalizados

Assim como com tags HTML, você pode definir Components de fechamento automático ou aninhar um dentro de outro.


> [!CAUTION]
> **Aviso:** Mantenha wrappers de layout em tela cheia livres de manipuladores de Pointer. Adicionar qualquer um dos manipuladores `onMouse` ou `pointerFilter: 'block'`, a uma `UiEntity` dimensionada `100%` por `100%` faz com que ela capture todos os cliques na tela, deixando toda a sua outra UI e o mundo 3D inteiro sem possibilidade de clique. Em vez disso, coloque manipuladores no Panel ou Button que precisa deles. Consulte [Bloqueio de Pointer](/docs/creator/content-creator-pt/scenes-sdk7/ui-2d/ui_button_events/#pointer-blocking).


***File ui.tsx:***

```tsx
import ReactEcs, { ReactEcsRenderer, UiEntity } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
  // Entity pai
  <UiEntity
    uiTransform={{
      width: 200,
      height: 200,
      margin: { top: '250px', left: '500px' },
    }}
    uiBackground={{ color: Color4.Blue() }}
  >
    {/* Entity filha de fechamento automático */}
    <UiEntity
      uiTransform={{
        width: 400,
        height: 400,
        margin: { top: '35px', left: '500px' },
      }}
      uiText={{ value: `Olá, mundo!`, fontSize: 40 }}
    />
    {/* declaração de fechamento da Entity pai */}
  </UiEntity>
)
```

***File index.ts:***

```ts
import { ReactEcsRenderer } from '@dcl/sdk/react-ecs'
import { uiMenu } from './ui'

export function main() {
    ReactEcsRenderer.setUiRenderer(uiMenu, { virtualWidth: 1920, virtualHeight: 1080 })
}
```

Uma definição de um Module de UI só pode ter uma Entity de nível pai. Você pode definir quantas outras Entities quiser, mas todas devem caber dentro de uma estrutura com um único pai no topo.

## Escala virtual da tela

Defina uma largura e altura virtuais para a UI. Isso garante que sua UI tenha a mesma aparência em diferentes tamanhos de tela, independentemente do tamanho real da tela em pixels.

```ts
export function setupUi() {
    ReactEcsRenderer.setUiRenderer(uiComponent, { virtualWidth: 1920, virtualHeight: 1080 })
}
```

Se você definir uma largura virtual de 1920 e uma altura virtual de 1080, a UI será escalada para se ajustar ao tamanho da tela. Se a tela for 1920x1080, a UI será exibida no mesmo tamanho que o tamanho virtual. Se a tela for maior ou menor, quaisquer valores em pixels serão escalados para se ajustar ao tamanho virtual. Por exemplo, se a tela for 3840x2160, um item definido com 100 pixels de largura será exibido em 200 pixels reais.

O cálculo real do fator de escala da UI que é multiplicado pelos valores de pixel é [`Math.min(realWidth / virtualWidth, realHeight / virtualHeight)`](https://github.com/decentraland/js-sdk-toolchain/blob/main/packages/%40dcl/react-ecs/src/system.ts).

#### A arte em tela cheia é mais alta que seu tamanho virtual

O fator de escala mantém sua resolução de referência **dentro** da tela, portanto, em uma tela que não tenha o mesmo formato que seu tamanho virtual, o espaço extra ainda faz parte da UI. As porcentagens não são escaladas, então um elemento raiz com `width: '100%'`, `height: '100%'` cobre a tela inteira, que pode ser maior que seu tamanho virtual em uma direção.

Em uma tela 16:10 com o padrão `1920x1080`, o fator de escala vem da largura, e essa raiz em tela cheia mede **1920x1200** em pixels virtuais. Em uma tela mais larga, ela fica mais larga que 1920.

Portanto, para uma imagem de plano de fundo que deve preencher a tela:

* Exporte a arte **maior que 1920x1080** e deixe-a ser cortada, da mesma forma que um ajuste CSS `cover` funciona. Uma arte dimensionada exatamente em 1920x1080 deixa barras em uma tela mais alta.
* Mantenha tudo que deve permanecer visível (logotipos, títulos, Buttons) dentro de uma área centralizada de 1920x1080 e considere tudo fora dela como seguro de perder.

### Tamanho virtual padrão

O tamanho virtual é opcional, mas suas duas dimensões devem ser usadas juntas: passe ambas ou nenhuma. Quando você não passa nenhuma, um padrão da plataforma é aplicado:

| Plataforma        | Tamanho virtual padrão |
| ----------------- | ---------------------- |
| Dispositivo móvel | `1600x720`             |
| Desktop e Web     | `1920x1080`            |

Isso significa que os valores de pixel em sua UI são sempre escalados em relação a uma resolução de referência, mesmo quando você não passa nenhuma opção. Isso também se aplica quando a Scene usa apenas `addUiRenderer()` e nunca chama `setUiRenderer()`.

Três casos especiais:

* **Desativar a escala**: passe um tamanho inválido — qualquer valor que seja `0` ou menor — para desativar completamente a tela virtual. Os valores de pixel passam a ser usados como pixels brutos do Canvas, sem escala. Esta é a forma documentada de desativar a tela virtual, portanto, nada é registrado.

  ```ts
  ReactEcsRenderer.setUiRenderer(uiComponent, { virtualWidth: 0, virtualHeight: 0 })
  ```
* **Tamanhos incompletos**: fornecer apenas uma das duas dimensões também é inválido, portanto, isso desativa a tela virtual como no caso acima — o padrão da plataforma **não** é aplicado. Diferentemente da desativação acima, esta é reportada: um tamanho fornecido pela metade é um erro, e não uma escolha deliberada, portanto o SDK registra `Tamanho de tela virtual incompleto (…): ambas as dimensões são necessárias; portanto, a tela virtual está desativada e nenhuma escala de UI é aplicada.` uma vez por tamanho. Passe ambas as dimensões ou nenhuma.

  ```ts
  // Não faça isso — nenhuma escala é aplicada
  ReactEcsRenderer.setUiRenderer(uiComponent, { virtualWidth: 1920 })
  ```

  Vale a pena verificar isso duas vezes no Renderer principal, pois um tamanho incompleto nele também é uma configuração para toda a Scene: uma chamada `setUiRenderer()` que menciona *qualquer* dimensão prevalece sobre qualquer `addUiRenderer()`, portanto, o exemplo acima desativa a tela virtual para toda a Scene e descarta um tamanho válido passado a um Renderer adicional.
* **Tamanhos 16:9 em dispositivos móveis**: as telas de celulares são muito mais largas que 16:9, portanto, um Canvas virtual 16:9 adicionaria barras à UI. Se você passar um tamanho 16:9 (`1920x1080`, `1280x720`, …) e a Scene for executada em um dispositivo móvel, ele será substituído por `1600x720`, e uma mensagem será registrada no Console uma vez. Tamanhos não 16:9 em dispositivos móveis, e todos os tamanhos válidos em desktop ou web, são respeitados como estão.


> [!NOTE]
> **📔 Nota**: As unidades `vw` e `vh` são independentes da tela virtual: `1vw` é 1% da largura do Canvas e `1vh` é 1% da altura do Canvas, exatamente como em CSS. Elas também ignoram o [`screenInset`](#screen-inset-area), então, dentro do inset padrão `'device'` uma `width: '100vw'` é mais larga que uma `width: '100%'` — a primeira ocupa a tela inteira, a segunda preenche a área do inset.



> [!NOTE]
> **📱 Observação**: A detecção da plataforma é resolvida de forma assíncrona. Durante os primeiros frames, o SDK ainda não sabe que está sendo executado em um dispositivo móvel, portanto, a tela virtual começa em `1920x1080` e muda para `1600x720` quando a detecção é concluída — você pode ver a UI ser brevemente reescalada no carregamento, e a mensagem de substituição 16:9 aparece no Console um momento após a Scene iniciar. Passe um tamanho virtual explícito que não seja 16:9 se quiser uma única resolução de referência estável desde o primeiro frame.



> [!WARNING]
> **📔 O que mudou para Scenes escritas antes desta versão do SDK**
> 
> Três mudanças afetam as Scenes existentes. Nenhuma delas exige adesão, portanto, uma Scene que não seja alterada terá aparência diferente assim que atualizar sua versão do SDK:
> 
> 1. **Agora, uma tela virtual é aplicada por padrão.** Uma Scene que não passava opções costumava organizar valores de pixel como pixels brutos do Canvas. Agora ela é escalada em relação a `1920x1080` (`1600x720` em dispositivos móveis). Para restaurar o comportamento anterior, desative explicitamente a tela virtual com `setUiRenderer(ui, { virtualWidth: 0, virtualHeight: 0 })`.
> 2. **`screenInset` tem como padrão `'device'`.** Agora sua UI é posicionada dentro da área segura do dispositivo, portanto, em dispositivos móveis ela se move para dentro, e um elemento de nível raiz `100%` não é mais a tela inteira — planos de fundo em tela cheia, escurecedores e sobreposições param antes do entalhe e do indicador de início. Para restaurar o comportamento anterior, passe `setUiRenderer(ui, { screenInset: 'none' })`. Consulte [Área de inset da tela](#screen-inset-area).
> 3. **`devicePixelRatio` não participa do layout da UI.** A UI dimensionada em pixels agora é até 2–3 vezes maior em telas de alta densidade (retina e dispositivos móveis) do que antes. **Não há desativação para isto** — verifique novamente quaisquer tamanhos que foram ajustados manualmente e remova qualquer fator de escala que sua Scene tenha calculado por conta própria.


## Área de inset da tela

As telas não são totalmente utilizáveis: um celular reserva espaço para o entalhe, a barra de status, o indicador de início e os cantos arredondados, e cada Explorer desenha seu próprio HUD (minimapa, chat, …) sobre parte do Canvas. A propriedade opcional `screenInset` das opções do Renderer seleciona em qual área da tela sua UI é posicionada:

| Valor                 | Área em que a UI é posicionada                                                                                                                                                                                                |
| --------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `'device'` *(padrão)* | A área segura do dispositivo, excluindo o entalhe, a barra de status e os cantos arredondados. Lida de `UiCanvasInformation.screenInsetArea`.                                                                                 |
| `'interactable'`      | A área que o Explorer reporta como livre de seu próprio HUD (minimapa, chat, …). Lida de `UiCanvasInformation.interactableArea`. O que ela cobre depende de cada Explorer — verifique nas plataformas que você pretende usar. |
| `'none'`              | A tela inteira, com `0,0` no canto superior esquerdo.                                                                                                                                                                         |

```ts
// A UI fica livre do entalhe, da barra de status e dos cantos arredondados — este é o padrão
ReactEcsRenderer.setUiRenderer(uiComponent, { virtualWidth: 1920, virtualHeight: 1080 })

// A UI fica livre do HUD do próprio Explorer
ReactEcsRenderer.setUiRenderer(uiComponent, { screenInset: 'interactable' })

// A UI cobre a tela inteira; você mesmo gerencia as margens
ReactEcsRenderer.setUiRenderer(uiComponent, { screenInset: 'none' })
```

No desktop, os insets do dispositivo são zero, portanto `'device'` posiciona a UI sobre a tela inteira — o mesmo resultado que `'none'`. A área é relida a cada tick, então a UI acompanha os insets quando eles mudam, por exemplo, na rotação ou quando as barras do sistema aparecem e desaparecem.


> [!WARNING]
> **📔 Não ancore a UI no canto superior esquerdo.** Esse canto, e toda a borda esquerda, é onde o Explorer desenha seu minimapa, seu chat e, em dispositivos móveis, o joystick virtual. A UI da Scene posicionada ali é Renderizada, mas a própria UI do Client é desenhada sobre ela, portanto, ela fica coberta e seus Buttons geralmente não podem ser clicados. Ancore sua UI à direita ou centralize-a, ou use `screenInset: 'interactable'` para que o Explorer a posicione livre de seu próprio HUD. Consulte [Nunca ancore sua UI no canto superior esquerdo](/docs/creator/content-creator-pt/scenes-sdk7/conceber-a-experiencia/ux-ui-guide/#never-anchor-your-ui-to-the-top-left).



> [!WARNING]
> **📔 `'interactable'` não é uma operação sem efeito no desktop.** Diferentemente dos insets do dispositivo, a área interativa é *não* zero no Client desktop: ele reserva aproximadamente os 25% à esquerda da tela para sua própria UI, portanto `screenInset: 'interactable'` posiciona sua UI nos 75% restantes. Esse é o objetivo da opção, mas isso significa que ela também altera seu layout no desktop — faça uma ramificação com [`isMobile()`](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/detect-platform/) se você quiser isso somente em celulares.
> 
> **Suporte do Client**: `'interactable'` precisa de um Explorer que reporte a área. Há suporte no desktop e, em dispositivos móveis, a partir da versão do Client `1.12.1` — em Clients móveis mais antigos, o valor é reportado como zero, e a UI volta a cobrir a tela inteira. Essa mesma versão `1.12.1` também normaliza a área `'device'` entre Android e iOS, portanto, trate-a como o mínimo para qualquer layout que dependa de qualquer um dos insets. `'none'` se comporta da mesma forma em todos os lugares.



> [!WARNING]
> **📔 Nota**: Não envolva sua UI nos Components [`ScreenInsetArea` ou `InteractableArea`](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/safe-area/#wrap-part-of-your-ui-instead) enquanto também deixa o valor correspondente `screenInset` no Renderer — o inset seria aplicado duas vezes, empurrando a UI para dentro pelo dobro da margem. Use apenas `screenInset`ou defina-o como `'none'` e posicione o wrapper por conta própria.


### As três áreas em um dispositivo real

As capturas abaixo mostram a mesma Scene no mesmo celular, Renderizada três vezes com apenas o valor `screenInset` alterado. O retângulo magenta é a própria UI; os contornos são as áreas reportadas pelo Explorer — ciano é o Canvas completo, âmbar é `screenInsetArea`, verde é `interactableArea`.

<figure><img src="/docs/_remote/75b3a862ee39f3dbb1722d14.webp" alt="Scene UI covering the whole phone screen, including the notch strip and the areas under the client controls"><figcaption><p><code>screenInset: 'none'</code> — a UI cobre todo o Canvas. Seus cantos ficam sob o entalhe e atrás dos próprios controles do Client.</p></figcaption></figure>

<figure><img src="/docs/_remote/b3f7d16c8434488f8b197eda.webp" alt="Scene UI inset to the device safe area, clear of the notch"><figcaption><p><code>screenInset: 'device'</code> (o padrão) — a UI é deslocada para dentro da área segura do dispositivo, livre do entalhe e dos cantos arredondados. Ela ainda se sobrepõe aos controles do Client, que não fazem parte dessa área.</p></figcaption></figure>

<figure><img src="/docs/_remote/de3e57715b49b467501da7ac.webp" alt="Scene UI inset to the area the client reports as free of its own HUD"><figcaption><p><code>screenInset: 'interactable'</code> — a UI é posicionada dentro do retângulo que o Client designa para a UI da Scene. No Client móvel <code>1.12.1</code>, capturado aqui, isso exclui a coluna à esquerda (perfil, chat, joystick); os Buttons de ação no canto inferior direito são desenhados sobre a área intencionalmente.</p></figcaption></figure>


> [!NOTE]
> **💡 Dica**: `'interactable'` fornece a área que cada Explorer designa para a UI da Scene. Isso deliberadamente não é o mesmo que "todo controle do Client fica fora dela" — como mostra a terceira captura, os Buttons de ação móveis são desenhados propositalmente sobre a área. Tudo o que você colocar sob eles continua acessível, mas compete pelos mesmos toques, portanto, mantenha os [cantos movimentados](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/safe-area/#where-the-client-controls-live) em mente e verifique as plataformas que você pretende usar: o que a área cobre depende de cada Explorer e pode mudar entre versões.


Cada Renderer respeita seu próprio `screenInset`, portanto, a UI principal e qualquer UI adicionada com `addUiRenderer()` podem usar áreas diferentes ao mesmo tempo. Diferentemente do tamanho virtual, esse valor nunca é compartilhado entre Renderers.

## Múltiplos módulos de UI

Se sua Scene contiver vários Systems ou Modules que definem suas próprias UIs, você poderá Renderizar cada Module de UI com `ReactEcsRenderer.addUiRenderer()`. Isso é especialmente útil ao trabalhar em uma Scene complexa com vários Components de UI ou ao definir UIs para um [smart item](/docs/creator/content-creator-pt/scene-editor/interatividade/smart-items/), que deve ser utilizável independentemente do que houver no código do restante da Scene.

O `ReactEcsRenderer.addUiRenderer()` A Function exige que você forneça uma Entity como proprietária da UI. Pode ser qualquer Entity, inclusive uma Entity fictícia criada apenas para ser usada como proprietária da UI.

```ts
export function setupUi() {

    // Crie uma Entity fictícia para ser a proprietária da UI
    const dummyEntity = engine.addEntity()

    // Defina o Module de UI como uma Function que retorna um array de Modules de UI
    const uiComponent = () => [
      // Function que retorna um Module de UI,
      // Function que retorna um Module de UI
      // ...
    ]

    // Renderize o Module de UI com a Entity fictícia como proprietária
    ReactEcsRenderer.addUiRenderer(dummyEntity, uiComponent)
}
```

Este trecho pode existir independentemente de qualquer outro código de UI na Scene. O restante da Scene pode incluir um `ReactEcsRenderer.setUiRenderer()`, ou nenhum, e a UI ainda será Renderizada.

Uma chamada de `addUiRenderer()` também pode incluir uma largura e altura virtuais, assim como `setUiRenderer()`. No entanto, se a Scene tiver uma chamada de `setUiRenderer()` que também define uma largura e altura virtuais, a largura e altura virtuais da chamada de `addUiRenderer()` serão ignoradas.

```tsx
ReactEcsRenderer.addUiRenderer(dummyEntity, uiComponent, { virtualWidth: 1920, virtualHeight: 1080 })
```

O tamanho virtual é um único valor para toda a Scene, resolvido da seguinte maneira: o tamanho em `setUiRenderer()` prevalece; caso contrário, a primeira chamada de `addUiRenderer()` que forneceu um prevalece; e, se nenhum Renderer fornecer um, o [padrão da plataforma](#default-virtual-size) é aplicado. Opções que carregam apenas uma `screenInset` não contam como um tamanho fornecido.

O [`screenInset`](#screen-inset-area) funciona ao contrário — é por Renderer, portanto, cada Module de UI pode ficar em uma área diferente da tela:

```tsx
// Este widget fica livre do HUD do Explorer, independentemente do que a UI principal usa
ReactEcsRenderer.addUiRenderer(dummyEntity, uiComponent, { screenInset: 'interactable' })
```

Essa UI pode ser removida com `ReactEcsRenderer.removeUiRenderer(dummyEntity)` , além disso, se a Entity proprietária da UI for destruída, a UI também será removida. Se `ReactEcsRenderer.addUiRenderer()` for chamada novamente para a mesma Entity, mas com um UiRenderer diferente, o anterior será limpo e o novo o substituirá, mantendo seu lugar na ordem de empilhamento. Chamá-la novamente também é como você altera as opções de um Module de UI que já está Renderizado, por exemplo, seu `zIndex`.

### Ordem de empilhamento entre módulos de UI

Por padrão, os módulos de UI são empilhados na ordem em que aparecem pela primeira vez, com os posteriores na frente. Os módulos que aparecem pela primeira vez no mesmo frame — normalmente tudo o que é registrado antes do primeiro frame da cena — são desenhados com a UI principal de `setUiRenderer()` na parte de trás, seguido pelos outros módulos na ordem em que foram adicionados. Para controlar isso explicitamente, passe um `zIndex` nas opções do renderer: módulos com um valor mais alto de `zIndex` são renderizados na frente dos que têm um valor mais baixo, independentemente da ordem em que apareceram. Os módulos deixados com `0` mantêm a ordem padrão. A opção funciona da mesma forma em `setUiRenderer()`, então a UI principal também pode ser colocada na frente dos outros módulos.

```tsx
// Registrado primeiro, mas sempre na frente dos outros módulos de UI
ReactEcsRenderer.addUiRenderer(dummyEntity, uiComponent, { zIndex: 10 })

// A UI principal fica atrás de todos os módulos que não definem um valor menor
ReactEcsRenderer.setUiRenderer(mainUi, { zIndex: -10 })
```

Esta é uma configuração diferente de `zIndex` de um `UiEntity`, que ordena apenas [irmãos dentro de um módulo de UI](/docs/creator/content-creator-pt/scenes-sdk7/ui-2d/ui-positioning/#z-index). A opção do renderer ordena módulos inteiros entre si, e os elementos dentro de cada módulo mantêm sua própria `zIndex`.

### Compartilhar uma única instrução setUiRenderer

Em vez de chamar `ReactEcsRenderer.addUiRenderer()` para cada módulo de UI, você pode chamar `ReactEcsRenderer.setUiRenderer()` uma vez com um array de módulos de UI, que podem estar em arquivos diferentes.

```ts
const uiComponent = () => [
  // Function que retorna um Module de UI,
  // Function que retorna um Module de UI
  // ...
]

ReactEcsRenderer.setUiRenderer(uiComponent, { virtualWidth: 1920, virtualHeight: 1080 })
```

Abaixo está um exemplo mais completo:

***File ui.tsx:***

```ts
export function UIModule1() {
  return (
    <UiEntity
      uiTransform={{
        flexDirection: 'column',
        alignItems: 'center',
        justifyContent: 'space-between',
        positionType: 'absolute',
        position: { right: '3%', bottom: '3%' },
      }}
    >
      <Label value="Hello World!" fontSize={18} textAlign="middle-center" />
    </UiEntity>
  )
}

export function UIModule2() {
  return (
    <UiEntity
      uiTransform={{
        flexDirection: 'column',
        alignItems: 'center',
        justifyContent: 'space-between',
        positionType: 'absolute',
        position: { right: '3%', top: '3%' },
      }}
    >
      <Label
        value="Aqui está mais um pouco de UI!"
        fontSize={18}
        textAlign="middle-center"
      />
    </UiEntity>
  )
}
```

***File index.ts:***

```ts
import { ReactEcsRenderer } from '@dcl/sdk/react-ecs'
import { UIModule1, UIModule2 } from './ui'

export function main() {
    ReactEcsRenderer.setUiRenderer(() => [
      UIModule1(),
      UIModule2(),
      // ...
      // A linha abaixo é para usar a biblioteca DCL UI Toolkit
      // https://github.com/decentraland-scenes/dcl-ui-toolkit
      ui.render(),
    ], { virtualWidth: 1920, virtualHeight: 1080 })
}
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/ui-2d/onscreen-ui.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
