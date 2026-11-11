# Exemplos de CLI

Esta prática demonstra como interagir com a content API em um terminal. Vamos usar a instância da Decentraland Foundation em `peer.decentraland.org`, e tudo o que você precisa é do `curl` ferramenta de linha de comando.


> [!NOTE]
> Você pode usar [`httpie`](https://github.com/httpie/httpie) ou formatar respostas com [`jq`](https://github.com/stedolan/jq) para obter JSONs mais legíveis em cada solicitação. Neste guia, mostraremos documentos formatados para maior clareza.


Isto é o que vamos fazer:

1. Consultar o estado do servidor de conteúdo.
2. Localize e baixe um snapshot diário [snapshot](/docs/contributor/contributor-pt/conteudo/snapshots/) com uma lista de entities.
3. Obtenha o manifest de uma Entity.
4. Baixe um dos arquivos da Entity.

Vamos começar consultando o status do server usando `/about`:

```bash
curl "https://peer.decentraland.org/about"
```

```js
{
  "healthy": true,
  "content": {
    "healthy": true,
    "version": "6.5.0",
  },
  // ... mais informações do server (feature flags, versões, paths, etc.)
}
```

Parece que o server está ativo e funcionando normalmente (`"healthy": true`), e está nos fornecendo informações sobre a versão que ele implementa para cada conjunto de funcionalidades, além de algumas opções de configuração da instância.

Estamos interessados em baixar algum conteúdo, então vamos explorar os [snapshots](/docs/contributor/contributor-pt/conteudo/snapshots/) para obter alguns identificadores por meio de `/content/snapshots`.

```bash
curl "https://peer.decentraland.org/content/snapshots"
```

```js
[
  {
    "hash": "bafybeia6qoum64psaooiqo3f45i6hykfwx723uc236waub3gng2naof224",
    "timeRange": {
      "initTimestamp": 1689120000000,
      "endTimestamp": 1689206400000
    },
    "replacedSnapshotHashes": [],
    "numberOfEntities": 981,
    "generationTimestamp": 1689219353866
  },
  // ...mais arquivos snapshot
]
```

Cada item do array descreve um [snapshot](/docs/contributor/contributor-pt/conteudo/snapshots/). Vamos pegar um `hash` e baixar o arquivo do `/content/contents` endpoint.

```bash
curl "https://peer.decentraland.org/content/contents/bafybeia6qoum64psaooiqo3f45i6hykfwx723uc236waub3gng2naof224" > snapshot
```


> [!NOTE]
> Você pode experimentar snapshots maiores, como no [exemplo avançado em Python](https://github.com/decentraland/documentation/blob/main/content/contributor/content/practice/snapshots.py). Para testar isso interativamente, você provavelmente vai querer um dos menores.


Podemos verificar se baixamos o arquivo certo em um formato que conhecemos, olhando a primeira linha:

```bash
head -n1 snapshot
```

```
### snapshot JSON da Decentraland
```

Ótimo! Agora temos um resumo local de todas as Entities que foram capturadas nesse snapshot. Vamos pegar o primeiro listado (a segunda linha no arquivo), um `perfil`:

```bash
tail -n+2 snapshot | head -n1 
```

```js
{
  "entityId": "bafkreif7hjremkxlvixyxoxnoo7bdcnf7qqp245sjb2pag2nk3n6o6yc4c",
  "entityType": "profile",
  "pointers": [
    "0x271cdb3b1c792c336c4b2bdc52c4f415d0046b92"
  ],
  "authChain": [
    // Veja https://docs.decentraland.org/contributor/auth/authchain/
  ],
  "entityTimestamp": 1689120135624
}
```


> [!NOTE]
> Como snapshots expiram e Entities são substituídas, os identificadores neste artigo não vão funcionar. Acompanhe no seu terminal para obter IDs de arquivos reais e ativos.


Estas são informações que poderíamos salvar. Vamos usar o `entityId` para baixar o manifest JSON da Entity, mas persistir os [pointer](/docs/contributor/contributor-pt/conteudo/pointers/) é uma boa ideia se quisermos localizar esta Entity e quaisquer versões atualizadas no futuro.

Também temos o [cadeia de autenticação](/docs/contributor/contributor-pt/conteudo/entities/#ownership) usado para assinar esta Entity, e poderíamos validar as assinaturas listadas para verificar a autenticidade de quaisquer arquivos relacionados que baixarmos.

Vamos obter o manifest da Entity. Lembre-se, o `entityId` é o [identificador do arquivo](/docs/contributor/contributor-pt/conteudo/file-system/#identifiers) de que precisamos, e podemos usar o `/content/contents` endpoint novamente:

```bash
curl "https://peer.decentraland.org/content/contents/bafkreigcreq7rv6b2wf4zc4fsnif43ziwb4q46v4qhsewpf7gbsyxew3om"
```

```js
{
  "version": "v3",
  "type": "profile",
  "pointers": [
    "0x273cdb3b1c791c336c4b2bcc52c4f415d0046b91"
  ],
  "timestamp": 1689120135624,
  "content": [
    {
      "file": "body.png",
      "hash": "bafybeibzaqkirz7fk474xvyhurho5xviphs7anawaceb6gscuigia4x33u"
    },
    // ...mais arquivos
  ],
  "metadata": {
    // ...avatars e outras informações
  }
}
```

Esta `perfil` Entity tem todas as informações que World Explorers usam para renderizar e animar um player. Se estivermos interessados em obter um dos arquivos empacotados, podemos continuar a usar o `/content/contents` endpoint.

Se olharmos para o `content` campo, podemos ver o `hash` do arquivo internamente chamado `body.png`. Vamos obtê-lo:

```bash
curl "https://peer.decentraland.org/content/contents/bafybeibzaqkirz7fk474xvyhurho5xviphs7anawaceb6gscuigia4x33u" > body.png
```

Podemos abrir este `png` arquivo em um visualizador de imagens ou navegador web, e conferir o trabalho do autor. Legal!


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/conteudo/pratica/cli-examples.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
