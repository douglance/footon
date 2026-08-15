# Footon release checklist

Release version: `__________`  Commit: `________________________________________`

Release operator: `________________`  Reviewer: `________________`

## Source and quality

- [ ] Version matches `cli/Cargo.toml`, `crates/footon-core/Cargo.toml`, tag,
      and dated `CHANGELOG.md` entry.
- [ ] Worktree is clean and the exact commit is pushed.
- [ ] CI quality job is green: `________________`.
- [ ] Linux, Apple silicon macOS, Intel macOS, and Windows CLI jobs are green.
- [ ] Rust and npm dependency audits are green.
- [ ] `footon-core` and `footon` package verification is green.
- [ ] Release workflow produced four native archives and SHA-256 files.

## Product and policy

- [ ] Pricing, limits, checkout, entitlement, customer portal, cancellation,
      renewal, refund, and duplicate-webhook behavior passed test mode.
- [ ] Privacy, terms, refund, security, pricing, and support copy is approved.
- [ ] The private support channel is monitored and its recovery procedure works.
- [ ] No known critical or high security defect remains open.
- [ ] Files over 500 production lines have an accepted split plan.

## Production

- [ ] Intended Cloudflare account, Worker, route, and D1 database were read back.
- [ ] Prior Worker version ID and current D1 bookmark were recorded.
- [ ] Remote migrations were applied and read back before deployment.
- [ ] Deployment message contains the full committed Git revision.
- [ ] New Worker version and deployment IDs were recorded.
- [ ] HTML and Markdown landing, legal, pricing, support, and share surfaces passed.
- [ ] Live email-code sign-in, Keychain persistence, refresh, publish, blackout,
      revoke, and sign-out passed without credential output.
- [ ] Test checkout and the complete billing lifecycle passed in production mode.
- [ ] Structured logs contain the required events and no secrets or personal data.
- [ ] Accessibility captures passed at 1440x900, 1024x768, and 390x844.
- [ ] Production p95 public-page response time is below 500 milliseconds.
- [ ] Worker rollback was rehearsed against a non-production version or preview.

## Publish and recovery

- [ ] GitHub release is published only after production smoke acceptance.
- [ ] Crate publication order is `footon-core`, then `footon`.
- [ ] Release URLs, crate manifests, bundle digest, deployment ID, migration list,
      screenshots, timings, and smoke report are archived together.
- [ ] Release operator and reviewer signed the result.

If any required item is unchecked, the release remains a draft and paid launch
does not begin. Use [operations.md](operations.md) for rollback and recovery.
