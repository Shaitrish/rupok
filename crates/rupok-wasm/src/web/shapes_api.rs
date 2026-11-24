//! The Shapes picker's part of the [`Board`] API (`engine/insert.rs`).

use wasm_bindgen::prelude::*;

use super::{Board, now};

/// `0` means none for the colour arguments below.
fn opt(rgba: u32) -> Option<u32> {
    (rgba != 0).then_some(rgba)
}

#[wasm_bindgen]
impl Board {
    /// Fill (0xRRGGBBAA, 0 = none) of shapes placed from now on.
    pub fn set_shape_fill(&mut self, rgba: u32) {
        self.engine.set_shape_fill(opt(rgba));
    }

    /// Shift or the picker's lock: square boxes and circles, 15° lines.
    pub fn set_shape_lock(&mut self, lock: bool) {
        self.engine.set_shape_lock(lock);
    }

    /// Ids (JSON list) of the items the last shape placement added, once;
    /// `""` when nothing was placed since the last call.
    pub fn take_placed_shape(&mut self) -> String {
        self.engine
            .take_placed()
            .map(|ids| serde_json::to_string(&ids).unwrap_or_default())
            .unwrap_or_default()
    }

    /// Fill the selected closed shapes (0 clears). One undo step.
    pub fn set_selection_fill(&mut self, rgba: u32) -> bool {
        self.engine.set_selection_fill(opt(rgba))
    }

    /// Dashed or solid outlines for the selected shapes. One undo step.
    pub fn set_selection_dash(&mut self, dash: bool) -> bool {
        self.engine.set_selection_dash(dash)
    }

    /// `row+`, `row-`, `col+` or `col-` on the selected table.
    pub fn table_edit(&mut self, op: &str) -> bool {
        self.engine.table_edit(op)
    }

    /// The selected table's cells (see `Engine::table_cells_json`).
    pub fn table_cells_json(&self) -> String {
        self.engine.table_cells_json()
    }

    /// A text item in a cell of the selected table; returns its id.
    pub fn table_cell_text(&mut self, row: usize, col: usize, text: &str) -> Option<String> {
        self.engine.table_cell_text(row, col, text, now())
    }

    /// The picker's shape ids, as a JSON list.
    pub fn shape_ids(&self) -> String {
        serde_json::to_string(&crate::catalog::IDS).unwrap_or_default()
    }
}
