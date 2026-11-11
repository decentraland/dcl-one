> Como você pode depurar sua scene que está sendo executada dentro do Decentraland

# Depuração em Produção

Ao executar uma scene que já foi publicada na LAND no Decentraland, há várias coisas que pode experimentar para a depurar.

## Antes de fazer deploy

### Preview

Antes de fazer deploy da sua scene no Decentraland, certifique-se de que a scene funciona bem em Preview usando a versão mais recente do Decentraland SDK. Veja [depurar no Preview](/docs/creator/content-creator-pt/scenes-sdk7/depuracao/debug-in-preview/).

## Recarregamento rápido

Se precisar de recarregar a scene em que está, escreva o seguinte no chat e prima Enter:

`/reload`

## Sobre o pipeline de publicação

Tenha em conta que, após cada publicação, um processo interno otimiza todos os modelos 3D antes de poderem ser renderizados. Isto normalmente demora apenas alguns segundos, mas pode demorar mais em scenes muito grandes ou quando os servidores estão ocupados. Até terminar, os jogadores recebem a versão anterior da sua scene.

Pode verificar o estado atual deste processo para a sua scene usando [esta ferramenta](https://decentraland.github.io/opscli/). Se a conversão estiver concluída, as três variações dos assets deverão ter luzes verdes.

## Registos e métricas do Multiplayer Server

Para uma scene publicada [Multiplayer Server](/docs/creator/content-creator-pt/scenes-sdk7/rede/authoritative-servers/) scene, utilize [Scene Insights](/docs/creator/content-creator-pt/scenes-sdk7/depuracao/scene-insights/) para pesquisar os últimos 7 dias de registos do servidor, seguir novas linhas em tempo real e inspecionar métricas de CPU e memória. Inclui o início de sessão com wallet e comandos que o utilizador ou o seu assistente de IA podem executar a partir do projeto da scene.

## Registos da scene

Quando usa o Decentraland normalmente, não é possível abrir a consola para verificar mensagens de depuração. Para disponibilizar a consola, tem de abrir o Decentraland com o `scene-console` parâmetro. Depois, pode alternar a consola premindo a tecla backtick no teclado: **\`**. Esta tecla fica à esquerda da tecla 1 na maioria dos teclados em inglês.

Para abrir o Decentraland com o `scene-console` parâmetro, pode fazê-lo de uma destas formas:

* Escreva o seguinte deep link numa janela do navegador: `decentraland://?position=0,0&scene-console=true`. Isto abrirá a aplicação de ambiente de trabalho do Decentraland se a tiver instalada.
* Escreva o seguinte na linha de comando:
  * **macOS**: `open Decentraland.app --args --position 0,0 --scene-console true`
  * **winOS**: `"C:\Users\[YOUR-USER]\Downloads\Decentraland_windows64\Decentraland.exe" --position 0,0 --scene-console true`


> [!NOTE]
> **💡 Dica**: Altere o **position** parâmetro para as coordenadas da sua scene, para carregar diretamente na sua scene.


Ao executar o Decentraland com o `scene-console`, pode abrir a consola de três formas:

* Clique no ![](/docs/_remote/92e8eb247fcf289c2feff8f8.webp) ícone no canto superior direito
* Prima a tecla **\`** para uma consola pequena
* Prima Shift + **\`** para abrir uma vista maior da consola

Tenha em conta que as mensagens de cada scene ativa serão registadas na consola, por isso algumas das coisas que vê na consola podem não ser relevantes para a sua scene.

## Reportar um bug

se encontrar um problema que não esteja na sua scene, mas sim no Decentraland SDK em geral, consulte [Reportar um bug](/docs/creator/content-creator-pt/scenes-sdk7/depuracao/report-bug/).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/depuracao/debug-in-prod.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
