INSERT OR REPLACE INTO token_usage (
  id, llm_call_id, timestamp_ms, pid, comm, provider, model,
  input_tokens, output_tokens, cache_creation_tokens, cache_read_tokens,
  total_tokens, source, adapter_id, confidence
)
WITH cli_messages AS (
  SELECT
    c.id AS event_id,
    c.timestamp_ms,
    c.pid,
    c.comm,
    msg.value AS message_json
  FROM canonical_events c,
       json_each(c.attributes_json, '$.parsed_json') AS msg
  WHERE c.source = 'cli_output'
    AND json_extract(c.attributes_json, '$.program') = 'opencode'
    AND json_extract(c.attributes_json, '$.stream') = 'stdout'
    AND json_type(c.attributes_json, '$.parsed_json') = 'array'

  UNION ALL

  SELECT
    c.id AS event_id,
    c.timestamp_ms,
    c.pid,
    c.comm,
    json_extract(c.attributes_json, '$.parsed_json') AS message_json
  FROM canonical_events c
  WHERE c.source = 'cli_output'
    AND json_extract(c.attributes_json, '$.program') = 'opencode'
    AND json_extract(c.attributes_json, '$.stream') = 'stdout'
    AND json_type(c.attributes_json, '$.parsed_json') = 'object'
),
step_finish AS (
  SELECT
    event_id,
    COALESCE(CAST(json_extract(message_json, '$.timestamp') AS INTEGER), timestamp_ms) AS timestamp_ms,
    pid,
    comm,
    message_json,
    COALESCE(CAST(json_extract(message_json, '$.part.tokens.input') AS INTEGER), 0) AS input_tokens,
    COALESCE(CAST(json_extract(message_json, '$.part.tokens.output') AS INTEGER), 0) AS output_tokens,
    COALESCE(CAST(json_extract(message_json, '$.part.tokens.cache.write') AS INTEGER), 0) AS cache_creation_tokens,
    COALESCE(CAST(json_extract(message_json, '$.part.tokens.cache.read') AS INTEGER), 0) AS cache_read_tokens,
    COALESCE(CAST(json_extract(message_json, '$.part.tokens.total') AS INTEGER), 0) AS total_tokens
  FROM cli_messages
  WHERE json_extract(message_json, '$.type') = 'step_finish'
    AND json_extract(message_json, '$.part.tokens') IS NOT NULL
)
SELECT
  'opencode-output-token-' || event_id || '-' || timestamp_ms,
  NULL,
  timestamp_ms,
  pid,
  comm,
  'opencode',
  'opencode',
  input_tokens,
  output_tokens,
  cache_creation_tokens,
  cache_read_tokens,
  CASE
    WHEN total_tokens > 0 THEN total_tokens
    ELSE input_tokens + output_tokens + cache_creation_tokens + cache_read_tokens
  END,
  'opencode_stdout',
  'opencode',
  0.95
FROM step_finish
WHERE (input_tokens > 0 OR output_tokens > 0 OR cache_creation_tokens > 0 OR cache_read_tokens > 0 OR total_tokens > 0);
