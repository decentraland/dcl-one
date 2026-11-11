> Detecte se sua scene está sendo executada em mobile, desktop ou web.

# Detectar a plataforma

Use o `isMobile()` função para adaptar sua UI, controles e jogabilidade especificamente para mobile. Esta é a forma recomendada de entregar uma ótima experiência em todos os clientes sem precisar bifurcar completamente a lógica da sua scene.

## Funções disponíveis

```ts
import { getPlatform, isMobile, isDesktop, isWeb } from '@dcl/sdk/platform'
```

* **`getPlatform()`** — retorna a plataforma atual como `'mobile' | 'desktop' | 'web' | null`. Retorna `null` até que o Explorer tenha informado sua plataforma de volta para a scene (isso acontece logo após a scene começar).
* **`isMobile()`** — retorna `true` se o jogador estiver no cliente mobile.
* **`isDesktop()`** — retorna `true` se o jogador estiver no cliente desktop.
* **`isWeb()`** — retorna `true` se o jogador estiver no cliente web.


> [!WARNING]
> **📔 Nota**: Estas funções leem um valor que é preenchido de forma assíncrona quando a scene começa, e nada garante que ele já esteja disponível quando `main()` é executado. Se você chamá-las cedo demais, elas ainda podem retornar `null` / `false`. Para garantir, adie qualquer configuração dependente da plataforma até que `getPlatform()` retorne algo diferente de `null`, por exemplo verificando dentro de um System:
> 
> ```ts
> import { engine } from '@dcl/sdk/ecs'
> import { getPlatform } from '@dcl/sdk/platform'
> 
> function platformCheckSystem() {
>   if (getPlatform() === null) return
>   engine.removeSystem(platformCheckSystem)
>   setupUI()
> }
> 
> engine.addSystem(platformCheckSystem)
> ```
> 


## Separe sua UI por plataforma

Um padrão comum é configurar uma UI diferente para jogadores de mobile e desktop:

```ts
import { isMobile, isDesktop } from '@dcl/sdk/platform'

function setupUI() {
  if (isMobile()) {
    // Botões maiores, layout mais simples para toque
    createMobileUI()
  } else if (isDesktop()) {
    // Layout mais denso, ajustado para teclado e mouse
    createDesktopUI()
  }
}
```

Você também pode usar o mesmo padrão para:

* Mostrar ou ocultar instruções na tela adaptadas a cada método de input.
* Substituir elementos pequenos clicáveis por alvos de toque maiores no mobile.
* Desativar ligações de input que não estão facilmente disponíveis no mobile (veja [Input no mobile](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/input-on-mobile/)).

## Verificando o valor bruto da plataforma

Se você precisar lidar com múltiplas plataformas em um único switch, use `getPlatform()`:

```ts
import { getPlatform } from '@dcl/sdk/platform'

const platform = getPlatform()

switch (platform) {
  case 'mobile':
    // comportamento específico para mobile
    break
  case 'desktop':
    // comportamento específico para desktop
    break
  case 'web':
    // comportamento específico para web
    break
  case null:
    // a plataforma ainda não é conhecida
    break
}
```

## Relacionado

* [Área segura móvel](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/safe-area/)
* [boas práticas de UI para dispositivos móveis](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/ui-best-practices/)
* [UI na tela](/docs/creator/content-creator-pt/scenes-sdk7/ui-2d/onscreen-ui/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/criar-para-mobile/desenvolver/detect-platform.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
