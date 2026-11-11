# Testes E2E


> [!NOTE]
> Esta seção está atualmente em andamento (WIP).


Esta seção descreve como os desenvolvedores irão testar a base de código relacionada aos testes e2e.

## Stack de testes

Todos os nossos testes de UI DEVEM ser feitos usando [Cypress](https://www.cypress.io/) como os principais frameworks de teste com [Synpress](https://github.com/Synthetixio/synpress) para suporte à wallet. O código do teste DEVE ser escrito usando [TypeScript](https://www.typescriptlang.org/) e executado usando [ts-jest](https://github.com/kulshekhar/ts-jest) para ter suporte de verificação de tipos.

## Estrutura de diretórios

Novos testes DEVEM ser colocados em um `tests/e2e` diretório, no caminho raiz. Os arquivos serão nomeados conforme o fluxo que irão testar; ou seja, se o arquivo de testes e2e estiver testando o `Publicação de coleção` fluxo, o nome do arquivo DEVE ser `collection-publication.spec.cy.ts`.

## Com que frequência serão executados

Agendado via GitHub Actions. Frequência a definir.

## O que testar

* As interações de usuário mais relevantes nas dApps (publicações de itens, compras de NFTs, etc.)

## Como testar

Todos os nossos testes DEVEM seguir o estilo semiestruturado de **Dado-Quando-Então** (GWT).

O seguinte exemplo:

1. **Dado** um usuário visita `https://example.cypress.io`
2. **Quando** eles clicam no link rotulado `tipo`
3. E eles digitam "<fake@email.com>" no `[data-testid="action-email"]` campo de entrada
4. **Depois** a URL deve incluir `/commands/actions`
5. E o `[data-testid="action-email"]` campo de entrada tem "<fake@email.com>" como seu valor

Deve ser escrito assim:

```jsx
describe('Dado que um usuário visita https://example.cypress.io', () => {
  beforeEach(() => {
    cy.visit('https://example.cypress.io')
  })

  describe('quando clica no link rotulado type', () => {
    beforeEach(() => {
      cy.contains('type').click()
    })

    describe('e digita um e-mail no input action-email', () => {
      beforeEach(() => {
        // Obtenha um campo de entrada e digite nele
        cy.get('.action-email').type('fake@email.com')
      })

      it('Então a URL deve incluir /commands/actions', () => {
        // Deve estar em uma nova URL que
        // inclui '/commands/actions'
        cy.url().should('include', '/commands/actions')
      })

      it('e o valor do input action-email deve conter o e-mail digitado', () => {
        // Verifique se o valor foi atualizado
        cy.get('.action-email').should('have.value', 'fake@email.com')
      })
    })
  })
})
```

## Testando contratos

Para testar smart contracts, consulte a documentação dos padrões Solidity.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/guias-para-colaboradores/testing-standards/testing-e2e.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
