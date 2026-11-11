> Configura cualquier ítem para que se comporte como un smart item.

# Convierte cualquier ítem en smart



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/wnnEU8GCLjc" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=wnnEU8GCLjc>">Watch on YouTube</a></p>



Los Smart items son simplemente items normales con un **Acción** y/o **Trigger** componente. Puedes agregar estos componentes a cualquier item en tu escena. También puedes importar tus propios modelos 3D personalizados y agregarles lo mismo.

Para agregar componentes a un item, haz clic en el **Icono de +** junto al nombre del item, y selecciona qué componente agregar de la lista desplegable.

![El menú de agregar componente se abre en una Entity, y muestra componentes agrupados bajo 3D Content e Interaction, incluidos Actions y Triggers.](/docs/_remote/e0e52c0c060b63e97e2466dd.webp)

Esto permite una enorme cantidad de posibilidades creativas. Convierte una vela en una palanca que abre un pasaje secreto detrás de una estantería, reproduce sonidos misteriosos desde dentro de un pozo, convierte diamantes en items coleccionables que se reducen a 0 cuando se hace clic en ellos. ¡Hay muchísimas formas imaginativas de combinar estas mecánicas!

## Interactividad

Puedes hacer que un item reaccione a distintas acciones del jugador.


> [!NOTE]
> **💡 Consejo**: Cuando un jugador interactúa con un item, deberías mostrar algún tipo de retroalimentación para dejar clara esa interacción. Si el modelo no tiene animaciones, considera al menos reproducir un sonido. En algunos casos podría funcionar hacer que el item tenga un pequeño tween de escala y luego vuelva a su escala original, como forma de retroalimentación.


Añade un **Trigger** componente, para detectar diferentes acciones del jugador:

* **Pointer events**: Cuando el jugador hace clic o presiona una tecla mientras apunta con el cursor al item.
* **Global button events** Cuando el jugador presiona una tecla, esté donde esté en la escena.
* **Proximidad del jugador**: Cuando el jugador entra en la posición del item.

El **Trigger** componente puede configurarse para estar al tanto de cualquiera de estos tipos de triggers. Cada vez que ocurre un trigger, puede llamar a Actions desde su propio **Actions** componente, o desde el **Actions** componentes de otros items en la escena. Ver [Smart items - Advanced](/docs/creator/content-creator-es/scene-editor/interactividad/smart-items-advanced/).


> [!NOTE]
> **💡 Consejo**: También puedes combinar estos triggers con [lógica condicional](/docs/creator/content-creator-es/scene-editor/interactividad/states-and-conditions/), de modo que las acciones no se llamen cada vez que ocurre el trigger, solo si las condiciones son verdaderas.
> 
> Por ejemplo, podrías agregar un **Pointer Events** trigger a una puerta, para que se abra al hacer clic, pero incluir lógica condicional para que solo se abra si está desbloqueada.


### Pointer events

Añade un **Trigger** component con **Al hacer clic** o **Acción de entrada** Eventos de trigger.

* **Al hacer clic** reacciona cada vez que el jugador hace clic con el botón izquierdo del mouse mientras apunta al item.
* **Acción de entrada** reacciona cada vez que el jugador presiona el Primary Button (E) mientras apunta al item.

![](/docs/_remote/342cba6acbd2ae9ce5e8ce5f.webp)

**Colliders**

Es importante que, para que un item sea clicable, debe tener un **Collider**. De lo contrario, tus clics atravesarán el modelo e intentarán interactuar con lo que esté detrás. Los modelos 3D de los Asset Packs predeterminados deberían tener todos colliders, pero si creas tu propio modelo o lo obtienes de otro lugar, puede que descubras que le falta uno.

Si a tu modelo le faltan colliders, cualquiera de las siguientes opciones debería solucionarlo:

* Añade un **Mesh Collider** componente. Esto creará un collider con una [forma primitiva](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/shape-components/#primitive-shapes) (cubo, plano, cilindro, esfera).
* Cambia las propiedades de la **Collisions** sección en el **GLTF** component. El **capa Visible** debería asignarse a **Pointer**.
* Edita el modelo 3D en Blender para incluir una geometría de collider invisible (cualquier mesh con un nombre que termine en `_collider`). Ver [colliders](/docs/creator/content-creator-es/modelado-y-animaciones-3d/colliders/).


> [!NOTE]
> **💡 Consejo**: Si usaste el **Mesh Renderer** componente para darle a tu modelo una forma primitiva, eso por sí solo no le dará un collider. También debes asignarle un **Mesh Collider** component.


**Personalizar eventos de puntero**

Puedes sobrescribir la configuración predeterminada que se usa cuando un item tiene un **Al hacer clic** o un **Acción de entrada** Trigger Action.

* **Texto al pasar el cursor**: Cambia la pista que los jugadores ven junto al cursor al pasar el cursor sobre el item. Esto puede ser muy útil para aclarar qué hace tu item.
* **Distancia máxima**: A qué distancia puede estar el jugador al interactuar con tu item.
* **Mostrar feedback**: Si no está marcado, el item no mostrará pista al pasar el cursor cuando el jugador lo enfoque.
* **Button**: Si usas el **Acción de entrada** Trigger Action, puedes reasignar el valor predeterminado **Primary (E)** a otra tecla. La pista al pasar el cursor incluirá un icono para aclarar qué tecla usar. Puedes usar **Secondary (F)**, o **Actions 3 a 6** (teclas numéricas 1 a 4).

### Global button events

Añade un **Trigger** component con **Al hacer clic global**, **Primario global** o **Secundario global** Eventos de trigger.

* **Al hacer clic global** reacciona cada vez que el jugador hace clic con el botón izquierdo del mouse, en cualquier parte de la escena.
* **Primario global** reacciona cada vez que el jugador presiona el Primary Button (E), en cualquier parte de la escena.
* **Secundario global** reacciona cada vez que el jugador presiona el Secondary Button (F), en cualquier parte de la escena.


> [!NOTE]
> **💡 Consejo**: A menudo tiene sentido combinar esto con [Estados y condiciones](/docs/creator/content-creator-es/scene-editor/interactividad/states-and-conditions/), para que los items solo reaccionen al evento del botón si el jugador está en la habitación, o si se cumple alguna otra condición.


### Posición del jugador

Añade un **Trigger** component con **El jugador entra en el área**, **El jugador sale del área** Eventos de trigger.

Esto reaccionará cuando el jugador entre o salga de un área de tamaño predeterminado de 1x1x1, posicionada en el centro del item.


> [!NOTE]
> **💡 Consejo**: A menudo es mejor usar el [**Trigger Area**](/docs/creator/content-creator-es/scene-editor/interactividad/smart-items/#trigger-areas) smart item en su lugar, ya que las dimensiones de este item se pueden visualizar claramente en el Scene Editor.


## Ver también

* [Smart items - Conceptos básicos](/docs/creator/content-creator-es/scene-editor/interactividad/smart-items/)
* [Smart items - Advanced](/docs/creator/content-creator-es/scene-editor/interactividad/smart-items-advanced/)
* [Estados y condiciones](/docs/creator/content-creator-es/scene-editor/interactividad/states-and-conditions/)
* [Combinar con código](/docs/creator/content-creator-es/scene-editor/extender-con-codigo/overview/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scene-editor/interactividad/make-any-item-smart.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
