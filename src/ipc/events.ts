import { listen as tauriListen, type UnlistenFn } from "@tauri-apps/api/event";
import { z } from "zod";

import {
  EVENT_NAMES,
  type EventName,
  type EventPayloads,
} from "./generated/contracts";

type EventCallback<E extends EventName> = (payload: EventPayloads[E]) => void;

const refreshProgressSchema = z.object({
  status: z.string(),
});

const dataInvalidatedSchema = z.object({
  scope: z.string(),
});

const settingsChangedSchema = z.object({
  revision: z.number().int(),
});

const accountSessionChangedSchema = z.object({
  reason: z.enum([
    "login_started",
    "login_completed",
    "login_cancelled",
    "login_failed",
    "logged_out",
  ]),
});

const platformStateChangedSchema = z.object({
  kind: z.string(),
});

const updateProgressSchema = z.object({
  status: z.string(),
});

/**
 * Per-event Zod schemas. Must stay aligned with generated `EventPayloads`
 * and Rust `src-tauri/src/ipc/events.rs`.
 */
const eventSchemas: {
  [K in EventName]: z.ZodType<EventPayloads[K]>;
} = {
  [EVENT_NAMES.refreshProgress]: refreshProgressSchema,
  [EVENT_NAMES.dataInvalidated]: dataInvalidatedSchema,
  [EVENT_NAMES.settingsChanged]: settingsChangedSchema,
  [EVENT_NAMES.accountSessionChanged]: accountSessionChangedSchema,
  [EVENT_NAMES.platformStateChanged]: platformStateChangedSchema,
  [EVENT_NAMES.updateProgress]: updateProgressSchema,
};

export function parseEventPayload<E extends EventName>(
  event: E,
  payload: unknown,
): EventPayloads[E] {
  return eventSchemas[event].parse(payload);
}

export async function subscribeToEvent<E extends EventName>(
  event: E,
  callback: EventCallback<E>,
  listen: EventListenerTransport = eventListenerTransport,
): Promise<UnlistenFn> {
  return listen(event, (payload) => {
    callback(parseEventPayload(event, payload));
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
