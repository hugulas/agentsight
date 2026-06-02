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
    AND json_extract(c.attributes_json, '$.program') = 'codex'
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
    AND json_extract(c.attributes_json, '$.program') = 'codex'
    AND json_extract(c.attributes_json, '$.stream') = 'stdout'
    AND json_type(c.attributes_json, '$.parsed_json') = 'object'
),
turn_completed AS (
  SELECT
    event_id,
    timestamp_ms,
    pid,
    comm,
    COALESCE(CAST(json_extract(message_json, '$.usage.input_tokens') AS INTEGER), 0) AS input_tokens,
    COALESCE(CAST(json_extract(message_json, '$.usage.output_tokens') AS INTEGER), 0) AS output_tokens,
    COALESCE(CAST(json_extract(message_json, '$.usage.cached_input_tokens') AS INTEGER), 0) AS cache_read_tokens
  FROM cli_messages
  WHERE json_extract(message_json, '$.type') = 'turn.completed'
    AND json_extract(message_json, '$.usage') IS NOT NULL
)
SELECT
  'codex-output-token-' || event_id || '-' || timestamp_ms,
  NULL,
  timestamp_ms,
  pid,
  comm,
  'openai',
  'codex',
  input_tokens,
  output_tokens,
  0,
  cache_read_tokens,
  input_tokens + output_tokens,
  'codex_stdout',
  'codex',
  0.95
FROM turn_completed
WHERE input_tokens > 0 OR output_tokens > 0 OR cache_read_tokens > 0;
