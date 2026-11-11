> A DAO permite que os usuários criem e votem em proposals que moldam o metaverso.

# O que você pode fazer com a DAO

A DAO permite dois tipos gerais de propostas: **propostas com ações vinculantes diretas**, e **propostas de governança**.

## Propostas com ações vinculantes diretas

A DAO permite que a comunidade vote em **ações vinculantes** que resultarão em alterações feitas nos smart contracts da Decentraland na rede Ethereum. Essas ações vinculantes são:

* Financiar um projeto da comunidade transferindo uma parte dos recursos da DAO para um contrato de vesting de grant.
* Adicionar um nó catalyst à rede de servers que hospedam e executam o mundo virtual da Decentraland.
* Adicionar ou remover pontos de interesse (POIs), ou locais destacados dentro do mundo virtual, para uma lista mostrada aos usuários. Essa lista ajuda os usuários a encontrar locais populares e interessantes para explorar.
* Banir um nome da Decentraland. Esse tipo de proposta permite que os usuários garantam que os avatares não possam receber nomes ofensivos e prejudiciais.

## Propostas de governança

Algumas propostas não são tão simples quanto adicionar ou remover um item de uma lista; elas exigem sinalização da comunidade, discussões e caminhos de implementação. Essas propostas devem ser enviadas por meio de um processo de governança em três etapas, que começa com uma enquete e termina com uma proposta vinculante.

O processo de votação inclui três etapas: uma Enquete de Pré-Proposta, uma Proposta de Rascunho e uma Proposta de Governança. Cada nível terá limiares de submissão e aprovação progressivamente maiores para garantir que decisões importantes de Governança sejam tomadas por uma maioria representativa (com base no Poder de Voto). Cada etapa deve atingir o limiar de VP definido para ser promovida à próxima.

### Etapa 1: enquete de pré-proposta

* Limiar de envio: 100 VP
* Limiar de aprovação: 500K VP (uma enquete que alcance pelo menos 500K VP e não obtenha a maioria do poder de voto participante ainda pode avançar para a etapa de Draft Proposal - garantindo que todas as questões com apoio suficiente tenham um caminho inicial para se tornarem política.)
* Período de votação: 5 dias
* Objetivo: introduzir uma questão de governança à comunidade, medir o sentimento da comunidade e determinar se há apoio suficiente para avançar com a elaboração de uma proposta inicial.

### Etapa 2: Draft Proposal

* Limiar de envio: 1.000 VP
* Limiar de aprovação: 1M VP e maioria simples (51%) do poder de voto participante (uma Draft Proposal que falhar ou não atingir esse limiar pode ser alterada e reenviada uma vez.)
* Período de votação: 1 semana
* Objetivo: apresentar uma política potencial à comunidade em um formato estruturado e formalizar a discussão sobre os impactos potenciais da proposta e os caminhos de implementação. Uma Draft Proposal que falhar ou não atingir esse limiar pode ser alterada e reenviada uma vez.

### Etapa 3: Governance Proposal

* Limiar de envio: 2.500 VP
* Limiar de aprovação: 6M VP e maioria simples do VP participante (ou os critérios de aceitação necessários para a sua categoria)
* Período de votação: 2 semanas
* Objetivo: formalizar a versão aprovada de uma Draft Proposal em um resultado vinculante de governança.

É importante observar que qualquer pessoa que atenda ao limiar de envio pode levar uma proposta aprovada (seja uma Poll ou um Draft) para a próxima etapa, não apenas o autor da proposta.

## E quanto a modificar os smart contracts de LAND ou Estate?

No momento, a DAO é dona de ambos os smart contracts de LAND e Estate. Quaisquer modificações em qualquer um desses contratos devem ser realizadas pelo Security Advisory Board (SAB) e pelo DAO Committee – grupos de pessoas de confiança e eleitas, encarregadas de garantir a segurança contínua dessas importantes partes da infraestrutura da Decentraland.

O SAB deve votar por meio de uma multi-sig wallet para aprovar quaisquer alterações nos contratos de LAND ou Estate, impedindo que um membro desonesto do SAB introduza uma vulnerabilidade.

Atualmente, não há categorias de proposta predefinidas para modificar os smart contracts de LAND ou Estate pela UI da DAO. Isso não significa que seja impossível para um membro que não seja do SAB iniciar alterações, mas é um processo mais demorado que exige obter apoio da comunidade em geral, além de ter a expertise técnica necessária para fornecer as alterações de código e uma terceira parte confiável para auditar essas alterações.

O código-fonte dos smart contracts de LAND e Estate está disponível no GitHub [aqui](https://github.com/decentraland/land/tree/master/contracts).

De modo geral, o processo para modificar qualquer um dos contratos seria:

* Consultar a comunidade por meio da publicação, pela DAO, de uma **enquete de pré-proposta**, e discutir publicamente as alterações propostas para obter apoio
* Após obter o apoio inicial da comunidade, publicar uma **proposta de rascunho** detalhando as alterações e obtendo aceitação por meio de uma votação.
* Escrever o código atualizado para ser incorporado ao contrato
* Obter uma revisão de código e uma auditoria bem-sucedidas de uma terceira parte respeitável
* Apresentar o código auditado à comunidade usando uma **proposta de Governança** e obter sua aprovação para que ele seja incorporado ao contrato
* Uma vez aprovado por outra votação da comunidade, o DAO Committee ou o SAB fariam a atualização do contrato com o novo código


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/player/player-pt/dao/dao/what-can-you-do-with-the-dao.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
