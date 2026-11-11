> Cómo añadir texto a tu escena

# Texto

Añade texto a una escena usando el `TextShape` componente. Este texto se sitúa en una posición

El texto en Decentraland admite todos los *utf8* caracteres; esto incluye caracteres orientales y especiales.


> [!WARNING]
> **📔 Nota**: Este componente es útil para etiquetas en el mundo y UI que existen en el espacio 3D de la escena, no para la UI HUD 2D del jugador.


Para añadir texto como una etiqueta a una Entity existente, una buena práctica es crear una segunda Entity que tenga el `TextShape` componente y establecerla como hija de la otra Entity.

## Usa el Scene Editor en Creator Hub

La forma más sencilla de colocar texto en el mundo es añadir un **Text** [Smart item](/docs/creator/content-creator-es/scene-editor/interactividad/smart-items/) visualmente en el Scene Editor. Luego puedes configurar todos los campos disponibles en la UI del Scene Editor.

![](/docs/_remote/339b81b8af14deb7d4666a01.webp)

## Crear un componente de texto

El siguiente ejemplo muestra cómo crear un `TextShape` componente y añadirlo a una Entity mediante código.

```ts
const sign = engine.addEntity()

Transform.create(sign, {
	position: Vector3.create(8, 1, 8),
})

TextShape.create(sign, {
	text: 'Hola Mundo',
})
```


> [!WARNING]
> **📔 Nota**: Si la Entity con el componente de texto es hija de otra Entity, entonces se verá afectada por la escala de la Entity padre. Si la Entity padre se escala de forma desigual a lo largo de su eje, esto hará que el texto también se estire o se comprima.



> [!WARNING]
> **📔 Nota**: `TextShape` los componentes no son clicables. `PointerEvents` los comopnents no se activan cuando se usan en entites que tienen un `TextShape` componente.



> [!WARNING]
> **📔 Nota**: `TextShape` debe importarse mediante
> 
> > `import { TextShape } from "@dcl/sdk/ecs"`
> 
> Consulta [Imports](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/coding-scenes/#imports) para ver cómo manejarlos fácilmente.


## Cambiar el valor del texto

Al crear un nuevo componente de texto, le asignas una cadena para mostrar. Esta cadena se almacena en el `text` campo.

Si quieres cambiar la cadena que muestra el componente, puedes hacerlo en cualquier momento cambiando el `text` campo en una [versión mutable](/docs/creator/content-creator-es/scenes-sdk7/patrones-de-programacion/mutable-data/) del componente.

```ts
const mutableText = TextShape.getMutable(myEntity)

mutableText.text = 'nueva cadena'
```

## Propiedades básicas del texto

El `TextShape` el componente tiene varias propiedades que se pueden configurar para dar estilo al texto. A continuación se muestran algunas de las más comunes:

* `font`: Valor del enum `Font`.
* `fontSrc`: *string*. Ruta a un `.ttf` archivo de fuente en tu escena, para renderizar el texto con una fuente personalizada. Consulta [Fuentes personalizadas](#custom-fonts).
* `fontSize`: *number*. Una Entity con font 10 mide 1 metro de alto.
* `textColor`: *Color4* object. *Color4* Los objects almacenan un *RBG* color como tres números de 0 a 1, más *alpha* para la transparencia. Consulta [tipos de color](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/color-types/) para obtener más detalles.

```ts
TextShape.create(sign, {
	text: 'Hola Mundo',
	textColor: { r: 1, g: 0, b: 0, a: 1 },
	fontSize: 5,
	font: Font.F_SANS_SERIF,
})
```

## Fuentes

Las TextShape pueden usar fuentes del enum `Font`. Este enum incluye actualmente las siguientes fuentes:

* `Font.F_SANS_SERIF`
* `Font.F_SERIF`
* `Font.F_MONOSPACE`

Por defecto usa `Font.F_SANS_SERIF`.

```ts
TextShape.create(sign, {
	text: 'Hola Mundo',
	textColor: { r: 1, g: 0, b: 0, a: 1 },
	fontSize: 5,
	font: Font.F_SANS_SERIF,
})
```


> [!NOTE]
> **💡 Consejo**: Si usas VS studio u otro IDE, escribe `Font.` y deberías ver una lista de sugerencias con todas las fuentes disponibles.


### Fuentes personalizadas

Para renderizar texto con tu propia fuente, añade un archivo de fuente TrueType (`.ttf`) a la carpeta de tu escena y establece la `fontSrc` propiedad con su ruta, relativa a la carpeta raíz de la escena (igual que la `src` de un `GltfContainer`). Mantén `font` también configurado: es la alternativa usada siempre que no se pueda cargar la fuente personalizada.


> [!WARNING]
> **📔 Nota**: `fontSrc` necesita `@dcl/sdk` 7.31.0 o posterior. En versiones anteriores la propiedad no existe.


```ts
TextShape.create(sign, {
	text: 'Hola Mundo',
	fontSize: 5,
	fontSrc: 'assets/fonts/Lobster-Regular.ttf',
	font: Font.F_SANS_SERIF,
})
```

Los mismos `fontSrc` la propiedad existe en la UI `Label`, `Button`, `Input` y `Dropdown` entidades, y en la `uiText` de un `UiEntity`. Consulta [Fuentes personalizadas en la UI](/docs/creator/content-creator-es/scenes-sdk7/ui-2d/ui_text/#custom-fonts).

Ten en cuenta lo siguiente al usar fuentes personalizadas:

* **Solo `.ttf` se admiten archivos**. El archivo debe ser una fuente TrueType. Las fuentes OpenType con contornos CFF (normalmente `.otf`), colecciones de fuentes (`.ttc`) y fuentes web (`.woff`, `.woff2`) se rechazan, y el texto se renderiza con la alternativa `font`.
* **La fuente debe ser un archivo en tu escena**. `fontSrc` no puede apuntar a una URL externa, y los nombres de familias tipográficas (por ejemplo, de Google Fonts) tampoco son compatibles. Descarga el `.ttf` archivo, inclúyelo en la carpeta de tu escena y referencia su ruta. Asegúrate de que la licencia de la fuente permita redistribuirla; la mayoría de las fuentes de Google Fonts usan la SIL Open Font License, y esa licencia lo permite.
* **En el Desktop Client, las fuentes personalizadas solo aparecen una vez que los assets de la escena están optimizados**. Cuando publicas una escena, sus fuentes se convierten en paquetes de assets optimizados junto con sus modelos 3D, y la fuente personalizada aparece una vez que se completa esa conversión. Hasta entonces, el texto usa la alternativa `font`. Las escenas publicadas antes de que se admitieran las fuentes personalizadas solo convierten sus fuentes cuando se vuelven a publicar. Lo mismo se aplica a las vistas previas locales: la fuente personalizada solo se muestra al previsualizar con assets optimizados (la **Compress Assets** opción en el Creator Hub, o `npm run start -- --asset-bundles` desde la CLI). Consulta [Vista previa con assets optimizados](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/preview-scene/#preview-with-optimized-assets). El cliente web de Bevy carga el `.ttf` archivo directamente, así que allí la fuente personalizada se muestra de inmediato, incluso en las vistas previas locales.
* **No compatible en móviles**. La aplicación móvil de Decentraland ignora `fontSrc` y renderiza la alternativa `font`. Elige una alternativa que siga viéndose bien y no dependas de la fuente personalizada para que el texto encaje en un espacio fijo. Consulta [Funciones faltantes en móviles](/docs/creator/content-creator-es/desarrollar-para-movil/cliente-movil/missing-features/).
* **El archivo de fuente puede tener como máximo 16 MB**. Los archivos más grandes se rechazan y se usa la alternativa `font` También pueden rechazarse las fuentes con contornos de glifos extremadamente complejos.
* **Caracteres**: Los caracteres latinos (ASCII y Latin-1) se prerenderizan. Cualquier otro carácter que incluya la fuente, como cirílico o chino, se renderiza la primera vez que aparece en la escena, lo que puede causar una pequeña pausa. Los caracteres que la fuente no incluye se dibujan con la fuente sans-serif integrada.
* **Un archivo es un solo estilo**. Un `.ttf` archivo contiene un solo peso y estilo. Para un título en negrita y texto normal del cuerpo, añade ambos archivos y apunta cada texto a su propio archivo. Usa archivos de fuente estáticos: si una familia tipográfica solo se distribuye como una fuente variable, usa sus instancias estáticas.
* **Las fuentes se comparten**: todas las entities de texto que usan la misma `fontSrc` comparten una sola copia de la fuente, así que reutilizar una fuente en toda la escena no cuesta más que usarla una sola vez. Prefiere unas pocas fuentes en lugar de muchas diferentes, ya que cada fuente tiene su propia textura. Cada archivo de fuente añade una descarga optimizada de unos 700 KB.

Cuando el cliente de escritorio no puede usar una fuente personalizada, registra un mensaje en la `SDK_FONTS` categoría de la consola que indica el nombre del archivo y explica por qué. Por ejemplo, `font_src "https://example.com/font.ttf" no puede ser una URL externa`, o que el archivo no forma parte del contenido de la escena, o que la escena todavía no tiene una fuente convertida para él. Estos mensajes están ocultos de forma predeterminada. Para mostrarlos, escribe `/log-matrix enable SDK_FONTS Warning` y `/log-matrix enable SDK_FONTS Log` en el chat, vuelve a cargar la escena con `/reload`, luego abre la carpeta de logs con `/logs`. Si la fuente no se convirtió, comprueba que el archivo sea una TrueType `.ttf` de menos de 16 MB y luego publica de nuevo.

## Propiedades de alineación y padding del texto

El `TextShape` el componente crea un cuadro de texto que tiene tamaño, padding, etc.

* `textAlign`: Selecciona un valor del `TextAlignMode` enum. Los valores posibles incluyen todas las combinaciones entre la alineación vertical (*top*, *middle*, *bottom*) y horizontal (*left*, *center*, *right*).
* `width`: *number*. El ancho del cuadro de texto.
* `height`: *number*. La altura del cuadro de texto.
* `paddingTop`: *number*. Espacio entre el texto y el contorno del cuadro de texto.
* `paddingRight`: *number*. Espacio entre el texto y el contorno del cuadro de texto.
* `paddingBottom`: *number*. Espacio entre el texto y el contorno del cuadro de texto.
* `paddingLeft`: *number*. Espacio entre el texto y el contorno del cuadro de texto.


> [!NOTE]
> **💡 Consejo**: Si un texto debe flotar en el espacio, es buena idea añadir un [`Billboard` componente](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/entity-positioning/#face-the-player) para que el texto rote y mire siempre al jugador, y sea legible.


## Propiedades de sombra y contorno del texto

El texto no tiene sombra por defecto, pero puedes configurar los siguientes valores para darle un efecto similar a una sombra.

* `shadowBlur`: *number*
* `shadowOffsetX`: *number*
* `shadowOffsetY`: *number*
* `shadowColor`: *Color3* object. *Color3* Los objects almacenan un *RBG* color como tres números de 0 a 1.

```ts
TextShape.create(sign, {
	text: 'Texto con sombra',
	shadowColor: { r: 1, g: 0, b: 0 },
	shadowOffsetY: 1,
	shadowOffsetX: -1,
})
```

Las letras del texto también pueden tener un contorno de otro color que rodee su perímetro.

* `outlineWidth`: *number*. Qué ancho tendrá el contorno del texto, en todas las direcciones, como un número de 0 a 1. Por defecto *0*, lo que lo hace invisible.
* `outlineColor`: *Color3* object. *Color3* Los objects almacenan un *RBG* color como tres números de 0 a 1.

## Varias líneas

Si quieres que tu texto ocupe varias líneas, usa `\n` como parte de la cadena. El siguiente ejemplo tiene dos líneas de texto separadas:

```ts
TextShape.create(sign, {
	text: 'Esta es una línea. \nEsta es otra línea',
})
```

También puedes configurar las siguientes propiedades relacionadas con textos de varias líneas:

* `lineCount`: *number*. Cuántas líneas de texto como máximo caben en el cuadro de texto. Si no se establece, no hay límite de líneas. La `textWrapping` propiedad debe ser *true* para usar más de una línea.
* `lineSpacing`: *number*. Cuánto espacio hay entre cada línea.

## Hacia qué lado mira el texto

A `TextShape` se lee correctamente desde su lado local **-Z** lado. Si creas uno y no lo rotas, los jugadores que estén en el lado -Z de la Entity podrán leerlo.

Así que para etiquetar la **+Z** cara +Z de un modelo, haz parent del texto al modelo, desplázalo ligeramente en **+Z** para que no se hunda en la Mesh, y rótalo 180 grados alrededor de Y:

```ts
// Una etiqueta pegada a la cara +Z de un modelo
import { engine, TextShape, Transform } from '@dcl/sdk/ecs'
import { Quaternion, Vector3 } from '@dcl/sdk/math'

const label = engine.addEntity()

Transform.create(label, {
	parent: myModel,
	// Un pequeño desplazamiento en +Z mantiene el texto separado de la superficie del modelo
	position: Vector3.create(0, 1, 0.01),
	rotation: Quaternion.fromEulerDegrees(0, 180, 0),
})

TextShape.create(label, { text: 'Frente' })
```


> [!NOTE]
> **💡 Consejo**: Si el texto debe seguir siendo legible desde dondequiera que esté el jugador, añade un [Billboard](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/entity-positioning/#face-the-player) en lugar de rotarlo manualmente.


## Distribuir el texto en columnas

Los espacios y las tabulaciones no alinean las columnas en las fuentes predeterminadas, porque sus caracteres tienen anchos diferentes. Rellenar una tabla de puntuación con espacios te da columnas irregulares que se desplazan en cuanto un nombre o un número cambia de longitud.

Para construir una tabla, usa un `TextShape` por columna. Dale a cada columna su propia Entity, colócala y pon las filas en su `text` campo separadas por `\n`. Entonces cada fila queda a la misma altura en todas las columnas.

```ts
// Una tabla de puntuación de dos columnas: un TextShape para los nombres y otro para las puntuaciones
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

Alinea cada columna hacia el espacio entre las columnas: `TAM_MIDDLE_LEFT` para la columna izquierda, `TAM_MIDDLE_RIGHT` para la de la derecha. Los números se leen mejor alineados a la derecha, así sus dígitos quedan alineados.

Para actualizar una fila, vuelve a escribir toda la `text` cadena de cada columna. Mantén el mismo número de líneas en cada columna; de lo contrario, las filas dejan de coincidir.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/text.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
