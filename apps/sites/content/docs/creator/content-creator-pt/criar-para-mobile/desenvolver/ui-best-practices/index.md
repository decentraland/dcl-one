> O que fazer e o que não fazer ao conceber UIs de cena que funcionem bem em mobile.

# Melhores práticas de UI

Ainda não existe uma fórmula única e comprovada para a UI mobile do Decentraland — a plataforma é nova e ainda estamos iterando. As recomendações nesta página são as melhores práticas atuais, extraídas do trabalho em scenes reais. Considere-as um ponto de partida e teste em um dispositivo real.



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/5OmTTzpPdDc" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=5OmTTzpPdDc>">Watch on YouTube</a></p>



## FAÇA

* **Projete UIs específicas para mobile**, ou varie a sua UI conforme o tamanho da tela e a plataforma. Use [`isMobile()`](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/detect-platform/) para ramificar.
* **Mantenha a UI crítica dentro da** [**área segura**](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/safe-area/)**.** As margens de hardware do dispositivo (notch, barra de status, indicador de início) são reservadas por padrão para você — basta não desativar isso com `screenInset: 'none'`. Os próprios controles do cliente são uma área separada, e `screenInset: 'interactable'` cumpre parte dessa função: posiciona a sua UI no retângulo que o Explorer designa para a UI da scene. No mobile, isso libera os controles da esquerda; os botões de ação no canto inferior direito são desenhados sobre a área por design, então mantenha [esses cantos](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/safe-area/#where-the-client-controls-live) em mente. `'interactable'` requer o client mobile `1.12.1` ou mais recente — clients mais antigos não reportam margens nenhuma e a UI cobre a tela inteira.
* **Minimize as opções.** Mostre apenas o que o jogador precisa neste momento e revele o restante gradualmente.
* **Coloque diálogos acionáveis no centro da tela** — em qualquer lugar em que o jogador precise ler e responder.
* **Coloque mensagens não acionáveis no centro superior** — status, notificações e informações ambientais.
* **Oculte os controles de toque atrás de uma UI em tela cheia.** Enquanto um placar, uma loja ou um painel de resultados estiver aberto, o jogador não precisa pular nem pressionar E. Veja [UI em tela cheia sobre os botões de ação](#full-screen-ui-over-the-action-buttons).

## NÃO FAÇA

* **Não dimensione elementos de UI apenas em pixels sem ter uma tela virtual em mente.** Os valores em pixels são escalados com base em `virtualWidth` / `virtualHeight` resolução de referência descrita em [UI na tela](/docs/creator/content-creator-pt/scenes-sdk7/ui-2d/onscreen-ui/#screen-virtual-scale) — `1600x720` por padrão no mobile, `1920x1080` no desktop e na web. Saiba para qual resolução de referência os seus valores em pixels foram definidos, passe-a explicitamente se ela não for a padrão e combine-a com um dimensionamento ciente da plataforma. Desative a tela virtual apenas (passando um tamanho de `0`) se você realmente quiser pixels brutos do canvas.
* **Não coloque elementos fora da área segura.** Eles conflitarão com os controles do cliente.
* **Não fixe a UI no canto superior esquerdo nem na borda esquerda.** É ali que o cliente desenha o chat, o perfil e o joystick virtual, tudo sobre a UI da sua scene. Em vez disso, ancore à direita ou ao centro, ou use `screenInset: 'interactable'`. Veja [Nunca ancore sua UI no canto superior esquerdo](/docs/creator/content-creator-pt/scenes-sdk7/conceber-a-experiencia/ux-ui-guide/#never-anchor-your-ui-to-the-top-left).
* **Não coloque elementos interativos no canto inferior direito.** Os botões de ação são desenhados sobre essa região, e os toques ali vão para os botões do cliente, não para a sua scene.
* **Não dependa de botões pequenos.** Alvos pequenos são pouco confiáveis para tocar em uma tela sensível ao toque.
* **Não vincule ações de teclas a `IA_SECONDARY` ou `IA_ACTION_3`–`IA_ACTION_6`** (a `F` e `1`/`2`/`3`/`4` teclas em um teclado) a menos que a sua scene mostre esses botões. Eles ficam ocultos por padrão no mobile (SDK 7.32.0+). Veja [Input no mobile](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/input-on-mobile/).

## UI em tela cheia sobre os botões de ação

`screenInset: 'interactable'` mantém a sua UI livre dos controles da esquerda, mas os botões de ação no canto inferior direito são desenhados sobre essa área por design. Um painel que preencha a área ainda fica coberto ali, e os toques nessa região acionam os botões do cliente em vez da sua UI.

Para um **HUD de jogabilidade** que precisa continuar usável enquanto o jogador se move, mantenha-o fora do canto inferior direito: ancore-o no canto superior direito ou no centro superior.

Para um **UI que interrompe a jogabilidade**, como um placar, uma loja ou uma tela de resultados, oculte os controles de toque enquanto ela estiver aberta e restaure-os ao fechá-la:

```ts
import { TouchScreenControls } from '@dcl/sdk/ecs'

function openScoreboard() {
  TouchScreenControls.hideAll()
  TouchScreenControls.hideJoystick()
  // ... mostre a sua UI
}

function closeScoreboard() {
  TouchScreenControls.showDefault()
  TouchScreenControls.showJoystick()
  // ... oculte a sua UI
}
```

Use `showDefault()` em vez de `showAll()` para restaurar os botões: as scenes começam com apenas jump, pointer e primary visíveis, então `showAll()` traria de volta cinco botões que o jogador nunca teve. Tanto o padrão de três botões quanto `showDefault()` precisam do SDK 7.32.0 ou posterior; em SDKs mais antigos, todos os oito botões aparecem e `showAll()` é a única maneira de restaurá-los. `showDefault()` restaura apenas os botões do gamepad, então combine-o com `showJoystick()`. Ambos os helpers não fazem nada no desktop, então não há necessidade de ramificar pela plataforma. Veja [Controles na tela](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/touch-screen-controls/) para o componente completo.

Se o avatar também deve parar de se mover enquanto o painel estiver aberto, adicione um [modificador de input](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/avatares/locomotion/#freeze-the-player) por cima.

## Dimensionamento

A antiga regra prática — *projete no desktop e depois multiplique os tamanhos da UI por 3× para mobile* — é anterior a duas mudanças no SDK e não deve mais ser aplicada às cegas. A maior parte desse 3× agora é aplicada automaticamente para você:

* **`devicePixelRatio` não participa do layout da UI.** A UI dimensionada em pixels agora é renderizada `devicePixelRatio` vezes maior do que antes — normalmente **2–3× em um telefone**, e também em laptops Retina. Não há como desativar isso.
* **A tela virtual tem padrões diferentes por plataforma**, `1600x720` no mobile contra `1920x1080` no desktop e na web, então o mesmo valor em pixels já cobre cerca de 1,2× mais da tela em um telefone.

Comece com os tamanhos do desktop sem alterações, meça em um dispositivo real — veja [Pré-visualizar no mobile](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/preview-on-mobile/) — e aumente a escala apenas onde os alvos de toque ou o texto realmente ficarem pequenos demais. Ramifique com [`isMobile()`](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/detect-platform/) quando os dois realmente precisarem ser diferentes.

## Relacionado

* [Recursos ausentes](/docs/creator/content-creator-pt/criar-para-mobile/cliente-movel/missing-features/)
* [Área segura para dispositivos móveis](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/safe-area/)
* [Detectar a plataforma pelo código](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/detect-platform/)
* [UI na tela](/docs/creator/content-creator-pt/scenes-sdk7/ui-2d/onscreen-ui/)
* [Guia de UX & UI](/docs/creator/content-creator-pt/scenes-sdk7/conceber-a-experiencia/ux-ui-guide/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/criar-para-mobile/desenvolver/ui-best-practices.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
