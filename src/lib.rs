use std::{
    borrow::{Borrow, Cow},
    cell::{Ref, RefCell, RefMut},
};

use zerocopy::{CastError, FromBytes, IntoBytes};
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

// TODO: describe
type LumpCell<'a> = RefCell<(Cow<'a, LumpMetadata>, Cow<'a, [u8]>)>;

/// Representation of a BSP file
pub struct Bsp<'a> {
    /// BSP Header
    pub header: Cow<'a, Header>,
    /// Array of [`LUMP_DEF_COUNT`] [`LumpPair`]'s
    lumps: [LumpCell<'a>; LUMP_DEF_COUNT],
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

                RefCell::new((
                    Cow::Borrowed(metadata),
                    Cow::Borrowed(&data[offset..offset + length]),
                ))
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
        let _ = self.lump_iter().zip(header.lump_defs.iter_mut()).fold(
            // Start at offset HEADER_SIZE
            HEADER_SIZE,
            |acc, ((metadata, data), def)| {
                def.offset = acc as u32;
                def.length = data.borrow().len() as u32;
                def.metadata = *metadata.borrow().as_ref();

                def.offset as usize + def.length as usize
            },
        );

        // Write data to writer
        writer.write_all(header.as_bytes())?;
        for lump in &self.lumps {
            let cell = lump.borrow();
            writer.write_all(&cell.1)?;
        }
        Ok(())
    }

    // TODO: figure out if we can reintroduce casting helper functions
    // they were quite convenient, but i was not able to implement them
    // following the interior mutability changes

    pub fn lump<I>(&self, index: I) -> (Ref<'_, Cow<'a, LumpMetadata>>, Ref<'_, Cow<'a, [u8]>>)
    where
        I: Into<usize>,
    {
        let cell = self.lump_cell(index);
        Ref::map_split(cell.borrow(), |v| (&v.0, &v.1))
    }

    pub fn lump_mut<I>(
        &self,
        index: I,
    ) -> (RefMut<'_, Cow<'a, LumpMetadata>>, RefMut<'_, Cow<'a, [u8]>>)
    where
        I: Into<usize>,
    {
        let cell = self.lump_cell(index);
        RefMut::map_split(cell.borrow_mut(), |v| (&mut v.0, &mut v.1))
    }

    fn lump_cell<I>(&self, index: I) -> &LumpCell<'a>
    where
        I: Into<usize>,
    {
        let index: usize = index.into();
        assert!(index < LUMP_DEF_COUNT);

        &self.lumps[index]
    }

    fn lump_iter(
        &self,
    ) -> impl Iterator<Item = (Ref<'_, Cow<'a, LumpMetadata>>, Ref<'_, Cow<'a, [u8]>>)> {
        self.lumps
            .iter()
            .map(|v| Ref::map_split(v.borrow(), |e| (&e.0, &e.1)))
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
