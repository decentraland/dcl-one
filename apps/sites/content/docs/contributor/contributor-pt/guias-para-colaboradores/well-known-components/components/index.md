# Components

Componentes são partes de software autocontidas, projetadas como caixas-pretas que recebem outros componentes (ou nenhum) para executar suas tarefas. O objetivo principal é desacoplar seções de código e maximizar a testabilidade.

## Estrutura de diretórios

Todos os componentes DEVEM ser definidos dentro de um diretório dedicado contendo, no mínimo, estes quatro arquivos:

* `component.ts` - Contém o código de implementação do componente
* `types.ts` - Contém a interface do componente e os tipos expostos
* `errors.ts` - Contém todas as classes de erro exportadas
* `index.ts` - Exporta a API pública do componente

### Estrutura de Exemplo

```
src/
└── components/
    └── my-component/
        ├── component.ts
        ├── types.ts
        ├── errors.ts
        └── index.ts
```

## Arquivo de Tipos

Componentes seguem uma interface que define sua API pública. Essa interface pode ser compartilhada entre componentes intercambiáveis ou usada para entender os métodos expostos pelo componente.

Todos os componentes que têm sua própria interface DEVEM defini-la em um `types.ts` arquivo dentro do diretório do componente.

Outros tipos expostos pelo componente DEVEM também ser colocados neste arquivo.

### Exemplo: `types.ts`

```tsx
export type Something = {
  aValue: boolean
  anotherValue: string
}

export interface IMyComponent {
  myFunction: () => boolean
  getSomething: () => Something
}
```

### Tipos Compartilhados

Tipos compartilhados entre vários componentes, como interfaces comuns, DEVEM ser colocados em um `types.ts` arquivo no diretório raiz do código-fonte do projeto (`/src/types.ts`).

## Arquivo do Componente

O arquivo do componente contém a função criadora do componente. Essa função segue uma convenção específica de nomenclatura e estrutura.

### Convenção de Nomenclatura

As funções criadoras de componentes DEVEM ser nomeadas como: `create` + `ComponentName` + `Component`

**Exemplo:** `createMyNewComponent`

### Assinatura da Função

A função criadora DEVE:

1. Receber um objeto components como o primeiro parâmetro contendo todas as dependências
2. Opcionalmente receber parâmetros adicionais de configuração
3. Retornar um objeto contendo os métodos expostos (públicos)

### Estrutura do componente

No início da função criadora do componente:

1. Extrair as dependências do objeto components
2. Inicializar variáveis comuns (por exemplo, loggers, configuração)
3. Definir funções auxiliares internas
4. Definir métodos públicos
5. Retornar a API pública

### Exemplo: `component.ts`

```tsx
import { IMyComponent, Something } from './types'
import { WrongStringError } from './errors'

export function createMyNewComponent(
  components: Pick<AppComponents, 'logs'>
): IMyComponent {
  const { logs } = components
  const logger = logs.getLogger('my-component')

  // Método interno - não exposto
  function computeString(fstString: string, sndString: string): string {
    if (fstString.length === 0) {
      throw new WrongStringError()
    }
    
    return fstString + ' ' + sndString
  }

  // Método público - exposto no objeto de retorno
  function myFunction(): boolean {
    return true
  }

  // Método público - exposto no objeto de retorno
  function getSomething(): Something {
    logger.info('Getting something')
    
    return {
      aValue: true,
      anotherValue: computeString('aString', 'anotherString')
    }
  }

  // Retornar a API pública
  return {
    myFunction,
    getSomething
  }
}
```

## Arquivo de Erros

O arquivo de erros contém classes de erro personalizadas que um componente pode lançar. Esses erros podem ser usados em outros componentes ou controllers para identificar corretamente e tratar condições de erro específicas.

### Exemplo: `errors.ts`

```tsx
export class WrongStringError extends Error {
  constructor(message?: string) {
    super(message || 'Incorrect String')
    this.name = 'WrongStringError'
  }
}

export class ComponentNotInitializedError extends Error {
  constructor() {
    super('Component has not been initialized')
    this.name = 'ComponentNotInitializedError'
  }
}
```

### Melhores Práticas de Tratamento de Erros

* Crie classes de erro específicas para diferentes condições de erro
* Inclua mensagens de erro significativas
* Defina o `name` propriedade para corresponder ao nome da classe e facilitar a depuração
* Documente quais erros podem ser lançados por cada método público

## Arquivo Index

O arquivo index serve como o ponto de entrada público do componente, exportando apenas o que deve estar acessível às outras partes da aplicação.

### Exemplo: `index.ts`

```tsx
export { createMyNewComponent } from './component'
export type { IMyComponent, Something } from './types'
export { WrongStringError } from './errors'
```

## Melhores práticas

1. **Responsabilidade Única** - Cada componente deve ter um propósito claro
2. **Dependências Explícitas** - Todas as dependências devem ser injetadas por meio do parâmetro components
3. **Imutabilidade** - Prefira estruturas de dados imutáveis sempre que possível
4. **Tratamento de erros** - Use classes de erro personalizadas para diferentes condições de erro
5. **Logging** - Use o logger para depuração e monitoramento
6. **Segurança de tipos** - Aproveite ao máximo o sistema de tipos do TypeScript
7. **Documentação** - Documente métodos complexos e lógica de negócio

## Ciclo de Vida do Componente

Alguns componentes podem precisar executar operações de configuração ou limpeza. O WKC fornece métodos de ciclo de vida:

* `[START_COMPONENT]` - Chamado quando o componente inicia
* `[STOP_COMPONENT]` - Chamado quando o componente para

Veja a [Adapters](/docs/contributor/contributor-pt/guias-para-colaboradores/well-known-components/adapters/) seção com exemplos de componentes que usam métodos de ciclo de vida.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/guias-para-colaboradores/well-known-components/components.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
