use crate::RawSettings;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScratchPlacement {
    #[default]
    Docked,
    FloatingBottom,
    FloatingRight,
}

impl ScratchPlacement {
    pub const ALL: [Self; 3] = [Self::Docked, Self::FloatingBottom, Self::FloatingRight];

    pub const fn settings_value(self) -> &'static str {
        match self {
            Self::Docked => "docked",
            Self::FloatingBottom => "floating_bottom",
            Self::FloatingRight => "floating_right",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScratchTerminal {
    pub placement: ScratchPlacement,
    pub size: u8,
    pub font_size_px: Option<f32>,
    pub inherit_directory: bool,
    pub escape_to_hide: bool,
}

impl ScratchTerminal {
    pub const PLACEMENT: &str = "scratch_terminal_placement";
    pub const SIZE: &str = "scratch_terminal_size";
    pub const FONT_SIZE: &str = "scratch_terminal_font_size";
    pub const FONT_SIZE_RANGE: std::ops::RangeInclusive<f32> = 4.0..=64.0;
    pub const INHERIT_DIRECTORY: &str = "scratch_terminal_inherit_directory";
    pub const ESCAPE_TO_HIDE: &str = "scratch_terminal_escape_to_hide";
    pub const SIZES: [u8; 4] = [25, 35, 50, 65];

    pub(crate) fn from_raw(raw: &RawSettings) -> Self {
        Self {
            placement: ScratchPlacement::ALL
                .into_iter()
                .find(|placement| raw.value(Self::PLACEMENT) == Some(placement.settings_value()))
                .unwrap_or_default(),
            size: raw
                .value(Self::SIZE)
                .and_then(|value| value.parse::<u8>().ok())
                .filter(|size| Self::SIZES.contains(size))
                .unwrap_or(35),
            font_size_px: raw
                .value(Self::FONT_SIZE)
                .and_then(|value| value.parse::<f32>().ok())
                .filter(|size| Self::FONT_SIZE_RANGE.contains(size)),
            inherit_directory: raw.bool_on(Self::INHERIT_DIRECTORY).unwrap_or(true),
            escape_to_hide: raw.bool_on(Self::ESCAPE_TO_HIDE).unwrap_or(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scratch_terminal_preferences_restore_and_validate_values() {
        let raw = RawSettings::from_text(
            "scratch_terminal_placement=floating_right\nscratch_terminal_size=50\n\
             scratch_terminal_font_size=12.50\nscratch_terminal_inherit_directory=0\n\
             scratch_terminal_escape_to_hide=1\n",
        );
        assert_eq!(
            ScratchTerminal::from_raw(&raw),
            ScratchTerminal {
                placement: ScratchPlacement::FloatingRight,
                size: 50,
                font_size_px: Some(12.5),
                inherit_directory: false,
                escape_to_hide: true,
            }
        );
        let fallback = ScratchTerminal::from_raw(&RawSettings::from_text(
            "scratch_terminal_size=99\nscratch_terminal_placement=unknown\n",
        ));
        assert_eq!(fallback, ScratchTerminal::from_raw(&RawSettings::from_text("")));
        assert_eq!(fallback.size, 35);
        for value in ["", "3", "65", "NaN", "invalid"] {
            let raw = RawSettings::from_text(&format!("{}={value}\n", ScratchTerminal::FONT_SIZE));
            assert_eq!(ScratchTerminal::from_raw(&raw).font_size_px, None);
        }
    }
}
