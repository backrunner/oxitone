export class WebRuntimeError extends Error {
  constructor(readonly code: string, message: string) { super(message); this.name = "WebRuntimeError"; }
}
