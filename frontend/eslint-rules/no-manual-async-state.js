// drift-generated
/**
 * ESLint rule: no-manual-async-state
 *
 * Warns when a component uses useState with names matching busy/loading/error
 * patterns for async operations. Use the useAsyncAction() hook instead.
 * See ADR 015.
 */
export default {
  meta: {
    type: "suggestion",
    docs: {
      description:
        "Disallow manual useState for async state (busy, loading, error). Use useAsyncAction() from hooks.ts instead.",
    },
    messages: {
      noManualAsyncState:
        "Use useAsyncAction() from hooks.ts instead of manual async state '{{ name }}'. See ADR-015.",
    },
    schema: [],
  },

  create(context) {
    const filename = context.filename || context.getFilename();

    // Only enforce inside component/view/hook files, not in store create() calls
    const isComponent =
      /[\\/](views|components|hooks)[\\/]/.test(filename) ||
      filename.endsWith("hooks.ts") ||
      filename.endsWith("hooks.tsx");

    // Skip the hooks file that defines useAsyncAction itself
    if (/[\\/]hooks\.(ts|tsx)$/.test(filename)) return {};
    if (!isComponent) return {};

    // Track whether we are inside a zustand create() call
    let insideStoreCreate = 0;

    return {
      // Detect entering a zustand create() or store factory
      "CallExpression[callee.name='create']"() {
        insideStoreCreate++;
      },
      "CallExpression[callee.name='create']:exit"() {
        insideStoreCreate--;
      },

      VariableDeclarator(node) {
        // Skip if inside a store create() call
        if (insideStoreCreate > 0) return;

        // Must be: const [foo, setFoo] = useState(...)
        if (
          node.id.type !== "ArrayPattern" ||
          !node.init ||
          node.init.type !== "CallExpression"
        ) {
          return;
        }

        const callee = node.init.callee;
        const isUseState =
          (callee.type === "Identifier" && callee.name === "useState") ||
          (callee.type === "MemberExpression" &&
            callee.property.type === "Identifier" &&
            callee.property.name === "useState");

        if (!isUseState) return;

        // Get the first element name (the state variable)
        const firstEl = node.id.elements[0];
        if (!firstEl || firstEl.type !== "Identifier") return;

        const name = firstEl.name;
        const asyncNamePattern = /^(is)?(busy|loading|submitting)/i;
        const errorNamePattern = /error/i;

        // Check initial value to confirm async-state usage
        const args = node.init.arguments;
        const initArg = args.length > 0 ? args[0] : null;

        // For busy/loading/submitting: flag if useState(false) or useState<...>(null) or useState(null)
        if (asyncNamePattern.test(name)) {
          if (
            !initArg ||
            (initArg.type === "Literal" &&
              (initArg.value === false || initArg.value === null))
          ) {
            context.report({
              node,
              messageId: "noManualAsyncState",
              data: { name },
            });
            return;
          }
        }

        // For error: flag if useState(null) or useState<string|null>(null)
        if (errorNamePattern.test(name)) {
          if (
            !initArg ||
            (initArg.type === "Literal" && initArg.value === null)
          ) {
            context.report({
              node,
              messageId: "noManualAsyncState",
              data: { name },
            });
          }
        }
      },
    };
  },
};
