> Projete visualmente a UI na tela da sua scene no Creator Hub, em um canvas em vez de no código.

# Editor de UI

O UI Editor permite que você crie a interface na tela da sua scene arrastando widgets para um canvas, em vez de escrever o código de layout manualmente. Ele grava arquivos reais na sua scene, então tudo o que você cria aqui é UI normal do SDK7 que você pode continuar editando em código.

## Abra-o

Use a **2D** / **3D** alternador no topo do painel esquerdo do editor. **3D** é o canvas da scene que você já conhece. **2D** é o UI Editor.

![O alternador das tabs 2D e 3D, com 3D selecionado.](/docs/_remote/4e696bf6f0bb7532d974ec02.webp)


> [!WARNING]
> **📔 Nota**: O UI Editor precisa de `@dcl/sdk` da versão 7.26.0 ou mais recente na sua scene. Em uma scene mais antiga, a **2D** tab mostra um **UI Editor indisponível** aviso em vez do canvas, com um **Atualizar SDK** botão que atualiza a scene para você e um **Talvez mais tarde** botão para dispensá-lo.


O Creator Hub lembra se você deixou uma scene em 2D ou 3D e a reabre da mesma forma. Isso é armazenado por projeto e nunca é publicado junto com sua scene.

Se você estiver usando o renderizador de scene Bevy, mudar para **2D** pausa a scene em execução, e voltar para **3D** retoma-a se ela estava em execução antes.

## Crie sua primeira UI

1. Mude para a **2D** tab.
2. Clique em **New GUI**, seja o botão no centro do canvas vazio ou o **+** ao lado de **GUIs** no painel esquerdo. Uma GUI é um componente de UI, armazenado em seu próprio arquivo.
3. Arraste um widget da paleta na parte inferior para o canvas. **Full Screen** é um bom primeiro widget; ele cria uma raiz que cobre a tela. Depois, solte outros widgets dentro dele.
4. Selecione um widget e defina suas propriedades no painel direito.

Não há botão de salvar. O editor grava suas alterações em disco à medida que você trabalha, e o selo à esquerda da barra de ferramentas mostra **Todas as alterações salvas**.

![O UI Editor no modo 2D: os painéis GUIs e Nodes à esquerda listam uma GUI chamada MainUI com um Container contendo uma Label e um Button, o canvas no meio mostra o container com o Button selecionado e alças de redimensionamento ao redor dele, a aba Properties à direita mostra a posição e o layout do Button, e a paleta de widgets ocupa a parte inferior.](/docs/_remote/3958b77d4c214591522f3386.webp)

## Os painéis

* **Esquerda**: **GUIs**, a lista dos seus componentes de UI, e **Nodes**, a árvore do nó selecionado. Uma caixa de busca filtra ambos. Passe o mouse sobre um nó para bloquear, ocultar ou excluir.
* **Direita**: um **Properties** aba para o nó selecionado, e um **Logic** tab.
* **Parte inferior**: a paleta de widgets. Arraste um card para o canvas para adicioná-lo.
* **Centro**: o canvas. Arraste nós para movê-los, use as alças para redimensionar e navegue livremente com a roda do mouse ou arrastando.

Dois grupos flutuantes de controles ficam no canto inferior direito do canvas:

* O **preview** grupo alterna os guias do HUD mobile e os guias da área segura, e alterna entre **Desktop preview** e **Mobile preview**.
* O **zoom** grupo tem um botão de menos e um de mais, e um campo de porcentagem. Clique na porcentagem para digitar um nível exato de zoom entre 10 e 200; depois pressione Enter ou clique fora para aplicá-lo, ou Escape para cancelar. O botão de mais desativa quando você atinge 200%.

<div align="left"><img src="/docs/_remote/589a76a30cb6467ad0c864b3.webp" alt="O painel esquerdo do UI Editor, com as tabs 2D e 3D, uma caixa de busca, a lista de GUIs contendo MainUI, e a árvore Nodes mostrando um Container com uma Label e um Button, com a linha do Button mostrando ícones de bloquear, ocultar e excluir." width="220"> <img src="/docs/_remote/e71a2f9dd042f80664595bb6.webp" alt="A aba Properties do UI Editor para um Button selecionado, com Visibility, Interaction States, uma seção Position com Constraints, Position e Z-Index, e uma seção Layout com Size, Min Size, Max Size, Padding e Margin." width="360"></div>

## Widgets

![A paleta de widgets ao longo da parte inferior do UI Editor, com cards para Full Screen, Container, Image, Label, Button, Input e Dropdown.](/docs/_remote/353fd7589685b5f5d6abd5f2.webp)

A paleta agrupa widgets em três categorias:

* **Containers**: **Full Screen**, **Container**, **Image**
* **Text**: **Label**, **Button**
* **Input**: **Input**, **Dropdown**

**Full Screen** é um container que preenche tudo em que é colocado, o que o torna um bom ponto de partida para uma UI que deve cobrir a tela.

## Movimentação e redimensionamento

A barra de ferramentas tem três modos de ferramenta:

* **Livre**: mover e redimensionar, o modo de uso geral.
* **Mover**: apenas arrastar.
* **Redimensionar**: apenas alças.

As posições se ajustam a uma grade de 10 pixels. Segure **Shift** enquanto arrasta para mover livremente, ignorando a grade. A pequena seta ao lado dos botões da ferramenta abre uma **Snap** caixa de seleção que desativa totalmente o ajuste.

![A barra de ferramentas 2D, com o selo Todas as alterações salvas, desfazer e refazer, as ferramentas Free, Move e Resize, e o dropdown de snap aberto mostrando uma opção Snap marcada, seguido pelos botões de play e stop.](/docs/_remote/5ab6e1170c733fa0011bba2b.webp)

## Layout: fluxo e posicionamento livre

Duas propriedades separadas decidem onde um nó termina. Entender isso evita muita confusão.

**Flow** é definido em um **container** e decide como ele organiza seus filhos:

* **Livre**: os filhos ficam onde você os colocou.
* **Row** / **Row reverse**: os filhos são organizados da esquerda para a direita, ou da direita para a esquerda.
* **Column** / **Column reverse**: os filhos são empilhados de cima para baixo, ou de baixo para cima.
* **Wrap children**: permite que os filhos passem para mais de uma linha quando não couberem.

**Ignore Layout Flow** é definido em um **child** e decide se esse nó opta por sair do arranjo do pai. Desligado, o pai o posiciona. Ligado, você o fixa manualmente com valores de Anchor e Position.

Duas consequências decorrem disso:

* Solte um widget em um **Livre** container e ele permanece exatamente onde você o soltou.
* Solte-o em um container com uma direção e ele entra no fluxo, ocupando seu lugar na linha.

Mudar um container para **Livre** fixa os filhos existentes onde eles já estão, então nada salta.

A raiz de uma GUI é sempre posicionada livremente.

## Outras propriedades que vale a pena conhecer

* **Opacity**: 100% é totalmente opaco, 0% totalmente transparente. O padrão é 100%.
* **Scene Inset**: em que parte da tela a GUI fica.

  * **Full Screen** usa toda a tela renderizável.
  * **Interactable Safe Area** permanece fora da interface do próprio cliente, como chat, o minimapa e os indicadores do HUD.
  * **Device Safe Area** também evita obstruções físicas como um notch ou barras do sistema. Esta opção é oferecida quando o canvas está em preview mobile.

  No código que o editor escreve, essas três opções se tornam `screenInset: 'none'`, `'interactable'` e `'device'`. Veja [Área de inset da tela](/docs/creator/content-creator-pt/scenes-sdk7/ui-2d/onscreen-ui/#screen-inset-area).

Se você der à raiz de uma GUI uma largura e uma altura em pixels fixos, o canvas a enquadra como um artboard exatamente desse tamanho, em vez de como uma tela.

## Verifique sua UI em um telefone

Os controles no canto inferior direito do canvas alternam a pré-visualização entre uma tela de desktop e um telefone. No preview mobile, o canvas é desenhado dentro de uma moldura de telefone, completa com um notch, para que você veja onde as bordas da tela realmente ficam.

Dois conjuntos de guias ajudam você a posicionar as coisas onde os jogadores podem vê-las e alcançá-las:

* **Guias da área segura** contornam a parte da tela à qual sua GUI fica confinada, correspondendo à **Scene Inset** que você escolheu. Enquanto o Scene Inset é **Device Safe Area** ou **Interactable Safe Area** o contorno está sempre ligado, porque é a ele que a GUI é recortada. Defina o Scene Inset como **Full Screen** e você pode ligar ou desligar o contorno com o botão do ícone de moldura.
* **Guias do HUD** desenham os controles do próprio cliente mobile, como o joystick, os botões de pular e ação, a roda de emotes, o perfil, o chat e a bússola. Eles servem apenas como referência; você não pode selecioná-los nem movê-los. O botão do controle de jogo os liga e desliga, ele só aparece no preview mobile, e os guias aparecem por padrão quando o Scene Inset é uma área segura.

O conteúdo colocado fora da área segura não fica oculto; ele é desenhado além do contorno. Esse overflow é o aviso: no cliente real, ele ficaria sob o próprio HUD do jogo.

**Interactable Safe Area** limpa a coluna esquerda de controles do cliente mobile, mas compartilha sua borda direita com a área do dispositivo, então os botões de ação no canto inferior direito são desenhados sobre ela por design. Mantenha esses cantos livres. Veja [Onde ficam os controles do cliente](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/safe-area/#where-the-client-controls-live).


> [!WARNING]
> **📔 Nota**: estes guias são apenas uma aproximação para o editor. As áreas reais são reportadas pelo cliente em runtime e variam conforme o dispositivo, então sempre confirme em um telefone real. Veja [Pré-visualizar no mobile](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/preview-on-mobile/).


## Personalize os controles mobile

No cliente mobile, os jogadores se movem e agem por meio de um conjunto de controles nativos na tela: um joystick, uma mira e uma fileira de botões de ação. A **MobileHUD** entrada permite remodelar esse HUD sem escrever código, e mostra o resultado no canvas à medida que você trabalha.

Assim que sua scene tiver pelo menos uma GUI, **MobileHUD** aparece como a primeira linha da **GUIs** lista, acima das suas próprias GUIs. Selecione-a para abrir seu painel à direita.

Três caixas de seleção no topo ocultam partes inteiras do HUD:

* **Ocultar joystick**: remove o stick de movimento.
* **Ocultar crosshair**: remove a mira.
* **Ocultar Input Actions**: remove todos os botões de ação de uma vez.

Abaixo delas, a **Input Actions** seção lista os oito botões, na ordem em que ocupam os espaços na tela:

| Ação                                  | Botão na tela                      |
| ------------------------------------- | ---------------------------------- |
| `IA_JUMP`                             | O grande botão central, por padrão |
| `IA_POINTER`                          | O botão de interação               |
| `IA_PRIMARY (E)`                      | O botão E                          |
| `IA_SECONDARY (F)`                    | O botão F                          |
| `IA_ACTION_3 (1)` a `IA_ACTION_6 (4)` | Os botões numerados                |

Cada linha lhe dá três coisas:

* **Principal**: um botão de opção que torna essa ação o grande botão central. `IA_JUMP` é a Principal por padrão, e apenas uma ação pode ser Principal por vez.
* Uma chamada de **ícone de olho** que mostra ou oculta esse botão. Quando você oculta um, os botões abaixo sobem para preencher a lacuna.
* **Ícone personalizado**: substitui o símbolo do botão por uma `.png`, `.jpg` ou `.jpeg` imagem da sua scene.

Neste painel, cada botão começa visível e `IA_JUMP` é o botão principal.


> [!WARNING]
> **📔 Nota**: Uma scene que nunca toca no HUD mostra apenas três botões: pular, ponteiro e principal. O painel só escreve `src/mobile-hud.ts` uma vez que você muda algo, então um painel intocado mantém esse padrão. Assim que o arquivo existe, seu código substitui o conjunto padrão por tudo o que você configurar aqui; portanto, ocultar apenas o joystick, por exemplo, também coloca os cinco botões extras na tela do jogador. Oculte os que sua scene não usa. Veja [O que aparece por padrão](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/touch-screen-controls/#what-shows-by-default).



> [!NOTE]
> **💡 Dica**: Com mais de cinco botões visíveis, os últimos ficam atrás de uma alternância de overflow "+". Oculte botões suficientes dos mais altos e o restante aparece diretamente. Veja [Controles na tela](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/touch-screen-controls/#how-the-button-layout-works) como os slots se preenchem.


### O arquivo que ele escreve

Suas alterações são gravadas em `src/mobile-hud.ts`, que exporta uma `setupMobileHud()` função construída sobre o [`TouchScreenControls`](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/touch-screen-controls/) component:

```ts
import { engine, InputAction, TouchScreenControls } from '@dcl/sdk/ecs'

export function setupMobileHud() {
  TouchScreenControls.createOrReplace(engine.RootEntity, {
    hideJoystick: true,
    hideCrosshair: false,
    mainAction: InputAction.IA_PRIMARY,
    touchInputs: [
      { inputAction: InputAction.IA_ACTION_3, hide: true },
    ],
  })
}
```

O editor também adiciona a importação e a chamada ao seu `src/index.ts`:

```ts
import { setupMobileHud } from './mobile-hud'

setupMobileHud()
```

O arquivo só existe enquanto tem algo a dizer. Se você voltar tudo ao padrão, ou excluir sua última GUI, o Creator Hub apaga `src/mobile-hud.ts` e remove essas duas linhas novamente.


> [!WARNING]
> **📔 Nota**: Enquanto **MobileHUD** está selecionado, a paleta de widgets fica desativada e os controles de preview ficam ocultos, já que não há nada para posicionar e o HUD é apenas mobile. Selecione uma de suas próprias GUIs para recuperá-los.


## Onde os arquivos vão

O UI Editor edita os arquivos-fonte reais da sua scene. Não há um formato salvo separado.

* Cada GUI é um `.tsx` arquivo em `src/ui/`.
* `src/ui/index.tsx` é gerado pelo editor para reuni-los. Não o edite manualmente.
* `src/mobile-hud.ts` abriga seus [controles mobile](#customize-the-mobile-controls), se você alterou algum. Ele fica fora de `src/ui/` de propósito, para nunca ser tratado como uma GUI.

Se sua scene tiver um único `src/ui.tsx` arquivo mais antigo, o editor faz um backup dele como `src/ui.tsx.bak` na primeira vez que você abre o UI Editor.

Como estes são arquivos comuns, você pode abri-los no seu [editor de código](/docs/creator/content-creator-pt/scene-editor/expandir-com-codigo/overview/) a qualquer momento. Veja [UI](https://github.com/decentraland/docs/tree/main/creator/sdk7/2d-ui/README.md) para saber o que o código subjacente faz.

## Código que o editor não consegue editar

O editor entende uma forma específica de código de UI. Um node escrito de outra maneira, por exemplo construído dentro de um loop ou de uma condicional, ou usando um componente personalizado ou spread props, é desenhado no canvas como um bloco cinza com um **selo de não padrão, edite em código** . Seus children não são mostrados, e você o edita em código em vez disso.

O resto da GUI ao redor continua funcionando normalmente.

## Páginas relacionadas

* [UI](https://github.com/decentraland/docs/tree/main/creator/sdk7/2d-ui/README.md): escrevendo UI da scene em código.
* [Combinar com código](/docs/creator/content-creator-pt/scene-editor/expandir-com-codigo/overview/): abra sua scene em um editor de código.
* [Entities and Components](/docs/creator/content-creator-pt/scene-editor/construir/components/): o lado 3D do editor.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scene-editor/construir/ui-editor.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
