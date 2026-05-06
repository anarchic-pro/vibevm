# `vibe search` — full-text query over configured registries

Search every `[[registry]]` configured in `vibe.toml` for packages whose name, description, keywords, capabilities, or `describes` PURL match a query. Read-only — does not touch the lockfile, the registry cache, or any remote git host.

`vibe search` consults each registry's optional **package index** ([PROP-005](../../spec/modules/vibe-index/PROP-005-package-index.md)) — a per-org metadata service that ships separately from the registry itself. Without an index URL configured for a registry, that registry is reported as `registries_unconfigured` in the envelope and silently skipped. Without an index there is nothing fast to query — naive `git ls-remote`-shape enumeration across an org of 100+ packages would be unacceptably slow, so the search command refuses to do that. Operators that want search either run their own [`vibe-index`](../../services/vibe-index/) instance or wait for one to land at the upstream org.

Spec: [ROADMAP §M2.10](../../ROADMAP.md), [PROP-005 §2.10](../../spec/modules/vibe-index/PROP-005-package-index.md#index-routes), [PROP-004 §5.12](../../spec/research/PROP-004-tessl-comparative-research.md#search).

## Usage

```
vibe search <query>...
            [--kind <flow|feat|stack|tool>]
            [--registry <name>]
            [--limit <N>]
            [--path <dir>]
            [--json | --quiet]
```

Multiple positional arguments are joined with a single space before being sent to the index — `vibe search wal log` is one query, not two.

## Flags

| Flag | Description | Default |
| --- | --- | --- |
| `<query>...` | Free-text query. Tokenised server-side (lowercase ASCII alphanumeric runs, ~30-stopword filter, single-character tokens dropped). | — |
| `--kind <K>` | Restrict to one package kind. | all |
| `--registry <NAME>` | Walk only the named `[[registry]]`. Errors if `NAME` is not in `vibe.toml`. | walk every configured registry |
| `--limit <N>` | Maximum hits to fetch from each registry's index. The server may apply its own cap; the union is then deduplicated by `(kind, name)` keeping the highest-score variant. | `20` |
| `--path <dir>` | Project root with `vibe.toml`. | `.` |
| `--json` | Structured JSON envelope — see [Output (JSON)](#output-json). | off |
| `--quiet` | One-line summary `vibe search: N hits across M registries`. | off |

## Index URL convention

Per registry, set `VIBEVM_INDEX_URL_<NAME>` in the environment to point at a vibe-index server (or any endpoint that serves the [PROP-005 §2.10](../../spec/modules/vibe-index/PROP-005-package-index.md#index-routes) wire shape). The suffix is uppercase ASCII alphanumeric with non-alphanumeric characters folded to `_`:

- `vibespecs` → `VIBEVM_INDEX_URL_VIBESPECS`
- `vibespecs-gitverse` → `VIBEVM_INDEX_URL_VIBESPECS_GITVERSE`

The same convention is used by `vibe registry publish`'s post-publish hook ([PROP-005 §2.14](../../spec/modules/vibe-index/PROP-005-package-index.md#publish-hook)) and the `vibe-registry`-side index fast path. One env-var feeds every consumer.

For each registry, `vibe search`:

1. Reads `VIBEVM_INDEX_URL_<R>` from the environment. Unset → `registries_unconfigured`.
2. Probes `<base>/repomd.json` (and `<base>/v1/index/repomd.json` for the static-mirror layout). 200 → keep going. Anything else → `registries_unreachable`.
3. Fetches `<base>/v1/packages?q=<query>[&kind=][&limit=]`. Non-200 → `registries_unreachable`.

A 404 on the search route specifically means the URL points at a static raw-file mirror without the live-server route — search is unavailable on that mirror, but version-fetch (`list_versions`) still works through the `by-name/` files.

## Output

### Human-readable

```
query     : wal
registries: 1 searched, 0 unreachable, 1 without index URL
  searched: vibespecs
  no VIBEVM_INDEX_URL_<R> set: vibespecs-gitverse

KIND    NAME                          LATEST       SCORE  REGISTRY              DESCRIPTION
flow    wal                           0.1.0        3      vibespecs             Write-ahead log discipline for spec-driven projects.

1 hit across 1 registry
```

When no registry has an index URL configured, the summary line points at this doc:

```
(no registry has VIBEVM_INDEX_URL_<R> configured — see docs/commands/search.md)
```

### Output (JSON)

```jsonc
{
  "ok": true,
  "command": "search",
  "project": "/path/to/project",
  "query": "wal",
  "registries_searched": ["vibespecs"],
  "registries_unconfigured": ["vibespecs-gitverse"],
  "registries_unreachable": [],
  "hit_count": 1,
  "hits": [
    {
      "kind": "flow",
      "name": "wal",
      "latest_stable": "0.1.0",
      "score": 3,
      "matched_tokens": ["wal", "log", "ahead"],
      "description": "Write-ahead log discipline for spec-driven projects.",
      "registry": "vibespecs"
    }
  ]
}
```

`registries_unreachable[]` carries `{ name, reason }` per failure (HTTP status / connect-fail / malformed JSON). `ok` stays `true` even when every registry fails — the command surfaces the failure mode in the envelope rather than aborting, so a CI step that wants strict semantics can `jq -e '.registries_unreachable | length == 0'`.

## Examples

```bash
# Search every configured registry for the WAL flow.
vibe search wal

# Restrict to flows only.
vibe search wal --kind flow

# One specific registry.
vibe search atomic --registry vibespecs

# Multi-word query.
vibe search "ahead of time" --kind feat

# Higher limit for deep org-wide searches.
vibe search auth --limit 100

# Programmatic.
vibe --json search auth | jq '.hits[].name'
```

## Edge cases

- **Query is empty after trimming.** Errors before any HTTP call. Multi-arg queries that consist entirely of whitespace or stopwords still send the original string to the server, which decides matching semantics.
- **`--registry NAME` does not match any `[[registry]]`.** Errors with the list of configured registry names so the operator sees the typo.
- **No registry has an index URL set.** Hit count is 0; summary line surfaces the missing-config state. Not an error — this is the expected state for projects whose orgs do not run an index yet.
- **Registry probes succeed but search returns 503 / 5xx.** Reported as `registries_unreachable` for that registry; the run continues across the remaining registries.
- **Same `(kind, name)` shows up on two registries.** Deduplicated to one row; the row with the higher `score` wins. Ties resolve to whichever registry came earlier in `vibe.toml`.

## Limitations (v0)

- **Index-only.** Without a `VIBEVM_INDEX_URL_<R>` configured, a registry contributes nothing to search results. A future revision may add a `--full-scan` mode that walks `git ls-remote` across the org for cases where the operator accepts the latency cost.
- **No client-side caching.** Each invocation hits the index server cold. ROADMAP §M2.10 mentions `~/.vibe/search-cache/`; that lands when search becomes a frequent enough operation to make the cache pay for its complexity.
- **Stable-only ranking signal.** Server-side score is term-overlap (one point per matched query token). Tantivy / BM25 upgrades land server-side without a client change — the wire shape `{score: u32}` is the same.
- **No `describes`-aware search.** Querying for `pkg:cargo/sqlx@0.8.0` returns hits whose name or description happens to contain the literal string. The dedicated `/v1/purls/{purl}` route on the index server already exists ([PROP-005 §2.10](../../spec/modules/vibe-index/PROP-005-package-index.md#index-routes)); a future `vibe search --purl <P>` flag would dispatch there directly.

## Related

- [PROP-005](../../spec/modules/vibe-index/PROP-005-package-index.md) — full design of the per-org package index, including the `/v1/packages` query route and operator handbook.
- [`services/vibe-index/`](../../services/vibe-index/) — standalone utility that produces and serves the index.
- [`vibe registry publish`](registry-publish.md) — populates the index via the post-publish hook when `VIBEVM_INDEX_URL_<R>` and `VIBEVM_INDEX_TOKEN_<R>` are set.
- [`vibe outdated`](../../ROADMAP.md#m110--vibe-outdated) — cousins-by-source: same `IndexClient` underpins outdated-checks and search.
- [`vibe show config`](show.md) — surfaces the `VIBEVM_INDEX_URL_<R>` env-vars that decide which registries are searchable.
