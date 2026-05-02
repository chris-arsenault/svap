// drift-generated
const expandPattern = /^(is)?expand(ed)?/i;

function isUseStateCallee(callee) {
  return (
    (callee.type === "Identifier" && callee.name === "useState") ||
    (callee.type === "MemberExpression" &&
      callee.property.type === "Identifier" &&
      callee.property.name === "useState")
  );
}

function isUseStateDeclarator(node) {
  return (
    node.id.type === "ArrayPattern" &&
    node.init &&
    node.init.type === "CallExpression" &&
    isUseStateCallee(node.init.callee)
  );
}

function stateName(node) {
  const firstEl = node.id.elements[0];
  return firstEl?.type === "Identifier" ? firstEl.name : null;
}

/**
 * ESLint rule: no-manual-expand-state
 *
 * Warns when a view/component manually implements the expand/collapse toggle
 * pattern instead of using useExpandSingle() or useExpandSet() from hooks.ts.
 *
 * Detects: const [expanded*, setExpanded*] = useState(...)
 * Ignores: hooks.ts itself, store files, and non-component files.
 * See ADR 020.
 */
export default {
  meta: {
    type: "suggestion",
    docs: {
      description:
        "Disallow manual useState for expand/collapse state. Use useExpandSingle() or useExpandSet() from hooks.ts.",
    },
    messages: {
      noManualExpandState:
        "Use useExpandSingle() or useExpandSet() from hooks.ts instead of manual expand state '{{ name }}'. See ADR-020.",
    },
    schema: [],
  },

  create(context) {
    const filename = context.filename || context.getFilename();

    // Only enforce in view and component files
    const isTarget =
      /[\\/](views|components)[\\/]/.test(filename);
    if (!isTarget) return {};

    return {
      VariableDeclarator(node) {
        if (!isUseStateDeclarator(node)) return;
        const name = stateName(node);
        if (!name) return;

        if (expandPattern.test(name)) {
          context.report({
            node,
            messageId: "noManualExpandState",
            data: { name },
          });
        }
      },
    };
  },
};
