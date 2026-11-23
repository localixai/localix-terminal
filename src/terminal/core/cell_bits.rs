//! The fields of a raw cell a frame needs (what it holds, its codepoint, whether it has a style,
//! its width), read from the cell's 64 bits where the library's own type manifest says they lie.
//! The library keeps a cell packed in one word but does not promise where each field sits; it
//! publishes the places instead. A frame reads every cell of every changed row, and asking the
//! library for each field is a call per field per cell, so the places are read once from the
//! manifest. Where the manifest does not say them as expected, every field is asked of the
//! library, which is always right and only slower.

use std::sync::OnceLock;

use ghostty_vt_sys as vt;
use serde_json::Value;

/// What a frame needs of one raw cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Fields {
    pub tag: vt::CellContentTag::Type,
    pub codepoint: u32,
    pub styled: bool,
    pub wide: vt::CellWide::Type,
}

/// The fields of `raw`, read by `places` (from `places()`, taken once for a row) or else each
/// asked of the library.
#[inline]
pub(super) fn fields(places: Option<Places>, raw: vt::Cell) -> Fields {
    match places {
        Some(places) => places.read(raw),
        None => asked(raw),
    }
}

/// One field: its lowest bit and how many bits it takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Field {
    lsb: u32,
    width: u32,
}

impl Field {
    fn of(self, raw: u64) -> u64 {
        (raw >> self.lsb) & ((1u64 << self.width) - 1)
    }
}

/// Where each field the frame needs lies in a cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Places {
    tag: Field,
    codepoint: Field,
    style: Field,
    wide: Field,
}

impl Places {
    #[inline]
    fn read(self, raw: vt::Cell) -> Fields {
        // Each width is checked below 32 bits as the places are read, so every value fits.
        let small = |field: Field| u32::try_from(field.of(raw)).unwrap_or_default();
        let tag = i32::try_from(small(self.tag)).unwrap_or_default();
        // Only a cell that holds text has a codepoint; the same bits of a ground are its colour.
        let text = matches!(
            tag,
            vt::CellContentTag::CODEPOINT | vt::CellContentTag::CODEPOINT_GRAPHEME
        );
        Fields {
            tag,
            codepoint: if text { small(self.codepoint) } else { 0 },
            styled: self.style.of(raw) != 0,
            wide: i32::try_from(small(self.wide)).unwrap_or_default(),
        }
    }
}

/// The places, read once from the library linked.
pub(super) fn places() -> Option<Places> {
    static PLACES: OnceLock<Option<Places>> = OnceLock::new();
    *PLACES.get_or_init(|| {
        // SAFETY: the library's manifest: never null, a string that lives as long as the process.
        let manifest = unsafe { vt::ghostty_type_json() };
        // SAFETY: as above, a nul-terminated string that is never freed.
        let text = unsafe { std::ffi::CStr::from_ptr(manifest) };
        text.to_str().ok().and_then(from_manifest)
    })
}

/// The places as `manifest` says them, when it says all four as a packed cell of one word.
fn from_manifest(manifest: &str) -> Option<Places> {
    let manifest: Value = serde_json::from_str(manifest).ok()?;
    let cell = manifest.get("types")?.get("GhosttyCell")?;
    if cell.get("kind")? != "packed" || cell.get("size")? != 8 {
        return None;
    }
    let bits = cell.get("bits")?;
    let field = |value: &Value, base: u64| -> Option<Field> {
        let lsb = base + value.get("lsb")?.as_u64()?;
        let width = value.get("width")?.as_u64()?;
        (width > 0 && width < 32 && lsb + width <= 64).then_some(Field {
            lsb: u32::try_from(lsb).ok()?,
            width: u32::try_from(width).ok()?,
        })
    };
    let content = bits.get("content")?;
    let base = content.get("lsb")?.as_u64()?;
    let codepoint = content
        .get("arms")?
        .get("CODEPOINT")?
        .get("bits")?
        .get("codepoint")?;
    let grapheme = content
        .get("arms")?
        .get("CODEPOINT_GRAPHEME")?
        .get("bits")?
        .get("codepoint")?;
    let places = Places {
        tag: field(bits.get("content_tag")?, 0)?,
        codepoint: field(codepoint, base)?,
        style: field(bits.get("style_id")?, 0)?,
        wide: field(bits.get("wide")?, 0)?,
    };
    // Both arms that hold text keep the codepoint in one place, so one read serves both.
    (field(grapheme, base)? == places.codepoint).then_some(places)
}

/// The fields of `raw`, each asked of the library.
pub(super) fn asked(raw: vt::Cell) -> Fields {
    Fields {
        tag: of(raw, vt::CellData::CONTENT_TAG),
        codepoint: of(raw, vt::CellData::CODEPOINT),
        styled: of(raw, vt::CellData::HAS_STYLING),
        wide: of(raw, vt::CellData::WIDE),
    }
}

/// What a raw cell says of `data`: its default where it says nothing.
fn of<T: Default>(raw: vt::Cell, data: vt::CellData::Type) -> T {
    let mut value = T::default();
    let out: *mut T = &raw mut value;
    // SAFETY: `out` points to the type `data` is written as; a raw cell is a plain value.
    let _ = unsafe { vt::ghostty_cell_get(raw, data, out.cast()) };
    value
}

#[cfg(test)]
#[path = "cell_bits_tests.rs"]
mod tests;
