# UI1 to UI2 Migration Guide

This guide covers the process of migrating components from UI1 (`decentraland-ui`) to UI2 (`decentraland-ui2`).

## When to Migrate

Migrations can start for three valid reasons:

### 1. Technical Improvement

You want to migrate a component for:
* Better theming support
* Improved TypeScript types
* Consistency with other UI2 components
* Performance optimization
* Accessibility improvements

### 2. Updating a Component

* A UI1 component needs to be updated
* There is no UI2 equivalent yet
* **Requirement**: Create it in UI2 first, then use it

### 3. Project Needs

* The component will be used in a new or existing project
* There is time available to migrate it properly
* Project resources allow for thorough migration


> [!WARNING]
> **Do not** migrate components "just because." Each migration should have a clear business or technical justification.


---

## Migration Process

### Step 1: Planning

Before starting the migration:

1. **Identify dependencies**
   * What other components does it use?
   * What projects currently use it?
   * Are there any breaking changes planned?

2. **Review current usage**
   * How many projects use this component?
   * What props are most commonly used?
   * Are there any known issues?

3. **Define scope**
   * Will this be a 1:1 migration?
   * Are there planned improvements?
   * What's the timeline?

### Step 2: Create UI2 Component

Follow the [Custom Components](/docs/contributor/contributor-guides/web-ui-standards/custom-components/) guide for UI2 candidate components.

**Requirements:**

* Use object syntax for styled-components
* Use only theme values (no arbitrary values)
* Add comprehensive Storybook stories
* Write complete tests
* Document all props and behaviors

**Example structure:**

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

### Step 3: Maintain Compatibility

The UI2 component **MUST** expose the same props and behaviors as the UI1 version.

#### Same Props

```tsx
// UI1 Button
interface ButtonProps {
  primary?: boolean;
  size?: 'small' | 'medium' | 'large';
  onClick?: () => void;
  disabled?: boolean;
  children: React.ReactNode;
}

// UI2 Button - MUST support same props
interface ButtonProps {
  primary?: boolean;
  size?: 'small' | 'medium' | 'large';
  onClick?: () => void;
  disabled?: boolean;
  children: React.ReactNode;
  // Can add new optional props
  variant?: 'text' | 'outlined' | 'contained';
}
```

#### Backward Compatible Changes

If you need to change or add props:

1. **First**, add the new props to UI1 as optional
2. **Then**, migrate UI1 consumers to use the new props
3. **Finally**, create the UI2 component with the new API

```tsx
// Step 1: Add optional prop to UI1
interface ButtonProps {
  primary?: boolean;
  // New optional prop
  variant?: 'primary' | 'secondary';
}

// Step 2: Update UI1 implementation
export function Button({ primary, variant = primary ? 'primary' : 'secondary' }: ButtonProps) {
  // Use variant instead of primary internally
}

// Step 3: Create UI2 with new API
interface ButtonProps {
  // variant is now the primary prop
  variant?: 'primary' | 'secondary';
  // Keep primary for compatibility, mark as deprecated
  /** @deprecated Use variant instead */
  primary?: boolean;
}
```

### Step 4: Deprecate UI1 Component

Add a deprecation notice to the UI1 component:

```tsx
/**
 * @deprecated This component has been migrated to UI2.
 * Import from 'decentraland-ui2' instead:
 * 
 * ```tsx
 * import { Button } from 'decentraland-ui2';
 * ```
 * 
 * See migration guide: https://docs.decentraland.org/contributor-guides/web-ui-standards/migration
 */
export function Button(props: ButtonProps) {
  // ... existing implementation
}
```

### Step 5: Gradual Adoption

Don't force immediate migration. Allow gradual adoption:

1. **Publish** UI2 component
2. **Document** migration path
3. **Update** new projects to use UI2
4. **Migrate** existing projects opportunistically
5. **Plan** eventual UI1 removal (with notice)

---

## Migration Examples

### Example 1: Simple Component

Migrating a basic `Card` component:

#### UI1 Version

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

#### UI2 Version

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

### Example 2: Component with Variants

Migrating a `Button` with variants:

#### UI1 Version

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

#### UI2 Version

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
  // Handle deprecated props
  const actualVariant = variant || 
    (primary ? 'contained' : secondary ? 'outlined' : 'text');
  
  return <StyledButton variant={actualVariant} {...props} />;
}
```

---

## Migration Checklist

Use this checklist for each component migration:

### Planning Phase

- [ ]  Identify all projects using the component
- [ ]  Document current props and behaviors
- [ ]  Define migration scope and timeline
- [ ]  Get stakeholder approval

### Implementation Phase

- [ ]  Create UI2 component following standards
- [ ]  Maintain prop compatibility
- [ ]  Use object syntax for styling
- [ ]  Use only theme values
- [ ]  Implement all states (idle, hover, focus, disabled, error)
- [ ]  Add comprehensive Storybook stories
- [ ]  Write unit tests
- [ ]  Document accessibility features

### Deprecation Phase

- [ ]  Add deprecation notice to UI1 component
- [ ]  Update UI1 documentation
- [ ]  Create migration guide for consumers
- [ ]  Publish UI2 component

### Adoption Phase

- [ ]  Update new projects to use UI2
- [ ]  Create migration PRs for existing projects
- [ ]  Monitor for issues
- [ ]  Gather feedback
- [ ]  Plan UI1 removal timeline

---

## Common Migration Patterns

### CSS to Styled Components

```tsx
// UI1: CSS file
.dcl-card {
  background: #fff;
  padding: 16px;
  border-radius: 8px;
}

// UI2: Styled component
const Card = styled('div')(({ theme }) => ({
  backgroundColor: theme.palette.background.paper,
  padding: theme.spacing(2),
  borderRadius: theme.shape.borderRadius,
}));
```

### Class Names to Props

```tsx
// UI1: Class-based variants
<Button className={primary ? 'primary' : 'secondary'} />

// UI2: Prop-based variants
<Button variant={primary ? 'contained' : 'outlined'} />
```

### Fixed Values to Theme

```tsx
// UI1: Fixed values
const styles = {
  color: '#333',
  fontSize: '14px',
  padding: '8px 16px',
};

// UI2: Theme values
const Component = styled('div')(({ theme }) => ({
  color: theme.palette.text.primary,
  fontSize: theme.typography.body2.fontSize,
  padding: theme.spacing(1, 2),
}));
```

---

## Breaking Changes

Sometimes breaking changes are necessary. Handle them carefully:

### When Breaking Changes Are Acceptable

* Security fixes
* Critical bugs
* Major version updates
* Removing deprecated features (with notice)

### How to Handle Breaking Changes

1. **Announce early** - Communicate changes well in advance
2. **Provide migration path** - Document how to update
3. **Version bump** - Follow semantic versioning
4. **Deprecation period** - Give time to migrate
5. **Codemods** - Provide automated migration tools if possible

### Example: Removing Deprecated Props

```tsx
// Version 1.0: Introduce new API, deprecate old
interface ButtonProps {
  /** @deprecated Use variant="contained" instead */
  primary?: boolean;
  variant?: 'text' | 'outlined' | 'contained';
}

// Version 1.5: Warn about removal
interface ButtonProps {
  /** @deprecated Will be removed in 2.0. Use variant instead */
  primary?: boolean;
  variant?: 'text' | 'outlined' | 'contained';
}

// Version 2.0: Remove deprecated prop
interface ButtonProps {
  variant?: 'text' | 'outlined' | 'contained';
}
```

---

## Testing Migration

Ensure migrated components work correctly:

### Visual Regression Testing

Compare UI1 and UI2 components visually:

```tsx
// Storybook story for comparison
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

### Behavioral Testing

Ensure props work the same way:

```tsx
describe('Button migration', () => {
  it('should handle primary prop (deprecated) the same as variant="contained"', () => {
    const { container: ui1 } = render(<UI1Button primary>Test</UI1Button>);
    const { container: ui2 } = render(<UI2Button primary>Test</UI2Button>);
    
    // Compare rendered output
    expect(ui1.textContent).toBe(ui2.textContent);
  });
});
```

---

## Documentation Updates

After migration, update documentation:

### Update UI1 Component Docs

```markdown
# Button (UI1 - Deprecated)

> ⚠️ **This component has been migrated to UI2.**
> See the [UI2 Button documentation](../ui2/button) for the new version.

This component is deprecated and will be removed in a future version.
Please migrate to UI2.

## Migration Guide

See [Migration Guide](./migration) for details.
```

### Create UI2 Component Docs

```markdown
# Button (UI2)

Modern button component with full theme support.

## Migration from UI1

If you're migrating from UI1:

- `primary` prop → `variant="contained"`
- `secondary` prop → `variant="outlined"`
- CSS classes → styled-components

See full [Migration Guide](./migration) for details.
```

---

## Next Steps

* Review [Custom Components](/docs/contributor/contributor-guides/web-ui-standards/custom-components/) for creating UI2 components
* See [Styling & Theming](/docs/contributor/contributor-guides/web-ui-standards/styling-and-theming/) for styling standards
* Check [Process Overview](/docs/contributor/contributor-guides/web-ui-standards/process-overview/) for the complete workflow
