> Como adicionar texto à sua scene

# Texto

Adicione texto a uma cena usando o `TextShape` component. Esse texto fica em uma posição

O texto em Decentraland suporta todos os *utf8* caracteres, incluindo caracteres orientais e especiais.


> [!WARNING]
> **📔 Nota**: Este componente é útil para labels e UIs no mundo que existem no espaço 3D da scene, não para a UI HUD 2D do jogador.


Para adicionar texto como um label a uma entity existente, uma boa prática é criar uma segunda entity que tenha o `TextShape` component e defini-la como filha da outra entity.

## Use o Scene Editor no Creator Hub

A maneira mais fácil de colocar texto in-world é adicionar um **Text** [Smart item](/docs/creator/content-creator-pt/scene-editor/interatividade/smart-items/) visualmente no Scene Editor. Você pode então definir todos os campos disponíveis na UI do Scene Editor.

![](/docs/_remote/339b81b8af14deb7d4666a01.webp)

## Criar um componente de texto

O exemplo a seguir mostra como criar um `TextShape` componente e adicioná-lo a uma entity via código.

```ts
const sign = engine.addEntity()

Transform.create(sign, {
	position: Vector3.create(8, 1, 8),
})

TextShape.create(sign, {
	text: 'Olá Mundo',
})
```


> [!WARNING]
> **📔 Nota**: Se a entity com o componente de texto for filha de outra entity, ela será afetada pela escala do parent. Se o parent for escalado de forma desigual ao longo do seu eixo, isso fará com que o texto também seja esticado ou comprimido.



> [!WARNING]
> **📔 Nota**: `TextShape` components não são clicáveis. `PointerEvents` components não são ativados quando usados em entities que têm um `TextShape` component.



> [!WARNING]
> **📔 Nota**: `TextShape` deve ser importado via
> 
> > `import { TextShape } from "@dcl/sdk/ecs"`
> 
> Veja [Imports](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/coding-scenes/#imports) para saber como lidar facilmente com isso.


## Alterar o valor do texto

Ao criar um novo componente de texto, você atribui a ele uma string para exibir. Essa string é armazenada no `campo text` .

Se você quiser alterar a string exibida pelo componente, pode fazê-lo a qualquer momento alterando o `campo text` campo em uma [versão mutável](/docs/creator/content-creator-pt/scenes-sdk7/padroes-de-programacao/mutable-data/) do componente.

```ts
const mutableText = TextShape.getMutable(myEntity)

mutableText.text = 'new string'
```

## Propriedades básicas de texto

O `TextShape` o componente tem várias propriedades que podem ser definidas para estilizar o texto. Abaixo estão algumas das mais comuns:

* `font`: Valor do enum `Font`.
* `fontSrc`: *string*. Caminho para um arquivo de fonte `.ttf` na sua scene, para renderizar o texto com uma fonte personalizada. Veja [Fontes personalizadas](#custom-fonts).
* `fontSize`: *number*. Uma entity com font 10 tem 1 metro de altura.
* `textColor`: *Color4* objeto. *Color4* objetos armazenam uma *cor RGB* como três números de 0 a 1, mais *alpha* para transparência. Veja [tipos de cor](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/color-types/) para mais detalhes.

```ts
TextShape.create(sign, {
	text: 'Olá Mundo',
	textColor: { r: 1, g: 0, b: 0, a: 1 },
	fontSize: 5,
	font: Font.F_SANS_SERIF,
})
```

## Fonts

As text shapes podem usar fontes do enum `Font`. Este enum inclui atualmente as seguintes fontes:

* `Font.F_SANS_SERIF`
* `Font.F_SERIF`
* `Font.F_MONOSPACE`

Por padrão, usa `Font.F_SANS_SERIF`.

```ts
TextShape.create(sign, {
	text: 'Olá Mundo',
	textColor: { r: 1, g: 0, b: 0, a: 1 },
	fontSize: 5,
	font: Font.F_SANS_SERIF,
})
```


> [!NOTE]
> **💡 Dica**: Se estiver usando o VS studio ou algum outro IDE, digite `Font.` e você deverá ver uma lista de sugestões com todas as fontes disponíveis.


### Fontes personalizadas

Para renderizar texto com sua própria fonte, adicione um arquivo de fonte TrueType (`.ttf`) à pasta da sua scene e defina a `fontSrc` propriedade para o seu caminho, relativo à pasta raiz da scene (assim como o `src` de um `GltfContainer`). Mantenha `font` definido também: ele é o fallback usado sempre que a fonte personalizada não puder ser carregada.


> [!WARNING]
> **📔 Nota**: `fontSrc` precisa de `@dcl/sdk` 7.31.0 ou posterior. Em versões anteriores, a propriedade não existe.


```ts
TextShape.create(sign, {
	text: 'Olá Mundo',
	fontSize: 5,
	fontSrc: 'assets/fonts/Lobster-Regular.ttf',
	font: Font.F_SANS_SERIF,
})
```

Os mesmos `fontSrc` a propriedade existe na UI `Label`, `Button`, `Input` e `Dropdown` entities, e no `uiText` de um `UiEntity`. Veja [Fontes personalizadas na UI](/docs/creator/content-creator-pt/scenes-sdk7/ui-2d/ui_text/#custom-fonts).

Tenha o seguinte em mente ao usar fontes personalizadas:

* **Apenas `.ttf` arquivos são suportados**. O arquivo deve ser uma fonte TrueType. Fontes OpenType com contornos CFF (normalmente `.otf`), coleções de fontes (`.ttc`) e fontes web (`.woff`, `.woff2`) são rejeitadas, e o texto é renderizado com o fallback `font`.
* **A fonte precisa ser um arquivo na sua scene**. `fontSrc` não pode apontar para uma URL externa, e nomes de famílias de fontes (por exemplo, do Google Fonts) também não são suportados. Baixe o `.ttf` arquivo, inclua-o na pasta da sua scene e referencie o seu caminho. Certifique-se de que a licença da fonte permite redistribuí-la; a maioria das fontes no Google Fonts usa a SIL Open Font License, que permite.
* **No Desktop Client, fontes personalizadas só aparecem depois que os assets da scene são otimizados**. Quando você publica uma scene, suas fontes são convertidas em pacotes de assets otimizados junto com seus modelos 3D, e a fonte personalizada aparece assim que essa conversão termina. Até lá, o texto usa o fallback `font`. Scenes publicadas antes de fontes personalizadas serem suportadas só têm suas fontes convertidas quando são publicadas novamente. O mesmo se aplica a prévias locais: a fonte personalizada só aparece ao pré-visualizar com assets otimizados (a opção **Compress Assets** no Creator Hub, ou `npm run start -- --asset-bundles` da CLI). Veja [Pré-visualização com assets otimizados](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/preview-scene/#preview-with-optimized-assets). O cliente web Bevy carrega o arquivo diretamente, então a fonte personalizada aparece imediatamente lá, inclusive em prévias locais. `.ttf` diretamente, então lá a fonte personalizada aparece imediatamente, inclusive em prévias locais.
* **Não suportado em mobile**. O aplicativo móvel de Decentraland ignora `fontSrc` e renderiza o fallback `font`. Escolha um fallback que ainda fique bom visualmente e não dependa da fonte personalizada para fazer o texto caber em um espaço fixo. Veja [Recursos ausentes no mobile](/docs/creator/content-creator-pt/criar-para-mobile/cliente-movel/missing-features/).
* **O arquivo da fonte pode ter no máximo 16 MB**. Arquivos maiores são rejeitados e o fallback `font` é usado. Fontes com contornos de glifo extremamente complexos também podem ser rejeitadas.
* **Caracteres**: Caracteres latinos (ASCII e Latin-1) são pré-renderizados. Quaisquer outros caracteres que a fonte inclua, como cirílico ou chinês, são renderizados da primeira vez que aparecem na scene, o que pode causar uma pequena pausa. Caracteres que a fonte não inclui são desenhados com a fonte sans-serif integrada.
* **Um arquivo é uma face**. Um `.ttf` arquivo contém um único peso e estilo. Para um título em negrito e texto normal do corpo, adicione ambos os arquivos e aponte cada texto para o seu próprio arquivo. Use arquivos de fonte estáticos: se uma família de fontes for distribuída apenas como fonte variável, use suas instâncias estáticas.
* **As fontes são compartilhadas**: todas as entities de texto que usam a mesma `fontSrc` compartilham uma única cópia da fonte, então reutilizar uma fonte na scene não custa mais do que usá-la uma vez. Prefira algumas poucas fontes em vez de muitas diferentes, já que cada fonte tem sua própria textura. Cada arquivo de fonte adiciona um download otimizado de cerca de 700 KB.

Quando o cliente desktop não consegue usar uma fonte personalizada, ele registra uma mensagem na `SDK_FONTS` categoria do console que nomeia o arquivo e explica o motivo. Por exemplo, `font_src "https://example.com/font.ttf" não pode ser uma URL externa`, ou que o arquivo não faz parte do conteúdo da scene, ou que a scene ainda não tem uma fonte convertida para ele. Essas mensagens ficam ocultas por padrão. Para mostrá-las, digite `/log-matrix enable SDK_FONTS Warning` e `/log-matrix enable SDK_FONTS Log` no chat, recarregue a scene com `/reload`, depois abra a pasta de logs com `/logs`. Se a fonte não foi convertida, verifique se o arquivo é uma TrueType `.ttf` com menos de 16 MB e publique novamente.

## Propriedades de alinhamento e padding do texto

O `TextShape` o componente cria uma caixa de texto que tem tamanho, padding etc.

* `textAlign`: Selecione um valor do `TextAlignMode` enum. Os valores possíveis incluem todas as combinações entre vertical (*top*, *middle*, *bottom*) e horizontal (*left*, *center*, *right*) do alinhamento.
* `width`: *number*. A largura da caixa de texto.
* `height`: *number*. A altura da caixa de texto.
* `paddingTop`: *number*. Espaço entre o texto e a borda da caixa de texto.
* `paddingRight`: *number*. Espaço entre o texto e a borda da caixa de texto.
* `paddingBottom`: *number*. Espaço entre o texto e a borda da caixa de texto.
* `paddingLeft`: *number*. Espaço entre o texto e a borda da caixa de texto.


> [!NOTE]
> **💡 Dica**: Se um texto for para flutuar no espaço, é uma boa ideia adicionar um [`Billboard` component](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/entity-positioning/#face-the-player) para que o texto gire para sempre ficar de frente para o jogador e ser legível.


## Propriedades de sombra e contorno do texto

O texto não tem sombra por padrão, mas você pode definir os seguintes valores para dar a ele um efeito de sombra.

* `shadowBlur`: *number*
* `shadowOffsetX`: *number*
* `shadowOffsetY`: *number*
* `shadowColor`: *Color3* objeto. *Color3* objetos armazenam uma *cor RGB* cor como três números de 0 a 1.

```ts
TextShape.create(sign, {
	text: 'Texto com sombra',
	shadowColor: { r: 1, g: 0, b: 0 },
	shadowOffsetY: 1,
	shadowOffsetX: -1,
})
```

As letras no texto também podem ter um contorno de outra cor ao redor do seu perímetro.

* `outlineWidth`: *number*. Quão largo será o contorno do texto, em todas as direções, como um número de 0 a 1. Por padrão *0*, o que o torna invisível.
* `outlineColor`: *Color3* objeto. *Color3* objetos armazenam uma *cor RGB* cor como três números de 0 a 1.

## Múltiplas linhas

Se você quiser que seu texto se espalhe por várias linhas, use `\n` como parte da string. O exemplo a seguir tem duas linhas de texto separadas:

```ts
TextShape.create(sign, {
	text: 'Esta é uma linha. \n Esta é outra linha',
})
```

Você também pode configurar as seguintes propriedades relacionadas a textos com várias linhas:

* `lineCount`: *number*. Quantas linhas de texto cabem na textbox como máximo. Se não for definido, não há limite para o número de linhas. A propriedade `textWrapping` deve estar *true* ativada para usar mais de uma linha.
* `lineSpacing`: *number*. Quanto espaço entre cada linha.

## Para que lado o texto fica voltado

A `TextShape` é lido corretamente a partir do seu lado local **-Z** . Se você criar um e não o girar, os jogadores que estiverem no lado -Z da entity poderão lê-lo.

Então, para rotular a face **+Z** de um modelo, torne o texto filho do modelo, desloque-o levemente em **+Z** para que não afunde na mesh e gire-o 180 graus em torno de Y:

```ts
// Um label preso à face +Z de um modelo
import { engine, TextShape, Transform } from '@dcl/sdk/ecs'
import { Quaternion, Vector3 } from '@dcl/sdk/math'

const label = engine.addEntity()

Transform.create(label, {
	parent: myModel,
	// Um pequeno deslocamento em +Z mantém o texto fora da superfície do modelo
	position: Vector3.create(0, 1, 0.01),
	rotation: Quaternion.fromEulerDegrees(0, 180, 0),
})

TextShape.create(label, { text: 'Frente' })
```


> [!NOTE]
> **💡 Dica**: Se o texto precisar permanecer legível de onde quer que o jogador esteja, adicione um [Billboard](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/entity-positioning/#face-the-player) em vez de rotacioná-lo manualmente.


## Organizar texto em colunas

Espaços e tabs não alinham colunas nas fontes padrão, porque seus caracteres têm larguras diferentes. Preencher um placar com espaços dá colunas irregulares que mudam assim que o comprimento de um nome ou de um número muda.

Para montar uma tabela, use um `TextShape` por coluna. Dê a cada coluna sua própria entity, posicione-a e coloque as linhas em seu `campo text` campo separadas por `\n`. Cada linha fica então na mesma altura em todas as colunas.

```ts
// Um placar de duas colunas: um TextShape para nomes, um para pontuações
const board = engine.addEntity()
Transform.create(board, { position: Vector3.create(8, 2, 8) })

const names = engine.addEntity()
Transform.create(names, { parent: board, position: Vector3.create(-1, 0, 0) })
TextShape.create(names, {
	text: 'Ana\nBruno\nChiara',
	textAlign: TextAlignMode.TAM_MIDDLE_LEFT,
	fontSize: 2,
})

const scores = engine.addEntity()
Transform.create(scores, { parent: board, position: Vector3.create(1, 0, 0) })
TextShape.create(scores, {
	text: '120\n85\n9',
	textAlign: TextAlignMode.TAM_MIDDLE_RIGHT,
	fontSize: 2,
})
```

Alinhe cada coluna em direção ao espaço entre as colunas: `TAM_MIDDLE_LEFT` para a coluna da esquerda, `TAM_MIDDLE_RIGHT` para a da direita. Números ficam melhores alinhados à direita, então seus dígitos ficam alinhados.

Para atualizar uma linha, reescreva a string inteira `campo text` de cada coluna. Mantenha o mesmo número de linhas em cada coluna, caso contrário as linhas deixam de corresponder.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/text.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
