> Onde a UI da scene pode ficar com segurança no mobile — fora das margens de hardware do dispositivo e dos próprios controles do client.

# Área Segura no Mobile

No telemóvel, duas coisas independentes consomem o espaço do ecrã que a sua UI pode usar com segurança:

* **As margens de hardware do dispositivo** — o notch ou recorte da câmara, a barra de estado, o indicador de início e os cantos arredondados. A UI desenhada por baixo deles fica parcialmente oculta ou difícil de tocar.
* **Os próprios controlos do cliente Decentraland** — joystick, chat, perfil, controlos da câmara, botões de ação. A UI da cena colocada por baixo deles colide visualmente e compete pelos mesmos toques.

Não precisa de medir nenhum dos dois. O cliente reporta ambas as áreas em runtime e o SDK posiciona a sua UI dentro da que escolher, com a [`screenInset`](/docs/creator/content-creator-pt/scenes-sdk7/ui-2d/onscreen-ui/#screen-inset-area) opção de renderer:

| O que quer                                                  | O que passar                              |
| ----------------------------------------------------------- | ----------------------------------------- |
| Longe das margens de hardware                               | nada — `screenInset: 'device'` é o padrão |
| Longe das margens de hardware **e** os controlos do cliente | `screenInset: 'interactable'`             |
| O ecrã inteiro, as margens são da sua responsabilidade      | `screenInset: 'none'`                     |

```ts
// Recomendado para UI da cena que não deve colidir com os controlos móveis
ReactEcsRenderer.setUiRenderer(uiComponent, { screenInset: 'interactable' })
```


> [!WARNING]
> **Suporte do cliente**: `'interactable'` precisa do cliente móvel `1.12.1` ou mais recente. Em clientes mais antigos, a área é reportada como zero e a UI recua para cobrir o ecrã inteiro. Essa mesma versão normaliza a `'device'` área entre Android e iOS.
> 
> **Não é um no-op no desktop** — o cliente de desktop reserva aproximadamente os 25% esquerdos do ecrã para a sua própria UI, por isso `'interactable'` também altera o layout do seu desktop. Faça uma branch com [`isMobile()`](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/detect-platform/) se quiser apenas o inset nos telefones.


Veja [Área de inset do ecrã](/docs/creator/content-creator-pt/scenes-sdk7/ui-2d/onscreen-ui/#screen-inset-area) para a referência completa: as três áreas lado a lado num dispositivo real, o comportamento por renderer e como se relacionam com o [ecrã virtual](/docs/creator/content-creator-pt/scenes-sdk7/ui-2d/onscreen-ui/#default-virtual-size).

## Onde vivem os controlos do cliente

`'interactable'` dá-lhe a área **que cada Explorer designa para a UI da cena** — o que deliberadamente não é uma promessa de que todo o controlo do cliente esteja fora dela. No cliente móvel, é a área segura do dispositivo menos a coluna do lado esquerdo (chat, perfil, joystick, emotes); os botões de ação no canto inferior direito são desenhados sobre a área **por design**.

<figure><img src="/docs/_remote/de3e57715b49b467501da7ac.webp" alt="Scene UI inset to the area the mobile client reports as free of its own HUD"><figcaption><p><code>screenInset: 'interactable'</code> no cliente móvel <code>1.12.1</code>. O retângulo magenta é a UI da cena, o contorno verde delimita a área interativa. A coluna da esquerda é excluída; os botões de ação no canto inferior direito ficam sobre a área.</p></figcaption></figure>

Portanto, mesmo com `'interactable'`, trate estas zonas como concorridas:

* **Canto inferior direito** — os botões de ação e o botão de interação são desenhados por cima. Tudo o que colocar aqui continua alcançável, mas compete pelos toques.
* **Canto superior direito** — os controlos de perfil e da câmara ficam mesmo fora da área, por isso a UI encostada a esse canto é lida como parte do HUD do cliente.

O que a área cobre é uma decisão de cada Explorer e pode mudar entre versões. Leia-a em runtime, nunca a codifique, e verifique-a nas plataformas que pretende usar.

## Onde colocar a UI da cena

* **Centro do ecrã** — diálogos acionáveis, tudo o que o jogador precise de ler e responder.
* **Centro superior** — mensagens não acionáveis, estado e notificações.
* **Centro inferior, acima do botão de interação** — dicas contextuais.
* **Não no canto inferior direito** — mesmo dentro da área interativa, pertence aos botões de ação.

## Em vez disso, envolva parte da sua UI

Se optou por não usar `screenInset: 'none'` e quiser proteger apenas parte da sua UI, envolva-a no componente `ScreenInsetArea` ou no `InteractableArea` Cada um lê a sua área correspondente e aplica-a aos seus filhos. Importe-os de `@dcl/sdk/react-ecs`:

```tsx
import ReactEcs, { ReactEcsRenderer, UiEntity, ScreenInsetArea } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export function setupUi() {
  ReactEcsRenderer.setUiRenderer(() => (
    <ScreenInsetArea>
      {/* Uma child com 100% × 100% ocupa exatamente a área segura do inset */}
      <UiEntity
        uiTransform={{ width: '100%', height: '100%' }}
        uiBackground={{ color: Color4.create(0, 0, 0, 0.5) }}
      />
    </ScreenInsetArea>
  ), { screenInset: 'none' })
}
```

Ambos os componentes posicionam-se absolutamente usando os valores que o cliente reporta, por isso os `positionType` e `posição` campos dos seus `uiTransform` são reservados — qualquer valor que definir para eles é ignorado. Todas as outras `uiTransform` propriedades (`padding`, `flexDirection`, `alignItems`, …) e componentes de UI (`uiBackground`, `onMouseDown`, …) funcionam normalmente. Reagem automaticamente quando a área muda, por exemplo na rotação ou quando as barras do sistema aparecem ou desaparecem, e compensam o [fator de escala da UI](/docs/creator/content-creator-pt/scenes-sdk7/ui-2d/onscreen-ui/#screen-virtual-scale), para que as margens fiquem corretas, seja qual for o tamanho do seu ecrã virtual.


> [!WARNING]
> **📔 Não aplique o inset duas vezes:** o valor `screenInset: 'none'` no snippet acima é o que importa. Envolver a sua UI num componente enquanto o renderer também aplica o inset correspondente empurra a sua UI para dentro com o dobro da margem. Escolha uma coisa ou outra.



> [!NOTE]
> **📱 Apenas no mobile:** os insets do dispositivo só têm valores reais no **cliente móvel**. No **cliente de desktop** eles são `(0, 0, 0, 0)`, portanto `screenInset: 'device'` e a `ScreenInsetArea` os componentes não têm efeito aí e a sua UI é renderizada exatamente como seria sem eles. A área interativa, ao contrário dos insets do dispositivo, *está* não é zero no desktop.


## Por que isto importa

A UI da cena que se sobrepuser aos controlos do cliente irá:

* Ficar parcialmente escondida atrás do joystick, do botão de interação ou dos controlos da câmara.
* Competir pelos toques com esses controlos — os jogadores vão ativar um ou outro acidentalmente.
* Fazer com que a sua cena pareça avariada no mobile, o que prejudica o destaque e a retenção.

Verifique sempre num dispositivo real usando o [código QR da Preview](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/preview-on-mobile/).

## Relacionado

* [Área de inset do ecrã](/docs/creator/content-creator-pt/scenes-sdk7/ui-2d/onscreen-ui/#screen-inset-area) — a `screenInset` referência completa.
* [Boas práticas de UI para mobile](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/ui-best-practices/)
* [Detete a plataforma a partir do código](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/detect-platform/) — use `isMobile()` para alternar layouts.
* [UI no ecrã](/docs/creator/content-creator-pt/scenes-sdk7/ui-2d/onscreen-ui/)
* [Guia de UX e UI](/docs/creator/content-creator-pt/scenes-sdk7/conceber-a-experiencia/ux-ui-guide/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/criar-para-mobile/desenvolver/safe-area.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
