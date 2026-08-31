# runtime and ui

- application and runtime code must not load, parse, or reference `.env` or any `.env.*` file.
- application and runtime code can read only process-provided environment variables.
- use compile-time `option_env!` fallbacks only for explicit cases.
- fail production-only release and build metadata during compilation or build-script execution.
- do not abort application startup because of missing production-only metadata.
- do not put dialogs, tooltips, or popovers inside the embedded webview.
- use a native window, native dialog, or separate webview window for content that must escape circular widget bounds.
- the webview clips overflow to the app window.
- use a native window, native dialog, or separate webview window for explanatory or help content.
- do not use an in-webview modal for this content.
