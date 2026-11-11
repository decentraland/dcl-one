> Tipos especiales de entidades para la UI, incluidos dropdowns y cajas de entrada.

# Tipos especiales de UI

Hay ciertos tipos de entidades especiales que permiten algunos tipos especiales de interacciones.

## Dropdown

Crea una `Dropdown` entidad para permitir que los usuarios desplieguen y seleccionen un elemento de una lista.

A `Dropdown` la entidad debe tener al menos las siguientes propiedades:

* `options`: Qué valores mostrar cuando el Dropdown se expande. Proporciona un array con un valor de cadena para cada opción. El primer valor del array se muestra como la opción predeterminada.
* `onChange`: Una función que se ejecuta cada vez que se selecciona un valor en el Dropdown, usando

También puedes configurar otros componentes de la `Dropdown` entidad, como un `uiTransform`, como en otras entidades UI. El texto del Dropdown acepta las mismas propiedades de estilo que un `Label`, como `color`, `fontSize`, `font` y `fontSrc` para una [fuente personalizada](/docs/creator/content-creator-es/scenes-sdk7/ui-2d/ui_text/#custom-fonts).

***Archivo ui.tsx:***

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
        value="Selecciona un color"
        fontSize={18}
        color={textColor}
        uiTransform={{
          width: '140px',
          height: '40px',
        }}
      />
      <Dropdown
        options={[`Rojo`, `Azul`, `Verde`]}
        onChange={selectOption}
        uiTransform={{
          width: '100px',
          height: '40px',
        }}
      />
    </UiEntity>
)
```

***Archivo index.ts:***

```ts
import { ReactEcsRenderer } from '@dcl/sdk/react-ecs'
import { uiMenu } from './ui'

export function main() {
    ReactEcsRenderer.setUiRenderer(uiMenu)
}
```


> [!WARNING]
> **📔 Nota**: Todos los siguientes fragmentos de esta página asumen que tienes un `.ts` similar al anterior, ejecutando el `ReactEcsRenderer.setUiRenderer()` función.


## Texto de entrada

Crea una `Input` entidad para permitir que los usuarios escriban texto. Los jugadores primero deben hacer clic en este cuadro antes de poder escribir en él.

El comportamiento de la `Input` entidad se gestiona mediante las siguientes propiedades:

* `onSubmit`: Una función que se ejecuta cuando el jugador pulsa Return/Enter, usando el texto proporcionado como entrada. El campo de texto se **no** limpia automáticamente cuando esto ocurre, tienes que limpiarlo tú mismo (consulta el ejemplo de abajo).
* `onChange`: Una función que se ejecuta cada vez que cambia un valor en el texto de entrada. A medida que el jugador escribe en el cuadro, esta función se ejecuta una vez por cada carácter que se añade o se elimina.
* `disabled`: Si *true*, el jugador no podrá interactuar con la entidad de entrada.

El siguiente ejemplo usa `onSubmit` para registrar el texto proporcionado en la consola.

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
      placeholder={'escribe algo'}
      placeholderColor={Color4.Black()}
      uiTransform={{
        width: '400px',
        height: '80px',
      }}
    ></Input>
  </UiEntity>
)
```

Es una buena práctica proporcionar tanto un botón para enviar como para gestionar el `onSubmit` evento cuando el jugador pulsa la tecla Enter/Return. El siguiente ejemplo muestra cómo puedes hacerlo.


> [!WARNING]
> **📔 Nota**: Haz **no** vincula el `valor` propiedad de la `Input` directamente a lo que el jugador escribe mediante `onChange`. Esto crea un ida y vuelta entre la escena y el Explorer que hace que se pierdan caracteres al escribir rápido. En su lugar, mantén `valor` como una cadena vacía y solo cámbialo cuando quieras limpiar el campo de entrada.


Para limpiar el campo de entrada, establece `valor` en `' '` (un solo espacio). El Explorer lo trata como vacío y reactiva el texto del placeholder. Como la función de UI se ejecuta en cada frame, usa una bandera para aplicar la limpieza solo durante un frame.

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
        placeholder={'escribe algo'}
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

Las siguientes propiedades también están disponibles para personalizar el aspecto del campo de texto, la mayoría similares a las presentes en `Label` entidades:

* `placeholder`: Cadena que se muestra antes de que el jugador empiece a introducir algo. Es útil hacer de este texto una pista sobre lo que debería escribir.
* `placeholderColor`: El color que se usará para el texto del placeholder, como un [Color4](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/color-types/).


> [!NOTE]
> **💡 Consejo**: Usa una tonalidad más pálida del color del texto que escribe el jugador.


* `fontSize`: El tamaño del texto, como un número.

  > NOTA: El `fontSize` no se ve afectado por el tamaño de su entidad o de sus entidades padre.
* `color`: El color del texto que escribe el jugador, como un [Color4](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/color-types/).


> [!WARNING]
> **📔 Nota**: Asegúrate de usar un color diferente al `placeholderColor`.


* `font`: La fuente a usar. Los valores admitidos son: - `'sans-serif'` *(predeterminado)* - `'serif'` - `'monospace'`
* `fontSrc`: Ruta a un `.ttf` archivo de fuente en tu escena, para renderizar el texto con una fuente personalizada. `font` se usa como alternativa. Consulta [Fuentes personalizadas](/docs/creator/content-creator-es/scenes-sdk7/ui-2d/ui_text/#custom-fonts).
* `textAlign`: Cómo se alineará el texto con su padre. Toma un valor de \`TextAlignType\`. TextAlignType = 'top-left' | 'top-center' | 'top-right' | 'middle-left' | 'middle-center' | 'middle-right' | 'bottom-left' | 'bottom-center' | 'bottom-right';

También puedes configurar otros componentes de la `Input` entidad, como un `uiTransform`, `OnMouseDown` como en otras entidades UI.

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
          currentTextString = 'has escrito: ' + e
        }}
        fontSize={35}
        placeholder={'escribe algo'}
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
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/ui-2d/ui_special_types.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
