# flow-vibevm-direct-push-smoke

Throwaway test fixture for the `vibe registry publish --repo-url` no-API path.

The contents do nothing useful. The point is to have a real, valid vibevm
package whose name screams "test" so that when it lands in a hosted git
repo, anyone reading the org page can tell at a glance that the repo is
not a production package and can be deleted at any time.

## How it gets used

```
vibe registry publish ./fixtures/manual-test-packages/flow-vibevm-direct-push-smoke \
    --repo-url <ssh-or-https-url-to-empty-repo> \
    --path <some-vibevm-project>
```

`<repo-url>` points at an already-provisioned, empty git repository on the
target host (GitVerse, GitHub, Gitea, anything `git push` understands). The
local user's git credentials handle authentication; vibevm's publish path
loads no token and makes no API call.
