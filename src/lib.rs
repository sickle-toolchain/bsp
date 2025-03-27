use std::{
    borrow::{Borrow, Cow},
    cell::{Ref, RefCell, RefMut},
    mem::MaybeUninit,
};

use zerocopy::{CastError, FromBytes, Immutable, IntoBytes, KnownLayout};
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
pub struct LumpDefinition {
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
    lump_defs: [LumpDefinition; LUMP_DEF_COUNT],
    /// File revision
    pub revision: i32,
}

/// Struct containing
pub struct Lump<'a> {
    pub metadata: Cow<'a, LumpMetadata>,
    pub data: Cow<'a, [u8]>,
}
/// Representation of a BSP file
pub struct Bsp<'a> {
    /// BSP Header
    pub header: Cow<'a, Header>,
    /// Array of [`Lump`]'s
    lumps: [RefCell<Lump<'a>>; LUMP_DEF_COUNT],
}

impl<'a> Bsp<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self, CastError<&'a [u8], Header>> {
        let (header, data) = Header::ref_from_prefix(data)?;

        // Construct array of `Lump` from lump definitions
        let lumps = header
            .lump_defs
            .each_ref()
            .map(
                |&LumpDefinition {
                     offset,
                     length,
                     ref metadata,
                 }| {
                    const HEADER_SIZE: usize = size_of::<Header>();
                    let (offset, length) = (offset as usize, length as usize);

                    // Adjust offset by HEADER_SIZE since `LumpDefinition::offset` is an absolute
                    // offset in file and we're indexing relative to the end of the header
                    let offset = offset.saturating_sub(HEADER_SIZE);

                    assert!((offset + length) <= data.len());

                    Lump {
                        metadata: Cow::Borrowed(metadata),
                        data: Cow::Borrowed(&data[offset..offset + length]),
                    }
                },
            )
            .map(RefCell::new);

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
        let _ = self
            .lumps
            .iter()
            .map(RefCell::borrow)
            .zip(header.lump_defs.iter_mut())
            .fold(
                // Start at offset HEADER_SIZE
                HEADER_SIZE,
                |acc, (lump, def)| {
                    def.offset = acc as u32;
                    def.length = lump.data.len() as u32;
                    def.metadata = *lump.metadata;

                    def.offset as usize + def.length as usize
                },
            );

        // Write data to writer
        writer.write_all(header.as_bytes())?;
        for lump in self.lumps.iter().map(RefCell::borrow) {
            writer.write_all(&lump.data)?;
        }

        Ok(())
    }

    pub fn lump_cast<T, I>(&'a self, index: I) -> Result<Ref<'a, T>, CastError<(), T>>
    where
        T: ?Sized + FromBytes + KnownLayout + Immutable,
        I: Into<usize>,
    {
        let cell = self.lump_cell(index);
        let mut err = MaybeUninit::uninit();
        Ref::filter_map(cell.borrow(), |v| match T::ref_from_bytes(&v.data) {
            Ok(o) => Some(o),
            Err(e) => {
                // TODO: we sadly throw away information from the error here since
                // this wouldn't work otherwise. It would be nice to see if this can
                // be solved in the future.
                //
                // If we can't resolve this, then properly document it and use a self-describing
                // type for the src such as `OmittedSrc`
                err.write(e.map_src(|_| ()));
                None
            }
        })
        // SAFETY: if we're Err(_) then `err` will be initialized
        .map_err(move |_| unsafe { err.assume_init() })
    }

    pub fn lump_cast_mut<T, I>(&'a self, index: I) -> Result<RefMut<'a, T>, CastError<(), T>>
    where
        T: ?Sized + FromBytes + IntoBytes + KnownLayout + Immutable,
        I: Into<usize>,
    {
        let cell = self.lump_cell(index);
        let mut err = MaybeUninit::uninit();
        RefMut::filter_map(cell.borrow_mut(), |v| {
            match T::mut_from_bytes(v.data.to_mut()) {
                Ok(o) => Some(o),
                Err(e) => {
                    // TODO: we sadly throw away information from the error here since
                    // this wouldn't work otherwise. It would be nice to see if this can
                    // be solved in the future.
                    //
                    // If we can't resolve this, then properly document it and use a self-describing
                    // type for the src such as `OmittedSrc`
                    err.write(e.map_src(|_| ()));
                    None
                }
            }
        })
        // SAFETY: if we're Err(_) then `err` will be initialized
        .map_err(|_| unsafe { err.assume_init() })
    }

    pub fn lump<I>(&self, index: I) -> Ref<'a, Lump>
    where
        I: Into<usize>,
    {
        let cell = self.lump_cell(index);
        cell.borrow()
    }

    pub fn lump_mut<I>(&self, index: I) -> RefMut<'a, Lump>
    where
        I: Into<usize>,
    {
        let cell = self.lump_cell(index);
        cell.borrow_mut()
    }

    fn lump_cell<I>(&self, index: I) -> &RefCell<Lump<'a>>
    where
        I: Into<usize>,
    {
        let index: usize = index.into();
        assert!(index < LUMP_DEF_COUNT);

        &self.lumps[index]
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
