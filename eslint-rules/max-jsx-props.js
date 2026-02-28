// drift-generated
/**
 * ESLint rule: max-jsx-props
 *
 * Warns when a JSX element receives too many props. High prop counts
 * indicate a component is doing too much, or that props should be
 * grouped into domain objects (Parameter Object pattern).
 *
 * Counts JSXAttribute nodes. JSXSpreadAttribute ({...obj}) counts as 1
 * each to discourage blind spreading.
 */

export default {
  meta: {
    type: "suggestion",
    docs: {
      description: "Enforce a maximum number of props on JSX elements",
      category: "Best Practices",
    },
    schema: [
      {
        type: "object",
        properties: {
          max: { type: "integer", minimum: 1 },
        },
        additionalProperties: false,
      },
    ],
    messages: {
      tooManyProps:
        "Too many props ({{count}}). Max allowed is {{max}}. Consider grouping into domain objects.",
    },
  },

  create(context) {
    const max = context.options[0]?.max ?? 12;
    return {
      JSXOpeningElement(node) {
        const count = node.attributes.length;
        if (count > max) {
          context.report({
            node,
            messageId: "tooManyProps",
            data: { count: String(count), max: String(max) },
          });
        }
      },
    };
  },
};
