// drift-generated
/**
 * ESLint rule: no-direct-fetch
 *
 * Bans calling `fetch()` directly and importing `getToken` in view/data files.
 * All HTTP calls must go through `apiGet()`/`apiPost()` from `data/api.ts`.
 * See ADR 014.
 */
export default {
  meta: {
    type: "suggestion",
    docs: {
      description:
        "Disallow direct fetch() calls and manual auth header construction. Use apiGet/apiPost from data/api.ts.",
    },
    messages: {
      noDirectFetch:
        "Do not call fetch() directly. Use apiGet() or apiPost() from 'data/api.ts' instead (ADR 014).",
      noGetTokenImport:
        "Do not import getToken in view files. Auth is handled by apiGet/apiPost from 'data/api.ts' (ADR 014).",
    },
    schema: [],
  },

  create(context) {
    const filename = context.filename || context.getFilename();
    // Only enforce in views and data layer (not in api.ts itself)
    const isViewOrData =
      /[\\/](views|data|components)[\\/]/.test(filename) &&
      !filename.endsWith("api.ts");
    if (!isViewOrData) return {};

    return {
      CallExpression(node) {
        if (
          node.callee.type === "Identifier" &&
          node.callee.name === "fetch"
        ) {
          context.report({
            node,
            messageId: "noDirectFetch",
          });
        }
      },
      ImportDeclaration(node) {
        const src = node.source.value;
        if (typeof src !== "string") return;
        // Ban importing getToken in views
        if (/[\\/]views[\\/]/.test(filename) && src.includes("auth")) {
          const importsGetToken = node.specifiers.some(
            (s) =>
              s.type === "ImportSpecifier" &&
              (s.imported.name === "getToken" || s.imported.name === "authHeaders"),
          );
          if (importsGetToken) {
            context.report({
              node,
              messageId: "noGetTokenImport",
            });
          }
        }
      },
    };
  },
};
