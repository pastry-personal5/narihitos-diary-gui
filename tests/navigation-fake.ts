import type { NavigationBridge } from "../src/application/navigation/contracts";
export function navigationFake(): NavigationBridge {
  return {
    getCalendar: async (anchor, action = "month") => {
      const now = new Date();
      const today = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}-${String(now.getDate()).padStart(2, "0")}`;
      let selected = anchor ?? today;
      if (action !== "month" && action !== "today") {
        const [y, m, d] = selected.split("-").map(Number);
        const value = new Date(y, m - 1, d);
        if (action === "next_day") value.setDate(value.getDate() + 1);
        if (action === "previous_day") value.setDate(value.getDate() - 1);
        if (action === "next_week") value.setDate(value.getDate() + 7);
        if (action === "previous_week") value.setDate(value.getDate() - 7);
        selected = `${value.getFullYear()}-${String(value.getMonth() + 1).padStart(2, "0")}-${String(value.getDate()).padStart(2, "0")}`;
      }
      const [year, month] = selected.split("-").map(Number);
      const count = new Date(year, month, 0).getDate();
      return {
        anchor: selected,
        today,
        year,
        month,
        days: Array.from({ length: count }, (_, i) => ({
          date: `${selected.slice(0, 7)}-${String(i + 1).padStart(2, "0")}`,
          day: i + 1,
          weekday: new Date(year, month - 1, i + 1).getDay(),
        })),
        previous_month: null,
        next_month: null,
      };
    },
    importDate: async () => ({
      import_id: "import",
      job: { id: "import", sequence: 1, state: "succeeded" },
    }),
    listSources: async () => ({
      import_id: "import",
      date: new Date().toLocaleDateString("sv"),
      zone: "UTC",
      timezone_database: "test",
      config_generation: 1,
      completed_at: 1,
      items: [],
      next_cursor: null,
      incomplete: false,
      groups: [],
      reimport_required: false,
    }),
    resolveSourceTime: async () => ({ resolved: true }),
    readSource: async () => {
      throw new Error("unused");
    },
    saveSource: async () => {
      throw new Error("unused");
    },
    getJob: async () => ({ id: "import", sequence: 1, state: "succeeded" }),
    cancelJob: async () => ({ id: "import", sequence: 2, state: "cancelled" }),
  };
}
