> Reacciona a la posición del jugador

# Área de activación

Para hacer que algo suceda cuando el jugador entra o sale de un área, usa un Trigger Area [smart item](/docs/creator/content-creator-es/scene-editor/interactividad/smart-items/).

![](/docs/_remote/9dcff76b0c7616640d8295ac.webp)

El cubo naranja que ves mientras editas tu escena solo es visible en el Scene Editor; se vuelve invisible cuando ejecutas un Preview de la escena. Puedes ajustar y escalar fácilmente el cubo naranja para cubrir exactamente el área que necesitas.

Arrástralo a tu escena y cambia su tamaño con el gizmo de escala. La caja o esfera que ves solo se dibuja en el editor; los jugadores nunca la ven.

Si alguna parte del avatar del jugador se superpone con este cubo naranja, se llamará al evento asignado. Los Trigger Areas solo reaccionan al jugador en la máquina local, no a los avatares de otros jugadores: cada jugador activa el trigger en su propia instancia de la escena.


> [!NOTE]
> **💡 Consejo**: Si los Trigger Areas de tu escena empiezan a estorbar al editar otro contenido, recuerda que siempre puedes bloquearlos y/o ocultarlos desde el [Entity Tree](/docs/creator/content-creator-es/scene-editor/primeros-pasos/scene-editor-essentials/#the-entity-tree).
> 
> <img src="/docs/_remote/c1baca1d0d660705a098e478.webp" alt="" data-size="original">


Varios Trigger Areas pueden superponerse y no se afectan entre sí.

Describes lo que debería suceder y el [Asistente de IA](/docs/creator/content-creator-es/scene-editor/extender-con-codigo/ai-assistant/) escribe un pequeño script para ello.

### Configuración

El componente del ítem **Script** componente tiene dos campos:

| Campo            | Qué hace                                | Opciones                                                                                                                       | Default      |
| ---------------- | --------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ | ------------ |
| **Activado por** | Qué activa el área.                     | `mi jugador`, `todos los jugadores`, `cualquier collider`, `clicable`, `personalizado 1`, `personalizado 2`, `personalizado 3` | `mi jugador` |
| **Forma**        | Si el volumen es una caja o una esfera. | `caja`, `esfera`                                                                                                               | `caja`       |

`mi jugador` reacciona solo al avatar del jugador que ejecuta la escena. `todos los jugadores` reacciona a cualquier avatar, incluidas las demás personas en la escena. Las opciones restantes coinciden con otras [collision layers](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/colliders/), así puedes hacer que el área reaccione a objetos físicos, a los clicables o a tus propias capas personalizadas.

Cambio **Forma** intercambia el placeholder del editor entre una caja y una esfera, así que lo que ves al editar es el volumen que obtienes.

### Describe lo que debería suceder

El área dispara dos eventos: **entrar** y **salir**. En la **Reacciones** sección del componente Script hay un botón para cada uno:

* **Cuando un jugador entra…**
* **Cuando un jugador sale…**

Haz clic en uno y Creator Hub escribe el inicio de una frase en el cuadro de mensaje del AI Assistant, nombrando el ítem, por ejemplo `Cuando un jugador entra en "Trigger Area",` . El prompt usa el nombre del ítem que ves en el entity tree. Completa la frase con lo que quieres que suceda y envíalo. El asistente escribe un script aparte y lo adjunta a la misma entidad, junto con el detector.

Los tres chips junto a los botones (**Reproducir un sonido**, **Mostrar un mensaje**, **Sumar puntos**) completan una frase para ti, así que puedes enviarlos tal cual.


> [!NOTE]
> **💡 Consejo**: Reactions necesitan que el [Asistente de IA](/docs/creator/content-creator-es/scene-editor/extender-con-codigo/ai-assistant/) esté activado. Si no lo está, Creator Hub te indica dónde activarlo en lugar de abrir el chat.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scene-editor/interactividad/trigger-area.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
