> Obtenha informações de parcelas de land, estates ou endereços usando a CLI.

# Informações do LAND

Você pode usar a Decentraland CLI para consultar informações diretamente dos tokens LAND na Blockchain e dos arquivos da scene enviados para o servidor de conteúdo.

Para executar estes comandos, primeiro você precisa [Instalar a CLI](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/using-the-cli/).

## Obter informações sobre uma scene

A `dcl info` comando retorna o conteúdo de uma scene's *scene.json* arquivo, incluindo proprietário, informações de contato e parcels na scene.

* `dcl info` da pasta da scene retorna informações sobre essa scene específica.
* `dcl info x,y`, onde *x* e *y* são as coordenadas do parcel, retorna informações sobre a scene nesse local.

  {% hint style="warning" %} **📔 Nota**: Não adicione um espaço entre as duas coordenadas. {% endhint %}
* `dcl info id`, onde *id* é um estate id, retorna informações sobre a scene com esse estate id.

A `dcl status` comando retorna uma lista com os arquivos implantados em uma scene. Isso inclui apenas nomes de arquivos e tamanhos. Você não pode acessar o conteúdo dos arquivos via CLI.

* `dcl status` da pasta da scene retorna informações sobre essa scene específica.
* `dcl status x,y`, onde *x* e *y* são coordenadas de parcel (sem espaços entre elas), retorna informações sobre a scene nesse local.


> [!WARNING]
> **📔 Nota**: Não adicione um espaço entre as duas coordenadas.


* `dcl status id`, onde *id* é um estate id, retorna informações sobre a scene com esse estate id.


> [!WARNING]
> **📔 Nota**: Tudo o que é enviado para o nosso content server é público e acessível por essa rede. Quando você implanta uma scene, por padrão você não está enviando o código-fonte original da scene; em vez disso, você envia uma versão compilada para JavaScript minificado, que é muito menos legível.


## Obter informações a partir de um endereço de Wallet

A `dcl info` comando também permite ver informações sobre uma wallet Ethereum e os tokens LAND que ela possui.

* `dcl info xxxx`, onde *xxxx* é o endereço Ethereum em que você tem interesse, retorna uma lista de todos os parcels e estates LAND pertencentes a esse endereço e seus detalhes.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/marketplace/info-from-land.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
