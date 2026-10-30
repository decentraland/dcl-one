export type ClientEvents = {
  cl_explore_opened: {
    place_count: number;
  };
  cl_open_jumped_in: {
    place_id: string;
  };
  cl_open_screen_shown: Record<string, never>;
};
