> Escribe texto en entidades de UI.

# Texto de UI

Agrega texto a tu UI creando un `Label` entity.

A `Label` entity tiene los siguientes campos que se pueden configurar:

* `valor`: La cadena a mostrar
* `fontSize`: El tamaño del texto, como un número.

  > NOTA: El `fontSize` no se ve afectado por el tamaño de su entity o parent entities.
* `color`: El color del texto, como un [Color4](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/color-types/).
* `font`: La fuente a usar. Los valores compatibles son:
  * `'serif'`
  * `'sans-serif'` *(predeterminado)*
  * `'monospace'`
* `fontSrc`: Ruta a un `.ttf` archivo de fuente en tu escena, para renderizar el texto con una fuente personalizada. `font` se usa como alternativa. Consulta [Fuentes personalizadas](#custom-fonts).
* `textAlign`: Cómo se alineará el texto con su parent. Toma un valor del `TextAlignType` type. TextAlignType = 'top-left' | 'top-center' | 'top-right' | 'middle-left' | 'middle-center' | 'middle-right' | 'bottom-left' | 'bottom-center' | 'bottom-right';
* `textWrap`: Define si el texto usa saltos de línea para evitar superar el ancho máximo. Está activado por defecto (`'wrap'`), para desactivarlo pasa el valor `'nowrap'`.


> [!WARNING]
> **📔 Nota**: El `fontSize` no se ve afectado por el tamaño de su entity o parent entities.


A `Label` La entity también puede tener otros components comunes que se encuentran en otros tipos de UI entities, como `uiTransform` y `uiBackground`.

***archivo ui.tsx:***

```ts
import { UiEntity, Label, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
	<UiEntity uiTransform={{ width: 'auto', height: 'auto' }}>
		<Label
			value="Esta es una etiqueta"
			color={Color4.Red()}
			fontSize={29}
			font="sans-serif"
			textAlign="top-left"
		/>
	</UiEntity>
)
```

***archivo index.ts:***

```ts
import { ReactEcsRenderer } from '@dcl/sdk/react-ecs'
import { uiMenu } from './ui'

export function main() {
    ReactEcsRenderer.setUiRenderer(uiMenu, { virtualWidth: 1920, virtualHeight: 1080 })
}
```


> [!WARNING]
> **📔 Nota**: Todos los siguientes fragmentos de esta página asumen que tienes un `.ts` similar al anterior, ejecutando el `ReactEcsRenderer.setUiRenderer()` función.


Si una línea de texto es demasiado larga para caber en el ancho asignado, o en el ancho máximo de su contenedor, el texto continuará en la siguiente línea. Puedes desactivar esto cambiando el valor de `textWrap` propiedad en `'nowrap'`.

También puedes forzar un salto de línea añadiendo explícitamente `\n` a la cadena.

```ts
import { UiEntity, Label, ReactEcs } from '@dcl/sdk/react-ecs'

export const uiMenu = () => (
	<UiEntity uiTransform={{ width: 700, height: 400 }}>
		<Label 
			value="¡Hola, mundo!\nEste otro fragmento es bastante largo. Probablemente no quepa en una sola línea, así que incluirá un salto de línea en algún punto.\nCuarta línea"
			textWrap= {`nowrap`} 
		/>
	</UiEntity>
)
```

Si no se define explícitamente `height` ni a `width` en el `uiTransform` del contenedor, el contenedor usará el valor `auto`, que se ajusta para encajar todo el texto. Puedes definir un `maxWidth` y un `maxHeight` para asegurarte de que no exceda ciertos límites. También puedes usar `minWidth` y `minHeight` para asegurarte de que el contenedor no se haga demasiado pequeño, incluso si el texto es más corto.


> [!WARNING]
> **📔 Nota**: No dependas de ese auto-fit para el texto. Cuánto espacio ocupa un `Label` cuando dejas su tamaño sin definir difiere entre explorers. El explorer basado en Bevy mide el texto renderizado y lo distribuye en consecuencia. El explorer de Unity le da casi ninguna altura, pero aun así dibuja las letras, así que las labels apiladas quedan una encima de otra y un parent dimensionado a partir de su texto se colapsa a nada.
> 
> Dale a cada `Label` un `width` **y** `height` explícito en su `uiTransform`, y dale una altura explícita a cualquier contenedor que apile labels. Para texto con salto de línea, dimensiona la altura según el número de líneas: dos líneas con `fontSize: 20` necesitan aproximadamente `height: 60`.
> 
> Como esto difiere según el explorer, una preview que se ve bien en un cliente no demuestra que el layout sea correcto en otro.


## Fuentes personalizadas

Para renderizar un `Label` con tu propia fuente, agrega una TrueType (`.ttf`) a la carpeta de tu escena y establece la `fontSrc` propiedad a su ruta, relativa a la carpeta raíz de la scene. Mantén `font` también configurado: es la alternativa usada siempre que no se pueda cargar la fuente personalizada.


> [!WARNING]
> **📔 Nota**: `fontSrc` necesita `@dcl/sdk` 7.31.0 o posterior. En versiones anteriores la propiedad no existe.


```tsx
<Label
	value="¡Bienvenido!"
	fontSize={40}
	uiTransform={{ width: 300, height: 60 }}
	fontSrc="assets/fonts/Lobster-Regular.ttf"
	font="sans-serif"
/>
```

El `Button` entity, la `uiText` de un `UiEntity` y en el [`Input` y `Dropdown`](/docs/creator/content-creator-es/scenes-sdk7/ui-2d/ui_special_types/) entities aceptan `fontSrc` también, y también lo hace el [`TextShape`](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/text/#custom-fonts) component para texto en el espacio 3D.

Ten en cuenta lo siguiente al usar fuentes personalizadas:

* **Solo `.ttf` se admiten archivos**. Las fuentes OpenType con contornos CFF (normalmente `.otf`), las font collections y las web fonts se rechazan, y el texto se renderiza con la fuente de respaldo `font`.
* **La fuente debe ser un archivo en tu escena**. `fontSrc` no puede apuntar a una URL externa, y los nombres de familias tipográficas (por ejemplo, de Google Fonts) tampoco son compatibles. Descarga el `.ttf` archivo e inclúyelo en la carpeta de tu scene. Asegúrate de que su licencia permita redistribuirlo.
* **En el Desktop Client, las fuentes personalizadas solo aparecen una vez que los assets de la escena están optimizados**. Cuando publicas una escena, sus fuentes se convierten en paquetes de assets optimizados junto con sus modelos 3D, y la fuente personalizada aparece una vez que se completa esa conversión. Hasta entonces, el texto usa la alternativa `font`. Lo mismo aplica a las previews locales: la fuente personalizada solo se muestra al previsualizar con assets optimizados (el **Compress Assets** opción en el Creator Hub, o `npm run start -- --asset-bundles` desde la CLI). Consulta [Vista previa con assets optimizados](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/preview-scene/#preview-with-optimized-assets). El cliente web de Bevy carga el `.ttf` archivo directamente, así que allí la fuente personalizada se muestra de inmediato, incluso en las vistas previas locales.
* **No compatible en móviles**. La aplicación móvil de Decentraland ignora `fontSrc` y renderiza la alternativa `font`. Elige una alternativa que siga viéndose bien y no dependas de la fuente personalizada para que el texto encaje en un espacio fijo. Consulta [Funciones faltantes en móviles](/docs/creator/content-creator-es/desarrollar-para-movil/cliente-movil/missing-features/).
* **Un archivo es un solo estilo**. Para texto en negrita y regular, agrega ambos `.ttf` archivos y apunta cada texto a su propio archivo.
* **El UI Designer de Creator Hub no muestra ni edita `fontSrc`**. Agrégalo al elemento en tu código manualmente. El UI Designer solo cambia las propiedades que editas en él, así que un `fontSrc` que escribiste permanece en su lugar. Su canvas no muestra la fuente personalizada.
* **Los caracteres que la fuente no incluye** se dibujan con la fuente sans-serif incorporada. Los caracteres no latinos que la fuente sí incluye se renderizan la primera vez que aparecen, lo que puede causar una pequeña pausa.

Para ver la lista completa de límites, como el tamaño máximo de archivo, consulta [Fuentes personalizadas](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/text/#custom-fonts) en la página Text.

## No uses emoji en el texto de la UI

No incluyas emoji en ningún `Label` ni a `Button` `valor`, `uiText.value`, `Input` `placeholder`, y `Dropdown` opción.

El SDK no incluye glifos de emoji. Que un emoji aparezca depende de las fuentes que cada explorer incluye, y el explorer de Unity no tiene ninguno, así que el carácter sale como un cuadro vacío o desaparece por completo. La misma advertencia aplica a otros Unicode decorativos como flechas y caracteres de dibujo de cajas.

Para una etiqueta pictórica, incluye el arte en su lugar: coloca una imagen sobre un pequeño `UiEntity` junto al texto, usando un `uiBackground` con una `Texture`. Consulta [Fondos de la UI](/docs/creator/content-creator-es/scenes-sdk7/ui-2d/ui_background/).

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
			value: '¡Hola, mundo!',
			fontSize: 18,
		}}
	/>
)
```

## Tamaño de texto adaptable

Usa la `scaleFontSize()` función para proporcionar valores de fuente que se ajusten al tamaño de pantalla del jugador. Al configurar la `fontSize` propiedad de una UI entity de texto, pasa esta función en lugar de un solo número.

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
      value: '¡Hola, mundo!',
      fontSize: scaleFontSize(15)
    }}
  />
)
```

El `scaleFontSize()` La función requiere dos parámetros:

* `fontSize`: El tamaño base de fuente a usar.
* `scaleUnit` *(opcional)*: El factor de escala. Esto determina si el texto debe ajustarse según el ancho o la altura de la pantalla, y un multiplicador de cuánto adaptarlo. Predeterminado: `0.39` (un número, por lo que se interpreta como relativo a *width*). Los valores pueden ser:
  * *Número*: Un número simple; en este caso se interpreta como relativo a *width*
  * *Cadena que termina en **vw***: Esto hace que el número sea relativo al ancho de la vista. Por ejemplo `"0.8vw"`
  * *Cadena que termina en **vh***: Esto hace que el número sea relativo a la altura de la vista. Por ejemplo `"0.8vh"`


> [!NOTE]
> **💡 Consejo**: Esta función funciona de forma similar a la CSS `calc()` función.


El valor de `scaleUnit` es un porcentaje del ancho o alto de la ventana. Así que un `scaleUnit` de `"100vw"` es el 100% del ancho de la pantalla, un valor de `"0.5vw"` es el 0.5% del ancho de la pantalla.

La fórmula que `scaleFontSize()` sigue multiplica el ancho o alto de la pantalla por el `scaleUnit`, y le suma el `fontSize` pasado en el primer parámetro.

```ts
final font = fontSize + (screen width * scaleUnit / 100)
```

Por ejemplo, en el fragmento de abajo se usa un `scaleUnit` valor de 0.8. Si el ancho de la pantalla es *1280px*, eso dará como resultado un texto de tamaño de **25.24**, siguiendo la ecuación `15 + (1280 * 0.8 / 100)`.


> [!WARNING]
> **📔 Nota**: `scaleFontSize()` devuelve un **number**, por lo que su resultado es un tamaño en *virtuales* píxeles — todavía se multiplica por el [factor de escala de la UI](/docs/creator/content-creator-es/scenes-sdk7/ui-2d/onscreen-ui/#screen-virtual-scale) antes de ser dibujado. En el ejemplo anterior, con la `1920x1080` pantalla virtual predeterminada y un canvas de 1280px de ancho, el factor de escala es `1280 / 1920 = 0.667`, así que el texto se renderiza a unos 16.8px. Si quieres un tamaño de fuente medido directamente contra el canvas, que ignore la pantalla virtual, pasa un `vw`/`vh` **string** directamente a `fontSize` en su lugar — por ejemplo `fontSize: '1.8vh'`.


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
      value: '¡Hola, mundo!',
      fontSize: scaleFontSize(15, 0.8)
    }}
  />
)
```


> [!NOTE]
> **💡 Consejo**: Si no tienes diferentes tamaños de pantalla para probar, puedes intentar usar el Web Explorer y redimensionar la ventana donde ejecutas la preview. El texto se ajustará al instante cada vez que cambies la ventana.


Como alternativa a `scaleFontSize()`, pasa una `vw`/`vh` cadena directamente como `fontSize` — por ejemplo `fontSize: '1.8vh'` — que dimensiona el texto con respecto al canvas e ignora por completo el factor de escala de la UI. No **no** multipliques los tamaños de fuente por un factor que calcules tú mismo a partir de `UiCanvasInformation`: el SDK ya aplica uno; consulta [Tamaño adaptable de la UI](/docs/creator/content-creator-es/scenes-sdk7/ui-2d/ui-positioning/#responsive-ui-size).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/ui-2d/ui_text.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
