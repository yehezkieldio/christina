use ratatui::style::Color;

// Base colors
pub const BASE: Color = Color::Rgb(30, 30, 46); // #1e1e2e

// Text colors
pub const TEXT: Color = Color::Rgb(205, 214, 244); // #cdd6f4
pub const SUBTEXT0: Color = Color::Rgb(166, 173, 200); // #a6adc8
pub const SUBTEXT1: Color = Color::Rgb(186, 194, 222); // #bac2de

// Surface colors
pub const SURFACE0: Color = Color::Rgb(49, 50, 68); // #313244
pub const SURFACE1: Color = Color::Rgb(69, 71, 90); // #45475a
pub const OVERLAY0: Color = Color::Rgb(108, 112, 134); // #6c7086

// Accent colors
pub const ROSEWATER: Color = Color::Rgb(245, 224, 220); // #f5e0dc
pub const RED: Color = Color::Rgb(243, 139, 168); // #f38ba8
pub const GREEN: Color = Color::Rgb(166, 227, 161); // #a6e3a1
pub const BLUE: Color = Color::Rgb(137, 180, 250); // #89b4fa

// Checkbox states
pub const CHECKBOX_SELECTED: &str = " ●  ";
pub const CHECKBOX_UNSELECTED: &str = " ○  ";

// File status indicators
pub const STATUS_MODIFIED: &str = " M ";
pub const STATUS_ADDED: &str = " A ";
pub const STATUS_DELETED: &str = " D ";
pub const STATUS_UNKNOWN: &str = " ? ";
