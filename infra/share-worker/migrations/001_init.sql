-- migrations/001_init.sql

CREATE TABLE IF NOT EXISTS shares (
  token       TEXT PRIMARY KEY,
  user_id     TEXT NOT NULL,           -- Supabase user UUID (from JWT sub)
  session_id  TEXT NOT NULL,
  title       TEXT,                    -- plaintext user-chosen label
  size_bytes  INTEGER NOT NULL DEFAULT 0,
  status      TEXT NOT NULL DEFAULT 'pending', -- pending | ready | deleted
  created_at  INTEGER NOT NULL,
  expires_at  INTEGER,                 -- null = no expiry
  view_count  INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_shares_user_id ON shares(user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_shares_status_created ON shares(status, created_at);

-- Sliding-window rate limit counters
CREATE TABLE IF NOT EXISTS rate_limits (
  key     TEXT NOT NULL,  -- "{user_id}:{endpoint}" or "{ip}:{endpoint}"
  window  INTEGER NOT NULL, -- unix timestamp floored to window size (seconds)
  count   INTEGER NOT NULL DEFAULT 1,
  PRIMARY KEY (key, window)
);
