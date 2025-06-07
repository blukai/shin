use std::{collections::HashMap, ops::Range};

use rangealloc::RangeAlloc;

use crate::Externs;

#[derive(Debug, Clone)]
pub enum TextureKind<E: Externs> {
    Internal(TextureHandle),
    External(E::TextureHandle),
}

// NOTE: TextureFormat is modeled after webgpu, see:
// - https://github.com/webgpu-native/webgpu-headers/blob/449359147fae26c07efe4fece25013df396287db/webgpu.h
// - https://www.w3.org/TR/webgpu/#texture-formats
pub enum TextureFormat {
    Rgba8Unorm,
    R8Unorm,
}

impl TextureFormat {
    pub fn block_size(&self) -> u8 {
        match self {
            Self::Rgba8Unorm => 4,
            Self::R8Unorm => 1,
        }
    }
}

pub struct TextureDesc {
    pub format: TextureFormat,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TextureRegion {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureHandle {
    id: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TextureCreateTicket {
    handle: TextureHandle,
}

// NOTE: non_exhaustive supposedly ensures that this struct cannot be constructed outside.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TextureUpdateTicket {
    handle: TextureHandle,
    // NOTE: having region here theoretically enables update deduplication.
    region: TextureRegion,
}

// TODO: make use of my RangeAlloc thing pointing into large chunk of linear memory. make some kind
// of TransientBuffer thing or something, idk, might need a better name, but i think the idea is
// solid. except that RangeAlloc is not highly enough efficient/optimized.
pub struct TexturePendingUpdate<'a> {
    pub handle: TextureHandle,
    pub data: &'a [u8],
    pub desc: &'a TextureDesc,
    pub region: &'a TextureRegion,
}

struct Materialization<E: Externs> {
    texture: E::TextureHandle,
    desc: TextureDesc,
}

// allows to defer texture creation and uploads; provides handles that allow to map to committed (/
// materialized) textures.
//
// NOTE: this may seem like an shitty stupid extra unnecessary layer of abstraction because in shin
// were dealing only with opengl/webgl which allows to do stuff in immediate-mode. but i want to be
// able to use uhi in other projects that use other graphics apis.
pub struct TextureService<E: Externs> {
    handle_id_acc: u32,

    buf: Vec<u8>,
    range_alloc: RangeAlloc<usize>,

    pending_creates: HashMap<TextureCreateTicket, TextureDesc>,
    // TODO: make use of my RangeAlloc thing pointing into large chunk of linear memory. make some
    // kind of TransientBuffer thing or something, idk, might need a better name, but i think the
    // idea is solid. except that RangeAlloc is not highly enough efficient/optimized.
    pending_updates: HashMap<TextureUpdateTicket, Range<usize>>,
    materializations: HashMap<TextureHandle, Materialization<E>>,
}

impl<E: Externs> Default for TextureService<E> {
    fn default() -> Self {
        Self {
            handle_id_acc: 0,

            buf: Vec::default(),
            range_alloc: RangeAlloc::new(0..0),

            pending_creates: HashMap::default(),
            pending_updates: HashMap::default(),
            materializations: HashMap::default(),
        }
    }
}

impl<E: Externs> TextureService<E> {
    pub fn enque_create(&mut self, desc: TextureDesc) -> TextureHandle {
        let handle = TextureHandle {
            id: self.handle_id_acc,
        };
        self.handle_id_acc += 1;
        self.pending_creates
            .insert(TextureCreateTicket { handle }, desc);
        handle
    }

    pub fn next_pending_create(&self) -> Option<(TextureCreateTicket, &TextureDesc)> {
        self.pending_creates
            .iter()
            .next()
            .map(|(k, v)| (k.clone(), v))
    }

    pub fn commit_create(&mut self, ticket: TextureCreateTicket, texture: E::TextureHandle) {
        let desc = self
            .pending_creates
            .remove(&ticket)
            .expect("pending create");
        let old_materialization = self
            .materializations
            .insert(ticket.handle, Materialization { texture, desc });
        assert!(old_materialization.is_none());
    }

    /// NOTE: returned buffer points into uninitialized or dirty memory (non-zeroed). you need to
    /// write each and every byte.
    pub fn enque_update(&mut self, handle: TextureHandle, region: TextureRegion) -> &mut [u8] {
        let tex_format = self
            .materializations
            .get(&handle)
            .map(|m| &m.desc.format)
            .or_else(|| {
                self.pending_creates
                    .get(&TextureCreateTicket { handle })
                    .map(|desc| &desc.format)
            })
            .expect("materialized or pending-create texture");
        let region_size = (region.w * region.h * tex_format.block_size() as u32) as usize;

        let range = self
            .range_alloc
            .allocate(region_size)
            // buf can't fit region, grow it.
            .unwrap_or_else(|_| {
                let full_range = self.range_alloc.full_range();
                let additional = full_range.len().max(region_size);
                let new_end = full_range.end + additional;

                self.buf.reserve_exact(additional);
                assert!(self.buf.capacity() == new_end);
                unsafe { self.buf.set_len(new_end) };

                self.range_alloc.grow(new_end);
                self.range_alloc.allocate(region_size).unwrap()
            });
        let dst = &mut self.buf[range.clone()];

        self.pending_updates
            .insert(TextureUpdateTicket { handle, region }, range);

        dst
    }

    pub fn next_pending_update(&self) -> Option<(TextureUpdateTicket, TexturePendingUpdate)> {
        self.pending_updates.iter().next().map(|(ticket, range)| {
            (
                ticket.clone(),
                TexturePendingUpdate {
                    handle: ticket.handle,
                    data: &self.buf[range.clone()],
                    desc: &self
                        .materializations
                        .get(&ticket.handle)
                        .expect("materialized texture")
                        .desc,
                    region: &ticket.region,
                },
            )
        })
    }

    pub fn commit_update(&mut self, ticket: TextureUpdateTicket) {
        let range = self.pending_updates.remove(&ticket).expect("valid ticket");
        self.range_alloc.deallocate(range);
    }

    // NOTE: this will panic if texture was not yet created.
    pub fn get(&self, handle: TextureHandle) -> &E::TextureHandle {
        self.materializations
            .get(&handle)
            .map(|m| &m.texture)
            .expect("dangling handle")
    }
}
