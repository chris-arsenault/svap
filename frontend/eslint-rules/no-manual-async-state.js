// drift-generated
const asyncNamePattern = /^(is)?(busy|loading|submitting)/i;
const errorNamePattern = /error/i;

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

function isLiteralValue(node, values) {
  return node?.type === "Literal" && values.includes(node.value);
}

function shouldReportAsyncState(name, initArg) {
  if (asyncNamePattern.test(name)) {
    return !initArg || isLiteralValue(initArg, [false, null]);
  }
  if (errorNamePattern.test(name)) {
    return !initArg || isLiteralValue(initArg, [null]);
  }
  return false;
}

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
        if (insideStoreCreate > 0) return;
        if (!isUseStateDeclarator(node)) return;

        const name = stateName(node);
        if (!name) return;
        const args = node.init.arguments;
        const initArg = args.length > 0 ? args[0] : null;

        if (shouldReportAsyncState(name, initArg)) {
          context.report({
            node,
            messageId: "noManualAsyncState",
            data: { name },
          });
        }
      },
    };
  },
};
