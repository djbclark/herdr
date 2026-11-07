use crossterm::event::{KeyCode, KeyEventKind, KeyModifiers};

use super::*;
use crate::config::{KeyCombo, Keybinds};
use crate::input::{keybind_inventory, BoundAction, IndexedBinding, KeybindAction, TerminalKey};

/// What activating a row does. Indexed rows open the digit dialog first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CommandRun {
    None,
    Action(KeybindAction),
    Indexed(IndexedCommand),
    Custom(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CommandEntry {
    pub(super) group: &'static str,
    pub(super) shortcut: String,
    pub(super) description: String,
    pub(super) run: CommandRun,
}

/// One visual line. Only `Full` and the first `Shortcut` chunk can be selected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum SurfaceLine {
    Title(&'static str),
    Blank,
    Full {
        selected: usize,
        shortcut: String,
        description: String,
    },
    Shortcut {
        selected: usize,
        text: String,
    },
    Continuation {
        text: String,
    },
    Description {
        text: String,
    },
}

impl SurfaceLine {
    fn selectable_start(&self) -> Option<usize> {
        match self {
            Self::Full { selected, .. } | Self::Shortcut { selected, .. } => Some(*selected),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CommandView {
    pub(super) lines: Vec<SurfaceLine>,
    pub(super) scroll: usize,
    pub(super) highlight_row: Option<usize>,
    pub(super) group: &'static str,
    pub(super) selected: usize,
    pub(super) visible: Vec<usize>,
}

pub(super) enum CommandSurfaceEffect {
    Redraw,
    Close,
    Run(CommandRun),
}

impl ClientShellState {
    pub(super) fn open_command_surface(&mut self) {
        let mut overlay = ClientCommandSurfaceOverlay::new();
        if self.config.command_surface_anticipate {
            let context = self.surface_context();
            if let Some(predicted) = self.command_anticipate.predict(&context) {
                let entries =
                    command_catalog(&self.config.keybinds.keybinds, &self.config.keybinds.prefix);
                if let Some(index) = entries
                    .iter()
                    .position(|entry| predicted.matches_entry(entry))
                {
                    overlay.selected = index;
                }
            }
        }
        self.overlay = Some(ClientShellOverlay::CommandSurface(overlay));
    }
}

pub(super) fn command_catalog(keybinds: &Keybinds, prefixes: &[KeyCombo]) -> Vec<CommandEntry> {
    keybind_inventory(keybinds, prefixes)
        .into_iter()
        .map(|fact| CommandEntry {
            group: fact.surface_group,
            shortcut: fact.shortcut,
            description: fact.surface_label.into_owned(),
            run: match fact.action {
                BoundAction::None => CommandRun::None,
                BoundAction::Action(action) => CommandRun::Action(action),
                BoundAction::Indexed(IndexedBinding::SwitchTab) => {
                    CommandRun::Indexed(IndexedCommand::SwitchTab)
                }
                BoundAction::Indexed(IndexedBinding::SwitchWorkspace) => {
                    CommandRun::Indexed(IndexedCommand::SwitchWorkspace)
                }
                BoundAction::Indexed(IndexedBinding::FocusAgent) => {
                    CommandRun::Indexed(IndexedCommand::FocusAgent)
                }
                BoundAction::Custom(index) => CommandRun::Custom(index),
            },
        })
        .collect()
}

/// Exact description, then prefix, then word start, then substring. A shortcut
/// match uses the same ladder one step lower, so a shortcut query still hits
/// and a named command still wins.
fn match_rank(text: &str, query: &str) -> Option<u8> {
    let text = text.to_lowercase();
    if text == query {
        Some(0)
    } else if text.starts_with(query) {
        Some(1)
    } else if text.split_whitespace().any(|word| word.starts_with(query)) {
        Some(2)
    } else if text.contains(query) {
        Some(3)
    } else {
        None
    }
}

pub(super) fn filter_entries(entries: &[CommandEntry], query: &str) -> Vec<usize> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return (0..entries.len()).collect();
    }
    let mut ranked = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        let description = match_rank(&entry.description, &query);
        let shortcut = match_rank(&entry.shortcut, &query).map(|rank| rank.saturating_add(4));
        let rank = match (description, shortcut) {
            (Some(description), Some(shortcut)) => Some(description.min(shortcut)),
            (Some(rank), None) | (None, Some(rank)) => Some(rank),
            (None, None) => None,
        };
        if let Some(rank) = rank {
            ranked.push((rank, index));
        }
    }
    ranked.sort_by_key(|(rank, index)| (*rank, *index));
    ranked.into_iter().map(|(_, index)| index).collect()
}

pub(super) fn surface_lines(
    entries: &[CommandEntry],
    visible: &[usize],
    content_width: usize,
) -> Vec<SurfaceLine> {
    let width = if content_width == 0 {
        usize::MAX / 4
    } else {
        content_width
    };
    let shown: Vec<(usize, &CommandEntry)> = visible
        .iter()
        .enumerate()
        .filter_map(|(ordinal, index)| entries.get(*index).map(|entry| (ordinal, entry)))
        .collect();
    let mut lines = Vec::new();
    let mut previous: Option<&str> = None;
    for (ordinal, entry) in shown {
        if previous.is_some() && previous != Some(entry.group) {
            lines.push(SurfaceLine::Blank);
            lines.push(SurfaceLine::Blank);
        }
        if previous != Some(entry.group) {
            lines.push(SurfaceLine::Title(entry.group));
            previous = Some(entry.group);
        }
        push_entry_lines(&mut lines, ordinal, entry, width);
    }
    lines
}

fn push_entry_lines(
    lines: &mut Vec<SurfaceLine>,
    ordinal: usize,
    entry: &CommandEntry,
    width: usize,
) {
    let shortcut_cells = entry.shortcut.chars().count();
    if shortcut_cells.saturating_add(2) <= width {
        lines.push(SurfaceLine::Full {
            selected: ordinal,
            shortcut: entry.shortcut.clone(),
            description: entry.description.clone(),
        });
        return;
    }
    let mut chunks = wrap_chars(&entry.shortcut, width.max(1));
    let first = chunks.remove(0);
    lines.push(SurfaceLine::Shortcut {
        selected: ordinal,
        text: first,
    });
    for text in chunks {
        lines.push(SurfaceLine::Continuation { text });
    }
    lines.push(SurfaceLine::Description {
        text: entry.description.clone(),
    });
}

fn wrap_chars(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut chunks = Vec::new();
    let mut current = String::new();
    for character in text.chars() {
        if current.chars().count() >= width {
            chunks.push(std::mem::take(&mut current));
        }
        current.push(character);
    }
    if current.is_empty() {
        chunks.push(String::new());
    } else {
        chunks.push(current);
    }
    chunks
}

pub(super) fn shortcut_pad(lines: &[SurfaceLine]) -> usize {
    lines
        .iter()
        .filter_map(|line| match line {
            SurfaceLine::Full { shortcut, .. } => Some(shortcut.chars().count()),
            _ => None,
        })
        .max()
        .unwrap_or(0)
}

pub(super) fn selection_line(lines: &[SurfaceLine], selected: usize) -> Option<usize> {
    lines
        .iter()
        .position(|line| line.selectable_start() == Some(selected))
}

/// Keep the selected row on the first viewport line until the tail is shorter
/// than the viewport, then let the highlight move down inside the stuck window.
pub(super) fn pinned_scroll(selected_line: usize, total: usize, viewport: usize) -> usize {
    if viewport == 0 {
        return 0;
    }
    selected_line.min(total.saturating_sub(viewport))
}

pub(super) fn selectable_capacity(lines: &[SurfaceLine], viewport: usize) -> usize {
    if viewport == 0 || lines.is_empty() {
        return 1;
    }
    let mut best = 0;
    for start in 0..lines.len() {
        let end = (start + viewport).min(lines.len());
        let count = lines[start..end]
            .iter()
            .filter(|line| line.selectable_start().is_some())
            .count();
        best = best.max(count);
    }
    best.max(1)
}

pub(super) fn command_view(
    entries: &[CommandEntry],
    query: &str,
    selected: usize,
    content_width: usize,
    viewport: usize,
) -> CommandView {
    let visible = filter_entries(entries, query);
    let selected = if visible.is_empty() {
        0
    } else {
        selected.min(visible.len() - 1)
    };
    let lines = surface_lines(entries, &visible, content_width);
    let group = visible
        .get(selected)
        .and_then(|index| entries.get(*index))
        .map(|entry| entry.group)
        .unwrap_or("commands");
    let (scroll, highlight_row) = selection_line(&lines, selected)
        .map(|line| {
            let scroll = pinned_scroll(line, lines.len(), viewport);
            (scroll, Some(line - scroll))
        })
        .unwrap_or((0, None));
    CommandView {
        lines,
        scroll,
        highlight_row,
        group,
        selected,
        visible,
    }
}

/// `1`–`99` become a 0-based index. A leading zero and an empty buffer are rejected.
pub(super) fn parse_command_index(buffer: &str) -> Option<usize> {
    if buffer.is_empty() || buffer.starts_with('0') || buffer.len() > 2 {
        return None;
    }
    let value: usize = buffer.parse().ok()?;
    (1..=99).contains(&value).then_some(value - 1)
}

pub(super) fn command_surface_rect(area: Rect, sidebar_width: u16) -> Option<Rect> {
    let x = area.x.saturating_add(sidebar_width).saturating_add(2);
    let y = area.y.saturating_add(2);
    let width = area.width.saturating_sub(sidebar_width).saturating_sub(4);
    let height = area.height.saturating_sub(4);
    if width < 22 || height < 8 {
        return None;
    }
    Some(Rect::new(x, y, width, height))
}

pub(super) fn handle_command_surface_key(
    overlay: &mut ClientCommandSurfaceOverlay,
    entries: &[CommandEntry],
    key: &TerminalKey,
    viewport_rows: usize,
    content_width: usize,
) -> CommandSurfaceEffect {
    if key.kind == KeyEventKind::Release {
        return CommandSurfaceEffect::Redraw;
    }
    if overlay.digit.is_some() {
        overlay.pending_g = false;
        return handle_digit(overlay, key);
    }
    if overlay.search_focused {
        if let Some(changed) = overlay.query.handle_key(key) {
            if changed {
                overlay.selected = 0;
            }
            overlay.pending_g = false;
            return CommandSurfaceEffect::Redraw;
        }
        return handle_search_command(overlay, entries, key, viewport_rows, content_width);
    }
    handle_idle_command(overlay, entries, key, viewport_rows, content_width)
}

fn handle_digit(
    overlay: &mut ClientCommandSurfaceOverlay,
    key: &TerminalKey,
) -> CommandSurfaceEffect {
    let (code, modifiers) = crate::config::normalize_key_combo((key.code, key.modifiers));
    if code == KeyCode::Esc {
        overlay.digit = None;
        return CommandSurfaceEffect::Redraw;
    }
    if code == KeyCode::Enter {
        let Some(digit) = overlay.digit.as_ref() else {
            return CommandSurfaceEffect::Redraw;
        };
        let Some(index) = parse_command_index(&digit.buffer) else {
            return CommandSurfaceEffect::Redraw;
        };
        let action = digit.kind.action(index);
        overlay.digit = None;
        return CommandSurfaceEffect::Run(CommandRun::Action(action));
    }
    let Some(digit) = overlay.digit.as_mut() else {
        return CommandSurfaceEffect::Redraw;
    };
    if code == KeyCode::Backspace && modifiers.is_empty() {
        digit.buffer.pop();
        return CommandSurfaceEffect::Redraw;
    }
    if modifiers.difference(KeyModifiers::SHIFT).is_empty() {
        if let Some(character) = crate::input::keybind_help_text_char(key) {
            if character.is_ascii_digit()
                && digit.buffer.len() < 2
                && !(digit.buffer.is_empty() && character == '0')
            {
                digit.buffer.push(character);
            }
        }
    }
    CommandSurfaceEffect::Redraw
}

fn handle_search_command(
    overlay: &mut ClientCommandSurfaceOverlay,
    entries: &[CommandEntry],
    key: &TerminalKey,
    viewport_rows: usize,
    content_width: usize,
) -> CommandSurfaceEffect {
    let (code, modifiers) = crate::config::normalize_key_combo((key.code, key.modifiers));
    if code == KeyCode::Esc {
        overlay.search_focused = false;
        overlay.query.clear();
        overlay.pending_g = false;
        overlay.selected = 0;
        return CommandSurfaceEffect::Redraw;
    }
    if code == KeyCode::Enter {
        return commit_search(overlay, entries);
    }
    if let Some(delta) = search_motion(code, modifiers) {
        move_selection(overlay, entries, content_width, viewport_rows, delta);
        return CommandSurfaceEffect::Redraw;
    }
    CommandSurfaceEffect::Redraw
}

fn search_motion(code: KeyCode, modifiers: KeyModifiers) -> Option<Motion> {
    let ctrl = modifiers == KeyModifiers::CONTROL;
    let alt = modifiers == KeyModifiers::ALT;
    let plain = modifiers.is_empty();
    match code {
        KeyCode::Up | KeyCode::Char('p') if plain || code == KeyCode::Char('p') && ctrl => {
            Some(Motion::Entries(-1))
        }
        KeyCode::Down | KeyCode::Char('n') if plain || code == KeyCode::Char('n') && ctrl => {
            Some(Motion::Entries(1))
        }
        KeyCode::PageUp => Some(Motion::Page(-1)),
        KeyCode::PageDown | KeyCode::Char('v')
            if plain || code == KeyCode::Char('v') && (ctrl || alt) =>
        {
            Some(Motion::Page(if code == KeyCode::Char('v') && alt {
                -1
            } else {
                1
            }))
        }
        _ => None,
    }
}

fn commit_search(
    overlay: &mut ClientCommandSurfaceOverlay,
    entries: &[CommandEntry],
) -> CommandSurfaceEffect {
    let query = overlay.query.to_string();
    let visible = filter_entries(entries, &query);
    match visible.len() {
        0 => CommandSurfaceEffect::Redraw,
        1 => {
            let index = visible[0];
            overlay.filter = query;
            overlay.query.clear();
            overlay.search_focused = false;
            overlay.selected = 0;
            overlay.pending_g = false;
            activate(overlay, entries, index)
        }
        _ => {
            overlay.filter = query;
            overlay.query.clear();
            overlay.search_focused = false;
            overlay.selected = 0;
            overlay.pending_g = false;
            CommandSurfaceEffect::Redraw
        }
    }
}

fn handle_idle_command(
    overlay: &mut ClientCommandSurfaceOverlay,
    entries: &[CommandEntry],
    key: &TerminalKey,
    viewport_rows: usize,
    content_width: usize,
) -> CommandSurfaceEffect {
    if overlay.pending_g {
        overlay.pending_g = false;
        if is_plain_g(key) {
            overlay.selected = 0;
            return CommandSurfaceEffect::Redraw;
        }
    } else if is_plain_g(key) {
        overlay.pending_g = true;
        return CommandSurfaceEffect::Redraw;
    }

    let (code, modifiers) = crate::config::normalize_key_combo((key.code, key.modifiers));
    if code == KeyCode::Esc {
        if overlay.filter.is_empty() {
            return CommandSurfaceEffect::Close;
        }
        overlay.filter.clear();
        overlay.selected = 0;
        return CommandSurfaceEffect::Redraw;
    }
    if code == KeyCode::Enter {
        let view = command_view(
            entries,
            &overlay.filter,
            overlay.selected,
            content_width,
            viewport_rows,
        );
        if view.visible.is_empty() {
            return CommandSurfaceEffect::Redraw;
        }
        overlay.selected = view.selected;
        return activate(overlay, entries, view.visible[view.selected]);
    }
    if crate::input::keybind_help_text_char(key) == Some('/')
        || (code == KeyCode::Char('s') && modifiers == KeyModifiers::CONTROL)
    {
        start_search(overlay);
        return CommandSurfaceEffect::Redraw;
    }
    if let Some(delta) = idle_motion(code, modifiers) {
        move_selection(overlay, entries, content_width, viewport_rows, delta);
    }
    CommandSurfaceEffect::Redraw
}

fn start_search(overlay: &mut ClientCommandSurfaceOverlay) {
    let previous = overlay.filter.clone();
    overlay.search_focused = true;
    overlay.query.clear();
    overlay.pending_g = false;
    if previous != overlay.query.as_str() {
        overlay.selected = 0;
    }
}

fn is_plain_g(key: &TerminalKey) -> bool {
    let (code, modifiers) = crate::config::normalize_key_combo((key.code, key.modifiers));
    matches!(code, KeyCode::Char('g')) && modifiers.is_empty()
}

fn is_last_motion(code: KeyCode, modifiers: KeyModifiers) -> bool {
    // A raw `G` arrives as Char('G') with Shift. End stays unmodified.
    match code {
        KeyCode::End => modifiers.is_empty(),
        KeyCode::Char('G') => modifiers.is_empty() || modifiers == KeyModifiers::SHIFT,
        KeyCode::Char('g') => modifiers == KeyModifiers::SHIFT,
        _ => false,
    }
}

enum Motion {
    Entries(isize),
    Page(isize),
    Half(isize),
    First,
    Last,
}

fn idle_motion(code: KeyCode, modifiers: KeyModifiers) -> Option<Motion> {
    let ctrl = modifiers == KeyModifiers::CONTROL;
    let alt = modifiers == KeyModifiers::ALT;
    let plain = modifiers.is_empty();
    if is_last_motion(code, modifiers) {
        return Some(Motion::Last);
    }
    match code {
        KeyCode::Home if plain => Some(Motion::First),
        KeyCode::Char('j') | KeyCode::Down if plain => Some(Motion::Entries(1)),
        KeyCode::Char('k') | KeyCode::Up if plain => Some(Motion::Entries(-1)),
        KeyCode::Char('n') if ctrl => Some(Motion::Entries(1)),
        KeyCode::Char('p') if ctrl => Some(Motion::Entries(-1)),
        KeyCode::Char('d') if ctrl => Some(Motion::Half(1)),
        KeyCode::Char('u') if ctrl => Some(Motion::Half(-1)),
        KeyCode::Char('f') if ctrl => Some(Motion::Page(1)),
        KeyCode::Char('b') if ctrl => Some(Motion::Page(-1)),
        KeyCode::Char('v') if ctrl => Some(Motion::Page(1)),
        KeyCode::Char('v') if alt => Some(Motion::Page(-1)),
        KeyCode::PageDown if plain || modifiers == KeyModifiers::SHIFT => Some(Motion::Page(1)),
        KeyCode::PageUp => Some(Motion::Page(-1)),
        _ => None,
    }
}

fn move_selection(
    overlay: &mut ClientCommandSurfaceOverlay,
    entries: &[CommandEntry],
    content_width: usize,
    viewport_rows: usize,
    motion: Motion,
) {
    let query = live_query(overlay);
    let visible = filter_entries(entries, query);
    if visible.is_empty() {
        overlay.selected = 0;
        return;
    }
    let last = visible.len() - 1;
    overlay.selected = overlay.selected.min(last);
    let lines = surface_lines(entries, &visible, content_width);
    let page = selectable_capacity(&lines, viewport_rows) as isize;
    let half = (page / 2).max(1);
    let delta = match motion {
        Motion::Entries(delta) => delta,
        Motion::Page(sign) => page * sign,
        Motion::Half(sign) => half * sign,
        Motion::First => {
            overlay.selected = 0;
            return;
        }
        Motion::Last => {
            overlay.selected = last;
            return;
        }
    };
    overlay.selected = (overlay.selected as isize + delta).clamp(0, last as isize) as usize;
}

fn live_query(overlay: &ClientCommandSurfaceOverlay) -> &str {
    if overlay.search_focused {
        overlay.query.as_str()
    } else {
        overlay.filter.as_str()
    }
}

fn activate(
    overlay: &mut ClientCommandSurfaceOverlay,
    entries: &[CommandEntry],
    index: usize,
) -> CommandSurfaceEffect {
    let Some(entry) = entries.get(index) else {
        return CommandSurfaceEffect::Redraw;
    };
    match entry.run {
        CommandRun::None => CommandSurfaceEffect::Redraw,
        CommandRun::Indexed(kind) => {
            overlay.digit = Some(ClientCommandDigitEntry {
                kind,
                buffer: String::new(),
            });
            CommandSurfaceEffect::Redraw
        }
        other => CommandSurfaceEffect::Run(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode, modifiers: KeyModifiers) -> TerminalKey {
        TerminalKey::new(code, modifiers)
    }

    fn entry(group: &'static str, shortcut: &str, description: &str) -> CommandEntry {
        CommandEntry {
            group,
            shortcut: shortcut.to_owned(),
            description: description.to_owned(),
            run: CommandRun::None,
        }
    }

    #[test]
    fn pinned_scroll_stays_at_the_top_until_the_tail() {
        assert_eq!(pinned_scroll(1, 100, 29), 1);
        assert_eq!(1 - pinned_scroll(1, 100, 29), 0);
        assert_eq!(pinned_scroll(80, 100, 29), 71);
        assert_eq!(80 - pinned_scroll(80, 100, 29), 9);
        assert_eq!(pinned_scroll(1, 10, 29), 0);
    }

    #[test]
    fn page_counts_selectable_starts_in_the_tallest_window() {
        let lines = vec![
            SurfaceLine::Title("pane"),
            SurfaceLine::Full {
                selected: 0,
                shortcut: "a".into(),
                description: "one".into(),
            },
            SurfaceLine::Blank,
            SurfaceLine::Full {
                selected: 1,
                shortcut: "b".into(),
                description: "two".into(),
            },
        ];
        assert_eq!(selectable_capacity(&lines, 0), 1);
        assert_eq!(selectable_capacity(&lines, 2), 1);
        assert_eq!(selectable_capacity(&lines, 4), 2);
    }

    #[test]
    fn index_parser_accepts_one_or_two_digits_without_a_leading_zero() {
        assert_eq!(parse_command_index(""), None);
        assert_eq!(parse_command_index("0"), None);
        assert_eq!(parse_command_index("00"), None);
        assert_eq!(parse_command_index("01"), None);
        assert_eq!(parse_command_index("1"), Some(0));
        assert_eq!(parse_command_index("12"), Some(11));
        assert_eq!(parse_command_index("99"), Some(98));
        assert_eq!(parse_command_index("100"), None);
    }

    #[test]
    fn default_catalog_splits_directions_and_keeps_group_order() {
        let entries = command_catalog(
            &Keybinds::default(),
            &[(KeyCode::Char('b'), KeyModifiers::CONTROL)],
        );
        let groups: Vec<&str> =
            entries
                .iter()
                .map(|entry| entry.group)
                .fold(Vec::new(), |mut groups, group| {
                    if groups.last().copied() != Some(group) {
                        groups.push(group);
                    }
                    groups
                });
        assert_eq!(
            groups,
            ["pane", "tab", "workspace", "global", "navigation", "agent"]
        );
        assert!(entries
            .iter()
            .all(|entry| entry.description != "move focus"));
        assert!(entries
            .iter()
            .all(|entry| entry.description != "workspace list"));
        assert!(entries
            .iter()
            .all(|entry| entry.description != "cycle pane"));
        assert_eq!(
            entries
                .iter()
                .filter(|entry| entry.description == "focus pane left")
                .count(),
            3
        );
        assert_eq!(
            entries
                .iter()
                .filter(|entry| entry.description == "focus pane right")
                .count(),
            3
        );
        assert_eq!(
            entries
                .iter()
                .filter(|entry| entry.description == "focus pane down")
                .count(),
            2
        );
        assert_eq!(
            entries
                .iter()
                .filter(|entry| entry.description == "cycle pane next")
                .count(),
            2
        );
        assert!(entries.iter().any(|entry| {
            entry.description == "switch tab 1-9" && matches!(entry.run, CommandRun::Indexed(_))
        }));
        assert!(entries
            .iter()
            .any(|entry| { entry.shortcut == "1..9" && entry.description == "switch workspace" }));
        let previous = entries
            .iter()
            .find(|entry| entry.description == "previous workspace")
            .unwrap();
        assert_eq!(previous.shortcut, "unset");
        assert_eq!(previous.run, CommandRun::None);
        assert!(filter_entries(&entries, "panes").is_empty());
        assert!(!filter_entries(&entries, "PaNe").is_empty());
        let new_tab = filter_entries(&entries, "new tab");
        assert_eq!(
            entries[new_tab[0]].description, "new tab",
            "an exact name stays ahead of a longer match"
        );
    }

    #[test]
    fn filter_ranks_exact_names_ahead_of_substrings_and_keeps_shortcuts() {
        let entries = vec![
            entry("pane", "a", "move pane to new tab"),
            entry("tab", "b", "new tab"),
            entry("workspace", "prefix+z", "zoom pane"),
            entry("global", "x", "prefix+z note"),
            entry("pane", "c", "xpane"),
            entry("tab", "d", "pane split"),
        ];
        assert_eq!(filter_entries(&entries, ""), vec![0, 1, 2, 3, 4, 5]);
        assert_eq!(filter_entries(&entries, "   "), vec![0, 1, 2, 3, 4, 5]);
        assert_eq!(
            filter_entries(&entries, "new tab")
                .into_iter()
                .map(|index| entries[index].description.as_str())
                .collect::<Vec<_>>(),
            vec!["new tab", "move pane to new tab"]
        );
        assert_eq!(
            filter_entries(&entries, "prefix+z")
                .into_iter()
                .map(|index| entries[index].description.as_str())
                .collect::<Vec<_>>(),
            vec!["prefix+z note", "zoom pane"]
        );
        assert_eq!(
            filter_entries(&entries, "pane")
                .into_iter()
                .map(|index| entries[index].description.as_str())
                .collect::<Vec<_>>(),
            vec!["pane split", "move pane to new tab", "zoom pane", "xpane"]
        );
    }

    #[test]
    fn lines_use_two_blanks_and_wrap_only_a_shortcut_that_does_not_fit() {
        let entries = vec![
            entry(
                "pane",
                "short",
                "a long description that would wrap in a narrow help panel",
            ),
            entry("tab", &"w".repeat(30), "wrapped shortcut"),
        ];
        let visible: Vec<usize> = (0..entries.len()).collect();
        let lines = surface_lines(&entries, &visible, 12);
        assert!(matches!(lines.first(), Some(SurfaceLine::Title("pane"))));
        assert!(matches!(lines[1], SurfaceLine::Full { .. }));
        assert!(matches!(lines[2], SurfaceLine::Blank));
        assert!(matches!(lines[3], SurfaceLine::Blank));
        assert!(matches!(lines[4], SurfaceLine::Title("tab")));
        assert!(matches!(lines[5], SurfaceLine::Shortcut { .. }));
        assert!(lines
            .iter()
            .any(|line| matches!(line, SurfaceLine::Description { .. })));
        assert!(!matches!(lines.last(), Some(SurfaceLine::Blank)));
        let view = command_view(&entries, "", 0, 80, 29);
        // The fixture is shorter than the viewport, so the title stays put and
        // the first command is the second visual line.
        assert_eq!(view.highlight_row, Some(1));
        assert_eq!(view.scroll, 0);
        assert_eq!(view.group, "pane");
    }

    #[test]
    fn digit_entry_confirms_a_two_digit_index_and_escape_stays_open() {
        let entries = command_catalog(&Keybinds::default(), &[]);
        let mut overlay = ClientCommandSurfaceOverlay::new();
        let switch = entries
            .iter()
            .position(|entry| entry.description == "switch tab 1-9")
            .unwrap();
        overlay.selected = switch;
        let effect = handle_command_surface_key(
            &mut overlay,
            &entries,
            &key(KeyCode::Enter, KeyModifiers::empty()),
            29,
            80,
        );
        assert!(matches!(effect, CommandSurfaceEffect::Redraw));
        assert!(overlay.digit.is_some());
        handle_command_surface_key(
            &mut overlay,
            &entries,
            &key(KeyCode::Char('1'), KeyModifiers::empty()),
            29,
            80,
        );
        handle_command_surface_key(
            &mut overlay,
            &entries,
            &key(KeyCode::Char('2'), KeyModifiers::empty()),
            29,
            80,
        );
        let effect = handle_command_surface_key(
            &mut overlay,
            &entries,
            &key(KeyCode::Enter, KeyModifiers::empty()),
            29,
            80,
        );
        assert!(matches!(
            effect,
            CommandSurfaceEffect::Run(CommandRun::Action(KeybindAction::SwitchTab(11)))
        ));

        overlay.digit = Some(ClientCommandDigitEntry {
            kind: IndexedCommand::SwitchTab,
            buffer: "4".into(),
        });
        let effect = handle_command_surface_key(
            &mut overlay,
            &entries,
            &key(KeyCode::Esc, KeyModifiers::empty()),
            29,
            80,
        );
        assert!(matches!(effect, CommandSurfaceEffect::Redraw));
        assert!(overlay.digit.is_none());
    }

    #[test]
    fn search_enter_runs_one_match_filters_many_and_escape_steps_back() {
        let entries = command_catalog(&Keybinds::default(), &[]);
        let mut overlay = ClientCommandSurfaceOverlay::new();
        handle_command_surface_key(
            &mut overlay,
            &entries,
            &key(KeyCode::Char('/'), KeyModifiers::empty()),
            29,
            80,
        );
        for character in ['d', 'e', 't', 'a', 'c', 'h'] {
            handle_command_surface_key(
                &mut overlay,
                &entries,
                &key(KeyCode::Char(character), KeyModifiers::empty()),
                29,
                80,
            );
        }
        let effect = handle_command_surface_key(
            &mut overlay,
            &entries,
            &key(KeyCode::Enter, KeyModifiers::empty()),
            29,
            80,
        );
        assert!(matches!(
            effect,
            CommandSurfaceEffect::Run(CommandRun::Action(KeybindAction::Detach))
        ));

        let mut overlay = ClientCommandSurfaceOverlay::new();
        handle_command_surface_key(
            &mut overlay,
            &entries,
            &key(KeyCode::Char('/'), KeyModifiers::empty()),
            29,
            80,
        );
        for character in ['s', 'p', 'l', 'i', 't'] {
            handle_command_surface_key(
                &mut overlay,
                &entries,
                &key(KeyCode::Char(character), KeyModifiers::empty()),
                29,
                80,
            );
        }
        let effect = handle_command_surface_key(
            &mut overlay,
            &entries,
            &key(KeyCode::Enter, KeyModifiers::empty()),
            29,
            80,
        );
        assert!(matches!(effect, CommandSurfaceEffect::Redraw));
        assert!(!overlay.search_focused);
        assert_eq!(overlay.filter, "split");
        assert!(filter_entries(&entries, &overlay.filter).len() > 1);

        let effect = handle_command_surface_key(
            &mut overlay,
            &entries,
            &key(KeyCode::Esc, KeyModifiers::empty()),
            29,
            80,
        );
        assert!(matches!(effect, CommandSurfaceEffect::Redraw));
        assert!(overlay.filter.is_empty());
        let effect = handle_command_surface_key(
            &mut overlay,
            &entries,
            &key(KeyCode::Esc, KeyModifiers::empty()),
            29,
            80,
        );
        assert!(matches!(effect, CommandSurfaceEffect::Close));
    }

    #[test]
    fn pending_g_moves_on_the_next_key_and_gg_returns_to_the_first_row() {
        let entries = command_catalog(&Keybinds::default(), &[]);
        let mut overlay = ClientCommandSurfaceOverlay::new();
        overlay.selected = 4;
        handle_command_surface_key(
            &mut overlay,
            &entries,
            &key(KeyCode::Char('g'), KeyModifiers::empty()),
            29,
            80,
        );
        assert!(overlay.pending_g);
        assert_eq!(overlay.selected, 4);
        handle_command_surface_key(
            &mut overlay,
            &entries,
            &key(KeyCode::Char('j'), KeyModifiers::empty()),
            29,
            80,
        );
        assert!(!overlay.pending_g);
        assert_eq!(overlay.selected, 5);
        handle_command_surface_key(
            &mut overlay,
            &entries,
            &key(KeyCode::Char('g'), KeyModifiers::empty()),
            29,
            80,
        );
        handle_command_surface_key(
            &mut overlay,
            &entries,
            &key(KeyCode::Char('g'), KeyModifiers::empty()),
            29,
            80,
        );
        assert_eq!(overlay.selected, 0);
        handle_command_surface_key(
            &mut overlay,
            &entries,
            &key(KeyCode::Char('G'), KeyModifiers::empty()),
            29,
            80,
        );
        assert_eq!(overlay.selected, entries.len() - 1);
        overlay.selected = 0;
        handle_command_surface_key(
            &mut overlay,
            &entries,
            &key(KeyCode::Char('G'), KeyModifiers::SHIFT),
            29,
            80,
        );
        assert_eq!(overlay.selected, entries.len() - 1);
    }
}
