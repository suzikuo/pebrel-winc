use crate::RawSettings;

/// Adjacent rows whose spacing the user can override independently.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SidebarGap {
    Tabs,
    HeadingToTab,
    Headings,
    TabToGroup,
    BeforeUngrouped,
}

impl SidebarGap {
    pub const ALL: [Self; 5] =
        [Self::Tabs, Self::HeadingToTab, Self::Headings, Self::TabToGroup, Self::BeforeUngrouped];
    pub const MAX: u8 = 48;

    pub const fn key(self) -> &'static str {
        match self {
            Self::Tabs => "sidebar_gap_tabs",
            Self::HeadingToTab => "sidebar_gap_heading_to_tab",
            Self::Headings => "sidebar_gap_headings",
            Self::TabToGroup => "sidebar_gap_tab_to_group",
            Self::BeforeUngrouped => "sidebar_gap_before_ungrouped",
        }
    }

    pub fn parse(value: &str) -> Option<u8> {
        value.trim().parse::<u8>().ok().filter(|&gap| gap <= Self::MAX)
    }
}

/// `None` inherits the current appearance. `Some(0)` removes the gap.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SidebarSpacing([Option<u8>; SidebarGap::ALL.len()]);

impl SidebarSpacing {
    pub fn from_raw(raw: &RawSettings) -> Self {
        Self(SidebarGap::ALL.map(|gap| raw.value(gap.key()).and_then(SidebarGap::parse)))
    }

    pub fn get(self, gap: SidebarGap) -> Option<u8> {
        self.0[gap as usize]
    }
}

/// 标签栏位置。这里只提供产品当前支持的两种布局，避免把设置扩成未实现的
/// 通用停靠系统。
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum TabsPositionName {
    #[default]
    Sidebar,
    Top,
}

impl TabsPositionName {
    pub fn from_settings(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "sidebar" => Some(Self::Sidebar),
            "top" => Some(Self::Top),
            _ => None,
        }
    }

    pub fn settings_value(self) -> &'static str {
        match self {
            Self::Sidebar => "sidebar",
            Self::Top => "top",
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum TabRevealName {
    #[default]
    Slide,
    Instant,
}

impl TabRevealName {
    pub fn from_settings(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "slide" => Some(Self::Slide),
            "instant" => Some(Self::Instant),
            _ => None,
        }
    }

    pub fn settings_value(self) -> &'static str {
        match self {
            Self::Slide => "slide",
            Self::Instant => "instant",
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum DensityName {
    #[default]
    Standard,
    Compact,
}

impl DensityName {
    pub fn from_settings(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "standard" => Some(Self::Standard),
            "compact" => Some(Self::Compact),
            _ => None,
        }
    }

    pub fn settings_value(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Compact => "compact",
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum NewTabPositionName {
    #[default]
    AfterCurrent,
    End,
}

impl NewTabPositionName {
    pub fn from_settings(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "after_current" => Some(Self::AfterCurrent),
            "end" => Some(Self::End),
            _ => None,
        }
    }

    pub fn settings_value(self) -> &'static str {
        match self {
            Self::AfterCurrent => "after_current",
            Self::End => "end",
        }
    }
}
