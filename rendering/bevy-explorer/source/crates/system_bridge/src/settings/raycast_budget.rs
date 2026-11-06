use bevy::{platform::collections::HashSet, prelude::*};
use bevy_console::ConsoleCommand;
use common::structs::AppConfig;
use console::DoAddConsoleCommand;

use super::{AppSetting, IntAppSetting, Settings};

#[derive(Debug, PartialEq, Eq)]
pub struct RaycastBudgetSetting(i32);

impl IntAppSetting for RaycastBudgetSetting {
    fn from_int(value: i32) -> Self {
        Self(value)
    }

    fn value(&self) -> i32 {
        self.0
    }

    fn min() -> i32 {
        0
    }

    fn max() -> i32 {
        10
    }
}

impl AppSetting for RaycastBudgetSetting {
    type Param = ();

    fn title() -> String {
        "Raycast budget".to_owned()
    }

    fn description(&self) -> String {
        "Raycast budget\n\nMilliseconds per frame spent on scene raycasts. Raycasts that do not fit run on the next frame; at least one runs every frame. 0 runs one raycast per frame.".to_string()
    }

    fn save(&self, config: &mut AppConfig) {
        config.raycast_budget_ms = self.0 as u32;
    }

    fn load(config: &AppConfig) -> Self {
        Self(config.raycast_budget_ms as i32)
    }

    fn category() -> super::SettingCategory {
        super::SettingCategory::Performance
    }

    // read by `run_raycasts` from the config every frame
    fn apply(&self, _: (), _: Commands, _: &HashSet<Entity>) {}
}

pub(super) fn register(app: &mut App) {
    app.add_console_command::<RaycastBudgetCommand, _>(raycast_budget);
}

/// Show the milliseconds per frame spent on scene raycasts, or set them.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/raycast_budget")]
struct RaycastBudgetCommand {
    /// New budget in milliseconds
    ms: Option<u32>,
}

fn parse_budget_ms(ms: u32) -> Result<u32, String> {
    if ms > RaycastBudgetSetting::max() as u32 {
        return Err(format!(
            "raycast budget must be between {} and {} ms",
            RaycastBudgetSetting::min(),
            RaycastBudgetSetting::max()
        ));
    }
    Ok(ms)
}

fn raycast_budget(
    mut input: ConsoleCommand<RaycastBudgetCommand>,
    mut settings: ResMut<Settings>,
    mut config: ResMut<AppConfig>,
) {
    if let Some(Ok(command)) = input.take() {
        if let Some(ms) = command.ms {
            let result = parse_budget_ms(ms).and_then(|ms| {
                settings
                    .set_value(&mut config, &RaycastBudgetSetting::title(), ms as f32)
                    .map_err(|e| e.to_string())
            });
            if let Err(e) = result {
                input.reply_failed(e);
                return;
            }
        }
        input.reply_ok(format!("raycast budget: {} ms", config.raycast_budget_ms));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn setting_round_trips_the_config() {
        let mut config = AppConfig::default();
        assert_eq!(RaycastBudgetSetting::load(&config).value(), 2);
        RaycastBudgetSetting::from_int(7).save(&mut config);
        assert_eq!(config.raycast_budget_ms, 7);
        assert_eq!(
            (RaycastBudgetSetting::min(), RaycastBudgetSetting::max()),
            (0, 10)
        );
    }

    #[test]
    fn budget_command_rejects_values_over_10() {
        assert!(parse_budget_ms(0).is_ok());
        assert!(parse_budget_ms(10).is_ok());
        assert_eq!(
            parse_budget_ms(11),
            Err("raycast budget must be between 0 and 10 ms".to_string())
        );
    }
}
