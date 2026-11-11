> Diseña visualmente la UI en pantalla de tu escena en Creator Hub, sobre un Canvas en lugar de en código.

# Editor de UI

El Editor de UI te permite construir la interfaz en pantalla de tu escena arrastrando widgets sobre un canvas, en lugar de escribir el código de layout a mano. Escribe archivos reales en tu escena, así que todo lo que hagas aquí es UI normal de SDK7 que puedes seguir editando en código.

## Ábrelo

Usa la **2D** / **3D** selector en la parte superior del panel izquierdo del editor. **3D** es el canvas de la escena que ya conoces. **2D** es el Editor de UI.

![El selector de pestañas 2D y 3D, con 3D seleccionado.](/docs/_remote/4e696bf6f0bb7532d974ec02.webp)


> [!WARNING]
> **📔 Nota**: El Editor de UI necesita `@dcl/sdk` la versión 7.26.0 o superior en tu escena. En una escena más antigua, la **2D** pestaña muestra un **Editor de UI no disponible** aviso en lugar del canvas, con un **Actualizar SDK** botón que actualiza la escena por ti y un **Quizá más tarde** botón para descartarlo.


Creator Hub recuerda si dejaste una escena en 2D o 3D y la vuelve a abrir de la misma forma. Esto se guarda por proyecto y nunca se publica con tu escena.

Si estás usando el renderizador de escenas de Bevy, cambiar a **2D** pausa la escena en ejecución, y volver a **3D** la reanuda si se estaba ejecutando antes.

## Crea tu primera UI

1. Cambia a la **2D** pestaña.
2. Haz clic en **Nueva GUI**, ya sea el botón en el centro del canvas vacío o el **+** junto a **GUIs** en el panel izquierdo. Una GUI es un componente de UI, almacenado en su propio archivo.
3. Arrastra un widget desde la paleta de la parte inferior al canvas. **Pantalla completa** es un buen primer widget; te da una raíz que cubre toda la pantalla. Luego suelta otros widgets dentro.
4. Selecciona un widget y configura sus propiedades en el panel derecho.

No hay botón de guardar. El editor escribe tus cambios en el disco a medida que trabajas, y la insignia a la izquierda de la barra de herramientas dice **Todos los cambios guardados**.

![El Editor de UI en modo 2D: los paneles GUIs y Nodes a la izquierda muestran una GUI llamada MainUI con un Container que contiene un Label y un Button, el canvas en el centro muestra el container con el Button seleccionado y tiradores de redimensionado alrededor, la pestaña Properties a la derecha muestra la posición y el layout del Button, y la paleta de widgets se extiende por la parte inferior.](/docs/_remote/3958b77d4c214591522f3386.webp)

## Paneles

* **Izquierda**: **GUIs**, la lista de tus componentes de UI, y **Nodes**, el árbol del seleccionado. Un cuadro de búsqueda filtra ambos. Pasa el cursor sobre un nodo para bloquearlo, ocultarlo o eliminarlo.
* **Derecha**: una **Properties** pestaña para el nodo seleccionado, y una **Logic** pestaña.
* **Parte inferior**: la paleta de widgets. Arrastra una tarjeta al canvas para añadirla.
* **Centro**: el canvas. Arrastra nodos para moverlos, usa los tiradores para redimensionarlos y desplázate libremente con la rueda del ratón o arrastrando.

Dos grupos flotantes de controles se sitúan en la esquina inferior derecha del canvas:

* El **vista previa** grupo activa o desactiva las guías del HUD móvil y las guías del área segura, y cambia entre **Vista previa de escritorio** y **Vista previa móvil**.
* El **zoom** grupo tiene un botón de menos y uno de más, y un campo de porcentaje. Haz clic en el porcentaje para escribir un nivel de zoom exacto entre 10 y 200, luego pulsa Enter o haz clic fuera para aplicarlo, o Escape para cancelarlo. El botón de más se desactiva una vez que llegas al 200%.

<div align="left"><img src="/docs/_remote/589a76a30cb6467ad0c864b3.webp" alt="El panel izquierdo del Editor de UI, con las pestañas 2D y 3D, un cuadro de búsqueda, la lista de GUIs que contiene MainUI y el árbol de Nodes que muestra un Container con un Label y un Button, con la fila del Button mostrando iconos de bloqueo, ocultar y eliminar." width="220"> <img src="/docs/_remote/e71a2f9dd042f80664595bb6.webp" alt="La pestaña Properties del Editor de UI para un Button seleccionado, con Visibility, Interaction States, una sección Position con Constraints, Position y Z-Index, y una sección Layout con Size, Min Size, Max Size, Padding y Margin." width="360"></div>

## Widgets

![La paleta de widgets en la parte inferior del Editor de UI, con tarjetas para Full Screen, Container, Image, Label, Button, Input y Dropdown.](/docs/_remote/353fd7589685b5f5d6abd5f2.webp)

La paleta agrupa los widgets en tres categorías:

* **Contenedores**: **Pantalla completa**, **Container**, **Image**
* **Texto**: **Label**, **Button**
* **Input**: **Input**, **Dropdown**

**Pantalla completa** es un contenedor que llena aquello en lo que se coloca, lo que lo convierte en un buen punto de partida para una UI que deba cubrir la pantalla.

## Mover y redimensionar

La barra de herramientas tiene tres modos de herramienta:

* **Libre**: mover y redimensionar, el modo de uso general.
* **Mover**: solo arrastrar.
* **Redimensionar**: solo tiradores.

Las posiciones se ajustan a una cuadrícula de 10 píxeles. Mantén pulsado **Shift** mientras arrastras para mover libremente, ignorando la cuadrícula. La pequeña flecha junto a los botones de herramienta abre una **Snap** casilla que desactiva por completo el ajuste.

![La barra de herramientas 2D, con la insignia Todos los cambios guardados, deshacer y rehacer, las herramientas Free, Move y Resize, y el desplegable Snap abierto mostrando una opción Snap marcada, seguido de los botones de reproducir y detener.](/docs/_remote/5ab6e1170c733fa0011bba2b.webp)

## Diseño: flujo y posicionamiento libre

Dos propiedades distintas deciden dónde termina un nodo. Tener esto claro evita mucha confusión.

**Flujo** se establece en un **contenedor** y decide cómo organiza a sus hijos:

* **Libre**: los hijos se quedan donde los pongas.
* **Fila** / **Fila inversa**: los hijos se distribuyen de izquierda a derecha, o de derecha a izquierda.
* **Columna** / **Columna inversa**: los hijos se apilan de arriba abajo, o de abajo arriba.
* **Ajustar hijos**: permite que los hijos pasen a más de una línea cuando no caben.

**Ignorar flujo del layout** se establece en un **hijo** y decide si ese nodo se excluye del orden de su padre. Desactivado, el padre lo posiciona. Activado, lo fijas tú mismo con valores de Anchor y Position.

De esto se derivan dos consecuencias:

* Suelta un widget en un **Libre** contenedor y se queda exactamente donde lo soltaste.
* Suelta uno en un contenedor con una dirección y se une al flujo, ocupando su lugar en la línea.

Cambiar un contenedor a **Libre** fija a sus hijos existentes donde están ahora, así que nada salta.

La raíz de una GUI siempre se posiciona libremente.

## Otras propiedades que conviene conocer

* **Opacidad**: 100% es completamente opaco, 0% completamente transparente. El valor predeterminado es 100%.
* **Inset de escena**: en qué parte de la pantalla se sitúa la GUI.

  * **Pantalla completa** usa toda la pantalla que se puede renderizar.
  * **Área segura interactuable** se mantiene alejada de la interfaz propia del cliente, como el chat, el minimapa y los indicadores del HUD.
  * **Área segura del dispositivo** también evita obstrucciones físicas como una muesca o las barras del sistema. Esta opción se ofrece cuando el canvas está en vista previa móvil.

  En el código que escribe el editor, estas tres opciones se convierten en `screenInset: 'none'`, `'interactable'` y `'device'`. Consulta [Área con inset de pantalla](/docs/creator/content-creator-es/scenes-sdk7/ui-2d/onscreen-ui/#screen-inset-area).

Si le das a la raíz de una GUI un ancho y un alto en píxeles fijos, el canvas la encuadra como una mesa de trabajo de exactamente ese tamaño en lugar de como una pantalla.

## Comprueba tu UI en un teléfono

Los controles de la esquina inferior derecha del canvas cambian la vista previa entre una pantalla de escritorio y un teléfono. En la vista previa móvil, el canvas se dibuja dentro de un marco de teléfono, con una muesca incluida, para que puedas ver dónde caen realmente los bordes de la pantalla.

Dos conjuntos de guías te ayudan a colocar las cosas donde los jugadores puedan verlas y alcanzarlas:

* **Guías del área segura** delimitan la parte de la pantalla a la que está confinada tu GUI, coincidiendo con el **Inset de escena** que elegiste. Mientras el Scene Inset sea **Área segura del dispositivo** o **Área segura interactuable** el contorno está siempre activo, porque es a lo que se recorta la GUI. Si estableces el Scene Inset en **Pantalla completa** puedes activar o desactivar el contorno con el botón del icono de marco.
* **Guías del HUD** dibujan los controles propios del cliente móvil, como el joystick, los botones de salto y acción, la rueda de emotes, el perfil, el chat y la brújula. Son solo de referencia; no puedes seleccionarlos ni moverlos. El botón del mando de juego los activa y desactiva, solo aparece en la vista previa móvil, y las guías se muestran por defecto cuando el Scene Inset es un área segura.

El contenido colocado fuera del área segura no se oculta; se dibuja más allá del contorno. Ese desbordamiento es la advertencia: en el cliente real quedaría debajo del HUD propio del juego.

**Área segura interactuable** deja libre la columna izquierda de controles del cliente móvil, pero comparte su borde derecho con el área del dispositivo, así que los botones de acción en la esquina inferior derecha se dibujan encima por diseño. Mantén esas esquinas libres. Consulta [Dónde viven los controles del cliente](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/safe-area/#where-the-client-controls-live).


> [!WARNING]
> **📔 Nota**: Estas guías son solo una aproximación para el editor. Las áreas reales las informa el cliente en runtime y varían según el dispositivo, así que confirma siempre en un teléfono real. Consulta [Vista previa en móvil](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/preview-on-mobile/).


## Personaliza los controles móviles

En el cliente móvil, los jugadores se mueven y actúan mediante un conjunto de controles nativos en pantalla: un joystick, una mira y una fila de botones de acción. La **MobileHUD** entrada te permite rediseñar ese HUD sin escribir código, y muestra el resultado en el canvas a medida que avanzas.

Una vez que tu escena tenga al menos una GUI, **MobileHUD** aparece como la primera fila de la **GUIs** lista, encima de tus propias GUIs. Selecciónala para abrir su panel a la derecha.

Tres casillas de verificación en la parte superior ocultan secciones completas del HUD:

* **Ocultar joystick**: elimina el stick de movimiento.
* **Ocultar mira**: elimina la retícula de puntería.
* **Ocultar Input Actions**: elimina todos los botones de acción de una sola vez.

Debajo de ellos, la **Input Actions** sección enumera los ocho botones, en el orden en que ocupan las ranuras en pantalla:

| Acción                                   | Botón en pantalla                  |
| ---------------------------------------- | ---------------------------------- |
| `IA_JUMP`                                | El gran botón central, por defecto |
| `IA_POINTER`                             | El botón de interacción            |
| `IA_PRIMARY (E)`                         | El botón E                         |
| `IA_SECONDARY (F)`                       | El botón F                         |
| `IA_ACTION_3 (1)` para `IA_ACTION_6 (4)` | Los botones numerados              |

Cada fila te da tres cosas:

* **Principal**: un botón de opción que hace que esta acción sea el gran botón central. `IA_JUMP` es Principal por defecto, y solo una acción puede ser Principal a la vez.
* Una **icono de ojo** que muestra u oculta ese botón. Cuando ocultas uno, los botones inferiores suben para llenar el hueco.
* **Icono personalizado**: reemplaza el glifo del botón con una `.png`, `.jpg` o `.jpeg` imagen de tu escena.

En este panel, todos los botones empiezan visibles y `IA_JUMP` es el botón principal.


> [!WARNING]
> **📔 Nota**: Una escena que nunca toca el HUD muestra solo tres botones: salto, puntero y principal. El panel solo escribe `src/mobile-hud.ts` una vez que cambias algo, así que un panel sin tocar conserva ese valor predeterminado. En cuanto el archivo existe, su código reemplaza el conjunto predeterminado con lo que configures aquí, así que ocultar solo el joystick, por ejemplo, también pone los cinco botones extra en la pantalla del jugador. Oculta los que tu escena no use. Consulta [Qué se muestra por defecto](/docs/creator/content-creator-es/scenes-sdk7/interactividad/touch-screen-controls/#what-shows-by-default).



> [!NOTE]
> **💡 Consejo**: Con más de cinco botones visibles, los últimos quedan detrás de un control desplegable de desbordamiento "+". Oculta suficientes botones de arriba y el resto se muestra directamente. Consulta [Controles en pantalla](/docs/creator/content-creator-es/scenes-sdk7/interactividad/touch-screen-controls/#how-the-button-layout-works) para ver cómo se llenan las ranuras.


### El archivo que escribe

Tus cambios se escriben en `src/mobile-hud.ts`, que exporta una `setupMobileHud()` función basada en los [`TouchScreenControls`](/docs/creator/content-creator-es/scenes-sdk7/interactividad/touch-screen-controls/) component:

```ts
import { engine, InputAction, TouchScreenControls } from '@dcl/sdk/ecs'

export function setupMobileHud() {
  TouchScreenControls.createOrReplace(engine.RootEntity, {
    hideJoystick: true,
    hideCrosshair: false,
    mainAction: InputAction.IA_PRIMARY,
    touchInputs: [
      { inputAction: InputAction.IA_ACTION_3, hide: true },
    ],
  })
}
```

El editor también añade el import y la llamada a tu `src/index.ts`:

```ts
import { setupMobileHud } from './mobile-hud'

setupMobileHud()
```

El archivo solo existe mientras tenga algo que decir. Si vuelves a poner todo en su valor predeterminado, o eliminas tu última GUI, Creator Hub elimina `src/mobile-hud.ts` y vuelve a eliminar esas dos líneas.


> [!WARNING]
> **📔 Nota**: Mientras **MobileHUD** está seleccionado, la paleta de widgets se desactiva y los controles de vista previa se ocultan, ya que no hay nada que colocar y el HUD es solo para móviles. Selecciona una de tus propias GUIs para recuperarlos.


## Dónde van los archivos

El Editor de UI edita los archivos fuente reales de tu escena. No hay un formato guardado separado.

* Cada GUI es un `.tsx` archivo en `src/ui/`.
* `src/ui/index.tsx` lo genera el editor para reunirlos. No lo edites a mano.
* `src/mobile-hud.ts` contiene tus [controles móviles](#customize-the-mobile-controls), si cambiaste alguno. Está fuera de `src/ui/` a propósito, así que nunca se trata como una GUI.

Si tu escena tiene un antiguo único `src/ui.tsx` archivo, el editor lo respalda como `src/ui.tsx.bak` la primera vez que abras el Editor de UI.

Como se trata de archivos normales, puedes abrirlos en tu [editor de código](/docs/creator/content-creator-es/scene-editor/extender-con-codigo/overview/) en cualquier momento. Consulta [UI](https://github.com/decentraland/docs/tree/main/creator/sdk7/2d-ui/README.md) para ver qué hace el código subyacente.

## Código que el editor no puede editar

El editor entiende una forma concreta de código de UI. Un nodo escrito de otra manera, por ejemplo construido dentro de un bucle o un condicional, o usando un componente personalizado o spread props, se dibuja en el canvas como un bloque gris con una **no estándar, edítalo en código** insignia. Sus hijos no se muestran, y en su lugar lo editas en código.

El resto de la GUI a su alrededor sigue funcionando con normalidad.

## Páginas relacionadas

* [UI](https://github.com/decentraland/docs/tree/main/creator/sdk7/2d-ui/README.md): escribir la UI de la escena en código.
* [Combinar con código](/docs/creator/content-creator-es/scene-editor/extender-con-codigo/overview/): abre tu escena en un editor de código.
* [Entidades y Componentes](/docs/creator/content-creator-es/scene-editor/construir/components/): la parte 3D del editor.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scene-editor/construir/ui-editor.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
