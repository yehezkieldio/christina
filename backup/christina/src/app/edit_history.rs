use compact_str::CompactString;
use std::collections::VecDeque;

/// Maximum number of undo states to keep.
const MAX_HISTORY_SIZE: usize = 50;

/// Edit history with undo/redo support.
#[derive(Debug, Clone)]
pub struct EditHistory {
    /// Stack of previous states (for undo).
    undo_stack: VecDeque<HistoryEntry>,
    /// Stack of undone states (for redo).
    redo_stack: Vec<HistoryEntry>,
    /// Current state.
    current: Option<HistoryEntry>,
    /// Maximum history size.
    max_size: usize,
}

/// A single entry in the edit history.
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    /// The message content.
    pub message: CompactString,
    /// Cursor position (line, column).
    pub cursor: (usize, usize),
}

impl HistoryEntry {
    pub fn new(
        message: impl Into<CompactString>,
        cursor: (usize, usize),
        _description: impl Into<CompactString>,
    ) -> Self {
        Self {
            message: message.into(),
            cursor,
        }
    }
}

impl EditHistory {
    pub fn new() -> Self {
        Self {
            undo_stack: VecDeque::with_capacity(MAX_HISTORY_SIZE),
            redo_stack: Vec::new(),
            current: None,
            max_size: MAX_HISTORY_SIZE,
        }
    }

    pub fn push(&mut self, entry: HistoryEntry) {
        // Save current state to undo stack
        if let Some(current) = self.current.take() {
            self.undo_stack.push_back(current);

            // Enforce maximum size
            while self.undo_stack.len() > self.max_size {
                self.undo_stack.pop_front();
            }
        }

        // Clear redo stack
        self.redo_stack.clear();

        // Set new current state
        self.current = Some(entry);
    }

    pub fn undo(&mut self) -> Option<&HistoryEntry> {
        if let Some(current) = self.current.take() {
            // Move current to redo stack
            self.redo_stack.push(current);

            // Pop from undo stack
            self.current = self.undo_stack.pop_back();
        }

        self.current.as_ref()
    }

    pub fn redo(&mut self) -> Option<&HistoryEntry> {
        if let Some(redo) = self.redo_stack.pop() {
            // Move current to undo stack
            if let Some(current) = self.current.take() {
                self.undo_stack.push_back(current);
            }

            // Restore from redo stack
            self.current = Some(redo);
        }

        self.current.as_ref()
    }

    pub fn current(&self) -> Option<&HistoryEntry> {
        self.current.as_ref()
    }

    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.current = None;
    }

    pub fn initialize(&mut self, message: impl Into<CompactString>) {
        self.clear();
        self.current = Some(HistoryEntry::new(message, (0, 0), "initial"));
    }
}

impl Default for EditHistory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_undo_redo() {
        let mut history = EditHistory::new();

        history.push(HistoryEntry::new("state1", (0, 0), "initial"));
        history.push(HistoryEntry::new("state2", (0, 5), "typing"));
        history.push(HistoryEntry::new("state3", (0, 10), "more typing"));

        assert_eq!(
            history.current().map(|e| e.message.as_str()),
            Some("state3")
        );

        // Undo
        let prev = history.undo();
        assert_eq!(prev.map(|e| e.message.as_str()), Some("state2"));

        // Redo
        let next = history.redo();
        assert_eq!(next.map(|e| e.message.as_str()), Some("state3"));
    }

    #[test]
    fn test_undo_clears_redo() {
        let mut history = EditHistory::new();

        history.push(HistoryEntry::new("state1", (0, 0), "initial"));
        history.push(HistoryEntry::new("state2", (0, 5), "typing"));

        history.undo();
        assert_eq!(history.redo_stack.len(), 1);

        // New push clears redo
        history.push(HistoryEntry::new("state3", (0, 7), "new typing"));
        assert_eq!(history.redo_stack.len(), 0);
    }

    #[test]
    fn test_max_size() {
        let mut history = EditHistory::new();

        for i in 0..10 {
            history.push(HistoryEntry::new(format!("state{}", i), (0, 0), "push"));
        }

        // Should only keep last MAX_HISTORY_SIZE in undo stack
        assert!(history.undo_stack.len() <= MAX_HISTORY_SIZE);
    }
}
