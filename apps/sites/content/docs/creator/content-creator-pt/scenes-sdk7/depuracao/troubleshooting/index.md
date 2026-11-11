> Correções para problemas comuns

# Solução de Problemas

## Depurar com um assistente de IA

Antes de investigar os problemas abaixo, considere passar o problema para um assistente de IA, como o chat do Cursor, o GitHub Copilot ou o Claude Code. Cole a mensagem de erro do console, ou descreva o que não está se comportando como esperado, e ele normalmente consegue encontrar o problema no código da sua scene e corrigi-lo para você.

Para obter bons resultados, certifique-se de que o assistente de IA tenha as skills do Decentraland SDK instaladas. As skills ensinam os padrões e as restrições do SDK, para que ele não adivinhe com base em informações genéricas ou desatualizadas. Instale-as executando este comando no projeto da sua scene:

```bash
npx skills add decentraland/sdk-skills
```

Veja [Vibe Coding com IA](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/vibe-coding/) para mais detalhes sobre a configuração e a forma de fornecer prompts a assistentes de IA.

Você também pode deixar a IA depurar a scene *enquanto ela está em execução*. O cliente desktop do Decentraland pode expor um servidor MCP que permite que um agente tire suas próprias capturas de tela, leia a saída do console da scene, mova o player e clique em objetos — então, em vez de você reproduzir o bug e colar o erro, o agente o reproduz por conta própria, vê o que acontece e itera até corrigir. Inicie a scene com `npm run start -- --mcp` e conecte seu agente a ele.

Veja [Deixe a IA ver sua cena no mundo](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/vibe-coding/#let-the-ai-see-your-scene-in-world) para a configuração completa, e instale a skill `unity-explorer-mcp` para que o agente conheça o fluxo de trabalho:

```bash
npx skills add decentraland/sdk-skills --skill unity-explorer-mcp
```

## Problemas ao executar a preview

#### Problema: Não consigo executar nenhuma preview da scene, a mensagem de erro menciona **Permissões negadas** ou **EACCES**

Seu sistema operacional não permite editar permissões na pasta onde você quer executar o projeto. Ao executar a scene, algumas dependências precisam ser instaladas, mas isso é proibido. Você precisa configurar as permissões da pasta para permitir que a conta de usuário do Windows/Mac/Linux possa editar os arquivos nela.

Recursos úteis:

* [docs.npmjs](https://docs.npmjs.com/resolving-eacces-permissions-errors-when-installing-packages-globally)
* [letscodepare](https://letscodepare.com/blog/npm-resolving-eacces-permissions-denied)

#### Problema: Não consigo executar a preview de uma scene específica, o erro diz **Erro: Erro ao construir o projeto**

Se você estiver executando uma scene que foi compartilhada com você, certifique-se de que essa scene não foi compartilhada contendo uma pasta `node_modules` ou `bin` ou um arquivo `package-lock.json` . Esses arquivos contêm dependências que usam versões específicas do seu sistema operacional e da sua máquina; eles devem ser gerados ao executar a scene pela primeira vez. Exclua essas pastas e esse arquivo manualmente e, depois, execute `npm run start` novamente.

#### Problema: Executando `npm run start` é executado, não há mensagem de erro, mas nenhuma janela do navegador é aberta e não há nenhuma URL na saída para abrir a preview

Certifique-se de que sua versão do Node está atualizada. Ela deve ser 20 ou superior.

#### Problema: Executando `npm run start` abre uma aba do navegador, mas a tela de carregamento nunca termina de carregar, ou vejo um banner vermelho de erro que diz "critical error".

* Certifique-se de ter a versão mais recente do Decentraland SDK instalada no seu projeto. Execute:

  `npm i @dcl/sdk@latest`

#### Problema: A scene é executada, no console eu vejo `dependências cíclicas` avisos.

Eles se referem a arquivos na sua scene que se referenciam mutuamente. Isso não é necessariamente um problema, mas não é um padrão recomendável para escrever software, pois pode levar a condições de corrida difíceis de depurar e a outros problemas. É provável que sua scene funcione bem apesar desses avisos.

Idealmente, o carregamento do código da sua scene deve seguir uma ordem sequencial clara. Código que tenha dependências cíclicas pode sofrer de um problema de ovo e galinha, em que o compilador não sabe qual iniciar primeiro. Muitas vezes isso é resolvido sem problemas, mas é uma boa prática evitar.

Para corrigir essas dependências, muitas vezes você precisa recorrer a chamar funções ou construtores de objetos, passando referências a entities/objects já instanciados nos argumentos da função; em vez de codificar de forma fixa referências a essas entities/objects na função, que podem ou não já estar instanciados.

## Problemas ao fazer deploy

#### Problema: Você não tem permissão para fazer deploy nestes parcels

* Certifique-se de que o `scene.json` arquivo da sua scene lista corretamente as coordenadas onde você quer fazer o deploy.
* Certifique-se de que o Metamask esteja configurado corretamente para usar a wallet certa para assinar a transação. Pode ser a wallet que possui os tokens LAND, ou pode ter permissões de operador concedidas pelo proprietário.

#### Problema: Executando `npm run deploy` falha

* Verifique os spawn points da sua scene; as três coordenadas x, y, z de um spawn point devem ser um número ou um intervalo. Ou todas as três são números ou todas as três são intervalos. Não é suportado ter intervalos para algumas e números para outras.

  Por exemplo, isto não é suportado:

  `"position": {"x": [1,4], "y": 0, "z": [1,4]}`

  Isto é suportado:

  `"position": {"x": [1,4], "y": [0,0], "z": [1,4]}`
* O servidor catalyst padrão para o qual você foi designado para fazer deploy pode estar fora do ar ou com problemas. Você pode forçar o `npm run deploy` comando para fazer deploy para um servidor catalyst específico. Para fazer deploy para um servidor específico no Decentraland Editor:

  1. Abra sua scene e clique **Publicar**
  2. Selecione a opção **Publicar para um servidor diferente** na parte inferior.
  3. No dropdown, selecione **Servidor Personalizado**
  4. Insira o endereço do servidor, por exemplo `peer-ec1.decentraland.org`
  5. Clique em **Publicar para servidor personalizado**
  6. Aprove a transação como em um deploy normal.

  Para fazer isso via CLI:

  `npm run deploy -- --target-content <nome-do-servidor>`

  Por exemplo:

  `npm run deploy -- --target-content peer-ec1.decentraland.org`

  Veja [catalyst-monitor](https://decentraland.github.io/catalyst-monitor/) para verificar o status de todos os servidores na rede catalyst. Você também pode copiar os endereços de cada um, no topo de cada cartão.
* Verifique o `package.json`da sua scene. Um problema comum é haver uma seção `bundleDependencies` e também uma seção `bundledDependencies` (d extra). Isso às vezes pode resultar da execução de diferentes versões do Node no mesmo projeto em momentos diferentes, ou do compartilhamento do projeto entre pessoas que o executaram com diferentes versões do Node instaladas. Exclua `bundleDependencies`, que se relaciona a versões mais antigas do Node.

Certifique-se também de que sua versão do Node esteja atualizada, pelo menos a versão 20.

#### Problema: Executando `npm run deploy` ou `npm run build` relata erros de tipo

Sua scene pode ter erros de tipo relatados pelo TypeScript, por exemplo dizendo que uma certa variável pode ser do tipo `qualquer` ou que `undefined` ou `null` não são permitidos. Ao executar `npm run deploy`, ele também executa `npm run build`, que é um pouco mais rigoroso com essas verificações do que `npm run start`.

Ao contrário do JavaScript, o TypeScript impõe tipagem estrita de todas as variáveis. Mesmo que sua scene esteja escrita de tal forma que, por exemplo, um certo valor nunca será `undefined`, o TypeScript precisa saber o que aconteceria nesse cenário, ou você precisa esclarecer explicitamente que o valor só pode ser, por exemplo, uma string.

Como alternativa, você pode executar `npm run deploy -- --skip-build` para pular a execução de `npm run build`e impedir que essas verificações sejam executadas.

#### Problema: Fiz deploy da minha scene, mas não vejo as mudanças quando entro no Decentraland

* Lembre-se de que isso pode levar alguns minutos se a scene contiver assets pesados.
* Enquanto os modelos 3D da nova versão estão sendo convertidos em asset bundles, os players recebem deliberadamente a última versão totalmente funcional da sua scene. Isso normalmente leva apenas alguns segundos, mas pode demorar mais em scenes muito grandes ou quando os servidores estão ocupados. Você pode [verificar o status da conversão](/docs/creator/content-creator-pt/scene-editor/publicar/publish-scene/#check-the-conversion-status) da sua scene diretamente.
* Recarregar a scene não é suficiente para obter uma nova versão: um recarregamento reinicia o código da scene, mas não busca a versão recém-publicada. Assim que a conversão estiver concluída, feche completamente e abra novamente o Decentraland e, depois, entre na scene novamente por meio de um link de salto ou do `/goto` comando de chat.

#### Problema: Alguns players veem a nova versão da minha scene, outros ainda veem a antiga

A conversão dos modelos 3D da sua scene termina em momentos diferentes para cada plataforma (Windows e Mac), e o cliente de cada player também mantém um cache local. Para verificar se a conversão foi concluída para todas as plataformas, [verificar o status da conversão](/docs/creator/content-creator-pt/scene-editor/publicar/publish-scene/#check-the-conversion-status) e compare os `windows` e `mac` valores em `assetBundles`. Quando ambos estiverem `concluídos`, peça aos players afetados que fechem completamente e abram novamente o Decentraland.

#### Problema: A publicação ficou presa na etapa de Converting, o botão Jump In nunca aparece

Sua scene está na fila atrás de outras scenes sendo convertidas, ou a conversão falhou.

* Open `https://asset-bundle-registry.decentraland.org/queues/status` e procure pelo ID da entity da sua scene. Você pode encontrar esse ID no campo `entityId` quando você [verificar o status da conversão](/docs/creator/content-creator-pt/scene-editor/publicar/publish-scene/#check-the-conversion-status) usando as coordenadas da sua scene. Se o seu ID estiver listado na fila, a scene está na fila de espera e você só precisa aguardar.
* Se ele não estiver na fila, [verificar o status da conversão](/docs/creator/content-creator-pt/scene-editor/publicar/publish-scene/#check-the-conversion-status) da sua scene. Se uma plataforma mostrar `failed`, publique a scene novamente. Se falhar uma segunda vez, [relate o problema](/docs/creator/content-creator-pt/scenes-sdk7/depuracao/report-bug/) e inclua o ID da entity.

#### Problema: Depois de fazer deploy, alguns modelos 3D estão ausentes, pretos ou sem textura

* Se você acabou de publicar, a conversão dos seus modelos em asset bundles talvez ainda não tenha terminado. Digite `/detectabs` na janela de chat: modelos com tonalidade vermelha ainda não foram convertidos. [Verifique o status da conversão](/docs/creator/content-creator-pt/scene-editor/publicar/publish-scene/#check-the-conversion-status) e aguarde até que ela seja concluída.
* Certifique-se de que os modelos 3D estejam todos dentro dos limites da scene, inclusive suas bounding boxes. Se qualquer parte dos seus modelos se estender além desses limites ao executar uma preview, essas partes que ultrapassarem serão cortadas e não serão renderizadas, tanto ao executar uma preview quanto na scene publicada.
* Se as texturas forem o problema, lembre-se de que as texturas em modelos 3D têm tamanho máximo de 1024x1024 pixels durante a conversão.

#### Problema: A scene fica boa de perto, mas está quebrada ou ausente quando vista de longe

As versões de baixo Level of Detail (LOD) dos seus assets, usadas para renderizar a scene de longe, são geradas na última etapa da publicação e talvez ainda não tenham terminado. Isso não bloqueia testar sua scene de perto. [Verifique o status da conversão](/docs/creator/content-creator-pt/scene-editor/publicar/publish-scene/#check-the-conversion-status) e veja os valores em `lods`, ou simplesmente espere e verifique novamente mais tarde.

#### Problema: Depois de fazer deploy, meus modelos 3D parecem diferentes

* Se as texturas parecerem diferentes, lembre-se de que as texturas em modelos 3D têm tamanho máximo de 1024x1024 pixels. Essa conversão é feita para garantir que o Decentraland funcione de forma suave para todos.
* Se os modelos parecerem diferentes, pode haver um problema com a conversão dos modelos em asset bundles. Leia mais sobre a compressão de asset bundle [aqui](/docs/creator/content-creator-pt/scenes-sdk7/otimizacao/performance-optimization/#asset-bundle-conversion).

  Para validar isso, tente executar a scene com o parâmetro de URL `&DISABLE_ASSET_BUNDLES`. Se os modelos ficarem bons com essa flag, o problema deve estar relacionado a um bug na conversão do modelo. Nesse caso, [relate o problema](/docs/creator/content-creator-pt/scenes-sdk7/depuracao/report-bug/) e inclua o ID da entity do seu deploy.

  Observe que a geração de versões comprimidas de asset-bundle dos seus modelos leva um pouco de tempo aos servidores (normalmente alguns minutos, mais tempo em scenes muito grandes ou quando os servidores estão ocupados). Você pode verificar se os modelos estão sendo carregados como asset bundles comprimidos ou não escrevendo o seguinte comando na janela de chat `/detectabs`. Modelos comprimidos ficam com tonalidade verde, os não comprimidos ficam com tonalidade vermelha.

  Você também pode reproduzir essa conversão localmente antes de publicar, ativando [assets otimizados na preview](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/preview-scene/#preview-with-optimized-assets). Isso permite que você encontre e depure esses problemas sem precisar fazer deploy da scene.

#### Problema: Minha scene desapareceu por completo

* Se a scene estiver em um World: seu orçamento de armazenamento do Worlds pode ter sido excedido, por exemplo após vender ou transferir NAMEs, LAND ou MANA. Você tem 48 horas para liberar espaço ou aumentar seu orçamento antes que seus Worlds se tornem inacessíveis. Verifique seu orçamento na aba **Gerir** do Creator Hub ou no **Worlds** separador do [Builder](https://decentraland.org/builder/worlds), e depois republique. Veja [limites de tamanho dos Worlds](/docs/creator/content-creator-pt/scenes-sdk7/tipos-de-projetos/kinds-of-project/#size-limits).
* Se a scene estiver em LAND: alguém com permissões de deploy pode ter publicado uma nova scene sobre seus parcels, o que apaga o conteúdo anterior. Veja [sobrescrita de scene](/docs/creator/content-creator-pt/scenes-sdk7/publicacao/publishing/#scene-overwriting).
* Em casos raros, conteúdo que viole a política de conteúdo do Decentraland pode ser colocado em denylist nos servidores de conteúdo. Se você acredita que isso aconteceu por engano, entre em contato por meio do [Discord da Decentraland](https://decentraland.org/discord).

#### Problema: Minha scene tem FPS baixo em produção, mesmo funcionando sem problemas na preview.

O desempenho da sua scene pode ser afetado por scenes vizinhas que seguem más práticas, já que elas também são executadas em paralelo. Você pode confirmar se esse é o caso abrindo as configurações e definindo a linha de visão para o mínimo, para que apenas 1 parcel ao redor da sua scene atual seja carregado.

Você pode reduzir ainda mais a linha de visão executando sua scene com o parâmetro `&LOS=0`, para não carregar nenhuma scene ao redor.

Se você acabou de fazer deploy da sua scene, o peso ao carregar a scene também pode ser reduzido assim que os servidores converterem os modelos 3D da scene em asset bundles comprimidos. Você pode verificar se os modelos estão sendo carregados como asset bundles comprimidos ou não escrevendo o seguinte comando na janela de chat `/detectabs`. Modelos comprimidos ficam com tonalidade verde, os não comprimidos ficam com tonalidade vermelha.

### Relatar um bug

Se você encontrar um problema que não seja com a sua cena, mas sim com o Decentraland SDK em geral, consulte [Relatar um bug](/docs/creator/content-creator-pt/scenes-sdk7/depuracao/report-bug/).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/depuracao/troubleshooting.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
