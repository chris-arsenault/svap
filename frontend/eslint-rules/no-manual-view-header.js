// drift-generated
/**
 * ESLint rule: no-manual-view-header
 *
 * Warns when a view uses raw <div className="view-header"> markup instead of
 * the shared <ViewHeader> component from SharedUI.tsx.
 * See ADR 019.
 */
export default {
  meta: {
    type: "suggestion",
    docs: {
      description:
        'Disallow raw view-header divs. Use <ViewHeader> from components/SharedUI instead.',
    },
    messages: {
      noManualViewHeader:
        'Use <ViewHeader title="..." description="..."> instead of raw view-header div. See ADR-019.',
    },
    schema: [],
  },

  create(context) {
    const filename = context.filename || context.getFilename();
    // Only enforce in view files
    if (!/[\\/]views[\\/]/.test(filename)) return {};

    return {
      JSXAttribute(node) {
        if (
          node.name.name === "className" &&
          node.value &&
          node.value.type === "Literal" &&
          typeof node.value.value === "string" &&
          /\bview-header\b/.test(node.value.value)
        ) {
          context.report({
            node: node.parent,
            messageId: "noManualViewHeader",
          });
        }
      },
    };
  },
};
