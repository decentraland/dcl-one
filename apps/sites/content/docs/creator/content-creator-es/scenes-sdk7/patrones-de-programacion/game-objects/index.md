> Usa el patrón de Game Object para que tu código sea más legible y más fácil de escalar.

# Objetos de juego

A medida que tu escena se vuelve más compleja, resulta útil poner parte de la lógica en archivos separados de game object. Al hacerlo, puedes mantener limpio y fácil de leer el código principal de la escena, mientras encapsulas partes reutilizables que controlan varias entidades en la escena.

Un Game Object contiene todas las propiedades y métodos de un tipo de entidad que podrías encontrar en tu escena, por ejemplo una puerta, un botón o un monstruo. Gracias a esta abstracción, los archivos con la lógica principal de tu escena pueden instanciar game objects completos con solo una línea de código. También puedes llamar a los métodos de estos objetos con la misma facilidad.

Recomendamos mantener las definiciones de game object cada una en un archivo separado.

## El patrón Type Object

Si tu escena usa varios tipos de game objects que tienen mucho en común, quizás quieras llevar esta refactorización aún más lejos y definir un *base* tipo de game object con todo lo que estos tienen en común. Luego puedes crear subclases que hereden de esta clase base.

Puedes leer más sobre este patrón en [Game Programming Patterns](http://www.gameprogrammingpatterns.com/type-object.html).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/patrones-de-programacion/game-objects.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
