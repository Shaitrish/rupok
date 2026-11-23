//! [`Lesson`]: the Loro-backed lesson document and its mutation API.
//!
//! Every mutating call commits immediately, so each user action is one Loro
//! change — one undo step, one replay step, one sync delta.
//!
//! Wall-clock time is always passed in by the caller (`now_ms`): the core runs
//! in `wasm32-unknown-unknown`, where `SystemTime::now()` is unavailable, and it
//! keeps tests deterministic.

use loro::{
    Container, ExportMode, LoroDoc, LoroMap, LoroMovableList, LoroValue, UndoManager,
    ValueOrContainer, VersionVector,
};
use rupok_ink::{decode_points, encode_points};

use crate::model::*;

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("page not found: {0}")]
    PageNotFound(String),
    #[error("item not found: {0}")]
    ItemNotFound(String),
    #[error("page index {0} out of range")]
    PageIndex(usize),
    #[error("corrupt document: {0}")]
    Corrupt(String),
    #[error("loro: {0}")]
    Loro(String),
}

impl From<loro::LoroError> for CoreError {
    fn from(e: loro::LoroError) -> Self {
        CoreError::Loro(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, CoreError>;

pub struct Lesson {
    doc: LoroDoc,
    undo: UndoManager,
    counter: u64,
    /// Highest z per page, so appending is O(1) instead of a page scan.
    /// A stale (too high) value is harmless; it is cleared on remote import.
    top_z: std::collections::HashMap<String, f64>,
}

impl std::fmt::Debug for Lesson {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Lesson")
            .field("peer", &self.doc.peer_id())
            .finish_non_exhaustive()
    }
}

impl Lesson {
    fn wrap(doc: LoroDoc) -> Self {
        doc.set_record_timestamp(true);
        // Never merge separate user actions into one change: each stroke must
        // stay individually undoable and replayable.
        doc.set_change_merge_interval(0);
        let mut undo = UndoManager::new(&doc);
        undo.set_merge_interval(0);
        undo.set_max_undo_steps(500);
        Self {
            doc,
            undo,
            counter: 0,
            top_z: std::collections::HashMap::new(),
        }
    }

    /// New lesson with one empty slide page.
    pub fn new(title: &str, now_ms: f64) -> Self {
        let mut l = Self::wrap(LoroDoc::new());
        let meta = l.doc.get_map("meta");
        meta.insert("format", crate::DOC_FORMAT).unwrap();
        meta.insert("title", title).unwrap();
        meta.insert("created", now_ms).unwrap();
        l.doc.commit();
        l.add_page(None, PageSpec::default()).expect("fresh doc");
        // Creating the lesson is not an undoable user action.
        l.undo = {
            let mut u = UndoManager::new(&l.doc);
            u.set_merge_interval(0);
            u.set_max_undo_steps(500);
            u
        };
        l
    }

    /// Load from a snapshot or update blob produced by [`Lesson::save`].
    pub fn load(bytes: &[u8]) -> Result<Self> {
        let doc = LoroDoc::new();
        doc.import(bytes)
            .map_err(|e| CoreError::Corrupt(e.to_string()))?;
        let format = doc
            .get_map("meta")
            .get("format")
            .and_then(|v| v.into_value().ok())
            .and_then(|v| match v {
                LoroValue::I64(i) => Some(i),
                LoroValue::Double(d) => Some(d as i64),
                _ => None,
            });
        match format {
            Some(f) if f <= crate::DOC_FORMAT => {}
            Some(f) => {
                return Err(CoreError::Corrupt(format!(
                    "document format {f} is newer than supported"
                )));
            }
            None => return Err(CoreError::Corrupt("missing meta.format".into())),
        }
        Ok(Self::wrap(doc))
    }

    /// Full snapshot including history (= the replayable recording).
    pub fn save(&self) -> Vec<u8> {
        self.doc
            .export(ExportMode::Snapshot)
            .expect("snapshot export")
    }

    pub fn version(&self) -> VersionVector {
        self.doc.oplog_vv()
    }

    /// Delta since `from` — the unit of sync between replicas.
    pub fn export_updates(&self, from: &VersionVector) -> Vec<u8> {
        self.doc
            .export(ExportMode::updates(from))
            .expect("update export")
    }

    /// Merge a snapshot/update from another replica.
    pub fn import(&mut self, bytes: &[u8]) -> Result<()> {
        self.doc
            .import(bytes)
            .map_err(|e| CoreError::Corrupt(e.to_string()))?;
        self.top_z.clear();
        Ok(())
    }

    pub fn peer_id(&self) -> u64 {
        self.doc.peer_id()
    }

    /// Access to the raw Loro document (sync engines, advanced tooling).
    pub fn loro(&self) -> &LoroDoc {
        &self.doc
    }

    fn new_id(&mut self) -> String {
        self.counter += 1;
        format!("{}-{}", base36(self.doc.peer_id()), base36(self.counter))
    }

    // ---------------------------------------------------------------- meta

    pub fn title(&self) -> String {
        str_of(&self.doc.get_map("meta"), "title").unwrap_or_default()
    }

    pub fn set_title(&mut self, title: &str) {
        self.doc.get_map("meta").insert("title", title).unwrap();
        self.doc.commit();
    }

    pub fn created(&self) -> f64 {
        f64_of(&self.doc.get_map("meta"), "created").unwrap_or(0.0)
    }

    // --------------------------------------------------------------- pages

    fn pages_list(&self) -> LoroMovableList {
        self.doc.get_movable_list("pages")
    }

    fn page_map_at(&self, idx: usize) -> Option<LoroMap> {
        match self.pages_list().get(idx)? {
            ValueOrContainer::Container(Container::Map(m)) => Some(m),
            _ => None,
        }
    }

    fn page_index(&self, page_id: &str) -> Result<usize> {
        let list = self.pages_list();
        (0..list.len())
            .find(|&i| {
                self.page_map_at(i)
                    .and_then(|m| str_of(&m, "id"))
                    .as_deref()
                    == Some(page_id)
            })
            .ok_or_else(|| CoreError::PageNotFound(page_id.to_string()))
    }

    fn page_map(&self, page_id: &str) -> Result<LoroMap> {
        let i = self.page_index(page_id)?;
        self.page_map_at(i)
            .ok_or_else(|| CoreError::Corrupt("page entry is not a map".into()))
    }

    fn items_map(&self, page_id: &str) -> Result<LoroMap> {
        let page = self.page_map(page_id)?;
        match page.get("items") {
            Some(ValueOrContainer::Container(Container::Map(m))) => Ok(m),
            _ => Err(CoreError::Corrupt(format!(
                "page {page_id} has no items map"
            ))),
        }
    }

    pub fn page_count(&self) -> usize {
        self.pages_list().len()
    }

    pub fn pages(&self) -> Vec<PageInfo> {
        (0..self.page_count())
            .filter_map(|i| self.page_map_at(i).map(|m| page_info(&m)))
            .collect()
    }

    pub fn page(&self, page_id: &str) -> Result<PageInfo> {
        Ok(page_info(&self.page_map(page_id)?))
    }

    pub fn page_id_at(&self, idx: usize) -> Result<PageId> {
        self.page_map_at(idx)
            .and_then(|m| str_of(&m, "id"))
            .ok_or(CoreError::PageIndex(idx))
    }

    /// Insert a page at `index` (default: end). Returns its id.
    pub fn add_page(&mut self, index: Option<usize>, spec: PageSpec) -> Result<PageId> {
        let list = self.pages_list();
        let idx = index.unwrap_or(list.len()).min(list.len());
        let id = self.new_id();
        let page = list.insert_container(idx, LoroMap::new())?;
        page.insert("id", id.as_str())?;
        page.insert("title", spec.title.as_str())?;
        if let Some(f) = spec.frame {
            page.insert("frame_w", f.w)?;
            page.insert("frame_h", f.h)?;
        }
        page.insert("bg_pattern", spec.pattern.as_str())?;
        page.insert("bg_color", i64::from(spec.bg_color))?;
        page.insert_container("items", LoroMap::new())?;
        self.doc.commit();
        Ok(id)
    }

    pub fn remove_page(&mut self, page_id: &str) -> Result<()> {
        let i = self.page_index(page_id)?;
        self.pages_list().delete(i, 1)?;
        self.doc.commit();
        Ok(())
    }

    pub fn move_page(&mut self, from: usize, to: usize) -> Result<()> {
        let n = self.page_count();
        if from >= n || to >= n {
            return Err(CoreError::PageIndex(from.max(to)));
        }
        self.pages_list().mov(from, to)?;
        self.doc.commit();
        Ok(())
    }

    pub fn set_page_background(
        &mut self,
        page_id: &str,
        pattern: Pattern,
        color: u32,
    ) -> Result<()> {
        let page = self.page_map(page_id)?;
        page.insert("bg_pattern", pattern.as_str())?;
        page.insert("bg_color", i64::from(color))?;
        self.doc.commit();
        Ok(())
    }

    pub fn set_page_title(&mut self, page_id: &str, title: &str) -> Result<()> {
        self.page_map(page_id)?.insert("title", title)?;
        self.doc.commit();
        Ok(())
    }

    /// Copy a page with all of its items right after it, as one change.
    /// Returns the new page id.
    ///
    /// The copy is made at the document level, so it keeps every key and
    /// item, including ones this build does not understand (spec §
    /// Versioning, rule 3). Items get fresh ids but keep their `z`,
    /// `created` and their references (`bundle`, `asset`), so a copied
    /// page replays like the original.
    pub fn duplicate_page(&mut self, page_id: &str) -> Result<PageId> {
        let idx = self.page_index(page_id)?;
        let src = self.page_map(page_id)?;
        let src_items = self.items_map(page_id)?;
        let mut keys: Vec<(String, LoroValue)> = Vec::new();
        if let LoroValue::Map(all) = src.get_deep_value() {
            for (k, v) in all.iter() {
                if k != "id" && k != "items" {
                    keys.push((k.clone(), v.clone()));
                }
            }
        }
        let mut entries: Vec<(LoroValue, bool)> = Vec::new();
        src_items.for_each(|_, v| {
            let container = matches!(v, ValueOrContainer::Container(_));
            entries.push((v.get_deep_value(), container));
        });
        let id = self.new_id();
        let page = self
            .pages_list()
            .insert_container(idx + 1, LoroMap::new())?;
        page.insert("id", id.as_str())?;
        for (k, v) in keys {
            page.insert(&k, v)?;
        }
        let items = page.insert_container("items", LoroMap::new())?;
        for (v, container) in entries {
            let item_id = self.new_id();
            if container {
                let m = items.insert_container(&item_id, LoroMap::new())?;
                if let LoroValue::Map(fields) = v {
                    for (k, fv) in fields.iter() {
                        m.insert(k, fv.clone())?;
                    }
                }
            } else {
                items.insert(&item_id, v)?;
            }
        }
        self.doc.commit();
        Ok(id)
    }

    // --------------------------------------------------------------- items

    fn next_z(&mut self, page_id: &str, items: &LoroMap) -> f64 {
        let z = match self.top_z.get(page_id) {
            Some(z) => *z,
            None => Self::max_z(items),
        } + 1.0;
        self.top_z.insert(page_id.to_string(), z);
        z
    }

    fn max_z(items: &LoroMap) -> f64 {
        let mut z = 0.0f64;
        items.for_each(|_, v| {
            if let ValueOrContainer::Container(Container::Map(m)) = v
                && let Some(iz) = f64_of(&m, "z")
            {
                z = z.max(iz);
            }
        });
        z
    }

    /// Add an item on top of the page. Returns its id.
    ///
    /// Strokes are stored as immutable map *values* (cheap to load by the
    /// hundred thousand); box items are containers so fields can be edited
    /// independently.
    pub fn add_item(&mut self, page_id: &str, data: &ItemData, now_ms: f64) -> Result<ItemId> {
        let items = self.items_map(page_id)?;
        let id = self.insert_item(page_id, &items, data, now_ms, None, None)?;
        self.doc.commit();
        Ok(id)
    }

    /// Add several items on top of the page, in order (the first ends up
    /// lowest), as **one** change: a paste or duplicate is one undo step.
    /// Each entry carries its own `created` time and optional group.
    pub fn add_items(&mut self, page_id: &str, list: &[NewItem]) -> Result<Vec<ItemId>> {
        let items = self.items_map(page_id)?;
        let mut ids = Vec::with_capacity(list.len());
        for it in list {
            ids.push(self.insert_item(
                page_id,
                &items,
                &it.data,
                it.created,
                it.group.as_deref(),
                it.z,
            )?);
        }
        if !ids.is_empty() {
            self.doc.commit();
        }
        Ok(ids)
    }

    /// Write one item without committing.
    fn insert_item(
        &mut self,
        page_id: &str,
        items: &LoroMap,
        data: &ItemData,
        created: f64,
        group: Option<&str>,
        z: Option<f64>,
    ) -> Result<ItemId> {
        let id = self.new_id();
        let z = match z {
            Some(z) if z.is_finite() => z,
            _ => self.next_z(page_id, items),
        };
        match data {
            ItemData::Stroke(_) => {
                let mut f = ValueFields::default();
                f.set("kind", data.kind());
                f.set("z", z);
                f.set("created", created);
                if let Some(g) = group {
                    f.set("group", g);
                }
                write_item_data(&mut f, data)?;
                items.insert(&id, f.into_value())?;
            }
            _ => {
                let m = items.insert_container(&id, LoroMap::new())?;
                m.insert("kind", data.kind())?;
                m.insert("z", z)?;
                m.insert("created", created)?;
                if let Some(g) = group {
                    m.insert("group", g)?;
                }
                write_item_data(&mut MapFields(&m), data)?;
            }
        }
        Ok(id)
    }

    /// A fresh group id (unique in the document).
    pub fn new_group_id(&mut self) -> String {
        format!("g{}", self.new_id())
    }

    /// Put items in one new group (replacing any group they were in), as
    /// one change. Returns the group id.
    pub fn group_items(&mut self, page_id: &str, ids: &[ItemId]) -> Result<String> {
        let g = self.new_group_id();
        self.set_group(page_id, ids, Some(&g))?;
        Ok(g)
    }

    /// Dissolve a group: its items stay, ungrouped. One change; returns how
    /// many items were in the group.
    pub fn ungroup(&mut self, page_id: &str, group: &str) -> Result<usize> {
        let ids: Vec<ItemId> = self
            .items(page_id)?
            .into_iter()
            .filter(|i| i.group.as_deref() == Some(group))
            .map(|i| i.id)
            .collect();
        if !ids.is_empty() {
            self.set_group(page_id, &ids, None)?;
        }
        Ok(ids.len())
    }

    /// Set or clear the `group` key on items, as one change.
    pub fn set_group(&mut self, page_id: &str, ids: &[ItemId], group: Option<&str>) -> Result<()> {
        let items = self.items_map(page_id)?;
        for id in ids {
            Self::write_group(&items, id, group)?;
        }
        self.doc.commit();
        Ok(())
    }

    fn write_group(items: &LoroMap, id: &ItemId, group: Option<&str>) -> Result<()> {
        match items.get(id) {
            Some(ValueOrContainer::Value(LoroValue::Map(v))) => {
                let mut f = ValueFields::from(&v);
                match group {
                    Some(g) => f.set("group", g),
                    None => f.remove("group"),
                }
                items.insert(id, f.into_value())?;
            }
            Some(ValueOrContainer::Container(Container::Map(m))) => match group {
                Some(g) => m.insert("group", g)?,
                None => {
                    if m.get("group").is_some() {
                        m.delete("group")?;
                    }
                }
            },
            _ => return Err(CoreError::ItemNotFound(id.clone())),
        }
        Ok(())
    }

    pub fn remove_items(&mut self, page_id: &str, ids: &[ItemId]) -> Result<usize> {
        let items = self.items_map(page_id)?;
        let mut n = 0;
        for id in ids {
            if items.get(id).is_some() {
                items.delete(id)?;
                n += 1;
            }
        }
        if n > 0 {
            self.doc.commit();
        }
        Ok(n)
    }

    /// Move items by (dx, dy). Strokes accumulate an offset; box items move.
    pub fn translate_items(
        &mut self,
        page_id: &str,
        ids: &[ItemId],
        dx: f64,
        dy: f64,
    ) -> Result<()> {
        let items = self.items_map(page_id)?;
        for id in ids {
            Self::write_translate(&items, id, dx, dy)?;
        }
        self.doc.commit();
        Ok(())
    }

    fn write_translate(items: &LoroMap, id: &ItemId, dx: f64, dy: f64) -> Result<()> {
        match items.get(id) {
            Some(ValueOrContainer::Value(LoroValue::Map(v))) => {
                let mut f = ValueFields::from(&v);
                f.set("ox", f64_of(&v, "ox").unwrap_or(0.0) + dx);
                f.set("oy", f64_of(&v, "oy").unwrap_or(0.0) + dy);
                items.insert(id, f.into_value())?;
            }
            Some(ValueOrContainer::Container(Container::Map(m))) => {
                m.insert("x", f64_of(&m, "x").unwrap_or(0.0) + dx)?;
                m.insert("y", f64_of(&m, "y").unwrap_or(0.0) + dy)?;
            }
            _ => return Err(CoreError::ItemNotFound(id.clone())),
        }
        Ok(())
    }

    /// Apply several edits as **one** change (one undo step). Order:
    /// translate, rewrite strokes, box geometry, groups, text colours,
    /// remove, add.
    /// Returns the ids of the added items. Nothing is committed when an id
    /// is missing (the error is returned and the edit is rolled back by the
    /// caller's next undo-free commit; callers pass ids from the scene).
    pub fn apply_edits(&mut self, page_id: &str, e: &Edits) -> Result<Vec<ItemId>> {
        let items = self.items_map(page_id)?;
        for (id, dx, dy) in &e.translate {
            Self::write_translate(&items, id, *dx, *dy)?;
        }
        for (id, data) in &e.strokes {
            Self::write_stroke(&items, id, data)?;
        }
        for (id, g, size) in &e.boxes {
            let m = item_map(&items, id)?;
            write_geom(&m, g)?;
            if let Some(sz) = size
                && str_of(&m, "kind").as_deref() == Some("text")
            {
                m.insert("size", *sz)?;
            }
        }
        for (id, g) in &e.groups {
            Self::write_group(&items, id, g.as_deref())?;
        }
        for (id, rgba) in &e.text_colors {
            let m = item_map(&items, id)?;
            if str_of(&m, "kind").as_deref() == Some("text") {
                m.insert("color", i64::from(*rgba))?;
            }
        }
        for id in &e.remove {
            if items.get(id).is_some() {
                items.delete(id)?;
            }
        }
        let mut ids = Vec::with_capacity(e.add.len());
        for it in &e.add {
            ids.push(self.insert_item(
                page_id,
                &items,
                &it.data,
                it.created,
                it.group.as_deref(),
                it.z,
            )?);
        }
        if !e.is_empty() {
            self.doc.commit();
        }
        Ok(ids)
    }

    /// Set the geometry of a box item (text, image, object).
    pub fn set_geom(&mut self, page_id: &str, id: &str, g: BoxGeom) -> Result<()> {
        let m = item_map(&self.items_map(page_id)?, id)?;
        write_geom(&m, &g)?;
        self.doc.commit();
        Ok(())
    }

    pub fn bring_to_front(&mut self, page_id: &str, id: &str) -> Result<()> {
        self.bring_items_to_front(page_id, &[id.to_string()])
    }

    /// Raise several items above everything else on the page, keeping their
    /// order relative to each other, as one change.
    pub fn bring_items_to_front(&mut self, page_id: &str, ids: &[ItemId]) -> Result<()> {
        let items = self.items_map(page_id)?;
        let mut order: Vec<(f64, &ItemId)> = Vec::with_capacity(ids.len());
        for id in ids {
            let z = match items.get(id) {
                Some(ValueOrContainer::Value(LoroValue::Map(v))) => f64_of(&v, "z"),
                Some(ValueOrContainer::Container(Container::Map(m))) => f64_of(&m, "z"),
                _ => return Err(CoreError::ItemNotFound(id.clone())),
            };
            order.push((z.unwrap_or(0.0), id));
        }
        order.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(b.1)));
        order.dedup_by(|a, b| a.1 == b.1);
        for (_, id) in order {
            let z = self.next_z(page_id, &items);
            match items.get(id) {
                Some(ValueOrContainer::Value(LoroValue::Map(v))) => {
                    let mut f = ValueFields::from(&v);
                    f.set("z", z);
                    items.insert(id, f.into_value())?;
                }
                Some(ValueOrContainer::Container(Container::Map(m))) => m.insert("z", z)?,
                _ => return Err(CoreError::ItemNotFound(id.clone())),
            }
        }
        self.doc.commit();
        Ok(())
    }

    /// Resize a text item: new box and font size in one change.
    pub fn resize_text(&mut self, page_id: &str, id: &str, g: BoxGeom, size: f64) -> Result<()> {
        let m = item_map(&self.items_map(page_id)?, id)?;
        if str_of(&m, "kind").as_deref() != Some("text") {
            return Err(CoreError::ItemNotFound(id.to_string()));
        }
        write_geom(&m, &g)?;
        m.insert("size", size)?;
        self.doc.commit();
        Ok(())
    }

    /// Rewrite strokes in place (points, style, `shape`, `geom`, offset) and
    /// remove items, as **one** change: a Smart Ink snap or a shape resize is
    /// one undo step, and undo brings the previous strokes back. A rewritten
    /// stroke keeps its id, `z`, `created`, `group` and any keys this build
    /// does not know.
    pub fn edit_strokes(
        &mut self,
        page_id: &str,
        replace: &[(ItemId, StrokeData)],
        remove: &[ItemId],
    ) -> Result<()> {
        let items = self.items_map(page_id)?;
        for (id, data) in replace {
            Self::write_stroke(&items, id, data)?;
        }
        for id in remove {
            if items.get(id).is_some() {
                items.delete(id)?;
            }
        }
        if !replace.is_empty() || !remove.is_empty() {
            self.doc.commit();
        }
        Ok(())
    }

    fn write_stroke(items: &LoroMap, id: &ItemId, data: &StrokeData) -> Result<()> {
        let Some(ValueOrContainer::Value(LoroValue::Map(v))) = items.get(id) else {
            return Err(CoreError::ItemNotFound(id.clone()));
        };
        if v.get("kind") != Some(&LoroValue::from("stroke")) {
            return Err(CoreError::ItemNotFound(id.clone()));
        }
        let mut f = ValueFields::from(&v);
        for key in ["ox", "oy", "shape", "geom"] {
            f.remove(key);
        }
        write_item_data(&mut f, &ItemData::Stroke(data.clone()))?;
        items.insert(id, f.into_value())?;
        Ok(())
    }

    /// Remove items and add one new item on top, as **one** change (ink
    /// converted to text: one undo brings the ink back). Returns the new id.
    pub fn replace_items(
        &mut self,
        page_id: &str,
        remove: &[ItemId],
        add: &ItemData,
        created: f64,
    ) -> Result<ItemId> {
        let items = self.items_map(page_id)?;
        for id in remove {
            if items.get(id).is_some() {
                items.delete(id)?;
            }
        }
        let id = self.insert_item(page_id, &items, add, created, None, None)?;
        self.doc.commit();
        Ok(id)
    }

    pub fn set_text(&mut self, page_id: &str, id: &str, text: &str) -> Result<()> {
        item_map(&self.items_map(page_id)?, id)?.insert("text", text)?;
        self.doc.commit();
        Ok(())
    }

    /// Recolour a text item (0xRRGGBBAA). Only text items; one commit.
    pub fn set_text_color(&mut self, page_id: &str, id: &str, rgba: u32) -> Result<()> {
        if !matches!(self.item(page_id, id)?.data, ItemData::Text(_)) {
            return Err(CoreError::ItemNotFound(id.to_string()));
        }
        item_map(&self.items_map(page_id)?, id)?.insert("color", i64::from(rgba))?;
        self.doc.commit();
        Ok(())
    }

    /// Change what a media item plays (its `media` map) and, when `poster`
    /// is given, its poster asset, as one change. Errors if `id` is not an
    /// image item or `media` is invalid.
    pub fn set_media(
        &mut self,
        page_id: &str,
        id: &str,
        media: &MediaData,
        poster: Option<&str>,
    ) -> Result<()> {
        if !matches!(self.item(page_id, id)?.data, ItemData::Image(_)) {
            return Err(CoreError::ItemNotFound(id.to_string()));
        }
        if !media.is_valid() {
            return Err(CoreError::Corrupt(format!("item {id}: invalid media")));
        }
        let m = item_map(&self.items_map(page_id)?, id)?;
        m.insert("media", media_value(media))?;
        if let Some(asset) = poster {
            m.insert("asset", asset)?;
        }
        self.doc.commit();
        Ok(())
    }

    pub fn set_object_step(&mut self, page_id: &str, id: &str, step: u32) -> Result<()> {
        item_map(&self.items_map(page_id)?, id)?.insert("step", i64::from(step))?;
        self.doc.commit();
        Ok(())
    }

    pub fn set_object_state(&mut self, page_id: &str, id: &str, state_json: &str) -> Result<()> {
        item_map(&self.items_map(page_id)?, id)?.insert("state", state_json)?;
        self.doc.commit();
        Ok(())
    }

    pub fn item(&self, page_id: &str, id: &str) -> Result<Item> {
        match self.items_map(page_id)?.get(id) {
            Some(ValueOrContainer::Value(LoroValue::Map(v))) => read_item(id, &v),
            Some(ValueOrContainer::Container(Container::Map(m))) => read_item(id, &m),
            _ => Err(CoreError::ItemNotFound(id.to_string())),
        }
    }

    /// All items of a page in stacking order (bottom first).
    pub fn items(&self, page_id: &str) -> Result<Vec<Item>> {
        let items = self.items_map(page_id)?;
        // One deep read of the whole page is far cheaper than per-key lookups.
        let LoroValue::Map(all) = items.get_deep_value() else {
            return Err(CoreError::Corrupt(format!(
                "page {page_id} items are not a map"
            )));
        };
        let mut out = Vec::with_capacity(all.len());
        for (k, v) in all.iter() {
            // Forward compatibility (spec § Versioning): an item this build
            // cannot read (unknown kind, undecodable points) is skipped, not
            // fatal. It stays in the document untouched.
            if let LoroValue::Map(fields) = v
                && let Ok(item) = read_item(k, fields)
            {
                out.push(item);
            }
        }
        out.sort_by(|a, b| a.z.total_cmp(&b.z).then_with(|| a.id.cmp(&b.id)));
        Ok(out)
    }

    /// Item ids in the order they were created — the replay sequence.
    pub fn timeline(&self, page_id: &str) -> Result<Vec<(f64, ItemId)>> {
        let mut t: Vec<_> = self
            .items(page_id)?
            .into_iter()
            .map(|i| (i.created, i.id))
            .collect();
        t.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        Ok(t)
    }

    // ---------------------------------------------------------------- undo

    pub fn undo(&mut self) -> Result<bool> {
        Ok(self.undo.undo()?)
    }

    pub fn redo(&mut self) -> Result<bool> {
        Ok(self.undo.redo()?)
    }

    pub fn can_undo(&self) -> bool {
        self.undo.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.undo.can_redo()
    }
}

// ------------------------------------------------------------------ helpers

fn base36(mut v: u64) -> String {
    const D: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    if v == 0 {
        return "0".into();
    }
    let mut s = Vec::new();
    while v > 0 {
        s.push(D[(v % 36) as usize]);
        v /= 36;
    }
    s.reverse();
    String::from_utf8(s).unwrap()
}

/// Read access shared by live maps and deep-value snapshots (the fast path
/// for reading a whole page at once).
trait Fields {
    fn val(&self, key: &str) -> Option<LoroValue>;
}

impl Fields for LoroMap {
    fn val(&self, key: &str) -> Option<LoroValue> {
        match self.get(key)? {
            ValueOrContainer::Value(v) => Some(v),
            _ => None,
        }
    }
}

impl Fields for loro::LoroMapValue {
    fn val(&self, key: &str) -> Option<LoroValue> {
        self.get(key).cloned()
    }
}

fn value_of(m: &impl Fields, key: &str) -> Option<LoroValue> {
    m.val(key)
}

fn str_of(m: &impl Fields, key: &str) -> Option<String> {
    match value_of(m, key)? {
        LoroValue::String(s) => Some(s.to_string()),
        _ => None,
    }
}

fn f64_of(m: &impl Fields, key: &str) -> Option<f64> {
    match value_of(m, key)? {
        LoroValue::Double(d) => Some(d),
        LoroValue::I64(i) => Some(i as f64),
        _ => None,
    }
}

fn u32_of(m: &impl Fields, key: &str) -> Option<u32> {
    match value_of(m, key)? {
        LoroValue::I64(i) => u32::try_from(i).ok(),
        LoroValue::Double(d) if d >= 0.0 && d <= f64::from(u32::MAX) => Some(d as u32),
        _ => None,
    }
}

fn item_map(items: &LoroMap, id: &str) -> Result<LoroMap> {
    match items.get(id) {
        Some(ValueOrContainer::Container(Container::Map(m))) => Ok(m),
        _ => Err(CoreError::ItemNotFound(id.to_string())),
    }
}

fn page_info(m: &LoroMap) -> PageInfo {
    let frame = match (f64_of(m, "frame_w"), f64_of(m, "frame_h")) {
        (Some(w), Some(h)) => Some(Frame { w, h }),
        _ => None,
    };
    let item_count = match m.get("items") {
        Some(ValueOrContainer::Container(Container::Map(i))) => i.len(),
        _ => 0,
    };
    PageInfo {
        id: str_of(m, "id").unwrap_or_default(),
        title: str_of(m, "title").unwrap_or_default(),
        frame,
        pattern: Pattern::parse(&str_of(m, "bg_pattern").unwrap_or_default()),
        bg_color: u32_of(m, "bg_color").unwrap_or(0xffffffff),
        item_count,
    }
}

/// Write access shared by live maps and plain map values.
trait FieldsMut {
    fn put(&mut self, key: &str, v: LoroValue) -> loro::LoroResult<()>;
    fn insert(&mut self, key: &str, v: impl Into<LoroValue>) -> loro::LoroResult<()> {
        self.put(key, v.into())
    }
}

struct MapFields<'a>(&'a LoroMap);

impl FieldsMut for MapFields<'_> {
    fn put(&mut self, key: &str, v: LoroValue) -> loro::LoroResult<()> {
        self.0.insert(key, v)
    }
}

#[derive(Default)]
struct ValueFields(std::collections::HashMap<String, LoroValue>);

impl ValueFields {
    fn from(v: &loro::LoroMapValue) -> Self {
        Self(v.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
    }
    fn set(&mut self, key: &str, v: impl Into<LoroValue>) {
        self.0.insert(key.to_string(), v.into());
    }
    fn remove(&mut self, key: &str) {
        self.0.remove(key);
    }
    fn into_value(self) -> LoroValue {
        LoroValue::from(self.0)
    }
}

impl FieldsMut for ValueFields {
    fn put(&mut self, key: &str, v: LoroValue) -> loro::LoroResult<()> {
        self.0.insert(key.to_string(), v);
        Ok(())
    }
}

fn write_geom(m: &LoroMap, g: &BoxGeom) -> loro::LoroResult<()> {
    write_geom_to(&mut MapFields(m), g)
}

fn write_geom_to(m: &mut impl FieldsMut, g: &BoxGeom) -> loro::LoroResult<()> {
    m.insert("x", g.x)?;
    m.insert("y", g.y)?;
    m.insert("w", g.w)?;
    m.insert("h", g.h)?;
    m.insert("rot", g.rot)?;
    Ok(())
}

fn read_geom(m: &impl Fields) -> BoxGeom {
    BoxGeom {
        x: f64_of(m, "x").unwrap_or(0.0),
        y: f64_of(m, "y").unwrap_or(0.0),
        w: f64_of(m, "w").unwrap_or(0.0),
        h: f64_of(m, "h").unwrap_or(0.0),
        rot: f64_of(m, "rot").unwrap_or(0.0),
    }
}

fn write_item_data(m: &mut impl FieldsMut, data: &ItemData) -> loro::LoroResult<()> {
    match data {
        ItemData::Stroke(s) => {
            m.insert("tool", s.tool.as_str())?;
            m.insert("color", i64::from(s.color))?;
            m.insert("width", f64::from(s.width))?;
            m.insert("opacity", f64::from(s.opacity))?;
            m.insert("pts", LoroValue::Binary(encode_points(&s.points).into()))?;
            if s.offset != [0.0, 0.0] {
                m.insert("ox", f64::from(s.offset[0]))?;
                m.insert("oy", f64::from(s.offset[1]))?;
            }
            if let Some(shape) = &s.shape {
                m.insert("shape", shape.as_str())?;
            }
            if let Some(g) = s.geom.as_ref().filter(|g| g.is_valid()) {
                m.insert("geom", shape_geom_value(g))?;
            }
        }
        ItemData::Text(t) => {
            write_geom_to(m, &t.geom)?;
            m.insert("text", t.text.as_str())?;
            m.insert("size", t.size)?;
            m.insert("color", i64::from(t.color))?;
            if t.latex {
                m.insert("latex", true)?;
            }
        }
        ItemData::Image(i) => {
            write_geom_to(m, &i.geom)?;
            m.insert("asset", i.asset.as_str())?;
            if let Some(media) = i.media.as_ref().filter(|x| x.is_valid()) {
                m.insert("media", media_value(media))?;
            }
        }
        ItemData::Object(o) => {
            write_geom_to(m, &o.geom)?;
            m.insert("bundle", o.bundle.as_str())?;
            m.insert("title", o.title.as_str())?;
            m.insert("step", i64::from(o.step))?;
            if let Some(st) = &o.state {
                m.insert("state", st.as_str())?;
            }
        }
    }
    Ok(())
}

/// The `geom` map of a shape stroke.
fn shape_geom_value(g: &ShapeGeom) -> LoroValue {
    let mut f = ValueFields::default();
    f.set("kind", g.kind.as_str());
    f.set("x", g.x);
    f.set("y", g.y);
    f.set("w", g.w);
    f.set("h", g.h);
    if g.rot != 0.0 {
        f.set("rot", g.rot);
    }
    if !g.v.is_empty() {
        f.set(
            "v",
            LoroValue::from(
                g.v.iter()
                    .map(|&x| LoroValue::Double(x))
                    .collect::<Vec<_>>(),
            ),
        );
    }
    if let Some(id) = &g.from {
        f.set("from", id.as_str());
    }
    if let Some(id) = &g.to {
        f.set("to", id.as_str());
    }
    f.into_value()
}

/// The `media` map of a media item (an image item that is a poster).
fn media_value(x: &MediaData) -> LoroValue {
    let mut f = ValueFields::default();
    f.set("type", x.kind.as_str());
    f.set("src", x.src.as_str());
    if !x.title.is_empty() {
        f.set("title", x.title.as_str());
    }
    if !x.site.is_empty() {
        f.set("site", x.site.as_str());
    }
    if let Some(t) = x.start {
        f.set("start", t);
    }
    if let Some(t) = x.end {
        f.set("end", t);
    }
    for (key, on) in [
        ("muted", x.muted),
        ("loop", x.looped),
        ("autoplay", x.autoplay),
    ] {
        if on {
            f.set(key, true);
        }
    }
    f.into_value()
}

/// Read `media`; anything malformed (or a kind this build does not play)
/// reads as absent, so the item is shown as its poster image.
fn read_media(m: &impl Fields) -> Option<MediaData> {
    media_from_value(&value_of(m, "media")?)
}

/// Parse an item's `media` value (see [`MediaData`]); `None` when it is
/// malformed or names a kind this build does not play.
pub fn media_from_value(v: &LoroValue) -> Option<MediaData> {
    let LoroValue::Map(g) = v else {
        return None;
    };
    let flag = |k: &str| matches!(g.get(k), Some(LoroValue::Bool(true)));
    let media = MediaData {
        kind: str_of(g, "type")?,
        src: str_of(g, "src")?,
        title: str_of(g, "title").unwrap_or_default(),
        site: str_of(g, "site").unwrap_or_default(),
        start: f64_of(g, "start"),
        end: f64_of(g, "end"),
        muted: flag("muted"),
        looped: flag("loop"),
        autoplay: flag("autoplay"),
    };
    media.is_valid().then_some(media)
}

/// Read `geom`; anything malformed reads as absent (the points still draw).
fn read_shape_geom(m: &impl Fields) -> Option<ShapeGeom> {
    let LoroValue::Map(g) = value_of(m, "geom")? else {
        return None;
    };
    let v = match g.get("v") {
        Some(LoroValue::List(l)) if l.len() <= 2 * ShapeGeom::MAX_VERTICES => l
            .iter()
            .map(|x| match x {
                LoroValue::Double(d) => Some(*d),
                LoroValue::I64(i) => Some(*i as f64),
                _ => None,
            })
            .collect::<Option<Vec<f64>>>()?,
        Some(_) => return None,
        None => Vec::new(),
    };
    let geom = ShapeGeom {
        kind: str_of(&g, "kind")?,
        x: f64_of(&g, "x")?,
        y: f64_of(&g, "y")?,
        w: f64_of(&g, "w")?,
        h: f64_of(&g, "h")?,
        rot: f64_of(&g, "rot").unwrap_or(0.0),
        v,
        // A malformed link reads as no link (the connector stays put).
        from: str_of(&g, "from").filter(|s| !s.is_empty() && s.len() <= 256),
        to: str_of(&g, "to").filter(|s| !s.is_empty() && s.len() <= 256),
    };
    geom.is_valid().then_some(geom)
}

fn read_item(id: &str, m: &impl Fields) -> Result<Item> {
    let kind =
        str_of(m, "kind").ok_or_else(|| CoreError::Corrupt(format!("item {id} has no kind")))?;
    let data = match kind.as_str() {
        "stroke" => {
            let pts = match value_of(m, "pts") {
                Some(LoroValue::Binary(b)) => {
                    decode_points(&b).map_err(|e| CoreError::Corrupt(format!("item {id}: {e}")))?
                }
                _ => Vec::new(),
            };
            ItemData::Stroke(StrokeData {
                tool: str_of(m, "tool").unwrap_or_else(|| "pen".into()),
                color: u32_of(m, "color").unwrap_or(0x000000ff),
                width: f64_of(m, "width").unwrap_or(2.0) as f32,
                opacity: f64_of(m, "opacity").unwrap_or(1.0) as f32,
                points: pts,
                offset: [
                    f64_of(m, "ox").unwrap_or(0.0) as f32,
                    f64_of(m, "oy").unwrap_or(0.0) as f32,
                ],
                shape: str_of(m, "shape"),
                geom: read_shape_geom(m),
            })
        }
        "text" => ItemData::Text(TextData {
            latex: matches!(value_of(m, "latex"), Some(LoroValue::Bool(true))),
            geom: read_geom(m),
            text: str_of(m, "text").unwrap_or_default(),
            size: f64_of(m, "size").unwrap_or(32.0),
            color: u32_of(m, "color").unwrap_or(0x000000ff),
        }),
        "image" => ItemData::Image(ImageData {
            geom: read_geom(m),
            asset: str_of(m, "asset").unwrap_or_default(),
            media: read_media(m),
        }),
        "object" => ItemData::Object(ObjectData {
            geom: read_geom(m),
            bundle: str_of(m, "bundle").unwrap_or_default(),
            title: str_of(m, "title").unwrap_or_default(),
            step: u32_of(m, "step").unwrap_or(0),
            state: str_of(m, "state"),
        }),
        other => {
            return Err(CoreError::Corrupt(format!(
                "item {id}: unknown kind {other}"
            )));
        }
    };
    Ok(Item {
        id: id.to_string(),
        z: f64_of(m, "z").unwrap_or(0.0),
        created: f64_of(m, "created").unwrap_or(0.0),
        group: str_of(m, "group").filter(|g| !g.is_empty()),
        data,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rupok_ink::StrokePoint;

    fn stroke(x0: f32) -> ItemData {
        ItemData::Stroke(StrokeData {
            tool: "pen".into(),
            color: 0x112233ff,
            width: 4.0,
            opacity: 1.0,
            points: (0..20)
                .map(|i| StrokePoint {
                    x: x0 + i as f32,
                    y: 5.0,
                    p: 0.5,
                    dt: i * 8,
                })
                .collect(),
            offset: [0.0, 0.0],
            shape: None,
            geom: None,
        })
    }

    #[test]
    fn new_lesson_has_one_page() {
        let l = Lesson::new("History of Computing", 1.0);
        assert_eq!(l.title(), "History of Computing");
        assert_eq!(l.page_count(), 1);
        assert_eq!(l.pages()[0].frame, Some(Frame::SLIDE_16_9));
        assert!(!l.can_undo());
    }

    #[test]
    fn items_round_trip_through_snapshot() {
        let mut l = Lesson::new("t", 0.0);
        let p = l.page_id_at(0).unwrap();
        let a = l.add_item(&p, &stroke(0.0), 10.0).unwrap();
        let obj = ItemData::Object(ObjectData {
            geom: BoxGeom {
                x: 10.0,
                y: 20.0,
                w: 640.0,
                h: 360.0,
                rot: 0.0,
            },
            bundle: "timeline-1".into(),
            title: "Timeline".into(),
            step: 0,
            state: None,
        });
        let b = l.add_item(&p, &obj, 20.0).unwrap();
        let bytes = l.save();
        let l2 = Lesson::load(&bytes).unwrap();
        let items = l2.items(&p).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id, a);
        assert_eq!(items[1].id, b);
        assert_eq!(items[1].data, obj);
        match &items[0].data {
            ItemData::Stroke(s) => assert_eq!(s.points.len(), 20),
            _ => panic!(),
        }
    }

    #[test]
    fn undo_redo_per_stroke() {
        let mut l = Lesson::new("t", 0.0);
        let p = l.page_id_at(0).unwrap();
        l.add_item(&p, &stroke(0.0), 1.0).unwrap();
        l.add_item(&p, &stroke(50.0), 2.0).unwrap();
        assert_eq!(l.items(&p).unwrap().len(), 2);
        assert!(l.undo().unwrap());
        assert_eq!(l.items(&p).unwrap().len(), 1);
        assert!(l.redo().unwrap());
        assert_eq!(l.items(&p).unwrap().len(), 2);
    }

    fn shape_stroke() -> StrokeData {
        let ItemData::Stroke(mut s) = stroke(100.0) else {
            unreachable!()
        };
        // Exact under the codec's pressure quantisation.
        s.points.iter_mut().for_each(|q| q.p = 1.0);
        s.shape = Some("triangle".into());
        s.geom = Some(ShapeGeom {
            kind: "poly".into(),
            x: 100.0,
            y: 0.0,
            w: 40.0,
            h: 30.0,
            rot: 0.25,
            v: vec![0.5, 0.0, 1.0, 1.0, 0.0, 1.0],
            from: None,
            to: Some("box-2".into()),
        });
        s
    }

    #[test]
    fn shape_geom_round_trips_and_bad_geom_reads_as_absent() {
        let mut l = Lesson::new("t", 0.0);
        let p = l.page_id_at(0).unwrap();
        let s = shape_stroke();
        let id = l.add_item(&p, &ItemData::Stroke(s.clone()), 1.0).unwrap();
        let back = Lesson::load(&l.save()).unwrap().item(&p, &id).unwrap();
        assert_eq!(back.data, ItemData::Stroke(s.clone()));
        // A move keeps geom (it lives in the points' frame).
        l.translate_items(&p, std::slice::from_ref(&id), 5.0, 0.0)
            .unwrap();
        let ItemData::Stroke(moved) = l.item(&p, &id).unwrap().data else {
            panic!()
        };
        assert_eq!(moved.geom, s.geom);
        // Invalid geom is never written, and reads as None.
        let mut bad = s.clone();
        bad.geom.as_mut().unwrap().w = f64::NAN;
        let id2 = l.add_item(&p, &ItemData::Stroke(bad), 2.0).unwrap();
        let ItemData::Stroke(b) = l.item(&p, &id2).unwrap().data else {
            panic!()
        };
        assert_eq!(b.geom, None);
        assert_eq!(b.shape.as_deref(), Some("triangle"));
    }

    #[test]
    fn latex_text_round_trips_and_plain_text_has_no_key() {
        let mut l = Lesson::new("t", 0.0);
        let p = l.page_id_at(0).unwrap();
        let t = |latex| TextData {
            latex,
            geom: BoxGeom {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 40.0,
                rot: 0.0,
            },
            text: "x^2".into(),
            size: 32.0,
            color: 0x000000ff,
        };
        let a = l.add_item(&p, &ItemData::Text(t(true)), 1.0).unwrap();
        let b = l.add_item(&p, &ItemData::Text(t(false)), 2.0).unwrap();
        let back = Lesson::load(&l.save()).unwrap();
        assert_eq!(back.item(&p, &a).unwrap().data, ItemData::Text(t(true)));
        assert_eq!(back.item(&p, &b).unwrap().data, ItemData::Text(t(false)));
        // Editing the source keeps it maths.
        l.set_text(&p, &a, "x^3").unwrap();
        let ItemData::Text(td) = l.item(&p, &a).unwrap().data else {
            panic!()
        };
        assert!(td.latex && td.text == "x^3");
    }

    #[test]
    fn apply_edits_is_one_undo_step() {
        let mut l = Lesson::new("t", 0.0);
        let p = l.page_id_at(0).unwrap();
        let a = l
            .add_item(&p, &ItemData::Stroke(shape_stroke()), 1.0)
            .unwrap();
        let b = l
            .add_item(&p, &ItemData::Stroke(shape_stroke()), 2.0)
            .unwrap();
        let text = ItemData::Text(TextData {
            latex: false,
            geom: BoxGeom {
                x: 0.0,
                y: 0.0,
                w: 40.0,
                h: 30.0,
                rot: 0.0,
            },
            text: "x".into(),
            size: 24.0,
            color: 0x000000ff,
        });
        let t = l.add_item(&p, &text, 3.0).unwrap();
        let mut moved = shape_stroke();
        moved.width = 9.0;
        let added = l
            .apply_edits(
                &p,
                &Edits {
                    translate: vec![(t.clone(), 10.0, 5.0)],
                    strokes: vec![(a.clone(), moved)],
                    boxes: vec![(
                        t.clone(),
                        BoxGeom {
                            x: 50.0,
                            y: 60.0,
                            w: 40.0,
                            h: 30.0,
                            rot: 0.0,
                        },
                        Some(48.0),
                    )],
                    groups: vec![
                        (a.clone(), Some("g1".into())),
                        (t.clone(), Some("g1".into())),
                    ],
                    remove: vec![b.clone()],
                    add: vec![NewItem {
                        data: text.clone(),
                        created: 4.0,
                        group: Some("g1".into()),
                        z: None,
                    }],
                    text_colors: vec![],
                },
            )
            .unwrap();
        assert_eq!(added.len(), 1);
        let items = l.items(&p).unwrap();
        assert_eq!(items.len(), 3);
        assert!(items.iter().all(|i| i.group.as_deref() == Some("g1")));
        let ItemData::Text(td) = &l.item(&p, &t).unwrap().data else {
            panic!()
        };
        assert_eq!((td.geom.x, td.size), (50.0, 48.0));
        // One undo step brings everything back.
        assert!(l.undo().unwrap());
        let items = l.items(&p).unwrap();
        assert_eq!(items.len(), 3);
        assert!(items.iter().all(|i| i.group.is_none()));
        assert!(items.iter().any(|i| i.id == b));
        // A missing id is an error.
        assert!(
            l.apply_edits(
                &p,
                &Edits {
                    remove: vec![],
                    groups: vec![("nope".into(), None)],
                    ..Default::default()
                }
            )
            .is_err()
        );
    }

    #[test]
    fn apply_edits_keeps_a_given_z_and_recolours_text_in_one_step() {
        let mut l = Lesson::new("t", 0.0);
        let p = l.page_id_at(0).unwrap();
        let a = l.add_item(&p, &stroke(0.0), 1.0).unwrap();
        let t = l.add_item(&p, &text_item(0.0), 2.0).unwrap();
        let z = l.item(&p, &a).unwrap().z;
        let ids = l
            .apply_edits(
                &p,
                &Edits {
                    remove: vec![a.clone()],
                    add: vec![
                        NewItem {
                            data: stroke(1.0),
                            created: 1.0,
                            group: None,
                            z: Some(z),
                        },
                        NewItem {
                            data: stroke(2.0),
                            created: 1.0,
                            group: None,
                            z: None,
                        },
                    ],
                    text_colors: vec![(t.clone(), 0xe5484dff)],
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(
            l.item(&p, &ids[0]).unwrap().z,
            z,
            "the piece keeps its place"
        );
        assert!(l.item(&p, &ids[1]).unwrap().z > z, "no z: on top");
        let ItemData::Text(td) = l.item(&p, &t).unwrap().data else {
            panic!("text")
        };
        assert_eq!(td.color, 0xe5484dff);
        assert!(l.undo().unwrap());
        assert!(l.item(&p, &a).is_ok(), "one undo step");
    }

    #[test]
    fn edit_strokes_is_one_undo_step_and_keeps_identity() {
        let mut l = Lesson::new("t", 0.0);
        let p = l.page_id_at(0).unwrap();
        let raw = l.add_item(&p, &stroke(0.0), 10.0).unwrap();
        let other = l.add_item(&p, &stroke(50.0), 20.0).unwrap();
        l.set_group(&p, std::slice::from_ref(&raw), Some("g1"))
            .unwrap();
        let before = l.item(&p, &raw).unwrap();
        let shape = shape_stroke();
        l.edit_strokes(
            &p,
            &[(raw.clone(), shape.clone())],
            std::slice::from_ref(&other),
        )
        .unwrap();
        let after = l.item(&p, &raw).unwrap();
        assert_eq!(after.data, ItemData::Stroke(shape));
        assert_eq!(
            (after.z, after.created, after.group.as_deref()),
            (before.z, 10.0, Some("g1"))
        );
        assert!(l.item(&p, &other).is_err());
        // One undo restores the hand-drawn stroke and the removed one.
        assert!(l.undo().unwrap());
        assert_eq!(l.item(&p, &raw).unwrap().data, before.data);
        assert!(l.item(&p, &other).is_ok());
        // Box items cannot be rewritten as strokes.
        let t = l.add_item(&p, &text_item(0.0), 30.0).unwrap();
        assert!(l.edit_strokes(&p, &[(t, shape_stroke())], &[]).is_err());
    }

    #[test]
    fn replace_items_swaps_ink_for_text_in_one_step() {
        let mut l = Lesson::new("t", 0.0);
        let p = l.page_id_at(0).unwrap();
        let a = l.add_item(&p, &stroke(0.0), 1.0).unwrap();
        let b = l.add_item(&p, &stroke(30.0), 2.0).unwrap();
        let t = l
            .replace_items(&p, &[a.clone(), b.clone()], &text_item(0.0), 3.0)
            .unwrap();
        let items = l.items(&p).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, t);
        assert!(l.undo().unwrap());
        let ids: Vec<String> = l.items(&p).unwrap().into_iter().map(|i| i.id).collect();
        assert_eq!(ids, vec![a, b]);
    }

    #[test]
    fn translate_moves_strokes_without_rewriting_points() {
        let mut l = Lesson::new("t", 0.0);
        let p = l.page_id_at(0).unwrap();
        let id = l.add_item(&p, &stroke(0.0), 1.0).unwrap();
        l.translate_items(&p, std::slice::from_ref(&id), 100.0, -10.0)
            .unwrap();
        let it = l.item(&p, &id).unwrap();
        match it.data {
            ItemData::Stroke(s) => {
                assert_eq!(s.offset, [100.0, -10.0]);
                assert_eq!(s.points[0].x, 0.0);
            }
            _ => panic!(),
        }
    }

    #[test]
    fn concurrent_edits_merge() {
        let mut teacher_pc = Lesson::new("t", 0.0);
        let p = teacher_pc.page_id_at(0).unwrap();
        let mut board = Lesson::load(&teacher_pc.save()).unwrap();
        // Offline on both devices:
        teacher_pc.add_item(&p, &stroke(0.0), 1.0).unwrap();
        board.add_item(&p, &stroke(100.0), 2.0).unwrap();
        board.add_page(None, PageSpec::default()).unwrap();
        // Sync both ways.
        let vb = board.version();
        let vt = teacher_pc.version();
        board.import(&teacher_pc.export_updates(&vb)).unwrap();
        teacher_pc.import(&board.export_updates(&vt)).unwrap();
        assert_eq!(teacher_pc.items(&p).unwrap().len(), 2);
        assert_eq!(board.items(&p).unwrap().len(), 2);
        assert_eq!(teacher_pc.page_count(), 2);
        let ids_t: Vec<_> = teacher_pc
            .items(&p)
            .unwrap()
            .into_iter()
            .map(|i| i.id)
            .collect();
        let ids_b: Vec<_> = board.items(&p).unwrap().into_iter().map(|i| i.id).collect();
        assert_eq!(ids_t, ids_b, "replicas must agree on stacking order");
    }

    #[test]
    fn timeline_is_creation_ordered() {
        let mut l = Lesson::new("t", 0.0);
        let p = l.page_id_at(0).unwrap();
        let a = l.add_item(&p, &stroke(0.0), 300.0).unwrap();
        let b = l.add_item(&p, &stroke(0.0), 100.0).unwrap();
        l.bring_to_front(&p, &a).unwrap();
        let t = l.timeline(&p).unwrap();
        assert_eq!(
            t.iter().map(|x| x.1.clone()).collect::<Vec<_>>(),
            vec![b, a]
        );
    }

    #[test]
    fn pages_can_be_reordered_and_removed() {
        let mut l = Lesson::new("t", 0.0);
        let p1 = l.page_id_at(0).unwrap();
        let p2 = l
            .add_page(
                None,
                PageSpec {
                    title: "two".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        l.move_page(1, 0).unwrap();
        assert_eq!(l.page_id_at(0).unwrap(), p2);
        l.remove_page(&p1).unwrap();
        assert_eq!(l.page_count(), 1);
        assert_eq!(l.pages()[0].title, "two");
    }

    fn text_item(x: f64) -> ItemData {
        ItemData::Text(TextData {
            latex: false,
            geom: BoxGeom {
                x,
                y: 10.0,
                w: 300.0,
                h: 60.0,
                rot: 0.0,
            },
            text: "Hello".into(),
            size: 40.0,
            color: 0x112233ff,
        })
    }

    #[test]
    fn unknown_kinds_and_bad_points_are_skipped_not_fatal() {
        let mut l = Lesson::new("t", 0.0);
        let p = l.page_id_at(0).unwrap();
        let good = l.add_item(&p, &stroke(0.0), 1.0).unwrap();
        let items = l.items_map(&p).unwrap();
        // A future item kind stored as a value…
        let mut future = ValueFields::default();
        future.set("kind", "hologram");
        future.set("z", 5.0);
        future.set("created", 2.0);
        items.insert("future-1", future.into_value()).unwrap();
        // …a future kind stored as a container…
        let c = items.insert_container("future-2", LoroMap::new()).unwrap();
        c.insert("kind", "sticker").unwrap();
        c.insert("z", 6.0).unwrap();
        // …an item without a kind…
        let mut nokind = ValueFields::default();
        nokind.set("z", 7.0);
        items.insert("nokind", nokind.into_value()).unwrap();
        // …and a stroke whose points use an unknown codec version.
        let mut bad = ValueFields::default();
        bad.set("kind", "stroke");
        bad.set("z", 8.0);
        bad.set("pts", LoroValue::Binary(vec![99u8, 1, 2, 3].into()));
        items.insert("badpts", bad.into_value()).unwrap();
        l.doc.commit();

        let read = l.items(&p).unwrap();
        assert_eq!(read.len(), 1, "only the readable item is returned");
        assert_eq!(read[0].id, good);
        assert_eq!(l.timeline(&p).unwrap().len(), 1);
        // Unknown data survives a save/load round trip untouched.
        let l2 = Lesson::load(&l.save()).unwrap();
        assert_eq!(l2.items(&p).unwrap().len(), 1);
        assert_eq!(l2.page(&p).unwrap().item_count, 5);
        assert!(l2.item(&p, "future-1").is_err());
    }

    #[test]
    fn add_items_is_one_undo_step_in_order() {
        let mut l = Lesson::new("t", 0.0);
        let p = l.page_id_at(0).unwrap();
        l.add_item(&p, &stroke(0.0), 1.0).unwrap();
        let new = |data, created| NewItem {
            data,
            created,
            group: None,
            z: None,
        };
        let ids = l
            .add_items(&p, &[new(stroke(10.0), 5.0), new(text_item(0.0), 6.0)])
            .unwrap();
        assert_eq!(ids.len(), 2);
        let items = l.items(&p).unwrap();
        assert_eq!(items.len(), 3);
        assert_eq!(items[1].id, ids[0]);
        assert_eq!(items[2].id, ids[1]);
        assert_eq!(items[2].created, 6.0);
        assert!(l.undo().unwrap());
        assert_eq!(l.items(&p).unwrap().len(), 1, "one undo removes the batch");
        assert!(l.add_items(&p, &[]).unwrap().is_empty());
    }

    #[test]
    fn duplicate_page_copies_everything_after_the_source() {
        let mut l = Lesson::new("t", 0.0);
        let p1 = l.page_id_at(0).unwrap();
        l.add_page(None, PageSpec::default()).unwrap();
        l.set_page_title(&p1, "Intro").unwrap();
        l.set_page_background(&p1, Pattern::Grid, 0x1f3b30ff)
            .unwrap();
        l.add_item(&p1, &stroke(0.0), 10.0).unwrap();
        let obj = ItemData::Object(ObjectData {
            geom: BoxGeom {
                x: 1.0,
                y: 2.0,
                w: 3.0,
                h: 4.0,
                rot: 0.0,
            },
            bundle: "quiz".into(),
            title: "Quiz".into(),
            step: 2,
            state: Some("{\"a\":1}".into()),
        });
        l.add_item(&p1, &obj, 20.0).unwrap();
        let mut future = ValueFields::default();
        future.set("kind", "hologram");
        l.items_map(&p1)
            .unwrap()
            .insert("future", future.into_value())
            .unwrap();
        l.doc.commit();

        let copy = l.duplicate_page(&p1).unwrap();
        assert_eq!(l.page_count(), 3);
        assert_eq!(l.page_id_at(1).unwrap(), copy, "inserted right after");
        let info = l.page(&copy).unwrap();
        assert_eq!(info.title, "Intro");
        assert_eq!(info.pattern, Pattern::Grid);
        assert_eq!(info.bg_color, 0x1f3b30ff);
        assert_eq!(info.frame, Some(Frame::SLIDE_16_9));
        assert_eq!(info.item_count, 3, "unknown items are copied too");
        let a = l.items(&p1).unwrap();
        let b = l.items(&copy).unwrap();
        assert_eq!(b.len(), 2);
        for (x, y) in a.iter().zip(&b) {
            assert_ne!(x.id, y.id, "fresh ids");
            assert_eq!(x.data, y.data);
            assert_eq!(x.z, y.z);
            assert_eq!(x.created, y.created);
        }
        // Box items stay editable containers in the copy.
        l.set_object_step(&copy, &b[1].id, 3).unwrap();
        assert!(matches!(&l.item(&p1, &a[1].id).unwrap().data, ItemData::Object(o) if o.step == 2));
        assert!(l.undo().unwrap());
        assert!(l.undo().unwrap());
        assert_eq!(l.page_count(), 2, "duplicating is one undo step");
    }

    #[test]
    fn bring_items_to_front_keeps_relative_order() {
        let mut l = Lesson::new("t", 0.0);
        let p = l.page_id_at(0).unwrap();
        let a = l.add_item(&p, &stroke(0.0), 1.0).unwrap();
        let b = l.add_item(&p, &text_item(0.0), 2.0).unwrap();
        let c = l.add_item(&p, &stroke(5.0), 3.0).unwrap();
        l.bring_items_to_front(&p, &[c.clone(), a.clone(), a.clone()])
            .unwrap();
        let order: Vec<_> = l.items(&p).unwrap().into_iter().map(|i| i.id).collect();
        assert_eq!(order, vec![b, a, c]);
        assert!(l.bring_items_to_front(&p, &["nope".into()]).is_err());
    }

    #[test]
    fn set_text_color_recolours_text_only_in_one_step() {
        let mut l = Lesson::new("t", 0.0);
        let p = l.page_id_at(0).unwrap();
        let t = l.add_item(&p, &text_item(0.0), 1.0).unwrap();
        l.set_text_color(&p, &t, 0xe5484dff).unwrap();
        assert!(
            matches!(l.item(&p, &t).unwrap().data, ItemData::Text(td) if td.color == 0xe5484dff)
        );
        assert!(l.undo().unwrap());
        assert!(
            matches!(l.item(&p, &t).unwrap().data, ItemData::Text(td) if td.color == 0x112233ff)
        );
        assert!(l.set_text_color(&p, "nope", 0).is_err());
    }

    #[test]
    fn resize_text_sets_box_and_size_in_one_change() {
        let mut l = Lesson::new("t", 0.0);
        let p = l.page_id_at(0).unwrap();
        let t = l.add_item(&p, &text_item(0.0), 1.0).unwrap();
        let g = BoxGeom {
            x: 5.0,
            y: 6.0,
            w: 600.0,
            h: 120.0,
            rot: 0.0,
        };
        l.resize_text(&p, &t, g, 80.0).unwrap();
        match l.item(&p, &t).unwrap().data {
            ItemData::Text(td) => {
                assert_eq!(td.geom, g);
                assert_eq!(td.size, 80.0);
            }
            _ => panic!(),
        }
        assert!(l.undo().unwrap());
        assert!(matches!(l.item(&p, &t).unwrap().data, ItemData::Text(td) if td.size == 40.0));
        let s = l
            .add_item(
                &p,
                &ItemData::Object(ObjectData {
                    geom: g,
                    bundle: "b".into(),
                    title: String::new(),
                    step: 0,
                    state: None,
                }),
                2.0,
            )
            .unwrap();
        assert!(l.resize_text(&p, &s, g, 10.0).is_err(), "only text items");
    }

    #[test]
    fn groups_are_stored_on_items_and_dissolve_in_one_step() {
        let mut l = Lesson::new("t", 0.0);
        let p = l.page_id_at(0).unwrap();
        let a = l.add_item(&p, &stroke(0.0), 1.0).unwrap();
        let b = l.add_item(&p, &text_item(0.0), 2.0).unwrap();
        let c = l.add_item(&p, &stroke(9.0), 3.0).unwrap();
        let a_data = l.item(&p, &a).unwrap().data;
        let g = l.group_items(&p, &[a.clone(), b.clone()]).unwrap();
        let read = |l: &Lesson| -> Vec<Option<String>> {
            l.items(&p).unwrap().into_iter().map(|i| i.group).collect()
        };
        assert_eq!(read(&l), vec![Some(g.clone()), Some(g.clone()), None]);
        // Survives save/load; the stroke's points are untouched.
        let l2 = Lesson::load(&l.save()).unwrap();
        assert_eq!(read(&l2), read(&l));
        assert_eq!(l2.item(&p, &a).unwrap().data, a_data);
        // Regrouping moves items to a new group; ungroup dissolves it.
        let g2 = l.group_items(&p, &[b.clone(), c.clone()]).unwrap();
        assert_ne!(g, g2);
        assert_eq!(l.ungroup(&p, &g2).unwrap(), 2);
        assert_eq!(read(&l), vec![Some(g.clone()), None, None]);
        assert!(l.undo().unwrap());
        assert_eq!(read(&l), vec![Some(g), Some(g2.clone()), Some(g2)]);
        assert_eq!(l.ungroup(&p, "nope").unwrap(), 0);
        assert!(l.group_items(&p, &["missing".into()]).is_err());
        // Batch adds can carry a group.
        let ids = l
            .add_items(
                &p,
                &[NewItem {
                    data: stroke(1.0),
                    created: 9.0,
                    group: Some("gx".into()),
                    z: None,
                }],
            )
            .unwrap();
        assert_eq!(l.item(&p, &ids[0]).unwrap().group.as_deref(), Some("gx"));
    }

    #[test]
    fn rejects_garbage_and_future_formats() {
        assert!(Lesson::load(b"not a loro doc").is_err());
        let doc = LoroDoc::new();
        doc.get_map("meta").insert("format", 999).unwrap();
        doc.commit();
        assert!(Lesson::load(&doc.export(ExportMode::Snapshot).unwrap()).is_err());
    }

    fn youtube() -> MediaData {
        MediaData {
            kind: "youtube".into(),
            src: "https://www.youtube.com/watch?v=dQw4w9WgXcQ".into(),
            title: "Photosynthesis".into(),
            start: Some(30.0),
            end: Some(95.5),
            muted: true,
            looped: true,
            ..MediaData::default()
        }
    }

    fn media_item(media: Option<MediaData>) -> ItemData {
        ItemData::Image(ImageData {
            geom: BoxGeom {
                x: 100.0,
                y: 50.0,
                w: 1280.0,
                h: 720.0,
                rot: 0.0,
            },
            asset: "ab.png".into(),
            media,
        })
    }

    #[test]
    fn media_round_trips_through_snapshot() {
        let mut l = Lesson::new("t", 0.0);
        let p = l.page_id_at(0).unwrap();
        let a = l.add_item(&p, &media_item(Some(youtube())), 1.0).unwrap();
        let clip = MediaData {
            kind: "video".into(),
            src: "cd.mp4".into(),
            autoplay: true,
            ..MediaData::default()
        };
        let b = l
            .add_item(&p, &media_item(Some(clip.clone())), 2.0)
            .unwrap();
        let back = Lesson::load(&l.save()).unwrap();
        let ItemData::Image(i) = back.item(&p, &a).unwrap().data else {
            panic!("not an image")
        };
        assert_eq!(i.media, Some(youtube()));
        assert_eq!(i.asset, "ab.png");
        let ItemData::Image(i) = back.item(&p, &b).unwrap().data else {
            panic!("not an image")
        };
        assert_eq!(i.media, Some(clip));
        assert_eq!(i.media.as_ref().unwrap().src_asset(), Some("cd.mp4"));
        // Flags that are off and empty strings are not written.
        let items = l.items_map(&p).unwrap();
        let LoroValue::Map(raw) = item_map(&items, &b).unwrap().get_deep_value() else {
            panic!()
        };
        let Some(LoroValue::Map(m)) = raw.get("media") else {
            panic!("no media map")
        };
        assert_eq!(m.get("type"), Some(&LoroValue::from("video")));
        assert!(m.get("muted").is_none() && m.get("title").is_none());
        assert_eq!(raw.get("kind"), Some(&LoroValue::from("image")));
    }

    #[test]
    fn image_without_media_reads_as_plain_image() {
        // A file from before media items: no `media` key.
        let mut l = Lesson::new("t", 0.0);
        let p = l.page_id_at(0).unwrap();
        let a = l.add_item(&p, &media_item(None), 1.0).unwrap();
        let back = Lesson::load(&l.save()).unwrap();
        assert_eq!(back.item(&p, &a).unwrap().data, media_item(None));
    }

    #[test]
    fn malformed_or_unknown_media_reads_as_poster() {
        let mut l = Lesson::new("t", 0.0);
        let p = l.page_id_at(0).unwrap();
        let a = l.add_item(&p, &media_item(None), 1.0).unwrap();
        let items = l.items_map(&p).unwrap();
        let m = item_map(&items, &a).unwrap();
        let cases: Vec<LoroValue> = vec![
            LoroValue::from("not a map"),
            LoroValue::from(std::collections::HashMap::from([
                ("type".to_string(), LoroValue::from("hologram")),
                ("src".to_string(), LoroValue::from("x")),
            ])),
            LoroValue::from(std::collections::HashMap::from([(
                "type".to_string(),
                LoroValue::from("youtube"),
            )])),
            LoroValue::from(std::collections::HashMap::from([
                ("type".to_string(), LoroValue::from("video")),
                ("src".to_string(), LoroValue::from("a.mp4")),
                ("start".to_string(), LoroValue::Double(f64::NAN)),
            ])),
            LoroValue::from(std::collections::HashMap::from([
                ("type".to_string(), LoroValue::from("video")),
                ("src".to_string(), LoroValue::from("a.mp4")),
                ("start".to_string(), LoroValue::Double(9.0)),
                ("end".to_string(), LoroValue::Double(3.0)),
            ])),
        ];
        for v in cases {
            m.insert("media", v).unwrap();
            l.doc.commit();
            let ItemData::Image(i) = l.item(&p, &a).unwrap().data else {
                panic!("not an image")
            };
            assert_eq!(i.media, None);
            assert_eq!(i.asset, "ab.png");
        }
    }

    #[test]
    fn set_media_is_one_undo_step() {
        let mut l = Lesson::new("t", 0.0);
        let p = l.page_id_at(0).unwrap();
        let a = l.add_item(&p, &media_item(Some(youtube())), 1.0).unwrap();
        let mut next = youtube();
        next.muted = false;
        next.start = None;
        l.set_media(&p, &a, &next, Some("ef.jpg")).unwrap();
        let ItemData::Image(i) = l.item(&p, &a).unwrap().data else {
            panic!()
        };
        assert_eq!((i.media, i.asset.as_str()), (Some(next), "ef.jpg"));
        assert!(l.undo().unwrap());
        assert_eq!(l.item(&p, &a).unwrap().data, media_item(Some(youtube())));
        let bad = MediaData {
            kind: "web".into(),
            ..MediaData::default()
        };
        assert!(l.set_media(&p, &a, &bad, None).is_err());
        let s = l.add_item(&p, &stroke(0.0), 3.0).unwrap();
        assert!(l.set_media(&p, &s, &youtube(), None).is_err());
    }
}
