> Tipos especiais de entities para a UI, incluindo dropdowns e caixas de entrada.

# Tipos Especiais de UI

Existem certos tipos especiais de Entity que permitem alguns tipos especiais de interações.

## Dropdown

Crie uma `Dropdown` Entity para permitir que os usuários expandam e selecionem um item de uma lista.

Um `Dropdown` A Entity deve ter pelo menos as seguintes propriedades:

* `options`: Quais valores exibir quando o dropdown estiver expandido. Forneça um array com um valor de string para cada opção. O primeiro valor do array é exibido como a opção padrão.
* `onChange`: Uma função que é executada toda vez que um valor é selecionado no dropdown, usando

Você também pode configurar outros componentes do `Dropdown` entity, como um `uiTransform`, como em outras entities de UI. O texto do dropdown aceita as mesmas propriedades de estilo que um `Label`, como `color`, `fontSize`, `font` e `fontSrc` para uma [fonte personalizada](/docs/creator/content-creator-pt/scenes-sdk7/ui-2d/ui_text/#custom-fonts).

***arquivo ui.tsx:***

```tsx
import { UiEntity, Label, Dropdown, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'
import { Color4Type } from '@dcl/sdk/ecs'

function selectOption(index: number) {
  switch (index) {
    case 0:
      textColor = Color4.Red()
      break
    case 1:
      textColor = Color4.Blue()
      break
    case 2:
      textColor = Color4.Green()
      break
  }
}

let textColor: Color4Type = Color4.Red()

export const uiMenu = () => (
    <UiEntity
      uiTransform={{
        width: '200px',
        height: '100px',
        alignContent: 'auto',
        flexDirection: 'column',
        alignSelf: 'center',
      }}
    >
      <Label
        value="Selecione uma cor"
        fontSize={18}
        color={textColor}
        uiTransform={{
          width: '140px',
          height: '40px',
        }}
      />
      <Dropdown
        options={[`Vermelho`, `Azul`, `Verde`]}
        onChange={selectOption}
        uiTransform={{
          width: '100px',
          height: '40px',
        }}
      />
    </UiEntity>
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


## Texto de entrada

Crie um `Input` entity para permitir que os usuários digitem texto. Os jogadores devem primeiro clicar nesta caixa antes de poder escrever nela.

O comportamento da `Input` entity é gerenciado pelas seguintes propriedades:

* `onSubmit`: Uma função que é executada quando o jogador pressiona Return/Enter, usando o texto fornecido como entrada. O campo de texto é **não** limpo automaticamente quando isso acontece; você precisa limpá-lo manualmente (veja o exemplo abaixo).
* `onChange`: Uma função que é executada toda vez que um valor no texto de entrada é alterado. À medida que o jogador digita na caixa, esta função é executada uma vez para cada caractere que é adicionado ou removido.
* `disabled`: Se *true*, o jogador não poderá interagir com a entity de entrada.

O exemplo a seguir usa `onSubmit` para registrar o texto fornecido no console.

```tsx
import { UiEntity, Input, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'


export const uiMenu = () => (
  <UiEntity
    uiTransform={{
      width: 400,
      height: 300,
      positionType: 'absolute',
      position: {
        left: '35%',
        top: '40%',
      },
    }}
    uiBackground={{
      color: Color4.Gray(),
    }}
  >
    <Input
      onSubmit={(value) => {
        console.log('valor enviado: ' + value)
      }}
      fontSize={35}
      placeholder={'digite algo'}
      placeholderColor={Color4.Black()}
      uiTransform={{
        width: '400px',
        height: '80px',
      }}
    ></Input>
  </UiEntity>
)
```

É uma boa prática fornecer tanto um botão para enviar quanto tratar o `onSubmit` evento quando o jogador pressiona a tecla Enter/Return. O exemplo a seguir mostra como você pode fazer isso.


> [!WARNING]
> **📔 Nota**: Não **não** vincule o `value` propriedade do `Input` diretamente ao que o jogador digita por meio de `onChange`. Isso cria uma ida e volta entre a scene e o Explorer que faz com que caracteres sejam perdidos ao digitar rapidamente. Em vez disso, mantenha `value` como uma string vazia e altere-a apenas quando quiser limpar o campo de entrada.


Para limpar o campo de entrada, defina `value` como `' '` (um único espaço). O Explorer trata isso como vazio e reativa o texto do placeholder. Como a função de UI é executada a cada frame, use um sinalizador para aplicar a limpeza apenas por um frame.

```tsx
import { UiEntity, Input, Button, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

let inputText = ''
let clearInput = false

export const uiMenu = () => {
  const inputValue = clearInput ? ' ' : ''
  if (clearInput) clearInput = false

  return (
    <UiEntity
      uiTransform={{
        width: 400,
        height: 300,
        positionType: 'absolute',
        position: {
          left: '35%',
          top: '40%',
        },
        flexDirection: 'column',
      }}
      uiBackground={{
        color: Color4.Gray(),
      }}
    >
      <Input
        onSubmit={() => {
          console.log('valor enviado: ' + inputText)
          inputText = ''
          clearInput = true
        }}
        fontSize={35}
        placeholder={'digite algo'}
        placeholderColor={Color4.Black()}
        value={inputValue}
        onChange={(value) => { inputText = value }}
        uiTransform={{
          height: '80px',
          margin: '15px',
        }}
      />
      <Button
        value="Enviar texto"
        variant="primary"
        uiTransform={{ alignSelf: 'center', padding: '25px' }}
        onMouseDown={() => {
          console.log('valor enviado: ' + inputText)
          inputText = ''
          clearInput = true
        }}
      />
    </UiEntity>
  )
}
```

As propriedades a seguir também estão disponíveis para personalizar a aparência do campo de texto, a maioria delas semelhante às presentes em `Label` entities:

* `placeholder`: String a exibir antes de o jogador começar a digitar qualquer coisa. É útil tornar esse texto uma dica sobre o que ele deve escrever.
* `placeholderColor`: A cor a ser usada para o texto do placeholder, como um [Color4](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/color-types/).


> [!NOTE]
> **💡 Dica**: Use um tom mais claro da cor do texto que o jogador escreve.


* `fontSize`: O tamanho do texto, como um número.

  > OBS.: O `fontSize` não é afetado pelo tamanho da sua entity ou das entities pai.
* `color`: A cor do texto que o jogador escreve, como um [Color4](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/color-types/).


> [!WARNING]
> **📔 Nota**: Certifique-se de usar uma cor diferente da `placeholderColor`.


* `font`: A fonte a ser usada. Os valores compatíveis são: - `'sans-serif'` *(predefinição)* - `'serif'` - `'monospace'`
* `fontSrc`: Caminho para um `.ttf` arquivo de fonte em sua scene, para renderizar o texto com uma fonte personalizada. `font` é usado como alternativa. Veja [Fontes personalizadas](/docs/creator/content-creator-pt/scenes-sdk7/ui-2d/ui_text/#custom-fonts).
* `textAlign`: Como o texto ficará alinhado com seu pai. Ele usa um valor de \`TextAlignType\`. TextAlignType = 'top-left' | 'top-center' | 'top-right' | 'middle-left' | 'middle-center' | 'middle-right' | 'bottom-left' | 'bottom-center' | 'bottom-right';

Você também pode configurar outros componentes do `Input` entity, como um `uiTransform`, `OnMouseDown` como em outras entities de UI.

```tsx
import { UiEntity, Input, Label, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

var currentTextString = ''

export const uiMenu = () => (
    <UiEntity
      uiTransform={{
        width: '50%',
        height: 150,
        flexDirection: 'column',
        alignContent: 'flex-start',
        margin: { left: 20, top: 20 },
        padding: { left: 10, top: 10, right: 10 },
        alignSelf: 'center',
      }}
      uiBackground={{ color: Color4.Gray() }}
    >
      <Input
        onChange={(e) => {
          currentTextString = 'você escreveu: ' + e
        }}
        fontSize={35}
        placeholder={'digite algo'}
        placeholderColor={Color4.Gray()}
      />
      <Label value={currentTextString} fontSize={40} />
    </UiEntity>
)

```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/ui-2d/ui_special_types.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
