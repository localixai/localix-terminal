//! A file as an editor holds it: its text with plain line ends, how the disk writes it (its line
//! ends and its mark of byte order, given back on a save), and a stamp of what the disk held,
//! so a change by another hand is told from the editor's own save. A save writes the file in
//! place: its links, its permissions and whoever watches it keep the same file.

use std::fs::{File, Metadata};
use std::io::{self, Read as _, Write as _};
use std::path::Path;
use std::time::SystemTime;

/// The largest file read as text: its text, and what a person left unsaved in it, reach an
/// editor's page whole, in one frame of the size a tab takes at once.
pub const LARGEST: u64 = 2 * 1024 * 1024;

/// How much of a file's start is looked at for what no text holds.
const LOOKED_AT: usize = 8 * 1024;
const BOM: &str = "\u{feff}";

/// How the disk writes a file's text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Shape {
    /// Lines end with a carriage return and a line feed.
    pub crlf: bool,
    /// The text starts with a mark of byte order.
    pub bom: bool,
}

/// What the disk held when a file was read or written: its length, its time, and its bytes
/// in short.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stamp {
    len: u64,
    modified: Option<SystemTime>,
    hash: u64,
}

/// A file read as text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Text {
    pub text: String,
    pub shape: Shape,
    pub stamp: Stamp,
}

/// Why a path was not read as text.
#[derive(Debug)]
pub enum Unread {
    Gone,
    Folder,
    /// Its length, past [`LARGEST`].
    Large(u64),
    /// Not UTF-8 text: a picture, a program, text in another encoding.
    Binary,
    Failed(io::Error),
}

impl From<io::Error> for Unread {
    fn from(why: io::Error) -> Self {
        if why.kind() == io::ErrorKind::NotFound {
            Self::Gone
        } else {
            Self::Failed(why)
        }
    }
}

/// The file at `path` as an editor holds it.
pub fn read(path: &Path) -> Result<Text, Unread> {
    let file = File::open(path)?;
    let about = file.metadata()?;
    if about.is_dir() {
        return Err(Unread::Folder);
    }
    if about.len() > LARGEST {
        return Err(Unread::Large(about.len()));
    }
    let mut bytes = Vec::with_capacity(usize::try_from(about.len()).unwrap_or(0));
    // Through the handle its length was read from, and no further than an editor takes: the
    // same file whatever takes its name meanwhile, however it grows while it is read.
    (&file).take(LARGEST + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > LARGEST {
        return Err(Unread::Large(bytes.len() as u64));
    }
    let stamp = Stamp::of(&bytes, &file.metadata()?);
    let looked_at = bytes.get(..LOOKED_AT).unwrap_or(&bytes);
    if looked_at.contains(&0) {
        return Err(Unread::Binary);
    }
    let whole = String::from_utf8(bytes).map_err(|_not_utf8| Unread::Binary)?;
    let bom = whole.starts_with(BOM);
    let body = whole.strip_prefix(BOM).unwrap_or(&whole);
    // The first line's end is the file's: a file is written one way.
    let crlf = (body.split_once('\n')).is_some_and(|(first, _)| first.ends_with('\r'));
    let text = if body.contains('\r') {
        body.replace("\r\n", "\n")
    } else if bom {
        body.to_string()
    } else {
        whole
    };
    Ok(Text {
        text,
        shape: Shape { crlf, bom },
        stamp,
    })
}

/// `text` written at `path` as `shape` says the disk writes it, in place; the stamp of what
/// the disk holds now. A file that is gone is made.
pub fn save(path: &Path, text: &str, shape: Shape) -> io::Result<Stamp> {
    let mut bytes = Vec::with_capacity(text.len() + BOM.len());
    if shape.bom {
        bytes.extend_from_slice(BOM.as_bytes());
    }
    if shape.crlf {
        bytes.extend_from_slice(text.replace('\n', "\r\n").as_bytes());
    } else {
        bytes.extend_from_slice(text.as_bytes());
    }
    let mut file = File::options()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)?;
    file.write_all(&bytes)?;
    file.flush()?;
    Ok(Stamp::of(&bytes, &file.metadata()?))
}

/// What the disk holds at `path` now, if it is not what `known` names: the file as it reads
/// now, or why it does not. None while the disk holds the same bytes, whoever wrote them.
pub fn news(path: &Path, known: &Stamp) -> Option<Result<Text, Unread>> {
    if let Ok(about) = std::fs::metadata(path)
        && about.len() == known.len
        && about.modified().ok() == known.modified
    {
        return None;
    }
    match read(path) {
        Ok(held) if held.stamp.hash == known.hash && held.stamp.len == known.len => None,
        told => Some(told),
    }
}

impl Stamp {
    fn of(bytes: &[u8], about: &Metadata) -> Self {
        // FNV-1a: the same for the same bytes in every build, since a stamp is kept with what
        // a person left unsaved.
        let hash = (bytes.iter()).fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
        });
        Self {
            len: bytes.len() as u64,
            modified: about.modified().ok(),
            hash,
        }
    }

    /// The bytes it names, in short: their length and their hash, as a draft keeps them.
    pub fn bytes(&self) -> (u64, u64) {
        (self.len, self.hash)
    }

    /// Whether `other` names the same bytes, whenever each was written.
    pub fn same_bytes(&self, other: &Self) -> bool {
        self.len == other.len && self.hash == other.hash
    }
}

#[cfg(test)]
#[path = "text_tests.rs"]
mod tests;
