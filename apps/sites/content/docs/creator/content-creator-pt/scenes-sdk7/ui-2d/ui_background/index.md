> Defina um fundo e uma borda de uma UI entity.

# Fundo da UI

As propriedades a seguir são usadas para definir um background e uma border em uma UI entity.

## Fundo

Um `uiBackground` O component fornece cor ou uma texture à área de uma entity. Ele usa o tamanho e a posição definidos pela entity. `uiTransform`.

Os seguintes campos podem ser configurados, e todos são opcionais:

* `color`: A cor a ser usada na entity, como um [Color4](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/color-types/) valor.


> [!NOTE]
> **💡 Dica**: Torne uma entity semitransparente definindo o 4º valor da `Color4` para menos de 1.


* `texture`: A texture a ser exibida na entity; isso recebe um objeto com vários parâmetros sobre a texture. As mesmas propriedades estão disponíveis nas textures em [materials em 3D entities](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/materials/#using-textures).

  * `src`: O caminho para o arquivo de imagem a ser usado como texture. (string)

  * `filterMode`: *(opcional)* Determina como os pixels na texture são esticados ou comprimidos quando renderizados. Veja [Escalonamento da texture](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/materials/#texture-scaling). (FilterMode = 'point' | 'bi-linear' | 'tri-linear')

  * `wrapMode`: *(opcional)* Determina como uma texture é aplicada em ladrilho a uma entity. Isso recebe um valor do `TextureWrapMode` enum. Veja \[Texture Wrapping]\(([Veja a documentação](https://github.com/decentraland/docs/tree/main/creator/sdk7/README.md)). (WrapMode = 'repeat' | 'clamp' | 'mirror')

  > Dica: Você pode combinar ambas as `texture` e `color` propriedades em um único `uiBackground` component para produzir uma texture com tonalidade.
* `textureMode`: Seleciona como você quer que a texture se adapte ao tamanho da entity à qual ela é aplicada. (TextureMode = 'nine-slices' | 'center' | 'stretch') enum, que suporta os seguintes valores:
  * `center`: A texture não é esticada; ela é posicionada centralizada na entity e partes dela podem ser cortadas dependendo do tamanho da entity.
  * `stretch`: A texture é esticada para corresponder a toda a superfície da entity.
  * `nine-slices`: Partes da texture são esticadas para corresponder a toda a superfície da entity, deixando as margens sem esticar. Veja [texturas nine-slice](#nine-slice-textures).
* `avatarTexture`: Exiba uma miniatura do perfil de um avatar, com base em um avatar ID. Veja \[Avatar Portraits]\(([Veja a documentação](https://github.com/decentraland/docs/tree/main/creator/sdk7/README.md)).
* `textureSlices`: Determine as margens a serem usadas ao utilizar o modo de texture nine-slice, veja [texturas nine-slice](#nine-slice-textures). Defina um número menor que 1, como uma fração da largura ou altura total da imagem.

Cor simples:

***arquivo ui.tsx:***

```tsx
import { ReactEcs, UiEntity } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
  <UiEntity
    uiTransform={{
      width: 700,
      height: 400
    }}
    uiBackground={{
		color: Color4.create(0.5, 0.8, 0.1, 0.6)
	}}
  />
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
> **📔 Nota**: Todos os trechos a seguir nesta página presumem que você tenha um `.ts` semelhante ao acima, executando a função `ReactEcsRenderer.setUiRenderer()` .


Padrão de texture repetida:

```ts
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'

export const uiMenu = () => (
  <UiEntity
    uiTransform={{
      width: 700,
      height: 400
    }}
    uiBackground={{
		textureMode: 'center',
		texture: {
			src: "images/brick-wall-texture.png",
			wrapMode: 'repeat'
		}
	}}
  />
)
```

## Bordas

Algumas propriedades são usadas para definir uma borda ao redor de uma UI entity. Essas propriedades existem no `uiTransform` component. Cada uma permite que você defina um único valor para todos os lados da borda, ou valores diferentes para cada lado.

* `borderColor`: A cor a ser usada na entity, como um [Color4](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/color-types/) valor.
* `borderWidth`: A largura da borda, como um número em pixels. Também suporta valores em porcentagens, por exemplo `borderWidth: '2%'` definirá a largura da borda para 2% da largura da entity.
* `borderRadius`: Use esta propriedade para dar aos cantos da entity uma borda arredondada. Ela define o raio dos cantos em pixels.

```ts
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
  <UiEntity
    uiTransform={{
      width: 700,
      height: 400,
      borderColor: Color4.Red(),
      borderWidth: 4,
      borderRadius: 10
    }}
  />
)
```

`borderWidth`, `borderColor` e `borderRadius` também pode ser definido com valores diferentes para cada lado da entity.

```ts
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
  <UiEntity
    uiTransform={{
      width: 700,
      height: 400,
      borderColor: { top: Color4.White(), left: Color4.Red(), right: Color4.Blue(), bottom: Color4.Gray() },
      borderRadius: { topLeft: 20, topRight: 20, bottomLeft: 20, bottomRight:0 },
      borderWidth: { top: 3, left: 2, right: 3, bottom: 4 }
    }}
  />
)
```

## Opacidade

Use o seletor `opacity` propriedade no `uiTransform` de um `UiEntity` para adicionar transparência à entity e a todos os seus children. A propriedade opacity é um valor de 0 a 1, em que 0 é totalmente transparente e 1 totalmente opaco.

```ts
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
  <UiEntity
    uiTransform={{
      width: 700,
      height: 400,
      opacity: 0.7
    }}
    uiBackground={{ color: Color4.Green() }}
  >
    <UiEntity
        uiTransform={{
          width: 100,
          height: 30,
        }}
        uiText={{
          value: "Este texto também é transparente",
          fontSize: 40
        }}
      />
   </UiEntity>
)
```

O valor de opacity afeta todos os children de uma UiEntity, aplicando transparência a cores de background, cores de texto e imagens de background. Quando tanto o parent quanto um child têm valores de opacity, a opacity final do child é o produto do seu próprio valor pelo do parent.

```ts
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
  <UiEntity
    uiTransform={{
      width: 700,
      height: 400,
      opacity: 0.7
    }}
    uiBackground={{ color: Color4.Green() }}
  >
    <UiEntity
      uiTransform={{
        width: 100,
        height: 30,
        opacity: 0.7
      }}
      uiText={{
        value: "Este texto é ainda mais transparente",
        fontSize: 40
      }}
    />
  </UiEntity>
)
```

## Texturas nine-slice

Podes usar [Escala 9-slice](https://en.wikipedia.org/wiki/9-slice_scaling) com suas textures, para garantir que cantos e margens não sejam esticados de forma desigual.

Com esta técnica popular, você divide uma imagem em 9 segmentos, que serão esticados de maneiras diferentes para preservar as proporções das margens e dos cantos. Por exemplo, use isso para definir fundos com cantos arredondados que se adaptam facilmente a qualquer tamanho. Considere a seguinte imagem (emprestada de [Wikipedia](https://en.wikipedia.org/wiki/9-slice_scaling#/media/File:Traditional_scaling_vs_9-slice_scaling.svg)):

![](/docs/_remote/e03c4d135bed8010ce6253a3.webp)

Nesta imagem vemos a texture original (canto superior esquerdo) e o resultado de escalá-la de forma tradicional (canto superior direito); observe como os cantos ficam deformados. Abaixo disso, vemos a texture segmentada em 9 slices (canto inferior esquerdo) e, em seguida, o resultado de esticar a imagem de acordo com o método 9-slice (canto inferior direito).

Veja como cada segmento é afetado, usando a imagem acima como referência.

* O segmento 5 é a única parte da imagem que é totalmente esticada em ambos os eixos x e y.
* Os segmentos 1, 3, 7 e 9 (os cantos) não são esticados de forma alguma.
* Os segmentos 2 e 8 são esticados apenas horizontalmente
* Os segmentos 4 e 6 são esticados apenas verticalmente.

Para usar o esticamento nine-slice em uma entity, defina o `textureMode` como `'nine-slices'`. Você também pode definir opcionalmente uma largura para a margem em cada lado em `textureSlices`.

```ts
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'

export const uiMenu = () => (
  <UiEntity
    uiTransform={{ width: 700, height: 400 }}
    uiBackground={{
      textureMode: 'nine-slices',
      texture: {
        src: 'images/rounded_alpha_square.png'
      },
      textureSlices: {
        top: 0.2,
        bottom: 0.2,
        left: 0.2,
        right: 0.2
      }
	}}
  />
)
```

## UVs da texture

Use o seletor `uvs` propriedade em um `uiBackground` component para exibir uma região específica de uma texture. Isso é útil para escolher sprites individuais de uma sprite sheet, ou para rotacionar uma imagem.

O `uvs` O campo recebe um array de 8 números, representando 4 pares de coordenadas UV para os quatro cantos da região da texture. A ordem é: **canto inferior esquerdo**, **canto superior esquerdo**, **canto superior direito**, **canto inferior direito**. Cada valor varia de 0 a 1, em que `(0, 0)` é o canto inferior esquerdo da imagem e `(1, 1)` é o canto superior direito.


> [!NOTE]
> **💡 Dica**: Ao usar custom `uvs`, defina `textureMode` como `'stretch'` para que a região selecionada preencha a área da entity.


### Sprites de uma sprite sheet

Para exibir uma parte de uma imagem maior (por exemplo, uma carta de uma sprite sheet que contém várias), defina o `uvs` para as coordenadas dessa região.

```ts
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'

// Exibir a metade esquerda de uma texture (por exemplo, a primeira carta em uma sheet de 2 colunas)
export const uiMenu = () => (
  <UiEntity
    uiTransform={{ width: 200, height: 300 }}
    uiBackground={{
      textureMode: 'stretch',
      texture: { src: 'images/card-atlas.png' },
      uvs: [
        // canto inferior esquerdo, canto superior esquerdo, canto superior direito, canto inferior direito
        0, 0,
        0, 1,
        0.5, 1,
        0.5, 0
      ]
    }}
  />
)
```

Para uma sprite sheet com uma grade de frames (por exemplo, uma sheet de 4 colunas por 2 linhas), calcule os UVs com base na coluna e na linha do frame que você deseja:

```ts
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'

// Escolher um único frame de uma sprite sheet em grade
function getFrameUVs(col: number, row: number, totalCols: number, totalRows: number): number[] {
  const stepU = 1 / totalCols
  const stepV = 1 / totalRows
  const left = col * stepU
  const right = (col + 1) * stepU
  const top = 1 - row * stepV
  const bottom = 1 - (row + 1) * stepV
  return [
    left, bottom,
    left, top,
    right, top,
    right, bottom
  ]
}

// Exibir a coluna 2, linha 0 de uma sprite sheet 4x2
export const uiMenu = () => (
  <UiEntity
    uiTransform={{ width: 128, height: 128 }}
    uiBackground={{
      textureMode: 'stretch',
      texture: { src: 'images/spritesheet.png' },
      uvs: getFrameUVs(2, 0, 4, 2)
    }}
  />
)
```

### Rotacionando uma imagem com UVs

Você pode rotacionar uma texture aplicando uma rotação 2D às coordenadas UV. Isso é útil para spinners ou indicadores de carregamento, sem precisar de rotação baseada em transform.

```ts
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { engine } from '@dcl/sdk/ecs'

// Rotacionar um ponto 2D ao redor de um centro
function rotate2D(angle: number, x: number, y: number, cx: number, cy: number): number[] {
  const cos = Math.cos(angle)
  const sin = Math.sin(angle)
  return [
    cos * (x - cx) - sin * (y - cy) + cx,
    sin * (x - cx) + cos * (y - cy) + cy
  ]
}

// Construir coordenadas UV rotacionadas
function rotateUVs(angle: number): number[] {
  const uv00 = rotate2D(angle, 0, 0, 0.5, 0.5)
  const uv01 = rotate2D(angle, 0, 1, 0.5, 0.5)
  const uv11 = rotate2D(angle, 1, 1, 0.5, 0.5)
  const uv10 = rotate2D(angle, 1, 0, 0.5, 0.5)
  return [uv00[0], uv00[1], uv01[0], uv01[1], uv11[0], uv11[1], uv10[0], uv10[1]]
}

let spinnerAngle = 0

// System que atualiza o ângulo a cada frame
engine.addSystem((dt: number) => {
  spinnerAngle += dt * 5
})

export const uiMenu = () => (
  <UiEntity
    uiTransform={{ width: 128, height: 128 }}
    uiBackground={{
      textureMode: 'stretch',
      texture: { src: 'images/spinner.png' },
      uvs: rotateUVs(spinnerAngle)
    }}
  />
)
```

O `rotateUVs` função que rotaciona os quatro cantos UV ao redor do ponto central `(0.5, 0.5)` pelo ângulo fornecido em radianos. Como isso é chamado a cada frame pelo renderizador semelhante ao React, o spinner é atualizado suavemente.


> [!NOTE]
> **💡 Dica**: Veja um exemplo completo e funcional com sprites, sprite sheets animadas, spinners e mais no [Animações de UI](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/ui-animations) cena de exemplo.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/ui-2d/ui_background.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
