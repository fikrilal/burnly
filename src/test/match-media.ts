export interface MatchMediaController {
  setMatches: (matches: boolean) => void;
}

/**
 * Installs a controllable `window.matchMedia` stub for tests (jsdom does not
 * implement it). Returns a controller whose `setMatches` dispatches `change`
 * events to registered listeners.
 */
export function installMatchMedia(
  initialMatches: boolean,
): MatchMediaController {
  const listeners = new Set<EventListenerOrEventListenerObject>();
  let matches = initialMatches;

  window.matchMedia = (query: string): MediaQueryList => ({
    matches,
    media: query,
    onchange: null,
    addEventListener: (
      _type: string,
      listener: EventListenerOrEventListenerObject,
    ) => {
      listeners.add(listener);
    },
    removeEventListener: (
      _type: string,
      listener: EventListenerOrEventListenerObject,
    ) => {
      listeners.delete(listener);
    },
    addListener: () => undefined,
    removeListener: () => undefined,
    dispatchEvent: () => true,
  });

  return {
    setMatches(next: boolean) {
      matches = next;
      const event = Object.assign(new Event("change"), { matches: next });
      for (const listener of listeners) {
        if (typeof listener === "function") {
          listener(event);
        } else {
          listener.handleEvent(event);
        }
      }
    },
  };
}
