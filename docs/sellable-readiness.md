Artifact: Software Requirements Specification FOOTON-SRS-001
Subject: Footon sellable product readiness
Status: in review
Version: 0.1
Owner: Product owner
Approvers: Douglas Lance
Inputs: User goal "finalize all work. professionalize it and we will begin selling"; release source tag be7c91a92f0f3644d5dfa9aaef1753cf79c292bc; release-workflow revision cbc161257576a35b2867ef21d7c4f0b48c8d75f5; production Worker version 514d05e0-9646-4c66-9613-0c8e12fedb38
Governing references: ISO/IEC/IEEE 29148:2018 information model; ISO/IEC/IEEE 15289:2019 information-item model; README.md; cli/README.md; wrangler.jsonc
Tailoring: This specification combines product, quality, release, and launch requirements owned by the product-owner phase. It does not claim formal standards conformance or legal review.

# Footon sellable readiness

## Problem

Footon has a committed, pushed, and packaged safety-first release candidate,
but the production service does not yet form a sellable release baseline.

The `v0.1.0` source tag, green CI record, and verified draft release artifacts
now exist. Production D1 includes the billing schema, but production still
serves an older Worker without the health, commercial, support, or billing
routes. Secure reusable CLI sessions are implemented and tested but still
require live email-code acceptance. Lemon Squeezy catalog, checkout, and webhook
configuration remain unavailable until an operator supplies account credentials
and creates the dashboard-only variants.

## Scope

In scope:

- Preserve local-first drafting, explicit approval, server-side rescanning, unlisted links, blackouts, and revocation.
- Make terminal email-code sign-in the default reusable CLI authentication path.
- Add one individual paid plan and one bounded free plan.
- Integrate Lemon Squeezy as merchant of record through hosted checkout and signed webhooks.
- Add accurate pricing, legal, privacy, security, and support surfaces.
- Add CI, release, health, migration, rollback, and operating evidence.
- Verify the rendered customer experience at desktop, tablet, and mobile widths.

Out of scope for the first paid release:

- Teams, shared workspaces, seat administration, SAML, SCIM, or enterprise contracts.
- Public discovery, indexing, search, comments, analytics, or custom domains.
- A custom payment form or storage of payment-card data.
- A formal legal opinion, compliance certification, or service-level agreement.

## Decisions

### DEC-COM-001: Launch plans

Footon MUST launch with these tax-inclusive USD prices:

- Free: $0, up to 3 active shares.
- Pro monthly: $12 per month, up to 100 active shares.
- Pro annual: $120 per year, up to 100 active shares.

Rationale: A single paid tier reduces launch and support complexity. GitHub Copilot Pro is $10 per month. Raycast Pro starts at $8 per month. Lemon Squeezy charges 5% plus $0.50 and advises sellers below $10 to request custom pricing.

### DEC-COM-002: Payment boundary

Lemon Squeezy MUST remain the merchant of record. Footon MUST use hosted checkout. Footon MUST NOT receive payment-card data.

### DEC-AUTH-001: Credential boundary

The CLI MUST store refreshable credentials in the operating-system credential store. The CLI MUST NOT print access or refresh tokens during normal sign-in.

`FOOTON_TOKEN` MUST remain an explicit non-persistent override for automation and recovery.

### DEC-LAUNCH-001: Launch boundary

The first paid release serves individual developers. Team and enterprise capabilities remain out of scope.

## Functional requirements

### Safety and sharing

- SAFE-001 Before a network request, the CLI MUST create and validate a local `footon.share.v2` draft.
- SAFE-002 Before publication, the user or agent MUST explicitly approve the exact sanitized draft.
- SAFE-003 During publication, the Worker MUST validate the wire shape and rescan the content.
- SAFE-004 An authenticated owner MUST be able to list, blackout, and revoke each active share.
- SAFE-005 A revoked share MUST stop returning its content immediately after the revocation write succeeds.
- SAFE-006 The service MUST reject a share larger than the documented limit before storing it.

### Authentication

- AUTH-001 When a user runs `footon signin <email>`, the CLI MUST send the normalized email to Footon and prompt for one six-digit code.
- AUTH-002 The CLI MUST keep the one-time code out of process arguments and command history.
- AUTH-003 After token exchange, the CLI MUST store the account, client, access, refresh, expiry, scope, and resource in the operating-system credential store.
- AUTH-004 An authenticated command MUST use `FOOTON_TOKEN` when set; otherwise it MUST use the stored session.
- AUTH-005 Before an expired stored access token is used, the CLI MUST rotate the refresh token and replace the stored session.
- AUTH-006 `footon signout` MUST revoke the stored token family and remove the local session.
- AUTH-007 Authentication errors MUST name a safe recovery action without returning credentials or OAuth state.

### Plans and billing

- BILL-001 The pricing surface MUST show the Free, Pro monthly, and Pro annual prices from DEC-COM-001.
- BILL-002 The Pro call to action MUST open a Lemon Squeezy hosted checkout for the selected billing interval.
- BILL-003 The checkout MUST associate the purchaser's normalized email with the Footon entitlement.
- BILL-004 The webhook endpoint MUST verify the Lemon Squeezy signature before reading or storing an event.
- BILL-005 The webhook endpoint MUST process each Lemon Squeezy event ID at most once.
- BILL-006 A paid, trialing, or grace-period subscription MUST grant the Pro entitlement.
- BILL-007 An expired, refunded, or fully canceled subscription MUST remove the Pro entitlement.
- BILL-008 Share creation MUST enforce the active-share limit for the current entitlement.
- BILL-009 A billing outage MUST NOT revoke existing access until the stored entitlement reaches its explicit expiry or grace deadline.
- BILL-010 The service MUST expose a customer-portal link for subscription management when Lemon Squeezy supplies one.

### Customer information

- DOC-001 The service MUST provide `/pricing`, `/privacy`, `/terms`, `/security`, and `/support` as HTML and Markdown.
- DOC-002 The privacy policy MUST identify stored data, processors, purposes, retention periods, deletion controls, billing data boundaries, and a private contact method.
- DOC-003 The terms MUST identify the operator, paid-plan renewal and cancellation behavior, refund policy, acceptable use, content rights, warranty boundary, and contact method.
- DOC-004 The security page MUST describe the local-first boundary, rescanning, unlisted-link risk, reporting channel, and supported-version policy without claiming immunity from secret leakage.
- DOC-005 The support page MUST state response channels, required diagnostic information, and credential-safe reporting instructions.

### Release and operations

- OPS-001 `GET /healthz` and `HEAD /healthz` MUST return a non-secret readiness response when the Worker can access D1.
- OPS-002 `HEAD` MUST match the status and headers of public `GET` routes without returning a body.
- OPS-003 Every deployment MUST identify a committed Git revision and a rollback target.
- OPS-004 Production migrations MUST be applied before code that depends on them receives traffic.
- OPS-005 The repository MUST provide a rollback procedure for Worker and D1 changes.
- OPS-006 Authentication, billing-webhook, share-create, blackout, and revoke failures MUST emit structured non-secret logs.
- REL-001 Pull requests and the default branch MUST run formatting, strict Clippy, workspace tests, Worker build, package verification, and dependency audit.
- REL-002 A release MUST have a version, changelog entry, Git tag, GitHub release, packaged CLI artifact, and production smoke report.
- REL-003 The checked-in release commit MUST reproduce the production Worker bundle.

## Quality requirements

- QUAL-001 The Rust workspace MUST compile with `unsafe_code = "forbid"` and Clippy `all`, `pedantic`, `cognitive_complexity`, and `too_many_lines` denied where configured.
- QUAL-002 No production Rust source file SHOULD exceed 500 lines. A larger file MUST have a recorded split plan before release.
- QUAL-003 Every secret-bearing response or log MUST be tested for token, code, state, email, path, and connection-string leakage.
- QUAL-004 Authentication and billing mutations MUST have idempotency or replay protection.
- QUAL-005 Public pages MUST meet WCAG 2.2 AA intent for semantics, keyboard access, focus visibility, contrast, form errors, and reduced motion.
- QUAL-006 The landing, pricing, auth, legal, support, and public-share surfaces MUST be inspected at 1440x900, 1024x768, and 390x844.
- QUAL-007 The p95 Worker response time for cached public pages SHOULD remain below 500 milliseconds from a US Cloudflare location during the launch smoke test.
- QUAL-008 A test purchase, renewal, cancellation, refund, and duplicate webhook MUST produce the expected entitlement state before paid launch.

## Risks

- RISK-001 Lemon Squeezy API authentication and catalog setup are not configured. This blocks live checkout and webhook proof.
- RISK-003 Production remains on an older Worker version. The current production service does not reproduce the tagged release candidate until the controlled deployment and smoke test succeed.
- RISK-004 The paid legal draft has not received legal review, and delivery to `support@footon.dev` is not yet verified.
- RISK-005 `worker/src/lib.rs` exceeds 2,600 lines, increasing review and release risk. The recorded split plan below limits further growth.
- RISK-006 Stored CLI credentials add a local secret-management dependency that requires platform-specific tests and recovery behavior.

## Verification matrix

| Requirement group | Verification method | Required evidence |
| --- | --- | --- |
| SAFE | Unit, integration, adversarial scanner, and live publish/revoke tests | Test IDs, one sanitized live share, revoked URL result |
| AUTH | Mock OAuth integration, credential-store adapter tests, and live email-code flow | No-token output assertion, refresh rotation, signout, live exit status |
| BILL | Signed fixture tests plus Lemon Squeezy test-mode checkout and webhook lifecycle | Event IDs, entitlement transitions, duplicate replay result |
| DOC | Route tests, copy review, content negotiation, link checks, and rendered inspection | HTML/Markdown bodies, screenshots, legal-review disposition |
| OPS | Health checks, structured-log inspection, migration list, deploy record, and rollback rehearsal | Committed SHA, D1 state, version ID, smoke report, rollback command |
| REL | CI run, package contents, tag, GitHub release, and production bundle digest | Green run URL, crate manifest, release URL, bundle hash |
| QUAL | Strict repository gate, accessibility checks, screenshots, performance sample | `npm run check`, audit results, three breakpoint captures, timings |

## Recorded split plan for `worker/src/lib.rs`

Owner: Douglas Lance. Target: first maintenance release after v0.1.0. The v0.1.0
release may retain the current file only while the complete suite, strict Clippy,
Worker build, and rendered/runtime acceptance remain green and no unrelated
feature logic is added.

1. Move OAuth registration, email-code authorization, token exchange, refresh,
   revocation, metadata, and credential-row types to `worker/src/auth.rs`.
2. Move authenticated share create/list/blackout/revoke, public-share loading,
   and share-row types to `worker/src/shares.rs`.
3. Move MCP request parsing, capability declarations, and tool dispatch to
   `worker/src/mcp.rs` while retaining share operations behind typed functions.
4. Move route declarations, content negotiation, assets, health, response
   headers, and error mapping to `worker/src/http.rs`.
5. Leave `lib.rs` as Worker event wiring and scheduled cleanup, then require
   every production Rust file to remain at or below 500 lines.

Each extraction must preserve the existing public routes, OAuth/MCP wire shapes,
structured non-secret logs, and D1 statements. Acceptance is the full repository
release gate plus the same live auth, billing, publish, blackout, revoke, and
rendered-page smoke checks used for v0.1.0.

## Evidence ledger

| Claim or requirement | Repository or live evidence | Interpretation | Confidence |
| --- | --- | --- | --- |
| Footon is local-first and safety-oriented | `README.md:3`, `README.md:60`; `cli/README.md:26` | Current public contract preserves local drafting and explicit token injection. | High |
| The complete repository release gate passes | Apoc execution `01a0037d-22e6-7081-8dc9-c39bd3d91c2c`; GitHub CI run `31867295310`; `npm run verify:release` on 2026-08-15 | Formatting, strict Clippy, 84 executable Rust tests, release Wasm build, RustSec audit of 394 dependencies, npm audit with zero vulnerabilities, and clean package verification passed. Four component doctest examples remain intentionally ignored. CI succeeded on main commit `cbc161257576a35b2867ef21d7c4f0b48c8d75f5`. | High |
| Required browser surfaces pass local rendered acceptance | `/tmp/footon-visual-acceptance-20260815-0308`; isolated `agent-browser` session on 2026-08-15 | Landing, pricing, security, support, privacy, terms, authorization, and public-share pages were inspected at 1440x900, 1024x768, and 390x844 with no horizontal overflow. Axe reported zero violations after fixes; the scrollable minimap moved from `scrollTop` 0 to 350 by keyboard and updated `aria-valuenow`. Local landing/pricing measurements were TTFB 7.1/2.9 ms, FCP/LCP 108/100 ms, and CLS 0. | High |
| CLI session work is not live-accepted | `cli/src/session.rs`; `cli/tests/session.rs`; `cli/tests/signin.rs` | Mock OAuth tests cover storage, rotation, safe output, and sign-out; a fresh production code flow remains required. | Medium |
| Commercial and legal routes are implemented locally | `worker/src/ui/commercial.rs`; `worker/src/ui/pages.rs`; `worker/src/lib.rs` | Pricing, security, support, privacy, and terms are available as HTML and Markdown. Operator, billing, retention, cancellation, refund, and private-contact copy is drafted but not approved or deployed. | High |
| Production and the tagged release are divergent | Wrangler Worker version `514d05e0-9646-4c66-9613-0c8e12fedb38`; tag `v0.1.0` at `be7c91a92f0f3644d5dfa9aaef1753cf79c292bc`; production `/healthz` returned `404` on 2026-08-15 | Email-code OAuth is live, but the finalized health, commercial, support, and billing release candidate remains undeployed. | High |
| CI and draft release records are current | GitHub CI run `31867295310`; release run `31867732822`; draft `Footon v0.1.0`; eight archive and checksum assets independently verified on 2026-08-15 | Main is clean and pushed, CI is green, the immutable source tag exists, and the four native CLI packages have portable checksums. The release intentionally remains a draft until production deployment and paid-service acceptance succeed. | High |
| Billing core is locally implemented; activation is unavailable | `worker/src/billing.rs`; `worker/src/billing_adapter.rs`; `migrations/0005_billing.sql`; local signed-webhook runtime on 2026-08-15 | Signature verification, replay protection, entitlement transitions, limits, checkout URL construction, and authenticated billing status are locally proven. No real store, variant, checkout, or webhook can be verified without Lemon Squeezy access. | High |
| Production D1 administration and billing schema are current | Apoc executions `01a0038c-21de-7022-87a4-94aab60e04ae`, `01a0038c-d453-7290-9cc1-188dfb88e5e8`, and `01a0038d-5d5b-7fe3-9677-ea63e13374b2`; post-migration bookmark `00000090-00000002-000050c8-a40bd268cc6baae0fdf60bca165d91c8` | Direct remote reads succeed, `0005_billing.sql` is applied, no migrations remain, and all three billing tables exist. | High |
| Outbound email configuration is current | Wrangler Email Sending and destination-address reads on 2026-08-15; public DNS MX, SPF, DKIM, and DMARC resolution | `footon.dev` sending is enabled and the launch account address is a verified destination. Final personal-inbox receipt and CLI exchange remain pending. | High |
| Production observability is configured | `wrangler.jsonc:9` | Cloudflare Workers observability is enabled at full head sampling. Log content and alerts remain unverified. | Medium |

## Open items

- OPEN-COM-001 Owner: Douglas Lance. Configure a Lemon Squeezy API key and create the Footon Pro monthly and annual variants. Effect: blocks live checkout, webhook, and entitlement acceptance.
- OPEN-AUTH-001 Owner: Douglas Lance and release operator. Complete one live CLI email-code sign-in and record the credential-safe exit status and authenticated status result. Effect: blocks live acceptance of the default sign-in path.
- OPEN-REL-001 Owner: Release operator and reviewer. Supply a scoped Cloudflare API token to the GitHub `production` environment, deploy `v0.1.0` with Worker version `514d05e0-9646-4c66-9613-0c8e12fedb38` recorded as the rollback target, and record the production smoke and rollback evidence. The environment and `CLOUDFLARE_ACCOUNT_ID` already exist. Effect: blocks a reproducible production release.
- OPEN-LEGAL-001 Owner: Douglas Lance. Approve Douglas Lance as the named operator, verify and monitor `support@footon.dev`, approve the non-refundable-except-law policy, and approve the final legal copy. Effect: blocks declaring the paid terms approved.

## Handoff

Receiver: Release operations
Accepted inputs: FOOTON-SRS-001 version 0.1; release source tag be7c91a92f0f3644d5dfa9aaef1753cf79c292bc; release-workflow revision cbc161257576a35b2867ef21d7c4f0b48c8d75f5; production Worker 514d05e0-9646-4c66-9613-0c8e12fedb38
Decisions: DEC-COM-001 launch prices; DEC-COM-002 Lemon Squeezy boundary; DEC-AUTH-001 secure credential storage; DEC-LAUNCH-001 individual launch
Produced outputs: SAFE-001 through SAFE-006; AUTH-001 through AUTH-007; BILL-001 through BILL-010; DOC-001 through DOC-005; OPS-001 through OPS-006; REL-001 through REL-003; QUAL-001 through QUAL-008
Verification evidence: Repository and live-state evidence ledger above
Open items: OPEN-COM-001, OPEN-AUTH-001, OPEN-REL-001, and OPEN-LEGAL-001
Acceptance checks: Preserve the out-of-scope set; trace every work item to a requirement; do not claim paid launch until all four open items close
