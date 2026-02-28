# ESLint Rule Patterns Reference

This reference covers the mechanics of writing and wiring ESLint rules. It tells you
HOW to create rules — WHAT to enforce is determined by the codebase context.

## Flat Config: Adding Custom Rules

### Inline Plugin (no npm package needed)

```js
// eslint.config.js
import myRule from './eslint-rules/my-rule.js';

const driftGuard = {
  rules: {
    'my-rule': myRule,
    // add more rules here
  }
};

export default [
  // ...other config
  {
    plugins: { 'drift-guard': driftGuard },
    rules: {
      'drift-guard/my-rule': 'warn',
    }
  }
];
```

### Directory-scoped overrides

Different directories may need different rules. For instance, an abstraction layer
should be allowed to use the low-level API it wraps:

```js
export default [
  {
    files: ['src/**/*.ts', 'src/**/*.tsx'],
    plugins: { 'drift-guard': driftGuard },
    rules: {
      'drift-guard/my-rule': 'warn',
    }
  },
  {
    // Override: allow the abstraction layer itself to use the low-level API
    files: ['src/some-layer/**/*.ts'],
    rules: {
      'drift-guard/my-rule': 'off',
    }
  }
];
```

---

## Rule Mechanisms

### 1. no-restricted-imports

**Use for:** Banning specific import paths or patterns.

```js
rules: {
  'no-restricted-imports': ['warn', {
    // Ban specific paths
    paths: [
      {
        name: 'some-module',
        message: 'Use X instead. See docs/patterns/whatever.md'
      },
      {
        name: 'some-module',
        importNames: ['specificExport'],
        message: 'specificExport is deprecated. Use Y instead.'
      }
    ],
    // Ban path patterns
    patterns: [
      {
        group: ['**/internal/*', '!**/internal/index'],
        message: 'Import from the public API (index) instead of internal modules.'
      }
    ]
  }]
}
```

### 2. no-restricted-syntax

**Use for:** Banning specific code patterns using AST selectors.

AST selectors use the same syntax as CSS selectors but for code. Use
[AST Explorer](https://astexplorer.net) to find the right selector.

```js
rules: {
  'no-restricted-syntax': ['warn',
    {
      // Ban a specific function call pattern
      selector: "CallExpression[callee.name='dangerousFunction']",
      message: 'Use safeAlternative() instead.'
    },
    {
      // Ban a member expression pattern (e.g., obj.method())
      selector: "CallExpression[callee.object.name='obj'][callee.property.name='method']",
      message: 'Use the wrapper hook instead of calling obj.method() directly.'
    },
    {
      // Ban module-level let declarations
      selector: "Program > VariableDeclaration[kind='let']",
      message: 'Avoid module-level mutable state.'
    }
  ]
}
```

**Common AST selector patterns:**

| What you want to match | Selector |
|------------------------|----------|
| Function call `foo()` | `CallExpression[callee.name='foo']` |
| Method call `a.b()` | `CallExpression[callee.object.name='a'][callee.property.name='b']` |
| Chained call `a.b.c()` | `CallExpression[callee.object.type='MemberExpression'][callee.object.object.name='a']` |
| JSX element `<Foo>` | `JSXOpeningElement[name.name='Foo']` |
| JSX attribute `prop={val}` | `JSXAttribute[name.name='prop']` |
| Variable with kind | `VariableDeclaration[kind='let']` |
| Top-level only | `Program > [selector]` |
| Inside a function | `FunctionDeclaration [selector]` |
| Containing something | `ForStatement:has(CatchClause)` |

### 3. Custom Rule Module

**Use for:** Anything more complex — file-path-aware rules, counting/threshold rules,
cross-node analysis, auto-fixable rules.

```js
// eslint-rules/example-rule.js
export default {
  meta: {
    type: 'suggestion',           // 'problem' | 'suggestion' | 'layout'
    docs: {
      description: 'What this rule does',
    },
    messages: {
      // Named messages with optional placeholders
      ruleViolation: 'Found {{thing}} — use {{alternative}} instead. See {{docLink}}',
    },
    // fixable: 'code',           // uncomment if rule provides auto-fixes
    schema: [],                    // options schema (empty = no options)
  },

  create(context) {
    // context.filename — current file path (use for directory-aware rules)
    // context.report({ node, messageId, data }) — report a violation
    // context.getSourceCode() — access the full AST

    return {
      // Visitor methods — named after AST node types
      // See https://astexplorer.net for node types

      CallExpression(node) {
        // Your detection logic here
        if (/* violation detected */) {
          context.report({
            node,
            messageId: 'ruleViolation',
            data: {
              thing: 'what was found',
              alternative: 'what to use instead',
              docLink: 'docs/patterns/relevant.md',
            },
          });
        }
      },

      ImportDeclaration(node) {
        // Check import source: node.source.value
        // Check imported names: node.specifiers
      },

      JSXOpeningElement(node) {
        // Check component name: node.name.name (for simple names)
        // Check attributes: node.attributes
      },
    };
  },
};
```

**File-path-aware rule example:**
```js
create(context) {
  const filename = context.filename || context.getFilename();
  const isInAllowedDirectory = /some-directory[/\\]/.test(filename);
  if (isInAllowedDirectory) return {};  // skip enforcement in this directory

  return {
    // ... visitors
  };
}
```

**Counting rule example (e.g., max N of something per file):**
```js
create(context) {
  let count = 0;
  const MAX = 10;

  return {
    'SomeNode'() {
      count++;
    },
    'Program:exit'() {
      if (count > MAX) {
        context.report({
          node: context.getSourceCode().ast,
          message: `File has ${count} occurrences (max ${MAX}). Consider refactoring.`,
        });
      }
    },
  };
}
```

---

## Testing Rules

```js
// eslint-rules/__tests__/example-rule.test.js
import { RuleTester } from 'eslint';
import rule from '../example-rule.js';

const tester = new RuleTester({
  languageOptions: { ecmaVersion: 2022, sourceType: 'module' }
});

tester.run('example-rule', rule, {
  valid: [
    // Code that should NOT trigger the rule
    { code: 'goodPattern()', filename: 'src/components/Foo.tsx' },
    // Code in an exempt directory
    { code: 'exemptedCall()', filename: 'src/allowed-dir/Bar.ts' },
  ],
  invalid: [
    // Code that SHOULD trigger the rule
    {
      code: 'badPattern()',
      filename: 'src/components/Baz.tsx',
      errors: [{ messageId: 'ruleViolation' }],
    },
  ],
});
```

---

## Severity Strategy

| Phase | Severity | Purpose |
|-------|----------|---------|
| Initial rollout | `warn` | Surface violations without blocking |
| After cleanup | `error` | Hard gate, fails CI |
| Intentional exceptions | `// eslint-disable-next-line drift-guard/rule -- reason` | Documented bypass |

Consider requiring a reason for all disable comments using `eslint-comments/require-description`
if the project uses `eslint-plugin-eslint-comments`.
