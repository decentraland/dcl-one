> Usa smart items en tu escena para añadir interactividad.

# Smart Items - Avanzado

La mayoría de los smart items tienen un módulo básico donde puedes configurar solo los ajustes más comunes de forma sencilla, pero puedes desplazarte hacia abajo más allá del **Avanzado** marcador para personalizar casi cualquier aspecto de cómo se comporta el item.



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/m_xWCSDDxpQ" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=m_xWCSDDxpQ>">Watch on YouTube</a></p>



El siguiente item tiene un componente Transform y un módulo básico que muestra solo los campos básicos para configurar un botón. Pero si te desplazas hacia abajo más allá del **Avanzado** marcador, encontrarás todos los ajustes disponibles.

![](/docs/_remote/6f2502ce1e009c232a69e7cb.webp)


> [!NOTE]
> **📔 Nota**: La mayoría de los ajustes del módulo básico también están disponibles en los componentes de más abajo. Los cambios realizados en el módulo básico se reflejan en los componentes de más abajo y viceversa, excepto en algunos casos en los que los ajustes básicos son una abstracción de varios ajustes de más abajo. En esos casos, cambiar los ajustes avanzados a valores que no son compatibles con el módulo básico hará que el campo del módulo básico se marque como indefinido.


## Configuración avanzada

Las propiedades se agrupan en [**componentes**](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/entities-components/). Distintos smart items pueden tener diferentes componentes, según su funcionalidad.

El comportamiento de la mayoría de los items se controla mediante:

* [**Actions**](#actions): El componente Actions define lo que el item puede hacer. Por ejemplo, reproducir un sonido, reproducir una animación, moverse hacia arriba o volverse invisible.
* [**Triggers**](#triggers): El componente Triggers asigna qué eventos hacen que esas acciones ocurran. Por ejemplo, cuando el jugador hace clic en el item, cuando el jugador entra en un área o cuando la scene se carga por primera vez.

Por ejemplo, en un smart item de puerta, el **Actions** componente incluye las acciones "Open" y "Close". El **Triggers** componente de ese item incluye un **On Click** trigger que activa la acción "Open" cuando el jugador hace clic en la puerta.

Los triggers de un smart item pueden activar acciones en cualquier smart item de la scene, no solo en ese mismo smart item. Por ejemplo, un smart item de botón puede tener un **Triggers** componente que activa la acción "move up" definida en el **Actions** componente de una plataforma flotante.

Los triggers también pueden ocurrir condicionalmente. Por ejemplo, los smart items de puerta incluyen dos **On Click** triggers en su componente Triggers: uno abre la puerta si esa puerta estaba cerrada, el otro cierra la puerta si estaba abierta. Para más detalles, consulta [Estados y lógica condicional](/docs/creator/content-creator-es/scene-editor/interactividad/states-and-conditions/).

## Interacciones entre items

Para hacer que los items interactúen entre sí:

* Un item necesita tener al menos una acción definida en un [Actions](#actions) component.
* El otro item necesita un trigger en el [Triggers](#triggers) componente que apunte a esa acción.

Por ejemplo, para hacer que un botón abra una puerta:

1. Añade cualquier smart item de botón, abre su **Triggers** componente. Tiene un evento trigger predeterminado que reproduce un sonido y una animación para el propio botón.
2. Haz clic en el **+** signo junto a **Acciones asignadas**, para añadir una tercera acción en ese mismo evento trigger.
3. Selecciona el smart item de la puerta en el primer desplegable.
4. En el segundo desplegable, selecciona la acción "Open".

![](/docs/_remote/564f4fe0ae016962310472b6.webp)


> [!NOTE]
> **💡 Consejo**: En su lugar, puedes crear un nuevo evento Trigger que solo gestione la acción de la puerta. Ambos eventos trigger se llaman cada vez que se hace clic en el botón.
> 
> <img src="/docs/_remote/4c4d3311baeb14d82a595558.webp" alt="" data-size="original">


Cualquier item puede activar cualquier acción de cualquier otro item, siempre que la acción esté definida. Consulta [Triggers](#triggers) para ver más formas en que se puede activar una acción.

Puedes usar [estados y lógica condicional](/docs/creator/content-creator-es/scene-editor/interactividad/states-and-conditions/) para activar una acción solo si se cumple una condición. La condición incluso puede comprobar el estado de un tercer smart item. Por ejemplo, un botón solo abre la puerta si un smart item personalizado "power generator" tiene su estado establecido en "On".

## Actions

El **Actions** El componente Actions enumera las acciones que el item puede realizar. Cada smart item incluye un conjunto de acciones predefinidas. Puedes personalizar acciones existentes o añadir nuevas. Los siguientes tipos de acciones están disponibles:

* **Reproducir animación**: Reproduce una animación en el modelo 3D del item. Consulta [Acerca de la reproducción de animaciones](#about-playing-animations)
* **Detener animación**: Detiene todas las animaciones que se están reproduciendo en el modelo 3D del item.
* **Reproducir sonido**: Reproduce un sonido desde un archivo, en la ubicación del item. Consulta [Acerca de la reproducción de sonidos](#about-playing-sounds)
* **Detener sonido**: Detiene todos los sonidos que se reproducen desde el item.
* **Iniciar tween**: Realiza un cambio gradual en la posición, rotación o escala durante un período determinado. Consulta [Mover, rotar o escalar](#moving-rotating-or-scaling).
* **Establecer visibilidad**: Hace que el item sea visible o invisible.
* **Adjuntar al jugador**: Establece el item como hijo del avatar del jugador. Por ejemplo, para llevarlo en la mano o sobre su cabeza.
* **Separar del jugador**: Desvincula el item del avatar del jugador.
* **Abrir enlace**: Abre un enlace a un sitio web externo en una pestaña del navegador. Se les pregunta a los jugadores si confían en el dominio antes de que se abra.


> [!NOTE]
> **📔 Nota**: Esta acción solo puede ocurrir como resultado de hacer clic en un item. No puede activarse al caminar hacia un área de trigger.


* **Mover al jugador**: Cambia la posición del jugador a un conjunto de coordenadas locales dentro de la scene. Solo es posible mover al jugador dentro de la misma scene.
* **Teletransportar al jugador**: Teletransporta a un jugador a otra ubicación en Decentraland. Usa el **modo de teleport** desplegable para enviarlo **A coordenadas** de otra scene en Genesis City, o **A World**, indicando el nombre de un World. Los jugadores aparecerán en el spawn-point de la scene de destino.
* **Reproducir emote**: Hace que el avatar del jugador realice una de las animaciones predeterminadas del avatar (por ejemplo: wave o clap).
* **Reproducir emote personalizado**: Hace que el avatar del jugador realice una animación personalizada, desde un archivo cargado en la scene.
* **Mostrar texto**: Muestra texto en la UI de la pantalla, para ocultarlo después de unos segundos. Ideal para pistas, líneas de diálogo, notificaciones, etc.
* **Ocultar texto**: Oculta cualquier texto de la UI que pueda estar mostrándose actualmente.
* **Iniciar retraso**: Retrasa otra acción del mismo item durante tantos segundos como necesites.
* **Detener retraso**: Cancela cualquier acción retrasada en el item.
* **Iniciar bucle**: Repite una acción del mismo item de forma recurrente a un intervalo determinado.
* **Detener bucle**: Cancela cualquier acción en bucle del item.
* **Reproducir video**: Reproduce un video como material en una forma primitiva.
* **Detener video**: Detiene cualquier video que se esté reproduciendo actualmente.
* **Reproducir flujo de audio**: Reproduce un flujo de audio.
* **Detener flujo de audio**: Detiene cualquier flujo de audio que se esté reproduciendo actualmente.
* **Clonar**: Duplica un item en la posición designada.
* **Spawn Entity**: Crea una nueva copia de un item en la scene, en una posición relativa al item que realiza la acción. El item que se va a generar no necesita estar colocado en la scene. Consulta [About spawning entities](#about-spawning-entities).
* **Eliminar**: Elimina un item de la scene.
* **Mostrar imagen**: Muestra una imagen en la UI, potencialmente durante un tiempo limitado. También puede incluir un pie de foto.
* **Ocultar imagen**: Oculta cualquier imagen que se esté mostrando actualmente en la UI mediante la acción Show Image.
* **Daño**: Reduce la salud de cualquier barra de salud que esté cerca. La *Layer* propiedad puede determinar si solo actúa sobre las barras de salud del jugador o sobre otros items.
* **Mover al jugador aquí**: Cambia la posición del jugador a la de este item.
* **Colocar en el jugador**: Cambia la posición del item a la del jugador.
* **Rotar como el jugador**: Cambia la rotación del item a la del jugador.
* **Colocar en la cámara**: Cambia la posición del item a la de la cámara.
* **Rotar como la cámara**: Cambia la rotación del item a la de la cámara.
* **Establecer posición**: Cambia la posición del item a una específica. Puede ser absoluta o relativa a su posición actual.
* **Establecer rotación**: Cambia la rotación del item a una específica. Puede ser absoluta o relativa a su rotación actual.
* **Establecer escala**: Cambia la escala del item a una específica. Puede ser absoluta o relativa a su escala actual.
* **Seguir al jugador**: Empieza a moverse y girar en dirección a la posición del jugador. Ignora cualquier obstáculo en el camino. Puedes establecer la velocidad y hacer que solo se mueva en ciertos ejes. La distancia mínima determina qué tan cerca llegará al jugador.
* **Dejar de seguir al jugador**: Detiene la acción Seguir al jugador.
* **Acción aleatoria**: Una de las acciones enumeradas aquí se reproducirá aleatoriamente con igual probabilidad cada vez que se invoque la acción aleatoria. Puedes enumerar cualquiera de las acciones que pertenezcan al item.
* **Acciones por lotes**: Todas las acciones enumeradas aquí se reproducirán simultáneamente cada vez que se invoque la acción por lotes. Puedes enumerar cualquiera de las acciones que pertenezcan al item.
* **Curar al jugador**: Restaura salud a la barra de salud del jugador.
* **Hacer que el jugador mire al item**: Hace que el avatar del jugador gire para mirar hacia el item.
* **Congelar al jugador**: Impide que el jugador se mueva, salte o realice emotes.
* **Descongelar al jugador**: Restaura la capacidad del jugador para moverse después de una acción Freeze Player.
* **Encender luces**: Enciende la **Light Source** component.
* **Apagar luces**: Apaga la **Light Source** component.
* **Modificar luces**: Cambia propiedades de la **Light Source** componente del item, como su color o intensidad.
* **Seleccionar cámara**: Cambia la vista del jugador a una **Virtual Camera**, ya sea en este item o en otra entity.
* **Cambiar texto**: Cambia el texto de un item con un componente **Text Shape** . También puede cambiar el tamaño de la fuente y el color.
* **Detener tween**: Detiene cualquier movimiento tween que esté actualmente en progreso en el item.
* **Deslizar textura**: Desliza continuamente la textura en el material del item en una dirección determinada, para efectos como agua corriente o cintas transportadoras.
* **Cambiar colisiones**: Activa o desactiva los colliders del item, tanto para física como para eventos de puntero.
* **Cambiar skybox**: Cambia el skybox de la scene a una hora fija del día, expresada en segundos desde medianoche.
* **Restablecer skybox**: Restablece el skybox de la scene a sus ajustes predeterminados.
* **Registrar en la consola**: Imprime un mensaje en la consola, útil para depurar mientras se desarrolla la scene.
* **Delete**: Elimina el item y todos sus hijos de la scene.

Consulta [estados y lógica condicional](/docs/creator/content-creator-es/scene-editor/interactividad/states-and-conditions/) para aprender sobre otras acciones relacionadas con condiciones lógicas, como **Establecer estado** y las acciones de contador.

El **Actions** El componente define posibles acciones, pero estas no hacen nada en la scene a menos que se activen. Las acciones se activan mediante un [trigger](#triggers), ya sea desde el mismo smart item o desde uno diferente.

Para añadir una nueva acción a un item, haz clic en el botón **Añadir nueva acción** en la parte inferior del componente Actions. Luego dale un nombre a la acción, selecciona un tipo y completa los campos adicionales específicos de ese tipo de acción.

![](/docs/_remote/af4f04d249dfe05512b5fb47.webp)

### Triggers

El **Triggers** El componente Triggers define eventos trigger; estos activan acciones cuando ocurre un determinado evento. Existen los siguientes tipos de eventos trigger:

* **On Click**: Cuando el jugador hace clic en el item. Consulta [Acerca de los triggers de clic](#about-click-triggers)
* **Al realizar una Input Action**: Cuando el jugador pulsa un botón de entrada mientras apunta al item.
* **Al hacer clic en cualquier lugar**: Cuando el jugador hace clic en el puntero en cualquier parte, sin necesidad de apuntar al item.
* **Acción primaria global**: Cuando el jugador pulsa el botón Primary (E) en cualquier lugar.
* **Acción secundaria global**: Cuando el jugador pulsa el botón Secondary (F) en cualquier lugar.
* **El jugador entra en el área**: Cuando el jugador entra en un área. Consulta [Trigger Area](/docs/creator/content-creator-es/scene-editor/interactividad/trigger-area/)
* **El jugador sale del área**: Cuando el jugador sale de un área. Consulta [Trigger Area](/docs/creator/content-creator-es/scene-editor/interactividad/trigger-area/)
* **Al aparecer**: Cuando la scene comienza, o el item aparece en la scene. Consulta [trigger al aparecer](#trigger-on-spawn)
* **Al finalizar el retraso**: Cuando una **Iniciar retraso** acción del item termina su cuenta regresiva.
* **En cada iteración del bucle**: En cada iteración de una **Iniciar bucle** acción del item.
* **Al clonar**: Cuando el item se clona mediante una **Clonar** acción, la nueva copia activa este trigger.
* **Al hacer clic en la imagen**: Cuando el jugador hace clic en una imagen mostrada por una **Mostrar imagen** acción.
* **Al recibir daño**: Cuando el item recibe daño de un **Daño** acción.
* **Al curar al jugador**: Cuando el jugador es curado por una **Curar al jugador** acción.
* **En cada tick**: En cada tick de la scene, una vez por frame. Úsalo con precaución, ya que las acciones que activa se ejecutan con mucha frecuencia.

Consulta [estados y lógica condicional](/docs/creator/content-creator-es/scene-editor/interactividad/states-and-conditions/) para aprender sobre otros triggers relacionados con condiciones lógicas, como **Al cambiar de estado**, **Al cambiar el contador** y **Al finalizar el tween**.

Para añadir un nuevo trigger, haz clic en el **Añadir nuevo evento trigger** en la parte inferior del componente Trigger. Luego selecciona el tipo de trigger, la entity que quieres activar y una acción de esa entity.

![](/docs/_remote/4c0c7ee0b8ac3b2401bebd31.webp)


> [!NOTE]
> **📔 Nota**: Es necesario definir una acción en el componente de la entity antes de poder activarla. Los triggers solo pueden afectar a entities que tengan un componente Actions. [Actions](#actions) componente Actions


## Acerca de la reproducción de animaciones

Usa una acción de tipo **Reproducir animación** para ejecutar una animación en el modelo 3D del smart item. La animación debe existir ya como parte del archivo del modelo 3D. El **Seleccionar animación** desplegable muestra una lista de todas las animaciones disponibles en el modelo 3D.

El **Modo de reproducción** campo te permite seleccionar si una animación debe reproducirse solo una vez o si debe seguir en bucle.

![](/docs/_remote/b62f8f6fe8ab5cf4a5ed6eba.webp)

Una vez creada la acción, puedes activarla mediante el [Triggers](#triggers) componente Actions de ese mismo item o de cualquier otro item.

Usa el **Detener animación** acción para detener todas las animaciones del item, tanto las que están en bucle como las que no.


> [!NOTE]
> **💡 Consejo**: Para comprobar fácilmente el contenido de un modelo 3D, ver qué animaciones incluye y cómo se ven, una buena herramienta es [Babylon Sandbox](https://sandbox.babylonjs.com/). Solo arrastra el archivo del modelo 3D a la ventana. Debería aparecer en la parte inferior un desplegable con una lista de sus animaciones.


Para aprender más sobre las animaciones y cómo puedes crear las tuyas propias como parte de un modelo 3D, consulta [Animaciones](/docs/creator/content-creator-es/modelado-y-animaciones-3d/animations/).

## Acerca de la reproducción de sonidos

Usa una acción de tipo **Reproducir sonido** para reproducir un archivo de sonido. Puedes reproducir cualquier archivo de sonido siempre que esté importado en el proyecto de la scene. El sonido se escucha de forma posicional, desde la ubicación del item, lo que significa que sonará más fuerte si el jugador está más cerca.


> [!NOTE]
> **💡 Consejo**: En lugar de escribir la ruta del archivo de sonido, puedes arrastrarlo al **Path** campo desde el menú de navegación de archivos en la parte inferior del Scene Editor.


Usa el **Modo de reproducción** campo para elegir si reproducir el sonido una vez o repetirlo continuamente en bucle.

![](/docs/_remote/eae4449cca4301e0122d9583.webp)

Una vez creada la acción, puedes activarla mediante el [Triggers](#triggers) componente Actions de ese mismo item o de cualquier otro item.

Usa el **Detener sonido** acción para detener todos los sonidos del item, tanto los que están en bucle como los que no. Esto también detiene los sonidos del **AudioSource** component.

Para hacer que un item reproduzca siempre un sonido en bucle, por ejemplo para ambientación o música, es más fácil usar el **AudioSource** componente, en lugar de usar Actions y Triggers. Este componente solo requiere que proporciones una ruta a un archivo y marques las casillas **Iniciar reproducción** y **Bucle**.

![](/docs/_remote/efba578789bf705bd7ec2955.webp)


> [!NOTE]
> **📔 Nota**: Un smart item solo puede reproducir un sonido a la vez. Llamar a un segundo sonido interrumpirá cualquier otro sonido que se esté reproduciendo en ese momento. Esto también se aplica a los sonidos del **AudioSource** componente. Si necesitas que dos sonidos suenen a la vez, considera añadir una entity invisible en la misma ubicación para alojar un **Reproducir sonido** acción.


Consulta [sonidos](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/sounds/) para saber más sobre reproducir sonidos en Decentraland.

## Mover, rotar o escalar

Usa una **Iniciar tween** acción para cambiar la **posición**, **escala**, o **rotación**, del item durante un período de tiempo. Todas las **Iniciar tween** acciones comienzan desde el estado original del item y pasan a un estado final durante un período de tiempo.

Los tweens de posición pueden ser relativos o absolutos. Un tween de posición absoluto mueve el item a una posición fija en relación con la scene. El item se moverá desde donde sea que esté hasta esa posición. Si ya está allí, no parecerá moverse. Un tween de posición relativo mueve el item una cierta distancia desde donde está ahora, por ejemplo un tween a una posición relativa de `1, 0, 0` mueve el item 1 metro hacia adelante, en la dirección hacia la que está mirando actualmente. Si ejecutas la acción tween una segunda vez, el item se moverá otro metro hacia adelante.

Los tweens de rotación también pueden ser relativos o absolutos. Una rotación relativa se añade a la rotación actual del item. Un tween de rotación absoluto hará que el item mire hacia una dirección específica, relativa a la scene.

Usa el **Duración** campo para establecer cuánto tiempo debe durar todo el movimiento, en segundos. Ten en cuenta que el control deslizante llega hasta 100 segundos, pero también puedes escribir manualmente un número mayor si lo necesitas.

![](/docs/_remote/04196395f61a96c50fbd476c.webp)

Una vez creada la acción, puedes activarla mediante el [Triggers](#triggers) componente Actions de ese mismo item o de cualquier otro item.

Los tweens pueden seguir distintos **tipos de curva** que afectan la velocidad del cambio con el tiempo. Una **lineal** curva (predeterminada) significa que la velocidad del cambio es constante de principio a fin. Hay muchas opciones para elegir, que dibujan curvas de distintas formas según si el inicio y/o el final empiezan despacio, y cuánto. Una **easeinexpo** curva empieza despacio y termina rápido, aumentando la velocidad exponencialmente; por el contrario, una **easeoutexpo** curva empieza rápido y termina despacio.

![](/docs/_remote/8451a60c562d729a69cf2138.webp)


> [!NOTE]
> **💡 Consejo**: Experimenta con distintas curvas de movimiento. Las diferencias suelen ser sutiles, pero interpretamos subconscientemente información a partir de cómo se mueven las cosas, como el peso, la fricción o incluso la personalidad.


Usa **Al finalizar el tween** eventos trigger en el **Triggers** componente para activar una acción después de que un tween haya terminado. Usa [estados y lógica condicional](/docs/creator/content-creator-es/scene-editor/interactividad/states-and-conditions/) para describir una ruta en bucle para una plataforma flotante, de modo que se mueva constantemente entre dos ubicaciones.

Cuando un item realiza un tween, esto afecta a todo el item. Por ejemplo, si cambia la escala, cambia la escala de su modelo 3D visible y también la geometría de collider invisible, el tamaño del texto, etc. Si el item tiene hijos (anidados en el árbol de entities de la izquierda), estas entities hijas también se ven afectadas por el tween.


> [!NOTE]
> **📔 Nota**: Cada entity solo puede realizar un tween a la vez. Por ejemplo, no puedes hacer que un item se mueva lateralmente y también gire al mismo tiempo. Como solución alternativa, puedes usar entities parentadas. Por ejemplo, puedes tener una entity padre invisible que se mueva lateralmente, con un hijo visible que gire.


## About spawning entities

Usa una **Spawn Entity** acción para crear una nueva copia de un item en la scene mientras está en ejecución. A diferencia de la **Clonar** acción, el item que generes no necesita haber sido colocado en la scene primero.

### Hacer que un item sea generable

Antes de que un item pueda generarse, debe estar disponible en los archivos de tu proyecto. Para añadir un item sin colocarlo en la scene:

1. En el **Assets** panel, encuentra el item en el catálogo o en tus **Custom Items**.
2. Haz clic derecho sobre el item y selecciona **Añadir al sistema de archivos**.

Esto copia el item en tu proyecto para que pueda generarse más tarde. No añade nada visible a la scene. Luego, el item aparece en el **Source** desplegable de cualquier **Spawn Entity** acción.

### Configura la acción

Un **Spawn Entity** la acción tiene dos campos:

* **Source**: El elemento a generar. Elige entre los elementos añadidos a los archivos de tu proyecto.
* **Posición**: Las coordenadas X, Y y Z donde aparece la nueva copia, con respecto al elemento que realiza la acción.

Si el elemento que generas es a su vez un smart item, cada copia generada conserva sus propias acciones y triggers funcionando de forma independiente.


> [!NOTE]
> **💡 Consejo**: Para generar elementos desde código en su lugar, consulta [Composites](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/composites/). Los elementos añadidos a los archivos de tu proyecto se almacenan como composites, el mismo formato que usa la función spawn del SDK.


## Acerca de los triggers de clic

Para activar una acción al hacer clic en un elemento, crea un **On Click** disparador. La acción se activará cada vez que el jugador haga clic en la entidad.

![](/docs/_remote/342cba6acbd2ae9ce5e8ce5f.webp)

Consulta [Haz que cualquier elemento sea un smart item](/docs/creator/content-creator-es/scene-editor/interactividad/make-any-item-smart/#interactivity) para más detalles.


> [!NOTE]
> **📔 Nota**: Al usar modelos 3D personalizados, el modelo debe tener una geometría de collider invisible para poder hacer clic en él. Consulta [colliders](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/colliders/#pointer-blocking).
> 
> Como alternativa, puedes configurar el **GLTF** componente del elemento, para que su **Visible Layer** de colisión se establezca en **Pointer**.
> 
> Otra alternativa es añadir un **Click Area** smart item, para dibujar un cubo que se superponga al elemento que quieres hacer clic. El smart item Click Area es un [elemento invisible](#invisible-items).


## trigger al aparecer

Los disparadores de tipo **Al aparecer** activan una acción cuando se carga la escena. En lugar de esperar a que el jugador interactúe con un elemento, la acción se ejecuta de inmediato.

Por ejemplo, úsalo para hacer que una plataforma se mueva continuamente. Usa un **Al aparecer** disparador para activar una acción tween. Luego usa **Al cambiar de estado** disparadores para mantenerla en movimiento entre dos o más posiciones.

![](/docs/_remote/b027f05ed8b8aee085690f4b.webp)

## Multiplayer

Todos los smart items son multijugador de forma predeterminada. Consulta [Smart Items - Básicos](/docs/creator/content-creator-es/scene-editor/interactividad/smart-items/) para más detalles.

Puedes cambiar o ajustar este comportamiento multijugador para sincronizar solo ciertos componentes del elemento.

En el **Multiplayer** componente, marca las casillas de los componentes que quieras compartir.

Por ejemplo, una puerta comparte su `Animator` para que todos vean las animaciones de apertura, su `AudioSource` para que todos oigan su sonido, y su `State` para que todos sepan si actualmente está abierta o cerrada. La puerta no comparte su `Visibility` componente, porque la puerta suele estar siempre visible. Si incluyes acciones para activar y desactivar su visibilidad, quizá quieras marcar también este componente para que los cambios se sincronicen entre todos los jugadores.

## Elementos invisibles

Algunos elementos no están pensados para que el jugador los vea, pero son visibles mientras editas tu escena para que sea más fácil gestionarlos. Este es el caso de elementos como **Ambience**, **Trigger Area**, **Click Area**, etc.

En el modo avanzado, estos elementos tienen un **Visibility** componente configurado como invisible. Este componente no afecta a la visibilidad de los elementos en el Scene Editor, pero cualquier elemento configurado como invisible no lo ven los jugadores al ejecutar un Preview.

## Ver también

* [Smart Items - Conceptos básicos](/docs/creator/content-creator-es/scene-editor/interactividad/smart-items/)
* [Estados y condiciones](/docs/creator/content-creator-es/scene-editor/interactividad/states-and-conditions/)
* [Cómo convertir cualquier elemento en un smart item](/docs/creator/content-creator-es/scene-editor/interactividad/make-any-item-smart/)
* [Combínalo con código](/docs/creator/content-creator-es/scene-editor/extender-con-codigo/overview/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scene-editor/interactividad/smart-items-advanced.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
