# Footon service keys and remote reports

Footon Pro accounts can issue a separate service key for each remote auth or
infrastructure system. The remote system uses the Footon key to call Footon; it
does not send its Auth0, Okta, Cloudflare, AWS, or other provider credential to
Footon.

## Credential model

| Credential | Intended actor | Storage | Allowed authority |
| --- | --- | --- | --- |
| Interactive session | Signed-in person or local CLI | Operating system credential store | Manage owned keys, read reports, and manage shares |
| Service key | One named remote system | Secret store controlled by that system | Only the scopes selected when the key is issued |

Service key secrets begin with `ftn_sk_`, are returned once, and are stored by
Footon only as SHA-256 hashes. Key listings return the non-secret prefix,
system, scopes, creation time, expiry, last use, and revocation time. A service
key cannot issue another key.

| Scope | Service-key capability |
| --- | --- |
| `logs:write` | Submit bounded reports for the key's assigned system |
| `logs:read` | Read reports submitted with that key |
| `shares:read` | Read shares available to the owning Footon identity |
| `shares:write` | Create or change shares within the owning identity's normal permissions |

Issuance requires Pro. An account can have 20 active keys. Keys expire after 90
days by default; the issuer can select 1 to 365 days. Revocation takes effect on
the next request. Service keys pause when the owning account no longer has Pro
and resume if Pro is restored before the key expires. Expiry and revocation do
not depend on an upstream provider.

## Issue and use a key

Sign in as the owner, then issue a key. Save the `key` field from the response
in the remote system's secret store because Footon will not show it again.

```sh
footon signin you@example.com
footon key-create "Auth0 production" auth0-prod \
  --scope "logs:write logs:read" --expires-in-days 90
```

Remote automation supplies the key through `FOOTON_SERVICE_KEY`, not a command
argument:

```sh
export FOOTON_SERVICE_KEY='<one-time-key>'
footon report auth.login.failed "Login failures exceeded the alert threshold" evt-123 \
  --environment production --level error
```

The signed-in owner can review all owned systems or filter one system:

```sh
footon reports --system auth0-prod --limit 50
footon key-list
footon key-revoke '<key-id>'
```

## Remote report contract

`POST /api/log-reports` accepts one report per request. The service key supplies
the owner and system identity, so the payload cannot impersonate another
system.

| Field | Constraint |
| --- | --- |
| `environment` | 1 to 64 URL-safe identifier characters |
| `level` | `debug`, `info`, `warn`, `error`, or `critical` |
| `event` | 1 to 100 URL-safe identifier characters |
| `summary` | 1 to 2,000 characters before redaction |
| `sourceEventId` | 1 to 160 URL-safe identifier characters; idempotent per key |
| `occurredAt` | RFC 3339 timestamp, no more than 5 minutes in the future |

Footon redacts recognized bearer tokens, provider keys, private keys, email
addresses, connection strings, payment-card patterns, Social Security number
patterns, and private filesystem paths before storage. Reports contain a
summary, not a raw log body or attachment. Each key can submit 1,000 reports per
hour. Reports are retained for 30 days.

Remote MCP exposes the same lifecycle through `service_key_create`,
`service_key_list`, `service_key_revoke`, `log_report_create`, and
`log_report_list`. `log_report_create` requires a service key with `logs:write`.
