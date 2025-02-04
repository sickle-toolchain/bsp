use std::borrow::Cow;

use zerocopy::*;
use zerocopy_derive::*;

/// Lump definition count
pub const LUMP_DEF_COUNT: usize = 64;

/// BSP lump metadata
#[derive(FromBytes, IntoBytes, KnownLayout, Debug)]
#[repr(C)]
pub struct LumpMetadata {
  /// Lump version
  pub version: u32,
  /// Lump identifier
  pub identifier: [u8; 4],
}

/// BSP lump definition
#[derive(FromBytes, IntoBytes, KnownLayout, Debug)]
#[repr(C)]
pub struct LumpDef {
  /// Absolute offset in file
  offset: u32,
  /// Length of data
  length: u32,
  metadata: LumpMetadata,
}

/// BSP file header
#[derive(FromBytes, IntoBytes, KnownLayout, Debug)]
#[repr(C)]
pub struct Header {
  /// File format identifier
  pub identifier: [u8; 4],
  /// File format version
  pub version: u32,
  /// Lump definitions
  pub lump_defs: [LumpDef; LUMP_DEF_COUNT],
  /// File revision
  pub revision: i32,
}

/// Representation of a lump's metadata and Clone-on-write pointer to the data
pub type Lump<'a> = (&'a mut LumpMetadata, Cow<'a, [u8]>);

/// Representation of a BSP file
pub struct Bsp<'a> {
  /// File format identifier
  pub identifier: &'a mut [u8; 4],
  /// File format version
  pub version: &'a mut u32,
  lumps: [Lump<'a>; LUMP_DEF_COUNT],
  /// File revision
  pub revision: &'a mut i32,
}

impl<'a> Bsp<'a> {
  pub fn new(data: &'a mut [u8]) -> Result<Self, CastError<&'a mut [u8], Header>> {
    let (
      Header {
        identifier,
        version,
        lump_defs,
        revision,
      },
      data,
    ) = Header::mut_from_prefix(data)?;

    // Construct array of (&'a mut LumpMetadata, Cow<'a, [u8]>) from lump entries
    let lumps = lump_defs.each_mut().map(
      |&mut LumpDef {
         offset,
         length,
         ref mut metadata,
       }| {
        const HEADER_SIZE: usize = size_of::<Header>();
        let (offset, length) = (offset as usize, length as usize);

        // Adjust offset by HEADER_SIZE since LumpDescriptor's offset field is absolute
        // offset in file and we're indexing relative to the end of the header
        let offset = offset.saturating_sub(HEADER_SIZE);

        assert!((offset + length) <= data.len());
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

  pub fn lump(&self, index: usize) -> &Lump<'a> {
    assert!(index < LUMP_DEF_COUNT);
    &self.lumps[index]
  }

  pub fn lump_mut(&mut self, index: usize) -> &mut Lump<'a> {
    assert!(index < LUMP_DEF_COUNT);
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
