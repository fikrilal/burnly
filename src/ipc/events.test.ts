import { describe, expect, it, vi } from "vitest";

import {
  EVENT_NAMES,
  parseEventPayload,
  subscribeToEvent,
  type EventListenerTransport,
} from "./events";

describe("IPC event subscriptions", () => {
  it("subscribes with generated event names and returns cleanup", async () => {
    const cleanup = vi.fn();
    const callback = vi.fn();
    const listen: EventListenerTransport = (event, received) => {
      expect(event).toBe(EVENT_NAMES.dataInvalidated);
      received({ scope: "usage" });
      return Promise.resolve(cleanup);
    };

    const unlisten = await subscribeToEvent(
      EVENT_NAMES.dataInvalidated,
      callback,
      listen,
    );
    unlisten();

    expect(callback).toHaveBeenCalledWith({ scope: "usage" });
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

describe("IPC event payloads", () => {
  it("rejects unit/null payloads (must be typed objects)", () => {
    expect(() =>
      parseEventPayload(EVENT_NAMES.accountSessionChanged, null),
    ).toThrow();
    expect(() =>
      parseEventPayload(EVENT_NAMES.accountSessionChanged, {}),
    ).toThrow();
  });

  it("parses account session change reasons", () => {
    expect(
      parseEventPayload(EVENT_NAMES.accountSessionChanged, {
        reason: "login_completed",
      }),
    ).toEqual({ reason: "login_completed" });
  });

  it("parses settings-changed revision", () => {
    expect(
      parseEventPayload(EVENT_NAMES.settingsChanged, { revision: 3 }),
    ).toEqual({ revision: 3 });
  });
});

describe("IPC event subscriptions - behavior and cleanup", () => {
  it("keeps duplicate and missed notifications as harmless invalidations", async () => {
    const callback = vi.fn();
    const listen: EventListenerTransport = (_event, received) => {
      received({ scope: "usage" });
      received({ scope: "usage" });
      return Promise.resolve(() => undefined);
    };

    await subscribeToEvent(EVENT_NAMES.dataInvalidated, callback, listen);

    expect(callback).toHaveBeenCalledTimes(2);
    expect(callback).toHaveBeenNthCalledWith(1, { scope: "usage" });
    expect(callback).toHaveBeenNthCalledWith(2, { scope: "usage" });
  });

  it("installs one listener and delegates cleanup to the transport once", async () => {
    const cleanup = vi.fn();
    const listen = vi.fn<EventListenerTransport>(() =>
      Promise.resolve(cleanup),
    );

    const unlisten = await subscribeToEvent(
      EVENT_NAMES.settingsChanged,
      vi.fn(),
      listen,
    );
    unlisten();

    expect(listen).toHaveBeenCalledOnce();
    expect(cleanup).toHaveBeenCalledOnce();
  });
});
