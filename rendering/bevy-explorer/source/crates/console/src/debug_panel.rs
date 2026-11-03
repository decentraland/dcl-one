//! HUD sections declared by the plugins that own their console commands.

use std::collections::BTreeMap;

use bevy::prelude::*;
use bevy_console::ConsoleCommand;
use serde::Serialize;

/// A read-only JSON command, its labeled fields (dot-separated keys or array indices),
/// and buttons paired with the existing console command lines they invoke.
#[derive(Serialize)]
pub struct DebugSection<'a> {
    pub title: &'a str,
    pub query: &'a str,
    pub fields: &'a [(&'a str, &'a str)],
    pub actions: &'a [(&'a str, &'a str)],
    pub inputs: &'a [DebugInput<'a>],
}

/// The command template substitutes the submitted value for `{value}`. A field path binds the
/// editor to the plugin's live value. An empty path is an argument supplied by the tester.
#[derive(Serialize)]
pub struct DebugInput<'a> {
    pub label: &'a str,
    pub field: &'a str,
    pub command: &'a str,
    pub kind: DebugInputKind<'a>,
}

#[derive(Serialize)]
#[serde(tag = "type", content = "options", rename_all = "lowercase")]
pub enum DebugInputKind<'a> {
    Text,
    Toggle,
    Number { min: f64, max: f64, step: f64 },
    Select(&'a [(&'a str, &'a str)]),
}

#[derive(Default, Resource)]
pub(crate) struct DebugSections(BTreeMap<String, serde_json::Value>);

pub trait AddDebugSection {
    fn add_debug_section(&mut self, section: DebugSection<'_>) -> &mut Self;

    /// In retail, show the bound values without the development-only controls. The plugin
    /// must also guard its mutating command in Rust.
    fn add_preview_debug_inputs(&mut self, section: DebugSection<'_>) -> &mut Self;
}

impl AddDebugSection for App {
    fn add_preview_debug_inputs(&mut self, section: DebugSection<'_>) -> &mut Self {
        if self
            .world()
            .get_resource::<common::structs::PreviewMode>()
            .is_some_and(|p| p.is_preview)
        {
            return self.add_debug_section(section);
        }
        let mut fields = section.fields.to_vec();
        for input in section.inputs {
            if !input.field.is_empty() && !fields.iter().any(|(_, path)| *path == input.field) {
                fields.push((input.label, input.field));
            }
        }
        self.add_debug_section(DebugSection {
            fields: &fields,
            actions: &[],
            inputs: &[],
            ..section
        })
    }

    fn add_debug_section(&mut self, section: DebugSection<'_>) -> &mut Self {
        let mut sections = self.world_mut().get_resource_or_init::<DebugSections>();
        let title = section.title;
        assert!(
            !sections.0.contains_key(title),
            "duplicate debug section: {title}"
        );
        sections.0.insert(
            title.to_owned(),
            serde_json::to_value(section).expect("debug section metadata"),
        );
        self
    }
}

/// List the running build's debug panel sections, fields and actions as JSON.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/debug_panel")]
pub(crate) struct DebugPanelCommand {
    /// Report whether this engine was compiled with debug assertions.
    #[arg(long)]
    dev_build: bool,
}

pub(crate) fn debug_panel(
    mut input: ConsoleCommand<DebugPanelCommand>,
    sections: Res<DebugSections>,
) {
    if let Some(Ok(command)) = input.take() {
        if command.dev_build {
            input.reply_ok(cfg!(debug_assertions).to_string());
        } else {
            input
                .reply_ok(serde_json::to_string(&sections.0.values().collect::<Vec<_>>()).unwrap());
        }
    }
}
