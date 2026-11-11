# Gestão de dependências

Este documento define as práticas recomendadas para gerenciar dependências em projetos JavaScript/TypeScript em todo o ecossistema da Decentraland. Aplica-se a front-end, back-end, SDKs, bibliotecas compartilhadas e qualquer pacote baseado em npm/yarn/pnpm.

> **Resumo**:
>
> * **Estratégia de versões** → Use **versões exatas/fixas** para `dependências` (segurança) e `devDependencies` (ambiente de desenvolvimento consistente). Use **intervalos de versão** somente para `peerDependencies` (flexibilidade).
> * **Exceção** → Pacotes da Decentraland (`@dcl/*`, `decentraland-*`) podem usar intervalos de versão (`^`) pois são pacotes internos confiáveis.
> * **Bibliotecas/pacotes compartilhados** → Use `peerDependencies` com intervalos de versão (^) para bibliotecas compartilhadas (React, @dcl/schemas etc.). Use `dependências` com versões exatas apenas para utilitários seguros para duplicar.
> * **Apps/serviços finais** → Use `dependências` com versões exatas para pacotes de runtime (React, ethers etc.).

## 1. Justificativa

Muitas bibliotecas dependem de singletons globais, React Context, identidade de classe, caches compartilhados, enums de schema ou pools de DB compartilhados. Instalar múltiplas versões da mesma biblioteca pode causar:

* Context não compartilhado entre cópias
* Caches ou pools duplicados
* Falhas nas verificações instanceof
* Incompatibilidades de enum/símbolo
* Aumento do tamanho do bundle
* Bugs de runtime difíceis de diagnosticar

O gerenciamento correto de dependências evita esses problemas.

### Não objetivos

Este padrão não **não** tenta:

* Impor um único gerenciador de pacotes (npm, yarn e pnpm são todos suportados)
* Garantir uma única cópia física em `node_modules` (o foco é na deduplicação em runtime/bundle)

## 2. Definições

| Campo                  | Propósito                                                                | Responsabilidade do consumidor                                  | Aplica-se a             |
| ---------------------- | ------------------------------------------------------------------------ | --------------------------------------------------------------- | ----------------------- |
| `dependências`         | Pacotes usados internamente pelo módulo                                  | Nenhum - não se espera que o consumidor os forneça              | Apps e bibliotecas      |
| `peerDependencies`     | Pacotes que devem resolver para uma única versão efetiva em runtime      | Devem instalá-los (ou usar peerDependenciesMeta para opcionais) | **Somente bibliotecas** |
| `peerDependenciesMeta` | Metadados para peerDependencies, usados para marcar peers como opcionais | Nenhum - afeta apenas o comportamento do gerenciador de pacotes | **Somente bibliotecas** |
| `devDependencies`      | Ferramentas usadas apenas durante o desenvolvimento                      | Nenhum                                                          | Apps e bibliotecas      |

### Sobre peerDependenciesMeta

`peerDependenciesMeta` é um campo que fornece metadados sobre os seus `peerDependencies`. O caso de uso mais comum é marcar peers como **opcionais**:

* **Peers obrigatórios** (padrão): o gerenciador de pacotes emitirá um aviso se não estiver instalado
* **Peers opcionais**: o gerenciador de pacotes não emitirá aviso se estiverem ausentes; seu package deve lidar com a ausência deles com elegância

Isso é útil para pacotes que podem funcionar com ou sem certas dependências (por exemplo, um utilitário que funciona com ou sem React, ou uma biblioteca que oferece suporte a múltiplos provedores Web3).

### Apps vs Bibliotecas

**Bibliotecas / Pacotes compartilhados:**

* Use `peerDependencies` para pacotes que devem resolver para uma única versão efetiva em runtime (React, ethers, @dcl/schemas)
* O consumidor (app) fornece essas dependências
* Evita instalações duplicadas e conflitos de singleton

**Apps / Serviços finais:**

* Frequentemente usam `dependências` para pacotes de runtime (React, ethers etc.)
* Eles são o consumidor final, então a duplicação não é uma preocupação
* `peerDependencies` ainda é válido se o app puder ser consumido como dependência

## 3. Quando Usar Cada Campo

> **Referência rápida**: Veja [Seção 4](#4-examples) para pacotes comuns e sua colocação recomendada.

### Use peerDependencies para (somente bibliotecas):

**Em bibliotecas/pacotes compartilhados**, use `peerDependencies` para pacotes que devem resolver para uma única versão efetiva em runtime:

* React, Redux, wagmi, ethers, viem
* @dcl/schemas e bibliotecas semelhantes entre ecossistemas
* decentraland/connect
* drivers de DB ao compartilhar pools
* Qualquer biblioteca que dependa de singletons ou contexto

✅ Correto (Biblioteca):

```json
{
  "peerDependencies": {
    "react": "^18.0.0",
    "@dcl/schemas": "^20.0.0"
  }
}
```

❌ Incorreto (Biblioteca usando dependencies para bibliotecas compartilhadas):

```json
{
  "dependencies": {
    "react": "^18.0.0"
  }
}
```

### Use dependencies para:

* Utilitários seguros para duplicar (lodash-es, date-fns)
* **Em apps**: pacotes de runtime como React, ethers (quando o app é o consumidor final)

> **Importante**: use sempre **versões exatas/fixas** em `dependências` para segurança.

✅ Correto (Biblioteca):

```json
{
  "dependencies": {
    "lodash-es": "4.17.21",
    "date-fns": "3.6.0"
  }
}
```

✅ Correto (App):

```json
{
  "dependencies": {
    "react": "18.3.1",
    "ethers": "6.13.0",
    "lodash-es": "4.17.21"
  }
}
```

### peerDependencies opcionais (peerDependenciesMeta)

Para pacotes reutilizáveis que funcionam com ou sem certas dependências, use `peerDependenciesMeta` para marcar peers como opcionais:

✅ Correto:

```json
{
  "peerDependencies": {
    "react": "^18.0.0",
    "ethers": "^6.0.0"
  },
  "peerDependenciesMeta": {
    "ethers": {
      "optional": true
    }
  }
}
```

**O que isso significa:**

* `react` estiver em **obrigatório**: o consumidor deve instalá-lo ou o gerenciador de pacotes emitirá um aviso
* `ethers` estiver em **opcionais**: o consumidor não precisa instalá-lo; seu package deve verificar sua presença antes de usá-lo

**Casos de uso:**

* Pacotes que funcionam tanto em ambientes React quanto não React
* Bibliotecas que oferecem suporte a múltiplos provedores Web3 (ethers, viem etc.)
* Utilitários que aprimoram outras bibliotecas, mas não são necessários

### Use devDependencies para:

* Ferramentas (TypeScript, ESLint, testadores, bundlers)

> **Importante**: use sempre **versões exatas/fixas** em `devDependencies` para garantir um ambiente de desenvolvimento consistente em toda a equipe.

✅ Correto:

```json
{
  "devDependencies": {
    "typescript": "5.4.5",
    "eslint": "8.57.0",
    "vitest": "1.6.0"
  }
}
```

## 4. Exemplos

### Pacotes que DEVEM SER peerDependencies (contexto compartilhado / singletons)

* `react`, `react-dom`, `react-redux`, `react-router-dom`
* `redux`, `@reduxjs/toolkit`
* `ethers`, `viem`, `wagmi`
* `@dcl/schemas`, `@dcl/ui-env`, `@dcl/crypto`
* `decentraland-dapps`, `decentraland-ui`, `decentraland-ui2`, `decentraland-connect`
* `pg`, `pg-pool`

### Pacotes que DEVEM SER dependencies (seguros para duplicar)

* `lodash-es`, `date-fns`
* `uuid`, `nanoid`
* `zod`, `ajv`
* `ms`, `mitt`, `fp-future`

## 5. Padrões relacionados

* [Padrões de teste](/docs/contributor/contributor-pt/guias-para-colaboradores/testing-standards/)
* [Padrões de UI](https://github.com/decentraland/docs/tree/main/contributor/contributor-guides/ui-standards/README.md)
* [Padrões de UI Web](https://github.com/decentraland/docs/tree/main/contributor/contributor-guides/web-ui-standards/README.md)
* [WKC Architecture](https://github.com/decentraland/docs/tree/main/contributor/contributor-guides/well-known-components/README.md)
* [Diretrizes de documentação de API](/docs/contributor/contributor-pt/guias-para-colaboradores/api-documentation/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/guias-para-colaboradores/dependency-management.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
