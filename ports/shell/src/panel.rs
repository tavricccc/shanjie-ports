//! What each candidate cell shows besides its text (macos/Sources/ShanjieKit/CandidateCells.swift `update`): the number,
//! the Tab glyph of the not-entered prediction row, and which cells show their number. Platforms draw these with their own
//! candidate window, so the rule lives here once.

/// `CellMetrics.tabGlyph`.
pub const TAB_GLYPH: &str = "⇥";

/// One cell's number slot: `Some(text)` when it shows, `None` when the cell shows no number.
///
/// - The not-entered prediction row (`selected` < 0) shows the Tab glyph on its first cell and nothing else.
/// - Otherwise the bar and the vertical window number every cell 1...; the expanded grid (`columns` > 0) numbers each row
///   1...columns and shows the numbers on the selected row only.
pub fn numbers(count: usize, selected: i32, columns: u32) -> Vec<Option<String>> {
    let grid = columns > 0;
    let cols = columns.max(1) as usize;
    let selected_row = if grid && selected >= 0 { selected as usize / cols } else { 0 };
    (0..count)
        .map(|i| {
            let tab = selected < 0 && i == 0;
            let shows = tab || (selected >= 0 && (!grid || i / cols == selected_row));
            shows.then(|| if tab { TAB_GLYPH.to_string() } else { (if grid { i % cols + 1 } else { i + 1 }).to_string() })
        })
        .collect()
}

/// For a candidate window that cannot draw the expanded grid: the row that holds the selection, as (start index, length).
/// The whole output when it is not a grid.
pub fn selected_row(count: usize, selected: i32, columns: u32) -> (usize, usize) {
    if columns == 0 || selected < 0 {
        return (0, count);
    }
    let cols = columns as usize;
    let start = (selected as usize / cols) * cols;
    (start.min(count), cols.min(count.saturating_sub(start)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_vertical_and_prediction_row() {
        assert_eq!(numbers(3, 0, 0), vec![Some("1".into()), Some("2".into()), Some("3".into())]);
        assert_eq!(numbers(3, -1, 0), vec![Some(TAB_GLYPH.into()), None, None]);
    }

    #[test]
    fn grid_numbers_only_the_selected_row() {
        let n = numbers(18, 10, 9);
        assert!(n[..9].iter().all(Option::is_none));
        assert_eq!(n[9].as_deref(), Some("1"));
        assert_eq!(n[17].as_deref(), Some("9"));
        assert_eq!(selected_row(18, 10, 9), (9, 9));
        assert_eq!(selected_row(12, 10, 9), (9, 3));
        assert_eq!(selected_row(5, 2, 0), (0, 5));
    }
}
