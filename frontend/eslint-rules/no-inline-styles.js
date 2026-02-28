// drift-generated
/**
 * ESLint rule: no-inline-styles
 *
 * Bans the `style` attribute on JSX elements. All styling should use
 * CSS classes in component-local .css files or global framework styles.
 *
 * Exception: style attributes that ONLY set CSS custom properties
 * (keys starting with "--") are allowed, because CSS custom properties
 * are the idiomatic way to pass dynamic values into CSS classes.
 *
 * See: docs/adr/004-css-architecture.md
 * See: docs/patterns/css-architecture.md
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
        "Inline style attribute found. Move styling to a component-local .css file using CSS classes. " +
        "For dynamic values, use CSS custom properties set via className + a scoped CSS rule. " +
        "See docs/adr/004-css-architecture.md",
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
          // Allow style={{ '--custom-prop': value }} when ALL keys are custom properties
          if (isCustomPropertiesOnly(node.value)) return;

          context.report({
            node,
            messageId: "noInlineStyle",
          });
        }
      },
    };
  },
};

/**
 * Returns true if the style value is an object expression where every
 * key is a CSS custom property (starts with "--").
 *
 * Handles: style={{ '--foo': val }} and style={{ '--foo': val, '--bar': val2 }}
 * Does NOT allow: style={{ '--foo': val, color: 'red' }} (mixed)
 */
function isCustomPropertiesOnly(valueNode) {
  // style={expr} -> valueNode is JSXExpressionContainer
  if (!valueNode || valueNode.type !== "JSXExpressionContainer") return false;

  const expr = valueNode.expression;

  // style={{ ... }} -> expr is ObjectExpression
  if (!expr || expr.type !== "ObjectExpression") return false;

  // Empty object {} -- technically no visual styles, allow it
  if (expr.properties.length === 0) return true;

  return expr.properties.every((prop) => {
    if (prop.type === "SpreadElement") return false;
    const key = prop.key;
    if (key.type === "Literal" && typeof key.value === "string") {
      return key.value.startsWith("--");
    }
    // Identifier keys like `color` never start with "--"
    return false;
  });
}
