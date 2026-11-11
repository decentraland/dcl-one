> Aprende a crear una UI para los jugadores en tu escena. Esto es útil, por ejemplo, para mostrar información relacionada con el juego.

# UI en pantalla

Puedes crear una UI para tu escena, que se mostrará en el espacio 2D fijo de la pantalla, en lugar de en el espacio 3D del mundo.

Los elementos de UI solo son visibles cuando el jugador está dentro de los parcels de LAND de la escena, ya que las escenas vecinas pueden tener su propia UI para mostrar. Partes de la UI también pueden activarse para abrirse cuando ocurren ciertos eventos en el espacio del mundo; por ejemplo, si el jugador hace click en un lugar específico.

Construye una UI definiendo una estructura de anidados `UiEntity` objetos en JSX. La sintaxis usada para las UIs es muy similar a la de [React](https://reactjs.org/) (una biblioteca basada en JavaScript muy popular para construir UIs web).


> [!WARNING]
> **📔 Nota**: Solo puedes definir la sintaxis de UI en archivos que tengan una extensión `.tsx` . `.tsx` Los archivos `.ts` admiten todo lo que admiten los archivos `ui.tsx` además de sintaxis de UI. Recomendamos crear un archivo `index.ts` y definir tu UI allí. Recuerda llamar a tu método de renderizado de UI desde `ReactEcsRenderer.setUiRenderer(yourUiMethodName)`, mira el ejemplo de abajo.


Una UI simple con elementos estáticos puede parecerse mucho a HTML, pero cuando añades elementos dinámicos que responden a un cambio de estado, puedes hacer cosas mucho más poderosas.

La UI predeterminada del Explorer de Decentraland incluye un widget de chat, un mapa y otros elementos. Estos elementos de UI siempre se muestran en la capa superior, por encima de cualquier UI específica de la escena. Así que, si tu escena tiene elementos de UI que ocupan el mismo espacio en pantalla que estos, quedarán ocultos.

Consulta [las pautas de UX](/docs/creator/content-creator-es/scenes-sdk7/disenar-la-experiencia/ux-ui-guide/) para obtener consejos sobre cómo diseñar el aspecto y la experiencia de tu UI.


> [!NOTE]
> **📱 Diseñar para móvil**: Los dispositivos reservan espacio en pantalla para el notch, la barra de estado, el indicador de inicio y las esquinas redondeadas, y el cliente dibuja sus propios controles sobre parte del canvas. No hace falta medir nada: elige el área en la que se coloca tu UI con [`screenInset`](#screen-inset-area) más abajo. Por defecto usa el área segura del dispositivo, así que los márgenes de hardware se evitan sin que tengas que hacer nada. Antes de publicar, revisa el [área segura móvil](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/safe-area/) y las [mejores prácticas de UI para móvil.](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/ui-best-practices/).


Cuando el jugador hace click en el botón *cerrar UI* en la esquina inferior derecha de la pantalla, todos los elementos de UI se ocultan.

## Renderizar una UI

Para mostrar una UI en tu escena, usa la función `ReactEcsRenderer.setUiRenderer()` pasándole una estructura válida de entities, descrita en un archivo `.tsx` .

Cada entity se define como un nodo similar a HTML, con propiedades para cada uno de sus components.

***archivo ui.tsx:***

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

***archivo index.ts:***

```ts
import { ReactEcsRenderer } from '@dcl/sdk/react-ecs'
import { uiMenu } from './ui'

export function main() {
    ReactEcsRenderer.setUiRenderer(uiMenu, { virtualWidth: 1920, virtualHeight: 1080 })
}
```

También puedes definir una estructura de entity y renderizarla, todo en un solo comando en un `.tsx` .

***archivo ui.tsx:***

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

***archivo index.ts:***

```ts
import { setupUI } from './ui'

export function main() {
    setupUI()
}
```


> [!WARNING]
> **📔 Nota**: Todos los elementos de tu UI deben estar anidados dentro de la misma estructura y tener un único padre en la raíz de la estructura. Solo puedes llamar a `ReactEcsRenderer.setUiRenderer()` una vez en la escena.


## UI Entities

Cada elemento de la UI debe definirse como una `UiEntity`separada, ya sea una imagen, texto, fondo, una caja de alineación invisible, etc. Igual que en el espacio 3D de la escena, cada `UiEntity` tiene sus propios components para darle una posición, color, etc.

La sintaxis similar a React te permite especificar cada component como una propiedad dentro del `UiEntity`, esto hace que el código sea más corto y legible.

Los components usados en una `UiEntity` son distintos de los usados en entities normales. No puedes aplicar un component de UI a una entity normal, ni un component normal a una UI entity.

Los siguientes components están disponibles para usar en una `UiEntity`:

* `uiTransform`
* `uiBackground`
* `uiText`
* `onMouseDown`
* `onMouseUp`
* `onMouseEnter`
* `onMouseLeave`
* [`uiInputBinding`](/docs/creator/content-creator-es/scenes-sdk7/ui-2d/ui_input_binding/) — mantén acciones de input mientras el elemento está presionado, para construir controles personalizados

Como ocurre con las etiquetas HTML, puedes definir components como autocerrados o anidar uno dentro de otro.


> [!CAUTION]
> **Advertencia:** Mantén los wrappers de diseño de pantalla completa libres de handlers de pointer. Añadir cualquiera de los handlers `onMouse` o `pointerFilter: 'block'`a un `UiEntity` tamaño `100%` por `100%` hace que capture cada click en la pantalla, dejando toda tu otra UI y todo el mundo 3D sin posibilidad de click. En su lugar, coloca los handlers en el panel o botón que los necesite. Consulta [Bloqueo de pointer](/docs/creator/content-creator-es/scenes-sdk7/ui-2d/ui_button_events/#pointer-blocking).


***archivo ui.tsx:***

```tsx
import ReactEcs, { ReactEcsRenderer, UiEntity } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
  // entity padre
  <UiEntity
    uiTransform={{
      width: 200,
      height: 200,
      margin: { top: '250px', left: '500px' },
    }}
    uiBackground={{ color: Color4.Blue() }}
  >
    {/* entity hija autocerrada */}
    <UiEntity
      uiTransform={{
        width: 400,
        height: 400,
        margin: { top: '35px', left: '500px' },
      }}
      uiText={{ value: `¡Hola mundo!`, fontSize: 40 }}
    />
    {/* declaración de cierre para la entity padre */}
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

Una definición de un módulo de UI solo puede tener una entity de nivel superior. Puedes definir tantas otras entities como quieras, pero todas deben encajar dentro de una estructura con un único padre en la parte superior.

## Escala virtual de pantalla

Define un ancho y un alto virtuales para la UI. Esto garantiza que tu UI se vea igual en distintos tamaños de pantalla, independientemente del tamaño real de la pantalla en píxeles.

```ts
export function setupUi() {
    ReactEcsRenderer.setUiRenderer(uiComponent, { virtualWidth: 1920, virtualHeight: 1080 })
}
```

Si defines un ancho virtual de 1920 y un alto virtual de 1080, la UI se escalará para ajustarse al tamaño de la pantalla. Si la pantalla es 1920x1080, la UI se mostrará del mismo tamaño que el tamaño virtual. Si la pantalla es más grande o más pequeña, cualquier valor en píxeles se escalará para ajustarse al tamaño virtual. Por ejemplo, si la pantalla es 3840x2160, un elemento definido con 100 píxeles de ancho se mostrará ocupando 200 píxeles reales.

El cálculo real del factor de escala de la UI que se multiplica sobre los valores en píxeles es [`Math.min(realWidth / virtualWidth, realHeight / virtualHeight)`](https://github.com/decentraland/js-sdk-toolchain/blob/main/packages/%40dcl/react-ecs/src/system.ts).

#### El arte a pantalla completa es más alto que tu tamaño virtual

El factor de escala mantiene tu resolución de referencia **dentro de** la pantalla, así que en una pantalla que no tenga la misma forma que tu tamaño virtual, el espacio extra sigue siendo parte de la UI. Los porcentajes no se escalan, así que un elemento raíz con `width: '100%'`, `height: '100%'` cubre toda la pantalla, lo que puede ser más que tu tamaño virtual en una dirección.

En una pantalla 16:10 con el valor predeterminado `1920x1080`, el factor de escala proviene del ancho, y ese root a pantalla completa mide **1920x1200** en píxeles virtuales. En una pantalla más ancha, en cambio, se vuelve más ancho que 1920.

Así que, para una imagen de fondo que debe llenar la pantalla:

* Exporta el arte **más grande que 1920x1080** y deja que se recorte, como funciona el ajuste CSS `cover` . Un arte con tamaño exacto 1920x1080 deja barras en una pantalla más alta.
* Mantén todo lo que deba seguir visible (logos, títulos, botones) dentro de un área centrada de 1920x1080, y trata todo lo que quede fuera como algo que se puede perder sin problema.

### Tamaño virtual predeterminado

El tamaño virtual es opcional, pero sus dos dimensiones van juntas: pasa ambas, o ninguna. Cuando no pases una, se aplicará un valor predeterminado de la plataforma:

| Plataforma       | Tamaño virtual predeterminado |
| ---------------- | ----------------------------- |
| Móvil            | `1600x720`                    |
| Escritorio y Web | `1920x1080`                   |

Esto significa que los valores en píxeles de tu UI siempre se escalan con respecto a una resolución de referencia, incluso cuando no pasas ninguna opción. También se aplica cuando la escena solo usa `addUiRenderer()` y nunca llama a `setUiRenderer()`.

Tres casos especiales:

* **Desactivar el escalado**: pasa un tamaño inválido — cualquier valor que sea `0` o menor — para desactivar por completo la pantalla virtual. En ese caso, los valores en píxeles se usan como píxeles crudos del canvas, sin escalado. Esta es la forma documentada de apagar la pantalla virtual, así que no se registra nada.

  ```ts
  ReactEcsRenderer.setUiRenderer(uiComponent, { virtualWidth: 0, virtualHeight: 0 })
  ```
* **Tamaños incompletos**: dar solo una de las dos dimensiones también es inválido, así que desactiva la pantalla virtual igual que el caso anterior — el valor predeterminado de la plataforma **no** interviene. A diferencia del caso anterior, este sí se informa: un tamaño dado a medias es un error, no una elección deliberada, así que el SDK registra `Tamaño de pantalla virtual incompleto (…): se requieren ambas dimensiones, así que la pantalla virtual se desactiva y no se aplica ningún escalado de UI.` una vez por tamaño. Pasa ambas dimensiones, o ninguna.

  ```ts
  // No hagas esto — no se aplica ningún escalado
  ReactEcsRenderer.setUiRenderer(uiComponent, { virtualWidth: 1920 })
  ```

  Conviene volver a comprobarlo en el renderer principal, porque un tamaño incompleto allí también es un interruptor que afecta a toda la escena: una `setUiRenderer()` llamada que mencione *cualquiera de las* dimensiones prevalece sobre cualquier `addUiRenderer()`, así que el ejemplo anterior desactiva la pantalla virtual para toda la escena y descarta un tamaño válido pasado a un renderer adicional.
* **Tamaños 16:9 en móvil**: las pantallas de los teléfonos son mucho más anchas que 16:9, así que un canvas virtual 16:9 pondría barras a la UI. Si pasas un tamaño 16:9 (`1920x1080`, `1280x720`, …) y la escena se ejecuta en móvil, se sustituye por `1600x720`, y se registra un mensaje en la consola una sola vez. Los tamaños que no sean 16:9 en móvil, y todos los tamaños válidos en escritorio o web, se respetan tal cual.


> [!NOTE]
> **📔 Nota**: Los `vw` y `vh` son independientes de la pantalla virtual: `1vw` es el 1% del ancho del canvas y `1vh` es el 1% del alto del canvas, exactamente como en CSS. También ignoran el [`screenInset`](#screen-inset-area), así que dentro del valor predeterminado `'device'` un inset `width: '100vw'` es más ancho que un `width: '100%'` — el primero ocupa toda la pantalla, el segundo llena el área con inset.



> [!NOTE]
> **📱 Nota**: La detección de plataforma se resuelve de forma asíncrona. Durante los primeros frames el SDK aún no sabe que se está ejecutando en móvil, así que la pantalla virtual empieza en `1920x1080` y cambia a `1600x720` cuando termina la detección — es posible que veas la UI reescalarse brevemente al cargar, y el mensaje de anulación 16:9 aparece en la consola un momento después de que empiece la escena. Pasa un tamaño virtual explícito que no sea 16:9 si quieres una única resolución de referencia estable desde el primer frame.



> [!WARNING]
> **📔 Lo que cambió para escenas escritas antes de esta versión del SDK**
> 
> Tres cambios afectan a las escenas existentes. Ninguno de ellos es opcional, así que una escena que no se toque en absoluto seguirá viéndose diferente cuando actualice su versión del SDK:
> 
> 1. **Ahora se aplica una pantalla virtual por defecto.** Una escena que no pasaba opciones antes solía distribuir los valores en píxeles como píxeles crudos del canvas. Ahora se escalan con respecto a `1920x1080` (`1600x720` en móvil). Para recuperar el comportamiento anterior, desactiva explícitamente la pantalla virtual con `setUiRenderer(ui, { virtualWidth: 0, virtualHeight: 0 })`.
> 2. **`screenInset` por defecto a `'device'`.** Tu UI ahora se posiciona dentro del área segura del dispositivo, así que en móvil se desplaza hacia adentro, y un `100%` a nivel raíz ya no ocupa toda la pantalla: los fondos a pantalla completa, los dimmers y los overlays se detienen antes del notch y del indicador de inicio. Para recuperar el comportamiento anterior, pasa `setUiRenderer(ui, { screenInset: 'none' })`. Consulta [Área con inset de pantalla](#screen-inset-area).
> 3. **`devicePixelRatio` no interviene en la distribución de la UI.** La UI con tamaño en píxeles ahora es hasta 2–3 veces más grande en pantallas de alta densidad (retina y móviles) que antes. **No hay forma de desactivar esto** — vuelve a comprobar cualquier tamaño que se haya ajustado a mano y elimina cualquier factor de escala que tu escena haya calculado por su cuenta.


## Área con inset de pantalla

Las pantallas no son totalmente utilizables: un teléfono reserva espacio para el notch, la barra de estado, el indicador de inicio y las esquinas redondeadas, y cada Explorer dibuja su propio HUD (minimapa, chat, …) sobre parte del canvas. La propiedad opcional `screenInset` de las opciones del renderer selecciona en qué área de pantalla se coloca tu UI:

| Valor                         | Área en la que se coloca la UI                                                                                                                                                                                                 |
| ----------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `'device'` *(predeterminado)* | El área segura del dispositivo, excluyendo el notch, la barra de estado y las esquinas redondeadas. Se lee desde `UiCanvasInformation.screenInsetArea`.                                                                        |
| `'interactable'`              | El área que el Explorer informa como libre de su propio HUD (minimapa, chat, …). Se lee desde `UiCanvasInformation.interactableArea`. Lo que cubre depende de cada Explorer — compruébalo en las plataformas que te interesen. |
| `'none'`                      | Toda la pantalla, con `0,0` en su esquina superior izquierda.                                                                                                                                                                  |

```ts
// La UI se mantiene libre del notch, la barra de estado y las esquinas redondeadas — este es el valor predeterminado
ReactEcsRenderer.setUiRenderer(uiComponent, { virtualWidth: 1920, virtualHeight: 1080 })

// La UI se mantiene libre del propio HUD del Explorer
ReactEcsRenderer.setUiRenderer(uiComponent, { screenInset: 'interactable' })

// La UI cubre toda la pantalla, tú te encargas de los márgenes
ReactEcsRenderer.setUiRenderer(uiComponent, { screenInset: 'none' })
```

En escritorio los insets del dispositivo son cero, así que `'device'` coloca la UI sobre toda la pantalla allí — el mismo resultado que `'none'`. El área se vuelve a leer en cada tick, así que la UI sigue los insets cuando cambian, por ejemplo al rotar o cuando las barras del sistema aparecen y desaparecen.


> [!WARNING]
> **📔 No ancles la UI en la esquina superior izquierda.** Esa esquina, y todo el borde izquierdo, es donde el Explorer dibuja su minimapa, su chat y, en móvil, el joystick virtual. La UI de la escena colocada allí se renderiza, pero la UI del cliente se dibuja encima, así que queda cubierta y sus botones a menudo no se pueden clickar. Ancla tu UI a la derecha o céntrala, o usa `screenInset: 'interactable'` para que el Explorer la coloque lejos de su propio HUD. Consulta [Nunca ancles tu UI en la esquina superior izquierda](/docs/creator/content-creator-es/scenes-sdk7/disenar-la-experiencia/ux-ui-guide/#never-anchor-your-ui-to-the-top-left).



> [!WARNING]
> **📔 `'interactable'` no es un no-op en escritorio.** A diferencia de los insets del dispositivo, el área interactable *no* es cero en el cliente de escritorio: reserva aproximadamente el 25% izquierdo de la pantalla para su propia UI, así que `screenInset: 'interactable'` coloca tu UI en el 75% restante allí. Ese es el objetivo de la opción, pero significa que también cambia tu diseño de escritorio — usa una rama con [`isMobile()`](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/detect-platform/) si solo lo quieres en teléfonos.
> 
> **Compatibilidad del cliente**: `'interactable'` necesita un Explorer que informe el área. Es compatible en escritorio y, en móvil, a partir de la versión del cliente `1.12.1` en adelante — en clientes móviles anteriores el valor se informa como cero, y la UI vuelve a cubrir toda la pantalla. Esa misma `1.12.1` versión también normaliza el área `'device'` entre Android e iOS, así que tómala como el mínimo para cualquier diseño que dependa de cualquiera de esos insets. `'none'` se comporta igual en todas partes.



> [!WARNING]
> **📔 Nota**: No envuelvas tu UI en los componentes [`ScreenInsetArea` o `InteractableArea`](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/safe-area/#wrap-part-of-your-ui-instead) mientras dejas también el valor correspondiente en el renderer — el inset se aplicaría dos veces, empujando la UI hacia adentro el doble del margen. O bien depende de `screenInset` , o configúralo en `screenInset`y coloca el wrapper tú mismo. `'none'` none


### Las tres áreas en un dispositivo real

Las capturas de abajo muestran la misma escena en el mismo teléfono, renderizada tres veces cambiando solo el valor `screenInset` . El rectángulo magenta es la propia UI; los contornos son las áreas que informa el Explorer: cian es el canvas completo, ámbar es `screenInsetArea`, verde es `interactableArea`.

<figure><img src="/docs/_remote/75b3a862ee39f3dbb1722d14.webp" alt="Scene UI covering the whole phone screen, including the notch strip and the areas under the client controls"><figcaption><p><code>screenInset: 'none'</code> — la UI cubre todo el canvas. Sus esquinas quedan bajo el notch y detrás de los propios controles del cliente.</p></figcaption></figure>

<figure><img src="/docs/_remote/b3f7d16c8434488f8b197eda.webp" alt="Scene UI inset to the device safe area, clear of the notch"><figcaption><p><code>screenInset: 'device'</code> (el valor predeterminado) — la UI se desplaza hacia dentro hasta el área segura del dispositivo, libre del notch y de las esquinas redondeadas. Sigue superponiéndose a los controles del cliente, que no forman parte de esta área.</p></figcaption></figure>

<figure><img src="/docs/_remote/de3e57715b49b467501da7ac.webp" alt="Scene UI inset to the area the client reports as free of its own HUD"><figcaption><p><code>screenInset: 'interactable'</code> — la UI se coloca dentro del rectángulo que el cliente designa para la UI de la escena. En el cliente móvil <code>1.12.1</code>, capturado aquí, eso excluye la columna izquierda (perfil, chat, joystick); los botones de acción en la parte inferior derecha se dibujan sobre el área por diseño.</p></figcaption></figure>


> [!NOTE]
> **💡 Consejo**: `'interactable'` te da el área que cada Explorer designa para la UI de la escena. Eso no significa deliberadamente lo mismo que "todo control del cliente queda fuera" — como muestra la tercera captura, los botones de acción móviles se dibujan sobre el área a propósito. Todo lo que pongas debajo sigue siendo accesible para ti, pero compite por los mismos taps, así que ten presentes las [esquinas concurridas](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/safe-area/#where-the-client-controls-live) y comprueba las plataformas que te interesan: lo que cubre el área depende de cada Explorer y puede cambiar entre versiones.


Cada renderer respeta su propio `screenInset`y, por tanto, la UI principal y cualquier UI añadida con `addUiRenderer()` pueden usar áreas diferentes al mismo tiempo. A diferencia del tamaño virtual, este valor nunca se comparte entre renderers.

## Múltiples módulos de UI

Si tu escena contiene varios systems o modules que definen su propia UI, puedes renderizar cada módulo de UI con `ReactEcsRenderer.addUiRenderer()`. Esto es especialmente útil cuando trabajas en una escena compleja con múltiples componentes de UI, o cuando defines UIs para un [smart item](/docs/creator/content-creator-es/scene-editor/interactividad/smart-items/), que debería poder usarse independientemente de lo que haya en el código del resto de la escena.

El `ReactEcsRenderer.addUiRenderer()` La función requiere que proporcionas una entity como propietaria de la UI. Puede ser cualquier entity, incluso una entity dummy creada solo para usarse como propietaria de la UI.

```ts
export function setupUi() {

    // Crea una entity dummy para que sea la propietaria de la UI
    const dummyEntity = engine.addEntity()

    // Define el módulo de UI como una función que devuelve un array de módulos de UI
    const uiComponent = () => [
      // Función que devuelve un módulo de UI,
      // Función que devuelve un módulo de UI
      // ...
    ]

    // Renderiza el módulo de UI con la entity dummy como propietaria
    ReactEcsRenderer.addUiRenderer(dummyEntity, uiComponent)
}
```

Este fragmento puede existir independientemente de cualquier otro código de UI en la escena. El resto de la escena podría incluir un `ReactEcsRenderer.setUiRenderer()`, o ninguno en absoluto, y aun así la UI se renderizará.

Una `addUiRenderer()` llamada también puede incluir un ancho y alto virtuales, igual que `setUiRenderer()`. Sin embargo, si la escena tiene una llamada a `setUiRenderer()` que también define un ancho y alto virtuales, el ancho y alto virtuales de la `addUiRenderer()` se ignorarán.

```tsx
ReactEcsRenderer.addUiRenderer(dummyEntity, uiComponent, { virtualWidth: 1920, virtualHeight: 1080 })
```

El tamaño virtual es un único valor para toda la escena, y se resuelve así: el tamaño de `setUiRenderer()` prevalece; de lo contrario, prevalece la primera llamada a `addUiRenderer()` que proporcionó uno; y si ningún renderer proporcionó uno, se aplica el [valor predeterminado de la plataforma](#default-virtual-size) . Las opciones que solo llevan un `screenInset` no cuentan como un tamaño proporcionado.

El [`screenInset`](#screen-inset-area) funciona al revés: es por renderer, así que cada módulo de UI puede colocarse en una área de pantalla distinta:

```tsx
// Este widget se mantiene libre del HUD del Explorer, independientemente de lo que use la UI principal
ReactEcsRenderer.addUiRenderer(dummyEntity, uiComponent, { screenInset: 'interactable' })
```

Esa UI puede eliminarse con `ReactEcsRenderer.removeUiRenderer(dummyEntity)` , además, si se destruye la entity propietaria de la UI, la UI también se eliminará. Si `ReactEcsRenderer.addUiRenderer()` se vuelve a llamar para la misma entity pero con un UiRenderer diferente, el anterior se limpia y el nuevo lo reemplaza, manteniendo su lugar en el orden de apilado. Volver a llamarlo es también la forma de cambiar las opciones de un módulo de UI que ya está renderizado, por ejemplo su `zIndex`.

### Orden de apilado entre módulos de UI

De forma predeterminada, los módulos de UI se apilan en el orden en que aparecen por primera vez, y los que aparecen después quedan delante. Los módulos que aparecen por primera vez en el mismo frame — normalmente todo lo registrado antes del primer frame de la escena — se dibujan con la UI principal `setUiRenderer()` al fondo, seguidos por los demás módulos en el orden en que se agregaron. Para controlarlo explícitamente, pasa un `zIndex` en las opciones del renderizador: los módulos con un `zIndex` más alto se renderizan delante de los que tienen uno más bajo, sin importar el orden en que aparecieron. Los módulos que se dejan en `0` conservan el orden predeterminado. La opción funciona igual en `setUiRenderer()`, por lo que la UI principal también puede colocarse delante de los otros módulos.

```tsx
// Registrado primero, pero siempre delante de los otros módulos de UI
ReactEcsRenderer.addUiRenderer(dummyEntity, uiComponent, { zIndex: 10 })

// La UI principal queda detrás de cualquier módulo que no establezca un valor más bajo
ReactEcsRenderer.setUiRenderer(mainUi, { zIndex: -10 })
```

Este es un ajuste diferente del `zIndex` de un `UiEntity`, que solo ordena [los elementos hermanos dentro de un módulo de UI](/docs/creator/content-creator-es/scenes-sdk7/ui-2d/ui-positioning/#z-index). La opción del renderizador ordena módulos completos entre sí, y los elementos dentro de cada módulo conservan su propio `zIndex`.

### Compartir una sola declaración setUiRenderer

En lugar de llamar `ReactEcsRenderer.addUiRenderer()` para cada módulo de UI, puedes llamar `ReactEcsRenderer.setUiRenderer()` una sola vez con un array de módulos de UI, que pueden estar en archivos diferentes.

```ts
const uiComponent = () => [
  // Función que devuelve un módulo de UI,
  // Función que devuelve un módulo de UI
  // ...
]

ReactEcsRenderer.setUiRenderer(uiComponent, { virtualWidth: 1920, virtualHeight: 1080 })
```

A continuación, un ejemplo más completo:

***archivo ui.tsx:***

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
      <Label value="¡Hola, mundo!" fontSize={18} textAlign="middle-center" />
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
        value="¡Aquí hay más UI!"
        fontSize={18}
        textAlign="middle-center"
      />
    </UiEntity>
  )
}
```

***archivo index.ts:***

```ts
import { ReactEcsRenderer } from '@dcl/sdk/react-ecs'
import { UIModule1, UIModule2 } from './ui'

export function main() {
    ReactEcsRenderer.setUiRenderer(() => [
      UIModule1(),
      UIModule2(),
      // ...
      // La línea de abajo es para usar la biblioteca DCL UI Toolkit
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
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/ui-2d/onscreen-ui.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
