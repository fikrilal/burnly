import {
  type PointerEvent as ReactPointerEvent,
  type ReactNode,
  type RefObject,
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from "react";

interface ScrollMetrics {
  visible: boolean;
  thumbHeight: number;
  thumbTop: number;
}

const MIN_SCROLL_THUMB_HEIGHT = 32;
const SCROLLBAR_IDLE_HIDE_MS = 1_600;

export function TrayScrollArea({
  label,
  resetKey,
  children,
}: {
  label: string;
  resetKey: string;
  children: ReactNode;
}) {
  const viewportRef = useRef<HTMLElement | null>(null);
  const { metrics, updateMetrics } = useTrayScrollMetrics(viewportRef);
  const { isActive, activate } = useTransientScrollbar(metrics.visible);
  const startDragging = useTrayScrollDrag(
    viewportRef,
    metrics.thumbHeight,
    activate,
  );

  useLayoutEffect(() => {
    updateMetrics();
  }, [children, label, updateMetrics]);

  useLayoutEffect(() => {
    const viewport = viewportRef.current;
    if (!viewport) return;
    viewport.scrollTop = 0;
    updateMetrics();
  }, [resetKey, updateMetrics]);

  const onScroll = () => {
    updateMetrics();
    activate();
  };

  return (
    <div className="tray-scroll-shell relative min-h-0 flex-1">
      <section
        ref={viewportRef}
        aria-label={label}
        className="tray-scroll-area h-full overflow-y-auto pr-5"
        onScroll={onScroll}
      >
        {children}
      </section>
      {metrics.visible ? (
        <div
          className="tray-scroll-track"
          data-active={isActive ? "true" : "false"}
          aria-hidden="true"
        >
          <div
            className="tray-scroll-thumb"
            onPointerDown={startDragging}
            style={{
              height: `${metrics.thumbHeight}px`,
              transform: `translateY(${metrics.thumbTop}px)`,
            }}
          />
        </div>
      ) : null}
    </div>
  );
}

function useTrayScrollMetrics(viewportRef: RefObject<HTMLElement | null>) {
  const [metrics, setMetrics] = useState<ScrollMetrics>({
    visible: false,
    thumbHeight: MIN_SCROLL_THUMB_HEIGHT,
    thumbTop: 0,
  });

  const updateMetrics = useCallback(() => {
    const viewport = viewportRef.current;
    if (!viewport) return;
    setMetrics(scrollMetrics(viewport));
  }, [viewportRef]);

  useEffect(() => {
    const viewport = viewportRef.current;
    if (!viewport) return;

    const observer =
      typeof ResizeObserver === "undefined"
        ? null
        : new ResizeObserver(updateMetrics);
    observer?.observe(viewport);
    window.addEventListener("resize", updateMetrics);

    return () => {
      observer?.disconnect();
      window.removeEventListener("resize", updateMetrics);
    };
  }, [updateMetrics, viewportRef]);

  return { metrics, updateMetrics };
}

function useTransientScrollbar(isScrollable: boolean) {
  const idleTimerRef = useRef<number | null>(null);
  const [isActive, setIsActive] = useState(false);

  const clearIdleTimer = useCallback(() => {
    if (idleTimerRef.current === null) return;
    window.clearTimeout(idleTimerRef.current);
    idleTimerRef.current = null;
  }, []);

  const activate = useCallback(() => {
    if (!isScrollable) return;
    clearIdleTimer();
    setIsActive(true);
    idleTimerRef.current = window.setTimeout(() => {
      setIsActive(false);
      idleTimerRef.current = null;
    }, SCROLLBAR_IDLE_HIDE_MS);
  }, [clearIdleTimer, isScrollable]);

  useEffect(() => clearIdleTimer, [clearIdleTimer]);

  return { isActive, activate };
}

function useTrayScrollDrag(
  viewportRef: RefObject<HTMLElement | null>,
  thumbHeight: number,
  activate: () => void,
) {
  const dragStateRef = useRef<{
    pointerOffsetY: number;
    maxScrollTop: number;
    maxThumbTop: number;
  } | null>(null);

  useEffect(() => {
    const onPointerMove = (event: PointerEvent) => {
      updateDraggedScrollTop(viewportRef.current, dragStateRef.current, event);
      if (dragStateRef.current) activate();
    };
    const onPointerUp = () => {
      dragStateRef.current = null;
    };

    window.addEventListener("pointermove", onPointerMove);
    window.addEventListener("pointerup", onPointerUp);
    return () => {
      window.removeEventListener("pointermove", onPointerMove);
      window.removeEventListener("pointerup", onPointerUp);
    };
  }, [activate, viewportRef]);

  return (event: ReactPointerEvent<HTMLDivElement>) => {
    const viewport = viewportRef.current;
    if (!viewport) return;

    const maxScrollTop = viewport.scrollHeight - viewport.clientHeight;
    const maxThumbTop = viewport.clientHeight - thumbHeight;
    if (maxScrollTop <= 0 || maxThumbTop <= 0) return;

    dragStateRef.current = {
      pointerOffsetY:
        event.clientY - event.currentTarget.getBoundingClientRect().top,
      maxScrollTop,
      maxThumbTop,
    };
    activate();
    event.preventDefault();
  };
}

function scrollMetrics(viewport: HTMLElement): ScrollMetrics {
  const { clientHeight, scrollHeight, scrollTop } = viewport;
  const maxScrollTop = scrollHeight - clientHeight;
  if (clientHeight <= 0 || maxScrollTop <= 0) {
    return {
      visible: false,
      thumbHeight: MIN_SCROLL_THUMB_HEIGHT,
      thumbTop: 0,
    };
  }

  const thumbHeight = Math.max(
    MIN_SCROLL_THUMB_HEIGHT,
    (clientHeight / scrollHeight) * clientHeight,
  );
  return {
    visible: true,
    thumbHeight,
    thumbTop: (scrollTop / maxScrollTop) * (clientHeight - thumbHeight),
  };
}

function updateDraggedScrollTop(
  viewport: HTMLElement | null,
  drag: {
    pointerOffsetY: number;
    maxScrollTop: number;
    maxThumbTop: number;
  } | null,
  event: PointerEvent,
) {
  if (!viewport || !drag || drag.maxThumbTop <= 0) return;

  const trackTop = viewport.getBoundingClientRect().top;
  const requestedThumbTop = event.clientY - trackTop - drag.pointerOffsetY;
  const boundedThumbTop = Math.max(
    0,
    Math.min(drag.maxThumbTop, requestedThumbTop),
  );
  viewport.scrollTop = (boundedThumbTop / drag.maxThumbTop) * drag.maxScrollTop;
}
