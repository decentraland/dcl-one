> Establece un fondo y un borde de una entidad de UI.

# Fondo de UI

Las siguientes propiedades se usan para establecer un fondo y un borde en una entidad de UI.

## Fondo

A `uiBackground` El componente da color o una textura al área de una entidad. Usa el tamaño y la posición definidos por la entidad. `uiTransform`.

Se pueden configurar los siguientes campos, todos son opcionales:

* `color`: El color que se usará en la entidad, como un [Color4](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/color-types/) valor.


> [!NOTE]
> **💡 Consejo**: Haz que una entidad sea semitransparente estableciendo el cuarto valor de `Color4` a menos de 1.


* `textura`: La textura que se mostrará en la entidad; esto toma un objeto con varios parámetros sobre la textura. Las mismas propiedades están disponibles que en las texturas en [materiales en entidades 3D](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/materials/#using-textures).

  * `src`: La ruta al archivo de imagen que se usará como textura. (string)

  * `filterMode`: *(opcional)* Determina cómo se estiran o comprimen los píxeles de la textura al renderizarse. . Consulta [Escalado de textura](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/materials/#texture-scaling). (FilterMode = 'point' | 'bi-linear' | 'tri-linear')

  * `wrapMode`: *(opcional)* Determina cómo se repite una textura en una entidad. Toma un valor de la `TextureWrapMode` enum. Consulta \[Repetición de textura]\(([Consulta la documentación](https://github.com/decentraland/docs/tree/main/creator/sdk7/README.md)). (WrapMode = 'repeat' | 'clamp' | 'mirror')

  > Consejo: puedes combinar ambas `textura` y `color` propiedades en un solo `uiBackground` componente para producir una textura tintada.
* `textureMode`: Selecciona cómo quieres que la textura se adapte al tamaño de la entidad a la que se aplica. (TextureMode = 'nine-slices' | 'center' | 'stretch') enum, que admite los siguientes valores:
  * `center`: La textura no se estira; se posiciona centrada en la entidad y partes de ella pueden recortarse según el tamaño de la entidad.
  * `stretch`: La textura se estira para cubrir toda la superficie de la entidad.
  * `nine-slices`: Partes de la textura se estiran para cubrir toda la superficie de la entidad, dejando los márgenes sin estirar. Consulta [texturas de 9 cortes](#nine-slice-textures).
* `avatarTexture`: Muestra una miniatura del perfil de un avatar, basada en un ID de avatar. Consulta \[Avatar Portraits]\(([Consulta la documentación](https://github.com/decentraland/docs/tree/main/creator/sdk7/README.md)).
* `textureSlices`: Determina los márgenes que se usarán al utilizar el modo de textura de 9 cortes, consulta [texturas de 9 cortes](#nine-slice-textures). Establece un número menor que 1, como fracción del ancho o alto total de la imagen.

Color simple:

***Archivo ui.tsx:***

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

***Archivo index.ts:***

```ts
import { ReactEcsRenderer } from '@dcl/sdk/react-ecs'
import { uiMenu } from './ui'

export function main() {
    ReactEcsRenderer.setUiRenderer(uiMenu, { virtualWidth: 1920, virtualHeight: 1080 })
}
```


> [!WARNING]
> **📔 Nota**: Todos los siguientes fragmentos de esta página asumen que tienes un `.ts` similar al anterior, ejecutando el `ReactEcsRenderer.setUiRenderer()` función.


Patrón de textura repetido:

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

## Bordes

Se usan algunas propiedades para establecer un borde alrededor de una entidad de UI. Estas propiedades existen en el `uiTransform` componente. Cada una te permite establecer un único valor para todos los lados del borde, o valores diferentes para cada lado.

* `borderColor`: El color que se usará en la entidad, como un [Color4](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/color-types/) valor.
* `borderWidth`: El ancho del borde, como un número en píxeles. También admite valores en porcentajes; por ejemplo `borderWidth: '2%'` establecerá el ancho del borde en el 2 % del ancho de la entidad.
* `borderRadius`: Usa esta propiedad para dar a las esquinas de la entidad un borde redondeado. Establece el radio de las esquinas en píxeles.

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

`borderWidth`, `borderColor` y `borderRadius` también se puede establecer con valores diferentes para cada lado de la entidad.

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

## Opacidad

Usa la `opacity` propiedad en el `uiTransform` de un `UiEntity` para añadir transparencia a la entidad y a todos sus hijos. La propiedad opacity es un valor de 0 a 1, donde 0 es completamente transparente y 1 completamente opaco.

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
          value: "Este texto también es transparente",
          fontSize: 40
        }}
      />
   </UiEntity>
)
```

El valor de opacity afecta a todos los hijos de un UiEntity, aplicando transparencia a los colores de fondo, colores de texto e imágenes de fondo. Cuando tanto el padre como un hijo tienen valores de opacity, la opacidad final del hijo es el producto de su propio valor y el del padre.

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
        value: "Este texto es aún más transparente",
        fontSize: 40
      }}
    />
  </UiEntity>
)
```

## Texturas de 9 cortes

Puedes usar [escalado de 9 cortes](https://en.wikipedia.org/wiki/9-slice_scaling) con tus texturas para garantizar que las esquinas y los márgenes no se estiren de forma desigual.

Con esta técnica popular, divides una imagen en 9 segmentos, que se estirarán de distintas maneras para preservar las proporciones de los márgenes y las esquinas. Por ejemplo, úsala para definir fondos con esquinas redondeadas que se adapten fácilmente a cualquier tamaño. Considera la siguiente imagen (tomada de [Wikipedia](https://en.wikipedia.org/wiki/9-slice_scaling#/media/File:Traditional_scaling_vs_9-slice_scaling.svg)):

![](/docs/_remote/e03c4d135bed8010ce6253a3.webp)

En esta imagen vemos la textura original (arriba a la izquierda) y el resultado de escalarla de forma tradicional (arriba a la derecha); observa cómo se deforman las esquinas. Debajo, vemos la textura segmentada en 9 cortes (abajo a la izquierda) y luego el resultado de estirar la imagen según el método de 9 cortes (abajo a la derecha).

Así es como se ve afectado cada segmento, usando la imagen anterior como referencia.

* El segmento 5 es la única parte de la imagen que se estira por completo tanto en el eje x como en el eje y.
* Los segmentos 1, 3, 7 y 9 (las esquinas) no se estiran en absoluto.
* Los segmentos 2 y 8 solo se estiran horizontalmente
* Los segmentos 4 y 6 solo se estiran verticalmente.

Para usar el estiramiento de 9 cortes en una entidad, establece `textureMode` en `'nine-slices'`. Opcionalmente, también puedes establecer un ancho para el margen en cada lado en `textureSlices`.

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

## UVs de textura

Usa la `uvs` propiedad en un `uiBackground` componente para mostrar una región específica de una textura. Esto es útil para seleccionar sprites individuales de una sprite sheet o para rotar una imagen.

El `uvs` El campo toma un array de 8 números, que representa 4 pares de coordenadas UV para las cuatro esquinas de la región de la textura. El orden es: **inferior izquierda**, **superior izquierda**, **superior derecha**, **inferior derecha**. Cada valor va de 0 a 1, donde `(0, 0)` es la esquina inferior izquierda de la imagen y `(1, 1)` es la esquina superior derecha.


> [!NOTE]
> **💡 Consejo**: Al usar UVs personalizados `uvs`, establece `textureMode` en `'stretch'` para que la región seleccionada llene el área de la entidad.


### Sprites de una sprite sheet

Para mostrar una parte de una imagen más grande (por ejemplo, una carta de una sprite sheet que contiene varias), establece la `uvs` a las coordenadas de esa región.

```ts
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'

// Mostrar la mitad izquierda de una textura (p. ej., la primera carta en una hoja de 2 columnas)
export const uiMenu = () => (
  <UiEntity
    uiTransform={{ width: 200, height: 300 }}
    uiBackground={{
      textureMode: 'stretch',
      texture: { src: 'images/card-atlas.png' },
      uvs: [
        // inferior izquierda, superior izquierda, superior derecha, inferior derecha
        0, 0,
        0, 1,
        0.5, 1,
        0.5, 0
      ]
    }}
  />
)
```

Para una sprite sheet con una cuadrícula de fotogramas (por ejemplo, una hoja de 4 columnas por 2 filas), calcula los UVs en función de la columna y la fila del fotograma que quieras:

```ts
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'

// Seleccionar un único fotograma de una sprite sheet en cuadrícula
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

// Mostrar la columna 2, fila 0 de una sprite sheet de 4x2
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

### Rotar una imagen con UVs

Puedes rotar una textura aplicando una rotación 2D a las coordenadas UV. Esto es útil para indicadores giratorios o de carga sin necesidad de una rotación basada en Transform.

```ts
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { engine } from '@dcl/sdk/ecs'

// Rotar un punto 2D alrededor de un centro
function rotate2D(angle: number, x: number, y: number, cx: number, cy: number): number[] {
  const cos = Math.cos(angle)
  const sin = Math.sin(angle)
  return [
    cos * (x - cx) - sin * (y - cy) + cx,
    sin * (x - cx) + cos * (y - cy) + cy
  ]
}

// Construir coordenadas UV rotadas
function rotateUVs(angle: number): number[] {
  const uv00 = rotate2D(angle, 0, 0, 0.5, 0.5)
  const uv01 = rotate2D(angle, 0, 1, 0.5, 0.5)
  const uv11 = rotate2D(angle, 1, 1, 0.5, 0.5)
  const uv10 = rotate2D(angle, 1, 0, 0.5, 0.5)
  return [uv00[0], uv00[1], uv01[0], uv01[1], uv11[0], uv11[1], uv10[0], uv10[1]]
}

let spinnerAngle = 0

// System que actualiza el ángulo en cada frame
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

El `rotateUVs` función que rota las cuatro esquinas UV alrededor del punto central `(0.5, 0.5)` por el ángulo dado en radianes. Como esto se llama en cada frame por el renderer similar a React, el spinner se actualiza suavemente.


> [!NOTE]
> **💡 Consejo**: Consulta un ejemplo completo y funcional con sprites, sprite sheets animadas, spinners y más en la [Animaciones de UI](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/ui-animations) escena de ejemplo.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/ui-2d/ui_background.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
