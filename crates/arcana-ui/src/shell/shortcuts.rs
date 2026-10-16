//! Translates the view model's toolkit-agnostic shortcuts ([`crate::commands`]) into egui and
//! reads them from egui input.

use crate::commands::{COMMANDS, CommandId, CommandRegistry, Key, KeyboardShortcut, Modifiers};

const fn key(k: Key) -> egui::Key {
    match k {
        Key::N => egui::Key::N,
        Key::O => egui::Key::O,
        Key::S => egui::Key::S,
        Key::W => egui::Key::W,
        Key::Y => egui::Key::Y,
        Key::Z => egui::Key::Z,
        Key::K => egui::Key::K,
        Key::Comma => egui::Key::Comma,
        Key::ArrowLeft => egui::Key::ArrowLeft,
        Key::F4 => egui::Key::F4,
        Key::F5 => egui::Key::F5,
        Key::F11 => egui::Key::F11,
    }
}

const fn modifiers(m: Modifiers) -> egui::Modifiers {
    let mut out = egui::Modifiers::NONE;
    if m.command {
        out = out.plus(egui::Modifiers::COMMAND);
    }
    if m.alt {
        out = out.plus(egui::Modifiers::ALT);
    }
    out
}

/// The egui equivalent of a command shortcut.
#[must_use]
pub const fn to_egui(s: KeyboardShortcut) -> egui::KeyboardShortcut {
    egui::KeyboardShortcut::new(modifiers(s.modifiers), key(s.key))
}

/// Consume pressed shortcuts from egui input, returning the triggered commands.
#[must_use]
pub fn take_shortcuts(ctx: &egui::Context) -> Vec<CommandId> {
    ctx.input_mut(|i| {
        COMMANDS
            .iter()
            .filter_map(|c| {
                c.shortcut
                    .filter(|s| i.consume_shortcut(&to_egui(*s)))
                    .map(|_| c.id)
            })
            .collect()
    })
}

/// Human-readable shortcut for menus.
#[must_use]
pub fn shortcut_text(ctx: &egui::Context, id: CommandId) -> Option<String> {
    CommandRegistry::get(id)
        .shortcut
        .map(|s| ctx.format_shortcut(&to_egui(s)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csharp_shortcuts_translate_to_egui() {
        let s = |id| to_egui(CommandRegistry::get(id).shortcut.unwrap());
        let ctrl = egui::Modifiers::COMMAND;
        assert_eq!(
            s(CommandId::New),
            egui::KeyboardShortcut::new(ctrl, egui::Key::N)
        );
        assert_eq!(
            s(CommandId::Settings),
            egui::KeyboardShortcut::new(ctrl, egui::Key::Comma)
        );
        assert_eq!(
            s(CommandId::Refresh),
            egui::KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::F5)
        );
        assert_eq!(
            s(CommandId::Exit),
            egui::KeyboardShortcut::new(egui::Modifiers::ALT, egui::Key::F4)
        );
    }

    #[test]
    fn every_shortcut_keeps_its_modifiers() {
        for c in COMMANDS {
            if let Some(sc) = c.shortcut {
                let e = to_egui(sc);
                assert_eq!(e.modifiers.command, sc.modifiers.command);
                assert_eq!(e.modifiers.alt, sc.modifiers.alt);
            }
        }
    }

    #[test]
    fn shortcuts_are_consumed_from_input() {
        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        input.events.push(egui::Event::Key {
            key: egui::Key::F5,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        let mut fired = Vec::new();
        ctx.run_ui(input, |ui| {
            fired = take_shortcuts(ui.ctx());
        })
        .drop_without_applying_deltas();
        assert_eq!(fired, vec![CommandId::Refresh]);
        assert!(shortcut_text(&ctx, CommandId::Save).is_some());
        assert!(shortcut_text(&ctx, CommandId::Cut).is_none());
    }
}
