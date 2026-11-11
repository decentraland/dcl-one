> Gifting Credits é um programa beta apenas por convite: os estúdios participantes oferecem Credits aos seus jogadores a partir da página Studio da Shop.

# Oferecer Credits (Beta)

A oferta de Credits está atualmente num programa beta apenas por convite. Os estúdios participantes podem oferecer Credits aos seus jogadores.

Se o seu estúdio foi convidado a participar, pode oferecer Credits através da página Studio da Shop em [decentraland.org/shop/studio](https://decentraland.org/shop/studio). Os jogadores podem gastar estes Credits em qualquer coisa na Shop.

## Antes de começar

* **A sua conta tem de gerir o estúdio.** A Decentraland adiciona as contas que podem oferecer Credits para o seu estúdio. Se iniciar sessão e vir *Esta conta não gere um estúdio*, peça ao seu contacto na Decentraland para adicionar essa conta.
* **As contas do seu próprio estúdio não podem receber os respetivos Credits.** Use uma conta diferente para qualquer gift de teste.
* **Os gifts são definitivos.** Os Credits oferecidos nunca expiram e não podem ser retirados, por isso verifique a lista antes de a confirmar.

## A sua página do Studio

Inicie sessão na página do Studio em [decentraland.org/shop/studio](https://decentraland.org/shop/studio). Se a sua conta gerir mais de um estúdio, selecione-o no **Studio** menu. A página mostra:

* **Orçamento restante**: os Credits que o seu estúdio ainda pode oferecer.
* **Oferecido até agora**: o que o seu estúdio ofereceu e em quantos gifts.
* **Máximo por jogador**: o maior gift que um único jogador pode receber de uma só vez.
* **Ofertas**: cada gift que o seu estúdio fez, com o jogador, os Credits, o motivo, a data e quem o enviou.

![A página do Studio, com o orçamento restante, os Credits oferecidos até agora, o máximo por jogador e a lista de gifts](/docs/_remote/eca2a726b6b3c94c7710e0b5.webp)

Se a página indicar que o estúdio está em pausa, não pode oferecer gifts até que a Decentraland o retome.

## Oferecer Credits

1. Pressione **Oferecer Credits**.
2. Escreva um **Motivo**. É usado para cada jogador sem um motivo próprio: uma linha, até 80 caracteres, como *Melhor jogador da semana*.
3. Adicione os jogadores, um por linha: a respetiva conta (o `0x…` endereço) e os Credits a oferecer. Pode atribuir a cada linha o seu próprio motivo.
   * **Adicionar jogador** adiciona uma linha.
   * **Colar uma lista** aceita um jogador por linha: conta, Credits e um motivo opcional. As colunas copiadas de uma folha de cálculo também funcionam.

     ```
     0x…, 100, Melhor jogador da semana
     0x…, 50
     ```
   * Escreva os valores sem separadores de milhares (`1000`, e não `1,000`).
   * Cada conta entra apenas numa linha: se um jogador receber dois valores, some-os.
4. Verifique o resumo por baixo da lista (jogadores, total de Credits e o orçamento restante após isso) e depois prima **Rever**.

   ![A janela Offer Credits com um motivo e dois jogadores, cada um com os seus próprios Credits e motivo](/docs/_remote/24ad20ec86951563169233a0.webp)
5. A revisão avisa-o de qualquer jogador que tenha recebido o mesmo gift (os mesmos Credits e motivo) do seu estúdio nos últimos 30 dias. Volte atrás e remova-o, a menos que pretenda oferecê-lo novamente.
6. Escreva o total de Credits para confirmar e prima **Oferecer a … jogadores**.

   ![A etapa de confirmação, com o total de Credits introduzido e o botão Oferecer a 2 jogadores](/docs/_remote/f677deb1676df2cd4f1c5912.webp)

Os gifts são enviados um a um e cada linha mostra como correu. Se a janela fechar ou parar a lista, abra **Oferecer Credits** novamente e prima **Continuar**: os jogadores que já receberam o respetivo gift nunca o recebem duas vezes.

## O que os seus jogadores recebem

* Uma notificação na Decentraland a dizer que o seu estúdio lhes ofereceu Credits.
* Um email, se tiverem confirmado o respetivo email na conta da Decentraland.
* O gift na atividade da Shop e os Credits no respetivo saldo, prontos a gastar.

## Se um gift não for efetuado

| Mensagem                           | O que fazer                                                                                                                                       |
| ---------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------- |
| Esta conta pertence ao seu estúdio | As contas do seu próprio estúdio não podem receber os respetivos Credits. Remova a linha.                                                         |
| Esta conta não pode receber gifts  | Este jogador não pode receber Credits. Remova a linha.                                                                                            |
| Acima do máximo por jogador        | Reduza o valor para o **Máximo por jogador** indicado na sua página do Studio.                                                                    |
| Não resta orçamento suficiente     | Ofereça menos, ou fale com o seu contacto na Decentraland sobre o seu orçamento.                                                                  |
| O estúdio está em pausa            | Espere até que a Decentraland retome o estúdio e depois prima **Continuar**.                                                                      |
| Não recebemos uma confirmação      | Consulte o **Ofertas** lista antes de o enviar novamente: o gift pode já lá estar. Premir **Continuar** nunca oferece gifts duas vezes a ninguém. |


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/recompensas/gifting-credits.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
