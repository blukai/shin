use std::ops::Range;

use mars::alloc;
use mars::array::GrowableArray;
use mars::handlearray::{Handle, HandleArray};
use mars::rangealloc::RangeAlloc;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureHandle(Handle<TextureDesc>);

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
    buf: GrowableArray<u8, alloc::Global>,
    range_alloc: RangeAlloc<usize>,

    // TODO: maybe parametrize texture service with allocator.
    descs: HandleArray<TextureDesc, alloc::Global>,
    commands: GrowableArray<TextureCommand<(), Range<usize>>, alloc::Global>,
}

impl TextureService {
    pub fn create(&mut self, desc: TextureDesc) -> TextureHandle {
        log::debug!("TextureService::create: {desc:?})");

        let handle = TextureHandle(self.descs.push(desc));
        self.commands.push(TextureCommand {
            handle,
            kind: TextureCommandKind::Create { desc: () },
        });
        handle
    }

    /// NOTE: returned buffer points into dirty memory. you need to write each and every byte.
    pub fn get_upload_buf(&mut self, handle: TextureHandle, region: TextureRegion) -> &mut [u8] {
        let desc = self.descs.get(handle.0);
        let buf_size = (region.w * region.h * desc.format.block_size() as u32) as usize;

        let buf_range = if let Ok(buf_range) = self.range_alloc.allocate(buf_size) {
            buf_range
        } else {
            // NOTE: buf can't fit region, grow it.

            let full_range = self.range_alloc.full_range();
            let delta = full_range.len().max(buf_size);
            let new_end = full_range.end + delta;

            self.buf.reserve_exact(delta);
            debug_assert!(self.buf.cap() == new_end);
            unsafe { self.buf.set_len(new_end) };

            self.range_alloc.grow(new_end);
            self.range_alloc.allocate(buf_size).unwrap()
        };

        self.commands.push(TextureCommand {
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

        self.descs.remove(handle.0);
        self.commands.push(TextureCommand {
            handle,
            kind: TextureCommandKind::Delete,
        });
    }

    pub fn drain_comands(&mut self) -> impl Iterator<Item = TextureCommand<&TextureDesc, &[u8]>> {
        self.commands.drain(..).map(|cmd| {
            let kind = match cmd.kind {
                TextureCommandKind::Create { desc: _ } => TextureCommandKind::Create {
                    desc: self.descs.get(cmd.handle.0),
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
            TextureCommand {
                handle: cmd.handle,
                kind,
            }
        })
    }
}
