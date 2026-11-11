> Crie scenes multiplayer do Decentraland com um servidor de terceiros.

# Servidores de Terceiros

A abordagem recomendada para interações multiplayer na sua scene é usar o [Multiplayer Server](/docs/creator/content-creator-pt/scenes-sdk7/rede/authoritative-servers/). Este documento abrange outras opções que também são suportadas.

* **REST API + DB**: Bom para dados que mudam com pouca frequência (leaderboards, guestbooks). Os players consultam a API por atualizações; o state persiste entre sessions. Veja [Conexões de rede](/docs/creator/content-creator-pt/scenes-sdk7/rede/network-connections/) para saber como fazer `fetch` requests a partir de uma scene.
* **servidor WebSocket**: Permite comunicação bidirecional em tempo real. Veja [Conexões de rede](/docs/creator/content-creator-pt/scenes-sdk7/rede/network-connections/) para o uso de WebSocket. Bibliotecas como [Colyseus](https://colyseus.io/) funcionam bem com o Decentraland SDK.


> [!WARNING]
> **📔 Nota**: Servidores de terceiros não se integram com `syncEntity`, `validateBeforeChange`, ou `Storage` — você precisará implementar sua própria lógica de gerenciamento de state e sync.


Também é possível optar por uma abordagem híbrida em que as mudanças são notificadas entre players via [serverless-multiplayer](/docs/creator/content-creator-pt/scenes-sdk7/rede/serverless-multiplayer/), mas o state final também é armazenado por meio de uma REST API para visitantes futuros.

Aqui estão alguns exemplos de scenes que usam um servidor de terceiros:

* [Tabela de classificação](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/leader-board)
* [Livro de visitas](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/guest-book-api)
* [Validar autenticidade](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/validate-player-authenticity)

## Testar Localmente

Para pré-visualizar uma scene que usa um servidor de terceiros, você deve executar tanto a scene quanto o servidor do qual ela depende. O servidor pode ser executado localmente na mesma máquina que a preview, como uma maneira mais fácil de testá-lo. Ao executar localmente, o servidor pode usar conexões ws inseguras, para facilitar a configuração. `http` ou `ws` connections, para facilitar a configuração.

Para iniciar o servidor, vá para a `/server` pasta e execute `npm run start`.

Abra a preview em duas janelas separadas; cada janela é tratada como um player separado. Conecte cada janela com um endereço diferente. Ambos os clients se conectarão à mesma instância local do servidor.

Usando o Creator Hub, ative **Multi-Instance Preview** no **Desktop Client** submenu de **Play Options**, depois clique no **Play** botão uma segunda vez para abrir uma segunda janela do Explorer do Decentraland. Você deve se conectar em ambas as janelas com addresses diferentes. As mesmas sessions permanecerão abertas enquanto a scene recarrega.

Como alternativa, você pode abrir uma segunda janela do Explorer do Decentraland escrevendo o seguinte em uma URL do browser:

> `decentraland://realm=http://127.0.0.1:8000&local-scene=true&debug=true&multi-instance=true`

## Realms separadas

Os players em Decentraland existem em muitos *realms*realms separados. Players em realms diferentes não conseguem se ver, interagir nem conversar entre si, mesmo que estejam parados nos mesmos parcels. Dividir os players dessa forma permite que o Decentraland lide com uma quantidade ilimitada de players sem encontrar nenhuma limitação. Também agrupa os players que estão em regiões próximas, para garantir que os tempos de ping entre players que interagem sejam aceitáveis.

Se a sua scene enviar dados para um servidor de terceiros para sincronizar mudanças entre players em tempo real, então é importante que as mudanças sejam sincronizadas apenas entre players que estejam no mesmo realm. Você deve tratar todas as mudanças que pertencem a um realm como separadas das de um realm diferente. Caso contrário, os players verão as coisas mudarem de uma maneira estranha, sem que ninguém tenha feito a mudança.

Veja como obter o realm de cada player em [get player data](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/user-data/)

## Persistência multiplayer

Ao contrário de scenes locais que são montadas novamente cada vez que um player entra nelas, scenes que usam servidores de terceiros têm um tempo de vida que se estende muito além de quando o player entra e sai da scene.

Portanto, você deve projetar a experiência levando em conta que o player nem sempre encontrará a scene no mesmo state inicial. Quaisquer mudanças feitas na scene permanecerão para que outros players as encontrem; você deve garantir que elas não interfiram nas experiências de players futuros de forma indesejada.

### Redefina o state

Ao carregar a scene, certifique-se de que ela seja construída com base nas informações compartilhadas armazenadas no server, e não em um state padrão.

Em alguns casos, faz sentido incluir algum tipo de botão de reset na scene. Pressionar o botão de reset redefiniria a scene de forma tranquila.

Às vezes, isso apenas implica definir as variáveis no state da scene de volta para valores padrão. Mas redefinir a scene também pode envolver cancelar a inscrição de listeners e interromper loops no lado do server. Se loops vazios permanecerem cada vez que a scene for redefinida, eles continuarão se acumulando e terão um efeito ruim no desempenho da scene.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/rede/third-party-servers.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
