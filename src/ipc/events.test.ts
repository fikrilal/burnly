import { describe, expect, it, vi } from "vitest";

import {
  EVENT_NAMES,
  subscribeToEvent,
  type EventListenerTransport,
} from "./events";

describe("IPC event subscriptions", () => {
  it("subscribes with generated event names and returns cleanup", async () => {
    const cleanup = vi.fn();
    const callback = vi.fn();
    const listen: EventListenerTransport = (event, received) => {
      expect(event).toBe(EVENT_NAMES.dataInvalidated);
      received({ reason: "settings_changed" });
      return Promise.resolve(cleanup);
    };

    const unlisten = await subscribeToEvent(
      EVENT_NAMES.dataInvalidated,
      callback,
      listen,
    );
    unlisten();

    expect(callback).toHaveBeenCalledWith({ reason: "settings_changed" });
    expect(cleanup).toHaveBeenCalledOnce();
  });

  it("rejects malformed event payloads at the boundary", async () => {
    const listen: EventListenerTransport = (_event, received) => {
      received("invalid payload");
      return Promise.resolve(() => undefined);
    };

    await expect(
      subscribeToEvent(EVENT_NAMES.refreshProgress, vi.fn(), listen),
    ).rejects.toThrow();
  });
});
