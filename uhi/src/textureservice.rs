use std::{
    collections::{HashMap, HashSet},
    ops::Range,
};

use rangealloc::RangeAlloc;

use crate::Externs;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureHandle {
    id: u32,
}

#[derive(Debug, Clone)]
pub enum TextureKind<E: Externs> {
    Internal(TextureHandle),
    External(E::TextureHandle),
}

// NOTE: TextureFormat is modeled after webgpu, see:
// - https://github.com/webgpu-native/webgpu-headers/blob/449359147fae26c07efe4fece25013df396287db/webgpu.h
// - https://www.w3.org/TR/webgpu/#texture-formats
#[derive(Debug)]
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

#[derive(Debug)]
pub struct TextureDesc {
    pub format: TextureFormat,
    pub w: u32,
    pub h: u32,
}

// NOTE: all the non-Debug stuff is derived because TextureRegion needs to be used as part of a
// hash map key.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TextureRegion {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug)]
pub struct TextureCreateTicket {
    handle: TextureHandle,
}

pub struct TexturePendingUpdate<'a, E: Externs> {
    pub region: TextureRegion,
    pub texture: &'a E::TextureHandle,
    pub desc: &'a TextureDesc,
    pub data: &'a [u8],
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

    pending_creates: HashMap<TextureHandle, TextureDesc>,
    pending_updates: HashMap<(TextureHandle, TextureRegion), Range<usize>>,
    pending_destroys: HashSet<TextureHandle>,

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
            pending_destroys: HashSet::default(),

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

        log::debug!("TextureService::enque_create ({handle:?}: {desc:?})");

        self.pending_creates.insert(handle, desc);
        handle
    }

    pub fn next_pending_create(&self) -> Option<(TextureCreateTicket, &TextureDesc)> {
        self.pending_creates
            .iter()
            .next()
            .map(|(handle, desc)| (TextureCreateTicket { handle: *handle }, desc))
    }

    pub fn commit_create(&mut self, ticket: TextureCreateTicket, texture: E::TextureHandle) {
        log::debug!("TextureService::commit_create ({:?})", &ticket.handle);

        let TextureCreateTicket { handle } = ticket;
        let desc = self
            .pending_creates
            .remove(&handle)
            .expect("pending create");
        let old_materialization = self
            .materializations
            .insert(handle, Materialization { texture, desc });
        assert!(old_materialization.is_none());
    }

    /// NOTE: this will panic if texture was not yet created.
    pub fn get(&self, handle: TextureHandle) -> &E::TextureHandle {
        self.materializations
            .get(&handle)
            .map(|m| &m.texture)
            .unwrap_or_else(|| panic!("dangling handle ({handle:?})"))
    }

    /// NOTE: returned buffer points into uninitialized or dirty memory (non-zeroed). you need to
    /// write each and every byte.
    pub fn enque_update(&mut self, handle: TextureHandle, region: TextureRegion) -> &mut [u8] {
        log::debug!("TextureService::enque_update ({handle:?}: {region:?})");

        let tex_format = self
            .materializations
            .get(&handle)
            .map(|m| &m.desc.format)
            .or_else(|| self.pending_creates.get(&handle).map(|desc| &desc.format))
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

        self.pending_updates.insert((handle, region), range);

        dst
    }

    pub fn next_pending_update(&mut self) -> Option<TexturePendingUpdate<E>> {
        let Some(key) = self.pending_updates.iter().next().map(|(k, _)| k.clone()) else {
            return None;
        };
        let range = self.pending_updates.remove(&key).unwrap();
        let (handle, region) = key;

        let materialization = self.materializations.get(&handle).expect("dangling handle");

        Some(TexturePendingUpdate {
            region,
            texture: &materialization.texture,
            desc: &materialization.desc,
            data: &self.buf[range],
        })
    }

    pub fn enque_destroy(&mut self, handle: TextureHandle) {
        log::debug!("TextureService::enque_destroy ({handle:?})");

        self.pending_destroys.insert(handle);
    }

    pub fn next_pending_destroy(&mut self) -> Option<E::TextureHandle> {
        while let Some(handle) = self.pending_destroys.iter().next().copied() {
            self.pending_destroys.remove(&handle);

            self.pending_updates.retain(|(h, _), range| {
                let ok = *h != handle;
                if !ok {
                    self.range_alloc.deallocate(range.clone());
                }
                ok
            });
            self.pending_creates.remove(&handle);

            if let Some(materialization) = self.materializations.remove(&handle) {
                return Some(materialization.texture);
            }
        }
        return None;
    }
}
