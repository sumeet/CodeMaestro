-- Your SQL goes here
CREATE TABLE codes (
  id SERIAL PRIMARY KEY,
  added_by TEXT NOT NULL,
  code JSON NOT NULL,
  instance_id INTEGER NOT NULL,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

SELECT diesel_manage_updated_at('codes');
