> Establece la posición, escala, padding y otras propiedades de las entidades de UI.

# Posicionamiento de UI

Para todo tipo de contenido de UI, usa el `uiTransform` componente para establecer el tamaño, la posición y otras propiedades relacionadas con la alineación de la entity.

El `uiTransform` el componente funciona en el espacio 2D de la pantalla de manera muy parecida a la `Transform` el componente funciona en el espacio 3D de la scene.

***Archivo ui.tsx:***

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


## Propiedades de posicionamiento

La alineación de las UI entities se basa en el modelo de alineación Flexbox. Este es un modelo muy potente para organizar dinámicamente entities anidadas dentro de modals que pueden variar de tamaño.


> [!NOTE]
> **💡 Consejo**: La implementación de UI de Decentraland se basa en la de [Yoga](https://yogalayout.com/docs/). Lee [este artículo](https://www.joshwcomeau.com/css/interactive-guide-to-flexbox/) para una cobertura muy accesible y detallada de las propiedades disponibles en Flexbox.


### Tamaño de la entity

Usa `ancho` y `altura` para establecer el tamaño de la entity. Se admiten los siguientes tipos de valores:

* `auto`: El tamaño se adapta para ajustarse al contenido interno. Esto es muy conveniente para texto que puede variar en longitud. Escribe el valor como "auto".
* **Porcentaje**: Como porcentaje de las medidas del parent. Escribe el valor como una cadena que termine en "%", por ejemplo `10 %`.
* **Píxeles**: Escribe el valor como un número.
* **Ancho o alto de pantalla**: Se pueden usar vw (view width) y vh (view height) para indicar una fracción del tamaño completo de la ventana que ejecuta Decentraland. Por ejemplo `10vw` se refiere al 10% del ancho de la ventana, `25vh` al 25% de la altura de la ventana.

Ten en cuenta que estas propiedades afectan el **default** tamaño de ese item, el tamaño del item antes de que se realicen los cálculos de flex grow y flex shrink. El tamaño final puede interpretarse de forma diferente según el tamaño de la entity parent, y las propiedades de Flexbox que estén definidas.


> [!WARNING]
> **📔 Nota**: En las propiedades que admiten tanto números como cadenas, para establecer el valor en píxeles, escribe un número. Para establecer estos campos como un porcentaje de las medidas del parent, escribe el valor como una cadena que termine en "%", por ejemplo `10 %`. También puedes establecer un valor en píxeles como cadena terminando la cadena en `px`, por ejemplo `200px`.
> 
> * Cuando los valores se expresan como porcentaje, siempre se refieren al contenedor del parent. La entity de nivel superior de tu UI también tiene un parent: el renderer la coloca dentro del área seleccionada por [`screenInset`](/docs/creator/content-creator-es/scenes-sdk7/ui-2d/onscreen-ui/#screen-inset-area), que por defecto es el área segura del dispositivo. Así que una `100%` entity de nivel raíz es el 100% de esa área — en un teléfono más estrecho que la pantalla, en escritorio ambas coinciden. Pasa `screenInset: 'none'` si necesitas porcentajes medidos con respecto a la pantalla completa.
> * Si los valores se expresan en píxeles, no se ven afectados por la escala del parent, pero tampoco son **no** píxeles de pantalla sin procesar: se multiplican por el factor de escala de UI derivado de [pantalla virtual](/docs/creator/content-creator-es/scenes-sdk7/ui-2d/onscreen-ui/#screen-virtual-scale), que está activo por defecto. Un `width: 200` significa "200 px en una pantalla del tamaño de la pantalla virtual", y crece o se reduce proporcionalmente en cualquier otra.
> * Si los valores se expresan en `vh` o `vw`, son un porcentaje de la ventana completa, y no se ven afectados por la escala del parent, por la pantalla virtual ni por `screenInset`.
> 
> Para el `auto` para que width/height funcionen, se aplican las siguientes reglas:
> 
> * El UiTransform que usa width/height como “auto” debe tener `alignSelf`: `“center”`/`“flex-start”`/`“flex-end”` O `positionType: “absolute”`
> * Si el UiTransform de un child usa `positionType: “absolute”`, el parent no se adaptará a su tamaño/posición
> * Si el UiTransform de un child usa cualquier sobrescritura de posición, el parent no se adaptará a su tamaño/posición


Estas otras propiedades también están disponibles para ajustar el tamaño de una forma más avanzada:

* `maxWidth` y `maxHeight`: *number* o cadena (como height y width). El tamaño máximo que puede tener la entity.
* `minWidth` y `minHeight`: *number* o cadena (como height y width). El tamaño mínimo que puede tener la entity. Si el parent es demasiado pequeño para acomodar el tamaño mínimo de las entities, desbordarán de su parent.
* `flexBasis`: Esta es una forma independiente del eje de proporcionar el tamaño predeterminado de un item a lo largo del main axis. Establecer la flex basis de un child es similar a establecer el width de ese child si su parent es un container con flex direction: row, o a establecer la height de un child si su parent es un container con flex direction: column.

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

### Organización de child entities

Por defecto, los child entities se posicionan en relación con la esquina superior izquierda de su parent. Puedes usar propiedades como `justifyContent` y `alignItems` para cambiar este comportamiento.


> [!NOTE]
> **💡 Consejo**: Cualquier propiedad que haga referencia a *contenido* hace referencia a entities a lo largo del main axis (determinado por `flexDirection`). Cualquier propiedad que haga referencia


* `flexDirection`: Flex direction controla la dirección en la que se distribuyen los children de un node. Esto también se conoce como main axis. El main axis es la dirección en la que se distribuyen los children. El cross axis es el eje perpendicular al main axis, o el eje en el que se distribuyen las líneas envueltas. Toma su valor de `FlexDirectionType` type. Las siguientes opciones están disponibles:
  * `row` (DEFAULT)
  * `row-reverse`
  * `column`
  * `column-reverse`
* `justifyContent`: Esta propiedad describe cómo alinear los children dentro del main axis de su container. Por ejemplo, puedes usar esta propiedad para centrar un child horizontalmente dentro de un container con `flexDirection` definido como row o verticalmente dentro de un container con `flexDirection` definido como column. El valor de esta propiedad debe ser del `JustifyType` type. Los valores posibles son:
  * `flex-start` (DEFAULT): Alinea los children de un container al inicio del main axis del container.
  * `flex-end`: Alinea los children de un container al final del main axis del container.
  * `center`: Alinea los children de un container en el centro del main axis del container.
  * `space-between`: Distribuye uniformemente el espacio entre los children a lo largo del main axis del container, repartiendo el espacio restante entre los children.
  * `space-around`: Distribuye uniformemente el espacio entre los children a lo largo del main axis del container, repartiendo el espacio restante alrededor de los children. En comparación con space-between, usar space-around hará que el espacio se distribuya al inicio del primer child y al final del último child.
  * `space-evenly`: Distribuye uniformemente el espacio entre los children a lo largo del main axis del container, repartiendo el espacio restante de modo que los espacios entre los children y entre los children y los bordes del container sean todos iguales.
* `alignItems`: Describe cómo alinear los children a lo largo del cross axis de su container. Align items es muy similar a justify content, pero en lugar de aplicarse al main axis, align items se aplica al cross axis. Esta propiedad requiere un valor del `AlignType` type. Las siguientes opciones están disponibles:
  * `stretch`: (DEFAULT) Estira los children de un container para que coincidan con la altura del cross axis del container.
  * `flex-start`: Alinea los children de un container al inicio del cross axis del container.
  * `flex-end`: Alinea los children de un container al final del cross axis del container.
  * `center`: Alinea los children de un container en el centro del cross axis del container.
  * `baseline`: Alinea los children de un container a lo largo de una baseline común. Los children individuales pueden configurarse para ser la baseline de referencia para sus parents.
* `alignSelf`: Align self tiene las mismas opciones y el mismo efecto que `alignItems` pero, en lugar de afectar a los children dentro de un container, puedes aplicar esta propiedad a un solo child para cambiar su alineación dentro de su parent. align self anula cualquier opción establecida por el parent con align items. Toma su valor de `AlignType`, consulta `alignItems` arriba para obtener detalles sobre estas opciones.
* `alignContent`: Align content define la distribución de las líneas a lo largo del cross-axis. Esto solo tiene efecto cuando los items se envuelven en varias líneas usando `flexWrap`. Toma su valor del `AlignType` type. Las siguientes opciones están disponibles:
  * `flex-start`: (DEFAULT) Alinea las líneas envueltas al inicio del cross axis del container.
  * `flex-end`: Alinea las líneas envueltas al final del cross axis del container.
  * `stretch`: Estira las líneas envueltas para que coincidan con la altura del cross axis del container.
  * `center`: Alinea las líneas envueltas en el centro del cross axis del container.
  * `space-between`: Distribuye uniformemente las líneas envueltas a lo largo del main axis del container, repartiendo el espacio restante entre las líneas.
  * `space-around`: Distribuye uniformemente las líneas envueltas a lo largo del main axis del container, repartiendo el espacio restante alrededor de las líneas. En comparación con space-between, usar space-around hará que el espacio se distribuya al inicio de las primeras líneas y al final de la última línea.
* `flexGrow`: Esto describe cómo debe distribuirse cualquier espacio dentro de un container entre sus children a lo largo del main axis. Después de distribuir a sus children, un container distribuirá cualquier espacio restante según los valores de flex grow especificados por sus children. Flex grow acepta cualquier valor de punto flotante >= 0, siendo 0 el valor predeterminado. Un container distribuirá cualquier espacio restante entre sus children ponderado por el valor de flex grow del child.
* `flexShrink`: Describe cómo reducir los children a lo largo del main axis en caso de que el tamaño total de los children desborde el tamaño del container en el main axis. flex shrink es muy similar a flex grow y puede entenderse del mismo modo si cualquier tamaño desbordado se considera espacio restante negativo. Estas dos propiedades también funcionan bien juntas al permitir que los children crezcan y se reduzcan según sea necesario. Flex shrink acepta cualquier valor de punto flotante >= 0, siendo 1 el valor predeterminado. Un container reducirá sus children ponderado por el valor de flex shrink del child.
* `overflow`: Determina qué sucede si el tamaño de los children de una entity desborda a su parent. Usa valores del `OverflowType` tipo.
  * `hidden`: Las entities que desbordan se vuelven invisibles.
  * `visible`: Las entities que desbordan salen de los márgenes del parent.
  * `scroll`: El área se vuelve desplazable, permitiendo al player desplazarse por el contenido desbordado. Consulta [Scrollable containers](#scrollable-containers) para más detalles.
* `flexWrap`: La propiedad flex wrap se establece en los containers y controla qué sucede cuando los children desbordan el tamaño del container a lo largo del main axis. De forma predeterminada, los children se envuelven en varias líneas a lo largo del main axis si es necesario. Si el wrap se desactiva con `nowrap`, los children se fuerzan a una sola línea (lo que puede reducir entities). wrap reverse se comporta igual que wrap, pero el orden de las líneas se invierte. Esta propiedad toma su valor del `FlexWrapType` tipo.
  * `wrap`
  * `nowrap`
  * `wrap-reverse`

### Márgenes y padding

* `margin`: Esta propiedad afecta al espaciado alrededor del exterior de un node. Un node con margin se desplazará respecto a los límites de su parent, pero también desplazará la ubicación de cualquier sibling. El margin de un node contribuye al tamaño total de su parent si el parent tiene tamaño automático. Establece el espacio entre la entity y los márgenes de su parent. El valor esperado es un objeto que contiene las propiedades `arriba`, `left`, `abajo`, y `right`.
* `padding`: Esta propiedad afecta al tamaño del node al que se aplica. Padding en Yoga actúa como si se hubiera establecido box-sizing: border-box;. Es decir, padding no se añadirá al tamaño total de una entity si tiene un tamaño explícito establecido. En nodes con tamaño automático, padding aumentará el tamaño del node así como el desplazamiento de la ubicación de cualquier child. El valor esperado es un objeto que contiene las propiedades `arriba`, `left`, `abajo`, y `right`.

### Ajustar la posición con precisión

En Flexbox, las posiciones de las entities se determinan sobre todo por cómo están parentadas y qué propiedades de organización se establecen en el parent y el child. A menudo no tienes que establecer la `position` propiedad en absoluto. Pero si quieres ajustarlo, o anular por completo el flujo normal de Flexbox y establecer una posición absoluta, aquí están las propiedades relevantes:

* `positionType`: Define cómo se posicionan las entities. Usa un valor del `PositionType` enum.
  * `relative`: (DEFAULT) De forma predeterminada, una entity se posiciona de manera relativa. Esto significa que una entity se posiciona según el flujo normal del layout, y luego se desplaza en relación con esa posición según los valores de `arriba`, `right`, `abajo`, y `left`. El desplazamiento no afecta a la posición de ningún sibling ni de entities parent.
  * `absolute`: Cuando se posiciona de forma absoluta, una entity no participa en el flujo normal del layout. En su lugar, se distribuye de forma independiente de sus siblings. La posición se determina según `arriba`, `right`, `abajo`, y `left` más bajos.
* `position`: Los valores de posición `arriba`, `right`, `abajo`, y `left` se comportan de manera diferente según el `positionType`. Para una entity relativa, desplazan la posición de la entity en la dirección especificada. Sin embargo, para una entity absoluta, estas propiedades especifican el desplazamiento del lado de la entity respecto al mismo lado en el parent. El valor esperado es un objeto que contiene las propiedades `arriba`, `left`, `abajo`, y `right`.


> [!WARNING]
> **📔 Nota** : Un valor positivo para `arriba` o `left` mide la distancia hacia el interior desde ese mismo borde del parent. Ejemplo: para posicionar un component dejando un margen de 20 píxeles con respecto al parent en los lados superior e izquierdo, establece `position` en `{ top: 20, left: 20 }`.


### Visibility

* `display`: Determina si una entity es visible o no. Para hacer invisible una entity, establece `display` en `none`.

### Z Index

El `zIndex` propiedad de un `UiEntity` determina el orden en que se renderizan las entities. Las entities con un `zIndex` más alto se renderizan por encima de las entities con un `zIndex`. El valor predeterminado `zIndex` es 0.

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
> **📔 Nota** : El `zIndex` La propiedad solo ordenará los elements en relación con siblings directos; no puede usarse para renderizar una entity por encima de otras partes del layout tree. En términos de HTML/CSS, cada elemento de UI de DCL crea un nuevo [stacking context](https://web.dev/learn/css/z-index#stacking_context). Para ordenar módulos de UI independientes añadidos con `addUiRenderer()` entre sí, usa las `zIndex` escala del [renderer options](/docs/creator/content-creator-es/scenes-sdk7/ui-2d/onscreen-ui/#stacking-order-between-ui-modules) en su lugar.
> 
> La UI predeterminada de Decentraland, incluyendo el mapa, el chat, etc., siempre se renderiza por encima de todos los demás elementos de UI.


## Scrollable containers

Cuando una entity de UI tiene más contenido del que cabe en el tamaño asignado, puedes hacer que el área sea desplazable estableciendo `overflow` en `scroll` en el `uiTransform`. Luego, el player puede desplazarse por el contenido arrastrando o usando la rueda del mouse.

Para crear un container desplazable, el parent entity debe tener un tamaño fijo (usando `ancho` y `altura`), y los children deben exceder ese tamaño.

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
		{/* Estos children exceden la altura de 400px del parent, haciendo que el área sea desplazable */}
		<Label value="Elemento 1" fontSize={18} uiTransform={{ width: '100%', height: 80 }} />
		<Label value="Elemento 2" fontSize={18} uiTransform={{ width: '100%', height: 80 }} />
		<Label value="Elemento 3" fontSize={18} uiTransform={{ width: '100%', height: 80 }} />
		<Label value="Elemento 4" fontSize={18} uiTransform={{ width: '100%', height: 80 }} />
		<Label value="Elemento 5" fontSize={18} uiTransform={{ width: '100%', height: 80 }} />
		<Label value="Elemento 6" fontSize={18} uiTransform={{ width: '100%', height: 80 }} />
		<Label value="Elemento 7" fontSize={18} uiTransform={{ width: '100%', height: 80 }} />
	</UiEntity>
)
```

Esto es útil para crear listas largas, inventarios, registros de chat, tablas de clasificación o cualquier panel en el que el contenido pueda crecer más allá de lo que cabe en pantalla.

También puedes anidar containers desplazables dentro de otros layouts de UI. Por ejemplo, un modal de diálogo con un encabezado fijo y un cuerpo desplazable:

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
		{/* Encabezado fijo */}
		<Label
			value="Tabla de clasificación"
			fontSize={22}
			uiTransform={{ width: '100%', height: 60 }}
		/>

		{/* Cuerpo desplazable */}
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
> **💡 Consejo**: Usa `flexGrow: 1` en la entity desplazable para que llene el espacio restante en el parent, de modo que se adapte si otros siblings (como un header o footer) cambian de tamaño.


## Tamaño de UI adaptable

Los players con distintos tamaños de pantalla pueden ver tu layout de UI de forma diferente. Los valores en píxeles se escalan según la [pantalla virtual](/docs/creator/content-creator-es/scenes-sdk7/ui-2d/onscreen-ui/#screen-virtual-scale) para ti, de modo que la misma UI conserve sus proporciones entre distintas resoluciones — **no necesitas calcular tú mismo un factor de escala**, y hacerlo aplica la escala dos veces.


> [!WARNING]
> **📔 Nota**: `devicePixelRatio` no participa en el layout de UI. Es una pista de densidad de pantalla — útil para elegir entre una versión 1x, 2x o 3x de una texture — y nada más. Si tu scene se dimensionó en una versión anterior de SDK, espera que la UI dimensionada en píxeles se renderice hasta 2–3 veces más grande en pantallas de alta densidad (retina y mobile), y vuelve a revisar cualquier ajuste hecho manualmente.



> [!CAUTION]
> **📔 Elimina tu propio factor de escala.** Si tu scene multiplica sus tamaños por un factor que calcula a partir de `UiCanvasInformation` — normalmente `Math.min(width / 1920, height / 1080)` — elimina ese multiplicador. Es el mismo factor que ahora aplica SDK por defecto, así que conservar ambos hace que tu UI crezca cuadráticamente con el tamaño de la pantalla. Si prefieres mantener tu propio factor como el único, desactiva la pantalla virtual con `setUiRenderer(ui, { virtualWidth: 0, virtualHeight: 0 })`.


`UiCanvasInformation`, añadido por defecto a la root entity de la scene, sigue siendo la herramienta adecuada para las decisiones de layout que la escala no puede expresar — una disposición de diálogo diferente en una pantalla estrecha, elegir una resolución de texture a partir de `devicePixelRatio`, o leer tú mismo las áreas inset. No es la herramienta adecuada para dimensionar.

El `UiCanvasInformation` El componente contiene la siguiente información:

* `altura`: Altura del Canvas en píxeles
* `ancho`: Anchura del Canvas en píxeles
* `devicePixelRatio`: La relación entre la resolución en píxeles físicos del dispositivo y los píxeles del Canvas. Útil como pista de densidad de pantalla, por ejemplo para elegir entre una versión 1x, 2x o 3x de una texture.
* `interactableArea`: Un `BorderRect` objeto, que detalla el área designada para los elementos de UI de la scene. Este objeto contiene valores para `arriba`, `abajo`, `left` y `right`, cada uno de los cuales es el número de píxeles de ese margen de la pantalla que ocupa la UI del Explorer.
* `screenInsetArea`: Un `BorderRect` objeto, que detalla el área inset de la pantalla (márgenes seguros) reservada por la UI del dispositivo o la plataforma, por ejemplo el notch, la barra de estado, el indicador de inicio o las esquinas redondeadas en mobile. Este objeto contiene valores para `arriba`, `abajo`, `left` y `right`, cada uno de los cuales es el número de píxeles reservados en ese borde de la pantalla. En desktop, esto suele ser `0` en todos los lados.


> [!WARNING]
> **📔 Nota** : Distintos Explorers de Decentraland tendrán valores diferentes para estos, ya que las UIs globales de la plataforma pueden diferir, y los valores pueden cambiar dinámicamente a medida que el usuario expande u oculta distintos menús globales de UI.


```ts
import { engine, UiCanvasInformation } from "@dcl/sdk/ecs"

export function Main() {
  const canvas = UiCanvasInformation.getOrNull(engine.RootEntity)
  if (!canvas) return
  console.log("DIMENSIONES DEL CANVAS: ", canvas.width, canvas.height)
}
```

Algunas otras buenas prácticas sobre tamaños de UI:

* Si el ancho o el alto de cualquier elemento de UI es dinámico, conviene usar también los `maxWidth`, `minWidth`, `maxHeight`, y `minHeight` parámetros para asegurarte de que se mantengan dentro de valores razonables.
* Un tamaño de fuente numérico es un valor de píxel virtual, escalado como cualquier otro. Si quieres un tamaño medido en relación con el Canvas en su lugar, para que se mantenga independientemente de la pantalla virtual, pasa una `vw`/`vh` cadena — consulta [Tamaño de texto adaptable](/docs/creator/content-creator-es/scenes-sdk7/ui-2d/ui_text/#responsive-text-size)


> [!NOTE]
> **💡 Consejo**: Para ver ejemplos prácticos de dimensionamiento de UI, consulta la [`81,-2-ui-screen-inset-area`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/81,-2-ui-screen-inset-area) scene de prueba, que ejecuta los tres `screenInset` modos (`'none'`, `'device'`, `'interactable'`) como tres renderers coexistentes e imprime los valores en vivo de `screenInsetArea` y `interactableArea` y [`76,-10-UiCanvasInformation`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/76,-10-UiCanvasInformation), que lee `UiCanvasInformation` en cada frame para dimensionar la UI de forma responsiva.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/ui-2d/ui-positioning.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
