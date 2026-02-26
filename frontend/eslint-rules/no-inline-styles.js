/**
 * ESLint rule: no-inline-styles
 *
 * Bans the `style` attribute on JSX elements. All styling should use
 * CSS classes in component .css files or global framework styles.
 */
export default {
  meta: {
    type: "suggestion",
    docs: {
      description:
        "Disallow inline style attributes on JSX elements. Use CSS classes instead.",
    },
    messages: {
      noInlineStyle:
        "Inline style attribute found. Move styling to a CSS file using CSS classes. " +
        "For dynamic values, use CSS custom properties set via className + a scoped CSS rule.",
    },
    schema: [],
  },

  create(context) {
    return {
      JSXAttribute(node) {
        if (
          node.name &&
          node.name.type === "JSXIdentifier" &&
          node.name.name === "style"
        ) {
          context.report({
            node,
            messageId: "noInlineStyle",
          });
        }
      },
    };
  },
};
