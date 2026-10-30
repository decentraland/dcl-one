# Testing End-to-End (E2E)


> [!NOTE]
> This section is currently a Work in Progress (WIP).


This section describes how developers will test codebase related to the e2e tests.

## Testing stack

All of our UI tests MUST be done using [Cypress](https://www.cypress.io/) as the main testing frameworks with [Synpress](https://github.com/Synthetixio/synpress) for wallet support. The test's code MUST be written using [Typescript](https://www.typescriptlang.org/) and ran using [ts-jest](https://github.com/kulshekhar/ts-jest) to have type checking support.

## Directory structure

New tests MUST be placed in a `tests/e2e` directory, in the root path. The files will be named as the flow that they will be testing, that is, if the e2e tests file will be testing the `Collection Publication` flow, the name of the file MUST be `collection-publication.spec.cy.ts`.

## How frequent they will run

Scheduled via GitHub Actions. Frequency TBD.

## What to test

* Most relevant user interactions in the dApps (Items Publications, NFTs Purchases, etc.)

## How to test

All of our tests MUST follow the semi-structured style of **Given-When-Then** (GWT).

The following example:

1. **Given** a user visits `https://example.cypress.io`
2. **When** they click the link labeled `type`
3. And they type "fake@email.com" into the `[data-testid="action-email"]` input
4. **Then** the URL should include `/commands/actions`
5. And the `[data-testid="action-email"]` input has "fake@email.com" as its value

Should be written as:

```jsx
describe('Given a user visits https://example.cypress.io', () => {
  beforeEach(() => {
    cy.visit('https://example.cypress.io')
  })

  describe('when clicks the link labeled type', () => {
    beforeEach(() => {
      cy.contains('type').click()
    })

    describe('and type an email into the action-email input', () => {
      beforeEach(() => {
        // Get an input, type into it
        cy.get('.action-email').type('fake@email.com')
      })

      it('Then the url should include /commands/actions', () => {
        // Should be on a new URL which
        // includes '/commands/actions'
        cy.url().should('include', '/commands/actions')
      })

      it('and the action-email input value should contain the typed email', () => {
        //  Verify that the value has been updated
        cy.get('.action-email').should('have.value', 'fake@email.com')
      })
    })
  })
})
```

## Testing contracts

For testing smart contracts, refer to the Solidity Standards documentation.
