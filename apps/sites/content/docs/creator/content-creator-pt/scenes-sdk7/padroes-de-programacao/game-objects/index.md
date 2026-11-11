> Use o padrão de objeto de jogo para tornar seu código mais legível e fácil de escalar.

# Objetos de Jogo

À medida que a sua cena se torna mais complicada, é útil colocar parte da lógica em ficheiros separados de Game Object. Ao fazer isso, pode manter o código principal da cena limpo e simples de ler, ao mesmo tempo que encapsula partes reutilizáveis que controlam várias entities na cena.

Um Game Object contém todas as propriedades e métodos para um tipo de entity que possa encontrar na sua cena, por exemplo uma porta, um botão ou um monstro. Graças a esta abstração, os ficheiros com a lógica principal da sua cena podem instanciar Game Objects completos com apenas uma linha de código. Também pode chamar os métodos destes objetos com a mesma facilidade.

Recomendamos manter as definições de Game Object em ficheiros separados, cada uma num ficheiro diferente.

## O padrão Type Object

Se a sua cena usar vários tipos de Game Objects que tenham muitas coisas em comum, talvez queira levar esta refatoração ainda mais longe e definir um *base* tipo de Game Object com tudo o que estes têm em comum. Depois, pode criar subclasses que herdam desta classe base.

Pode ler mais sobre este padrão em [Game Programming Patterns](http://www.gameprogrammingpatterns.com/type-object.html).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/padroes-de-programacao/game-objects.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
