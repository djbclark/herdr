use std::borrow::Cow;

use crossterm::event::{KeyCode, KeyModifiers};

use super::keybindings::KeybindAction;
use crate::{
    config::{ActionKeybinds, IndexedKeybind, Keybinds},
    input::TerminalKey,
};

pub(crate) type KeybindHelpEntry = (String, Cow<'static, str>);
pub(crate) type KeybindHelpGroup = (&'static str, Vec<KeybindHelpEntry>);

/// The character a key types, for overlay shortcuts and text fields.
pub(crate) fn keybind_help_text_char(key: &TerminalKey) -> Option<char> {
    if !key.modifiers.difference(KeyModifiers::SHIFT).is_empty() {
        return None;
    }
    // Text the host says the key produced is authoritative: kitty hosts may
    // report Shift+/ as `/` with text `?` and no shifted alternate.
    if let Some(text) = key.generated_text.as_deref() {
        let mut chars = text.chars();
        if let (Some(character), None) = (chars.next(), chars.next()) {
            if !character.is_control() {
                return Some(character);
            }
        }
    }
    if let Some(character) = key.shifted_codepoint.and_then(char::from_u32) {
        return Some(character);
    }
    let KeyCode::Char(character) = key.code else {
        return None;
    };
    Some(character)
}

#[cfg(test)]
fn entry(key: impl Into<String>, label: &'static str) -> KeybindHelpEntry {
    (key.into(), Cow::Borrowed(label))
}

fn binding_label(bindings: &ActionKeybinds) -> String {
    bindings.label().unwrap_or_else(|| "unset".to_owned())
}

pub(crate) fn indexed_binding_label(bindings: &[IndexedKeybind]) -> String {
    if bindings.is_empty() {
        return "unset".to_owned();
    }
    let mut parts = Vec::new();
    let mut index = 0;
    while index < bindings.len() {
        if let Some(prefix) = indexed_range_prefix(&bindings[index..]) {
            parts.push(format!("{prefix}1..9"));
            index += 9;
        } else {
            parts.push(bindings[index].label.clone());
            index += 1;
        }
    }
    parts.join(" / ")
}

fn indexed_range_prefix(bindings: &[IndexedKeybind]) -> Option<&str> {
    let run = bindings.get(..9)?;
    let prefix = run[0].label.strip_suffix('1')?;
    for (offset, binding) in run.iter().enumerate() {
        let digit = char::from(b'1' + offset as u8);
        if binding.label.strip_suffix(digit) != Some(prefix) {
            return None;
        }
    }
    Some(prefix)
}

/// What a shared keybind row does when the command surface runs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BoundAction {
    None,
    Action(KeybindAction),
    Indexed(IndexedBinding),
    Custom(usize),
}

/// A row that asks for a typed 1–99 index before it runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IndexedBinding {
    SwitchTab,
    SwitchWorkspace,
    FocusAgent,
}

/// One row in the only keybind inventory. Help and the command surface are
/// projections of this list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct KeybindFact {
    pub(crate) help_group: &'static str,
    pub(crate) help_ord: u16,
    pub(crate) help_part: u8,
    pub(crate) help_label: Cow<'static, str>,
    pub(crate) surface_group: &'static str,
    pub(crate) shortcut: String,
    pub(crate) surface_label: Cow<'static, str>,
    pub(crate) action: BoundAction,
}

const HELP_GROUP_ORDER: [&str; 5] = [
    "global",
    "navigation",
    "workspaces / tabs",
    "panes",
    "custom",
];

pub(crate) fn keybind_inventory(
    keybinds: &Keybinds,
    prefixes: &[crate::config::KeyCombo],
) -> Vec<KeybindFact> {
    let mut rows = Vec::new();
    let panes = "panes";
    let pane = "pane";
    bound(
        &mut rows,
        panes,
        0,
        pane,
        &keybinds.split_vertical,
        "split vertical",
        KeybindAction::SplitVertical,
    );
    bound(
        &mut rows,
        panes,
        1,
        pane,
        &keybinds.split_horizontal,
        "split horizontal",
        KeybindAction::SplitHorizontal,
    );
    bound(
        &mut rows,
        panes,
        2,
        pane,
        &keybinds.close_pane,
        "close pane",
        KeybindAction::ClosePane,
    );
    bound(
        &mut rows,
        panes,
        3,
        pane,
        &keybinds.rename_pane,
        "rename pane",
        KeybindAction::RenamePane,
    );
    bound(
        &mut rows,
        panes,
        4,
        pane,
        &keybinds.edit_scrollback,
        "edit scrollback",
        KeybindAction::EditScrollback,
    );
    bound(
        &mut rows,
        panes,
        5,
        pane,
        &keybinds.clear_pane,
        "clear pane",
        KeybindAction::ClearPane,
    );
    bound(
        &mut rows,
        panes,
        6,
        pane,
        &keybinds.copy_mode,
        "copy mode",
        KeybindAction::CopyMode,
    );
    bound(
        &mut rows,
        panes,
        7,
        pane,
        &keybinds.zoom,
        "zoom pane",
        KeybindAction::Zoom,
    );
    bound(
        &mut rows,
        panes,
        8,
        pane,
        &keybinds.resize_mode,
        "resize mode",
        KeybindAction::EnterResizeMode,
    );
    bound(
        &mut rows,
        panes,
        9,
        pane,
        &keybinds.resize_pane_left,
        "resize pane left",
        KeybindAction::ResizePaneLeft,
    );
    bound(
        &mut rows,
        panes,
        10,
        pane,
        &keybinds.resize_pane_down,
        "resize pane down",
        KeybindAction::ResizePaneDown,
    );
    bound(
        &mut rows,
        panes,
        11,
        pane,
        &keybinds.resize_pane_up,
        "resize pane up",
        KeybindAction::ResizePaneUp,
    );
    bound(
        &mut rows,
        panes,
        12,
        pane,
        &keybinds.resize_pane_right,
        "resize pane right",
        KeybindAction::ResizePaneRight,
    );
    bound(
        &mut rows,
        panes,
        14,
        pane,
        &keybinds.focus_pane_left,
        "focus pane left",
        KeybindAction::FocusPaneLeft,
    );
    bound(
        &mut rows,
        panes,
        15,
        pane,
        &keybinds.focus_pane_down,
        "focus pane down",
        KeybindAction::FocusPaneDown,
    );
    bound(
        &mut rows,
        panes,
        16,
        pane,
        &keybinds.focus_pane_up,
        "focus pane up",
        KeybindAction::FocusPaneUp,
    );
    bound(
        &mut rows,
        panes,
        17,
        pane,
        &keybinds.focus_pane_right,
        "focus pane right",
        KeybindAction::FocusPaneRight,
    );
    bound(
        &mut rows,
        panes,
        18,
        pane,
        &keybinds.cycle_pane_next,
        "cycle pane next",
        KeybindAction::CyclePaneNext,
    );
    bound(
        &mut rows,
        panes,
        19,
        pane,
        &keybinds.cycle_pane_previous,
        "cycle pane previous",
        KeybindAction::CyclePanePrevious,
    );
    bound(
        &mut rows,
        panes,
        20,
        pane,
        &keybinds.last_pane,
        "last pane",
        KeybindAction::LastPane,
    );
    bound(
        &mut rows,
        panes,
        13,
        pane,
        &keybinds.toggle_sidebar,
        "toggle sidebar",
        KeybindAction::ToggleSidebar,
    );
    navigate_focus(
        &mut rows,
        &keybinds.navigate.pane_left,
        0,
        "focus pane left",
        KeybindAction::FocusPaneLeft,
    );
    navigate_focus(
        &mut rows,
        &keybinds.navigate.pane_down,
        1,
        "focus pane down",
        KeybindAction::FocusPaneDown,
    );
    navigate_focus(
        &mut rows,
        &keybinds.navigate.pane_up,
        2,
        "focus pane up",
        KeybindAction::FocusPaneUp,
    );
    navigate_focus(
        &mut rows,
        &keybinds.navigate.pane_right,
        3,
        "focus pane right",
        KeybindAction::FocusPaneRight,
    );
    plain(
        &mut rows,
        "navigation",
        2,
        4,
        "move focus",
        pane,
        "left",
        "focus pane left",
        BoundAction::Action(KeybindAction::FocusPaneLeft),
    );
    plain(
        &mut rows,
        "navigation",
        2,
        5,
        "move focus",
        pane,
        "right",
        "focus pane right",
        BoundAction::Action(KeybindAction::FocusPaneRight),
    );
    plain(
        &mut rows,
        "navigation",
        3,
        0,
        "cycle pane",
        pane,
        "tab",
        "cycle pane next",
        BoundAction::Action(KeybindAction::CyclePaneNext),
    );
    plain(
        &mut rows,
        "navigation",
        3,
        1,
        "cycle pane",
        pane,
        "shift+tab",
        "cycle pane previous",
        BoundAction::Action(KeybindAction::CyclePanePrevious),
    );

    let tabs = "workspaces / tabs";
    let tab = "tab";
    bound(
        &mut rows,
        tabs,
        14,
        tab,
        &keybinds.new_tab,
        "new tab",
        KeybindAction::NewTab,
    );
    bound(
        &mut rows,
        tabs,
        15,
        tab,
        &keybinds.rename_tab,
        "rename tab",
        KeybindAction::RenameTab,
    );
    bound(
        &mut rows,
        tabs,
        16,
        tab,
        &keybinds.previous_tab,
        "previous tab",
        KeybindAction::PreviousTab,
    );
    bound(
        &mut rows,
        tabs,
        17,
        tab,
        &keybinds.next_tab,
        "next tab",
        KeybindAction::NextTab,
    );
    bound(
        &mut rows,
        tabs,
        18,
        tab,
        &keybinds.move_tab_previous,
        "move tab left",
        KeybindAction::MoveTabPrevious,
    );
    bound(
        &mut rows,
        tabs,
        19,
        tab,
        &keybinds.move_tab_next,
        "move tab right",
        KeybindAction::MoveTabNext,
    );
    indexed(
        &mut rows,
        tabs,
        20,
        tab,
        &keybinds.switch_tab,
        "switch tab 1-9",
        IndexedBinding::SwitchTab,
    );
    bound(
        &mut rows,
        tabs,
        21,
        tab,
        &keybinds.close_tab,
        "close tab",
        KeybindAction::CloseTab,
    );

    let workspace = "workspace";
    bound(
        &mut rows,
        tabs,
        0,
        workspace,
        &keybinds.workspace_picker,
        "workspace navigation",
        KeybindAction::WorkspacePicker,
    );
    bound(
        &mut rows,
        tabs,
        1,
        workspace,
        &keybinds.goto,
        "session navigator",
        KeybindAction::OpenNavigator,
    );
    bound(
        &mut rows,
        tabs,
        2,
        workspace,
        &keybinds.new_workspace,
        "new workspace",
        KeybindAction::NewWorkspace,
    );
    bound(
        &mut rows,
        tabs,
        3,
        workspace,
        &keybinds.new_worktree,
        "new worktree",
        KeybindAction::NewWorktree,
    );
    bound(
        &mut rows,
        tabs,
        4,
        workspace,
        &keybinds.open_worktree,
        "open worktree",
        KeybindAction::OpenWorktree,
    );
    bound(
        &mut rows,
        tabs,
        5,
        workspace,
        &keybinds.remove_worktree,
        "delete worktree checkout",
        KeybindAction::RemoveWorktree,
    );
    bound(
        &mut rows,
        tabs,
        6,
        workspace,
        &keybinds.rename_workspace,
        "rename workspace",
        KeybindAction::RenameWorkspace,
    );
    bound(
        &mut rows,
        tabs,
        7,
        workspace,
        &keybinds.close_workspace,
        "close workspace",
        KeybindAction::CloseWorkspace,
    );
    bound(
        &mut rows,
        tabs,
        8,
        workspace,
        &keybinds.previous_workspace,
        "previous workspace",
        KeybindAction::PreviousWorkspace,
    );
    bound(
        &mut rows,
        tabs,
        9,
        workspace,
        &keybinds.next_workspace,
        "next workspace",
        KeybindAction::NextWorkspace,
    );
    indexed(
        &mut rows,
        tabs,
        10,
        workspace,
        &keybinds.switch_workspace,
        "switch workspace 1-9",
        IndexedBinding::SwitchWorkspace,
    );
    plain(
        &mut rows,
        "navigation",
        5,
        0,
        "switch workspace",
        workspace,
        "1..9",
        "switch workspace",
        BoundAction::Indexed(IndexedBinding::SwitchWorkspace),
    );

    let global = "global";
    plain(
        &mut rows,
        global,
        0,
        0,
        "prefix mode",
        global,
        crate::config::format_prefix_combos(prefixes),
        "prefix mode",
        BoundAction::None,
    );
    bound(
        &mut rows,
        global,
        1,
        global,
        &keybinds.help,
        "keybinds",
        KeybindAction::Help,
    );
    bound(
        &mut rows,
        global,
        2,
        global,
        &keybinds.settings,
        "settings",
        KeybindAction::Settings,
    );
    bound(
        &mut rows,
        global,
        3,
        global,
        &keybinds.detach,
        "detach",
        KeybindAction::Detach,
    );
    bound(
        &mut rows,
        global,
        4,
        global,
        &keybinds.reload_config,
        "reload config",
        KeybindAction::ReloadConfig,
    );
    bound(
        &mut rows,
        global,
        5,
        global,
        &keybinds.open_notification_target,
        "open notification target",
        KeybindAction::OpenNotificationTarget,
    );

    let navigation = "navigation";
    plain(
        &mut rows,
        navigation,
        0,
        0,
        "back",
        navigation,
        "esc",
        "back",
        BoundAction::None,
    );
    let up = binding_label(&keybinds.navigate.workspace_up);
    let down = binding_label(&keybinds.navigate.workspace_down);
    plain(
        &mut rows,
        navigation,
        1,
        0,
        "workspace list",
        navigation,
        up,
        "workspace list up",
        BoundAction::None,
    );
    plain(
        &mut rows,
        navigation,
        1,
        1,
        "workspace list",
        navigation,
        down,
        "workspace list down",
        BoundAction::None,
    );
    plain(
        &mut rows,
        navigation,
        4,
        0,
        "open workspace",
        navigation,
        "enter",
        "open workspace",
        BoundAction::None,
    );

    let agent = "agent";
    bound(
        &mut rows,
        tabs,
        11,
        agent,
        &keybinds.previous_agent,
        "previous agent",
        KeybindAction::PreviousAgent,
    );
    bound(
        &mut rows,
        tabs,
        12,
        agent,
        &keybinds.next_agent,
        "next agent",
        KeybindAction::NextAgent,
    );
    indexed(
        &mut rows,
        tabs,
        13,
        agent,
        &keybinds.focus_agent,
        "focus agent 1-9",
        IndexedBinding::FocusAgent,
    );

    for (index, binding) in keybinds.custom_commands.iter().enumerate() {
        let label = binding
            .description
            .clone()
            .map(Cow::Owned)
            .unwrap_or(Cow::Borrowed("custom command"));
        rows.push(KeybindFact {
            help_group: "custom",
            help_ord: index as u16,
            help_part: 0,
            help_label: label.clone(),
            surface_group: "custom",
            shortcut: binding.label.clone(),
            surface_label: label,
            action: BoundAction::Custom(index),
        });
    }
    rows
}

fn bound(
    rows: &mut Vec<KeybindFact>,
    help_group: &'static str,
    help_ord: u16,
    surface_group: &'static str,
    bindings: &ActionKeybinds,
    label: &'static str,
    command: KeybindAction,
) {
    let (shortcut, action) = match bindings.label() {
        Some(shortcut) => (shortcut, BoundAction::Action(command)),
        None => ("unset".to_owned(), BoundAction::None),
    };
    plain(
        rows,
        help_group,
        help_ord,
        0,
        label,
        surface_group,
        shortcut,
        label,
        action,
    );
}

fn indexed(
    rows: &mut Vec<KeybindFact>,
    help_group: &'static str,
    help_ord: u16,
    surface_group: &'static str,
    bindings: &[IndexedKeybind],
    label: &'static str,
    kind: IndexedBinding,
) {
    let action = if bindings.is_empty() {
        BoundAction::None
    } else {
        BoundAction::Indexed(kind)
    };
    plain(
        rows,
        help_group,
        help_ord,
        0,
        label,
        surface_group,
        indexed_binding_label(bindings),
        label,
        action,
    );
}

fn navigate_focus(
    rows: &mut Vec<KeybindFact>,
    bindings: &ActionKeybinds,
    help_part: u8,
    label: &'static str,
    command: KeybindAction,
) {
    let (shortcut, action) = match bindings.label() {
        Some(shortcut) => (shortcut, BoundAction::Action(command)),
        None => ("unset".to_owned(), BoundAction::None),
    };
    plain(
        rows,
        "navigation",
        2,
        help_part,
        "move focus",
        "pane",
        shortcut,
        label,
        action,
    );
}

fn plain(
    rows: &mut Vec<KeybindFact>,
    help_group: &'static str,
    help_ord: u16,
    help_part: u8,
    help_label: &'static str,
    surface_group: &'static str,
    shortcut: impl Into<String>,
    surface_label: &'static str,
    action: BoundAction,
) {
    rows.push(KeybindFact {
        help_group,
        help_ord,
        help_part,
        help_label: Cow::Borrowed(help_label),
        surface_group,
        shortcut: shortcut.into(),
        surface_label: Cow::Borrowed(surface_label),
        action,
    });
}

pub(crate) fn keybind_help_groups(
    keybinds: &Keybinds,
    prefixes: &[crate::config::KeyCombo],
) -> Vec<KeybindHelpGroup> {
    let facts = keybind_inventory(keybinds, prefixes);
    let mut groups = Vec::new();
    for group in HELP_GROUP_ORDER {
        let mut rows: Vec<&KeybindFact> = facts
            .iter()
            .filter(|fact| fact.help_group == group)
            .collect();
        if rows.is_empty() {
            continue;
        }
        rows.sort_by_key(|fact| (fact.help_ord, fact.help_part));
        let mut entries = Vec::new();
        let mut index = 0;
        while index < rows.len() {
            let ord = rows[index].help_ord;
            let label = rows[index].help_label.clone();
            let mut parts = Vec::new();
            while index < rows.len() && rows[index].help_ord == ord {
                parts.push(rows[index].shortcut.clone());
                index += 1;
            }
            entries.push((parts.join(" / "), label));
        }
        groups.push((group, entries));
    }
    groups
}

pub(crate) fn filter_keybind_help_groups(
    groups: Vec<KeybindHelpGroup>,
    query: &str,
) -> Vec<KeybindHelpGroup> {
    if query.is_empty() {
        return groups;
    }
    let query = query.to_lowercase();
    groups
        .into_iter()
        .filter_map(|(group, entries)| {
            let entries = entries
                .into_iter()
                .filter(|(key, label)| {
                    key.to_lowercase().contains(&query) || label.to_lowercase().contains(&query)
                })
                .collect::<Vec<_>>();
            (!entries.is_empty()).then_some((group, entries))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn groups() -> Vec<KeybindHelpGroup> {
        vec![
            (
                "workspaces / tabs",
                vec![entry("w", "workspace navigation"), entry("c", "new tab")],
            ),
            (
                "panes",
                vec![entry("v", "split vertical"), entry("x", "close pane")],
            ),
        ]
    }

    #[test]
    fn text_char_prefers_generated_text_then_alternate_then_code() {
        let parse = |bytes: &str| {
            crate::input::parse_terminal_key_sequence(bytes)
                .unwrap_or_else(|| panic!("{bytes:?} parses"))
        };
        for (bytes, want) in [
            // Shift+/ typing `?` with no shifted alternate.
            ("\x1b[47;2;63u", Some('?')),
            ("\x1b[47u", Some('/')),
            ("/", Some('/')),
            // Portuguese Shift+7: alternate and text agree.
            ("\x1b[55:47;2;47u", Some('/')),
            ("\x1b[55:47;2u", Some('/')),
            ("\x1b[55;2u", Some('7')),
            ("\x1b[47;5u", None),
            ("\x1b[47;3u", None),
        ] {
            assert_eq!(keybind_help_text_char(&parse(bytes)), want, "{bytes:?}");
        }
    }

    #[test]
    fn filter_matches_labels_and_shortcuts_case_insensitively() {
        let filtered = filter_keybind_help_groups(groups(), "WoRk");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].1[0].1, "workspace navigation");

        let filtered = filter_keybind_help_groups(groups(), "x");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].1[0].1, "close pane");
        assert!(filter_keybind_help_groups(groups(), "panes").is_empty());
    }

    #[test]
    fn help_and_the_command_surface_read_one_inventory() {
        let prefixes = [(KeyCode::Char('b'), KeyModifiers::CONTROL)];
        let keybinds = Keybinds::default();
        let facts = keybind_inventory(&keybinds, &prefixes);
        let groups = keybind_help_groups(&keybinds, &prefixes);
        assert_eq!(
            groups.iter().map(|(group, _)| *group).collect::<Vec<_>>(),
            ["global", "navigation", "workspaces / tabs", "panes"]
        );
        let navigation = &groups[1].1;
        assert_eq!(navigation[0].0, "esc");
        assert_eq!(navigation[0].1, "back");
        assert_eq!(navigation[1].1, "workspace list");
        assert_eq!(navigation[2].1, "move focus");
        assert!(navigation[2].0.ends_with(" / left / right"));
        assert_eq!(navigation[3].0, "tab / shift+tab");
        assert_eq!(navigation[3].1, "cycle pane");
        assert_eq!(navigation[4].0, "enter");
        assert_eq!(navigation[4].1, "open workspace");
        assert_eq!(navigation[5].0, "1..9");
        assert_eq!(navigation[5].1, "switch workspace");

        let panes = &groups[3].1;
        let toggle = panes
            .iter()
            .position(|entry| entry.1 == "toggle sidebar")
            .expect("toggle");
        let focus = panes
            .iter()
            .position(|entry| entry.1 == "focus pane left")
            .expect("focus");
        assert!(toggle < focus);
        let zoom = facts
            .iter()
            .find(|fact| fact.surface_label == "zoom pane")
            .expect("zoom");
        let help_zoom = panes
            .iter()
            .find(|entry| entry.1 == "zoom pane")
            .expect("help zoom");
        assert_eq!(help_zoom.0, zoom.shortcut);
        assert_eq!(zoom.surface_group, "pane");
        assert_eq!(
            facts
                .iter()
                .filter(|fact| fact.surface_label == "focus pane left")
                .count(),
            3
        );
        let mut surface_groups = Vec::new();
        for fact in &facts {
            if surface_groups.last().copied() != Some(fact.surface_group) {
                surface_groups.push(fact.surface_group);
            }
        }
        assert_eq!(
            surface_groups,
            ["pane", "tab", "workspace", "global", "navigation", "agent"]
        );
    }

    #[test]
    fn help_lists_every_configured_prefix() {
        let groups = keybind_help_groups(
            &Keybinds::default(),
            &[
                (KeyCode::Char(' '), KeyModifiers::CONTROL),
                (KeyCode::Char('s'), KeyModifiers::CONTROL),
            ],
        );
        let global = &groups[0].1;
        assert_eq!(global[0].0, "ctrl+space / ctrl+s");
        assert_eq!(global[0].1, "prefix mode");
    }
}
