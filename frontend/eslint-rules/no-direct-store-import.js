// drift-generated
/**
 * ESLint rule: no-direct-store-import
 *
 * Bans importing `usePipelineStore` directly in view files.
 * Views must use selector hooks from `usePipelineSelectors.ts` instead.
 * See ADR 013.
 */
export default {
  meta: {
    type: "suggestion",
    docs: {
      description:
        "Disallow direct imports of usePipelineStore in view files. Use usePipelineSelectors hooks instead.",
    },
    messages: {
      noDirectStore:
        "Do not import '{{ source }}' directly in views. " +
        "Import action and data hooks from 'data/usePipelineSelectors' instead (ADR 013).",
    },
    schema: [],
  },

  create(context) {
    const filename = context.filename || context.getFilename();
    const isView = /[\\/]views[\\/]/.test(filename);
    if (!isView) return {};

    return {
      ImportDeclaration(node) {
        const src = node.source.value;
        if (
          typeof src === "string" &&
          (src.includes("pipelineStore") || src.includes("PipelineStore"))
        ) {
          context.report({
            node,
            messageId: "noDirectStore",
            data: { source: src },
          });
        }
      },
    };
  },
};
