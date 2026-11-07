use super::*;
use crossterm::event::{KeyModifiers, MouseButton, MouseEventKind};
use ratatui::layout::Rect;
use ratatui::style::Modifier;

fn prepared() -> ClientShellState {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    state.set_pane_surface(surface());
    state.set_endpoint_status(&ClientEndpointId::Local, ClientEndpointStatus::Online);
    state
}

fn open_default(state: &mut ClientShellState) {
    state.handle_input_bytes(&[0x02]);
    state.handle_input_bytes(b"/");
}

fn command_overlay(state: &ClientShellState) -> &ClientCommandSurfaceOverlay {
    match &state.overlay {
        Some(ClientShellOverlay::CommandSurface(overlay)) => overlay,
        _ => panic!("command surface should be open"),
    }
}

fn body_row(area: Rect) -> u16 {
    area.y + 4
}

fn row_background(frame: &FrameData, x: u16, y: u16) -> ratatui::style::Color {
    let buffer = frame.to_ratatui_buffer().expect("ratatui buffer");
    buffer[(x, y)].bg
}

fn right_click(state: &mut ClientShellState, column: u16, row: u16) {
    state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Right),
        column,
        row,
        modifiers: KeyModifiers::empty(),
    })]);
}

fn left_click(state: &mut ClientShellState, column: u16, row: u16) -> ClientShellInput {
    state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::empty(),
    })])
}

fn menu_items(state: &ClientShellState) -> Vec<ClientContextMenuItem> {
    match state.overlay.as_ref() {
        Some(ClientShellOverlay::ContextMenu(menu)) => menu.items(),
        _ => panic!("context menu should be open"),
    }
}

#[test]
fn prefix_slash_opens_the_surface_and_question_mark_keeps_help() {
    let mut state = prepared();
    open_default(&mut state);
    assert!(matches!(
        state.overlay,
        Some(ClientShellOverlay::CommandSurface(_))
    ));
    assert!(!state.handle_input_bytes(b"\x1b").detach);
    assert!(state.overlay.is_none());

    state.handle_input_bytes(&[0x02]);
    state.handle_input_bytes(b"?");
    assert!(matches!(state.overlay, Some(ClientShellOverlay::Help(_))));
}

#[test]
fn configured_ctrl_a_prefix_opens_the_surface() {
    let config: Config = toml::from_str("[keys]\nprefix = \"ctrl+a\"\n").unwrap();
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&config));
    state.set_snapshot(Box::new(snapshot()));
    state.set_pane_surface(surface());
    state.handle_input_bytes(&[0x01]);
    state.handle_input_bytes(b"/");
    assert!(matches!(
        state.overlay,
        Some(ClientShellOverlay::CommandSurface(_))
    ));

    let mut classic = ClientShellState::new(ClientShellConfig::from_config(&config));
    classic.set_snapshot(Box::new(snapshot()));
    classic.set_pane_surface(surface());
    classic.handle_input_bytes(&[0x02]);
    classic.handle_input_bytes(b"?");
    assert!(classic.overlay.is_none());
}

#[test]
fn replace_help_is_off_until_enabled() {
    let mut state = prepared();
    assert!(!state.config.command_surface_replaces_help);
    state.handle_input_bytes(&[0x02]);
    state.handle_input_bytes(b"?");
    assert!(matches!(state.overlay, Some(ClientShellOverlay::Help(_))));

    state.overlay = None;
    state.config.command_surface_replaces_help = true;
    state.handle_input_bytes(&[0x02]);
    state.handle_input_bytes(b"?");
    assert!(matches!(
        state.overlay,
        Some(ClientShellOverlay::CommandSurface(_))
    ));
}

#[test]
fn commands_is_the_last_context_menu_item_until_the_flag_is_off() {
    let mut state = prepared();
    state.compose(120, 40).expect("frame");
    let targets = [
        state.hits.workspaces[0].rect,
        state.hits.tabs[0].0,
        state.hits.panes[0].rect,
    ];
    for target in targets {
        state.overlay = None;
        state.compose(120, 40).expect("frame");
        right_click(&mut state, target.x + 1, target.y);
        state.compose(120, 40).expect("menu");
        let items = menu_items(&state);
        let last = items.last().expect("menu item");
        assert_eq!(last.label, "Commands");
        assert_eq!(last.action, ClientContextMenuAction::Commands);
        let row = state.hits.context_menu_rows[items.len() - 1].0;
        left_click(&mut state, row.x + 1, row.y);
        assert!(matches!(
            state.overlay,
            Some(ClientShellOverlay::CommandSurface(_))
        ));
    }

    state.config.command_surface_context_menu = false;
    state.overlay = None;
    state.compose(120, 40).expect("frame");
    let workspace = state.hits.workspaces[0].rect;
    right_click(&mut state, workspace.x + 1, workspace.y);
    let items = menu_items(&state);
    assert!(items
        .iter()
        .all(|item| item.action != ClientContextMenuAction::Commands));
    assert!(items.len() > 1);
}

#[test]
fn surface_uses_two_cells_beside_the_sidebar_and_help_styling() {
    let mut state = prepared();
    open_default(&mut state);
    let frame = state.compose(120, 40).expect("command surface");
    let sidebar = state.layout(120, 40).sidebar.width;
    let area = state.hits.help_popup;
    assert_eq!(area, Rect::new(sidebar + 2, 2, 120 - sidebar - 4, 36));
    assert!(area.width > 76 && area.height > 22);
    assert_eq!(area.right(), 118);
    assert_eq!(area.bottom(), 38);
    let corner = (area.y as usize) * (frame.width as usize) + area.x as usize;
    assert_eq!(frame.cells[corner].symbol, "┌");

    let buffer = frame.to_ratatui_buffer().expect("buffer");
    let palette = &state.config.palette;
    let title = &buffer[(area.x + 2, area.y + 1)];
    assert_eq!(title.symbol(), "p");
    assert_eq!(title.fg, palette.accent);
    assert!(title.modifier.contains(Modifier::BOLD));

    let row = body_row(area);
    let text = super::frame_rows(&frame)[row as usize].clone();
    assert!(text.contains("split vertical"), "{text}");
    assert_eq!(buffer[(area.x + 1, row)].bg, palette.surface0);
    let (description_x, description_y) =
        super::cell_symbol_position(&frame, area, "split vertical");
    let description = &buffer[(description_x, description_y)];
    assert_eq!(description.fg, palette.text);
    let shortcut = &buffer[(description_x - 1, description_y)];
    assert_eq!(shortcut.fg, palette.mauve);
    assert!(shortcut.modifier.contains(Modifier::BOLD));

    let footer_y = area.bottom() - 2;
    let footer = super::frame_rows(&frame)[footer_y as usize].clone();
    assert!(footer.contains("search /"), "{footer}");
    assert!(footer.contains("close esc"), "{footer}");
    let (footer_x, footer_row) = super::cell_symbol_position(&frame, area, "close esc");
    assert_eq!(buffer[(footer_x, footer_row)].fg, palette.overlay0);
}

#[test]
fn highlight_stays_pinned_until_the_last_rows() {
    let mut state = prepared();
    open_default(&mut state);
    let frame = state.compose(120, 40).expect("surface");
    let area = state.hits.help_popup;
    let row = body_row(area);
    assert_eq!(command_overlay(&state).selected, 0);
    assert_eq!(
        row_background(&frame, area.x + 1, row),
        state.config.palette.surface0
    );

    state.handle_input_bytes(b"j");
    let frame = state.compose(120, 40).expect("moved");
    assert_eq!(command_overlay(&state).selected, 1);
    assert_eq!(
        row_background(&frame, area.x + 1, row),
        state.config.palette.surface0
    );
    assert_ne!(
        row_background(&frame, area.x + 1, row + 1),
        state.config.palette.surface0
    );

    let before = command_overlay(&state).selected;
    state.handle_input_bytes(b"\x1b[6~");
    assert!(command_overlay(&state).selected > before + 1);
    let frame = state.compose(120, 40).expect("page");
    assert_eq!(
        row_background(&frame, area.x + 1, row),
        state.config.palette.surface0
    );

    state.handle_input_bytes(b"G");
    let frame = state.compose(120, 40).expect("end");
    assert!(command_overlay(&state).selected > 1);
    assert_ne!(
        row_background(&frame, area.x + 1, row),
        state.config.palette.surface0
    );
    let highlighted = (row..area.bottom().saturating_sub(2))
        .any(|y| row_background(&frame, area.x + 1, y) == state.config.palette.surface0);
    assert!(highlighted);

    state.handle_input_bytes(b"gg");
    let frame = state.compose(120, 40).expect("top");
    assert_eq!(command_overlay(&state).selected, 0);
    assert_eq!(
        row_background(&frame, area.x + 1, row),
        state.config.palette.surface0
    );
}

#[test]
fn vim_and_emacs_keys_move_by_line_and_page() {
    let mut state = prepared();
    open_default(&mut state);
    state.compose(120, 40).expect("surface");
    state.handle_input_bytes(&[0x0e]);
    assert_eq!(command_overlay(&state).selected, 1);
    state.handle_input_bytes(&[0x10]);
    assert_eq!(command_overlay(&state).selected, 0);
    state.handle_input_bytes(b"j");
    state.handle_input_bytes(b"k");
    assert_eq!(command_overlay(&state).selected, 0);

    state.handle_input_bytes(&[0x06]);
    let paged = command_overlay(&state).selected;
    assert!(paged > 1);
    state.handle_input_bytes(&[0x02]);
    assert!(command_overlay(&state).selected < paged);
    assert!(matches!(
        state.overlay,
        Some(ClientShellOverlay::CommandSurface(_))
    ));

    state.handle_input_bytes(b"\x1b[6~");
    let down = command_overlay(&state).selected;
    state.handle_input_bytes(b"\x1b[5~");
    assert!(command_overlay(&state).selected < down);
    state.handle_input_bytes(b"\x1b[F");
    let end = command_overlay(&state).selected;
    assert!(end > 1);
    state.handle_input_bytes(b"\x1b[H");
    assert_eq!(command_overlay(&state).selected, 0);

    state.handle_raw_events(vec![RawInputEvent::Key(crate::input::TerminalKey::new(
        crossterm::event::KeyCode::Char('v'),
        KeyModifiers::ALT,
    ))]);
    assert_eq!(command_overlay(&state).selected, 0);
    state.handle_input_bytes(&[0x16]);
    assert!(command_overlay(&state).selected > 1);
}

#[test]
fn search_runs_one_match_filters_many_and_escape_steps_back() {
    let mut state = prepared();
    open_default(&mut state);
    state.handle_input_bytes(b"/rename pane\r");
    assert!(matches!(
        state.overlay,
        Some(ClientShellOverlay::Rename(ClientRenameOverlay {
            title: "rename pane",
            ..
        }))
    ));

    let mut state = prepared();
    open_default(&mut state);
    state.handle_input_bytes(b"/new tab\r");
    assert!(matches!(
        state.overlay,
        Some(ClientShellOverlay::Rename(ClientRenameOverlay {
            title: "new tab",
            ..
        }))
    ));

    let mut state = prepared();
    open_default(&mut state);
    state.handle_input_bytes(b"/close pane\r");
    assert!(matches!(
        state.overlay,
        Some(ClientShellOverlay::ConfirmClose(_))
    ));

    let mut state = prepared();
    open_default(&mut state);
    state.handle_input_bytes(b"/previous workspace\r");
    assert!(matches!(
        state.overlay,
        Some(ClientShellOverlay::CommandSurface(_))
    ));
    assert!(!command_overlay(&state).search_focused);

    let mut state = prepared();
    open_default(&mut state);
    let detach = state.handle_input_bytes(b"/detach\r");
    assert!(detach.detach);
    assert!(state.overlay.is_none());

    let mut state = prepared();
    open_default(&mut state);
    state.handle_input_bytes(b"/keybinds\r");
    assert!(matches!(state.overlay, Some(ClientShellOverlay::Help(_))));

    let mut state = prepared();
    open_default(&mut state);
    state.set_endpoint_status(&ClientEndpointId::Local, ClientEndpointStatus::Online);
    let filtered = state.handle_input_bytes(b"/split\r");
    assert!(filtered.actions.is_empty());
    assert_eq!(command_overlay(&state).filter, "split");
    assert!(!command_overlay(&state).search_focused);
    let ran = state.handle_input_bytes(b"\r");
    assert!(ran.actions.iter().any(|action| matches!(
        action,
        ClientShellAction::Endpoint { request, .. }
            if matches!(request.method, crate::api::schema::Method::PaneSplit(_))
    )));

    let mut state = prepared();
    open_default(&mut state);
    state.handle_input_bytes(b"/split\r");
    state.handle_input_bytes(b"\x1b");
    assert!(command_overlay(&state).filter.is_empty());
    assert!(matches!(
        state.overlay,
        Some(ClientShellOverlay::CommandSurface(_))
    ));
    let closed = state.handle_input_bytes(b"\x1b");
    assert!(!closed.detach);
    assert!(state.overlay.is_none());

    let mut state = prepared();
    open_default(&mut state);
    state.handle_input_bytes(b"/not-a-command");
    state.handle_input_bytes(b"\x1b");
    assert!(command_overlay(&state).filter.is_empty());
    assert!(!command_overlay(&state).search_focused);
    assert!(state.overlay.is_some());
}

#[test]
fn digit_dialog_accepts_one_or_two_digits_and_escape_returns() {
    let mut state = prepared();
    open_default(&mut state);
    state.compose(120, 40).expect("surface");
    state.handle_input_bytes(b"/switch tab\r");
    assert!(command_overlay(&state).digit.is_some());
    assert!(!command_overlay(&state).search_focused);
    let frame = state.compose(120, 40).expect("digit dialog");
    let text = super::frame_rows(&frame).join("\n");
    assert!(text.contains("type an index from 1 to 99"), "{text}");
    state.handle_input_bytes(b"\x1b");
    assert!(command_overlay(&state).digit.is_none());
    assert!(matches!(
        state.overlay,
        Some(ClientShellOverlay::CommandSurface(_))
    ));

    state.handle_input_bytes(b"/switch tab\r");
    state.handle_input_bytes(b"1");
    assert!(command_overlay(&state).digit.is_some());
    let focused = state.handle_input_bytes(b"\r");
    assert!(state.overlay.is_none());
    assert!(focused.actions.iter().any(|action| matches!(
        action,
        ClientShellAction::Endpoint { request, .. }
            if matches!(request.method, crate::api::schema::Method::TabFocus(_))
    )));

    let mut state = prepared();
    open_default(&mut state);
    state.handle_input_bytes(b"/switch tab\r");
    let switched = state.handle_input_bytes(b"12\r");
    assert!(switched.actions.iter().any(|action| matches!(
        action,
        ClientShellAction::Keybind(crate::input::KeybindAction::SwitchTab(11))
    )));
}

#[test]
fn custom_command_search_invokes_the_snapshot_command() {
    let mut state = prepared();
    let mut projection = snapshot();
    projection
        .commands
        .push(crate::protocol::ClientShellCommand {
            command_id: "plugin-action".into(),
            binding_label: "prefix+z".into(),
            binding_labels: vec!["prefix+z".into()],
            action: crate::protocol::ClientShellCommandAction::PluginAction,
            description: Some("run plugin action".into()),
        });
    state.set_snapshot(Box::new(projection));
    state.set_endpoint_status(&ClientEndpointId::Local, ClientEndpointStatus::Online);
    open_default(&mut state);
    let invoked = state.handle_input_bytes(b"/run plugin action\r");
    assert!(invoked.actions.iter().any(|action| matches!(
        action,
        ClientShellAction::Endpoint { request, .. }
            if matches!(
                &request.method,
                crate::api::schema::Method::CommandInvoke(params)
                    if params.command_id == "plugin-action"
            )
    )));
}

#[test]
fn mouse_outside_closes_and_cancel_steps_back_one_level() {
    let mut state = prepared();
    open_default(&mut state);
    state.compose(120, 40).expect("surface");
    left_click(&mut state, 0, 0);
    assert!(state.overlay.is_none());

    open_default(&mut state);
    state.handle_input_bytes(b"/split\r");
    state.compose(120, 40).expect("filtered");
    let cancel = state.hits.overlay_cancel;
    left_click(&mut state, cancel.x + 1, cancel.y);
    assert!(command_overlay(&state).filter.is_empty());
    assert!(matches!(
        state.overlay,
        Some(ClientShellOverlay::CommandSurface(_))
    ));
    let cancel = state.hits.overlay_cancel;
    left_click(&mut state, cancel.x + 1, cancel.y);
    assert!(state.overlay.is_none());
}
