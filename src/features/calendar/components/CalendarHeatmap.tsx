import { useMemo } from "react";
import type { ActivityCalendarDayResponse } from "../../../ipc/generated/contracts";

interface CalendarHeatmapProps {
  days: ActivityCalendarDayResponse[];
  startDate: string;
  endDate: string;
  onSelectDate?: (date: string) => void;
  selectedDate?: string | null;
}

export function CalendarHeatmap({
  days,
  startDate,
  endDate,
  onSelectDate,
  selectedDate,
}: CalendarHeatmapProps) {
  const allDays = useMemo(() => {
    const start = new Date(startDate);
    const end = new Date(endDate);
    const daysArray: {
      date: string;
      data: ActivityCalendarDayResponse | null;
    }[] = [];

    const daysMap = new Map(days.map((d) => [d.date, d]));

    const current = new Date(start);
    while (current <= end) {
      const dateStr = current.toISOString().substring(0, 10);
      daysArray.push({
        date: dateStr,
        data: daysMap.get(dateStr) ?? null,
      });
      current.setUTCDate(current.getUTCDate() + 1);
    }
    return daysArray;
  }, [startDate, endDate, days]);

  // Calculate offset for the first day
  const startDayOfWeek = new Date(startDate).getUTCDay(); // 0 is Sunday

  const maxTokens = Math.max(
    ...days.map((d) => Number.parseInt(d.totalTokens, 10)),
    0,
  );

  const getIntensity = (totalTokens: string) => {
    const tokens = Number.parseInt(totalTokens, 10);
    if (tokens === 0) return 0;
    if (maxTokens === 0) return 0;
    const ratio = tokens / maxTokens;
    if (ratio <= 0.25) return 1;
    if (ratio <= 0.5) return 2;
    if (ratio <= 0.75) return 3;
    return 4;
  };

  // Group cells into weeks
  const weeks: ({
    date: string;
    data: ActivityCalendarDayResponse | null;
  } | null)[][] = [];
  let currentWeek: ({
    date: string;
    data: ActivityCalendarDayResponse | null;
  } | null)[] = Array.from({ length: startDayOfWeek }, () => null);

  for (const day of allDays) {
    currentWeek.push(day);
    if (currentWeek.length === 7) {
      weeks.push(currentWeek);
      currentWeek = [];
    }
  }

  if (currentWeek.length > 0) {
    while (currentWeek.length < 7) {
      currentWeek.push(null);
    }
    weeks.push(currentWeek);
  }

  const getIntensityClass = (intensity: number) => {
    switch (intensity) {
      case 1:
        return "bg-cyan-900/40 border-cyan-800/50";
      case 2:
        return "bg-cyan-700/60 border-cyan-600/50";
      case 3:
        return "bg-cyan-500/80 border-cyan-400/50";
      case 4:
        return "bg-cyan-400 border-cyan-300";
      default:
        return "bg-zinc-800/50 border-zinc-800"; // 0
    }
  };

  return (
    <div className="flex flex-col gap-2 overflow-x-auto pb-4">
      <div className="flex gap-1" style={{ width: "max-content" }}>
        {weeks.map((week, weekIndex) => (
          <div key={weekIndex} className="flex flex-col gap-1">
            {week.map((day, dayIndex) => {
              if (!day) {
                return (
                  <div
                    key={`empty-${weekIndex}-${dayIndex}`}
                    className="h-3 w-3 rounded-sm bg-transparent"
                  />
                );
              }

              const isSelected = selectedDate === day.date;
              const intensity = day.data
                ? getIntensity(day.data.totalTokens)
                : 0;
              const tokensDisplay = day.data ? day.data.totalTokens : "0";

              return (
                <button
                  key={day.date}
                  type="button"
                  title={`${day.date}: ${tokensDisplay} tokens`}
                  onClick={() => onSelectDate?.(day.date)}
                  className={`h-3 w-3 rounded-sm border transition-colors hover:border-zinc-400 ${
                    isSelected
                      ? "ring-1 ring-white ring-offset-1 ring-offset-zinc-950"
                      : ""
                  } ${getIntensityClass(intensity)}`}
                  aria-label={`${day.date}: ${tokensDisplay} tokens`}
                  aria-pressed={isSelected}
                />
              );
            })}
          </div>
        ))}
      </div>
      <div className="flex items-center justify-end gap-2 text-xs text-zinc-500">
        <span>Less</span>
        <div className="flex gap-1">
          <div
            className={`h-3 w-3 rounded-sm border ${getIntensityClass(0)}`}
          />
          <div
            className={`h-3 w-3 rounded-sm border ${getIntensityClass(1)}`}
          />
          <div
            className={`h-3 w-3 rounded-sm border ${getIntensityClass(2)}`}
          />
          <div
            className={`h-3 w-3 rounded-sm border ${getIntensityClass(3)}`}
          />
          <div
            className={`h-3 w-3 rounded-sm border ${getIntensityClass(4)}`}
          />
        </div>
        <span>More</span>
      </div>
    </div>
  );
}
