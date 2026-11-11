> Gestionar estados de los ítems y lógica condicional

# Estados y condiciones



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/wm8ZD2kSyKA" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=wm8ZD2kSyKA>">Watch on YouTube</a></p>



## Lógica condicional

Añade condiciones a un trigger, de modo que la acción solo ocurra si se cumplen esas condiciones. Por ejemplo, hacer clic en una puerta solo activa la acción "Open" si todavía no estaba abierta.

Para añadir una condición, haz clic en el ícono de tres puntos junto a **Evento del trigger** y selecciona **Añadir condición del trigger**.

![](/docs/_remote/39d3afce89ca9fad61d21f61.webp)

Un solo trigger puede incluir varias condiciones. Haz clic en el **+** ícono para añadir más condiciones. Cuando exista más de una condición, puedes seleccionar una de estas opciones:

* **Deben cumplirse todas las condiciones (AND)**: El trigger solo ocurre si todas las condiciones son verdaderas.
* **Puede cumplirse cualquier condición (OR)** El trigger ocurre si al menos una de las condiciones es verdadera.

![](/docs/_remote/95e6cb8369815d94d5f26f5a.webp)

### States

El **States** El component se incluye en varios smart items. Enumera los estados posibles en los que puede estar el smart item. En cualquier momento dado, el smart item está en uno de estos estados. Por ejemplo, una puerta puede estar *Abrir* o *Cerrado*. La acción Open establece el estado en *Abrir*, la acción Close establece el estado en *Cerrado*.

Puedes hacer lo siguiente con los estados:

1. Usa una condición en un trigger para comprobar el estado de una Entity. De esa manera, la acción solo se ejecuta si un estado específico está activo.

![](/docs/_remote/39d3afce89ca9fad61d21f61.webp)

2. Cambiar un estado mediante la acción **Establecer estado** .

![](/docs/_remote/6f9bc751de8405306a6f58e0.webp)

3. Responder a los cambios de estado mediante el **Al cambiar el estado** evento del trigger.

Para alternar entre dos acciones, define dos triggers, cada uno con una condición que compruebe un estado. Por ejemplo, las puertas tienen un trigger que activa la acción Open, con una condición que primero comprueba que el estado de la puerta es *Cerrado*, y otro trigger que activa la acción Close, con una condición que comprueba que el estado de la puerta es *Abrir*. Solo uno de los dos se activa cada vez que el player hace clic en la puerta.

![](/docs/_remote/819921eca04a4f69ab10b587.webp)

Puedes añadir tantos estados como quieras a un smart item. Solo haz clic en el **Añadir nuevo estado** botón para añadir otro a la lista.

![](/docs/_remote/fb464b1832beb2b74eb0d3e9.webp)

Uno de los estados se selecciona como predeterminado; el smart item siempre comenzará en este estado cuando la scene se ejecute. Puedes asignar un estado diferente como predeterminado haciendo clic en los tres puntos junto a otro de los estados y seleccionando **Establecer como predeterminado**.


> [!NOTE]
> **💡 Consejo**: Mantén las interacciones entre items sencillas. Por ejemplo, evita escenarios como tener un botón que abra una puerta activando tres acciones: reproducir la animación de la puerta, reproducir el sonido de la puerta y cambiar el estado de la puerta. En su lugar, haz que el botón cambie el estado de la puerta. Luego usa un **Al cambiar el estado** trigger para que la puerta se encargue de reproducir la animación y el sonido cada vez que cambie el estado.


### Counter

Usa la **Counter** component para llevar la cuenta de un número, que puede cambiar a medida que el player realiza acciones en la scene. Puedes usar los valores del contador en la lógica condicional.

Cuando una Entity tiene un Counter component, puedes ejecutar las siguientes acciones sobre ella:

* **Incrementar contador**: Incrementa el valor del contador por una **Cantidad**, por defecto 1.
* **Disminuir contador**: Disminuye el valor del contador por una **Cantidad**, por defecto 1.
* **Establecer contador**: Establece el valor del contador en un número específico, por ejemplo para volver a 0.

Usa la **Al cambiar el contador** trigger para realizar una acción cada vez que cambie el valor del contador. Añade una condición a este trigger para que solo se active después de superar un cierto umbral.

![](/docs/_remote/c4189a0d754c03116088f69e.webp)

En una condición, puedes comprobar si el valor del contador es

* Mayor que un valor dado
* Menor que un valor dado
* Igual a un valor dado


> [!NOTE]
> **💡 Consejo**: Para comprobar si es mayor o igual, puedes añadir dos condiciones al evento del trigger, usando la opción AND.
> 
> Para hacer que una acción ocurra solo una vez al superar un umbral, y que no se repita en cada incremento posterior, combina el contador con un **Estado** component. Establece el estado en "Done" cada vez que alcances el valor deseado, y añade una condición para comprobar este estado en el evento del trigger.


### Ver también

* [Smart items - Conceptos básicos](/docs/creator/content-creator-es/scene-editor/interactividad/smart-items/)
* [Smart items - Advanced](/docs/creator/content-creator-es/scene-editor/interactividad/smart-items-advanced/)
* [Hacer que cualquier objeto sea smart](/docs/creator/content-creator-es/scene-editor/interactividad/make-any-item-smart/)
* [Combinar con código](/docs/creator/content-creator-es/scene-editor/extender-con-codigo/overview/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scene-editor/interactividad/states-and-conditions.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
