const parseJson = (value: string): unknown | undefined => {
  try {
    return JSON.parse(value);
  } catch {
    return undefined;
  }
};

export const parseStreamingJson = (value: string | undefined): unknown => {
  if (!value) return {};
  const parsed = parseJson(value);
  return parsed === undefined ? {} : parsed;
};

export const parsePossiblyCompleteJson = parseJson;

export const STREAMING_JSON_PARSE_MIN_GROWTH = 256;

/**
 * Throttles complete-JSON parse attempts for a growing streaming buffer.
 *
 * A non-null result means an attempt occurred, even when `value` is undefined.
 * Callers must persist `attemptedLength` so invalid partial JSON is throttled too.
 */
export const parsePossiblyCompleteJsonThrottled = (
  value: string,
  lastAttemptedLength: number
): Readonly<{ value: unknown | undefined; attemptedLength: number }> | null => {
  const attemptedLength = value.length;
  if (
    attemptedLength === 0 ||
    (lastAttemptedLength > 0 && attemptedLength - lastAttemptedLength < STREAMING_JSON_PARSE_MIN_GROWTH)
  ) {
    return null;
  }
  return { value: parsePossiblyCompleteJson(value), attemptedLength };
};
