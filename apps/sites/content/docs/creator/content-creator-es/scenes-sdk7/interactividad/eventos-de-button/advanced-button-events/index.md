> Aprende cómo manejar los clics de los usuarios en tu escena.

# Eventos avanzados de Button

Si necesitas que la interacción en tu scene siga una lógica personalizada que no sea compatible con el [Registrar callback](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/register-callback/) o el [Basado en System](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/system-based-events/) enfoques, puedes tratar directamente con los datos brutos del evento de entrada. Este enfoque es el más difícil, pero también el más flexible.

## El componente PointerEventsResult

Cuando el engine detecta un evento de pointer, a la entidad en la que se hizo clic se le asigna un `PointerEventsResult` componente. Este componente contiene todos los datos brutos sobre el evento de impacto y almacena datos históricos sobre eventos anteriores.

Las funciones auxiliares en `inputSystem`, como `inputSystem.isTriggered` o `inputSystem.getInputCommand` son buenas para la mayoría de los escenarios simples, pero si necesitas obtener más detalles sobre el evento de impacto, revisa los datos brutos en el `PointerEventsResult`.

El `PointerEventsResult` componente contiene una lista de pointer events, con un objeto por cada evento. Almacena una lista de hasta 100 eventos; los eventos más nuevos se guardan al final de la lista. Cuando la lista alcanza una longitud de 100, comienza a descartar los eventos antiguos por cada nuevo que llega.

Cada evento de la lista tiene los siguientes datos:

* `analog`: Número opcional, solo presente en eventos procedentes de una entrada analógica (como un joystick), que almacena el valor analógico de la entrada.
* `button`: Qué id de botón se presionó. El número corresponde al `InputAction` enum, que enumera todos los botones disponibles.
* `state`: Tipo de evento de pointer, de la enum `PointerEventType`. *0* se refiere a `PointerEventType.PET_UP`, *1* en `PointerEventType.PET_DOWN`, *2* en `PointerEventType.PET_HOVER_ENTER`, *3* en `PointerEventType.PET_HOVER_LEAVE`, *4* en `PointerEventType.PET_PROXIMITY_ENTER`, *5* en `PointerEventType.PET_PROXIMITY_LEAVE`
* `timestamp`: Un [marca de tiempo de Lamport](https://en.wikipedia.org/wiki/Lamport_timestamp) para identificar cada evento de botón.

  > Nota: esta marca de tiempo no se numera en función del tiempo actual. Piensa en ella como un contador que empieza en 0 y se incrementa en 1 por cada evento.
* `impacto`: Un objeto que contiene los siguientes datos sobre el evento de impacto:
  * `entityId`: Número de Id de la entity que fue golpeada por el rayo.
  * `meshName`: *String* con el nombre interno del mesh específico del modelo 3D que fue golpeado. Esto es útil cuando un modelo 3D está compuesto por múltiples meshes.
  * `globalOrigin`: *Vector3* para la posición desde donde se origina el rayo (relativa a la escena)
  * `dirección`: *Vector3* con el vector de dirección del rayo, en coordenadas globales
  * `position`: *Vector3* para la posición donde el rayo intersectó con la entity golpeada (relativa a la escena)
  * `length`: Longitud del rayo desde su origen hasta la posición donde ocurrió el golpe contra la entity.
  * `normalHit`: *Vector3* con un vector de dirección normalizado, que describe el ángulo de la normal del impacto en el espacio global.

```ts
import { engine, PointerEventsResult } from '@dcl/sdk/ecs'

function PointerReadingSystem() {
  for (const [entity] of engine.getEntitiesWith(PointerEventsResult)) {
    const events = PointerEventsResult.get(entity)
    for (const event of events) {
      console.log('POINTER EVENT DATA:', event)
    }
  }
}

engine.addSystem(PointerReadingSystem)
```

## Rastrea los movimientos del jugador

En juegos multijugador en tiempo real donde el momento de los movimientos del jugador es crítico, quizá quieras hacer un seguimiento de la posición de cada jugador usando un servidor de terceros como fuente de verdad. Puedes mejorar el tiempo de respuesta escuchando el botón con antelación y prediciendo sus efectos en tu servidor antes de que el avatar haya cambiado de posición.

Este enfoque ayuda a compensar los retrasos de red, pero seguramente dará lugar a discrepancias, así que también deberías consultar regularmente la posición actual del jugador para hacer correcciones. Equilibrar estas predicciones y correcciones puede requerir mucho ajuste fino.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/advanced-button-events.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
