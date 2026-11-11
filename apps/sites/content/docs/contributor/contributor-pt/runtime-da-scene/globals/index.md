# Globais

O runtime da scene garante a existência de certos objetos e funções dentro do ambiente JavaScript em sandbox, além das utilidades padrão, como `Promise`, `Date` ou `Math`.


> [!NOTE]
> Os globais descritos abaixo são conceitos típicos de JavaScript, mas foram adaptados ao runtime da scene da Decentraland e podem não se comportar de forma idêntica aos seus equivalentes em ambientes de navegador ou Node.


### Globais

O runtime injeta 6 definições no escopo global da scene:

1. [`console`](#console): uma versão simplificada do típico `console` objeto.
2. [`exports`](#module): um objeto no qual a scene pode adicionar sua [interface pública](/docs/contributor/contributor-pt/runtime-da-scene/execution/).
3. [`module`](#module): um contêiner para o `exports` objeto.
4. [`require`](#module): uma função para carregar módulos fornecidos pelo runtime pelo nome.
5. [`fetch`](#http): uma implementação restrita da `fetch` função.
6. [`WebSocket`](#http): uma implementação restrita da `WebSocket` classe.

Todas elas são definidas como propriedades somente leitura, então não podem ser reatribuídas. Algumas lançarão exceções quando usadas, a menos que certas [permissões](/docs/contributor/contributor-pt/conteudo/tipos-de-entity/scenes/#permissions) sejam concedidas à scene.

### Console <a href="#console" id="console"></a>

As scenes têm acesso a um `console` objeto, muito parecido com o fornecido por um ambiente de navegador ou Node, embora limitado apenas a alguns dos métodos que você normalmente encontraria.

```ts
type Console = {
  log(...args: any): void
  info(...args: any): void
  debug(...args: any): void
  warning(...args: any): void
  error(...args: any): void
}
```

Assim como suas equivalentes padrão, cada método aceita argumentos variáveis de qualquer tipo e os renderiza como mensagens legíveis por humanos. Por exemplo, isso é válido:

```js
console.log("The thing just appeared", { thing: "foo" }, [1, 2, 3])
```

O comportamento exato desses métodos depende do provider, mas as mensagens devem estar acessíveis aos desenvolvedores da scene que estiverem depurando seu código.


> [!NOTE]
> No World Explorer baseado em navegador da Foundation, as mensagens de log aparecem no painel de ferramentas do desenvolvedor.


### Imports e Exports <a href="#module" id="module"></a>

As scenes podem importar e exportar objetos usando o tradicional [CommonJS](https://wiki.commonjs.org/wiki/Modules/1.0) interface de módulo.

```ts
exports: Object
require(moduleName: string): Object
```

O `require` função permite que as scenes acessem módulos fornecidos pelo runtime (como [EngineApi](/docs/contributor/contributor-pt/runtime-da-scene/modulos-de-runtime/engine-api/) ou [RestrictedActions](/docs/contributor/contributor-pt/runtime-da-scene/modulos-de-runtime/restricted-actions/)), e nada mais (ele **não** acessa pacotes NPM ou módulos por caminho).

As propriedades adicionadas ao `exports` objeto são a interface pública da scene e serão expostas ao runtime. Na verdade, as scenes *devem* expor pelo menos um método para funcionar corretamente (veja [execução](/docs/contributor/contributor-pt/runtime-da-scene/execution/)).


> [!NOTE]
> As scenes escritas em linguagens como TypeScript usam a forma mais moderna `import` e `export` declarações, que podem ser transpiladas em usos compatíveis com CommonJS de `require` e `exports`.


### HTTP e WebSockets <a href="#http" id="http"></a>

O `fetch` e `WebSocket` globais funcionam exatamente como seus equivalentes conhecidos (veja [fetch](https://developer.mozilla.org/en-US/docs/Web/API/Fetch_API) e [WebSocket](https://developer.mozilla.org/en-US/docs/Web/API/WebSockets_API) no MDN), mas com algumas restrições impostas pelo runtime.

Ao chamar a `fetch` função:

* Um erro é lançado se a URL não começar com `https://`.
* Um erro é lançado se a scene não tiver o [`USE_FETCH` permissão](/docs/contributor/contributor-pt/conteudo/tipos-de-entity/scenes/#permissions).
* Um timeout definido pela implementação pode abortar a requisição.

Ao usar a `WebSocket` classe:

* Um erro é lançado se a URL não começar com `wss:`
* Um erro é lançado se a scene não tiver o [`USE_WEBSOCKET` permissão](/docs/contributor/contributor-pt/conteudo/tipos-de-entity/scenes/#permissions).

Além dessas diferenças, ambos os casos seguem o comportamento padrão.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/runtime-da-scene/globals.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
