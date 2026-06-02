INSERT OR REPLACE INTO agent_sessions (
  id, agent_type, agent_name, pid, comm, start_timestamp_ms, end_timestamp_ms,
  status, model, input_tokens, output_tokens, total_tokens, adapter_id, confidence,
  attributes_json
)
SELECT
  'opencode-pid-' || pid,
  'opencode',
  'opencode',
  pid,
  comm,
  MIN(timestamp_ms),
  MAX(timestamp_ms),
  'observed',
  MAX(model),
  COALESCE(SUM(input_tokens), 0),
  COALESCE(SUM(output_tokens), 0),
  COALESCE(SUM(total_tokens), 0),
  'opencode',
  0.95,
  json_object('projection', 'cli-output')
FROM token_usage
WHERE adapter_id = 'opencode'
  AND source = 'opencode_stdout'
GROUP BY pid, comm;
