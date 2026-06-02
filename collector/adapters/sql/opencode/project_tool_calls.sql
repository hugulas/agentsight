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
    AND json_extract(c.attributes_json, '$.program') = 'opencode'
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
    AND json_extract(c.attributes_json, '$.program') = 'opencode'
    AND json_extract(c.attributes_json, '$.stream') = 'stdout'
    AND json_type(c.attributes_json, '$.parsed_json') = 'object'
)
SELECT
  'opencode-tool-' || event_id || '-' ||
    COALESCE(json_extract(message_json, '$.part.callID'), json_extract(message_json, '$.timestamp')),
  json_extract(message_json, '$.sessionID'),
  'opencode-conv-' || COALESCE(json_extract(message_json, '$.sessionID'), event_id),
  COALESCE(CAST(json_extract(message_json, '$.timestamp') AS INTEGER), timestamp_ms),
  json_extract(message_json, '$.part.tool'),
  json_extract(message_json, '$.part.callID'),
  COALESCE(json_extract(message_json, '$.part.state.status'), 'observed'),
  json_object(
    'redacted', 1,
    'filepath', json_extract(message_json, '$.part.state.metadata.filepath'),
    'title', json_extract(message_json, '$.part.state.title')
  ),
  NULL,
  pid,
  event_id,
  'opencode',
  0.90
FROM cli_messages
WHERE json_extract(message_json, '$.type') = 'tool_use';
