export class IpcUnavailableError extends Error {
  constructor() {
    super("Burnly IPC is not configured yet.");
    this.name = "IpcUnavailableError";
  }
}
