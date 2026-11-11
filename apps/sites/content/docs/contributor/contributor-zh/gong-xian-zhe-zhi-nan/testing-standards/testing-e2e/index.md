# 测试端到端


> [!NOTE]
> 本节目前仍在开发中（WIP）。


本节介绍开发者将如何测试与 e2e 测试相关的代码库。

## 测试栈

我们所有的 UI 测试都必须使用 [Cypress](https://www.cypress.io/) 作为主要测试框架，与 [Synpress](https://github.com/Synthetixio/synpress) 一起用于钱包支持。测试代码必须使用 [TypeScript](https://www.typescriptlang.org/) 并使用 [ts-jest](https://github.com/kulshekhar/ts-jest) 以获得类型检查支持。

## 目录结构

新测试必须放在 `tests/e2e` 目录中，位于根路径下。文件将以它们要测试的流程命名，也就是说，如果 e2e 测试文件将测试 `合集发布` 流程，那么文件名必须是 `collection-publication.spec.cy.ts`.

## 运行频率

通过 GitHub Actions 定时运行。频率待定。

## 测试内容

* dApp 中最相关的用户交互（项目发布、NFT 购买等）

## 如何测试

我们所有的测试必须遵循 **Given-When-Then** （GWT）的半结构化风格。

以下示例：

1. **假设** 用户访问 `https://example.cypress.io`
2. **当** 他们点击标记为 `类型`
3. 并将“<fake@email.com>”输入到 `[data-testid="action-email"]` 输入框
4. **然后** 网址应包含 `/commands/actions`
5. 并且该 `[data-testid="action-email"]` 输入框的值为“<fake@email.com>”

应写成：

```jsx
describe('假设用户访问 https://example.cypress.io', () => {
  beforeEach(() => {
    cy.visit('https://example.cypress.io')
  })

  describe('当点击标记为 type 的链接时', () => {
    beforeEach(() => {
      cy.contains('type').click()
    })

    describe('并在 action-email 输入框中输入电子邮件时', () => {
      beforeEach(() => {
        // 获取输入框，并在其中输入
        cy.get('.action-email').type('fake@email.com')
      })

      it('那么 URL 应包含 /commands/actions', () => {
        // 应位于一个新的 URL 上
        // 包含 '/commands/actions'
        cy.url().should('include', '/commands/actions')
      })

      it('并且 action-email 输入框的值应包含输入的电子邮件', () => {
        // 验证该值已更新
        cy.get('.action-email').should('have.value', 'fake@email.com')
      })
    })
  })
})
```

## 测试合约

如需测试智能合约，请参阅 Solidity 标准文档。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/gong-xian-zhe-zhi-nan/testing-standards/testing-e2e.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
