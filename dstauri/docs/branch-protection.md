# Branch protection

`main` is production. It is protected so a release can only come from a merged,
CI-verified pull request.

## Applied rules (repository `Alaric-senpai/tsuki-app`)

- Require a pull request before merging (direct pushes rejected).
- Require 1 approving review; stale reviews are **not** dismissed.
- Require status check **All checks** to pass; branches must be up to date before merging (`strict: true`).
- Force pushes and branch deletion are disabled.
- Administrators are **not** additionally restricted (`enforce_admins: false`), so an admin can still
  recover the branch in an emergency. Keep that exception in mind when reviewing.

`All checks` is the aggregate job at the bottom of `.github/workflows/ci.yml`; it succeeds only when
both the Rust gates (`cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`) and the frontend
gates (typecheck, test, lint, static-export build) succeed. CI runs on pushes and PRs to `main`,
`master` and `1.1.2`, so the check is present on a PR from `1.1.2` into `main`.

The Release workflow additionally verifies that `All checks` succeeded for *the exact commit* a `v*`
tag points at before building installers, so a tag cannot bypass CI.

## How the rules were applied

```sh
gh api -X PUT repos/Alaric-senpai/tsuki-app/branches/main/protection \
  -H 'Accept: application/vnd.github+json' \
  --input main-protection.json
```

with `main-protection.json`:

```json
{
  "required_status_checks": {"strict": true, "contexts": ["All checks"]},
  "enforce_admins": false,
  "required_pull_request_reviews": {"required_approving_review_count": 1},
  "restrictions": null,
  "allow_force_pushes": false,
  "allow_deletions": false
}
```

To verify: `gh api repos/Alaric-senpai/tsuki-app/branches/main/protection`.
The same settings can be set in the GitHub web UI under **Settings → Branches → Branch protection
rules → Add rule** for `main`.
