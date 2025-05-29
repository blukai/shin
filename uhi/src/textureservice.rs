use std::collections::HashMap;

use crate::Renderer;

#[derive(Debug, Clone)]
pub enum TextureKind<R: Renderer> {
    Internal(TextureHandle),
    External(R::TextureHandle),
}

// NOTE: TextureFormat is modeled after webgpu, see:
// - https://github.com/webgpu-native/webgpu-headers/blob/449359147fae26c07efe4fece25013df396287db/webgpu.h
// - https://www.w3.org/TR/webgpu/#texture-formats
pub enum TextureFormat {
    Rgba8Unorm,
    R8Unorm,
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
    // NOTE: region theoretically enables update deduplication.
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

struct Materialization<R: Renderer> {
    texture: R::TextureHandle,
    desc: TextureDesc,
}

// allows to defer texture creation and uploads; provides handles that allow to map to committed (/
// materialized) textures.
//
// NOTE: this may seem like an shitty stupid extra unnecessary layer of abstraction because in shin
// were dealing only with opengl/webgl which allows to do stuff in immediate-mode. but i want to be
// able to use uhi in other projects that use other graphics apis.
pub struct TextureService<R: Renderer> {
    id_acc: u32,
    pending_creates: HashMap<TextureCreateTicket, TextureDesc>,
    // TODO: make use of my RangeAlloc thing pointing into large chunk of linear memory. make some
    // kind of TransientBuffer thing or something, idk, might need a better name, but i think the
    // idea is solid. except that RangeAlloc is not highly enough efficient/optimized.
    pending_updates: HashMap<TextureUpdateTicket, Vec<u8>>,
    materializations: HashMap<TextureHandle, Materialization<R>>,
}

impl<R: Renderer> Default for TextureService<R> {
    fn default() -> Self {
        Self {
            id_acc: 0,
            pending_creates: HashMap::default(),
            pending_updates: HashMap::default(),
            materializations: HashMap::default(),
        }
    }
}

impl<R: Renderer> TextureService<R> {
    pub fn enque_create(&mut self, desc: TextureDesc) -> TextureHandle {
        let handle = TextureHandle { id: self.id_acc };
        self.id_acc += 1;
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

    pub fn commit_create(&mut self, ticket: TextureCreateTicket, texture: R::TextureHandle) {
        let desc = self
            .pending_creates
            .remove(&ticket)
            .expect("pending create");
        let old_materialization = self
            .materializations
            .insert(ticket.handle, Materialization { texture, desc });
        assert!(old_materialization.is_none());
    }

    pub fn enque_update(&mut self, handle: TextureHandle, region: TextureRegion, data: Vec<u8>) {
        self.pending_updates.insert(
            TextureUpdateTicket {
                handle,
                // NOTE: this is awkwrard that both key and value needs to contain region, but this
                // makes things more convenient. no biggie.
                region: region.clone(),
            },
            data,
        );
    }

    pub fn next_pending_update(&self) -> Option<(TextureUpdateTicket, TexturePendingUpdate)> {
        self.pending_updates.iter().next().map(|(k, v)| {
            (
                k.clone(),
                TexturePendingUpdate {
                    handle: k.handle,
                    data: v.as_slice(),
                    desc: &self
                        .materializations
                        .get(&k.handle)
                        .expect("materialized texture")
                        .desc,
                    region: &k.region,
                },
            )
        })
    }

    pub fn commit_update(&mut self, ticket: TextureUpdateTicket) {
        self.pending_updates.remove(&ticket);
    }

    // NOTE: this will panic if texture was not yet created.
    pub fn get(&self, handle: TextureHandle) -> &R::TextureHandle {
        self.materializations
            .get(&handle)
            .map(|m| &m.texture)
            .expect("dangling handle")
    }
}
