// drift-generated
/**
 * ESLint rule: no-escape-hatches
 *
 * Prevents patterns that bypass framework protections:
 * 1. Methods that return internal objects (getGraph, getMapper, getInternal*)
 * 2. Fallback defaults for required config (config ?? default, config || default)
 * 3. Deprecated method definitions that should have been removed
 */

export default {
  meta: {
    type: 'problem',
    docs: {
      description: 'Disallow escape hatches that bypass framework protections',
      category: 'Best Practices',
    },
    messages: {
      noInternalAccessor: "Method '{{name}}' returns internal object. Expose specific methods instead of raw access.",
      noConfigFallback: "No fallback defaults for config. If '{{name}}' is required, make it required. If missing, throw.",
      noDeprecatedMethod: "Deprecated methods must be removed, not left for compatibility. Delete '{{name}}' and fix callers.",
    },
  },

  create(context) {
    return {
      // Rule 1: Ban methods that return internal objects
      MethodDefinition(node) {
        const name = node.key.name;

        // Ban getGraph, getInternalGraph, getMapper, getInternal*
        if (name === 'getGraph' ||
            name === 'getInternalGraph' ||
            name === 'getMapper' ||
            name.startsWith('getInternal')) {
          context.report({
            node,
            messageId: 'noInternalAccessor',
            data: { name },
          });
        }

        // Check for @deprecated in comments - if present, method should be deleted
        const comments = context.getCommentsBefore(node);
        const hasDeprecated = comments.some(c => c.value.includes('@deprecated'));
        if (hasDeprecated) {
          context.report({
            node,
            messageId: 'noDeprecatedMethod',
            data: { name },
          });
        }
      },

      // Rule 2: Ban fallback defaults for config-like parameters
      // Catches: config ?? defaultConfig, options || {}, config?.foo ?? 'default'
      LogicalExpression(node) {
        if (node.operator === '??' || node.operator === '||') {
          const leftName = node.left.name || node.left.property?.name || '';

          // Flag if left side looks like config/options/context
          if (/config|options|context|settings/i.test(leftName)) {
            context.report({
              node,
              messageId: 'noConfigFallback',
              data: { name: leftName },
            });
          }
        }
      },

      // Also catch conditional expressions: config ? config : default
      ConditionalExpression(node) {
        const testName = node.test.name || node.test.property?.name || '';
        if (/config|options|context|settings/i.test(testName)) {
          context.report({
            node,
            messageId: 'noConfigFallback',
            data: { name: testName },
          });
        }
      },
    };
  },
};
