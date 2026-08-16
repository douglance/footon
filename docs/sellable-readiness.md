Artifact: Software Requirements Specification FOOTON-SRS-001
Subject: Footon sellable product readiness
Status: in review
Version: 0.2
Owner: Product owner
Approvers: Douglas Lance
Inputs: User goal "finalize all work. professionalize it and we will begin selling"; user clarifications "same rules as Google Docs ... keep it ultra simple" and "public is free forever, private costs money"; release tag `v0.2.0` at c62a128bc35e04ac92062cc7ad25c06dff3bc89a; production Worker version 357e8348-5304-4605-9a8c-4b9c3c05c424
Governing references: ISO/IEC/IEEE 29148:2018 information model; ISO/IEC/IEEE 15289:2019 information-item model; README.md; cli/README.md; wrangler.jsonc
Tailoring: This specification combines product, quality, release, and launch requirements owned by the product-owner phase. It does not claim formal standards conformance or legal review.

# Footon sellable readiness

## Problem

Footon `v0.2.0` is published and deployed with passwordless CLI sign-in,
public and private shares, named roles, service keys, remote reports, billing
state, and customer information routes. Production D1 contains every migration
through `0007_service_keys_and_log_reports.sql`.

The current release work tightens the commercial invariant: public shares are
unlimited and free forever; private creation, access, and mutation require an
active Pro entitlement. When Pro ends, private data remains stored but pauses
until the owner renews, makes the share public, or revokes it. This change is
locally implemented and tested but is not yet committed or deployed.

Paid checkout is not active. Production lacks the Lemon Squeezy webhook secret
and monthly and annual checkout URLs, and the local Lemon Squeezy operator has
no API key. Catalog creation is a dashboard-only action. Footon is not ready to
accept money until those values exist and the complete test-mode billing
lifecycle passes.

## Scope

In scope:

- Preserve local-first drafting, explicit approval, server-side rescanning, unlisted links, blackouts, and revocation.
- Make terminal email-code sign-in the default reusable CLI authentication path.
- Add Google-style general access, named people, and simple per-share roles to Pro.
- Offer unlimited public sharing for free and one individual paid plan for private sharing.
- Integrate Lemon Squeezy as merchant of record through hosted checkout and signed webhooks.
- Add accurate pricing, legal, privacy, security, and support surfaces.
- Add CI, release, health, migration, rollback, and operating evidence.
- Verify the rendered customer experience at desktop, tablet, and mobile widths.

Out of scope for the first paid release:

- Teams, shared workspaces, seat administration, SAML, SCIM, or enterprise contracts.
- Public discovery, indexing, search, comments, analytics, or custom domains.
- A custom payment form or storage of payment-card data.
- Live coauthoring, arbitrary transcript rewriting, comments, suggestions, folders, groups, domains, or inherited workspace permissions.
- A formal legal opinion, compliance certification, or service-level agreement.

## Decisions

### DEC-COM-001: Launch plans

Footon MUST launch with these tax-inclusive USD prices:

- Free: $0, unlimited public shares, and no private shares.
- Pro monthly: $12 per month, unlimited public shares, and up to 100 private shares.
- Pro annual: $120 per year, unlimited public shares, and up to 100 private shares.

Rationale: A single paid tier reduces launch and support complexity. GitHub Copilot Pro is $10 per month. Raycast Pro starts at $8 per month. Lemon Squeezy charges 5% plus $0.50 and advises sellers below $10 to request custom pricing.

### DEC-COM-002: Payment boundary

Lemon Squeezy MUST remain the merchant of record. Footon MUST use hosted checkout. Footon MUST NOT receive payment-card data.

### DEC-AUTH-001: Credential boundary

The CLI MUST store refreshable credentials in the operating-system credential store. The CLI MUST NOT print access or refresh tokens during normal sign-in.

`FOOTON_TOKEN` MUST remain an explicit non-persistent override for automation and recovery.

### DEC-ACCESS-001: Simple share access model

Footon MUST apply a Google-style access envelope to each published snapshot.
The snapshot content remains immutable except for explicit blackouts and its
title.

- General access is `Restricted` or `Anyone with the link`.
- Every share is `Anyone with the link` by default.
- An owner with active Pro can explicitly create or convert a share to `Restricted`.
- A Pro owner can grant a normalized email address `Viewer` or `Editor` access.
- A Viewer can read the approved snapshot.
- An Editor can rename the share, apply blackouts, and add or remove Viewers.
- Only the Owner can change general access, grant or remove Editors, transfer ownership, or revoke the share.

Rationale: This model keeps the familiar Google sharing concepts without turning
Footon into a collaborative document editor. The access envelope can change;
the approved snapshot cannot gain new transcript content.

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
- BILL-008 Private share creation MUST enforce the private-share limit for the active entitlement; public shares MUST NOT consume that limit.
- BILL-009 A billing outage MUST NOT revoke existing access until the stored entitlement reaches its explicit expiry or grace deadline.
- BILL-010 The service MUST expose a customer-portal link for subscription management when Lemon Squeezy supplies one.
- BILL-011 When Pro ends, the service MUST pause private reads and mutations while preserving the owner's ability to make the share public or revoke it.
- BILL-012 Service keys MUST stop authenticating when the owning account no longer has active Pro.

### Share access and permissions

- ACCESS-001 When a user creates a share without explicit visibility, the service MUST store `Anyone with the link`.
- ACCESS-002 Public share creation and access MUST remain available without Pro and without a public-share quota.
- ACCESS-003 An Owner with active Pro MUST be able to switch a public share to `Restricted`; any Owner MUST be able to make an owned private share public.
- ACCESS-004 An Owner MUST be able to grant or remove `Viewer` and `Editor` access for a normalized email address.
- ACCESS-005 An Editor MUST be able to rename a share, apply an exact blackout, and grant or remove Viewer access.
- ACCESS-006 A Viewer MUST be able to read an authorized share as HTML or Markdown.
- ACCESS-007 Only the Owner MUST be able to grant Editor access, change general access, transfer ownership, or revoke a share.
- ACCESS-008 A recipient of a Restricted share MUST authenticate with a one-time email code before the service returns share content.
- ACCESS-009 The service MUST enforce the same permission decision for browser, Markdown, CLI, API, and MCP access.
- ACCESS-010 When access is removed, the service MUST reject the next read after the permission write succeeds.
- ACCESS-011 An unauthorized response MUST NOT reveal the share title, content, owner email, recipient list, or permission assignments.
- ACCESS-012 A Restricted share response MUST use `Cache-Control: private, no-store` and MUST NOT enter a shared cache.
- ACCESS-013 An Owner MUST be able to inspect every named person and role on a share.
- ACCESS-014 The service MUST reject duplicate, malformed, owner-shadowing, and unsupported role assignments before storage.
- ACCESS-015 The service MUST record a non-secret audit event for each permission mutation without logging an email address or share identifier.

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
- QUAL-009 The access-control test matrix MUST cover Owner, Editor, Viewer, unauthenticated, unauthorized, removed, revoked, Free, and expired-Pro states across HTML, Markdown, API, CLI, and MCP entry points.
- QUAL-010 A cached or replayed response MUST NOT expose Restricted share content after access removal or revocation.

## Risks

- RISK-001 Lemon Squeezy API authentication and catalog setup are not configured. This blocks live checkout and webhook proof.
- RISK-003 Production runs `v0.2.0`; the current public-free/private-paid retention changes remain undeployed until they are committed, reviewed, and released.
- RISK-004 The paid legal draft has not received legal review, and delivery to `support@footon.dev` is not yet verified.
- RISK-005 `worker/src/lib.rs` exceeds 2,600 lines, increasing review and release risk. The recorded split plan below limits further growth.
- RISK-006 Stored CLI credentials add a local secret-management dependency that requires platform-specific tests and recovery behavior.
- RISK-007 A route or content-negotiation branch that bypasses the permission decision could disclose a Restricted share.
- RISK-008 Browser viewer sessions use secure HTTP-only cookies, but the paid private lifecycle still needs live multi-account acceptance after checkout is configured.
- RISK-009 The term `Editor` can imply arbitrary rewriting. Footon limits editing to title changes, blackouts, and Viewer management.

## Verification matrix

| Requirement group | Verification method | Required evidence |
| --- | --- | --- |
| SAFE | Unit, integration, adversarial scanner, and live publish/revoke tests | Test IDs, one sanitized live share, revoked URL result |
| AUTH | Mock OAuth integration, credential-store adapter tests, and live email-code flow | No-token output assertion, refresh rotation, signout, live exit status |
| ACCESS | Permission-matrix integration tests plus live multi-account acceptance | General access transitions, role mutations, authorized reads, denied reads, cache headers, immediate removal |
| BILL | Signed fixture tests plus Lemon Squeezy test-mode checkout and webhook lifecycle | Event IDs, entitlement transitions, duplicate replay result |
| DOC | Route tests, copy review, content negotiation, link checks, and rendered inspection | HTML/Markdown bodies, screenshots, legal-review disposition |
| OPS | Health checks, structured-log inspection, migration list, deploy record, and rollback rehearsal | Committed SHA, D1 state, version ID, smoke report, rollback command |
| REL | CI run, package contents, tag, GitHub release, and production bundle digest | Green run URL, crate manifest, release URL, bundle hash |
| QUAL | Strict repository gate, accessibility checks, screenshots, performance sample | `npm run check`, audit results, three breakpoint captures, timings |

## Recorded split plan for `worker/src/lib.rs`

Owner: Douglas Lance. Target: the first maintenance release after paid launch.
The current release may retain the file only while the complete suite, strict
Clippy, Worker build, and rendered/runtime acceptance remain green and no
unrelated feature logic is added.

1. Move OAuth registration, email-code authorization, token exchange, refresh,
   revocation, metadata, and credential-row types to `worker/src/auth.rs`.
2. Move authenticated share create/list/blackout/revoke, public-share loading,
   and share-row types to focused modules below 500 production lines each.
3. Move MCP request parsing, capability declarations, and tool dispatch to
   `worker/src/mcp.rs` while retaining share operations behind typed functions.
4. Move route declarations, content negotiation, assets, health, response
   headers, and error mapping to `worker/src/http.rs`.
5. Leave `lib.rs` as Worker event wiring and scheduled cleanup, then require
   every production Rust file to remain at or below 500 lines.

Each extraction must preserve the existing public routes, OAuth/MCP wire shapes,
structured non-secret logs, and D1 statements. Acceptance is the full repository
release gate plus the same live auth, billing, publish, blackout, revoke, and
rendered-page smoke checks used for the paid launch.

## Evidence ledger

| Claim or requirement | Repository or live evidence | Interpretation | Confidence |
| --- | --- | --- | --- |
| Footon is local-first and safety-oriented | `README.md:3`, `README.md:60`; `cli/README.md:26` | Current public contract preserves local drafting and explicit token injection. | High |
| The complete current repository release gate passes | Apoc execution `01a00848-01e2-7340-a25a-0ac7bb4779ba` on 2026-08-16 | Formatting, strict Clippy, every workspace test, the optimized Worker Wasm build, RustSec audit of 394 dependencies, npm audit with zero vulnerabilities, and clean package verification passed. Four component doctest examples remain intentionally ignored. | High |
| The current pricing surface passed rendered acceptance | `/tmp/footon-visual-jgvJvX`; isolated local server `01a0083c-6895-7300-813f-a6e059ef7121`; browser executions `01a00842-b08d-7721-84b4-027d023b46a1` through `01a00843-8b2e-7453-ad12-d96b47011ab3` | Desktop 1440x900, tablet 1024x768, and mobile 390x844 captures were inspected. All viewports had no horizontal overflow; both email inputs had explicit labels; keyboard order moved from each email field to its matching checkout action; axe-core 4.12.1 reported zero WCAG A/AA violations and zero incomplete checks on desktop and mobile. | High |
| CLI session work is not live-accepted | `cli/src/session.rs`; `cli/tests/session.rs`; `cli/tests/signin.rs` | Mock OAuth tests cover storage, rotation, safe output, and sign-out; a fresh production code flow remains required. | Medium |
| Owner authentication can be reused | `migrations/0003_rust_oauth.sql`; `worker/src/lib.rs`; `cli/src/session.rs`; CLI sign-in tests | Email-code OAuth produces scoped user identities and reusable sessions. A fresh production acceptance run remains required for this release. | High |
| Viewer authorization is implemented | `migrations/0006_share_access.sql`; `worker/src/access.rs`; `worker/src/shares.rs` | Private shares use named Owner, Editor, and Viewer roles, email-code browser sessions, generic unauthorized responses, and private cache controls. | High |
| Mutations use one role matrix across HTTP and MCP | `worker/src/access.rs`; `worker/src/shares.rs`; `cli/src/sharing.rs` | Owners, Editors, and Viewers receive the documented fixed capabilities. Current tests cover the pure matrix and wire values; live multi-account acceptance remains required. | High |
| Commercial and legal routes are live and locally updated | `worker/src/ui/commercial.rs`; `worker/src/ui/pages.rs`; production Worker `357e8348-5304-4605-9a8c-4b9c3c05c424` | Production serves pricing, security, support, privacy, and terms as HTML and Markdown. The current checkout makes the free-public/paid-private boundary explicit and awaits deployment. | High |
| Production reproduces tagged `v0.2.0` | Tag `v0.2.0` at `c62a128bc35e04ac92062cc7ad25c06dff3bc89a`; Worker version `357e8348-5304-4605-9a8c-4b9c3c05c424` at 100% traffic | The deployed source and version were read back on 2026-08-15. Current uncommitted changes are intentionally not represented in production yet. | High |
| CI and release records are current for `v0.2.0` | GitHub CI run `31906958751`; release run `31906958766`; crates.io `footon` 0.2.0 | The published source release and CLI package are current. A new version, tag, CI run, release, and crates.io publication are required for the present changes. | High |
| Billing core is locally implemented; activation is unavailable | `worker/src/billing.rs`; `worker/src/billing_adapter.rs`; `migrations/0005_billing.sql`; local signed-webhook runtime on 2026-08-15 | Signature verification, replay protection, entitlement transitions, limits, checkout URL construction, and authenticated billing status are locally proven. No real store, variant, checkout, or webhook can be verified without Lemon Squeezy access. | High |
| Production D1 administration and schema are current | Remote migration readback on 2026-08-15; post-deployment bookmark `000000a5-00000000-000050c8-357feef709ad9401ea5190d1fa6285da` | Migrations through `0007_service_keys_and_log_reports.sql` are applied and no migration is pending. | High |
| Outbound email configuration is current | Wrangler Email Sending and destination-address reads on 2026-08-15; public DNS MX, SPF, DKIM, and DMARC resolution | `footon.dev` sending is enabled and the launch account address is a verified destination. Final personal-inbox receipt and CLI exchange remain pending. | High |
| Production observability is configured | `wrangler.jsonc:9` | Cloudflare Workers observability is enabled at full head sampling. Log content and alerts remain unverified. | Medium |

## Open items

- OPEN-COM-001 Owner: Douglas Lance. Configure a Lemon Squeezy API key and create the Footon Pro monthly and annual variants. Effect: blocks live checkout, webhook, and entitlement acceptance.
- OPEN-AUTH-001 Owner: Douglas Lance and release operator. Complete one live CLI email-code sign-in and record the credential-safe exit status and authenticated status result. Effect: blocks live acceptance of the default sign-in path.
- OPEN-REL-001 Owner: Release operator and reviewer. Version, commit, push, run CI, tag, publish, deploy, and smoke-test the current free-public/paid-private release with Worker version `357e8348-5304-4605-9a8c-4b9c3c05c424` recorded as the rollback target. Effect: blocks a reproducible production release.
- OPEN-LEGAL-001 Owner: Douglas Lance. Approve Douglas Lance as the named operator, verify and monitor `support@footon.dev`, approve the non-refundable-except-law policy, and approve the final legal copy. Effect: blocks declaring the paid terms approved.

## Handoff

Receiver: Release operations
Accepted inputs: FOOTON-SRS-001 version 0.2; user access-model and pricing clarifications; release tag `v0.2.0` at c62a128bc35e04ac92062cc7ad25c06dff3bc89a; production Worker 357e8348-5304-4605-9a8c-4b9c3c05c424
Decisions: DEC-COM-001 launch prices; DEC-COM-002 Lemon Squeezy boundary; DEC-AUTH-001 secure credential storage; DEC-ACCESS-001 simple share access model; DEC-LAUNCH-001 individual launch
Produced outputs: SAFE-001 through SAFE-006; AUTH-001 through AUTH-007; ACCESS-001 through ACCESS-015; BILL-001 through BILL-012; DOC-001 through DOC-005; OPS-001 through OPS-006; REL-001 through REL-003; QUAL-001 through QUAL-010
Verification evidence: Repository and live-state evidence ledger above
Open items: OPEN-COM-001, OPEN-AUTH-001, OPEN-REL-001, and OPEN-LEGAL-001
Acceptance checks: Preserve the immutable-snapshot boundary and the out-of-scope set; verify every public path remains free; verify every private and service-key path pauses without Pro; do not claim paid launch until every open item closes
