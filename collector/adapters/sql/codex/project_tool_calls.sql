INSERT OR REPLACE INTO tool_calls (
  id, session_id, conversation_id, timestamp_ms, tool_name, tool_call_id,
  status, input_json, output_json, related_pid, related_event_id, adapter_id,
  confidence
)
WITH cli_messages AS (
  SELECT
    c.id AS event_id,
    c.timestamp_ms,
    c.pid,
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
    json_extract(c.attributes_json, '$.parsed_json') AS message_json
  FROM canonical_events c
  WHERE c.source = 'cli_output'
    AND json_extract(c.attributes_json, '$.program') = 'codex'
    AND json_extract(c.attributes_json, '$.stream') = 'stdout'
    AND json_type(c.attributes_json, '$.parsed_json') = 'object'
)
SELECT
  'codex-tool-' || event_id || '-' ||
    COALESCE(json_extract(message_json, '$.item.id'), timestamp_ms),
  NULL,
  'codex-conv-' || event_id,
  timestamp_ms,
  json_extract(message_json, '$.item.type'),
  json_extract(message_json, '$.item.id'),
  COALESCE(json_extract(message_json, '$.item.status'), 'observed'),
  json_object(
    'redacted', 1,
    'command_redacted', json_extract(message_json, '$.item.command_redacted'),
    'exit_code', json_extract(message_json, '$.item.exit_code')
  ),
  NULL,
  pid,
  event_id,
  'codex',
  0.90
FROM cli_messages
WHERE json_extract(message_json, '$.type') = 'item.completed'
  AND json_extract(message_json, '$.item.type') = 'command_execution';
