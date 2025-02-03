use std::array;
use std::borrow::Cow;

use zerocopy::*;
use zerocopy_derive::*;

pub const LUMP_COUNT: usize = 64;

type Lump<'a> = Cow<'a, [u8]>;

#[derive(FromBytes, IntoBytes, KnownLayout, Immutable, Debug)]
#[repr(C)]
pub(crate) struct LumpRange {
  /// Absolute offset in file
  offset: u32,
  /// Length of data
  length: u32,
}

/// Entry describing a BSP lump
#[derive(FromBytes, IntoBytes, KnownLayout, Immutable, Debug)]
#[repr(C)]
pub struct LumpEntry {
  pub(crate) range: LumpRange,
  /// Lump version
  pub version: u32,
  /// Lump identifier
  pub identifier: [u8; 4],
}

/// BSP Header
#[derive(FromBytes, IntoBytes, KnownLayout, Immutable, Debug)]
#[repr(C)]
pub struct Header {
  /// File format identifier
  pub identifier: [u8; 4],
  /// File format version
  pub version: u32,
  /// Lump entries
  pub lump_entries: [LumpEntry; LUMP_COUNT],
  /// File revision
  pub revision: i32,
}

pub struct Bsp<'a> {
  /// File format identifier
  pub identifier: &'a mut [u8; 4],
  /// File format version
  pub version: &'a mut u32,
  lumps: [(&'a LumpEntry, Lump<'a>); LUMP_COUNT],
  /// Revision number of file
  pub revision: &'a mut i32,
}

impl<'a> Bsp<'a> {
  pub fn new(data: &'a mut [u8]) -> Result<Self, CastError<&'a mut [u8], Header>> {
    let (
      Header {
        identifier,
        version,
        lump_entries,
        revision,
      },
      data,
    ) = Header::mut_from_prefix(data.as_mut())?;

    // Construct array of (&'a LumpEntry, Cow<'a, [u8]>) from lump entries
    let lumps = array::from_fn(|i| {
      let entry = &lump_entries[i];

      // Adjust lump entry offset since they're absolute but we're indexing relative to the end of `Header`
      // TODO: add debug assertions
      let offset = entry.range.offset as usize - size_of::<Header>();
      let lump = Cow::from(&data[offset..offset + entry.range.length as usize]);

      (entry, lump)
    });

    let bsp = Self {
      identifier,
      version,
      lumps,
      revision,
    };
    Ok(bsp)
  }

  pub fn lump(&self, index: usize) -> &Lump {
    assert!(index < LUMP_COUNT);
    &self.lumps[index].1
  }

  pub fn lump_mut(&mut self, index: usize) -> &mut Lump<'a> {
    assert!(index < LUMP_COUNT);
    &mut self.lumps[index].1
  }
}

impl<'a> std::fmt::Debug for Bsp<'a> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Bsp")
      .field("identifier", &self.identifier)
      .field("version", &self.version)
      .field("revision", &self.revision)
      // Indicate that we have omitted data (lump entries)
      .finish_non_exhaustive()
  }
}
