use ratatui::style::Color;

// Base colors - using terminal defaults for adaptive theming
pub const BASE: Color = Color::Reset;

// Text colors - standard terminal colors that adapt to user's theme
pub const TEXT: Color = Color::Reset;
pub const SUBTEXT0: Color = Color::DarkGray;
pub const SUBTEXT1: Color = Color::Gray;

// Surface colors - using terminal color palette
pub const SURFACE0: Color = Color::Black;
pub const SURFACE1: Color = Color::DarkGray;
pub const OVERLAY0: Color = Color::Gray;

// Accent colors - standard ANSI colors
pub const ROSEWATER: Color = Color::LightMagenta;
pub const RED: Color = Color::Red;
pub const GREEN: Color = Color::Green;
pub const BLUE: Color = Color::Blue;

// Checkbox states
pub const CHECKBOX_SELECTED: &str = " ●  ";
pub const CHECKBOX_UNSELECTED: &str = " ○  ";

// File status indicators
pub const STATUS_MODIFIED: &str = " M ";
pub const STATUS_ADDED: &str = " A ";
pub const STATUS_DELETED: &str = " D ";
pub const STATUS_UNKNOWN: &str = " ? ";
