INSERT OR REPLACE INTO tool_calls (
  id, session_id, conversation_id, timestamp_ms, tool_name, tool_call_id,
  start_timestamp_ms, end_timestamp_ms, duration_ms, status, input_json,
  output_json, related_pid, related_event_id, adapter_id, confidence
)
WITH tool_uses AS (
  SELECT
    c.id AS llm_call_id,
    c.pid AS pid,
    c.response_event_id AS response_event_id,
    COALESCE(c.end_timestamp_ms, c.start_timestamp_ms) AS start_ms,
    json_extract(e.value, '$.parsed_data.content_block.name') AS tool_name,
    json_extract(e.value, '$.parsed_data.content_block.id') AS tool_call_id,
    COALESCE(json_extract(e.value, '$.parsed_data.content_block.input'), '{}') AS input_json,
    e.key AS event_key
  FROM llm_calls c,
       json_each(c.response_body_json, '$.sse_events') AS e
  WHERE c.response_body_json IS NOT NULL
    AND json_extract(e.value, '$.parsed_data.content_block.type') = 'tool_use'
),
tool_results AS (
  SELECT
    c.start_timestamp_ms AS result_ms,
    json_extract(content.value, '$.tool_use_id') AS tool_call_id,
    content.value AS output_json
  FROM llm_calls c,
       json_each(c.request_body_json, '$.messages') AS msg,
       json_each(
         CASE
           WHEN json_type(msg.value, '$.content') = 'array'
           THEN json_extract(msg.value, '$.content')
           ELSE '[]'
         END
       ) AS content
  WHERE c.request_body_json IS NOT NULL
    AND json_extract(content.value, '$.type') = 'tool_result'
),
matched AS (
  SELECT
    u.*,
    (
      SELECT MIN(r.result_ms)
      FROM tool_results r
      WHERE r.tool_call_id = u.tool_call_id
        AND r.result_ms >= u.start_ms
    ) AS end_ms,
    (
      SELECT r.output_json
      FROM tool_results r
      WHERE r.tool_call_id = u.tool_call_id
        AND r.result_ms >= u.start_ms
      ORDER BY r.result_ms
      LIMIT 1
    ) AS output_json
  FROM tool_uses u
)
SELECT
  'claude-tool-' || llm_call_id || '-' || COALESCE(tool_call_id, event_key),
  'claude-code-pid-' || pid,
  'claude-conv-' || llm_call_id,
  start_ms,
  tool_name,
  tool_call_id,
  start_ms,
  end_ms,
  CASE WHEN end_ms IS NOT NULL THEN end_ms - start_ms ELSE NULL END,
  CASE WHEN end_ms IS NOT NULL THEN 'completed' ELSE 'observed' END,
  input_json,
  output_json,
  pid,
  response_event_id,
  'claude-code',
  0.85
FROM matched;
