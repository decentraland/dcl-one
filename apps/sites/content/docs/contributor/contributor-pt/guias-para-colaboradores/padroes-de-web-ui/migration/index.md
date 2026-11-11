# Guia de migração

Este guia aborda o processo de migração de componentes de UI1 (`decentraland-ui`) para UI2 (`decentraland-ui2`).

## Quando migrar

As migrações podem começar por três motivos válidos:

### 1. Melhoria técnica

Você quer migrar um componente para:

* Melhor suporte a temas
* Tipos TypeScript aprimorados
* Consistência com outros componentes UI2
* Otimização de desempenho
* Melhorias de acessibilidade

### 2. Atualização de um componente

* Um componente UI1 precisa ser atualizado
* Ainda não existe equivalente em UI2
* **Requisito**: Crie-o primeiro em UI2 e depois use-o

### 3. Necessidades do projeto

* O componente será usado em um projeto novo ou existente
* Há tempo disponível para migrá-lo corretamente
* Os recursos do projeto permitem uma migração completa


> [!WARNING]
> **Não** migre componentes "só porque sim". Cada migração deve ter uma justificativa técnica ou de negócios clara.


***

## Processo de migração

### Passo 1: Planejamento

Antes de iniciar a migração:

1. **Identifique as dependências**
   * Quais outros componentes ele usa?
   * Quais projetos o usam atualmente?
   * Há alguma mudança que quebre compatibilidade planejada?
2. **Revise o uso atual**
   * Quantos projetos usam este componente?
   * Quais props são usadas com mais frequência?
   * Há algum problema conhecido?
3. **Defina o escopo**
   * Será uma migração 1:1?
   * Há melhorias planejadas?
   * Qual é o cronograma?

### Passo 2: Criar o componente UI2

Siga o [Componentes Personalizados](/docs/contributor/contributor-pt/guias-para-colaboradores/padroes-de-web-ui/custom-components/) guia para componentes candidatos a UI2.

**Requisitos:**

* Use sintaxe de objeto para styled-components
* Use apenas valores do tema (sem valores arbitrários)
* Adicione stories completas no Storybook
* Escreva testes completos
* Documente todas as props e comportamentos

**Estrutura de exemplo:**

```
ui2/src/components/Button/
├── Button.tsx
├── Button.styles.ts
├── Button.stories.tsx
├── Button.test.tsx
├── types.ts
├── index.ts
└── README.md
```

### Passo 3: Manter a compatibilidade

O componente UI2 **DEVEM** deve expor as mesmas props e comportamentos que a versão UI1.

#### Mesmas props

```tsx
// Button do UI1
interface ButtonProps {
  primary?: boolean;
  size?: 'small' | 'medium' | 'large';
  onClick?: () => void;
  disabled?: boolean;
  children: React.ReactNode;
}

// Button do UI2 - DEVE oferecer suporte às mesmas props
interface ButtonProps {
  primary?: boolean;
  size?: 'small' | 'medium' | 'large';
  onClick?: () => void;
  disabled?: boolean;
  children: React.ReactNode;
  // Pode adicionar novas props opcionais
  variant?: 'text' | 'outlined' | 'contained';
}
```

#### Alterações compatíveis com versões anteriores

Se você precisar alterar ou adicionar props:

1. **Primeiro**, adicione as novas props ao UI1 como opcionais
2. **Depois**, migre os consumidores do UI1 para usar as novas props
3. **Por fim**, crie o componente UI2 com a nova API

```tsx
// Passo 1: Adicione a prop opcional ao UI1
interface ButtonProps {
  primary?: boolean;
  // Nova prop opcional
  variant?: 'primary' | 'secondary';
}

// Passo 2: Atualize a implementação do UI1
export function Button({ primary, variant = primary ? 'primary' : 'secondary' }: ButtonProps) {
  // Use variant em vez de primary internamente
}

// Passo 3: Crie UI2 com a nova API
interface ButtonProps {
  // variant agora é a prop principal
  variant?: 'primary' | 'secondary';
  // Mantenha primary por compatibilidade, marque como obsoleta
  /** @deprecated Use variant em vez disso */
  primary?: boolean;
}
```

### Passo 4: Descontinuar o componente UI1

Adicione um aviso de descontinuação ao componente UI1:

````tsx
/**
 * @deprecated Este componente foi migrado para UI2.
 * Importe de 'decentraland-ui2' em vez disso:
 * 
 * ```tsx
 * import { Button } from 'decentraland-ui2';
 * ```
 * 
 * Veja o guia de migração: https://docs.decentraland.org/contributor-guides/web-ui-standards/migration
 */
export function Button(props: ButtonProps) {
  // ... implementação existente
}
````

### Passo 5: Adoção gradual

Não force a migração imediata. Permita adoção gradual:

1. **Publicar** componente UI2
2. **Documentar** caminho de migração
3. **Atualizar** novos projetos para usar UI2
4. **Migrar** projetos existentes de forma oportunista
5. **Planejar** remoção eventual do UI1 (com aviso)

***

## Exemplos de migração

### Exemplo 1: Componente simples

Migrando um `Card` componente:

#### Versão UI1

```tsx
// decentraland-ui/src/components/Card/Card.tsx
import React from 'react';
import './Card.css';

export interface CardProps {
  className?: string;
  children: React.ReactNode;
}

export function Card({ className, children }: CardProps) {
  return (
    <div className={`dcl-card ${className || ''}`}>
      {children}
    </div>
  );
}
```

#### Versão UI2

```tsx
// decentraland-ui2/src/components/Card/Card.tsx
import { styled } from '@mui/material/styles';

export interface CardProps {
  className?: string;
  children: React.ReactNode;
}

const StyledCard = styled('div')(({ theme }) => ({
  backgroundColor: theme.palette.background.paper,
  borderRadius: theme.shape.borderRadius,
  padding: theme.spacing(2),
  boxShadow: theme.shadows[1],
  
  [theme.breakpoints.down('sm')]: {
    padding: theme.spacing(1),
  },
}));

export function Card({ className, children }: CardProps) {
  return (
    <StyledCard className={className}>
      {children}
    </StyledCard>
  );
}
```

### Exemplo 2: Componente com variantes

Migrando um `Button` com variantes:

#### Versão UI1

```tsx
// UI1
import './Button.css';

interface ButtonProps {
  primary?: boolean;
  secondary?: boolean;
  size?: 'small' | 'medium' | 'large';
}

export function Button({ primary, secondary, size = 'medium', ...props }: ButtonProps) {
  const classes = [
    'dcl-button',
    primary && 'primary',
    secondary && 'secondary',
    `size-${size}`,
  ].filter(Boolean).join(' ');
  
  return <button className={classes} {...props} />;
}
```

#### Versão UI2

```tsx
// UI2
import { styled } from '@mui/material/styles';

interface ButtonProps {
  /** @deprecated Use variant="contained" instead */
  primary?: boolean;
  /** @deprecated Use variant="outlined" instead */
  secondary?: boolean;
  variant?: 'text' | 'outlined' | 'contained';
  size?: 'small' | 'medium' | 'large';
}

const StyledButton = styled('button')<ButtonProps>(({ theme, variant = 'contained', size = 'medium' }) => {
  const sizes = {
    small: theme.spacing(0.5, 1),
    medium: theme.spacing(1, 2),
    large: theme.spacing(1.5, 3),
  };
  
  const variants = {
    text: {
      backgroundColor: 'transparent',
      color: theme.palette.primary.main,
    },
    outlined: {
      backgroundColor: 'transparent',
      color: theme.palette.primary.main,
      border: `1px solid ${theme.palette.primary.main}`,
    },
    contained: {
      backgroundColor: theme.palette.primary.main,
      color: theme.palette.primary.contrastText,
    },
  };
  
  return {
    padding: sizes[size],
    borderRadius: theme.shape.borderRadius,
    border: 'none',
    cursor: 'pointer',
    ...variants[variant],
    
    '&:hover': {
      opacity: 0.9,
    },
    
    '&:disabled': {
      opacity: 0.5,
      cursor: 'not-allowed',
    },
  };
});

export function Button({ 
  primary, 
  secondary, 
  variant, 
  ...props 
}: ButtonProps) {
  // Lidar com props obsoletas
  const actualVariant = variant || 
    (primary ? 'contained' : secondary ? 'outlined' : 'text');
  
  return <StyledButton variant={actualVariant} {...props} />;
}
```

***

## Lista de verificação de migração

Use esta lista de verificação para cada migração de componente:

### Fase de planejamento

* [ ] Identifique todos os projetos que usam o componente
* [ ] Documente as props e comportamentos atuais
* [ ] Defina o escopo e o cronograma da migração
* [ ] Obtenha aprovação das partes interessadas

### Fase de implementação

* [ ] Crie o componente UI2 seguindo os padrões
* [ ] Mantenha a compatibilidade das props
* [ ] Use sintaxe de objeto para estilização
* [ ] Use apenas valores do tema
* [ ] Implemente todos os estados (idle, hover, focus, disabled, error)
* [ ] Adicione stories completas no Storybook
* [ ] Escreva testes unitários
* [ ] Documente os recursos de acessibilidade

### Fase de descontinuação

* [ ] Adicione um aviso de descontinuação ao componente UI1
* [ ] Atualize a documentação do UI1
* [ ] Crie um guia de migração para os consumidores
* [ ] Publique o componente UI2

### Fase de adoção

* [ ] Atualize novos projetos para usar UI2
* [ ] Crie PRs de migração para projetos existentes
* [ ] Monitore problemas
* [ ] Reúna feedback
* [ ] Planeje o cronograma de remoção do UI1

***

## Padrões comuns de migração

### De CSS para Styled Components

```tsx
// UI1: arquivo CSS
.dcl-card {
  background: #fff;
  padding: 16px;
  border-radius: 8px;
}

// UI2: componente estilizado
const Card = styled('div')(({ theme }) => ({
  backgroundColor: theme.palette.background.paper,
  padding: theme.spacing(2),
  borderRadius: theme.shape.borderRadius,
}));
```

### Nomes de classe para props

```tsx
// UI1: variantes baseadas em classes
<Button className={primary ? 'primary' : 'secondary'} />

// UI2: variantes baseadas em props
<Button variant={primary ? 'contained' : 'outlined'} />
```

### Valores fixos para tema

```tsx
// UI1: valores fixos
const styles = {
  color: '#333',
  fontSize: '14px',
  padding: '8px 16px',
};

// UI2: valores do tema
const Component = styled('div')(({ theme }) => ({
  color: theme.palette.text.primary,
  fontSize: theme.typography.body2.fontSize,
  padding: theme.spacing(1, 2),
}));
```

***

## Alterações que quebram compatibilidade

Às vezes, alterações que quebram compatibilidade são necessárias. Lide com cuidado:

### Quando alterações que quebram compatibilidade são aceitáveis

* Correções de segurança
* Bugs críticos
* Atualizações de versão principal
* Remoção de recursos obsoletos (com aviso)

### Como lidar com alterações que quebram compatibilidade

1. **Anuncie com antecedência** - Comunique as mudanças com bastante antecedência
2. **Forneça um caminho de migração** - Documente como atualizar
3. **Aumento de versão** - Siga o versionamento semântico
4. **Período de descontinuação** - Dê tempo para migrar
5. **Codemods** - Forneça ferramentas automatizadas de migração, se possível

### Exemplo: Removendo props obsoletas

```tsx
// Versão 1.0: introduza a nova API, descontinue a antiga
interface ButtonProps {
  /** @deprecated Use variant="contained" instead */
  primary?: boolean;
  variant?: 'text' | 'outlined' | 'contained';
}

// Versão 1.5: avise sobre a remoção
interface ButtonProps {
  /** @deprecated Será removida na 2.0. Use variant em vez disso */
  primary?: boolean;
  variant?: 'text' | 'outlined' | 'contained';
}

// Versão 2.0: remova a prop obsoleta
interface ButtonProps {
  variant?: 'text' | 'outlined' | 'contained';
}
```

***

## Testando a migração

Garanta que os componentes migrados funcionem corretamente:

### Testes de regressão visual

Compare visualmente os componentes UI1 e UI2:

```tsx
// Storybook story para comparação
export const ComparisonStory: Story = {
  render: () => (
    <div style={{ display: 'flex', gap: '2rem' }}>
      <div>
        <h3>UI1</h3>
        <UI1Button primary>Click Me</UI1Button>
      </div>
      <div>
        <h3>UI2</h3>
        <UI2Button variant="contained">Click Me</UI2Button>
      </div>
    </div>
  ),
};
```

### Testes comportamentais

Garanta que as props funcionem da mesma forma:

```tsx
describe('Button migration', () => {
  it('should handle primary prop (deprecated) the same as variant="contained"', () => {
    const { container: ui1 } = render(<UI1Button primary>Test</UI1Button>);
    const { container: ui2 } = render(<UI2Button primary>Test</UI2Button>);
    
    // Compare a saída renderizada
    expect(ui1.textContent).toBe(ui2.textContent);
  });
});
```

***

## Atualizações da documentação

Após a migração, atualize a documentação:

### Atualize a documentação do componente UI1

```markdown
# Button (UI1 - Obsoleto)

> ⚠️ **Este componente foi migrado para UI2.**
> Veja a [documentação do Button no UI2](../ui2/button) para a nova versão.

Este componente está obsoleto e será removido em uma versão futura.
Por favor, migre para UI2.

## Guia de migração

Veja [Guia de migração](./migration) para detalhes.
```

### Crie a documentação do componente UI2

```markdown
# Button (UI2)

Componente moderno de botão com suporte completo a temas.

## Migração de UI1

Se você estiver migrando de UI1:

- `primary` prop → `variant="contained"`
- `secondary` prop → `variant="outlined"`
- classes CSS → styled-components

Veja o [Guia de migração](./migration) completo para detalhes.
```

***

## Próximos Passos

* Revisar [Componentes Personalizados](/docs/contributor/contributor-pt/guias-para-colaboradores/padroes-de-web-ui/custom-components/) para criar componentes UI2
* Veja [Estilização e temas](/docs/contributor/contributor-pt/guias-para-colaboradores/padroes-de-web-ui/styling-and-theming/) para padrões de estilização
* Verifique [Visão geral do processo](/docs/contributor/contributor-pt/guias-para-colaboradores/padroes-de-web-ui/process-overview/) para o fluxo de trabalho completo


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/guias-para-colaboradores/padroes-de-web-ui/migration.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
