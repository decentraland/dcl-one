> Lugares

# Lugares

<details>

<summary>O que são Places?</summary>

[Places](https://decentraland.org/places/) mostra certos pontos de interesse em Genesis City e Worlds.

</details>

<details>

<summary>Como edito o título, a descrição ou a imagem do meu Place?</summary>

O título, a descrição e a imagem do seu Place são obtidos dos metadados da scene. Saiba mais sobre como editar os metadados da scene na documentação do Creator.

</details>

<details>

<summary>Como o meu Place é atualizado?</summary>

Quando você reenviar a sua scene, o site de Places atualizará automaticamente quaisquer metadados.

</details>

<details>

<summary>O que acontece se eu alterar significativamente o meu Place?</summary>

A cada Place é atribuído um ID único. Quando certas condições são atendidas, o seu Place pode ser considerado "novo", resultando na atribuição de um novo ID. Essa alteração fará com que o seu Place perca todos os favoritos e likes acumulados. Para mais detalhes sobre essas condições, consulte [**ADR: Identificadores de Place**](https://adr.decentraland.org/adr/ADR-186).

</details>

<details>

<summary>Como a porcentagem de likes é calculada?</summary>

Para evitar abuso, a porcentagem de likes é calculada a partir de likes e dislikes de usuários que tenham pelo menos 100 de poder de voto. Qualquer pessoa com uma conta pode dar like ou dislike, mas apenas votos elegíveis são usados para calcular a porcentagem de likes.

Aumente suas chances de obter uma boa avaliação certificando-se de que o seu Place tenha um título, descrição e miniatura.

</details>

<details>

<summary>Como o meu Place pode se tornar um Point of Interest?</summary>

Pontos de interesse são locais notáveis em Decentraland. Esses "POIs" são destacados em várias áreas da UI do mundo virtual e são promovidos como bons lugares para os usuários explorarem, especialmente pessoas novas na Decentraland.

Você pode criar uma proposta no [**DAO**](https://decentraland.org/dao/en/) para nomear o seu Place como um Point of Interest.

</details>

<details>

<summary>Quais Worlds aparecem na aba Worlds?</summary>

Conforme definido no *Worlds 1.0 - Plano de Curto Prazo* [**proposta de DAO**](https://decentraland.org/governance/proposal/?id=e712bb50-e822-11ed-b8f1-75dbe089d333/), permitimos a descoberta de Worlds implantados no World Content Server da Foundation apenas para proprietários de NAME que também possuam LAND ou um contrato de aluguel de LAND ativo.

</details>


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/player/player-pt/faqs/places.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
