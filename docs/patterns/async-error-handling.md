<!-- drift-generated -->
# Async Error Handling Pattern Guide

**ADR:** [015-async-error-handling](../adr/015-async-error-handling.md)

## Quick Rules

1. Every view with async operations MUST use `useAsyncAction()` from `hooks.ts`
2. Every view with `useAsyncAction()` MUST render `<ErrorBanner />` at the top
3. Never `console.error` and swallow — surface errors to the user
4. Never use raw `useState(false)` for busy tracking

## Pattern

```tsx
import { useAsyncAction } from "../hooks";
import { ErrorBanner } from "../components/SharedUI";

export default function MyView() {
  const { busy, error, run, clearError } = useAsyncAction();

  const handleSubmit = () =>
    run("Submitting", async () => {
      await apiPost("/some/endpoint", data);
    });

  return (
    <div>
      <ErrorBanner error={error} onDismiss={clearError} />

      <button onClick={handleSubmit} disabled={busy !== null}>
        {busy === "Submitting" ? "Submitting..." : "Submit"}
      </button>
    </div>
  );
}
```

## API Reference

### `useAsyncAction()`

```tsx
const { busy, error, run, clearError } = useAsyncAction();
```

| Field | Type | Description |
|-------|------|-------------|
| `busy` | `string \| null` | Label of the running action, or `null` |
| `error` | `string \| null` | Error message from the last failed action |
| `run(label, fn)` | `(string, () => Promise<unknown>) => Promise<void>` | Wraps an async fn with busy/error tracking |
| `clearError()` | `() => void` | Dismisses the error |

### `<ErrorBanner />`

```tsx
<ErrorBanner error={error} onDismiss={clearError} />
```

| Prop | Type | Description |
|------|------|-------------|
| `error` | `string \| null` | Error message to display (hidden when null) |
| `onDismiss` | `() => void` | Optional dismiss callback |

Renders with `.error-banner` CSS class. Includes a dismiss button when `onDismiss` is provided.

## Multiple Async Actions

For views with several independent async actions, `run()` handles serialization automatically.
Each `run()` call sets the busy label to identify which action is running:

```tsx
const handleApprove = () => run("Approving", async () => { ... });
const handleReject = () => run("Rejecting", async () => { ... });

<button disabled={busy !== null}>
  {busy === "Approving" ? "Approving..." : "Approve"}
</button>
<button disabled={busy !== null}>
  {busy === "Rejecting" ? "Rejecting..." : "Reject"}
</button>
```

## What NOT To Do

```tsx
// BAD: raw boolean busy state
const [loading, setLoading] = useState(false);

// BAD: swallowing errors
try { await apiPost(...) } catch (e) { console.error(e); }

// BAD: custom error state without ErrorBanner
const [err, setErr] = useState<string | null>(null);
{err && <div className="my-custom-error">{err}</div>}

// GOOD: useAsyncAction + ErrorBanner
const { busy, error, run, clearError } = useAsyncAction();
<ErrorBanner error={error} onDismiss={clearError} />
```
