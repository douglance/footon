export const TOOL_DEFINITIONS = [
  {
    name: 'share_create',
    description:
      'Publish a locally sanitized and explicitly approved conversation. Raw threads are rejected.',
    inputSchema: {
      type: 'object',
      properties: {
        document: { type: 'object', description: 'A footon.share.v1 sanitized document.' },
      },
      required: ['document'],
      additionalProperties: false,
    },
  },
  {
    name: 'share_list',
    description: 'List the authenticated owner’s shares, including revoked shares.',
    inputSchema: { type: 'object', properties: {}, additionalProperties: false },
    annotations: { readOnlyHint: true },
  },
  {
    name: 'share_revoke',
    description: 'Revoke one owned share immediately.',
    inputSchema: {
      type: 'object',
      properties: { id: { type: 'string', minLength: 20, maxLength: 40 } },
      required: ['id'],
      additionalProperties: false,
    },
    annotations: { destructiveHint: true },
  },
]
