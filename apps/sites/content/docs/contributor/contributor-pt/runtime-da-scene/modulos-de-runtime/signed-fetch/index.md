# Fetch assinado

O `SignedFetch` o módulo fornece uma implementação da `fetch` interface que é transparentemente compatível com o [signed fetch](/docs/contributor/contributor-pt/autenticacao/signed-fetch/) protocolo, que define como anexar cadeias de [autenticação assinadas](/docs/contributor/contributor-pt/autenticacao/authchain/) a requisições de saída.

Os procedimentos para assinar e verificar esta requisição estão detalhados na [cadeia de autenticação](/docs/contributor/contributor-pt/autenticacao/authchain/) página.

Usando `SignedFetch` a partir de uma scene requer o [`USE_FETCH`](/docs/contributor/contributor-pt/conteudo/tipos-de-entity/scenes/#permissions) permissão.

### Métodos

**`signedFetch`**

Faça uma requisição HTTP como faria com ([`fetch`](/docs/contributor/contributor-pt/runtime-da-scene/globals/#http)), adicionando automaticamente os cabeçalhos de verificação.

```ts
interface Request {
  // A URL de destino da requisição:
  url: string;

  // Parâmetros opcionais, autoexplicativos, para a requisição:
  init?: {
    method?: string;
    body?: string;
    headers: { [key: string]: string };
  };
}

interface Response {
  // Se a requisição HTTP foi executada com sucesso (códigos diferentes de 2xx não são falhas)
  ok: boolean;

  // Os detalhes autoexplicativos da resposta:
  status: number;
  statusText: string;
  headers: { [key: string]: string };
  body: string;
}

function signedFetch(Request): Promise<Response>;
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/runtime-da-scene/modulos-de-runtime/signed-fetch.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
