# vibevm-github-smoke — PROTOCOL

Test fixture used by the live smoke for `vibe registry publish` against
GitHub (the API path, with token + repo creation + tagged release).
Contents are deliberately trivial; the point is exercising the full
publish pipeline end to end against the real `vibespecs` GitHub
organisation without burning a real package name.

## What this package would do (if it were real)

Nothing. It is a no-op flow whose only artefact is this protocol file
plus the boot snippet under `spec/boot/`. Operators who accidentally
install it should `vibe uninstall flow:vibevm-github-smoke` and
forget about it.

## Versioning

Stays at `0.0.1` forever. Bumping it would defeat the "throwaway" point.
