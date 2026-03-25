# macos notarization workflow

Add macOS Developer ID signing (+ hardened runtime + timestamp) to our existing GitHub Actions workflow that builds and ships a DMG for our Rust/Wry app.

Goal: produce a DMG that passes Gatekeeper (“just open and run”) by doing:

1. codesign the built .app bundle with Developer ID Application cert (hardened runtime + timestamp)
2. build the DMG containing that signed .app
3. (optional but recommended) codesign the DMG container too
4. notarize the DMG with xcrun notarytool and then staple the ticket to the DMG
5. verify with spctl

Implementation notes:

- Use an ephemeral keychain in the workflow (security create-keychain / import cert / set-key-partition-list) so codesign can run non-interactively.
- Read signing material from GitHub Secrets: base64-encoded .p12 for “Developer ID Application”, its password, a keychain password, and notarization creds (Apple ID + app-specific password + Team ID, or ASC API key).
- Ensure nested code inside the .app (Frameworks, helpers, XPC services, plugins) is signed correctly; if needed, sign nested items first, then sign the outer .app last.
- Use codesign flags: --options runtime --timestamp --force --sign "<Developer ID Application: … (TEAMID)>"
- After signing: codesign --verify --deep --strict --verbose=2 on the .app; after stapling: spctl -a -vv on the DMG.
- Ideally will be a `make` task that can be run locally, and the github action will simply call that `make` task with the appropriate env vars loaded
