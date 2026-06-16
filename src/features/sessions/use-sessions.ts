import {
  useInfiniteQuery,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { useEffect } from "react";

import { getSessions, getSessionDetail } from "../../ipc/client";
import { EVENT_NAMES, subscribeToEvent } from "../../ipc/events";
import type { SessionListRequest } from "../../ipc/generated/contracts";

export function useSessions(
  request: Omit<SessionListRequest, "afterActivityMs">,
) {
  const queryClient = useQueryClient();
  const queryKey = ["usage", "sessions", request.sourceId, request.limit];

  const query = useInfiniteQuery({
    queryKey,
    queryFn: async ({ pageParam }) => {
      const response = await getSessions({
        ...request,
        afterActivityMs: pageParam,
      });
      return response.data;
    },
    initialPageParam: null as number | null,
    getNextPageParam: (lastPage) => lastPage.nextCursor,
  });

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let active = true;

    const setup = async () => {
      const fn = await subscribeToEvent(EVENT_NAMES.dataInvalidated, () => {
        void queryClient.invalidateQueries({ queryKey: ["usage", "sessions"] });
      });

      if (active) {
        unlisten = fn;
      } else {
        fn();
      }
    };

    void setup();

    return () => {
      active = false;
      if (unlisten) {
        unlisten();
      }
    };
  }, [queryClient]);

  return query;
}

export function useSessionDetail(sessionId: number | null) {
  const queryClient = useQueryClient();
  const queryKey = ["usage", "session-detail", sessionId];

  const query = useQuery({
    queryKey,
    queryFn: async () => {
      if (sessionId === null) return null;
      const response = await getSessionDetail({ sessionId });
      return response.data;
    },
    enabled: sessionId !== null,
  });

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let active = true;

    const setup = async () => {
      const fn = await subscribeToEvent(EVENT_NAMES.dataInvalidated, () => {
        void queryClient.invalidateQueries({
          queryKey: ["usage", "session-detail"],
        });
      });

      if (active) {
        unlisten = fn;
      } else {
        fn();
      }
    };

    void setup();

    return () => {
      active = false;
      if (unlisten) {
        unlisten();
      }
    };
  }, [queryClient]);

  return query;
}
