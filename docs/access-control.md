# Footon access control

Footon has two visibility settings. Public shares are free forever. Private
shares and their named access controls require an active Pro plan and available
private-share capacity.

| Share setting | Who can read | Billing rule |
| --- | --- | --- |
| Public | Anyone with the link | Free forever; no Pro check and no private-share limit |
| Private | Owner and named Editors or Viewers | Pro required to create, convert to private, or add members |

Private roles use this fixed permission model:

| Action | Owner | Editor | Viewer |
| --- | --- | --- | --- |
| Read | Yes | Yes | Yes |
| Rename | Yes | Yes | No |
| Blackout text | Yes | Yes | No |
| View access list | Yes | Yes | No |
| Add or remove Viewers | Yes | Yes | No |
| Add or remove Editors | Yes | No | No |
| Change public/private | Yes | No | No |
| Transfer ownership | Yes | No | No |
| Revoke share | Yes | No | No |

Making a share public removes its named member grants and outstanding viewer
codes. Moving it back to private starts with the owner only. Ownership transfer
requires the destination email to have an existing Footon identity; on a private
share, the destination must also have Pro.

Browsers opening a private link see a generic email form. Footon sends a
six-digit code only when the normalized email is allowed, while returning the
same page either way. Codes expire after 10 minutes and allow five attempts.
Successful verification creates a 30-day secure, HTTP-only browser session.
Private responses use `Cache-Control: private, no-store`.

Private access depends on the owner's active Pro entitlement. When Pro ends,
Footon keeps the private share and its member list, but pauses private reads and
mutations. The owner can renew Pro, make the share public, or revoke it. Public
shares remain readable and do not consume private-share capacity.

The access model is shared by the HTTP API, CLI commands, local stdio MCP, and
the remote OAuth MCP endpoint. `SHARE_ACCESS_WRITES_ENABLED=false` stops new
private expansion without disabling public creation or reads of existing shares.

Service keys are separate Pro automation credentials. They act as the owning
Footon identity only for their explicit scopes and cannot issue more keys.
Remote log reports remain isolated by owner and service key. See
[Footon service keys and remote reports](service-keys.md).
