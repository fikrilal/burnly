import { useState } from "react";
import { AlertCircle } from "lucide-react";
import { useCalendar, useDayDetail } from "./use-calendar";
import { CalendarHeatmap } from "./components/CalendarHeatmap";
import { DayDetailCard } from "./components/DayDetailCard";

function useDateRange() {
  const [dateRange] = useState(() => {
    const end = new Date();
    const start = new Date(Date.now() - 365 * 24 * 60 * 60 * 1000); // 1 year range for calendar
    return {
      startDate: start.toISOString().substring(0, 10),
      endDate: end.toISOString().substring(0, 10),
    };
  });
  return dateRange;
}

export function CalendarView() {
  const dateRange = useDateRange();
  const [selectedDate, setSelectedDate] = useState<string | null>(null);

  const [reportingTimezone] = useState(
    () => Intl.DateTimeFormat().resolvedOptions().timeZone,
  );

  const {
    data: calendarData,
    isPending: isCalendarPending,
    isError: isCalendarError,
    error: calendarError,
    refetch: refetchCalendar,
  } = useCalendar({
    startDate: dateRange.startDate,
    endDate: dateRange.endDate,
    reportingTimezone,
    metric: "tokens",
  });

  const {
    data: dayDetail,
    isPending: isDayDetailPending,
    isError: isDayDetailError,
    error: dayDetailError,
  } = useDayDetail(selectedDate, reportingTimezone);

  return (
    <div className="flex flex-col gap-8">
      <div>
        <h2 className="text-2xl font-semibold tracking-tight text-white mb-6">
          Activity Calendar
        </h2>

        {isCalendarPending ? (
          <div className="flex h-32 items-center justify-center rounded-lg border border-zinc-800 bg-zinc-900/50">
            <p className="text-zinc-500">Loading calendar...</p>
          </div>
        ) : isCalendarError ? (
          <div className="flex flex-col items-center justify-center rounded-lg border border-red-900/50 bg-red-950/20 p-8 text-center">
            <AlertCircle className="mb-4 h-8 w-8 text-red-500" aria-hidden />
            <p className="font-medium text-red-400">Failed to load calendar</p>
            <p className="mt-2 text-sm text-red-500/70 max-w-md">
              {String(calendarError)}
            </p>
            <button
              type="button"
              onClick={() => void refetchCalendar()}
              className="mt-6 inline-flex items-center justify-center rounded-md bg-red-900/50 px-4 py-2 text-sm font-medium text-red-200 transition-colors hover:bg-red-900/70"
            >
              Retry
            </button>
          </div>
        ) : (
          <div className="rounded-lg border border-zinc-800 bg-zinc-900/50 p-6 overflow-x-auto">
            <CalendarHeatmap
              cells={calendarData.cells}
              selectedDate={selectedDate}
              onSelectDate={setSelectedDate}
            />
          </div>
        )}
      </div>

      {selectedDate && (
        <div>
          {isDayDetailPending ? (
            <div className="flex h-48 items-center justify-center rounded-lg border border-zinc-800 bg-zinc-900/50">
              <p className="text-zinc-500">Loading day detail...</p>
            </div>
          ) : isDayDetailError ? (
            <div className="flex flex-col items-center justify-center rounded-lg border border-red-900/50 bg-red-950/20 p-8 text-center">
              <AlertCircle className="mb-4 h-8 w-8 text-red-500" aria-hidden />
              <p className="font-medium text-red-400">
                Failed to load day detail
              </p>
              <p className="mt-2 text-sm text-red-500/70 max-w-md">
                {String(dayDetailError)}
              </p>
            </div>
          ) : (
            <DayDetailCard detail={dayDetail} />
          )}
        </div>
      )}
    </div>
  );
}
