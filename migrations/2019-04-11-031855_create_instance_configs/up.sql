-- Your SQL goes here
CREATE TABLE service_configs (
  id SERIAL PRIMARY KEY,
  nickname TEXT NOT NULL,
  instance_id INTEGER NOT NULL,
  service_type TEXT NOT NULL,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  config JSONB NOT NULL
);

SELECT diesel_manage_updated_at('service_configs');
