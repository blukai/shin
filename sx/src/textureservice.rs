use std::collections::VecDeque;
use std::hash::{Hash, Hasher};
use std::iter;
use std::ops::Range;

use mars::nohash::{NoHash, NoHashMap};
use mars::rangealloc::RangeAlloc;

use crate::Externs;

// TODO: consider using word "image" instead of "texture"
//   but that makes naming of a thing like TexturePacker not so simple, idk.
//   the word texture doesn't make a lot of sense to me in context of images or graphics really.
//   texture is something physical, something that you can sense; or imagine how it would feel.

// NOTE: TextureFormat is modeled after webgpu
//   see:
//   - https://github.com/webgpu-native/webgpu-headers/blob/449359147fae26c07efe4fece25013df396287db/webgpu.h
//   - https://www.w3.org/TR/webgpu/#texture-formats
#[derive(Debug, Clone, Copy)]
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

#[derive(Debug, Clone)]
pub struct TextureDesc {
    pub format: TextureFormat,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextureHandle {
    id: u32,
}

impl Hash for TextureHandle {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u32(self.id);
    }
}

impl NoHash for TextureHandle {}

#[derive(Debug, Clone)]
pub enum TextureHandleKind<E: Externs> {
    Internal(TextureHandle),
    External {
        handle: E::TextureHandle,
        format: TextureFormat,
    },
}

#[derive(Debug, Clone)]
pub struct TextureRegion {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug)]
pub enum TextureCommandKind<Desc, Buf> {
    Create { desc: Desc },
    Upload { region: TextureRegion, buf: Buf },
    Delete,
}

#[derive(Debug)]
pub struct TextureCommand<Desc, Buf> {
    pub handle: TextureHandle,
    pub kind: TextureCommandKind<Desc, Buf>,
}

#[derive(Default)]
pub struct TextureService {
    next_id: u32,

    buf: Vec<u8>,
    range_alloc: RangeAlloc<usize>,

    descs: NoHashMap<TextureHandle, TextureDesc>,
    commands: VecDeque<TextureCommand<(), Range<usize>>>,
}

impl TextureService {
    pub fn create(&mut self, desc: TextureDesc) -> TextureHandle {
        let handle = TextureHandle { id: self.next_id };
        self.next_id += 1;

        log::debug!("TextureService::create: ({handle:?}: {desc:?})");

        self.descs.insert(handle, desc);
        self.commands.push_back(TextureCommand {
            handle,
            kind: TextureCommandKind::Create { desc: () },
        });
        handle
    }

    /// NOTE: returned buffer points into dirty memory. you need to write each and every byte.
    pub fn get_upload_buf_mut(
        &mut self,
        handle: TextureHandle,
        region: TextureRegion,
    ) -> &mut [u8] {
        let block_size = self
            .descs
            .get(&handle)
            .map(|desc| desc.format.block_size())
            .expect("invalid handle");
        let buffer_size = (region.w * region.h * block_size as u32) as usize;

        let buf_range = self
            .range_alloc
            .allocate(buffer_size)
            // buf can't fit region, grow it.
            .unwrap_or_else(|_| {
                let full_range = self.range_alloc.full_range();
                let delta = full_range.len().max(buffer_size);
                let new_end = full_range.end + delta;

                self.buf.reserve_exact(delta);
                assert!(self.buf.capacity() == new_end);
                unsafe { self.buf.set_len(new_end) };

                self.range_alloc.grow(new_end);
                self.range_alloc.allocate(buffer_size).unwrap()
            });

        self.commands.push_back(TextureCommand {
            handle,
            kind: TextureCommandKind::Upload {
                region,
                buf: buf_range.clone(),
            },
        });

        &mut self.buf[buf_range]
    }

    pub fn delete(&mut self, handle: TextureHandle) {
        log::debug!("TextureService::delete: ({handle:?})");

        let desc = self.descs.remove(&handle);
        assert!(desc.is_some());
        self.commands.push_back(TextureCommand {
            handle,
            kind: TextureCommandKind::Delete,
        });
    }

    pub fn drain_comands(&mut self) -> impl Iterator<Item = TextureCommand<&TextureDesc, &[u8]>> {
        iter::from_fn(|| {
            let TextureCommand {
                handle,
                kind: staging_kind,
            } = self.commands.pop_front()?;
            let ret_kind = match staging_kind {
                TextureCommandKind::Create { desc: _ } => TextureCommandKind::Create {
                    desc: self.descs.get(&handle).expect("invalid handle"),
                },
                TextureCommandKind::Upload {
                    region,
                    buf: buf_range,
                } => {
                    // NOTE: we want to make popped range available for reuse.
                    self.range_alloc.deallocate(buf_range.clone());
                    TextureCommandKind::Upload {
                        region,
                        buf: &self.buf[buf_range],
                    }
                }
                TextureCommandKind::Delete => TextureCommandKind::Delete,
            };
            Some(TextureCommand {
                handle,
                kind: ret_kind,
            })
        })
    }
}
