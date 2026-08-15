# Footon production operations

This runbook controls production changes to the `footon` Cloudflare Worker and
the D1 database named `footon` (`6a2b3e9a-df44-45c6-853f-a9df34670cfd`). Run
commands from the repository root with the checked-in Wrangler version.

## Roles

| Actor | Authority | Required evidence |
| --- | --- | --- |
| Release operator | Build, migrate, deploy, smoke test, and initiate Worker rollback | Commit, tag, CI URL, D1 bookmark, prior and new Worker version IDs |
| Incident lead | Approve a destructive D1 Time Travel restore | Incident record, restore point, current bookmark, affected data window |
| Reviewer | Confirm evidence and release/rollback outcome | Checklist sign-off and production readback |

The release operator and reviewer must be different people for a paid release.
Never paste OAuth credentials, email codes, API tokens, database exports, or
customer data into a release record.

## Preconditions

1. The release commit is clean, pushed, and tagged with the version in
   `cli/Cargo.toml` and `CHANGELOG.md`.
2. CI is green on that exact commit. The release workflow has produced all four
   native CLI archives and checksums.
3. `npm ci`, `npm run check`, `cargo audit --deny warnings`,
   `npm audit --audit-level=high`, and
   `cargo package --workspace --exclude footon-worker --locked` pass.
4. `npx wrangler whoami` names the intended production account.
5. The release operator records the current Worker deployment and D1 bookmark:

   ```sh
   npx wrangler deployments list --json
   npx wrangler d1 info footon
   npx wrangler d1 time-travel info footon
   npx wrangler d1 migrations list footon --remote
   ```

Stop if the D1 identity, production storage version, unapplied migration list,
or rollback target is ambiguous.

## Paid-service configuration

Configure these values in the intended production Worker without putting secrets
in Git, shell history, logs, screenshots, or release records:

| Binding | Kind | Required value and check |
| --- | --- | --- |
| `LEMON_SQUEEZY_WEBHOOK_SECRET` | Wrangler secret | Exact signing secret for the production Footon webhook; verify with one signed test-mode event and one invalid-signature rejection. |
| `LEMON_SQUEEZY_MONTHLY_CHECKOUT_URL` | Worker variable | HTTPS `*.lemonsqueezy.com/checkout/buy/...` URL for the $12 monthly Pro variant. |
| `LEMON_SQUEEZY_ANNUAL_CHECKOUT_URL` | Worker variable | HTTPS `*.lemonsqueezy.com/checkout/buy/...` URL for the $120 annual Pro variant. |
| `EMAIL` | Cloudflare Send Email binding | Sends one-time codes from `login@footon.dev`. |
| `SHARE_ACCESS_WRITES_ENABLED` | Worker variable | `true` enables private creation and member/visibility expansion. Set to `false` to stop new private expansion while keeping public shares and existing authorized private reads available. |
| `support@footon.dev` | Monitored mailbox or route | Receive and reply to a private test message; verify the recovery owner and spam handling. |

The Lemon Squeezy webhook target is
`https://footon.dev/webhooks/lemon-squeezy`. Subscribe it to subscription and
order events required for purchase, renewal, cancellation, expiry, and refund.
Hosted checkout must prefill and return the same normalized Footon email. Never
copy a webhook body into a release record because it contains customer data.

## Deploy

Apply migrations before code that depends on them receives traffic. Migration
files must be forward-compatible with the currently deployed Worker.

```sh
npx wrangler d1 migrations apply footon --remote
npx wrangler d1 migrations list footon --remote
npx wrangler deploy --message "release v0.1.0 commit <full-git-sha>"
npx wrangler deployments list --json
```

Replace the version and commit literally. Save Wrangler's new version ID and
deployment output in the smoke report. A successful build is not a successful
deployment; the production readback is required.

## Production smoke test

Verify each response from `https://footon.dev` and record status, content type,
security headers, elapsed time, and the Worker version ID:

| Surface | Expected result |
| --- | --- |
| `/healthz` | `200` with `{"status":"ok"}` when D1 is reachable; otherwise `503` without dependency detail |
| `/` as HTML | `200`, landing page, no internal implementation copy |
| `/` as Markdown | `200`, agent-first installation and sharing instructions |
| `/.well-known/oauth-authorization-server` | `200`, production OAuth endpoints and scopes |
| `/privacy` and `/terms` | `200` as HTML and Markdown |
| `/pricing`, `/security`, and `/support` | `200` as HTML and Markdown; prices, limits, safety boundary, and private contact match approved copy |
| One existing public share | `200` as HTML and Markdown |
| One unauthorized private share | Generic email-code page as HTML and `401` for Markdown; no title, owner, or member disclosure |
| One authorized private share | Viewer can read; Editor can rename, blackout, and manage Viewers; only Owner can change visibility, manage Editors, transfer, or revoke |
| Public/private billing boundary | Public creation remains available without Pro or a private-share slot; private creation and expansion require active Pro capacity |
| Service-key lifecycle | Pro owner can issue, list, use, and revoke a scoped key; a revoked key receives `401`; listings never return the secret |
| Remote reports | Seeded report is idempotent, stored under the issuing owner and key, redacts a bearer token, and is hidden from other owners and keys |
| Unknown share | `404` without internal error detail |
| OAuth request and email code | Correct normalized email, one code, no credential output |
| CLI publish and revoke | New sanitized share is readable, then becomes `404` after revoke |
| Authenticated `/api/billing` | Current plan, usage, limit, expiry, and validated Lemon Squeezy portal URL match D1 |
| Monthly and annual checkout | `303` to the expected Lemon Squeezy checkout host with the normalized Footon email; no card data reaches Footon |
| Billing lifecycle | Purchase, renewal, cancellation grace, expiry, refund, and exact duplicate event produce the required entitlement state |

Inspect structured Worker logs for the smoke window. Authentication, billing,
share-create, blackout, and revoke failures must contain only the operation,
result, and static reason. They must not include tokens, codes, email addresses,
local paths, request bodies, share IDs, or transcript content.

## Worker rollback

Worker rollback is the first recovery action when the schema remains compatible.
It creates a new deployment pointing at the selected earlier Worker version; it
does not change D1.

```sh
npx wrangler deployments list --json
npx wrangler rollback <known-good-version-id> --message "rollback <incident-id> to <full-git-sha>"
npx wrangler deployments list --json
```

Repeat the production smoke test and record the new rollback deployment ID. To
recover from a mistaken rollback, deploy the previously active version with
`npx wrangler versions deploy <version-id>@100% -y` after review.

## D1 recovery

D1 Time Travel overwrites production data in place and cancels in-flight work.
Use it only when a migration or data mutation damaged production and a Worker
rollback cannot restore service.

1. Stop write-producing operational work and record the incident start time.
2. Capture the current bookmark so the restore itself can be undone:

   ```sh
   npx wrangler d1 time-travel info footon
   ```

3. Resolve and review the intended pre-incident bookmark:

   ```sh
   npx wrangler d1 time-travel info footon --timestamp="<RFC3339-time>"
   ```

4. After incident-lead approval, restore the exact reviewed bookmark:

   ```sh
   npx wrangler d1 time-travel restore footon --bookmark=<reviewed-bookmark>
   ```

5. Verify migrations, authentication, entitlement data, active shares, and the
   full production smoke test. Record the customer-data window that was lost or
   replayed.

If the restore point was wrong, use the bookmark captured in step 2 to undo the
restore. Do not edit the `d1_migrations` table manually.

## Failed-release recovery

- Failed verification: do not deploy; fix the source and cut a new tag.
- Failed migration before deploy: keep the current Worker active, inspect the D1
  transaction result, and use Time Travel only if current data changed.
- Failed deploy after migrations: roll back the Worker only if the old version
  remains compatible; otherwise ship a forward fix.
- Failed smoke test: roll back before announcing the release, retain the draft
  release record, and attach the failure evidence without secrets.
- Partial GitHub release: the release workflow leaves it as a draft. Rerun the
  failed job or delete the draft after preserving its evidence.

## Sources

- [Cloudflare Worker deployment and rollback commands](https://developers.cloudflare.com/workers/wrangler/commands/workers/)
- [Cloudflare Worker versions and deployments](https://developers.cloudflare.com/workers/versions-and-deployments/)
- [Cloudflare D1 migrations](https://developers.cloudflare.com/d1/reference/migrations/)
- [Cloudflare D1 Time Travel](https://developers.cloudflare.com/d1/reference/time-travel/)
