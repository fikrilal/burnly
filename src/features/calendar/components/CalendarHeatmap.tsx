import { useMemo } from "react";
import type { ActivityCalendarCellResponse } from "../../../ipc/generated/contracts";

interface CalendarHeatmapProps {
  cells: ActivityCalendarCellResponse[];
  onSelectDate?: (date: string) => void;
  selectedDate?: string | null;
}

export function CalendarHeatmap({
  cells,
  onSelectDate,
  selectedDate,
}: CalendarHeatmapProps) {
  // Assuming the cells come in date order, but let's be sure.
  const sortedCells = useMemo(
    () => [...cells].sort((a, b) => a.date.localeCompare(b.date)),
    [cells],
  );

  if (sortedCells.length === 0) {
    return (
      <div className="flex h-32 items-center justify-center text-sm text-zinc-500">
        No calendar data available.
      </div>
    );
  }

  // Create a grid of 7 rows (Sunday - Saturday)
  // Calculate offset for the first day
  const firstDateStr = sortedCells[0]?.date;
  if (!firstDateStr) return null;
  const firstDate = new Date(firstDateStr);
  const startDayOfWeek = firstDate.getUTCDay(); // 0 is Sunday

  // Group cells into weeks
  const weeks: (ActivityCalendarCellResponse | null)[][] = [];
  let currentWeek: (ActivityCalendarCellResponse | null)[] = Array.from(
    { length: startDayOfWeek },
    () => null,
  );

  for (const cell of sortedCells) {
    currentWeek.push(cell);
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
            {week.map((cell, dayIndex) => {
              if (!cell) {
                return (
                  <div
                    key={`empty-${weekIndex}-${dayIndex}`}
                    className="h-3 w-3 rounded-sm bg-transparent"
                  />
                );
              }

              const isSelected = selectedDate === cell.date;

              return (
                <button
                  key={cell.date}
                  type="button"
                  title={`${cell.date}: ${cell.value} tokens`}
                  onClick={() => onSelectDate?.(cell.date)}
                  className={`h-3 w-3 rounded-sm border transition-colors hover:border-zinc-400 ${
                    isSelected
                      ? "ring-1 ring-white ring-offset-1 ring-offset-zinc-950"
                      : ""
                  } ${getIntensityClass(cell.intensity)}`}
                  aria-label={`${cell.date}: ${cell.value} tokens`}
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
