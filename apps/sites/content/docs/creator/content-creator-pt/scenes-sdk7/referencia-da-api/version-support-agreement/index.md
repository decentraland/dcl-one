> Contrato de suporte de versão

# Contrato de Suporte de Versão

Este documento descreve a política de versionamento aplicada tanto ao Scene Editor no Creator Hub quanto ao framework de scripting (o `@dcl/sdk` pacote).

O objetivo desta política de versionamento é estabelecer um contrato entre a equipe de SDK da Decentraland Foundation e os criadores de conteúdo, para definir expectativas claras e permitir que os criadores de conteúdo planejem seu trabalho em conformidade.

## Definições

* *Número da versão* -- um identificador único para uma versão de software disponível publicamente. Esse identificador consiste em um *número da versão principal* e um *número da versão secundária* separados por pontos (por exemplo, 7.2).
* *Família de versões* -- todas as versões que têm a mesma versão principal formam uma família. Chamamos a versão A de *sucessora* da versão B se A e B pertencerem à mesma família e o número da versão secundária de A for maior que o de B (por exemplo, 7.11 é sucessora de 7.10).
* *Mudança que quebra compatibilidade* -- uma alteração que obriga um usuário a mudar seu código ou assets para mantê-los em estado funcional. Por exemplo, uma propriedade muda de nome e obriga o usuário a alterar esse nome de propriedade sempre que ele é usado em seu código.
* *Mudança sem quebra de compatibilidade* -- uma alteração que não exige nenhuma ação por parte do usuário. O comportamento e as propriedades do código e dos assets do usuário permanecem inalterados.
* *Versão estável* -- uma versão que proíbe mudanças que quebram compatibilidade em todos os seus sucessores (os quais também são considerados estáveis). Qualquer mudança que quebre compatibilidade deve ser introduzida apenas por meio da criação de uma nova família de versões, incrementando o número da versão principal; no entanto, veja a ressalva na seção de [mudanças que quebram compatibilidade](#breaking-changes) . Mudanças sem quebra de compatibilidade são refletidas pelo incremento do número da versão secundária.
* *Versão instável* -- uma versão que permite mudanças que quebram compatibilidade em seus sucessores. Mudanças que quebram compatibilidade podem ser introduzidas incrementando o número da versão secundária.

## Política de suporte

Em toda família de versões estáveis, a Decentraland Foundation oferece suporte apenas à versão secundária mais recente. Em qualquer momento, deve haver no máximo duas famílias com suporte.

Por exemplo, se a versão secundária mais recente da família 6.x for 6.11 e a versão secundária mais recente da família 7.x for 7.3, espera-se que os criadores de conteúdo desenvolvam suas scenes em 6.11 ou 7.3. As scenes que foram desenvolvidas e publicadas com a versão 6.10 ou anterior provavelmente continuarão funcionando, e os jogadores poderão aproveitá-las. No entanto, se essas scenes mais antigas apresentarem algum problema, elas devem primeiro ser atualizadas para uma versão com suporte, e o problema só será investigado se ainda ocorrer em uma versão com suporte.

O Scene Editor atualiza automaticamente o pacote de scripting de todas as scenes dentro da mesma família de versões, para que todos os desenvolvedores usem a versão com suporte mais recente ao desenvolver suas scenes.

## Desenvolvimento de funcionalidades

Novas funcionalidades serão lançadas apenas na versão mais recente em desenvolvimento. Assim que a equipe de desenvolvimento começar a trabalhar em uma nova família de versões, as famílias de versões mais antigas que ainda estiverem com suporte receberão apenas correções de bugs importantes, e nenhuma funcionalidade adicional será implementada.

## Mudanças que quebram compatibilidade

Mudanças que quebram compatibilidade devem ocorrer apenas em lançamentos principais. Não deve haver mudanças que quebrem compatibilidade dentro dos lançamentos secundários estáveis da mesma família de versões, exceto em caso de emergência, quando não houver outro meio de resolver o problema. Mudanças que quebram compatibilidade dentro de um lançamento secundário são uma medida drástica que os desenvolvedores evitarão a todo custo. Novos lançamentos secundários ampliarão as capacidades da sintaxe existente, mas nunca deverão alterar o que a sintaxe estabelecida produz, exceto ao corrigir bugs.

### Mudanças isoladas

Em ocasiões muito raras, pode ser preferível fazer uma pequena mudança isolada que quebre compatibilidade se ela incomodar apenas um pequeno subconjunto de usuários. (Criar uma nova versão principal incomoda todos os usuários.) Nesse caso, o SDK pode descontinuar uma funcionalidade, mas deve continuar a oferecê-la por um período razoável.

### Mudanças de emergência

Em certos casos excepcionais, como questões de segurança ou requisitos regulatórios, qualquer funcionalidade pode ser alterada de forma que quebre compatibilidade, independentemente do seu nível de estabilidade, e uma descontinuação não é prometida nessas situações.

## Lançamentos estáveis e instáveis

### Alpha

Sempre que uma nova versão principal for introduzida, alguns dos primeiros lançamentos secundários podem ser rotulados como instáveis **alpha** versões. Mudanças que quebram compatibilidade devem ser permitidas e esperadas em lançamentos alpha, e os usuários não devem ter nenhuma expectativa de estabilidade.

Os desenvolvedores são livres para experimentar com essas versões alpha, mas não são incentivados a publicar conteúdo criado com versões alpha instáveis, pois não há garantia de que o conteúdo continuará funcionando após mudanças subsequentes. Também não é recomendado iniciar grandes migrações complexas neste momento, já que mais mudanças podem ser necessárias antes do próximo lançamento estável.

### Beta

Assim que uma família de versões atinge certo nível de maturidade, um **beta** lançamento é disponibilizado. Um lançamento beta é considerado concluído e pronto para ser declarado estável, sujeito a testes públicos.

As famílias de versões beta devem ser o mais estáveis possível; no entanto, é permitido que mudem ao longo do tempo. Essas mudanças devem ser mínimas, mas podem incluir mudanças que quebram compatibilidade. Mudanças que quebram compatibilidade só podem ser introduzidas após um período razoável de descontinuação, para dar aos criadores de conteúdo a oportunidade de migrar suas scenes. Esse período de descontinuação deve ser definido quando a mudança que quebra compatibilidade for introduzida.

As famílias de versões beta devem permanecer em beta apenas por um período limitado, especificado no momento em que são marcadas como beta. Elas devem ser promovidas para estáveis se nenhum problema for encontrado nesse período. A duração desse período pode variar caso a caso, mas uma boa regra prática é 90 dias.

O conteúdo escrito com a sintaxe de uma versão beta deve ser fácil de migrar para versões posteriores dentro dessa família. Ainda não é aconselhável desenvolver conteúdo para eventos importantes com uma versão beta, pois os testes ainda estão em andamento e é provável que haja bugs.

### Estável

Assim que o período beta termina sem problemas significativos, a família de versões é considerada **estável** e não deve haver outras mudanças na sintaxe, além da adição de novas funcionalidades. A partir desse ponto, a versão é considerada a opção recomendada e incentivada para todos os desenvolvedores usarem.

Uma família de versões estáveis deve receber suporte total durante todo o seu ciclo de vida. Não deve haver mudanças que quebrem compatibilidade, sujeitas às ressalvas descritas abaixo.

## Funcionalidades instáveis em lançamentos estáveis

Funcionalidades específicas em um lançamento podem ter níveis de estabilidade diferentes do lançamento como um todo. Isso pode ocorrer porque a funcionalidade foi introduzida recentemente e requer mais testes, ou porque está destinada a ser substituída em breve.

Por exemplo, um novo tipo de componente poderia ser introduzido como alpha em um lançamento do framework do SDK que já é estável, já que esse componente específico ainda pode precisar do seu próprio ciclo de testes. Ele pode então passar pelo fluxo de versionamento descrito acima, indo de alpha para beta e depois para estável.

Qualquer funcionalidade de um lançamento estável que não seja considerada estável deve ser claramente identificada como tal na documentação. Os criadores que fizerem uso de funcionalidades instáveis devem estar cientes de que a funcionalidade pode passar por mudanças que quebram compatibilidade. Quaisquer mudanças que quebrem compatibilidade serão comunicadas claramente, incluindo guias de migração, e haverá um período de transição para que os criadores ajustem o código de sua scene.

## Por quanto tempo oferecemos suporte a uma família de versões estáveis?

Assim que uma nova família de versões se torna estável (7.x), a equipe se compromete a oferecer suporte (correções de bugs importantes) à versão anterior (6.x) por vários meses, para dar aos criadores bastante tempo para migrar. A duração é determinada caso a caso, dependendo do esforço de migração necessário.

## Versões pré-lançadas

É sempre possível acessar as adições mais recentes ao framework de scripting instalando a `@next` versão do `@dcl/sdk` pacote em uma scene.

As funcionalidades neste branch podem ser instáveis ou não documentadas, pois não são disponibilizadas como parte de uma versão do SDK oficialmente com suporte.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/referencia-da-api/version-support-agreement.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
