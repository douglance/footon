CREATE TABLE IF NOT EXISTS billing_event_receipts (
  event_id TEXT PRIMARY KEY,
  event_name TEXT NOT NULL,
  source_type TEXT NOT NULL,
  source_id TEXT NOT NULL,
  user_id TEXT,
  subscription_id TEXT,
  order_id TEXT,
  customer_id TEXT,
  test_mode INTEGER NOT NULL DEFAULT 0 CHECK (test_mode IN (0, 1)),
  received_at TEXT NOT NULL,
  processed_at TEXT,
  error TEXT
);

CREATE INDEX IF NOT EXISTS billing_event_receipts_user_idx
  ON billing_event_receipts (user_id, received_at DESC);

CREATE INDEX IF NOT EXISTS billing_event_receipts_subscription_idx
  ON billing_event_receipts (subscription_id, received_at DESC);

CREATE TABLE IF NOT EXISTS billing_subscriptions (
  subscription_id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL,
  email TEXT NOT NULL,
  status TEXT NOT NULL CHECK (
    status IN (
      'on_trial',
      'active',
      'paused',
      'past_due',
      'unpaid',
      'cancelled',
      'cancelled_grace',
      'expired'
    )
  ),
  pro_valid_until TEXT,
  grace_until TEXT,
  renews_at TEXT,
  ends_at TEXT,
  trial_ends_at TEXT,
  customer_id TEXT,
  order_id TEXT,
  product_id TEXT,
  variant_id TEXT,
  customer_portal_url TEXT,
  test_mode INTEGER NOT NULL DEFAULT 0 CHECK (test_mode IN (0, 1)),
  source_event_id TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS billing_subscriptions_user_idx
  ON billing_subscriptions (user_id, updated_at DESC);

CREATE INDEX IF NOT EXISTS billing_subscriptions_customer_idx
  ON billing_subscriptions (customer_id);

CREATE TABLE IF NOT EXISTS user_entitlements (
  user_id TEXT PRIMARY KEY,
  plan TEXT NOT NULL CHECK (plan IN ('free', 'pro')),
  active_share_limit INTEGER NOT NULL CHECK (active_share_limit >= 0),
  valid_until TEXT NOT NULL,
  grace_until TEXT,
  source_subscription_id TEXT,
  source_event_id TEXT NOT NULL,
  customer_portal_url TEXT,
  test_mode INTEGER NOT NULL DEFAULT 0 CHECK (test_mode IN (0, 1)),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  revoked_at TEXT
);

CREATE INDEX IF NOT EXISTS user_entitlements_active_idx
  ON user_entitlements (plan, valid_until, revoked_at);
