use crate::{
    AtlasKey, AtlasTextureId, AtlasTextureKind, AtlasTile, Bounds, DevicePixels, PlatformAtlas,
    Point, Size, platform::AtlasTextureList,
};
use anyhow::{Context as _, Result};
use collections::FxHashMap;
use derive_more::{Deref, DerefMut};
use etagere::BucketedAtlasAllocator;
use log::debug;
use metal::Device;
use parking_lot::Mutex;
use std::borrow::Cow;

/// Maximum number of Polychrome textures.
const MAX_POLYCHROME_TEXTURES: usize = 4;

pub(crate) struct MetalAtlas(Mutex<MetalAtlasState>);

impl MetalAtlas {
    pub(crate) fn new(device: Device) -> Self {
        MetalAtlas(Mutex::new(MetalAtlasState {
            device: AssertSend(device),
            monochrome_textures: Default::default(),
            polychrome_textures: Default::default(),
            thumbnail_textures: Default::default(),
            tiles_by_key: Default::default(),
        }))
    }

    pub(crate) fn metal_texture(&self, id: AtlasTextureId) -> metal::Texture {
        self.0.lock().texture(id).metal_texture.clone()
    }
}

struct MetalAtlasState {
    device: AssertSend<Device>,
    monochrome_textures: AtlasTextureList<MetalAtlasTexture>,
    polychrome_textures: AtlasTextureList<MetalAtlasTexture>,
    thumbnail_textures: AtlasTextureList<MetalAtlasTexture>,
    tiles_by_key: FxHashMap<AtlasKey, AtlasTile>,
}

impl PlatformAtlas for MetalAtlas {
    fn get_or_insert_with<'a>(
        &self,
        key: &AtlasKey,
        build: &mut dyn FnMut() -> Result<Option<(Size<DevicePixels>, Cow<'a, [u8]>)>>,
    ) -> Result<Option<AtlasTile>> {
        let Some((size, bytes)) = build()? else {
            return Ok(None);
        };
        let mut lock = self.0.lock();
        if let Some(tile) = lock.tiles_by_key.get(key) {
            Ok(Some(tile.clone()))
        } else {
            let tile = lock
                .allocate(size, key.texture_kind())
                .context("failed to allocate")?;
            let img_id = match key {
                AtlasKey::Image(p) => format!("Image({})", p.image_id.0),
                _ => "other".into(),
            };
            debug!(
                "[atlas] INSERT key={} size={}x{} tile_tid={:?}",
                img_id, size.width.0, size.height.0, tile.texture_id,
            );
            let texture = lock.texture(tile.texture_id);
            texture.upload(tile.bounds, &bytes);
            lock.tiles_by_key.insert(key.clone(), tile.clone());
            Ok(Some(tile))
        }
    }

    fn remove(&self, key: &AtlasKey) {
        let mut lock = self.0.lock();
        let img_id = match key {
            AtlasKey::Image(p) => format!("Image({})", p.image_id.0),
            _ => "other".into(),
        };
        let Some(tile) = lock.tiles_by_key.get(key) else {
            debug!("[atlas] REMOVE key={} NOT_FOUND in tiles_by_key", img_id);
            return;
        };
        let texture_id = tile.texture_id;
        let tile_id: etagere::AllocId = tile.tile_id.into();

        let textures = match texture_id.kind {
            AtlasTextureKind::Monochrome => &mut lock.monochrome_textures,
            AtlasTextureKind::Polychrome => &mut lock.polychrome_textures,
            AtlasTextureKind::Thumbnail => &mut lock.thumbnail_textures,
        };

        let Some(texture_index) = textures
            .textures
            .iter()
            .position(|t| t.as_ref().is_some_and(|v| v.id == texture_id))
        else {
            debug!(
                "[atlas] REMOVE key={} tex={:?} SLOT_IS_NONE",
                img_id, texture_id
            );
            return;
        };

        let unused_count_before = textures
            .textures
            .iter()
            .filter(|t| t.as_ref().is_some_and(|v| v.live_atlas_keys == 0))
            .count();

        let texture_slot = &mut textures.textures[texture_index];

        if let Some(mut texture) = texture_slot.take() {
            let before_free = texture.allocator.free_space();
            let before_alive = texture.live_atlas_keys;
            texture.allocator.deallocate(tile_id);
            texture.decrement_ref_count();
            let after_free = texture.allocator.free_space();
            let after_alive = texture.live_atlas_keys;
            let is_unref = after_alive == 0;

            debug!(
                "[atlas] REMOVE key={} tex={:?} alive={}->{} free={}->{} unref={}",
                img_id, texture_id, before_alive, after_alive, before_free, after_free, is_unref,
            );

            if is_unref {
                // Cap unused textures to limit GPU memory.
                const MAX_UNUSED_POLYCHROME: usize = 4;
                let max_unused = match texture_id.kind {
                    AtlasTextureKind::Polychrome => MAX_UNUSED_POLYCHROME,
                    AtlasTextureKind::Monochrome | AtlasTextureKind::Thumbnail => usize::MAX,
                };

                // unused_count_before doesn't include current (was taken out).
                if unused_count_before < max_unused {
                    texture.reset_allocator();
                    *texture_slot = Some(texture);
                } else {
                    debug!(
                        "[atlas] DROP_UNUSED tex={:?} kind={:?} unused_before={} max={}",
                        texture_id, texture_id.kind, unused_count_before, max_unused,
                    );
                    textures.free_list.push(texture_id.index as usize);
                }
                lock.tiles_by_key.remove(key);
            } else {
                *texture_slot = Some(texture);
            }
        }
    }
}

impl MetalAtlasState {
    fn allocate(
        &mut self,
        size: Size<DevicePixels>,
        texture_kind: AtlasTextureKind,
    ) -> Option<AtlasTile> {
        let evict_id: Option<AtlasTextureId> = {
            let textures = match texture_kind {
                AtlasTextureKind::Monochrome => &mut self.monochrome_textures,
                AtlasTextureKind::Polychrome => &mut self.polychrome_textures,
                AtlasTextureKind::Thumbnail => &mut self.thumbnail_textures,
            };

            if let Some(tile) = textures.iter_mut().rev().find_map(|texture| {
                debug!(
                    "[atlas] ALLOC_TRY tex={:?} kind={:?} live={} free={} want={}x{}",
                    texture.id,
                    texture_kind,
                    texture.live_atlas_keys,
                    texture.allocator.free_space(),
                    size.width.0,
                    size.height.0,
                );
                texture.allocate(size)
            }) {
                debug!(
                    "[atlas] ALLOC reuse tex={:?} size={}x{} tile={:?}",
                    tile.texture_id, size.width.0, size.height.0, tile,
                );
                return Some(tile);
            }

            if texture_kind == AtlasTextureKind::Polychrome {
                let active = textures.textures.iter().filter(|t| t.is_some()).count();
                if active >= MAX_POLYCHROME_TEXTURES {
                    let idx = textures.textures.iter().position(|t| t.is_some()).unwrap();
                    let tex_id = textures.textures[idx].as_ref().unwrap().id;
                    debug!(
                        "[atlas] CAP Polychrome active={} max={} — force-evict {:?}",
                        active, MAX_POLYCHROME_TEXTURES, tex_id,
                    );
                    Some(tex_id)
                } else {
                    debug!(
                        "[atlas] ALLOC no_reuse kind={:?} size={}x{} textures={} free_list={:?}",
                        texture_kind,
                        size.width.0,
                        size.height.0,
                        textures.textures.len(),
                        textures.free_list,
                    );
                    None
                }
            } else {
                debug!(
                    "[atlas] ALLOC no_reuse kind={:?} size={}x{} textures={} free_list={:?}",
                    texture_kind,
                    size.width.0,
                    size.height.0,
                    textures.textures.len(),
                    textures.free_list,
                );
                None
            }
        };

        if let Some(tex_id) = evict_id {
            self.tiles_by_key
                .retain(|_, tile| tile.texture_id != tex_id);
            let idx = tex_id.index as usize;
            let textures = match tex_id.kind {
                AtlasTextureKind::Monochrome => &mut self.monochrome_textures,
                AtlasTextureKind::Polychrome => &mut self.polychrome_textures,
                AtlasTextureKind::Thumbnail => &mut self.thumbnail_textures,
            };
            let texture = textures.textures[idx].as_mut().unwrap();

            if size.width.0 as u64 > texture.metal_texture.width()
                || size.height.0 as u64 > texture.metal_texture.height()
            {
                const DEFAULT_ATLAS_SIZE: Size<DevicePixels> = Size {
                    width: DevicePixels(1024),
                    height: DevicePixels(1024),
                };
                const MAX_ATLAS_SIZE: Size<DevicePixels> = Size {
                    width: DevicePixels(16384),
                    height: DevicePixels(16384),
                };
                let new_size = size.min(&MAX_ATLAS_SIZE).max(&DEFAULT_ATLAS_SIZE);
                let descriptor = metal::TextureDescriptor::new();
                descriptor.set_width(new_size.width.into());
                descriptor.set_height(new_size.height.into());
                descriptor.set_pixel_format(metal::MTLPixelFormat::BGRA8Unorm);
                descriptor.set_usage(metal::MTLTextureUsage::ShaderRead);
                texture.metal_texture = AssertSend(self.device.new_texture(&descriptor));
            }

            texture.reset_allocator();
            let tile = texture.allocate(size)?;
            debug!(
                "[atlas] FORCE_EVICT tex={:?} size={}x{} tile={:?}",
                tex_id, size.width.0, size.height.0, tile,
            );
            return Some(tile);
        }

        let texture = self.push_texture(size, texture_kind);
        texture.allocate(size)
    }

    fn push_texture(
        &mut self,
        min_size: Size<DevicePixels>,
        kind: AtlasTextureKind,
    ) -> &mut MetalAtlasTexture {
        const DEFAULT_ATLAS_SIZE: Size<DevicePixels> = Size {
            width: DevicePixels(1024),
            height: DevicePixels(1024),
        };
        // Max texture size on all modern Apple GPUs. Anything bigger than that crashes in validateWithDevice.
        const MAX_ATLAS_SIZE: Size<DevicePixels> = Size {
            width: DevicePixels(16384),
            height: DevicePixels(16384),
        };
        let size = min_size.min(&MAX_ATLAS_SIZE).max(&DEFAULT_ATLAS_SIZE);
        let texture_descriptor = metal::TextureDescriptor::new();
        texture_descriptor.set_width(size.width.into());
        texture_descriptor.set_height(size.height.into());
        let pixel_format;
        let usage;
        match kind {
            AtlasTextureKind::Monochrome => {
                pixel_format = metal::MTLPixelFormat::A8Unorm;
                usage = metal::MTLTextureUsage::ShaderRead;
            }
            AtlasTextureKind::Polychrome | AtlasTextureKind::Thumbnail => {
                pixel_format = metal::MTLPixelFormat::BGRA8Unorm;
                usage = metal::MTLTextureUsage::ShaderRead;
            }
        }
        texture_descriptor.set_pixel_format(pixel_format);
        texture_descriptor.set_usage(usage);
        let metal_texture = self.device.new_texture(&texture_descriptor);
        let tex_bytes = size.width.0 as u64 * size.height.0 as u64 * 4u64;

        let texture_list = match kind {
            AtlasTextureKind::Monochrome => &mut self.monochrome_textures,
            AtlasTextureKind::Polychrome => &mut self.polychrome_textures,
            AtlasTextureKind::Thumbnail => &mut self.thumbnail_textures,
        };

        let index = texture_list.free_list.pop();

        debug!(
            "[atlas] PUSH_TEXTURE kind={:?} size={}x{} ({} MB) index={:?} total_textures={}",
            kind,
            size.width.0,
            size.height.0,
            tex_bytes / (1024 * 1024),
            index,
            texture_list.textures.len(),
        );

        let atlas_texture = MetalAtlasTexture {
            id: AtlasTextureId {
                index: index.unwrap_or(texture_list.textures.len()) as u32,
                kind,
            },
            allocator: etagere::BucketedAtlasAllocator::new(size.into()),
            metal_texture: AssertSend(metal_texture),
            live_atlas_keys: 0,
        };

        if let Some(ix) = index {
            texture_list.textures[ix] = Some(atlas_texture);
            texture_list.textures.get_mut(ix)
        } else {
            texture_list.textures.push(Some(atlas_texture));
            texture_list.textures.last_mut()
        }
        .unwrap()
        .as_mut()
        .unwrap()
    }

    fn texture(&self, id: AtlasTextureId) -> &MetalAtlasTexture {
        let textures = match id.kind {
            crate::AtlasTextureKind::Monochrome => &self.monochrome_textures,
            crate::AtlasTextureKind::Polychrome => &self.polychrome_textures,
            crate::AtlasTextureKind::Thumbnail => &self.thumbnail_textures,
        };
        textures[id.index as usize].as_ref().unwrap()
    }
}

struct MetalAtlasTexture {
    id: AtlasTextureId,
    allocator: BucketedAtlasAllocator,
    metal_texture: AssertSend<metal::Texture>,
    live_atlas_keys: u32,
}

impl MetalAtlasTexture {
    fn allocate(&mut self, size: Size<DevicePixels>) -> Option<AtlasTile> {
        let before_free = self.allocator.free_space();
        let before_alloc = self.allocator.allocated_space();
        let allocation = self.allocator.allocate(size.into())?;
        let tile = AtlasTile {
            texture_id: self.id,
            tile_id: allocation.id.into(),
            bounds: Bounds {
                origin: allocation.rectangle.min.into(),
                size,
            },
            padding: 0,
        };
        self.live_atlas_keys += 1;
        debug!(
            "[atlas] TILE_ALLOC tex={:?} size={}x{} free={}->{} alloc={}->{} live={}",
            self.id,
            size.width.0,
            size.height.0,
            before_free,
            self.allocator.free_space(),
            before_alloc,
            self.allocator.allocated_space(),
            self.live_atlas_keys,
        );
        Some(tile)
    }

    fn upload(&self, bounds: Bounds<DevicePixels>, bytes: &[u8]) {
        let region = metal::MTLRegion::new_2d(
            bounds.origin.x.into(),
            bounds.origin.y.into(),
            bounds.size.width.into(),
            bounds.size.height.into(),
        );
        self.metal_texture.replace_region(
            region,
            0,
            bytes.as_ptr() as *const _,
            bounds.size.width.to_bytes(self.bytes_per_pixel()) as u64,
        );
    }

    fn bytes_per_pixel(&self) -> u8 {
        use metal::MTLPixelFormat::*;
        match self.metal_texture.pixel_format() {
            A8Unorm | R8Unorm => 1,
            RGBA8Unorm | BGRA8Unorm => 4,
            _ => unimplemented!(),
        }
    }

    fn decrement_ref_count(&mut self) {
        self.live_atlas_keys -= 1;
    }

    fn reset_allocator(&mut self) {
        let size = etagere::Size::new(
            self.metal_texture.width() as i32,
            self.metal_texture.height() as i32,
        );
        self.allocator = etagere::BucketedAtlasAllocator::new(size);
        self.live_atlas_keys = 0;
    }
}

impl From<Size<DevicePixels>> for etagere::Size {
    fn from(size: Size<DevicePixels>) -> Self {
        etagere::Size::new(size.width.into(), size.height.into())
    }
}

impl From<etagere::Point> for Point<DevicePixels> {
    fn from(value: etagere::Point) -> Self {
        Point {
            x: DevicePixels::from(value.x),
            y: DevicePixels::from(value.y),
        }
    }
}

impl From<etagere::Size> for Size<DevicePixels> {
    fn from(size: etagere::Size) -> Self {
        Size {
            width: DevicePixels::from(size.width),
            height: DevicePixels::from(size.height),
        }
    }
}

impl From<etagere::Rectangle> for Bounds<DevicePixels> {
    fn from(rectangle: etagere::Rectangle) -> Self {
        Bounds {
            origin: rectangle.min.into(),
            size: rectangle.size().into(),
        }
    }
}

#[derive(Deref, DerefMut)]
struct AssertSend<T>(T);

unsafe impl<T> Send for AssertSend<T> {}
