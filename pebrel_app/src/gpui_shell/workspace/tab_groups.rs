//! Profile-owned group definitions and the window-local sidebar projection.

use super::*;
use crate::i18n::Message;
use gpui::AnyElement;
use gpui_component::menu::PopupMenuItem;
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, io, path::PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Group {
    id: u64,
    name: String,
}

#[derive(Clone, Default)]
struct ProfileGroups {
    groups: Arc<Vec<Group>>,
    load_failed: bool,
}

impl gpui::Global for ProfileGroups {}

fn groups_path() -> PathBuf {
    pebrel_settings::settings_dir().join("tab-groups.json")
}

impl ProfileGroups {
    fn load() -> Self {
        let groups: io::Result<Vec<Group>> = std::fs::read(groups_path())
            .and_then(|bytes| serde_json::from_slice(&bytes).map_err(io::Error::other));
        match groups {
            Ok(groups) => Self { groups: Arc::new(groups), load_failed: false },
            Err(err) if err.kind() == io::ErrorKind::NotFound => Self::default(),
            Err(_) => Self { load_failed: true, ..Self::default() },
        }
    }
}

pub(super) struct TabGroups {
    profile: ProfileGroups,
    collapsed: HashSet<u64>,
    _subscription: Subscription,
}

impl TabGroups {
    pub(super) fn new(cx: &mut Context<NebulaWorkspace>) -> Self {
        if cx.try_global::<ProfileGroups>().is_none() {
            cx.set_global(ProfileGroups::load());
        }
        let profile = cx.global::<ProfileGroups>().clone();
        let subscription = cx.observe_global::<ProfileGroups>(|this, cx| {
            this.tab_groups.profile = cx.global::<ProfileGroups>().clone();
            this.tab_groups
                .collapsed
                .retain(|id| this.tab_groups.profile.groups.iter().any(|group| group.id == *id));
            this.clamp_tabs_scroll();
            cx.notify();
        });
        Self { profile, collapsed: HashSet::new(), _subscription: subscription }
    }

    pub(super) fn resolve(&self, id: Option<u64>) -> Option<u64> {
        id.filter(|id| self.profile.groups.iter().any(|group| group.id == *id))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SidebarRow {
    Group(usize),
    Ungrouped,
    Tab(usize),
}

fn project_rows(
    groups: &[Group],
    collapsed: &HashSet<u64>,
    membership: impl IntoIterator<Item = Option<u64>>,
) -> Vec<SidebarRow> {
    let mut ungrouped = Vec::new();
    let mut members = vec![Vec::new(); groups.len()];
    for (ix, id) in membership.into_iter().enumerate() {
        if let Some(group_ix) = groups.iter().position(|group| Some(group.id) == id) {
            if !collapsed.contains(&groups[group_ix].id) {
                members[group_ix].push(SidebarRow::Tab(ix));
            }
        } else {
            ungrouped.push(SidebarRow::Tab(ix));
        }
    }
    let mut rows = Vec::new();
    for (ix, members) in members.into_iter().enumerate() {
        rows.push(SidebarRow::Group(ix));
        rows.extend(members);
    }
    if !groups.is_empty() && !ungrouped.is_empty() {
        rows.push(SidebarRow::Ungrouped);
    }
    rows.extend(ungrouped);
    rows
}

#[derive(Clone, Copy)]
enum GroupEdit {
    Create,
    Rename(u64),
    Move(u64, isize),
    Delete(u64),
}

impl NebulaWorkspace {
    pub(super) fn sidebar_rows(&self) -> Vec<SidebarRow> {
        project_rows(
            &self.tab_groups.profile.groups,
            &self.tab_groups.collapsed,
            self.tab_meta.iter().map(|meta| meta.group_id),
        )
    }

    pub(super) fn sidebar_row_count(&self) -> usize {
        let groups = &self.tab_groups;
        if groups.profile.groups.is_empty() {
            return self.tabs.len();
        }
        let mut has_ungrouped = false;
        let mut hidden = 0;
        for meta in &self.tab_meta {
            match groups.resolve(meta.group_id) {
                Some(id) if groups.collapsed.contains(&id) => hidden += 1,
                None => has_ungrouped = true,
                _ => {},
            }
        }
        self.tabs.len() + groups.profile.groups.len() + usize::from(has_ungrouped) - hidden
    }

    pub(super) fn sidebar_tab_order(&self) -> Vec<usize> {
        if self.tab_groups.profile.groups.is_empty() {
            return (0..self.tabs.len()).collect();
        }
        project_rows(
            &self.tab_groups.profile.groups,
            &HashSet::new(),
            self.tab_meta.iter().map(|meta| meta.group_id),
        )
        .into_iter()
        .filter_map(|row| match row {
            SidebarRow::Tab(ix) => Some(ix),
            _ => None,
        })
        .collect()
    }

    pub(super) fn reveal_sidebar_tab(&mut self) -> usize {
        if self.tab_groups.profile.groups.is_empty() {
            return self.active;
        }
        if let Some(id) = self.tab_meta.get(self.active).and_then(|meta| meta.group_id) {
            self.tab_groups.collapsed.remove(&id);
        }
        self.sidebar_rows().iter().position(|row| *row == SidebarRow::Tab(self.active)).unwrap_or(0)
    }

    pub(super) fn tab_group_at(&self, ix: usize) -> Option<u64> {
        self.tab_groups.resolve(self.tab_meta.get(ix).and_then(|meta| meta.group_id))
    }

    pub(super) fn sidebar_drag_peers(&self, source: usize) -> impl Iterator<Item = usize> + '_ {
        let group = self.tab_group_at(source);
        self.tab_meta.iter().enumerate().filter_map(move |(ix, meta)| {
            (self.tab_groups.resolve(meta.group_id) == group).then_some(ix)
        })
    }

    fn set_tab_group(&mut self, ix: usize, id: Option<u64>, cx: &mut Context<Self>) {
        let Some(meta) = self.tab_meta.get_mut(ix) else { return };
        meta.group_id = id;
        self.reveal_active_tab();
        self.clamp_tabs_scroll();
        cx.notify();
    }

    pub(super) fn tab_group_submenu(
        &self,
        menu: PopupMenu,
        ix: usize,
        window: &mut Window,
        cx: &mut Context<PopupMenu>,
        workspace: gpui::WeakEntity<Self>,
    ) -> PopupMenu {
        let groups = self.tab_groups.profile.groups.clone();
        if groups.is_empty()
            || !self.tabs[ix].is_terminal()
            || self.tabs_position != pebrel_settings::TabsPositionName::Sidebar
        {
            return menu;
        }
        let language = crate::gpui_shell::config::ui_language(cx);
        let meta = &self.tab_meta[ix];
        let current = self.tab_groups.resolve(meta.group_id);
        let tab_id = meta.runtime_id.0.clone();
        menu.submenu(language.text(Message::TabGroupsAssign), window, cx, move |menu, _, _| {
            let choices = std::iter::once((None, language.text(Message::TabGroupsUngrouped)))
                .chain(groups.iter().map(|group| (Some(group.id), group.name.as_str())));
            choices.fold(menu, |menu, (id, name)| {
                let workspace = workspace.clone();
                let tab_id = tab_id.clone();
                menu.item(PopupMenuItem::new(name.to_owned()).disabled(id == current).on_click(
                    move |_, _, cx| {
                        let _ = workspace.update(cx, |this, cx| {
                            if this.tab_groups.resolve(id) == id
                                && let Some(ix) = this
                                    .tab_meta
                                    .iter()
                                    .position(|meta| meta.runtime_id.0 == tab_id)
                            {
                                this.set_tab_group(ix, id, cx);
                            }
                        });
                    },
                ))
            })
        })
        .separator()
    }

    pub(super) fn render_tab_group_header(
        &self,
        ix: usize,
        width: f32,
        height: f32,
        cx: &Context<Self>,
    ) -> AnyElement {
        let group = &self.tab_groups.profile.groups[ix];
        let id = group.id;
        let collapsed = self.tab_groups.collapsed.contains(&id);
        let active = collapsed
            && self.tab_meta.get(self.active).is_some_and(|meta| meta.group_id == Some(id));
        let count = self.tab_meta.iter().filter(|meta| meta.group_id == Some(id)).count();
        let language = crate::gpui_shell::config::ui_language(cx);
        let workspace = cx.entity().downgrade();
        let last = ix + 1 == self.tab_groups.profile.groups.len();
        h_flex()
            .id(("tab-group", ix))
            .w(px(width))
            .h(px(height))
            .items_center()
            .gap(px(2.0))
            .child(
                Button::new(("tab-group-toggle", ix))
                    .ghost()
                    .h_full()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .selected(active)
                    .tooltip(group.name.clone())
                    .child(
                        h_flex()
                            .w_full()
                            .min_w_0()
                            .gap_1()
                            .child(
                                Icon::new(if collapsed {
                                    IconName::ChevronRight
                                } else {
                                    IconName::ChevronDown
                                })
                                .xsmall(),
                            )
                            .child(div().flex_1().min_w_0().truncate().child(group.name.clone()))
                            .child(div().flex_shrink_0().text_xs().child(count.to_string())),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !this.tab_groups.collapsed.remove(&id) {
                            this.tab_groups.collapsed.insert(id);
                        }
                        this.clamp_tabs_scroll();
                        cx.notify();
                    })),
            )
            .child(
                Button::new(("tab-group-new-tab", ix))
                    .ghost()
                    .compact()
                    .icon(IconName::Plus)
                    .tooltip(language.text(Message::TabGroupsNewTab))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        let previous_len = this.tabs.len();
                        this.add_terminal(window, cx);
                        if this.tabs.len() > previous_len {
                            this.set_tab_group(this.active, Some(id), cx);
                        }
                    })),
            )
            .child(
                Button::new(("tab-group-menu", ix))
                    .ghost()
                    .compact()
                    .icon(IconName::EllipsisVertical)
                    .tooltip(language.text(Message::TabGroupsActions))
                    .dropdown_menu(move |menu, _, _| {
                        [
                            (Message::TabGroupsRename, GroupEdit::Rename(id), false),
                            (Message::TabGroupsMoveUp, GroupEdit::Move(id, -1), ix == 0),
                            (Message::TabGroupsMoveDown, GroupEdit::Move(id, 1), last),
                            (Message::TabGroupsDelete, GroupEdit::Delete(id), false),
                        ]
                        .into_iter()
                        .fold(menu, |menu, (label, edit, disabled)| {
                            let workspace = workspace.clone();
                            menu.item(
                                PopupMenuItem::new(language.text(label))
                                    .disabled(disabled)
                                    .on_click(move |_, window, cx| {
                                        let _ = workspace.update(cx, |this, cx| match edit {
                                            GroupEdit::Rename(_) => {
                                                this.edit_tab_group_name(edit, window, cx)
                                            },
                                            _ => {
                                                this.apply_tab_group_edit(edit, "", window, cx);
                                            },
                                        });
                                    }),
                            )
                        })
                    }),
            )
            .into_any_element()
    }

    pub(super) fn create_tab_group(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.edit_tab_group_name(GroupEdit::Create, window, cx);
    }

    fn edit_tab_group_name(
        &mut self,
        edit: GroupEdit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let initial = match edit {
            GroupEdit::Rename(id) => {
                let Some(group) =
                    self.tab_groups.profile.groups.iter().find(|group| group.id == id)
                else {
                    return;
                };
                group.name.clone()
            },
            _ => String::new(),
        };
        let language = crate::gpui_shell::config::ui_language(cx);
        let title = language.text(if matches!(edit, GroupEdit::Create) {
            Message::TabGroupsCreate
        } else {
            Message::TabGroupsRename
        });
        let input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(language.text(Message::TabGroupsName))
        });
        input.update(cx, |state, cx| state.set_value(initial, window, cx));
        let dialog_input = input.clone();
        let workspace = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, window, _| {
            let input = dialog_input.clone();
            let workspace = workspace.clone();
            center_modal_dialog(dialog, window, 220.0)
                .title(title)
                .child(Input::new(&dialog_input).w_full().accessibility_id("tab-group-name"))
                .footer(
                    DialogFooter::new()
                        .child(div().flex_1())
                        .child(
                            DialogClose::new().child(
                                Button::new("tab-group-cancel")
                                    .label(language.text(Message::CommonCancel)),
                            ),
                        )
                        .child(
                            DialogAction::new().child(
                                Button::new("tab-group-save")
                                    .primary()
                                    .label(language.text(Message::CommonSave)),
                            ),
                        ),
                )
                .on_ok(move |_, window, cx| {
                    let name = input.read(cx).value().to_string();
                    workspace
                        .update(cx, |this, cx| this.apply_tab_group_edit(edit, &name, window, cx))
                        .unwrap_or(true)
                })
        });
        input.update(cx, |state, cx| state.focus(window, cx));
    }

    fn apply_tab_group_edit(
        &mut self,
        edit: GroupEdit,
        name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let profile = cx.global::<ProfileGroups>();
        let mut groups = (*profile.groups).clone();
        let name = name.trim();
        let invalid_name = matches!(edit, GroupEdit::Create | GroupEdit::Rename(_))
            && (name.is_empty() || name.chars().count() > 80 || name.chars().any(char::is_control));
        let result = if invalid_name {
            Err(Message::TabGroupsInvalidName)
        } else if profile.load_failed {
            Err(Message::TabGroupsSaveFailed)
        } else {
            update_groups(&mut groups, edit, name)
                .and_then(|()| {
                    let bytes = serde_json::to_vec_pretty(&groups).map_err(io::Error::other)?;
                    crate::atomic_file::write(&groups_path(), &bytes)
                })
                .map_err(|_| Message::TabGroupsSaveFailed)
        };
        match result {
            Ok(()) => {
                let profile = ProfileGroups { groups: Arc::new(groups), load_failed: false };
                self.tab_groups.profile = profile.clone();
                cx.set_global(profile);
                self.clamp_tabs_scroll();
                cx.notify();
                true
            },
            Err(message) => {
                let text = crate::gpui_shell::config::ui_language(cx).text(message);
                crate::gpui_shell::toast::toast(
                    window,
                    cx,
                    crate::display::ToastKind::Warning,
                    text,
                );
                false
            },
        }
    }
}

fn update_groups(groups: &mut Vec<Group>, edit: GroupEdit, name: &str) -> io::Result<()> {
    let id = match edit {
        GroupEdit::Create => {
            let mut bytes = [0; 8];
            getrandom::fill(&mut bytes).map_err(|err| io::Error::other(err.to_string()))?;
            let id = u64::from_le_bytes(bytes);
            if groups.iter().any(|group| group.id == id) {
                return Err(io::Error::other("duplicate group id"));
            }
            groups.push(Group { id, name: name.to_owned() });
            return Ok(());
        },
        GroupEdit::Rename(id) | GroupEdit::Move(id, _) | GroupEdit::Delete(id) => id,
    };
    let Some(ix) = groups.iter().position(|group| group.id == id) else { return Ok(()) };
    match edit {
        GroupEdit::Rename(_) => groups[ix].name = name.to_owned(),
        GroupEdit::Move(_, delta) => {
            if let Some(to) = ix.checked_add_signed(delta).filter(|&to| to < groups.len()) {
                groups.swap(ix, to);
            }
        },
        GroupEdit::Delete(_) => {
            groups.remove(ix);
        },
        GroupEdit::Create => unreachable!(),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projected_rows_include_empty_groups_and_keep_orphans_visible() {
        let mut groups = vec![Group { id: 1, name: "A".into() }, Group { id: 2, name: "B".into() }];
        let membership = [None, Some(1), Some(99), Some(1)];
        use SidebarRow::{Group as Header, Tab, Ungrouped};
        assert_eq!(
            project_rows(&groups, &HashSet::new(), membership),
            [Header(0), Tab(1), Tab(3), Header(1), Ungrouped, Tab(0), Tab(2)]
        );
        assert_eq!(
            project_rows(&groups, &HashSet::from([1]), membership),
            [Header(0), Header(1), Ungrouped, Tab(0), Tab(2)]
        );
        update_groups(&mut groups, GroupEdit::Delete(1), "").unwrap();
        assert_eq!(
            project_rows(&groups, &HashSet::new(), membership),
            [Header(0), Ungrouped, Tab(0), Tab(1), Tab(2), Tab(3)]
        );
    }
}
