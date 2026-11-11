> Recopila valoraciones y comentarios de los jugadores durante el juego o mediante un botón de feedback, un kiosco o un área, con resultados CSV en el Storage de la escena.

# Feedback de la escena

<figure><img src="/docs/_remote/a0baa00d823b7604d1c33550.webp" alt="Coin Hunt participation invitation with Skip and Give feedback buttons" width="400"><figcaption><p>La demo invita a los jugadores a elegir si quieren participar.</p></figcaption></figure>

Aprende qué les resulta confuso, frustrante o agradable a los jugadores, incluso cuando no puedes verlos jugar. Este módulo reutilizable incluye una escena de búsqueda de monedas que muestra cómo recopilar comentarios durante el juego y cuando los jugadores eligen darlos.

* [Jugar la compilación de prueba compartida (Zone)](https://decentraland.zone/jump/?dclenv=zone&realm=sdk7testscenes.dcl.eth&position=0,-1)
* [Guía de instalación y uso](https://github.com/decentraland/sdk7-test-scenes/blob/main/scenes/0,-1-scene-feedback/README.md)

## Información del proyecto

**Mantenido por:** Decentraland

[Ver código fuente](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,-1-scene-feedback)

**Licencia:** No identificada en este catálogo. Consulte el repositorio original y al mantenedor para conocer los términos aplicables antes de reutilizarlo.

## Añádelo con un agente de IA

1. Asegúrate de [Decentraland SDK Skills](https://github.com/decentraland/sdk-skills#updating-skills) están instalados y actualizados.
2. Dile a tu agente de IA: "Quiero recopilar comentarios de los jugadores en esta escena."

El agente puede usar la habilidad Scene Feedback para ayudar en dos etapas:

* **Antes de una prueba de juego:** adapta las preguntas, las escalas de valoración y la temporización a tu juego, y luego integra el módulo después de revisar el plan.
* **Después de una prueba de juego:** vuelve a usar la habilidad con tus archivos CSV para identificar problemas y preferencias recurrentes, revisa las evidencias y decide qué investigar después. También puedes pedir un [informe HTML legible por humanos](#example-ai-generated-feedback-report) con respuestas agrupadas y resúmenes de valoración.

¿Prefieres integrarlo tú mismo? Sigue la [guía de instalación](https://github.com/decentraland/sdk7-test-scenes/blob/main/scenes/0,-1-scene-feedback/README.md#add-it-to-your-scene).

## Cómo encaja el módulo en tu escena

Tú eliges cuándo invitar a los jugadores, qué preguntas hacer y cuándo aparecen esas preguntas en tu juego. La [demo de búsqueda de monedas en Zone](https://decentraland.zone/jump/?dclenv=zone&realm=sdk7testscenes.dcl.eth&position=0,-1) muestra una posible configuración:

1. **Recoge la primera moneda.** Una invitación pregunta si el jugador quiere dar feedback: "¡Hola, somos el equipo de Coin Hunt!" Si acepta, le sigue una pregunta: "¿Qué tan fácil o difícil fue ver las monedas?" El jugador elige una valoración y puede añadir un comentario.
2. **Completa la primera ronda (cinco monedas).** Los jugadores que aceptaron participar reciben dos preguntas, una tras otra: qué tan claro es el siguiente objetivo y si quieren seguir jugando.
3. **Da feedback en cualquier momento.** Cualquier visitante puede usar el **Dejar feedback** botón, quiosco clicable o área marcada para abrir un mensaje de bienvenida seguido de un conjunto de preguntas, incluso después de rechazar las preguntas de juego.

Elige cómo participan los jugadores:

* **Jugadores que aceptan participar:** invita a los jugadores con un aviso en pantalla, o permite que abran un formulario de feedback mediante un botón, quiosco o área marcada.
* **Probadores seleccionados:** usa la lógica de tu juego para elegir probadores por lista de Wallet u otra regla, y luego inscríbelos con `feedback.enroll`. Consulta el [ejemplo de probador seleccionado](https://github.com/decentraland/sdk7-test-scenes/blob/main/scenes/0,-1-scene-feedback/EXAMPLES.md#4-selected-testers-without-an-intro).
* **O pregúntale a cualquiera:** muestra preguntas a los visitantes durante el juego sin una invitación previa ni una lista de testers. Los jugadores aún pueden rechazarlo. Consulta la [configuración de participación](https://github.com/decentraland/sdk7-test-scenes/blob/main/scenes/0,-1-scene-feedback/README.md#asking-participants-during-play-dynamic-approach).

<figure><img src="/docs/_remote/c600bed5d83d04aa544fb1e0.webp" alt="Example feedback form with five rating choices and an optional comment field" width="400"><figcaption><p>Una pregunta con una valoración etiquetada y un comentario opcional.</p></figcaption></figure>

## Características clave

* **Preguntas flexibles.** Elige preguntas y escalas de valoración predefinidas, adáptalas a tu juego o crea las tuyas propias. Usa valoraciones etiquetadas, comentarios opcionales o respuestas solo de texto.
* **Recompensas por participar.** Fomenta la participación con una recompensa que se ajuste a tu juego. En la demo, responder todas las preguntas de un formulario otorga una placa Playtest Contributor visible solo para ti durante la visita, +5 puntos de moneda y puntuación doble para las siguientes cinco recogidas, una vez por visita. Cualquier valoración cuenta; los comentarios siguen siendo opcionales.
* **Resultados CSV en el Storage de la escena.** Un servidor multijugador guarda cada respuesta junto con su pregunta y el momento de juego. Descarga los archivos para revisarlos en una hoja de cálculo.

## Adáptalo a tu escena

Usa la [recetas de integración](https://github.com/decentraland/sdk7-test-scenes/blob/main/scenes/0,-1-scene-feedback/EXAMPLES.md) para conectar las preguntas con los eventos de tu propio juego, incluyendo invitar a los participantes ahora y preguntar después de fases de juego posteriores. Para planificar la prueba de juego y encontrar jugadores, consulta [Pruebas de juego](/docs/creator/sdk7/design-experience/playtests/).

## Ejemplo de informe de feedback generado por IA

**El informe lo crea un agente de IA, no el módulo Scene Feedback.** Dale a tu agente los archivos CSV exportados y usa la habilidad Scene Feedback del SDK para analizarlos y generar un informe HTML.

Este ejemplo utiliza respuestas sintéticas de una escena ficticia. Tu agente puede adaptar el análisis y el informe a las preguntas y los datos de tu propia prueba de juego.

<figure><img src="/docs/_remote/ad88dfb78877894462538f58.webp" alt="Synthetic racing-scene report showing a main finding, response counts and a rating distribution" width="600"><figcaption><p>Informe ilustrativo que utiliza respuestas sintéticas de una escena ficticia.</p></figcaption></figure>

{% file src="/files/a2bb546348442bad9755ae1cae68bb5370f10351" %}
Descarga el informe de ejemplo, descomprime el ZIP y abre el archivo HTML en tu navegador.
{% endfile %}


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/tutoriales-y-ejemplos/scene-management-feedback/playtest-feedback.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
