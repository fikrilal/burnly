import { listen as tauriListen, type UnlistenFn } from "@tauri-apps/api/event";
import { z } from "zod";

import {
  EVENT_NAMES,
  type EventName,
  type UnknownEventPayload,
} from "./generated/contracts";

type EventCallback = (payload: UnknownEventPayload) => void;

const eventPayloadSchema = z.record(z.string(), z.unknown());

export async function subscribeToEvent(
  event: EventName,
  callback: EventCallback,
  listen: EventListenerTransport = eventListenerTransport,
): Promise<UnlistenFn> {
  return listen(event, (payload) => {
    callback(eventPayloadSchema.parse(payload));
  });
}

export const eventListenerTransport: EventListenerTransport = (
  event,
  callback,
) =>
  tauriListen(event, ({ payload }) => {
    callback(payload);
  });

export type EventListenerTransport = (
  event: EventName,
  callback: (payload: unknown) => void,
) => Promise<UnlistenFn>;

export { EVENT_NAMES };
