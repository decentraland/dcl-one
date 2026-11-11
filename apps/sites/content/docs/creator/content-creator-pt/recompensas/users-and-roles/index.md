> Gerencie quem pode operar uma campanha do Decentraland Rewards atribuindo funções e permissões a endereços de wallet.

# Usuários e Funções

Para realizar quaisquer ações numa campaign dentro do serviço Rewards, tem de ter sessão iniciada com uma wallet que tenha as permissões necessárias.

Quando cria uma campaign, é automaticamente atribuído como seu owner e recebe todas as permissões. Como owner, pode adicionar outros utilizadores à campaign e atribuir-lhes funções com permissões específicas. No entanto, cada campaign só pode ter um owner — o criador — que é o único utilizador com autoridade para gerir os fundos.

Para adicionar um utilizador à campaign.

1. Clique na **Funções de utilizador** separador para ver a lista de utilizadores que têm acesso à campaign.
2. No final desta lista, encontrará um formulário para adicionar um novo utilizador. Nota: Este formulário só ficará visível se tiver as permissões necessárias para adicionar utilizadores.
3. Preencha o formulário introduzindo o endereço da wallet do utilizador (ou o nome ENS) e selecionando a respetiva função.
4. Clique nos **Adicionar utilizador** botão para finalizar a adição.

![](/docs/_remote/f926b41052805cdf281f1c90.webp)

## Permissões para funções

Estas são as ações permitidas para cada função:

| Permissão                               | visualizador | desenvolvedor | colaborador | owner |
| --------------------------------------- | :----------: | :-----------: | :---------: | :---: |
| Ver nome da campaign, rede e gas máximo |       ✅      |       ✅       |      ✅      |   ✅   |
| Editar nome da campaign e gas máximo    |       ❌      |       ❌       |      ✅      |   ✅   |
| Ativar e desativar campaign             |       ❌      |       ❌       |      ✅      |   ✅   |
| Ver supply                              |       ✅      |       ✅       |      ✅      |   ✅   |
| Adicionar/remover supply                |       ❌      |       ✅       |      ✅      |   ✅   |
| Ver dispensers                          |       ✅      |       ✅       |      ✅      |   ✅   |
| Ver chave do dispenser                  |       ❌      |       ✅       |      ✅      |   ✅   |
| Adicionar/remover dispenser             |       ❌      |       ✅       |      ✅      |   ✅   |
| Ver transações da campaign              |       ✅      |       ✅       |      ✅      |   ✅   |
| Ver utilizadores                        |       ✅      |       ✅       |      ✅      |   ✅   |
| Adicionar/Remover utilizadores          |       ❌      |       ❌       |      ✅      |   ✅   |
| Remover utilizadores owner              |       ❌      |       ❌       |      ❌      |   ✅   |
| Gerencie fundos                         |       ❌      |       ❌       |      ❌      |   ✅   |


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/recompensas/users-and-roles.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
