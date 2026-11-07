use super::command_surface::{filter_entries, CommandEntry, CommandRun};
use super::*;

const MAX_EPISODES: usize = 64;
const MAX_MANUAL: usize = 8;

/// Workspace, tab, and pane the choice was made in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SurfaceContext {
    pub(super) workspace: String,
    pub(super) tab: String,
    pub(super) pane: String,
}

/// One command-surface row. Shortcut keeps repeated descriptions apart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ChoiceId {
    group: String,
    shortcut: String,
    description: String,
}

impl ChoiceId {
    fn from_entry(entry: &CommandEntry) -> Self {
        Self {
            group: entry.group.to_owned(),
            shortcut: entry.shortcut.clone(),
            description: entry.description.clone(),
        }
    }

    pub(super) fn matches_entry(&self, entry: &CommandEntry) -> bool {
        self.group == entry.group
            && self.shortcut == entry.shortcut
            && self.description == entry.description
    }
}

#[derive(Debug)]
struct Episode {
    context: SurfaceContext,
    previous: Option<ChoiceId>,
    manual: Vec<String>,
    choice: ChoiceId,
}

/// Session memory of command-surface choices and the keystrokes between them.
#[derive(Debug, Default)]
pub(super) struct CommandAnticipate {
    episodes: Vec<Episode>,
    previous: Option<ChoiceId>,
    manual: Vec<String>,
}

impl CommandAnticipate {
    pub(super) fn note_manual(&mut self, token: String) {
        if self.manual.len() == MAX_MANUAL {
            self.manual.remove(0);
        }
        self.manual.push(token);
    }

    pub(super) fn note_choice(&mut self, context: SurfaceContext, choice: ChoiceId) {
        self.episodes.push(Episode {
            context,
            previous: self.previous.clone(),
            manual: std::mem::take(&mut self.manual),
            choice: choice.clone(),
        });
        if self.episodes.len() > MAX_EPISODES {
            self.episodes.remove(0);
        }
        self.previous = Some(choice);
    }

    /// The row that most often followed this context, last choice, and the
    /// keystrokes since that choice. A context with its own history does not
    /// borrow another context's order.
    pub(super) fn predict(&self, context: &SurfaceContext) -> Option<ChoiceId> {
        let previous = self.previous.as_ref()?;
        if let Some(choice) = tally(&self.episodes, |episode| {
            episode.previous.as_ref() == Some(previous)
                && &episode.context == context
                && episode.manual == self.manual
        }) {
            return Some(choice);
        }
        let seen_here = self
            .episodes
            .iter()
            .any(|episode| &episode.context == context);
        if seen_here {
            return None;
        }
        tally(&self.episodes, |episode| {
            episode.previous.as_ref() == Some(previous) && episode.manual == self.manual
        })
    }
}

fn tally(episodes: &[Episode], include: impl Fn(&Episode) -> bool) -> Option<ChoiceId> {
    let mut counts: Vec<(&ChoiceId, usize, usize)> = Vec::new();
    for (index, episode) in episodes
        .iter()
        .enumerate()
        .filter(|(_, episode)| include(episode))
    {
        if let Some(found) = counts
            .iter_mut()
            .find(|(choice, _, _)| *choice == &episode.choice)
        {
            found.1 += 1;
            found.2 = index;
        } else {
            counts.push((&episode.choice, 1, index));
        }
    }
    counts
        .into_iter()
        .max_by_key(|(_, count, index)| (*count, *index))
        .map(|(choice, _, _)| choice.clone())
}

pub(super) fn selected_choice(
    overlay: &ClientCommandSurfaceOverlay,
    entries: &[CommandEntry],
) -> Option<ChoiceId> {
    let query = if overlay.search_focused {
        overlay.query.as_str()
    } else {
        overlay.filter.as_str()
    };
    let visible = filter_entries(entries, query);
    if visible.is_empty() {
        return None;
    }
    let selected = overlay.selected.min(visible.len() - 1);
    let entry = entries.get(visible[selected])?;
    if matches!(entry.run, CommandRun::None) {
        return None;
    }
    Some(ChoiceId::from_entry(entry))
}

pub(super) fn manual_token(binding: &crate::input::KeybindMatch) -> Option<String> {
    match binding {
        crate::input::KeybindMatch::Action(
            crate::input::KeybindAction::Help | crate::input::KeybindAction::CommandSurface,
        ) => None,
        crate::input::KeybindMatch::Action(action) => Some(format!("{action:?}")),
        crate::input::KeybindMatch::Command(command) => Some(format!("custom:{}", command.label)),
    }
}

impl ClientShellState {
    pub(super) fn surface_context(&self) -> SurfaceContext {
        let snapshot = self.snapshot.as_deref();
        SurfaceContext {
            workspace: snapshot
                .and_then(|snapshot| snapshot.focused_workspace_id.clone())
                .unwrap_or_default(),
            tab: snapshot
                .and_then(|snapshot| snapshot.focused_tab_id.clone())
                .unwrap_or_default(),
            pane: snapshot
                .and_then(|snapshot| snapshot.focused_pane_id.clone())
                .unwrap_or_default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context(name: &str) -> SurfaceContext {
        SurfaceContext {
            workspace: name.into(),
            tab: format!("{name}-tab"),
            pane: format!("{name}-pane"),
        }
    }

    fn choice(description: &str) -> ChoiceId {
        ChoiceId {
            group: "pane".into(),
            shortcut: description.into(),
            description: description.into(),
        }
    }

    #[test]
    fn order_and_context_pick_the_usual_next_row() {
        let mut memory = CommandAnticipate::default();
        let here = context("here");
        let elsewhere = context("elsewhere");
        memory.note_choice(here.clone(), choice("clear pane"));
        memory.note_choice(here.clone(), choice("zoom pane"));
        memory.note_choice(here.clone(), choice("clear pane"));
        assert_eq!(
            memory
                .predict(&here)
                .as_ref()
                .map(|choice| choice.description.as_str()),
            Some("zoom pane")
        );

        memory.note_choice(elsewhere.clone(), choice("clear pane"));
        memory.note_choice(elsewhere.clone(), choice("copy mode"));
        memory.note_choice(elsewhere.clone(), choice("clear pane"));
        assert_eq!(
            memory
                .predict(&elsewhere)
                .as_ref()
                .map(|choice| choice.description.as_str()),
            Some("copy mode")
        );
        assert_eq!(
            memory
                .predict(&here)
                .as_ref()
                .map(|choice| choice.description.as_str()),
            Some("zoom pane")
        );
        memory.note_choice(elsewhere, choice("rename pane"));
        assert_eq!(memory.predict(&here), None);
    }

    #[test]
    fn an_unseen_context_reuses_the_same_order_and_manual_keys() {
        let mut memory = CommandAnticipate::default();
        let here = context("here");
        memory.note_choice(here.clone(), choice("clear pane"));
        memory.note_manual("Zoom".into());
        memory.note_choice(here, choice("toggle sidebar"));
        assert_eq!(
            memory
                .predict(&context("new"))
                .map(|choice| choice.description),
            None
        );

        let mut memory = CommandAnticipate::default();
        let here = context("here");
        memory.note_choice(here.clone(), choice("clear pane"));
        memory.note_manual("Zoom".into());
        memory.note_choice(here.clone(), choice("toggle sidebar"));
        memory.note_choice(here, choice("clear pane"));
        memory.note_manual("Zoom".into());
        assert_eq!(
            memory
                .predict(&context("new"))
                .as_ref()
                .map(|choice| choice.description.as_str()),
            Some("toggle sidebar")
        );
        memory.manual.clear();
        assert_eq!(memory.predict(&context("new")), None);
    }
}
