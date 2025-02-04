use std::borrow::Cow;

use zerocopy::*;
use zerocopy_derive::*;

pub const LUMP_COUNT: usize = 64;

type Lump<'a> = Cow<'a, [u8]>;

/// Metadata of a BSP lump
#[derive(FromBytes, IntoBytes, KnownLayout, Debug)]
#[repr(C)]
pub struct LumpMetadata {
  /// Lump version
  pub version: u32,
  /// Lump identifier
  pub identifier: [u8; 4],
}

/// Entry describing a BSP lump
#[derive(FromBytes, IntoBytes, KnownLayout, Debug)]
#[repr(C)]
pub struct LumpEntry {
  /// Absolute offset in file
  offset: u32,
  /// Length of data
  length: u32,
  metadata: LumpMetadata,
}

/// BSP Header
#[derive(FromBytes, IntoBytes, KnownLayout, Debug)]
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
  lumps: [(&'a mut LumpMetadata, Lump<'a>); LUMP_COUNT],
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
    ) = Header::mut_from_prefix(data)?;

    // Construct array of (&mut LumpMetadata, Cow<'a, [u8]>) from lump entries
    let lumps = lump_entries.each_mut().map(
      |&mut LumpEntry {
         offset,
         length,
         ref mut metadata,
       }| {
        const HEADER_SIZE: usize = size_of::<Header>();
        let (offset, length) = (offset as usize, length as usize);

        // Adjust offset by HEADER_SIZE since LumpDescriptor's offset field is absolute offset in file
        // and we're indexing relative to the end of the header
        let offset = offset.saturating_sub(HEADER_SIZE);
        let lump = Cow::from(&data[offset..offset + length]);

        (metadata, lump)
      },
    );

    let bsp = Self {
      identifier,
      version,
      lumps,
      revision,
    };
    Ok(bsp)
  }

  pub fn lump(&self, index: usize) -> &(&'a mut LumpMetadata, Lump<'a>) {
    assert!(index < LUMP_COUNT);
    &self.lumps[index]
  }

  pub fn lump_mut(&mut self, index: usize) -> &mut (&'a mut LumpMetadata, Lump<'a>) {
    assert!(index < LUMP_COUNT);
    &mut self.lumps[index]
  }
}

impl std::fmt::Debug for Bsp<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Bsp")
      .field("identifier", &self.identifier)
      .field("version", &self.version)
      .field("revision", &self.revision)
      // Indicate that we have omitted data (lump entries)
      .finish_non_exhaustive()
  }
}
