> Escreva texto em UI entities.

# Texto da UI

Adicione texto à sua UI criando um `Label` entity.

A `Label` entity tem os seguintes campos que podem ser configurados:

* `value`: A string a ser exibida
* `fontSize`: O tamanho do texto, como um número.

  > NOTA: O `fontSize` não é afetado pelo tamanho da sua entity ou das entities pai.
* `color`: A cor do texto, como uma [Color4](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/color-types/).
* `font`: A fonte a ser usada. Os valores suportados são:
  * `'serif'`
  * `'sans-serif'` *(padrão)*
  * `'monospace'`
* `fontSrc`Caminho para um `.ttf` arquivo de fonte na sua scene, para renderizar o texto com uma fonte personalizada. `font` é usado como fallback. Veja [Fontes personalizadas](#custom-fonts).
* `textAlign`: Como o texto se alinhará com o seu parent. Ele usa um valor do `TextAlignType` type. TextAlignType = 'top-left' | 'top-center' | 'top-right' | 'middle-left' | 'middle-center' | 'middle-right' | 'bottom-left' | 'bottom-center' | 'bottom-right';
* `textWrap`: Define se o texto usa quebras de linha para evitar exceder a largura máxima. Ele vem ativado por padrão (`'wrap'`), para desativá-lo passe o valor `'nowrap'`.


> [!WARNING]
> **📔 Nota**: O `fontSize` não é afetado pelo tamanho da sua entity ou das entities pai.


A `Label` entity também pode ter outros components comuns encontrados em outros tipos de UI entities, como `uiTransform` e `uiBackground`.

***arquivo ui.tsx:***

```ts
import { UiEntity, Label, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
	<UiEntity uiTransform={{ width: 'auto', height: 'auto' }}>
		<Label
			value="Este é um rótulo"
			color={Color4.Red()}
			fontSize={29}
			font="sans-serif"
			textAlign="top-left"
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


> [!WARNING]
> **📔 Nota**: Todos os trechos a seguir nesta página assumem que você tem um `.ts` semelhante ao acima, executando o `ReactEcsRenderer.setUiRenderer()` função.


Se uma linha de texto for longa demais para caber na largura atribuída, ou na largura máxima do seu container, o texto continuará na linha seguinte. Você pode desativar isso alterando o valor de `textWrap` propriedade para `'nowrap'`.

Você também pode forçar uma quebra de linha adicionando explicitamente `\n` à string.

```ts
import { UiEntity, Label, ReactEcs } from '@dcl/sdk/react-ecs'

export const uiMenu = () => (
	<UiEntity uiTransform={{ width: 700, height: 400 }}>
		<Label 
			value="Olá, mundo!\nEste outro trecho é bem longo. Provavelmente não caberá em uma única linha, então incluirá uma quebra de linha em algum ponto.\nQuarta linha"
			textWrap= {`nowrap`} 
		/>
	</UiEntity>
)
```

Se nenhum valor explícito `height` ou `width` for definido no `uiTransform` do container, o container usará o valor `auto`, que se ajusta para caber em todo o texto. Você pode definir um `maxWidth` e um `maxHeight` para garantir que ele não exceda certos limites. Você também pode usar `minWidth` e `minHeight` para garantir que o container não fique pequeno demais, mesmo que o texto seja mais curto.


> [!WARNING]
> **📔 Nota**: Não dependa desse ajuste automático para texto. Quanto espaço uma `Label` ocupa quando você deixa seu tamanho indefinido varia entre explorers. O explorer baseado em Bevy mede o texto renderizado e o organiza de acordo. O explorer do Unity lhe dá quase nenhuma altura, mas ainda desenha as letras, então labels empilhados acabam um sobre o outro e um parent dimensionado a partir do texto colapsa para nada.
> 
> Dê a cada `Label` um valor explícito `width` **e** `height` , `uiTransform`em seu `fontSize: 20` precisa de cerca de `height: 60`.
> 
> Como isso difere por explorer, um preview que parece correto em um client não prova que o layout esteja correto em outro.


## Fontes personalizadas

Para renderizar um `Label` com sua própria fonte, adicione uma TrueType (`.ttf`) à pasta da sua scene e defina a `fontSrc` propriedade ao seu path, relativo à pasta raiz da scene. Mantenha `font` definido também: ele é o fallback usado sempre que a fonte personalizada não puder ser carregada.


> [!WARNING]
> **📔 Nota**: `fontSrc` precisa de `@dcl/sdk` 7.31.0 ou posterior. Em versões anteriores, a propriedade não existe.


```tsx
<Label
	value="Bem-vindo!"
	fontSize={40}
	uiTransform={{ width: 300, height: 60 }}
	fontSrc="assets/fonts/Lobster-Regular.ttf"
	font="sans-serif"
/>
```

O `Button` entity, o `uiText` de um `UiEntity` e o [`Input` e `Dropdown`](/docs/creator/content-creator-pt/scenes-sdk7/ui-2d/ui_special_types/) entities aceitam `fontSrc` também, e o mesmo vale para o [`TextShape`](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/text/#custom-fonts) component para texto no espaço 3D.

Tenha o seguinte em mente ao usar fontes personalizadas:

* **Apenas `.ttf` arquivos são suportados**. Fontes OpenType com contornos CFF (geralmente `.otf`), coleções de fontes e web fonts são rejeitadas, e o texto é renderizado com o fallback `font`.
* **A fonte precisa ser um arquivo na sua scene**. `fontSrc` não pode apontar para uma URL externa, e nomes de famílias de fontes (por exemplo, do Google Fonts) também não são suportados. Baixe o `.ttf` arquivo e inclua-o na pasta da sua scene. Certifique-se de que a licença permite redistribuí-lo.
* **No Desktop Client, fontes personalizadas só aparecem depois que os assets da scene são otimizados**. Quando você publica uma scene, suas fontes são convertidas em pacotes de assets otimizados junto com seus modelos 3D, e a fonte personalizada aparece assim que essa conversão termina. Até lá, o texto usa o fallback `font`. O mesmo se aplica a previews locais: a fonte personalizada só aparece ao fazer preview com assets otimizados (o **Compress Assets** no Creator Hub, ou `npm run start -- --asset-bundles` da CLI). Veja [Pré-visualização com assets otimizados](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/preview-scene/#preview-with-optimized-assets). O cliente web Bevy carrega o arquivo diretamente, então a fonte personalizada aparece imediatamente lá, inclusive em prévias locais. `.ttf` diretamente, então lá a fonte personalizada aparece imediatamente, inclusive em prévias locais.
* **Não suportado em mobile**. O aplicativo móvel de Decentraland ignora `fontSrc` e renderiza o fallback `font`. Escolha um fallback que ainda fique bom visualmente e não dependa da fonte personalizada para fazer o texto caber em um espaço fixo. Veja [Recursos ausentes no mobile](/docs/creator/content-creator-pt/criar-para-mobile/cliente-movel/missing-features/).
* **Um arquivo é uma face**. Para texto em negrito e regular, adicione ambos os `.ttf` arquivos e aponte cada texto para seu próprio arquivo.
* **O UI Designer do Creator Hub não mostra nem edita `fontSrc`**. Adicione-o ao elemento no seu código manualmente. O UI Designer só altera as propriedades que você edita nele, então um `fontSrc` que você escreveu permanece no lugar. Seu canvas não mostra a fonte personalizada.
* **Os caracteres que a fonte não inclui** são desenhados com a fonte sans-serif integrada. Os caracteres não latinos que a fonte inclui são renderizados na primeira vez que aparecem, o que pode causar uma breve pausa.

Para a lista completa de limites, como o tamanho máximo do arquivo, veja [Fontes personalizadas](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/text/#custom-fonts) na página Text.

## Não use emoji em texto de UI

Deixe emoji fora de qualquer `Label` ou `Button` `value`, `uiText.value`, `Input` `placeholder`, e `Dropdown` opção.

O SDK não inclui glifos de emoji. Se um emoji aparece depende das fontes que cada explorer empacota, e o explorer do Unity não tem nenhuma, então o caractere aparece como uma caixa vazia ou desaparece por completo. O mesmo cuidado se aplica a outros Unicode decorativos, como setas e caracteres de desenho de caixas.

Para um rótulo pictórico, inclua a arte em vez disso: coloque uma imagem em um pequeno `UiEntity` ao lado do texto, usando um `uiBackground` com um `textura`. Veja [Fundos de UI](/docs/creator/content-creator-pt/scenes-sdk7/ui-2d/ui_background/).

```ts
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
	<UiEntity
		uiTransform={{
			minWidth: 100,
			maxWidth: 300,
			height: 'auto',
			alignSelf: 'center',
			padding: 10,
		}}
		uiBackground={{
			color: Color4.Red(),
		}}
		uiText={{
			value: 'Olá, mundo!',
			fontSize: 18,
		}}
	/>
)
```

## Tamanho de texto responsivo

Use a `scaleFontSize()` função para fornecer valores de fonte que se ajustem ao tamanho da tela do jogador. Ao definir a `fontSize` propriedade de uma entity de UI de texto, passe esta função em vez de um único número.

```ts
import { scaleFontSize } from '@dcl/sdk/react-ecs'
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
  <UiEntity
    uiTransform={{
      width: 'auto',
      height: 'auto',
      alignSelf: 'center',
      padding: 10,
    }}
    uiText={{
      value: 'Olá, mundo!',
      fontSize: scaleFontSize(15)
    }}
  />
)
```

O `scaleFontSize()` função requer dois parâmetros:

* `fontSize`: O tamanho base da fonte a ser usado.
* `scaleUnit` *(opcional)*: O fator de escala. Isso determina se o texto deve ser ajustado com base na largura ou na altura da tela, e um multiplicador de quanto adaptar. Padrão: `0.39` (um número, então é interpretado como relativo a *width*). Os valores podem ser:
  * *Número*: Um número simples; neste caso, ele é interpretado como relativo a *width*
  * *String terminando em **vw***: Isso torna o número relativo à largura da view. Por exemplo `"0.8vw"`
  * *String terminando em **vh***: Isso torna o número relativo à altura da view. Por exemplo `"0.8vh"`


> [!NOTE]
> **💡 Dica**: Esta função funciona de forma semelhante ao CSS `calc()` função.


O valor de `scaleUnit` é uma porcentagem da largura ou da altura da janela. Portanto, um `scaleUnit` de `"100vw"` é 100% da largura da tela, um valor de `"0.5vw"` é 0,5% da largura da tela.

A fórmula que `scaleFontSize()` a seguir multiplica a largura ou a altura da tela pelo `scaleUnit`, e adiciona a isso o `fontSize` passado no primeiro parâmetro.

```ts
final font = fontSize + (screen width * scaleUnit / 100)
```

Por exemplo, no trecho abaixo usa um `scaleUnit` valor de 0,8. Se a largura da tela for *1280px*, isso resultará em texto de tamanho de **25.24**, seguindo a equação `15 + (1280 * 0.8 / 100)`.


> [!WARNING]
> **📔 Nota**: `scaleFontSize()` retorna um **number**, então seu resultado é um tamanho em *virtual* pixels — ele ainda é multiplicado pelo [UI scale factor](/docs/creator/content-creator-pt/scenes-sdk7/ui-2d/onscreen-ui/#screen-virtual-scale) antes de ser desenhado. No exemplo acima, com a `1920x1080` tela virtual padrão e um canvas de 1280px de largura, o fator de escala é `1280 / 1920 = 0.667`, então o texto é renderizado em cerca de 16,8px. Se você quiser um tamanho de fonte medido diretamente em relação ao canvas, ignorando a tela virtual, passe um `vw`/`vh` **string** diretamente para `fontSize` em vez disso — por exemplo `fontSize: '1.8vh'`.


```ts
import { scaleFontSize } from '@dcl/sdk/react-ecs'
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'

export const uiMenu = () => (
  <UiEntity
    uiTransform={{
      width: 'auto',
      height: 'auto',
      alignSelf: 'center',
      padding: 10,
    }}
    uiText={{
      value: 'Olá, mundo!',
      fontSize: scaleFontSize(15, 0.8)
    }}
  />
)
```


> [!NOTE]
> **💡 Dica**: Se você não tiver diferentes tamanhos de tela para testar, você pode tentar usar o Web Explorer e redimensionar a janela onde executa o preview. O texto se ajustará instantaneamente toda vez que você alterar a janela.


Como alternativa a `scaleFontSize()`, passe uma `vw`/`vh` string diretamente como o `fontSize` — por exemplo `fontSize: '1.8vh'` — que dimensiona o texto em relação ao canvas e ignora completamente o fator de escala da UI. Não **não** multiplique tamanhos de fonte por um fator que você mesmo calcule a partir de `UiCanvasInformation`: o SDK já aplica um, veja [Tamanho responsivo da UI](/docs/creator/content-creator-pt/scenes-sdk7/ui-2d/ui-positioning/#responsive-ui-size).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/ui-2d/ui_text.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
