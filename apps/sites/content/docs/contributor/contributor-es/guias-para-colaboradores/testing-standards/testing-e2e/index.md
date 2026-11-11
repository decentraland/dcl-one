# Pruebas E2E


> [!NOTE]
> Esta sección está actualmente en desarrollo (WIP).


Esta sección describe cómo los desarrolladores probarán la base de código relacionada con las pruebas e2e.

## Pila de pruebas

Todas nuestras pruebas de UI DEBEN realizarse usando [Cypress](https://www.cypress.io/) como los principales frameworks de pruebas con [Synpress](https://github.com/Synthetixio/synpress) para compatibilidad con Wallet. El código de la prueba DEBE escribirse usando [TypeScript](https://www.typescriptlang.org/) y ejecutarse usando [ts-jest](https://github.com/kulshekhar/ts-jest) para tener soporte de comprobación de tipos.

## Estructura de directorios

Las nuevas pruebas DEBEN colocarse en un `tests/e2e` directorio, en la ruta raíz. Los archivos se nombrarán según el flujo que probarán; es decir, si el archivo de pruebas e2e probará el `Publicación de colecciones` flujo, el nombre del archivo DEBE ser `collection-publication.spec.cy.ts`.

## Con qué frecuencia se ejecutarán

Programado mediante GitHub Actions. Frecuencia por determinar.

## Qué probar

* Las interacciones de usuario más relevantes en las dApps (publicaciones de Items, compras de NFTs, etc.)

## Cómo probar

Todas nuestras pruebas DEBEN seguir el estilo semiestructurado de **Dado-Cuando-Entonces** (GWT).

El siguiente ejemplo:

1. **Dado** un usuario visita `https://example.cypress.io`
2. **Cuando** hacen clic en el enlace etiquetado `tipo`
3. Y escriben "<fake@email.com>" en el `[data-testid="action-email"]` campo de entrada
4. **Luego** la URL debe incluir `/commands/actions`
5. Y el `[data-testid="action-email"]` campo de entrada tiene "<fake@email.com>" como valor

Debe escribirse así:

```jsx
describe('Dado que un usuario visita https://example.cypress.io', () => {
  beforeEach(() => {
    cy.visit('https://example.cypress.io')
  })

  describe('cuando hace clic en el enlace etiquetado type', () => {
    beforeEach(() => {
      cy.contains('type').click()
    })

    describe('y escribe un correo electrónico en el input action-email', () => {
      beforeEach(() => {
        // Obtener un campo de entrada, escribir en él
        cy.get('.action-email').type('fake@email.com')
      })

      it('Entonces la URL debe incluir /commands/actions', () => {
        // Debe estar en una nueva URL que
        // incluye '/commands/actions'
        cy.url().should('include', '/commands/actions')
      })

      it('y el valor del input action-email debe contener el correo electrónico escrito', () => {
        //  Verificar que el valor se haya actualizado
        cy.get('.action-email').should('have.value', 'fake@email.com')
      })
    })
  })
})
```

## Pruebas de contratos

Para probar Smart Contracts, consulta la documentación de Solidity Standards.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/guias-para-colaboradores/testing-standards/testing-e2e.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
