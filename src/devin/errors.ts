export class DevinAuthError extends Error {
  readonly name = "DevinAuthError";

  constructor(
    message: string,
    readonly cause?: unknown
  ) {
    super(message);
  }
}

export class DevinApiError extends Error {
  readonly name = "DevinApiError";

  constructor(
    message: string,
    readonly status: number,
    readonly body?: string
  ) {
    super(message);
  }
}

export class DevinProtocolError extends Error {
  readonly name = "DevinProtocolError";

  constructor(message: string, readonly cause?: unknown) {
    super(message);
  }
}
