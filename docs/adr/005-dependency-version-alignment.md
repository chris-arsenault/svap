<!-- drift-generated -->
# ADR-005: Dependency Version Alignment

## Status

Accepted (2026-02-26)

## Context

The monorepo had a "two-cohort split": archivist and chronicler were scaffolded
with newer tooling (vite 7.x, @vitejs/plugin-react 5.x, TypeScript ~5.9.3)
while the other 7 apps remained on older versions (vite 6.x, plugin-react 4.x,
TypeScript 5.3-5.7). This created systematic drift in dependencies, build
behavior, and module federation compatibility.

## Decision

All apps and shared packages now target a single dependency cohort:

| Dependency | Version | Range |
|---|---|---|
| vite | 7.x | ^7.2.4 |
| @vitejs/plugin-react | 5.x | ^5.1.1 |
| @module-federation/vite | 1.x | ^1.3.1 |
| react / react-dom | 19.x | ^19.2.0 |
| typescript | 5.9.x | ~5.9.3 |

TypeScript uses tilde (`~`) rather than caret (`^`) because minor TS versions
can introduce breaking type-checking changes.

### Module/resolution strategy

All tsconfig.json files use `module: "ESNext"` and
`moduleResolution: "bundler"`. The previous Node16/NodeNext/ES2022 variants
are eliminated.

### ESLint configuration

The root `eslint.config.js` is the sole authority. Per-app ESLint configs and
redundant per-app ESLint devDependencies are deleted. Apps inherit the root
config which includes typescript-eslint, SonarJS, jsx-a11y, react-perf,
prettier, and custom rules.

### Module Federation shared config

`config/federation.js` provides `federationOnWarn` (suppress benign eval
warnings) and `sharedDeps(...keys)` (composable shared dependency builder).
All vite configs import from this utility.

## Consequences

- `npm install` must be re-run after these changes to update the lockfile
- Any new app scaffolded must use the versions above
- Per-app eslint.config.js files should not be created
- Per-app eslint devDependencies should not be added to package.json
