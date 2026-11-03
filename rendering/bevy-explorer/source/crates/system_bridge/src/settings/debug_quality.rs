use super::{AppConfig, Settings};
use bevy::prelude::*;
use bevy_console::ConsoleCommand;
use console::{AddDebugSection, DebugInput, DebugInputKind, DebugSection, DoAddConsoleCommand};
use serde_json::{Map, Value};

const GROUPS: &[(&str, &str)] = &[
    ("Graphics", "Quality settings"),
    ("Performance", "Performance settings"),
    ("Gameplay", "Gameplay settings"),
    ("Controls", "Control settings"),
    ("Audio", "Audio settings"),
];

pub(super) fn register(app: &mut App, settings: &Settings) {
    app.add_console_command::<QualityCommand, _>(quality);
    for &(category, title) in GROUPS {
        register_group(app, settings, category, title);
    }
}

fn register_group(app: &mut App, settings: &Settings, category: &str, title: &str) {
    let rows: Vec<_> = settings
        .settings
        .iter()
        .enumerate()
        .filter(|(_, s)| s.info.category == category)
        .map(|(i, s)| {
            (
                &s.info,
                i.to_string(),
                format!("/debug_settings {category} {i} {{value}}"),
            )
        })
        .collect();
    let variant_indices: Vec<_> = (0..rows
        .iter()
        .map(|(s, _, _)| s.named_variants.len())
        .max()
        .unwrap_or(0))
        .map(|n| n.to_string())
        .collect();
    let variants: Vec<Vec<_>> = rows
        .iter()
        .map(|(s, _, _)| {
            s.named_variants
                .iter()
                .zip(&variant_indices)
                .map(|(variant, index)| (variant.name.as_str(), index.as_str()))
                .collect()
        })
        .collect();
    let inputs: Vec<_> = rows
        .iter()
        .zip(&variants)
        .map(|((s, field, command), options)| DebugInput {
            label: &s.name,
            field,
            command,
            kind: if options.is_empty() {
                DebugInputKind::Number {
                    min: s.min_value.into(),
                    max: s.max_value.into(),
                    step: s.step_size.into(),
                }
            } else {
                DebugInputKind::Select(options)
            },
        })
        .collect();
    app.add_debug_section(DebugSection {
        title,
        query: &format!("/debug_settings {category}"),
        fields: &[],
        actions: &[],
        inputs: &inputs,
    });
}

/// Read live graphics and performance settings, or set one by its panel index and numeric value.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/debug_settings")]
struct QualityCommand {
    category: String,
    #[arg(requires = "value")]
    index: Option<usize>,
    #[arg(requires = "index", allow_hyphen_values = true)]
    value: Option<f32>,
}

fn quality(
    mut input: ConsoleCommand<QualityCommand>,
    mut config: ResMut<AppConfig>,
    mut settings: ResMut<Settings>,
) {
    let Some(Ok(cmd)) = input.take() else { return };
    if !GROUPS.iter().any(|(category, _)| *category == cmd.category) {
        input.reply_failed("Unknown settings category");
        return;
    }
    if let (Some(index), Some(value)) = (cmd.index, cmd.value) {
        let Some(info) = settings
            .settings
            .get(index)
            .map(|s| &s.info)
            .filter(|s| s.category == cmd.category)
        else {
            input.reply_failed("Unknown quality setting");
            return;
        };
        if !value.is_finite()
            || value < info.min_value
            || value > info.max_value
            || (!info.named_variants.is_empty()
                && (value.fract() != 0.0 || value as usize >= info.named_variants.len()))
        {
            input.reply_failed("Value outside the setting's range");
            return;
        }
        let name = info.name.clone();
        match settings.set_value(&mut config, &name, value) {
            Ok(()) => input.reply_ok(format!("Updated {name}")),
            Err(error) => input.reply_failed(error.to_string()),
        }
    } else {
        let values: Map<_, _> = settings
            .settings
            .iter()
            .enumerate()
            .filter(|(_, s)| s.info.category == cmd.category)
            .map(|(i, s)| (i.to_string(), Value::from(s.info.value)))
            .collect();
        input.reply_ok(Value::Object(values).to_string());
    }
}
