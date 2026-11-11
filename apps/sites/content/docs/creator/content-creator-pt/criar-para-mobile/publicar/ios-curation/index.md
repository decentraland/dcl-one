> Como funciona a curadoria de conteúdo do iOS e como enviar sua scene ou World para revisão.

# Curadoria do iOS

Para cumprir [as Diretrizes de Revisão da App Store da Apple sobre Conteúdo Gerado pelo Utilizador (UGC)](https://developer.apple.com/app-store/review/guidelines/#user-generated-content), a Decentraland introduziu um **processo de revisão manual** para todas as cenas e Worlds que aparecem na **secção Discover da app iOS**.


> [!NOTE]
> Este processo de revisão aplica-se apenas ao iOS. A secção Discover do Android e a pesquisa continuam a mostrar todo o conteúdo publicado sem restrições.


## Como funciona

* Todo o conteúdo deve ser revisto manualmente antes de aparecer na secção Discover da app iOS.
* As Cenas e Worlds que não forem aprovados ficarão ocultos da secção Discover do iOS.
* Os Eventos associados a Cenas ou Worlds não aprovados também ficarão ocultos na secção Discover do iOS.
* **Todas as cenas continuam acessíveis** no iOS a qualquer momento através de deep linking ou do `/goto` e `/world` comandos de chat — a curadoria apenas afeta a visibilidade no Discover.

## Envie a sua cena ou world para revisão no iOS

Para solicitar que a sua cena ou world seja revistada para inclusão na secção Discover do iOS, submeta um **Pedido de Revisão de Conteúdo** no servidor Decentraland Discord:

1. Junte-se ao [Decentraland Discord](https://dcl.gg/discord).
2. Vá para o **#rl-mobile-curation** canal.
3. Publique o seu pedido incluindo:
   * Nome da cena ou world
   * Coordenadas ou nome do world (por exemplo, `0,0` ou `MyWorld.dcl.eth`)
   * Uma breve descrição do conteúdo

As submissões são revistas no prazo de **24 horas**. Será notificado no canal assim que o seu conteúdo for aprovado ou se for necessária alguma ação de seguimento.

## Relacionado

* [Destaque-se](/docs/creator/content-creator-pt/criar-para-mobile/publicar/get-featured/)
* [boas práticas de UI para dispositivos móveis](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/ui-best-practices/)
* [Pré-visualizar em mobile](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/preview-on-mobile/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/criar-para-mobile/publicar/ios-curation.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
