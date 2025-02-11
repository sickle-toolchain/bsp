use std::borrow::Cow;

use zerocopy::*;
use zerocopy_derive::*;

/// Lump definition count
pub const LUMP_DEF_COUNT: usize = 64;

/// BSP lump metadata
#[derive(FromBytes, IntoBytes, KnownLayout, Immutable, Debug, Clone, Copy)]
#[repr(C)]
pub struct LumpMetadata {
  /// Lump version
  pub version: u32,
  /// Lump identifier
  pub identifier: [u8; 4],
}

/// BSP lump definition
#[derive(FromBytes, IntoBytes, KnownLayout, Immutable, Debug, Clone, Copy)]
#[repr(C)]
pub struct LumpDef {
  /// Absolute offset in file
  offset: u32,
  /// Length of data
  length: u32,
  metadata: LumpMetadata,
}

/// BSP file header
#[derive(FromBytes, IntoBytes, KnownLayout, Immutable, Debug, Clone)]
#[repr(C)]
pub struct Header {
  /// File format identifier
  pub identifier: [u8; 4],
  /// File format version
  pub version: u32,
  /// Lump definitions
  lump_defs: [LumpDef; LUMP_DEF_COUNT],
  /// File revision
  pub revision: i32,
}

/// Tuple containing clone-on-write smart pointers to a lump's associated data and metadata
type LumpPair<'a> = (Cow<'a, LumpMetadata>, Cow<'a, [u8]>);

/// Representation of a BSP file
pub struct Bsp<'a> {
  /// BSP Header
  pub header: Cow<'a, Header>,
  /// Array of [`LUMP_DEF_COUNT`] [`LumpPair`]'s
  lumps: [LumpPair<'a>; LUMP_DEF_COUNT],
}

impl<'a> Bsp<'a> {
  pub fn parse(data: &'a [u8]) -> Result<Self, CastError<&'a [u8], Header>> {
    let (header, data) = Header::ref_from_prefix(data)?;

    // Construct array of (&'a mut LumpMetadata, Cow<'a, [u8]>) from lump entries
    let lumps = header.lump_defs.each_ref().map(
      |&LumpDef {
         offset,
         length,
         ref metadata,
       }| {
        const HEADER_SIZE: usize = size_of::<Header>();
        let (offset, length) = (offset as usize, length as usize);

        // Adjust offset by HEADER_SIZE since LumpDef's offset field is an absolute
        // offset in file and we're indexing relative to the end of the header
        let offset = offset.saturating_sub(HEADER_SIZE);

        assert!((offset + length) <= data.len());

        (
          Cow::Borrowed(metadata),
          Cow::Borrowed(&data[offset..offset + length]),
        )
      },
    );

    let bsp = Self {
      header: Cow::Borrowed(header),
      lumps,
    };
    Ok(bsp)
  }

  pub fn write_to_io<W>(&self, mut writer: W) -> std::io::Result<()>
  where
    W: std::io::Write,
  {
    const HEADER_SIZE: usize = size_of::<Header>();
    let mut header = self.header.clone().into_owned();

    // Update lump definitions
    let _ = self.lumps.iter().zip(header.lump_defs.iter_mut()).fold(
      // Start at offset HEADER_SIZE
      HEADER_SIZE,
      |acc, ((metadata, data), def)| {
        def.offset = acc as u32;
        def.length = data.len() as u32;
        def.metadata = *metadata.as_ref();

        acc + data.len()
      },
    );

    // Write data to writer
    writer.write_all(header.as_bytes())?;
    for (_, lump) in &self.lumps {
      writer.write_all(lump)?;
    }
    Ok(())
  }

  pub fn lump_data<T, I>(&self, index: I) -> Result<&T, CastError<&[u8], T>>
  where
    T: ?Sized + FromBytes + KnownLayout + Immutable,
    I: Into<usize>,
  {
    let data = &self.lump(index).1;
    T::ref_from_bytes(data)
  }

  pub fn lump_data_mut<T, I>(&mut self, index: I) -> Result<&mut T, CastError<&mut [u8], T>>
  where
    T: ?Sized + FromBytes + IntoBytes + KnownLayout,
    I: Into<usize>,
  {
    let data = &mut self.lump_mut(index).1;
    T::mut_from_bytes(data.to_mut())
  }

  pub fn lump_meta<I>(&self, index: I) -> &Cow<'a, LumpMetadata>
  where
    I: Into<usize>,
  {
    &self.lump(index).0
  }

  pub fn lump_meta_mut<I>(&mut self, index: I) -> &mut Cow<'a, LumpMetadata>
  where
    I: Into<usize>,
  {
    &mut self.lump_mut(index).0
  }

  pub fn lump<I>(&self, index: I) -> &LumpPair<'a>
  where
    I: Into<usize>,
  {
    let index: usize = index.into();
    assert!(index < LUMP_DEF_COUNT);

    &self.lumps[index]
  }

  pub fn lump_mut<I>(&mut self, index: I) -> &mut LumpPair<'a>
  where
    I: Into<usize>,
  {
    let index: usize = index.into();
    assert!(index < LUMP_DEF_COUNT);

    &mut self.lumps[index]
  }
}

impl std::fmt::Debug for Bsp<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Bsp")
      .field("identifier", &self.header.identifier)
      .field("version", &self.header.version)
      .field("revision", &self.header.revision)
      // Indicate that we have omitted data (lump entries)
      .finish_non_exhaustive()
  }
}
