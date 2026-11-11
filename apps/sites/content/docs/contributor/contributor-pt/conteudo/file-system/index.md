# Sistema de ficheiros

Subjacente às diferentes APIs de conteúdo no protocolo Decentraland está uma implementação de um sistema de arquivos distribuído. Cada content server mantém uma cópia de todo o armazenamento e sincroniza as atualizações que recebe com outras instâncias.

Os arquivos enviados para o sistema são **imutáveis**. Uma vez implantados, nem o identificador nem o conteúdo deles mudam. Eles só podem ser atualizados no sentido de que uma nova versão é enviada e as aplicações escolhem usá-la, enquanto a antiga pode ser descartada.

Você pode experimentar arquivos na [prática](https://github.com/decentraland/docs/tree/main/contributor/content/practice/README.md) seção.

### Identificadores de arquivos <a href="#identifiers" id="identifiers"></a>

O sistema de arquivos do Decentraland é organizado em um índice plano (ou seja, não há uma hierarquia natural de diretórios), em que cada arquivo é identificado por uma string única.

As strings de identificador são prefixadas [base32](https://en.wikipedia.org/wiki/Base32) codificações do hash SHA-256 do conteúdo do arquivo, usando o [IPFS CID v1](https://docs.ipfs.tech/concepts/content-addressing/) algoritmo. Elas se parecem com isto:

```
bafybeicgclohdfaccu2sqqkzrzuenjxzcry3m5vcb4mpxgucjl3oheq5tq
```

O prefixo nos primeiros bytes indica a codificação, a versão e o tipo de hash do próprio identificador.


> [!NOTE]
> Por razões históricas, algumas APIs chamam o identificador do arquivo de *hash* em vez de *id*, mas eles se referem à mesma coisa. Você também pode encontrar identificadores legados em conteúdos antigos que parecem um pouco diferentes.


Na prática, esses detalhes não são necessários para descobrir e baixar conteúdo. A maioria dos clientes do protocolo pode tratar identificadores como strings opacas sem perder capacidades.

Observe que os identificadores CIDv1 modernos são compatíveis com IPFS, gerando o mesmo CID para o mesmo arquivo, então o IPFS pode ser usado como o provedor de arquivos.

### Propriedade e Persistência

Exceto por [snapshots](/docs/contributor/contributor-pt/conteudo/snapshots/), os arquivos armazenados na rede estão associados a uma [entity](/docs/contributor/contributor-pt/conteudo/entities/) de propriedade de uma conta Ethereum. O proprietário é o único autorizado a atualizar a entity e os arquivos relacionados.

Os content servers são obrigados pelo protocolo a sempre armazenar a versão mais recente de uma entity e de seus arquivos, mas podem escolher se mantêm as versões antigas de acordo com sua configuração individual.

### Baixando Arquivos <a href="#downloading" id="downloading"></a>

O [`/contents/<fileId>`](https://decentraland.github.io/catalyst-api-specs/#tag/Content-Server/operation/getContentFile) endpoint do content server pode ser usado para baixar qualquer arquivo. Seus identificadores são encontrados no manifest da entity pai. [da entity](/docs/contributor/contributor-pt/conteudo/entities/)manifest.

### Enviando Arquivos

Os arquivos não são enviados de forma independente. Em vez disso, eles são agrupados dentro de entities e enviados durante o deploy da entity para o content server.

Veja a [documentação para criadores de conteúdo](https://github.com/decentraland/docs/tree/main/creator/README.md) para mais detalhes.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/conteudo/file-system.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
