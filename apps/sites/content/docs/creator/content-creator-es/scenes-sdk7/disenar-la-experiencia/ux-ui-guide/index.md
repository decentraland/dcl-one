> Consejos y prácticas para crear escenas en las que los jugadores querrán quedarse y volver.

# Guía de UX y UI

Es de vital importancia para nosotros que los creadores de contenido de Decentraland sean libres de hacer uso de su creatividad en toda su plenitud. Esto es esencial para que Decentraland se convierta en un lugar gratificante para explorar. Sin embargo, también es importante tener en cuenta que las escenas de Decentraland serán visitadas por una gran variedad de usuarios y queremos hacerlas accesibles para todos ellos. Por ello, no podemos ignorar la necesidad de establecer criterios de diseño unificados, una guía que pueda garantizar que los jugadores se encuentren con una experiencia uniformemente intuitiva y agradable, sin importar en qué escena estén.

En este documento compartimos los Valores de Diseño de Decentraland, con el objetivo de promover lo que consideramos las mejores prácticas para diseñar la interfaz y la experiencia de los jugadores. Esperamos que estos criterios puedan servir como punto de partida para construir y/o mejorar las escenas que componen el Metaverso.

## Valores de diseño

Todas las escenas de Decentraland deberían diseñarse teniendo en cuenta estos valores:

* **Acogedora**: El jugador se siente cálidamente bienvenido.
* **Fácil de usar**: La UI es fácil y divertida de usar; deberías evitar reinventar la rueda
* **Fácil de aprender**: Los jugadores encuentran la UI familiar e intuitiva. Los patrones deberían usarse de forma consistente en toda la escena.
* **Ofrecer guía**: La escena da una mano amiga. El texto, el movimiento, el sonido y los gráficos guiarán y orientarán a los jugadores en la dirección correcta.
* **Reactiva**: Una clara reacción a las acciones de los jugadores.
* **Minimalista**: Menos es más. Los jugadores pueden centrar su atención en lo que importa.
* **Interesante**: La escena aprovecha los elementos que hacen que Decentraland sea único y digno de visitar.
* **Con propósito**: Los jugadores tienen una fuerte razón para volver.
* **Agradable**: Los jugadores disfrutan de cómo se ven, suenan y se sienten las cosas

## Experiencia de usuario

### ¿Por dónde empezar?

¿Cuál es el objetivo de tu escena? ¿Es ofrecer un hermoso paisaje para contemplar y explorar? ¿Es un museo? ¿Es un juego para un solo jugador? ¿Uno competitivo? ¿O sigue un flujo narrativo lineal?

Es importante tener muy presente este objetivo en cada paso del proceso de diseño; debería guiar todas tus decisiones, las cuales pueden afectar profundamente la experiencia del visitante.

Para las escenas que siguen un flujo lineal, en el que cada paso depende del anterior, recomendamos delimitar la escena con paredes y establecer uno o varios puntos de entrada fijos, para evitar que los jugadores se encuentren con las cosas en el orden incorrecto. Por cierto, los puntos de entrada son un lugar excelente para mostrar un mensaje de bienvenida al jugador y explicar cuál debería ser su objetivo en la escena.

<figure><img src="/docs/_remote/978e0e21e5bb4482320c0ae1.webp" alt="In-world banner" width="800"><figcaption><p>Instrucciones en el mundo</p></figcaption></figure>

Si la experiencia del jugador en tu escena no está pensada para ser lineal, pero aun así necesitas proporcionar algunas instrucciones básicas para asegurarte de que la disfrute al máximo, puedes mostrar instrucciones en un pop-up. Es posible mostrar un popup en la pantalla del jugador tan pronto como entre, independientemente de la dirección desde la que llegue, y así asegurarte de que los jugadores siempre conozcan lo esencial.

<figure><img src="/docs/_remote/bab29f1240d71035b8426a51.webp" alt="Pop-up banner" width="800"><figcaption><p>Instrucciones emergentes</p></figcaption></figure>


> [!WARNING]
> **📔 Nota**: Asegúrate de que el popup no sea demasiado intrusivo y de que sea fácil cerrarlo haciendo clic en cualquier parte.


Un mensaje de bienvenida debería comunicar lo siguiente al jugador:

* El título de la escena
* Da la bienvenida al jugador
* Objetivo final y motivación para el jugador
* Instrucciones/Siguientes pasos
* *(Opcional)* Controles. Principalmente útil si tu escena usa eventos de botón globales. De lo contrario, los propios elementos indican cómo usarlos al pasar el cursor sobre ellos.

### Motivación

Probablemente querrás que tus visitantes sigan volviendo a tu escena; piensa en maneras de motivarlos a hacerlo. Pregúntate: “¿Qué haría que un jugador quisiera volver a mi escena o recomendarla a un amigo?”

Por ejemplo, puedes desafiarlos dándoles algo que lograr y que sea lo bastante difícil como para requerir varios intentos. También puedes organizar eventos multijugador competitivos, que además son una oportunidad interesante para socializar. ¡Haz de tu escena un lugar dinámico donde los jugadores sientan que disfrutan pasar tiempo allí!

### Retroalimentación y sonido

El sonido cumple una doble función clave, ¡no dejes de usarlo! Por un lado, es vital para generar inmersión; muchas cosas no se sentirán reales si no van acompañadas de sonido. La otra función crucial del sonido es proporcionar retroalimentación a las acciones del jugador. Si te apoyas solo en señales visuales, no puedes estar seguro de que un jugador esté mirando en la dirección correcta para ver los efectos de sus acciones. Al usar sonido, puedes asegurarte de que sea consciente de que algo ocurrió. Si combinas señales visuales y sonoras, oír el sonido invita al jugador a mirar alrededor en busca de la señal visual, por si no la vio.

Ten en cuenta que algunos jugadores podrían estar jugando con el sonido desactivado. Si usas el sonido como una forma de dar instrucciones de la escena o como una pieza clave de la mecánica del juego, intenta proporcionar también algo visual que apunte en la misma dirección. Por ejemplo, podrías mostrar texto en la parte inferior de la UI para acompañar esos sonidos, como subtítulos en una película.

## UI

Esta sección está orientada a ayudarte a construir una narrativa a través del diseño de tu escena que dirija la atención del jugador hacia los elementos que más importan. Hacer un buen uso de las herramientas descritas aquí puede hacer que su experiencia sea mucho más inmersiva y exitosa.

No podemos exagerar lo valiosos que son el color, los iconos, el movimiento y la escala para transmitir el mensaje correcto: todo en la escena comunica un significado, recuerda eso.

### Layout

Ten cuidado al colocar tu UI en regiones de la pantalla que se superponen con los elementos de la UI predeterminada de Decentraland. En el cliente de escritorio, la UI predeterminada de Decentraland (barra lateral, minimapa, chat, etc.) está diseñada para ocupar solo el 25% izquierdo de la pantalla (resaltado en rojo en la imagen). El espacio restante de la pantalla es una zona segura que los creadores de contenido pueden usar libremente.

<figure><img src="/docs/_remote/1f94cb72ac68f183e0aaa73e.webp" alt="Screen regions used by UI" width="800"><figcaption></figcaption></figure>

No hay restricciones para colocar elementos de UI en el 25% izquierdo de la pantalla, pero ten en cuenta que Explorer está realizando continuamente cambios y mejoras en su diseño. Cualquier espacio que hoy no esté cubierto en esa región podría quedar cubierto en versiones futuras.


> [!NOTE]
> **📱 Área segura en móvil**: Los [cliente móvil](/docs/creator/content-creator-es/desarrollar-para-movil/cliente-movil/overview/) superpone sus propios controles sobre parte del Canvas: la **lado izquierdo** (chat, búsqueda, perfil, joystick, emotes), la **parte superior derecha** (controles de perfil y cámara), y la **parte inferior derecha** (botones de acción e interacción). No midas esas regiones a mano: renderiza dentro del área que informa el cliente, con `screenInset: 'interactable'`. Mantén la UI crítica en el centro o en la parte superior central de la pantalla y sigue la [área segura móvil](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/safe-area/). Usa [`isMobile()`](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/detect-platform/) para alternar disposiciones cuando las limitaciones de escritorio y móvil no coincidan.



> [!WARNING]
> **📔 Nota**: Los elementos de UI de las escenas y los wearables inteligentes siempre aparecen en una capa detrás de la UI predeterminada de Decentraland.


#### Nunca ancles tu UI en la esquina superior izquierda

El error de layout más común es anclar un panel a la esquina superior izquierda o dejarlo en el origen de un root que cubre la pantalla. Esa esquina, y todo el borde izquierdo, es donde Explorer dibuja su propio minimapa y chat, y en móvil el joystick virtual. La UI de la escena colocada allí sigue renderizándose, pero la propia UI del cliente se dibuja encima, así que termina cubierta y sus botones a menudo no se pueden pulsar. Nada en el motor de layout te advierte de esto.

Usa una de estas opciones en su lugar:

* **Ancla el elemento a la derecha o céntralo.** Para una puntuación, un temporizador o un panel pequeño, establece `positionType: 'absolute'` y `position: { top: 40, right: 40 }`, o céntralo con `justifyContent: 'center'` en el root.
* **Deja que Explorer te indique dónde es seguro.** Establece [`screenInset: 'interactable'`](/docs/creator/content-creator-es/scenes-sdk7/ui-2d/onscreen-ui/#screen-inset-area) en el renderer. Tu UI se coloca entonces dentro del rectángulo que Explorer reserva para la UI de la escena, lo cual ya excluye el minimapa, el chat y los controles del lado izquierdo.

```ts
ReactEcsRenderer.setUiRenderer(uiComponent, { screenInset: 'interactable' })
```

En móvil, los botones de acción en la esquina inferior derecha se dibujan sobre esa zona a propósito, y los toques allí van a los botones del cliente en lugar de a tu escena. Mantén los elementos interactivos fuera de la esquina inferior derecha o, para una UI que se adueñe de la pantalla de todos modos, oculta los controles táctiles mientras esté abierta:

```ts
import { TouchScreenControls } from '@dcl/sdk/ecs'

// mientras un panel a pantalla completa está abierto, el jugador no necesita saltar ni pulsar E
TouchScreenControls.hideAll()
TouchScreenControls.hideJoystick()

// restáuralos cuando el panel se cierre
TouchScreenControls.showDefault()
TouchScreenControls.showJoystick()
```

`showDefault()` restaura el conjunto con el que comienza una escena: jump, pointer y primary. Usa `showAll()` solo si tu escena realmente quiere los ocho botones en pantalla. Ambos necesitan SDK 7.32.0 o posterior; en SDK anteriores, los ocho botones se muestran por defecto y `showAll()` es la única restauración. Estas ayudas no hacen nada en desktop, así que puedes llamarlas incondicionalmente. Consulta [Controles en pantalla](/docs/creator/content-creator-es/scenes-sdk7/interactividad/touch-screen-controls/).

Mantén siempre una rejilla en mente y úsala como tu criterio principal al organizar espacialmente tu UI. Si estás trabajando en una escena donde se necesita un HUD, puedes empezar agrupando todos los consumibles (por ejemplo, moneda, comida, vida), y en el otro lado colocar objetos almacenables como herramientas o armas. Intenta ser consistente con las métricas de la rejilla y el padding. Haz que la retroalimentación de cada acción del jugador sea clara.

### Color

Es importante elegir una paleta de colores, ya que esto le da identidad a tu escena y también señala relaciones y jerarquía entre elementos.

Al crear una paleta de colores, empieza seleccionando un color principal y, opcionalmente, uno secundario. Luego deberías decidir si quieres que la paleta sea análoga, complementaria o triádica, etc., en relación con ese color principal.

<figure><img src="/docs/_remote/06d8337c29d507083f4727ab.webp" alt="Chromatic color palette" width="300"><figcaption><p>Paleta de colores cromática</p></figcaption></figure>

<figure><img src="/docs/_remote/cf0b4424895884aa68f549b1.webp" alt="Strategies for combining colors properly" width="300"><figcaption><p>Estrategias para combinar correctamente los colores</p></figcaption></figure>


> [!NOTE]
> **💡 Consejo**: Generadores de paletas de colores gratuitos que recomendamos: [Coolors](https://coolors.co), [Adobe Color CC](https://color.adobe.com/), [Colour Lovers](http://www.colourlovers.com), [Color Hunt](http://www.colorhunt.co), [Color by Hailpixel](http://color.hailpixel.com), [Colour Code](http://colourco.de), [Sip](https://sipapp.io/), [Color Scheme Designer by Paletton](http://paletton.com), [Cohesive Colors](http://javierbyte.github.io/cohesive-colors), [Colr](http://www.colr.org)


El color principal debería ser el que más se use en todos los componentes de tu UI. Si tu paleta no tiene un color secundario, puedes resaltar elementos combinando el color principal con negro o blanco. Tener un color secundario no es obligatorio, pero ayuda a enfatizar y distinguir los elementos de la UI de tu escena. Los colores secundarios funcionan mejor para resaltes, controles de selección (sliders y switches), enlaces y encabezados.

Recuerda que cada color tiene sus propias cualidades expresivas únicas; aprovéchalas para comunicar mensajes a través de ellos. Por ejemplo, el rojo suele asociarse con connotaciones negativas, mientras que el verde se asocia con connotaciones positivas.

<figure><img src="/docs/_remote/f0a0dc7306efbc4e1bb5c257.webp" alt="Colored switches" width="300"><figcaption><p>Usar el color de forma incorrecta puede ser confuso</p></figcaption></figure>

También puedes usar los colores estratégicamente para indicar cambios de estado; por ejemplo, cambiando el color de un elemento para indicar si está activo o inactivo.


> [!WARNING]
> **📔 Nota**: Los estados comunican la situación de los elementos de UI. Los estados de un elemento deberían mantener cierta continuidad, pero deben tener affordances claras y ser fácilmente distinguibles de otros estados y del layout circundante.


<figure><img src="/docs/_remote/006fc8ebd49f1ffb83ca7363.webp" alt="Active inactive button" width="300"><figcaption><p>Uso del color para los estados de los botones</p></figcaption></figure>

¡Advertencia! Al seleccionar colores que se mostrarán en superposición, ten especial cuidado de asegurarte de que sean legibles cuando se usen juntos. Aquí tienes una regla fundamental: los colores de los elementos que se muestran juntos siempre deberían tener mucho contraste entre sí. Ten en cuenta que algunos jugadores podrían estar mirando sus pantallas bajo condiciones de iluminación subóptimas, lo que dificulta la lectura.

<figure><img src="/docs/_remote/78f863c16ac2c51e8736e68a.webp" alt="Icon contrast" width="300"><figcaption><p>Ambos iconos pueden distinguirse, pero el primero tiene mejor contraste. Se vuelve más fácil de leer y requiere menos esfuerzo para entenderlo.</p></figcaption></figure>

### Jerarquía tipográfica

Intenta definir al menos una escala de 3 tipos para Títulos, Subtítulos y Cuerpo. Ten cuidado con el uso del color y el tamaño de la fuente. Todos los tamaños de fuente deberían ser de 12 px o más.

Si vas a colocar texto que se muestra sobre imágenes (o sobre el mundo), seguramente necesitarás experimentar con su legibilidad. En esos casos, sugerimos que añadas una región sólida de color, en una capa entre el texto y las imágenes; de ese modo puedes garantizar que el texto siga siendo legible.

<figure><img src="/docs/_remote/26f47a30aa943bf272e86fd8.webp" alt="Text over images" width="300"><figcaption><p>Prioriza la legibilidad</p></figcaption></figure>

### Iconos

Los iconos sintetizan información y te ayudan a identificar acciones mediante imágenes. Son una herramienta increíblemente poderosa para proporcionar entradas que se puedan interpretar rápidamente, a diferencia de usar texto para etiquetar cosas, lo que exige más atención y tiempo por parte del jugador. También es útil mostrar iconos en combinación con texto, ya que esto ayuda a desambiguar sus significados.

<figure><img src="/docs/_remote/d3655e8d247e6f40487d2d6a.webp" alt="Icon examples" width="300"><figcaption><p>Usa los iconos como un lenguaje universal</p></figcaption></figure>

### Movimiento

Usa el movimiento para proporcionar retroalimentación y guiar el camino cuando sea necesario. El movimiento ayuda a los jugadores a centrar su atención y mantiene la continuidad a medida que la UI cambia.

<figure><img src="/docs/_remote/db15a9b89e2052e22e2de076.webp" alt="Adding glow" width="300"><figcaption><p>Usa partículas, escala o bordes desvanecidos para hacer que las cosas destaquen</p></figcaption></figure>

#### Evita los efectos intermitentes

Las luces intermitentes, los estrobos y los cambios rápidos de brillo o color pueden provocar convulsiones en personas con epilepsia fotosensible. Esto incluye materiales emisivos pulsantes, luces parpadeantes, destellos de UI a pantalla completa y videos.

* Limita cualquier parpadeo a **3 destellos por segundo o menos**, siguiendo la [WCAG «Three Flashes or Below Threshold»](https://www.w3.org/WAI/WCAG22/Understanding/three-flashes-or-below-threshold.html) directriz.
* Mantén pequeñas las áreas que parpadean y evita el rojo saturado y los destellos grandes, de alto contraste, que llenen la pantalla.
* Si tu escena depende de efectos intensos, como estrobos de discoteca, relámpagos o destellos de daño, añade una opción en la escena para desactivarlos o suavizarlos.

### Escritura y estructura del contenido

El texto de la UI puede hacer que las interfaces sean más usables y darles a los jugadores más confianza en sus acciones. Haz siempre que el texto de la UI sea lo más conciso posible. Los jugadores están allí para jugar, no para leer. Cualquier texto que parezca demasiado largo no será leído por la mayoría de los jugadores.

Recibe a tus jugadores con un mensaje de bienvenida y cuéntales el objetivo de la escena. Empieza aclarando su meta en la escena y luego las acciones necesarias para lograrla. Después puedes revelar la información de forma progresiva a medida que se necesite; así los jugadores no se sentirán abrumados al comienzo de la experiencia. Al igual que con los recursos gráficos, intenta usar palabras consistentes en todas tus funciones de UI y en la narrativa.

#### Muestra, no cuentes

Dos patrones hacen que los jugadores se vayan antes de su primera interacción:

* **El pop-up de reglas**: un panel de introducción con varios párrafos densos que cubren cada mecánica, regla de puntuación y control. Los jugadores lo cierran sin leerlo.
* **El libro de reglas dentro de la escena**: los mismos párrafos en un cartel o en un panel dentro de la escena. Parece diseño, pero el texto dentro del mundo es demasiado pequeño y demasiado largo para leerlo, y los jugadores no se quedarán quietos para entrecerrar los ojos y leerlo.

Mantén el panel de introducción en una sola pantalla, sin desplazamiento: de 3 a 5 líneas cortas, idealmente una línea para el objetivo, una línea para los controles y una imagen. Un cartel debería decir una sola cosa, en 10 palabras o menos, con un tamaño de fuente grande y un cartel por idea.

Luego muestra el resto en lugar de escribirlo:

* Haz que el primer objetivo sea inconfundible. Coloca un objeto brillante, animado o de gran tamaño en la línea de visión inicial del jugador. Un contorno pulsante enseña «haz clic en mí» mejor que una frase.
* Deja que el jugador realice una mecánica una vez de forma segura, en un área de práctica, antes de que empiece el juego real.
* Usa `hoverText` en entidades interactivas como una instrucción justo a tiempo. Aparece exactamente cuando es relevante y no exige lectura previa. Consulta [Retroalimentación al pasar el cursor](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/register-callback/#hover-feedback).
* Usa un diagrama para reglas espaciales o de secuencia. Una imagen de «gemas rojas = 1, gemas azules = 5» reemplaza un párrafo.
* Entrega cualquier cosa más larga un mensaje a la vez, a través de un NPC o de un diálogo que el jugador pueda omitir.

Si una regla necesita más texto que eso, rediseña la mecánica o muéstrala visualmente.

### Interacciones con objetos

Asegúrate de que los jugadores puedan distinguir qué objetos son interactivos y cuáles no. Al registrar eventos de botón en una entidad, el SDK muestra retroalimentación al pasar el cursor por defecto: un resaltado en el borde de la entidad y una pista junto al cursor que muestra qué entrada usar, con texto personalizado para describir los efectos de interactuar con ella. Recomendamos encarecidamente usarlo; tu escena se sentirá más natural y fácil de usar. Consulta [Retroalimentación al pasar el cursor](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/register-callback/#hover-feedback).

### Cámara en tercera persona

Los jugadores pueden cambiar entre la cámara en primera y tercera persona en cualquier momento pulsando la tecla ‘V’. Diseña tu escena para que las interacciones funcionen cómodamente en ambos modos: por ejemplo, los objetivos de clic pequeños son más difíciles de acertar en tercera persona. Tu escena puede leer el modo de cámara actual del jugador o forzar uno específico en un área; consulta [Cámara](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/camera/).

La cámara en tercera persona también cambia el tamaño que debe tener una habitación. Los interiores construidos con dimensiones reales se sienten estrechos en pantalla; consulta [Construye interiores más grandes que en la vida real](/docs/creator/content-creator-es/scenes-sdk7/disenar-la-experiencia/design-games/#build-interiors-larger-than-real-life).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/disenar-la-experiencia/ux-ui-guide.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
