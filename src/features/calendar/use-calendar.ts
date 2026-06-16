import { useQuery, keepPreviousData } from "@tanstack/react-query";
import { getActivityCalendar, getDayDetail } from "../../ipc/client";

interface UseCalendarOptions {
  startDate: string;
  endDate: string;
  reportingTimezone: string;
  metric?: "tokens" | "cost";
  source?: string | null;
  project?: string | null;
}

export function useCalendar(options: UseCalendarOptions) {
  const {
    startDate,
    endDate,
    reportingTimezone,
    metric = "tokens",
    source = null,
    project = null,
  } = options;

  const queryKey = [
    "usage",
    "calendar",
    { startDate, endDate, metric, reportingTimezone, source, project },
  ];

  const query = useQuery({
    queryKey,
    queryFn: async () => {
      const response = await getActivityCalendar({
        startDate,
        endDate,
        metric,
        timezone: reportingTimezone,
        source: source ?? null,
        project: project ?? null,
      });
      return response.data;
    },
    placeholderData: keepPreviousData,
  });

  return query;
}

export function useDayDetail(
  date: string | null,
  timezone: string,
  source: string | null = null,
  project: string | null = null,
) {
  const queryKey = ["usage", "day_detail", { date, timezone, source, project }];

  const query = useQuery({
    queryKey,
    queryFn: async () => {
      if (!date) throw new Error("No date provided");
      const response = await getDayDetail({
        date,
        timezone,
        source: source ?? null,
        project: project ?? null,
      });
      return response.data;
    },
    enabled: !!date,
  });

  return query;
}
