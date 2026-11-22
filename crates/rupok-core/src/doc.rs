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
        let id = self.new_id();
        let z = self.next_z(page_id, &items);
        match data {
            ItemData::Stroke(_) => {
                let mut f = ValueFields::default();
                f.set("kind", data.kind());
                f.set("z", z);
                f.set("created", now_ms);
                write_item_data(&mut f, data)?;
                items.insert(&id, f.into_value())?;
            }
            _ => {
                let m = items.insert_container(&id, LoroMap::new())?;
                m.insert("kind", data.kind())?;
                m.insert("z", z)?;
                m.insert("created", now_ms)?;
                write_item_data(&mut MapFields(&m), data)?;
            }
        }
        self.doc.commit();
        Ok(id)
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
        }
        self.doc.commit();
        Ok(())
    }

    /// Set the geometry of a box item (text, image, object).
    pub fn set_geom(&mut self, page_id: &str, id: &str, g: BoxGeom) -> Result<()> {
        let m = item_map(&self.items_map(page_id)?, id)?;
        write_geom(&m, &g)?;
        self.doc.commit();
        Ok(())
    }

    pub fn bring_to_front(&mut self, page_id: &str, id: &str) -> Result<()> {
        let items = self.items_map(page_id)?;
        let z = self.next_z(page_id, &items);
        match items.get(id) {
            Some(ValueOrContainer::Value(LoroValue::Map(v))) => {
                let mut f = ValueFields::from(&v);
                f.set("z", z);
                items.insert(id, f.into_value())?;
            }
            Some(ValueOrContainer::Container(Container::Map(m))) => m.insert("z", z)?,
            _ => return Err(CoreError::ItemNotFound(id.to_string())),
        }
        self.doc.commit();
        Ok(())
    }

    pub fn set_text(&mut self, page_id: &str, id: &str, text: &str) -> Result<()> {
        item_map(&self.items_map(page_id)?, id)?.insert("text", text)?;
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
            if let LoroValue::Map(fields) = v {
                out.push(read_item(k, fields)?);
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
        }
        ItemData::Text(t) => {
            write_geom_to(m, &t.geom)?;
            m.insert("text", t.text.as_str())?;
            m.insert("size", t.size)?;
            m.insert("color", i64::from(t.color))?;
        }
        ItemData::Image(i) => {
            write_geom_to(m, &i.geom)?;
            m.insert("asset", i.asset.as_str())?;
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
            })
        }
        "text" => ItemData::Text(TextData {
            geom: read_geom(m),
            text: str_of(m, "text").unwrap_or_default(),
            size: f64_of(m, "size").unwrap_or(32.0),
            color: u32_of(m, "color").unwrap_or(0x000000ff),
        }),
        "image" => ItemData::Image(ImageData {
            geom: read_geom(m),
            asset: str_of(m, "asset").unwrap_or_default(),
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

    #[test]
    fn rejects_garbage_and_future_formats() {
        assert!(Lesson::load(b"not a loro doc").is_err());
        let doc = LoroDoc::new();
        doc.get_map("meta").insert("format", 999).unwrap();
        doc.commit();
        assert!(Lesson::load(&doc.export(ExportMode::Snapshot).unwrap()).is_err());
    }
}
