> Use áreas de modificador no Scene Editor para alterar o comportamento do avatar ou forçar um modo de câmera dentro de uma região da sua scene.

# Áreas de Modificador

Áreas de modificação são regiões invisíveis que alteram a forma como os jogadores vivenciam uma parte da sua cena. Existem dois tipos:

* **Avatar Modifier Area**: Altera a forma como os avatares se comportam ou aparecem para os jogadores dentro da região, por exemplo, ocultando avatares ou desativando pop-ups de passaporte.
* **Camera Modifier Area**: Força a câmara do jogador para a primeira pessoa ou terceira pessoa enquanto ele estiver dentro da região.

Ambos os tipos regressam ao comportamento normal quando o jogador sai.

## Adicionar uma área de modificação

Podes adicionar uma área de modificação de duas formas:

**A partir do catálogo de assets:** Abre o catálogo de assets e procura por **Avatar Modifier Area** ou **Camera Modifier Area** na **Utils** categoria. Arrasta-o para a cena. O item aparece como um cubo de placeholder semitransparente que só é visível no editor, não na cena em execução.

**A partir do menu de componentes:** Seleciona qualquer entidade existente, clica no **+** botão na parte superior do painel de propriedades e escolhe **Avatar Modifier Area** ou **Camera Modifier Area** na lista. Vê [Adicionar componentes](/docs/creator/content-creator-pt/scene-editor/construir/components/#add-components) para mais detalhes.

![O menu Adicionar componente aberto numa entidade chamada Stage area, listando componentes sob os grupos Conteúdo 3D e Interação, incluindo Avatar Modifier Area e Camera Modifier Area.](/docs/_remote/e0e52c0c060b63e97e2466dd.webp)

## Dimensionar a área

O tamanho de uma área de modificação é controlado pela **escala do Transform**. Se redimensionares a entidade para 4 x 3 x 4, a região de modificação torna-se uma caixa de 4 x 3 x 4 metros centrada na posição da entidade. Usa o gizmo de escala ou escreve valores exatos no componente Transform para definir o tamanho que precisas.


> [!WARNING]
> **Nota:** Não existe um campo "Area" separado no painel do editor. O editor mantém a propriedade interna da área sincronizada com a escala da entidade automaticamente, por isso tudo o que precisas fazer é redimensionar a entidade.


## Avatar Modifier Area

O painel Avatar Modifier Area tem duas definições:

![O painel do componente Avatar Modifier Area, com um menu dropdown Modifiers a mostrar Hide Avatars selecionado, uma secção Exclude Player IDs e um botão Add Player ID.](/docs/_remote/d49ca42bf27a1995edcc536d.webp)

### Modificadores

Um menu dropdown de seleção múltipla com as seguintes opções:

* **Ocultar avatares**: Os avatares dentro da área tornam-se invisíveis. Os jogadores dentro da área não se conseguem ver uns aos outros, mas os jogadores fora da área continuam a vê-los normalmente.
* **Desativar passaportes**: Clicar no avatar de um jogador dentro da área já não abre o pop-up do perfil.

Podes selecionar um ou ambos os modificadores na mesma área. Quando adicionas o componente, **Ocultar avatares** está selecionado por predefinição. Clica no **x** num modificador para o remover.

### Excluir IDs de jogador

Uma lista de endereços de wallet que não são afetados pelos modificadores. Os jogadores cujo endereço está nesta lista comportam-se normalmente, mesmo dentro da área.

Clique em **Adicionar ID de jogador** para adicionar uma entrada e depois cola o endereço da wallet. Para remover uma, clica no menu de três pontos ao lado dela e seleciona **Remover ID de jogador**.

Isto é útil para eventos em que queres ocultar os avatares da audiência num palco, mas manter os performers visíveis.


> [!WARNING]
> **Nota:** Certifica-te de que os endereços de wallet estão em minúsculas. A área de modificação usa correspondência exata de strings.


### Casos de uso

* **Zonas de privacidade**: Oculta os avatares nos bastidores ou no camarim.
* **Palcos limpos**: Oculta os avatares da audiência num palco para que apenas os intérpretes (listados em IDs de jogador excluídos) fiquem visíveis.
* **Arenas de jogo**: Desativa os passaportes numa área de combate para que os jogadores não abram perfis acidentalmente durante o jogo.

## Camera Modifier Area

O painel Camera Modifier Area tem uma definição:

![O painel do componente Camera Modifier Area, com um menu dropdown Modo da câmara definido para Primeira pessoa.](/docs/_remote/199bb09636ac6f76d722633e.webp)

### Modo da câmara

Um menu dropdown com duas opções:

* **Primeira pessoa**: Força o jogador para a vista em primeira pessoa enquanto estiver dentro da área.
* **Terceira pessoa**: Força o jogador para a vista em terceira pessoa enquanto estiver dentro da área.

A predefinição é **Primeira pessoa**. Quando um jogador sai da área, o modo de câmara anterior é restaurado.


> [!NOTE]
> **Dica:** O modo de câmara cinematográfica não está disponível aqui. Para controlar a câmara para além da alternância entre primeira/terceira pessoa, usa uma [Virtual Camera](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/camera/#using-virtual-cameras) com código.


### Casos de uso

* **Corredores estreitos ou salas de puzzles**: Força a primeira pessoa para clicar com precisão em objetos pequenos.
* **Miradouros panorâmicos**: Força a terceira pessoa para que os jogadores vejam o seu avatar com o cenário de fundo.
* **Zonas de tutorial**: Garante que todos os jogadores usam o mesmo modo de câmara durante uma experiência guiada.

## Combinar áreas de modificação

Podes colocar ambos os tipos de área de modificação na mesma entidade, ou sobrepor áreas de modificação separadas no mesmo espaço. Cada área aplica os seus efeitos de forma independente.


> [!WARNING]
> **Nota:** Mantém baixo o número de áreas de modificação na tua cena. Demasiadas podem afetar o desempenho. Um pequeno número é suficiente para a maioria das cenas.


## Usar áreas de modificação com código

Os componentes do Scene Editor correspondem diretamente aos componentes do SDK7 `AvatarModifierArea` e `CameraModeArea` componentes. Para detalhes completos sobre os modificadores disponíveis, o comportamento em Runtime e opções avançadas como alterar dinamicamente a lista de exclusão, consulta:

* [áreas de modificação de avatar](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/avatares/modifier-areas/) (SDK7)
* [áreas de modo de câmara](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/camera/#1st-and-3rd-person-camera-modes) (SDK7)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scene-editor/interatividade/modifier-areas.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
