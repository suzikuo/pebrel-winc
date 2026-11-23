//! Profile-owned group definitions and the window-local sidebar projection.

use super::*;
use crate::i18n::Message;
use gpui::AnyElement;
use gpui_component::menu::PopupMenuItem;
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, io, path::PathBuf};

pub(super) const GROUP_GUIDE_X: f32 = 13.0;
const GROUP_CORNER_RADIUS: f32 = 12.0;
const GROUP_TRAILING_INSET: f32 = 10.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum GroupColor {
    Blue,
    Purple,
    Teal,
    Orange,
    Pink,
    Green,
    #[default]
    #[serde(other)]
    Auto,
}

impl GroupColor {
    const CHOICES: [(Self, Message); 7] = [
        (Self::Auto, Message::TabGroupsColorAuto),
        (Self::Blue, Message::TabGroupsColorBlue),
        (Self::Purple, Message::TabGroupsColorPurple),
        (Self::Teal, Message::TabGroupsColorTeal),
        (Self::Orange, Message::TabGroupsColorOrange),
        (Self::Pink, Message::TabGroupsColorPink),
        (Self::Green, Message::TabGroupsColorGreen),
    ];

    fn is_auto(&self) -> bool {
        *self == Self::Auto
    }

    fn accent(self, index: usize, dark: bool) -> gpui::Hsla {
        let hue = match self {
            Self::Auto => [215.0, 276.0, 168.0, 32.0, 335.0, 95.0][index % 6],
            Self::Blue => 215.0,
            Self::Purple => 276.0,
            Self::Teal => 168.0,
            Self::Orange => 32.0,
            Self::Pink => 335.0,
            Self::Green => 95.0,
        };
        gpui::hsla(hue / 360.0, 0.61, if dark { 0.68 } else { 0.36 }, 1.0)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Group {
    id: u64,
    name: String,
    #[serde(default, skip_serializing_if = "GroupColor::is_auto")]
    color: GroupColor,
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

    pub(super) fn accent(&self, index: usize, dark: bool) -> gpui::Hsla {
        self.profile.groups[index].color.accent(index, dark)
    }

    pub(super) fn plugin_group_snapshot(&self) -> Vec<serde_json::Value> {
        self.profile.groups.iter().map(|group| {
            serde_json::json!({"id": group.id.to_string(), "name": group.name})
        }).collect()
    }

    pub(super) fn group_id(&self, index: usize) -> u64 {
        self.profile.groups[index].id
    }

    pub(super) fn resolve(&self, id: Option<u64>) -> Option<u64> {
        id.filter(|id| self.profile.groups.iter().any(|group| group.id == *id))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SidebarRow {
    Group(usize),
    Ungrouped,
    Tab(usize, Option<usize>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct SidebarGroupDrop(pub(super) Option<u64>);

impl SidebarRow {
    fn group_index(self) -> Option<usize> {
        match self {
            Self::Group(index) | Self::Tab(_, Some(index)) => Some(index),
            _ => None,
        }
    }
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
                members[group_ix].push(SidebarRow::Tab(ix, Some(group_ix)));
            }
        } else {
            ungrouped.push(SidebarRow::Tab(ix, None));
        }
    }
    let mut rows = Vec::new();
    for (ix, members) in members.into_iter().enumerate() {
        rows.push(SidebarRow::Group(ix));
        rows.extend(members);
    }
    if !groups.is_empty() {
        rows.push(SidebarRow::Ungrouped);
    }
    rows.extend(ungrouped);
    rows
}

#[derive(Clone, Copy)]
enum GroupEdit {
    Create,
    Rename(u64),
    Color(u64, GroupColor),
    Move(u64, isize),
    Delete(u64),
}

impl NebulaWorkspace {
    pub(super) fn sidebar_group_drop_at(
        &self,
        source: usize,
        x: f32,
        y: f32,
    ) -> Option<SidebarGroupDrop> {
        if self.tabs_section_collapsed
            || self.tabs_fold_frozen
            || self.tab_groups.profile.groups.is_empty()
            || !self.tabs.get(source).is_some_and(WorkspaceTab::is_terminal)
        {
            return None;
        }
        let x = x - f32::from(self.tabs_list_origin.x);
        let y = y - f32::from(self.tabs_list_origin.y);
        let row_width =
            (self.sidebar_width - 16.0).min(self.tabs_list_width) - tab_scroll::TAB_SCROLL_GUTTER;
        if x < 0.0 || x >= row_width || y >= self.tabs_viewport_h {
            return None;
        }
        let rows = self.sidebar_rows();
        let layout = self.sidebar_row_layout_for(&rows);
        let (scroll, visible) = self.tabs_visible_window(&layout);
        let row = rows[layout.row_at(y, scroll, visible)?];
        let destination = row.group_index().map(|index| self.tab_groups.group_id(index));
        (destination != self.tab_group_at(source)).then_some(SidebarGroupDrop(destination))
    }

    pub(super) fn drop_tab_in_group(
        &mut self,
        source: usize,
        destination: SidebarGroupDrop,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let SidebarGroupDrop(group) = destination;
        if !self.tabs.get(source).is_some_and(WorkspaceTab::is_terminal)
            || self.tab_groups.resolve(group) != group
        {
            return;
        }
        let last = self
            .tab_meta
            .iter()
            .enumerate()
            .rfind(|(index, meta)| {
                *index != source && self.tab_groups.resolve(meta.group_id) == group
            })
            .map(|(index, _)| index);
        let source_active = self.active == source;
        self.tab_meta[source].group_id = group;
        if let Some(group) = group {
            self.tab_groups.collapsed.remove(&group);
        }
        if let Some(last) = last {
            let insert_at = last + usize::from(source > last);
            self.move_tab(source, insert_at, window, cx);
        }
        if source_active {
            self.reveal_active_tab();
        }
        self.clamp_tabs_scroll();
        cx.notify();
    }

    pub(super) fn sidebar_rows(&self) -> Vec<SidebarRow> {
        project_rows(
            &self.tab_groups.profile.groups,
            &self.tab_groups.collapsed,
            self.tab_meta.iter().map(|meta| meta.group_id),
        )
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
            SidebarRow::Tab(ix, _) => Some(ix),
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
        self.sidebar_rows()
            .iter()
            .position(|row| matches!(row, SidebarRow::Tab(ix, _) if *ix == self.active))
            .unwrap_or(0)
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

    pub(super) fn assign_shell_picker_group(&mut self, group: Option<u64>, cx: &mut Context<Self>) {
        if let Some(group) = self.tab_groups.resolve(group) {
            self.set_tab_group(self.active, Some(group), cx);
        }
    }

    pub(super) fn apply_plugin_group(
        &mut self,
        ix: usize,
        group: Option<u64>,
        cx: &mut Context<Self>,
    ) -> crate::plugins::Result<()> {
        if self.tab_groups.resolve(group) != group {
            return Err(crate::plugins::Error::new("target_not_found", "the tab group was removed"));
        }
        self.set_tab_group(ix, group, cx);
        Ok(())
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
        menu_offset: f32,
        drop_target: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        let group = &self.tab_groups.profile.groups[ix];
        let id = group.id;
        let collapsed = self.tab_groups.collapsed.contains(&id);
        let active = collapsed
            && self.tab_meta.get(self.active).is_some_and(|meta| meta.group_id == Some(id));
        let count = self.tab_meta.iter().filter(|meta| meta.group_id == Some(id)).count();
        let language = crate::gpui_shell::config::ui_language(cx);
        let theme = cx.theme();
        let accent = group.color.accent(ix, theme.is_dark());
        let tint = accent.opacity(if active { 0.30 } else { 0.18 });
        let count_text_color = theme.foreground;
        let style = self.plugin_workspace.sidebar;
        let trailing_inset = style.group_inset.map_or(GROUP_TRAILING_INSET, f32::from);
        let radius = px(style.group_radius.map_or(GROUP_CORNER_RADIUS, f32::from));
        h_flex()
            .id(("tab-group", ix))
            .w(px(width))
            .h(px(height))
            .items_center()
            .relative()
            .gap(px(2.0))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    this.open_tab_group_menu(id, event.position, window, cx);
                }),
            )
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .rounded(radius)
                    .bg(if drop_target { accent.opacity(0.29) } else { tint })
                    .when(drop_target, |fill| fill.border_1().border_color(accent)),
            )
            .child(
                Button::new(("tab-group-toggle", ix))
                    .ghost()
                    .rounded(radius)
                    .h_full()
                    .flex_1()
                    .min_w_0()
                    .pl(px(GROUP_GUIDE_X - 3.0))
                    .pr(px(if collapsed { trailing_inset } else { 2.0 }))
                    .overflow_hidden()
                    .selected(active)
                    .aria_label(group.name.clone())
                    .tooltip(group.name.clone())
                    .on_key_down(cx.listener(
                        move |this, event: &gpui::KeyDownEvent, window, cx| {
                            let key = &event.keystroke;
                            if key.key == "menu" || (key.key == "f10" && key.modifiers.shift) {
                                cx.stop_propagation();
                                let position =
                                    this.tabs_list_origin + gpui::point(px(0.0), px(menu_offset));
                                this.open_tab_group_menu(id, position, window, cx);
                            }
                        },
                    ))
                    .child(
                        h_flex()
                            .w_full()
                            .min_w_0()
                            .gap(px(5.0))
                            .child(div().size(px(8.0)).flex_shrink_0().rounded_full().bg(accent))
                            .child(
                                Icon::new(if collapsed {
                                    IconName::ChevronRight
                                } else {
                                    IconName::ChevronDown
                                })
                                .xsmall(),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .font_semibold()
                                    .truncate()
                                    .child(group.name.clone()),
                            )
                            .when(collapsed && style.show_collapsed_count.unwrap_or(true), |row| {
                                row.child(
                                    h_flex()
                                        .ml(px(2.0))
                                        .h(px(21.0))
                                        .min_w(px(24.0))
                                        .px(px(6.0))
                                        .flex_shrink_0()
                                        .items_center()
                                        .justify_center()
                                        .rounded_full()
                                        .bg(accent.opacity(0.20))
                                        .text_color(count_text_color)
                                        .text_xs()
                                        .child(count.to_string()),
                                )
                            }),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !this.tab_groups.collapsed.remove(&id) {
                            this.tab_groups.collapsed.insert(id);
                        }
                        this.clamp_tabs_scroll();
                        cx.notify();
                    })),
            )
            .when(!collapsed && style.show_group_new_tab.unwrap_or(true), |header| {
                header.child(
                    Button::new(("tab-group-new-tab", ix))
                        .ghost()
                        .compact()
                        .rounded(px(9.0))
                        .mr(px(trailing_inset - 3.0))
                        .icon(IconName::Plus)
                        .aria_label(language.text(Message::TabGroupsNewTab))
                        .tooltip(language.text(Message::TabGroupsNewTab))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.open_shell_palette_for_group(Some(id), window, cx);
                        })),
                )
            })
            .into_any_element()
    }

    fn open_tab_group_menu(
        &mut self,
        id: u64,
        position: gpui::Point<gpui::Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let groups = &self.tab_groups.profile.groups;
        let Some((ix, group)) = groups.iter().enumerate().find(|(_, group)| group.id == id) else {
            return;
        };
        let last = ix + 1 == groups.len();
        let current_color = group.color;
        let dark = cx.theme().is_dark();
        let language = crate::gpui_shell::config::ui_language(cx);
        let workspace = cx.entity().downgrade();
        let previous_focus = window.focused(cx);
        let menu = PopupMenu::build(window, cx, move |menu, window, cx| {
            let color_workspace = workspace.clone();
            let menu = menu
                .when_some(previous_focus, |menu, focus| menu.action_context(focus))
                .submenu(language.text(Message::TabGroupsColor), window, cx, move |menu, _, _| {
                    group_color_items(menu, ix, id, current_color, dark, language, &color_workspace)
                });
            [
                (Message::TabGroupsRename, GroupEdit::Rename(id), false),
                (Message::TabGroupsMoveUp, GroupEdit::Move(id, -1), ix == 0),
                (Message::TabGroupsMoveDown, GroupEdit::Move(id, 1), last),
                (Message::TabGroupsDelete, GroupEdit::Delete(id), false),
            ]
            .into_iter()
            .fold(menu, |menu, (label, edit, disabled)| {
                let workspace = workspace.clone();
                menu.item(PopupMenuItem::new(language.text(label)).disabled(disabled).on_click(
                    move |_, window, cx| {
                        let _ = workspace.update(cx, |this, cx| match edit {
                            GroupEdit::Rename(_) => this.edit_tab_group_name(edit, window, cx),
                            _ => {
                                this.apply_tab_group_edit(edit, "", window, cx);
                            },
                        });
                    },
                ))
            })
        });
        self.show_context_menu(menu, tab_menu::ContextMenuTarget::Group(id), position, window, cx);
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

fn group_color_items(
    menu: PopupMenu,
    index: usize,
    id: u64,
    current: GroupColor,
    dark: bool,
    language: crate::i18n::UiLanguage,
    workspace: &gpui::WeakEntity<NebulaWorkspace>,
) -> PopupMenu {
    GroupColor::CHOICES.into_iter().fold(menu, |menu, (color, label)| {
        let workspace = workspace.clone();
        menu.item(
            PopupMenuItem::new(language.text(label))
                .icon(Icon::new(IconName::Palette).text_color(color.accent(index, dark)))
                .checked(color == current)
                .on_click(move |_, window, cx| {
                    let _ = workspace.update(cx, |this, cx| {
                        this.apply_tab_group_edit(GroupEdit::Color(id, color), "", window, cx);
                    });
                }),
        )
    })
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
            groups.push(Group { id, name: name.to_owned(), color: GroupColor::Auto });
            return Ok(());
        },
        GroupEdit::Rename(id)
        | GroupEdit::Color(id, _)
        | GroupEdit::Move(id, _)
        | GroupEdit::Delete(id) => id,
    };
    let Some(ix) = groups.iter().position(|group| group.id == id) else { return Ok(()) };
    match edit {
        GroupEdit::Rename(_) => groups[ix].name = name.to_owned(),
        GroupEdit::Color(_, color) => groups[ix].color = color,
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
    fn group_color_round_trips_and_old_profiles_default_to_automatic() {
        let mut groups: Vec<Group> = serde_json::from_str(r#"[{"id":1,"name":"A"}]"#).unwrap();
        assert_eq!(groups[0].color, GroupColor::Auto);
        assert_eq!(
            project_rows(&groups, &HashSet::new(), [Some(1)]),
            [SidebarRow::Group(0), SidebarRow::Tab(0, Some(0)), SidebarRow::Ungrouped],
        );
        update_groups(&mut groups, GroupEdit::Color(1, GroupColor::Purple), "").unwrap();
        let saved = serde_json::to_string(&groups).unwrap();
        assert_eq!(
            serde_json::from_str::<Vec<Group>>(&saved).unwrap()[0].color,
            GroupColor::Purple
        );
        assert_eq!(GroupColor::Purple.accent(0, true), GroupColor::Purple.accent(1, true));
        update_groups(&mut groups, GroupEdit::Color(1, GroupColor::Auto), "").unwrap();
        assert!(!serde_json::to_string(&groups).unwrap().contains("color"));
    }

    #[test]
    fn group_drop_only_hits_visible_rows_and_skips_gaps() {
        let rows = [SidebarRow::Group(0), SidebarRow::Group(1), SidebarRow::Ungrouped];
        let density = pebrel_settings::DensityName::Standard;
        let layout = tab_scroll::SidebarRowLayout::new(&rows, density, Default::default(), Default::default());
        let height = layout.row_height(0);
        let pitch = height + tab_scroll::tab_row_gap(density);
        assert_eq!(layout.row_at(-1.0, 0, 2), None);
        assert_eq!(layout.row_at(20.0, 0, 2), Some(0));
        assert_eq!(layout.row_at(height - 1.0, 0, 2), Some(0));
        assert_eq!(layout.row_at(height, 0, 2), None);
        assert_eq!(layout.row_at(pitch + 3.0, 0, 2), None);
        assert_eq!(layout.row_at(pitch + 6.0, 0, 2), Some(1));
        assert_eq!(layout.row_at(pitch + 6.0 + height, 0, 2), None);
        assert_eq!(layout.row_at(0.0, 1, 2), Some(1));
    }

    #[test]
    fn projected_rows_include_empty_groups_and_keep_orphans_visible() {
        let mut groups: Vec<Group> =
            serde_json::from_str(r#"[{"id":1,"name":"A"},{"id":2,"name":"B"}]"#).unwrap();
        let membership = [None, Some(1), Some(99), Some(1)];
        use SidebarRow::{Group as Header, Tab, Ungrouped};
        assert_eq!(
            project_rows(&groups, &HashSet::new(), membership),
            [
                Header(0),
                Tab(1, Some(0)),
                Tab(3, Some(0)),
                Header(1),
                Ungrouped,
                Tab(0, None),
                Tab(2, None),
            ]
        );
        assert_eq!(
            project_rows(&groups, &HashSet::from([1]), membership),
            [Header(0), Header(1), Ungrouped, Tab(0, None), Tab(2, None)]
        );
        update_groups(&mut groups, GroupEdit::Delete(1), "").unwrap();
        assert_eq!(
            project_rows(&groups, &HashSet::new(), membership),
            [Header(0), Ungrouped, Tab(0, None), Tab(1, None), Tab(2, None), Tab(3, None)]
        );
    }
}
