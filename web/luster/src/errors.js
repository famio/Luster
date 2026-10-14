/** Why a badge could not be made: the document, its source, or the engine. */
export class LusterError extends Error {
  /**
   * @param {string} kind
   * @param {string} message
   */
  constructor(kind, message) {
    super(message);
    this.name = 'LusterError';
    this.kind = kind;
  }
}

/** What a worker's failure is to the page: an abort stays one. */
export function engineError(e) {
  if (e instanceof LusterError) return e;
  if (e?.name === 'AbortError') return e;
  return new LusterError(e?.kind ?? 'engineFailure', e?.message ?? String(e));
}
