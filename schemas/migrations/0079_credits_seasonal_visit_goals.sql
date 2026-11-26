-- Existing databases accept the three room-driven goals alongside take-photo.
ALTER TABLE IF EXISTS credits.season_goals DROP CONSTRAINT IF EXISTS season_goals_goal_id_check;
ALTER TABLE IF EXISTS credits.season_goals ADD CONSTRAINT season_goals_goal_id_check
    CHECK (goal_id IN ('take-photo', 'log-in', 'visit-location', 'visit-event-and-stay'));
