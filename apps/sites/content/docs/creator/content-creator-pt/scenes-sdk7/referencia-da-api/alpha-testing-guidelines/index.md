> Diretrizes sobre como experimentar o SDK7 e fornecer feedback

# Testes e Feedback

## Documentação

Encontre a documentação completa do SDK7 na **SDK7** seção da documentação.

## Usando o playground

Para testar facilmente o SDK7, visite [o playground](https://playground.decentraland.org/). Escreva código SDK7 à esquerda da tela e veja a cena resultante à direita. À medida que você altera o código, a cena é atualizada automaticamente.

Se você encontrar algum problema, copie uma URL da configuração atual da sua cena com o **Share** botão, e [relate um problema](#reporting-issues) incluindo esse link para o seu código.

## Usando o SDK 7

Instale o [Creator Hub](/docs/creator/content-creator-pt/scene-editor/primeiros-passos/about-editor/).


> [!WARNING]
> **📔 Nota**: Observe que as cenas que usam SDK7 não têm o `decentraland-ecs` pacote instalado. Em vez disso, elas têm o `@dcl/sdk` pacote.


## Em produção

Você pode implantar cenas com SDK7 por meio do **Publish** botão no Scene Editor no Creator Hub.

## Cenas de exemplo

Você pode encontrar mais cenas de exemplo escritas com SDK7 aqui:

* [Exemplos de cenas](/docs/creator/content-creator-pt/tutoriais-e-exemplos/examples/)

## Relatar problemas

Se você encontrar um problema, consulte [Relatar um bug](/docs/creator/content-creator-pt/scenes-sdk7/depuracao/report-bug/).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/referencia-da-api/alpha-testing-guidelines.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
