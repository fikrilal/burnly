import { useQuery, keepPreviousData } from "@tanstack/react-query";
import { getActivityCalendar, getDayDetail } from "../../ipc/client";

interface UseCalendarOptions {
  startDate: string;
  endDate: string;
  reportingTimezone: string;
}

export function useCalendar(options: UseCalendarOptions) {
  const { startDate, endDate, reportingTimezone } = options;

  const queryKey = [
    "usage",
    "calendar",
    { startDate, endDate, reportingTimezone },
  ];

  const query = useQuery({
    queryKey,
    queryFn: async () => {
      const response = await getActivityCalendar({
        startDate,
        endDate,
        reportingTimezone,
      });
      return response.data;
    },
    placeholderData: keepPreviousData,
  });

  return query;
}

export function useDayDetail(date: string | null) {
  const queryKey = ["usage", "day_detail", { date }];

  const query = useQuery({
    queryKey,
    queryFn: async () => {
      if (!date) throw new Error("No date provided");
      const response = await getDayDetail({
        date,
      });
      return response.data;
    },
    enabled: !!date,
  });

  return query;
}
