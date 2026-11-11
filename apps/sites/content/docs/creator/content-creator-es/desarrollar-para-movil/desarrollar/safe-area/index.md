> Dónde la UI de la escena puede ubicarse de forma segura en móvil, sin invadir los márgenes de hardware del dispositivo ni los controles del propio cliente.

# Área segura móvil

En un teléfono, dos cosas no relacionadas consumen el espacio de pantalla que tu UI puede usar con seguridad:

* **Los márgenes de hardware del dispositivo** — la muesca o el recorte de la cámara, la barra de estado, el indicador de inicio y las esquinas redondeadas. La UI dibujada debajo de ellos queda parcialmente oculta o es difícil de tocar.
* **Los propios controles del cliente de Decentraland** — joystick, chat, perfil, controles de cámara, botones de acción. La UI de Scene colocada debajo de ellos choca visualmente y compite por los mismos toques.

No tienes que medir ninguno de los dos. El cliente informa de ambas áreas en tiempo de ejecución y el SDK posiciona tu UI dentro de la que elijas, con la [`screenInset`](/docs/creator/content-creator-es/scenes-sdk7/ui-2d/onscreen-ui/#screen-inset-area) opción del renderer:

| Lo que quieres                                                    | Lo que debes pasar                                        |
| ----------------------------------------------------------------- | --------------------------------------------------------- |
| Libre de los márgenes de hardware                                 | nada — `screenInset: 'device'` es el valor predeterminado |
| Libre de los márgenes de hardware **y** los controles del cliente | `screenInset: 'interactable'`                             |
| Toda la pantalla, los márgenes quedan a tu cargo                  | `screenInset: 'none'`                                     |

```ts
// Recomendado para la UI de Scene que no debe chocar con los controles móviles
ReactEcsRenderer.setUiRenderer(uiComponent, { screenInset: 'interactable' })
```


> [!WARNING]
> **Compatibilidad del cliente**: `'interactable'` requiere el cliente móvil `1.12.1` o más reciente. En clientes anteriores, el área se informa como cero y la UI vuelve a cubrir toda la pantalla. Esa misma versión normaliza el área `'device'` entre Android e iOS.
> 
> **No es un no-op en escritorio** — el cliente de escritorio reserva aproximadamente el 25 % izquierdo de la pantalla para su propia UI, así que `'interactable'` cambia también tu diseño de escritorio. Haz una bifurcación con [`isMobile()`](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/detect-platform/) si solo quieres el inset en teléfonos.


Consulta [Área de screen inset](/docs/creator/content-creator-es/scenes-sdk7/ui-2d/onscreen-ui/#screen-inset-area) para la referencia completa: las tres áreas una al lado de la otra en un dispositivo real, el comportamiento por renderer y cómo se relacionan con la [pantalla virtual](/docs/creator/content-creator-es/scenes-sdk7/ui-2d/onscreen-ui/#default-virtual-size).

## Dónde viven los controles del cliente

`'interactable'` te da el área **que cada Explorer designa para la UI de Scene** — eso no es deliberadamente una promesa de que todos los controles del cliente queden fuera de ella. En el cliente móvil es el área segura del dispositivo menos la columna izquierda (chat, perfil, joystick, emotes); los botones de acción en la esquina inferior derecha se dibujan sobre el área **por diseño**.

<figure><img src="/docs/_remote/de3e57715b49b467501da7ac.webp" alt="Scene UI inset to the area the mobile client reports as free of its own HUD"><figcaption><p><code>screenInset: 'interactable'</code> en el cliente móvil <code>1.12.1</code>. El rectángulo magenta es la UI de Scene, el contorno verde delimita el área interactable. La columna izquierda queda excluida; los botones de acción en la esquina inferior derecha se sitúan sobre el área.</p></figcaption></figure>

Así que incluso con `'interactable'`, trata estas zonas como concurridas:

* **Abajo a la derecha** — los botones de acción y el botón de interacción se dibujan encima. Todo lo que coloques aquí sigue siendo accesible, pero compite por los toques.
* **Arriba a la derecha** — los controles de perfil y cámara quedan justo fuera del área, así que una UI pegada a esa esquina se percibe como parte del HUD del cliente.

Lo que cubre el área es decisión de cada Explorer y puede cambiar entre versiones. Léele en tiempo de ejecución, nunca lo codifiques a mano, y compruébalo en las plataformas a las que apuntas.

## Dónde colocar la UI de Scene

* **Centro de la pantalla** — diálogos accionables, cualquier cosa que el jugador necesite leer y responder.
* **Centro superior** — mensajes no accionables, estado y notificaciones.
* **Centro inferior, encima del botón de interacción** — sugerencias contextuales.
* **No en la esquina inferior derecha** — incluso dentro del área interactable, pertenece a los botones de acción.

## Envuelve parte de tu UI en su lugar

Si optaste por no usarlo con `screenInset: 'none'` y quieres proteger solo parte de tu UI, envuélvela en el `ScreenInsetArea` o el `InteractableArea` componente. Cada uno lee su área correspondiente y la aplica a sus hijos. Impórtalos desde `@dcl/sdk/react-ecs`:

```tsx
import ReactEcs, { ReactEcsRenderer, UiEntity, ScreenInsetArea } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export function setupUi() {
  ReactEcsRenderer.setUiRenderer(() => (
    <ScreenInsetArea>
      {/* Un hijo con tamaño 100% × 100% llena exactamente el área segura del inset */}
      <UiEntity
        uiTransform={{ width: '100%', height: '100%' }}
        uiBackground={{ color: Color4.create(0, 0, 0, 0.5) }}
      />
    </ScreenInsetArea>
  ), { screenInset: 'none' })
}
```

Ambos componentes se posicionan de forma absoluta usando los valores que informa el cliente, así que los `positionType` y `position` de sus `uiTransform` están reservados — cualquier valor que establezcas para ellos se ignora. Todas las demás `uiTransform` propiedades (`padding`, `flexDirection`, `alignItems`, …) y los componentes de UI (`uiBackground`, `onMouseDown`, …) funcionan como siempre. Reaccionan automáticamente cuando el área cambia, por ejemplo al girar o cuando aparecen o desaparecen las barras del sistema, y compensan el [factor de escala de la UI](/docs/creator/content-creator-es/scenes-sdk7/ui-2d/onscreen-ui/#screen-virtual-scale), así que los márgenes se colocan correctamente sin importar cuál sea el tamaño de tu pantalla virtual.


> [!WARNING]
> **📔 No apliques el inset dos veces:** el `screenInset: 'none'` en el fragmento anterior importa. Envolver tu UI en un componente mientras el renderer también aplica el inset correspondiente empuja tu UI hacia adentro el doble del margen. Elige una opción u otra.



> [!NOTE]
> **📱 Solo móvil:** los insets del dispositivo solo tienen valores reales en el **cliente móvil**. En el **cliente de escritorio** son `(0, 0, 0, 0)`cero, así que `screenInset: 'device'` el `ScreenInsetArea` componente no tiene efecto allí y tu UI se renderiza exactamente como lo haría sin ellos. El área interactable, a diferencia de los insets del dispositivo, *es* distinto de cero en escritorio.


## Por qué importa

La UI de Scene que se superpone a los controles del cliente:

* Quedará parcialmente oculta detrás del joystick, el botón de interacción o los controles de cámara.
* Competirá por los toques con esos controles — los jugadores activarán accidentalmente uno u otro.
* Harán que tu scene parezca rota en móvil, lo que perjudica la aparición destacada y la retención.

Verifica siempre en un dispositivo real usando el [código QR de preview](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/preview-on-mobile/).

## Relacionado

* [Área de screen inset](/docs/creator/content-creator-es/scenes-sdk7/ui-2d/onscreen-ui/#screen-inset-area) — la `screenInset` referencia completa.
* [Buenas prácticas de UI para móvil](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/ui-best-practices/)
* [Detecta la plataforma desde el código](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/detect-platform/) — usa `isMobile()` para alternar diseños.
* [UI en pantalla](/docs/creator/content-creator-es/scenes-sdk7/ui-2d/onscreen-ui/)
* [Guía de UX y UI](/docs/creator/content-creator-es/scenes-sdk7/disenar-la-experiencia/ux-ui-guide/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/desarrollar-para-movil/desarrollar/safe-area.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
