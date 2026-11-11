# Custom Components

Diferenciamos dois tipos de componentes customizados, cada um com processos e expectativas diferentes.

## Tipos de Componentes

### A) Componentes Customizados Específicos do Projeto

Componentes criados para um projeto ou tela específicos, não destinados à reutilização em outros projetos.

**Exemplos:**

* Um `Box` com layout especial usado dentro de um projeto
* Um `Card` variante com layout sob medida para as telas de um projeto
* Visualizações de dados específicas do projeto
* Componentes de layout únicos

**Quando usar:**

* O componente resolve um problema exclusivo de um projeto
* É improvável que seja necessário em outros projetos
* Específico demais para generalizar

### B) Componentes candidatos ao UI2

Componentes destinados à reutilização em vários projetos e produtos.

**Exemplos:**

* `Navbar` - Navegação em todo o site
* `UserMenu` - Menu da conta do usuário
* Padronizado `Modal` diálogos
* Componentes sendo migrados do UI1

**Quando usar:**

* O componente será usado em vários projetos
* Representa um padrão comum do Decentraland
* Substitui ou estende um componente do UI1

***

## Componentes Específicos do Projeto

### Requisitos

#### Use MUI como base

**DEVEM** estenda os componentes MUI existentes sempre que possível:

```tsx
// ✅ Bom: Estende MUI Card
import { Card as MuiCard } from '@mui/material';
import { styled } from '@mui/material/styles';

const ProjectCard = styled(MuiCard)(({ theme }) => ({
  padding: theme.spacing(3),
  display: 'flex',
  flexDirection: 'column',
  gap: theme.spacing(2),
}));

// ❌ Ruim: Constrói do zero
const ProjectCard = styled('div')(({ theme }) => ({
  padding: theme.spacing(3),
  borderRadius: '4px',
  boxShadow: '0 2px 4px rgba(0,0,0,0.1)',
  // Duplicando a funcionalidade do Card
}));
```

**Não faça fork nem duplique padrões que o MUI já cobre:**

* Use `Card` em vez de criar uma caixa personalizada com sombras
* Use `Button` em vez de criar uma âncora estilizada
* Use `TextField` em vez de criar um input personalizado
* Estender `Dialog` em vez de criar um modal personalizado

#### Somente valores do tema

**DEVEM** use valores do tema do UI2:

```tsx
// ✅ Bom: Todos os valores do tema
const StyledBox = styled('div')(({ theme }) => ({
  color: theme.palette.text.primary,
  backgroundColor: theme.palette.background.paper,
  padding: theme.spacing(2),
  borderRadius: theme.shape.borderRadius,
  border: `1px solid ${theme.palette.divider}`,
}));

// ❌ Ruim: Valores ad hoc
const StyledBox = styled('div')({
  color: '#333333',
  backgroundColor: '#FFFFFF',
  padding: '16px',
  borderRadius: '8px',
  border: '1px solid #E0E0E0',
});
```

**Não são permitidos valores arbitrários:**

* Cores: use `theme.palette` ou `dclColors`
* Espaçamento: use `theme.spacing(n)`
* Raio da borda: use `theme.shape.borderRadius`
* Tipografia: use `theme.typography` variantes
* Breakpoints: use `theme.breakpoints` helpers

#### Estados e Acessibilidade

**DEVEM** defina e implemente todos os estados interativos:

```tsx
const ActionButton = styled('button')(({ theme }) => ({
  // Estado base/inativo
  padding: theme.spacing(1, 2),
  backgroundColor: theme.palette.primary.main,
  color: theme.palette.primary.contrastText,
  border: 'none',
  borderRadius: theme.shape.borderRadius,
  cursor: 'pointer',
  transition: theme.transitions.create(['background-color', 'transform']),
  
  // Estado Hover
  '&:hover': {
    backgroundColor: theme.palette.primary.dark,
  },
  
  // Estado de focus (navegação por teclado)
  '&:focus-visible': {
    outline: `2px solid ${theme.palette.primary.main}`,
    outlineOffset: 2,
  },
  
  // Estado Active/pressed
  '&:active': {
    transform: 'scale(0.98)',
  },
  
  // Estado disabled
  '&:disabled': {
    backgroundColor: theme.palette.action.disabledBackground,
    color: theme.palette.action.disabled,
    cursor: 'not-allowed',
  },
}));
```

**DEVEM** implementar acessibilidade básica:\*\*

* **Navegação por teclado** - Foco e operação com teclado
* **Indicadores de foco** - Estados de foco visíveis
* **Labels ARIA** - Quando o texto não está visível
* **HTML semântico** - Use elementos apropriados
* **Contraste de cores** - Atenda aos padrões WCAG AA

### Exemplo: componente específico do projeto

```tsx
// src/components/LandCard/LandCard.tsx
import { Card, CardContent, CardActions, Typography, Button } from '@mui/material';
import { styled } from '@mui/material/styles';
import type { Parcel } from '@/types';

interface LandCardProps {
  parcel: Parcel;
  onTransfer: (id: string) => void;
  onView: (id: string) => void;
}

const StyledCard = styled(Card)(({ theme }) => ({
  display: 'flex',
  flexDirection: 'column',
  height: '100%',
  transition: theme.transitions.create('transform'),
  
  '&:hover': {
    transform: 'translateY(-4px)',
  },
}));

const CoordinatesText = styled(Typography)(({ theme }) => ({
  color: theme.palette.text.secondary,
  fontFamily: theme.typography.fontFamilyMono,
}));

export function LandCard({ parcel, onTransfer, onView }: LandCardProps) {
  return (
    <StyledCard>
      <CardContent>
        <Typography variant="h6" gutterBottom>
          {parcel.name || `Parcel ${parcel.x},${parcel.y}`}
        </Typography>
        <CoordinatesText variant="body2">
          ({parcel.x}, {parcel.y})
        </CoordinatesText>
        <Typography variant="body2" color="text.secondary">
          Proprietário: {parcel.owner}
        </Typography>
      </CardContent>
      <CardActions>
        <Button size="small" onClick={() => onView(parcel.id)}>
          Ver
        </Button>
        <Button size="small" onClick={() => onTransfer(parcel.id)}>
          Transferir
        </Button>
      </CardActions>
    </StyledCard>
  );
}
```

***

## Componentes candidatos ao UI2

Componentes que serão compartilhados entre projetos exigem padrões mais altos e documentação mais completa.

### Requisitos

#### Alinhamento com o tema

**DEVEM** dependa exclusivamente dos valores do tema do UI2:

```tsx
// ✅ Bom: Integração completa com o tema
const NavbarContainer = styled('nav')(({ theme }) => ({
  backgroundColor: theme.palette.background.paper,
  borderBottom: `1px solid ${theme.palette.divider}`,
  padding: theme.spacing(0, 2),
  height: 64,
  display: 'flex',
  alignItems: 'center',
  gap: theme.spacing(2),
  
  [theme.breakpoints.down('md')]: {
    padding: theme.spacing(0, 1),
  },
}));
```

#### Cobertura do Storybook

**DEVEM** adicione histórias abrangentes no Storybook:

**Cobertura obrigatória:**

1. **Todas as props e variantes**
   * Todas as combinações de props
   * Todas as variantes de tamanho
   * Todas as variantes de cor
2. **Todos os estados**
   * Inativo/padrão
   * A carregar
   * Erro
   * Desativado
   * Hover (via addon de pseudo states)
   * Focus (via addon de pseudo states)
3. **Interações**
   * Handlers de clique
   * Envios de formulário
   * Navegação por teclado
4. **Esquemas de cores**
   * Modo claro
   * Modo escuro
5. **Comportamento responsivo**
   * Principais breakpoints (xs, md, lg)
   * Documente o comportamento em cada breakpoint

**Arquivo de exemplo do Storybook:**

```tsx
// Navbar.stories.tsx
import type { Meta, StoryObj } from '@storybook/react';
import { Navbar } from './Navbar';

const meta: Meta<typeof Navbar> = {
  title: 'Components/Navbar',
  component: Navbar,
  parameters: {
    layout: 'fullscreen',
  },
  argTypes: {
    variant: {
      control: 'select',
      options: ['default', 'compact'],
    },
    showUserMenu: {
      control: 'boolean',
    },
  },
};

export default meta;
type Story = StoryObj<typeof Navbar>;

export const Default: Story = {
  args: {
    variant: 'default',
    showUserMenu: true,
  },
};

export const Compact: Story = {
  args: {
    variant: 'compact',
    showUserMenu: true,
  },
};

export const WithoutUserMenu: Story = {
  args: {
    variant: 'default',
    showUserMenu: false,
  },
};

export const Loading: Story = {
  args: {
    variant: 'default',
    showUserMenu: true,
    isLoading: true,
  },
};

// Teste diferentes viewports
export const Mobile: Story = {
  args: {
    variant: 'compact',
    showUserMenu: true,
  },
  parameters: {
    viewport: {
      defaultViewport: 'mobile1',
    },
  },
};

export const Tablet: Story = {
  args: {
    variant: 'default',
    showUserMenu: true,
  },
  parameters: {
    viewport: {
      defaultViewport: 'tablet',
    },
  },
};

// Teste esquemas de cores
export const DarkMode: Story = {
  args: {
    variant: 'default',
    showUserMenu: true,
  },
  parameters: {
    backgrounds: {
      default: 'dark',
    },
  },
};
```

### Estrutura do componente

Os componentes candidatos ao UI2 DEVEM seguir esta estrutura:

```
src/components/Navbar/
├── Navbar.tsx           # Componente principal
├── Navbar.styles.ts     # Componentes estilizados
├── Navbar.stories.tsx   # Histórias do Storybook
├── Navbar.test.tsx      # Testes unitários
├── types.ts             # Tipos do TypeScript
├── index.ts             # Exportações públicas
└── README.md            # Documentação do componente
```

### Requisitos de documentação

**DEVEM** incluir no README do componente:

1. **Propósito** - Que problema isto resolve?
2. **Uso** - Como usar o componente
3. **Props** - Todas as props com tipos e descrições
4. **Exemplos** - Casos de uso comuns
5. **Acessibilidade** - Suporte a teclado, rótulos ARIA
6. **Tematização** - Quais valores do tema ele usa
7. **Notas de migração** - Se estiver substituindo um componente do UI1

**README de exemplo:**

````markdown
# Navbar

Componente de navegação em todo o site com menu de usuário e comportamento responsivo.

## Uso

\```tsx
import { Navbar } from 'decentraland-ui2';

function App() {
  return (
    <Navbar
      variant="default"
      showUserMenu={true}
      onLogoClick={() => navigate('/')}
      onLoginClick={handleLogin}
    />
  );
}
\```

## Props

| Prop         | Type                   | Padrão   | Descrição                   |
| ------------ | ---------------------- | --------- | ----------------------------- |
| variant      | 'default' \| 'compact' | 'default' | Variante de navegação       |
| showUserMenu | boolean                | true      | Mostrar menu do usuário quando logado |
| onLogoClick  | () => void             | -         | Handler de clique do logo   |
| onLoginClick | () => void             | -         | Handler de clique do botão de login |

## Acessibilidade

* Navegação por teclado: use Tab para percorrer os itens do menu
* ARIA: marcos e rótulos adequados
* Leitor de tela: anuncia o estado do menu

## Tematização

Usa estes valores do tema:

* `theme.palette.background.paper`
* `theme.palette.divider`
* `theme.spacing`
* `theme.breakpoints`

````

### Requisitos de teste

**DEVEM** incluir testes para:

* Renderização das props
* Interações do usuário
* Recursos de acessibilidade
* Comportamento responsivo
* Estados de erro

```tsx
// Navbar.test.tsx
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { Navbar } from './Navbar';

describe('Navbar', () => {
  it('deve renderizar o logo', () => {
    render(<Navbar />);
    expect(screen.getByRole('banner')).toBeInTheDocument();
  });

  it('deve chamar onLogoClick quando o logo é clicado', async () => {
    const onLogoClick = jest.fn();
    render(<Navbar onLogoClick={onLogoClick} />);
    
    await userEvent.click(screen.getByRole('link', { name: /decentraland/i }));
    expect(onLogoClick).toHaveBeenCalled();
  });

  it('deve ser navegável por teclado', async () => {
    render(<Navbar />);
    const firstLink = screen.getAllByRole('link')[0];
    
    firstLink.focus();
    expect(firstLink).toHaveFocus();
  });
});
```

***

## Matriz de decisão

Use isto para decidir que tipo de componente criar:

| Pergunta                                  | Específico do projeto | Candidato ao UI2 |
| ----------------------------------------- | --------------------- | ---------------- |
| Outros projetos vão usar isto?            | Não                   | Sim              |
| O UI1 tem um equivalente?                 | N/A                   | Provavelmente    |
| Precisa de documentação no Storybook?     | Não                   | **Sim**          |
| Precisa de testes abrangentes?            | Básico                | **Abrangente**   |
| Revisão de design necessária?             | Nível do projeto      | **Nível do UI2** |
| Pode usar padrões específicos do projeto? | Sim                   | **Não**          |
| Precisa funcionar em todos os temas?      | Não                   | **Sim**          |

***

## Processo de aprovação

### Componentes Específicos do Projeto

1. Revisão de código pelo mantenedor do projeto
2. Verificar conformidade com o tema
3. Testar no contexto do projeto
4. Fazer merge quando aprovado

### Componentes candidatos ao UI2

1. Revisão e aprovação de design
2. Revisão de design técnico
3. Implementação
4. Histórias do Storybook
5. Testes abrangentes
6. Revisão de acessibilidade
7. Revisão de código
8. PR para o repositório do UI2
9. Versionar e publicar
10. Atualizar projetos dependentes

***

## Melhores práticas

### Composição em vez de customização

```tsx
// ✅ Bom: Componha componentes MUI
function FeatureCard({ title, children }) {
  return (
    <Card>
      <CardContent>
        <Typography variant="h6">{title}</Typography>
        {children}
      </CardContent>
    </Card>
  );
}

// ❌ Ruim: Reimplemente a funcionalidade do Card
function FeatureCard({ title, children }) {
  return (
    <div className="custom-card">
      <div className="custom-card-content">
        <h3>{title}</h3>
        {children}
      </div>
    </div>
  );
}
```

### Melhoria Progressiva

Comece com algo simples e adicione funcionalidades conforme necessário:

1. Versão básica com funcionalidade principal
2. Adicione comportamento responsivo
3. Adicione recursos de acessibilidade
4. Adicione interações avançadas
5. Otimize o desempenho

### Documentação Primeiro

Antes de escrever o código:

1. Escreva o README do componente
2. Defina a interface de props
3. Liste os estados necessários
4. Planeie as stories do Storybook
5. Depois implemente

***

## Próximos Passos

* Revisar [Estilização e temas](/docs/contributor/contributor-pt/guias-para-colaboradores/padroes-de-web-ui/styling-and-theming/) para detalhes de implementação
* Veja [Guia de Migração](/docs/contributor/contributor-pt/guias-para-colaboradores/padroes-de-web-ui/migration/) para migrações de UI1 para UI2
* Verifique [Visão geral do processo](/docs/contributor/contributor-pt/guias-para-colaboradores/padroes-de-web-ui/process-overview/) para o fluxo de trabalho completo


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/guias-para-colaboradores/padroes-de-web-ui/custom-components.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
