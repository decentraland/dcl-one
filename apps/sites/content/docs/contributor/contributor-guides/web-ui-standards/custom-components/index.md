# Custom Components

We distinguish between two types of custom components, each with different processes and expectations.

## Component Types

### A) Project-Specific Custom Components

Components built for a specific project or screen, not intended for reuse across other projects.

**Examples:**

* A `Box` with special layout used within one project
* A `Card` variant with bespoke layout for one project's screens
* Project-specific data visualizations
* One-off layout components

**When to use:**

* Component solves a problem unique to one project
* Unlikely to be needed in other projects
* Too specific to generalize

### B) UI2 Candidate Components

Components intended for reuse across multiple projects and products.

**Examples:**

* `Navbar` - Site-wide navigation
* `UserMenu` - User account menu
* Standardized `Modal` dialogs
* Components being migrated from UI1

**When to use:**

* Component will be used in multiple projects
* Represents a common Decentraland pattern
* Replaces or extends a UI1 component

***

## Project-Specific Components

### Requirements

#### Use MUI as Base

**MUST** extend existing MUI components whenever possible:

```tsx
// ✅ Good: Extends MUI Card
import { Card as MuiCard } from '@mui/material';
import { styled } from '@mui/material/styles';

const ProjectCard = styled(MuiCard)(({ theme }) => ({
  padding: theme.spacing(3),
  display: 'flex',
  flexDirection: 'column',
  gap: theme.spacing(2),
}));

// ❌ Bad: Builds from scratch
const ProjectCard = styled('div')(({ theme }) => ({
  padding: theme.spacing(3),
  borderRadius: '4px',
  boxShadow: '0 2px 4px rgba(0,0,0,0.1)',
  // Duplicating Card functionality
}));
```

**Don't fork or duplicate patterns that MUI already covers:**

* Use `Card` instead of creating a custom box with shadows
* Use `Button` instead of creating a styled anchor
* Use `TextField` instead of creating a custom input
* Extend `Dialog` instead of creating a custom modal

#### Theme Values Only

**MUST** use values from the UI2 theme:

```tsx
// ✅ Good: All values from theme
const StyledBox = styled('div')(({ theme }) => ({
  color: theme.palette.text.primary,
  backgroundColor: theme.palette.background.paper,
  padding: theme.spacing(2),
  borderRadius: theme.shape.borderRadius,
  border: `1px solid ${theme.palette.divider}`,
}));

// ❌ Bad: Ad-hoc values
const StyledBox = styled('div')({
  color: '#333333',
  backgroundColor: '#FFFFFF',
  padding: '16px',
  borderRadius: '8px',
  border: '1px solid #E0E0E0',
});
```

**No arbitrary values allowed:**

* Colors: Use `theme.palette` or `dclColors`
* Spacing: Use `theme.spacing(n)`
* Border radius: Use `theme.shape.borderRadius`
* Typography: Use `theme.typography` variants
* Breakpoints: Use `theme.breakpoints` helpers

#### States and Accessibility

**MUST** define and implement all interactive states:

```tsx
const ActionButton = styled('button')(({ theme }) => ({
  // Base/idle state
  padding: theme.spacing(1, 2),
  backgroundColor: theme.palette.primary.main,
  color: theme.palette.primary.contrastText,
  border: 'none',
  borderRadius: theme.shape.borderRadius,
  cursor: 'pointer',
  transition: theme.transitions.create(['background-color', 'transform']),
  
  // Hover state
  '&:hover': {
    backgroundColor: theme.palette.primary.dark,
  },
  
  // Focus state (keyboard navigation)
  '&:focus-visible': {
    outline: `2px solid ${theme.palette.primary.main}`,
    outlineOffset: 2,
  },
  
  // Active/pressed state
  '&:active': {
    transform: 'scale(0.98)',
  },
  
  // Disabled state
  '&:disabled': {
    backgroundColor: theme.palette.action.disabledBackground,
    color: theme.palette.action.disabled,
    cursor: 'not-allowed',
  },
}));
```

**MUST** implement basic accessibility:\*\*

* **Keyboard navigation** - Focusable and operable with keyboard
* **Focus indicators** - Visible focus states
* **ARIA labels** - Where text isn't visible
* **Semantic HTML** - Use appropriate elements
* **Color contrast** - Meet WCAG AA standards

### Example: Project-Specific Component

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
          Owner: {parcel.owner}
        </Typography>
      </CardContent>
      <CardActions>
        <Button size="small" onClick={() => onView(parcel.id)}>
          View
        </Button>
        <Button size="small" onClick={() => onTransfer(parcel.id)}>
          Transfer
        </Button>
      </CardActions>
    </StyledCard>
  );
}
```

***

## UI2 Candidate Components

Components that will be shared across projects require higher standards and more thorough documentation.

### Requirements

#### Theme Alignment

**MUST** rely exclusively on UI2 theme values:

```tsx
// ✅ Good: Full theme integration
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

#### Storybook Coverage

**MUST** add comprehensive Storybook stories:

**Required coverage:**

1. **All props and variants**
   * Every prop combination
   * All size variants
   * All color variants
2. **All states**
   * Idle/default
   * Loading
   * Error
   * Disabled
   * Hover (via pseudo states addon)
   * Focus (via pseudo states addon)
3. **Interactions**
   * Click handlers
   * Form submissions
   * Keyboard navigation
4. **Color schemes**
   * Light mode
   * Dark mode
5. **Responsive behavior**
   * Key breakpoints (xs, md, lg)
   * Document behavior at each breakpoint

**Example Storybook file:**

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

// Test different viewports
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

// Test color schemes
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

### Component Structure

UI2 candidate components SHOULD follow this structure:

```
src/components/Navbar/
├── Navbar.tsx           # Main component
├── Navbar.styles.ts     # Styled components
├── Navbar.stories.tsx   # Storybook stories
├── Navbar.test.tsx      # Unit tests
├── types.ts             # TypeScript types
├── index.ts             # Public exports
└── README.md            # Component documentation
```

### Documentation Requirements

**MUST** include in component README:

1. **Purpose** - What problem does this solve?
2. **Usage** - How to use the component
3. **Props** - All props with types and descriptions
4. **Examples** - Common use cases
5. **Accessibility** - Keyboard support, ARIA labels
6. **Theming** - Which theme values it uses
7. **Migration notes** - If replacing a UI1 component

**Example README:**

```markdown
# Navbar

Site-wide navigation component with user menu and responsive behavior.

## Usage

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

| Prop         | Type                   | Default   | Description                   |
| ------------ | ---------------------- | --------- | ----------------------------- |
| variant      | 'default' \| 'compact' | 'default' | Navigation variant            |
| showUserMenu | boolean                | true      | Show user menu when logged in |
| onLogoClick  | () => void             | -         | Logo click handler            |
| onLoginClick | () => void             | -         | Login button click handler    |

## Accessibility

* Keyboard navigation: Tab through menu items
* ARIA: Proper landmarks and labels
* Screen reader: Announces menu state

## Theming

Uses these theme values:

* `theme.palette.background.paper`
* `theme.palette.divider`
* `theme.spacing`
* `theme.breakpoints`

```

### Testing Requirements

**MUST** include tests for:

* Prop rendering
* User interactions
* Accessibility features
* Responsive behavior
* Error states

```tsx
// Navbar.test.tsx
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { Navbar } from './Navbar';

describe('Navbar', () => {
  it('should render logo', () => {
    render(<Navbar />);
    expect(screen.getByRole('banner')).toBeInTheDocument();
  });

  it('should call onLogoClick when logo is clicked', async () => {
    const onLogoClick = jest.fn();
    render(<Navbar onLogoClick={onLogoClick} />);
    
    await userEvent.click(screen.getByRole('link', { name: /decentraland/i }));
    expect(onLogoClick).toHaveBeenCalled();
  });

  it('should be keyboard navigable', async () => {
    render(<Navbar />);
    const firstLink = screen.getAllByRole('link')[0];
    
    firstLink.focus();
    expect(firstLink).toHaveFocus();
  });
});
```

***

## Decision Matrix

Use this to decide which type of component to create:

| Question                           | Project-Specific | UI2 Candidate |
| ---------------------------------- | ---------------- | ------------- |
| Will other projects use this?      | No               | Yes           |
| Does UI1 have an equivalent?       | N/A              | Probably      |
| Needs Storybook documentation?     | No               | **Yes**       |
| Needs comprehensive tests?         | Basic            | **Extensive** |
| Design review required?            | Project-level    | **UI2-level** |
| Can use project-specific patterns? | Yes              | **No**        |
| Must work in all themes?           | No               | **Yes**       |

***

## Approval Process

### Project-Specific Components

1. Code review by project maintainer
2. Verify theme compliance
3. Test in project context
4. Merge when approved

### UI2 Candidate Components

1. Design review and approval
2. Technical design review
3. Implementation
4. Storybook stories
5. Comprehensive tests
6. Accessibility review
7. Code review
8. PR to UI2 repository
9. Version and publish
10. Update dependent projects

***

## Best Practices

### Composition Over Customization

```tsx
// ✅ Good: Compose MUI components
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

// ❌ Bad: Re-implement Card functionality
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

### Progressive Enhancement

Start simple and add features as needed:

1. Basic version with core functionality
2. Add responsive behavior
3. Add accessibility features
4. Add advanced interactions
5. Optimize performance

### Documentation First

Before writing code:

1. Write component README
2. Define props interface
3. List required states
4. Plan Storybook stories
5. Then implement

***

## Next Steps

* Review [Styling & Theming](/docs/contributor/contributor-guides/web-ui-standards/styling-and-theming/) for implementation details
* See [Migration Guide](/docs/contributor/contributor-guides/web-ui-standards/migration/) for UI1 to UI2 migrations
* Check [Process Overview](/docs/contributor/contributor-guides/web-ui-standards/process-overview/) for the full workflow
