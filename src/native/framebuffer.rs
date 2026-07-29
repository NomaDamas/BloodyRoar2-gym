#![allow(clippy::too_many_arguments)]

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex, OnceLock};

pub const VRAM_WIDTH: usize = 1024;
pub const VRAM_HEIGHT: usize = 1024;
pub const PSX_VRAM_HEIGHT: usize = 512;
pub const DEFAULT_DISPLAY_WIDTH: usize = 320;
pub const DEFAULT_DISPLAY_HEIGHT: usize = 240;
const GPU_MAX_PRIMITIVE_WIDTH: i32 = 1024;
const GPU_MAX_PRIMITIVE_HEIGHT: i32 = 512;
const RECOVERY_RASTER_CACHE_ENTRY_LIMIT: usize = 8_192;
const RECOVERY_RASTER_CACHE_WRITE_LIMIT: usize = 4_194_304;
const RECOVERY_RASTER_CACHE_SEEN_LIMIT: usize = 32_768;
const RECOVERY_PALETTE_HISTORY_ENTRY_LIMIT: usize = 512;
const VRAM_DEPENDENCY_TILE_SIZE: usize = 16;
const VRAM_DEPENDENCY_TILE_COLUMNS: usize = VRAM_WIDTH / VRAM_DEPENDENCY_TILE_SIZE;
const VRAM_DEPENDENCY_TILE_ROWS: usize = VRAM_HEIGHT / VRAM_DEPENDENCY_TILE_SIZE;
const VRAM_DEPENDENCY_TILE_COUNT: usize = VRAM_DEPENDENCY_TILE_COLUMNS * VRAM_DEPENDENCY_TILE_ROWS;

#[derive(Debug)]
pub struct NativeFrameBuffer {
    pixels: Vec<u32>,
    raw_pixels: Vec<u16>,
    clip: Option<ClipRect>,
    recovery_raster_context: Option<u64>,
    recovery_raster_capture: Option<Vec<(usize, u16)>>,
    recovery_raster_cache: Arc<Mutex<RecoveryRasterCache>>,
    recovery_palette_history: Arc<Mutex<RecoveryPaletteHistory>>,
    vram_dependency_tile_hashes: Vec<u64>,
    vram_dependency_tile_dirty: Vec<bool>,
}

impl Clone for NativeFrameBuffer {
    fn clone(&self) -> Self {
        Self {
            pixels: self.pixels.clone(),
            raw_pixels: self.raw_pixels.clone(),
            clip: self.clip,
            recovery_raster_context: None,
            recovery_raster_capture: None,
            // Recovery transactions clone the GPU every vblank. Share only the
            // immutable-on-hit acceleration cache, never the framebuffer data.
            recovery_raster_cache: Arc::clone(&self.recovery_raster_cache),
            recovery_palette_history: Arc::clone(&self.recovery_palette_history),
            vram_dependency_tile_hashes: self.vram_dependency_tile_hashes.clone(),
            vram_dependency_tile_dirty: self.vram_dependency_tile_dirty.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct RecoveryRasterCacheKey {
    command: u64,
    raster: u64,
}

#[derive(Clone, Debug)]
struct RecoveryRasterCacheEntry {
    writes: Arc<Vec<(usize, u16)>>,
    stats: [TexturedDrawStats; 2],
}

#[derive(Debug, Default)]
struct RecoveryRasterCache {
    entries: HashMap<RecoveryRasterCacheKey, RecoveryRasterCacheEntry>,
    order: VecDeque<RecoveryRasterCacheKey>,
    seen: HashSet<RecoveryRasterCacheKey>,
    seen_order: VecDeque<RecoveryRasterCacheKey>,
    writes: usize,
    hits: u64,
    misses: u64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct RecoveryPaletteHistoryKey {
    texture_page: u16,
    clut: u16,
    texture_signature: u64,
}

#[derive(Debug, Default)]
struct RecoveryPaletteHistory {
    entries: HashMap<RecoveryPaletteHistoryKey, [u16; 16]>,
    order: VecDeque<RecoveryPaletteHistoryKey>,
}

#[derive(Clone, Debug)]
pub(crate) struct FrameBufferRegionSnapshot {
    rows: Vec<FrameBufferRegionSnapshotRow>,
    raw_pixels: Vec<u16>,
}

#[derive(Clone, Copy, Debug)]
struct FrameBufferRegionSnapshotRow {
    vram_start: usize,
    raw_start: usize,
    len: usize,
}

#[derive(Clone, Debug)]
pub struct TextureCandidateImage {
    pub label: String,
    pub x: i32,
    pub y: i32,
    pub raw_width: usize,
    pub raw_height: usize,
    pub decoded_png: Vec<u8>,
    pub raw_png: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PixelWriteOptions {
    pub set_mask_bit: bool,
    pub check_mask_bit: bool,
    pub semi_transparent: bool,
    pub semi_transparency_mode: u8,
}

impl Default for NativeFrameBuffer {
    fn default() -> Self {
        Self {
            pixels: vec![0; VRAM_WIDTH * VRAM_HEIGHT],
            raw_pixels: vec![0; VRAM_WIDTH * VRAM_HEIGHT],
            clip: None,
            recovery_raster_context: None,
            recovery_raster_capture: None,
            recovery_raster_cache: Arc::new(Mutex::new(RecoveryRasterCache::default())),
            recovery_palette_history: Arc::new(Mutex::new(RecoveryPaletteHistory::default())),
            vram_dependency_tile_hashes: vec![0; VRAM_DEPENDENCY_TILE_COUNT],
            vram_dependency_tile_dirty: vec![true; VRAM_DEPENDENCY_TILE_COUNT],
        }
    }
}

impl NativeFrameBuffer {
    pub fn set_clip(&mut self, clip: Option<ClipRect>) {
        self.clip = clip;
    }

    pub fn set_pixel(&mut self, x: i32, y: i32, color: u32) {
        self.set_pixel_with_options(x, y, color, PixelWriteOptions::default());
    }

    pub fn set_pixel_with_options(
        &mut self,
        x: i32,
        y: i32,
        color: u32,
        options: PixelWriteOptions,
    ) -> bool {
        if !self.in_clip(x, y) {
            return false;
        }
        if x < 0 || y < 0 || x >= VRAM_WIDTH as i32 || y >= VRAM_HEIGHT as i32 {
            return false;
        }

        let index = y as usize * VRAM_WIDTH + x as usize;
        let rgb = color & 0x00ff_ffff;
        self.write_raw_pixel_index(index, rgb888_to_rgb555(rgb), options)
    }

    pub fn set_raw_pixel(&mut self, x: i32, y: i32, color: u16) -> bool {
        self.set_raw_pixel_with_options(x, y, color, PixelWriteOptions::default())
    }

    pub fn set_raw_pixel_with_options(
        &mut self,
        x: i32,
        y: i32,
        color: u16,
        options: PixelWriteOptions,
    ) -> bool {
        if !self.in_clip(x, y) {
            return false;
        }
        self.set_raw_pixel_unclipped_with_options(x, y, color, options)
    }

    fn set_raw_pixel_unclipped_with_options(
        &mut self,
        x: i32,
        y: i32,
        color: u16,
        options: PixelWriteOptions,
    ) -> bool {
        if x < 0 || y < 0 || x >= VRAM_WIDTH as i32 || y >= VRAM_HEIGHT as i32 {
            return false;
        }

        let index = y as usize * VRAM_WIDTH + x as usize;
        self.write_raw_pixel_index(index, color, options)
    }

    fn write_raw_pixel_index(
        &mut self,
        index: usize,
        color: u16,
        options: PixelWriteOptions,
    ) -> bool {
        if options.check_mask_bit && self.raw_pixels[index] & 0x8000 != 0 {
            return false;
        }

        let color = if options.semi_transparent {
            blend_rgb555(
                color,
                self.raw_pixels[index],
                options.semi_transparency_mode,
            )
        } else {
            color
        };
        let color = if options.set_mask_bit {
            color | 0x8000
        } else {
            color
        };
        if self.raw_pixels[index] != color {
            self.raw_pixels[index] = color;
            self.mark_vram_dependency_tile_dirty(index);
        }
        self.pixels[index] = rgb555_to_rgb888(color);
        if let Some(capture) = self.recovery_raster_capture.as_mut() {
            capture.push((index, color));
        }
        true
    }

    pub(crate) fn begin_recovery_raster_command(&mut self, fingerprint: u64) {
        self.recovery_raster_context = Some(fingerprint);
        self.recovery_raster_capture = None;
    }

    pub(crate) fn end_recovery_raster_command(&mut self) {
        self.recovery_raster_capture = None;
        self.recovery_raster_context = None;
    }

    pub(crate) fn recovery_raster_cache_stats(&self) -> (u64, u64, usize, usize) {
        let cache = self
            .recovery_raster_cache
            .lock()
            .expect("recovery raster cache lock");
        (cache.hits, cache.misses, cache.entries.len(), cache.writes)
    }

    fn recovery_textured_raster_key(
        &mut self,
        kind: u64,
        points: &[TexturedPoint],
        extra: &[u64],
        texture_page: u16,
        clut: u16,
        options: TextureDrawOptions,
        texture_window: TextureWindow,
    ) -> Option<RecoveryRasterCacheKey> {
        if native_disable_recovery_raster_cache() {
            return None;
        }
        let command = self.recovery_raster_context?;
        if options.semi_transparent || options.check_mask_bit {
            return None;
        }

        let mut raster = recovery_raster_mix(0xcbf2_9ce4_8422_2325, kind);
        raster = recovery_raster_mix(raster, u64::from(texture_page));
        raster = recovery_raster_mix(raster, u64::from(clut));
        raster = recovery_raster_mix(raster, u64::from(options.primitive_color));
        raster = recovery_raster_mix(raster, u64::from(options.raw_texture));
        raster = recovery_raster_mix(raster, u64::from(options.set_mask_bit));
        raster = recovery_raster_mix(raster, u64::from(options.texture_flip_x));
        raster = recovery_raster_mix(raster, u64::from(options.texture_flip_y));
        raster = recovery_raster_mix(raster, u64::from(options.allow_palette_fallback));
        raster = recovery_raster_mix(raster, u64::from(options.allow_texture_descriptor_alias));
        raster = recovery_raster_mix(raster, u64::from(texture_window.gp0_value()));
        if let Some(clip) = self.clip {
            for value in [clip.left, clip.top, clip.right, clip.bottom] {
                raster = recovery_raster_mix(raster, value as u32 as u64);
            }
        } else {
            raster = recovery_raster_mix(raster, u64::MAX);
        }
        for point in points {
            raster = recovery_raster_mix(raster, point.point.x as u32 as u64);
            raster = recovery_raster_mix(raster, point.point.y as u32 as u64);
            raster = recovery_raster_mix(raster, u64::from(point.u));
            raster = recovery_raster_mix(raster, u64::from(point.v));
        }
        for value in extra {
            raster = recovery_raster_mix(raster, *value);
        }
        raster = recovery_raster_mix(
            raster,
            self.recovery_texture_dependency_fingerprint(
                texture_page,
                clut,
                TextureSamplingPolicy::from_draw_options(options),
            ),
        );

        Some(RecoveryRasterCacheKey { command, raster })
    }

    fn recovery_texture_dependency_fingerprint(
        &mut self,
        texture_page: u16,
        clut: u16,
        sampling_policy: TextureSamplingPolicy,
    ) -> u64 {
        let (texture_page, clut) = sampling_policy.resolve_descriptor(texture_page, clut);
        let mut fingerprint = 0x243f_6a88_85a3_08d3;
        fingerprint = self.mix_vram_dependency_bounds(
            fingerprint,
            texture_page_raw_bounds_for_clut(texture_page, clut),
        );

        if let Some(palette_bounds) = texture_palette_raw_bounds(texture_page, clut) {
            fingerprint = self.mix_vram_dependency_bounds(fingerprint, palette_bounds);
            if sampling_policy.allow_palette_fallback {
                fingerprint = self.mix_vram_dependency_bounds(
                    fingerprint,
                    recovery_palette_fallback_dependency_bounds(texture_page, clut),
                );
            }
        }

        fingerprint
    }

    fn mix_vram_dependency_bounds(
        &mut self,
        mut fingerprint: u64,
        bounds: (i32, i32, i32, i32),
    ) -> u64 {
        let (left, top, right, bottom) = bounds;
        let left = left.clamp(0, VRAM_WIDTH as i32 - 1) as usize;
        let top = top.clamp(0, VRAM_HEIGHT as i32 - 1) as usize;
        let right = right.clamp(0, VRAM_WIDTH as i32 - 1) as usize;
        let bottom = bottom.clamp(0, VRAM_HEIGHT as i32 - 1) as usize;
        if left > right || top > bottom {
            return recovery_raster_mix(fingerprint, u64::MAX);
        }

        let first_tile_x = left / VRAM_DEPENDENCY_TILE_SIZE;
        let last_tile_x = right / VRAM_DEPENDENCY_TILE_SIZE;
        let first_tile_y = top / VRAM_DEPENDENCY_TILE_SIZE;
        let last_tile_y = bottom / VRAM_DEPENDENCY_TILE_SIZE;
        for tile_y in first_tile_y..=last_tile_y {
            for tile_x in first_tile_x..=last_tile_x {
                let tile_index = tile_y * VRAM_DEPENDENCY_TILE_COLUMNS + tile_x;
                let tile_hash = self.vram_dependency_tile_hash(tile_index);
                fingerprint = recovery_raster_mix(fingerprint, tile_index as u64);
                fingerprint = recovery_raster_mix(fingerprint, tile_hash);
            }
        }
        fingerprint
    }

    fn vram_dependency_tile_hash(&mut self, tile_index: usize) -> u64 {
        if !self.vram_dependency_tile_dirty[tile_index] {
            return self.vram_dependency_tile_hashes[tile_index];
        }

        let tile_x = tile_index % VRAM_DEPENDENCY_TILE_COLUMNS;
        let tile_y = tile_index / VRAM_DEPENDENCY_TILE_COLUMNS;
        let start_x = tile_x * VRAM_DEPENDENCY_TILE_SIZE;
        let start_y = tile_y * VRAM_DEPENDENCY_TILE_SIZE;
        let mut hash = recovery_raster_mix(0x1319_8a2e_0370_7344, tile_index as u64);
        for y in start_y..(start_y + VRAM_DEPENDENCY_TILE_SIZE).min(VRAM_HEIGHT) {
            let row_start = y * VRAM_WIDTH + start_x;
            let row_end = row_start + VRAM_DEPENDENCY_TILE_SIZE.min(VRAM_WIDTH - start_x);
            for raw in &self.raw_pixels[row_start..row_end] {
                hash = recovery_raster_mix(hash, u64::from(*raw));
            }
        }
        self.vram_dependency_tile_hashes[tile_index] = hash;
        self.vram_dependency_tile_dirty[tile_index] = false;
        hash
    }

    fn mark_vram_dependency_tile_dirty(&mut self, pixel_index: usize) {
        let x = pixel_index % VRAM_WIDTH;
        let y = pixel_index / VRAM_WIDTH;
        let tile_index = (y / VRAM_DEPENDENCY_TILE_SIZE) * VRAM_DEPENDENCY_TILE_COLUMNS
            + x / VRAM_DEPENDENCY_TILE_SIZE;
        self.vram_dependency_tile_dirty[tile_index] = true;
    }

    fn try_replay_recovery_raster(
        &mut self,
        key: RecoveryRasterCacheKey,
    ) -> Option<[TexturedDrawStats; 2]> {
        let entry = {
            let mut cache = self
                .recovery_raster_cache
                .lock()
                .expect("recovery raster cache lock");
            let entry = cache.entries.get(&key).cloned();
            if entry.is_some() {
                cache.hits = cache.hits.saturating_add(1);
            }
            entry
        };
        if let Some(entry) = entry {
            for (index, raw) in entry.writes.iter().copied() {
                if self.raw_pixels[index] != raw {
                    self.raw_pixels[index] = raw;
                    self.mark_vram_dependency_tile_dirty(index);
                }
                self.pixels[index] = rgb555_to_rgb888(raw);
            }
            return Some(entry.stats);
        }

        let repeated = {
            let mut cache = self
                .recovery_raster_cache
                .lock()
                .expect("recovery raster cache lock");
            cache.misses = cache.misses.saturating_add(1);
            let repeated = cache.seen.contains(&key);
            if !repeated {
                cache.seen.insert(key);
                cache.seen_order.push_back(key);
                while cache.seen.len() > RECOVERY_RASTER_CACHE_SEEN_LIMIT {
                    let Some(oldest) = cache.seen_order.pop_front() else {
                        break;
                    };
                    cache.seen.remove(&oldest);
                }
            }
            repeated
        };
        if repeated {
            self.recovery_raster_capture = Some(Vec::new());
        }
        None
    }

    fn finish_recovery_raster(
        &mut self,
        key: Option<RecoveryRasterCacheKey>,
        stats: [TexturedDrawStats; 2],
        source_independent: bool,
    ) {
        let writes = self.recovery_raster_capture.take();
        let Some(key) = key else {
            return;
        };
        let Some(mut writes) = writes else {
            return;
        };
        if !source_independent || writes.len() > RECOVERY_RASTER_CACHE_WRITE_LIMIT {
            return;
        }

        // Opaque triangles can touch the same edge pixel more than once. Keep
        // only the final value so a hit is a compact direct framebuffer patch.
        let mut retained = HashSet::with_capacity(writes.len());
        writes.reverse();
        writes.retain(|(index, _)| retained.insert(*index));
        writes.reverse();

        let mut cache = self
            .recovery_raster_cache
            .lock()
            .expect("recovery raster cache lock");
        if let Some(previous) = cache.entries.remove(&key) {
            cache.writes = cache.writes.saturating_sub(previous.writes.len());
            if let Some(position) = cache.order.iter().position(|queued| *queued == key) {
                cache.order.remove(position);
            }
        }
        while cache.entries.len() >= RECOVERY_RASTER_CACHE_ENTRY_LIMIT
            || cache.writes.saturating_add(writes.len()) > RECOVERY_RASTER_CACHE_WRITE_LIMIT
        {
            let Some(oldest) = cache.order.pop_front() else {
                break;
            };
            if let Some(entry) = cache.entries.remove(&oldest) {
                cache.writes = cache.writes.saturating_sub(entry.writes.len());
            }
        }
        cache.writes = cache.writes.saturating_add(writes.len());
        cache.entries.insert(
            key,
            RecoveryRasterCacheEntry {
                writes: Arc::new(writes),
                stats,
            },
        );
        cache.order.push_back(key);
    }

    pub fn pixel(&self, x: i32, y: i32) -> u32 {
        if x < 0 || y < 0 || x >= VRAM_WIDTH as i32 || y >= VRAM_HEIGHT as i32 {
            return 0;
        }

        self.pixels[y as usize * VRAM_WIDTH + x as usize]
    }

    pub fn raw_pixel(&self, x: i32, y: i32) -> u16 {
        if x < 0 || y < 0 || x >= VRAM_WIDTH as i32 || y >= VRAM_HEIGHT as i32 {
            return 0;
        }

        self.raw_pixels[y as usize * VRAM_WIDTH + x as usize]
    }

    pub(crate) fn snapshot_psx_display_region(
        &self,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
    ) -> FrameBufferRegionSnapshot {
        let row_segments = if width == 0 {
            0
        } else {
            (x % VRAM_WIDTH).saturating_add(width).div_ceil(VRAM_WIDTH)
        };
        let mut rows = Vec::with_capacity(height.saturating_mul(row_segments));
        let mut raw_pixels = Vec::with_capacity(width.saturating_mul(height));
        for row in 0..height {
            let source_y = y.wrapping_add(row) % PSX_VRAM_HEIGHT;
            let mut column = 0usize;
            while column < width {
                let source_x = x.wrapping_add(column) % VRAM_WIDTH;
                let len = (width - column).min(VRAM_WIDTH - source_x);
                let vram_start = source_y * VRAM_WIDTH + source_x;
                let raw_start = raw_pixels.len();
                raw_pixels.extend_from_slice(&self.raw_pixels[vram_start..vram_start + len]);
                rows.push(FrameBufferRegionSnapshotRow {
                    vram_start,
                    raw_start,
                    len,
                });
                column += len;
            }
        }
        FrameBufferRegionSnapshot { rows, raw_pixels }
    }

    pub(crate) fn restore_region_snapshot(&mut self, snapshot: &FrameBufferRegionSnapshot) {
        for row in &snapshot.rows {
            let raw_end = row.raw_start + row.len;
            let vram_end = row.vram_start + row.len;
            let raw_slice = &snapshot.raw_pixels[row.raw_start..raw_end];
            for (offset, raw) in raw_slice.iter().copied().enumerate() {
                let index = row.vram_start + offset;
                if self.raw_pixels[index] != raw {
                    self.mark_vram_dependency_tile_dirty(index);
                }
            }
            self.raw_pixels[row.vram_start..vram_end].copy_from_slice(raw_slice);
            for (pixel, raw) in self.pixels[row.vram_start..vram_end]
                .iter_mut()
                .zip(raw_slice.iter().copied())
            {
                *pixel = rgb555_to_rgb888(raw);
            }
        }
    }

    pub fn fill_rect_unclipped(&mut self, x: i32, y: i32, width: i32, height: i32, color: u32) {
        self.fill_rect_unclipped_with_options(
            x,
            y,
            width,
            height,
            color,
            PixelWriteOptions::default(),
        );
    }

    pub fn fill_rect_unclipped_with_options(
        &mut self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        color: u32,
        options: PixelWriteOptions,
    ) {
        if width <= 0 || height <= 0 {
            return;
        }

        let left = x.clamp(0, VRAM_WIDTH as i32) as usize;
        let top = y.clamp(0, VRAM_HEIGHT as i32) as usize;
        let right = x.saturating_add(width).clamp(0, VRAM_WIDTH as i32) as usize;
        let bottom = y.saturating_add(height).clamp(0, VRAM_HEIGHT as i32) as usize;
        if left >= right || top >= bottom {
            return;
        }

        self.fill_rect_region(left, top, right, bottom, color, options);
    }

    pub fn fill_rect(&mut self, x: i32, y: i32, width: i32, height: i32, color: u32) {
        self.fill_rect_with_options(x, y, width, height, color, PixelWriteOptions::default());
    }

    pub fn fill_rect_with_options(
        &mut self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        color: u32,
        options: PixelWriteOptions,
    ) {
        if width <= 0 || height <= 0 {
            return;
        }

        let (clip_left, clip_top, clip_right, clip_bottom) = self.clip_bounds();
        let left = x.clamp(clip_left, clip_right) as usize;
        let top = y.clamp(clip_top, clip_bottom) as usize;
        let right = x.saturating_add(width).clamp(clip_left, clip_right) as usize;
        let bottom = y.saturating_add(height).clamp(clip_top, clip_bottom) as usize;
        if left >= right || top >= bottom {
            return;
        }

        self.fill_rect_region(left, top, right, bottom, color, options);
    }

    fn fill_rect_region(
        &mut self,
        left: usize,
        top: usize,
        right: usize,
        bottom: usize,
        color: u32,
        options: PixelWriteOptions,
    ) {
        let rgb = color & 0x00ff_ffff;
        let raw = rgb888_to_rgb555(rgb);
        for row in top..bottom {
            let offset = row * VRAM_WIDTH;
            for col in left..right {
                self.write_raw_pixel_index(offset + col, raw, options);
            }
        }
    }

    pub fn copy_rect(
        &mut self,
        source_x: i32,
        source_y: i32,
        dest_x: i32,
        dest_y: i32,
        width: i32,
        height: i32,
    ) {
        self.copy_rect_with_options(
            source_x,
            source_y,
            dest_x,
            dest_y,
            width,
            height,
            PixelWriteOptions::default(),
        );
    }

    pub fn copy_rect_with_options(
        &mut self,
        source_x: i32,
        source_y: i32,
        dest_x: i32,
        dest_y: i32,
        width: i32,
        height: i32,
        options: PixelWriteOptions,
    ) {
        if width <= 0 || height <= 0 {
            return;
        }

        let mut copied = Vec::with_capacity((width as usize).saturating_mul(height as usize));
        for row in 0..height {
            for col in 0..width {
                let source_x = wrap_vram_x(source_x + col);
                let source_y = wrap_vram_y(source_y + row);
                copied.push((
                    self.pixel(source_x, source_y),
                    self.raw_pixel(source_x, source_y),
                ));
            }
        }

        for row in 0..height {
            for col in 0..width {
                let index = (row as usize)
                    .saturating_mul(width as usize)
                    .saturating_add(col as usize);
                if let Some((rgb, raw)) = copied.get(index) {
                    let x = wrap_vram_x(dest_x + col);
                    let y = wrap_vram_y(dest_y + row);
                    let dest_index = y as usize * VRAM_WIDTH + x as usize;
                    if self.write_raw_pixel_index(dest_index, *raw, options) {
                        self.pixels[dest_index] = *rgb;
                    }
                }
            }
        }
    }

    pub fn write_rgb555_image(&mut self, x: i32, y: i32, width: i32, height: i32, words: &[u32]) {
        self.write_rgb555_image_with_options(
            x,
            y,
            width,
            height,
            words,
            PixelWriteOptions::default(),
        );
    }

    pub fn write_rgb555_image_with_options(
        &mut self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        words: &[u32],
        options: PixelWriteOptions,
    ) {
        if width <= 0 || height <= 0 {
            return;
        }

        let pixels = (width as usize).saturating_mul(height as usize);
        for index in 0..pixels {
            let Some(word) = words.get(index / 2) else {
                break;
            };
            let raw = if index & 1 == 0 {
                word & 0xffff
            } else {
                word >> 16
            };
            let col = (index % width as usize) as i32;
            let row = (index / width as usize) as i32;
            self.set_raw_pixel_unclipped_with_options(
                wrap_vram_x(x + col),
                wrap_vram_y(y + row),
                raw as u16,
                options,
            );
        }
    }

    pub fn draw_line(&mut self, a: Point, b: Point, color: u32) {
        self.draw_line_with_options(a, b, color, PixelWriteOptions::default());
    }

    pub fn draw_line_with_options(
        &mut self,
        a: Point,
        b: Point,
        color: u32,
        options: PixelWriteOptions,
    ) {
        let mut x0 = a.x;
        let mut y0 = a.y;
        let x1 = b.x;
        let y1 = b.y;
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;

        loop {
            self.set_pixel_with_options(x0, y0, color, options);
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = err * 2;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }

    pub fn draw_triangle(&mut self, a: Point, b: Point, c: Point, color: u32) {
        self.draw_triangle_with_options(a, b, c, color, PixelWriteOptions::default());
    }

    pub fn draw_triangle_with_options(
        &mut self,
        a: Point,
        b: Point,
        c: Point,
        color: u32,
        options: PixelWriteOptions,
    ) {
        if triangle_exceeds_gpu_size_limit(a, b, c) {
            return;
        }
        let (clip_left, clip_top, clip_right, clip_bottom) = self.clip_bounds();
        if clip_left >= clip_right || clip_top >= clip_bottom {
            return;
        }

        let min_x = a.x.min(b.x).min(c.x).clamp(clip_left, clip_right - 1);
        let max_x = a.x.max(b.x).max(c.x).clamp(clip_left, clip_right - 1);
        let min_y = a.y.min(b.y).min(c.y).clamp(clip_top, clip_bottom - 1);
        let max_y = a.y.max(b.y).max(c.y).clamp(clip_top, clip_bottom - 1);
        if min_x > max_x || min_y > max_y {
            return;
        }

        let area = edge(a, b, c);
        if area == 0 {
            return;
        }

        let rgb = color & 0x00ff_ffff;
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let point = Point { x, y };
                if psx_triangle_weights(a, b, c, point, area).is_some() {
                    self.set_pixel_with_options(x, y, rgb, options);
                }
            }
        }
    }

    pub fn draw_shaded_triangle_with_options(
        &mut self,
        a: Point,
        b: Point,
        c: Point,
        colors: [u32; 3],
        options: PixelWriteOptions,
    ) {
        if triangle_exceeds_gpu_size_limit(a, b, c) {
            return;
        }
        let (clip_left, clip_top, clip_right, clip_bottom) = self.clip_bounds();
        if clip_left >= clip_right || clip_top >= clip_bottom {
            return;
        }

        let min_x = a.x.min(b.x).min(c.x).clamp(clip_left, clip_right - 1);
        let max_x = a.x.max(b.x).max(c.x).clamp(clip_left, clip_right - 1);
        let min_y = a.y.min(b.y).min(c.y).clamp(clip_top, clip_bottom - 1);
        let max_y = a.y.max(b.y).max(c.y).clamp(clip_top, clip_bottom - 1);
        if min_x > max_x || min_y > max_y {
            return;
        }

        let area = edge(a, b, c);
        if area == 0 {
            return;
        }

        let denom = area.unsigned_abs() as i64;
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let point = Point { x, y };
                let Some((w0, w1, w2)) = psx_triangle_weights(a, b, c, point, area) else {
                    continue;
                };

                let color = interpolate_psx_rgb(colors, w0, w1, w2, denom);
                self.set_pixel_with_options(x, y, psx_rgb_to_rgb888(color), options);
            }
        }
    }

    pub fn draw_textured_triangle(
        &mut self,
        a: TexturedPoint,
        b: TexturedPoint,
        c: TexturedPoint,
        texture_page: u16,
        clut: u16,
        options: TextureDrawOptions,
        texture_window: TextureWindow,
    ) -> TexturedDrawStats {
        let Some(bounds) = self.textured_triangle_raster_bounds(a, b, c) else {
            return TexturedDrawStats::default();
        };
        let cache_key = self.recovery_textured_raster_key(
            1,
            &[a, b, c],
            &[],
            texture_page,
            clut,
            options,
            texture_window,
        );
        if let Some(key) = cache_key
            && let Some(stats) = self.try_replay_recovery_raster(key)
        {
            return stats[0];
        }
        let sampling_policy = TextureSamplingPolicy::from_draw_options(options);
        let resources = PreparedTextureDrawResources::new(
            self,
            bounds.dest_bounds(),
            texture_page,
            clut,
            sampling_policy,
        );
        let source_independent = resources.source_snapshot.is_none();
        let stats = self.draw_textured_triangle_with_resources(
            a,
            b,
            c,
            bounds,
            &resources,
            options,
            texture_window,
        );
        self.finish_recovery_raster(
            cache_key,
            [stats, TexturedDrawStats::default()],
            source_independent,
        );
        stats
    }

    pub fn draw_shaded_textured_triangle(
        &mut self,
        a: TexturedPoint,
        b: TexturedPoint,
        c: TexturedPoint,
        colors: [u32; 3],
        texture_page: u16,
        clut: u16,
        options: TextureDrawOptions,
        texture_window: TextureWindow,
    ) -> TexturedDrawStats {
        let Some(bounds) = self.textured_triangle_raster_bounds(a, b, c) else {
            return TexturedDrawStats::default();
        };
        let cache_key = self.recovery_textured_raster_key(
            2,
            &[a, b, c],
            &colors.map(u64::from),
            texture_page,
            clut,
            options,
            texture_window,
        );
        if let Some(key) = cache_key
            && let Some(stats) = self.try_replay_recovery_raster(key)
        {
            return stats[0];
        }
        let sampling_policy = TextureSamplingPolicy::from_draw_options(options);
        let resources = PreparedTextureDrawResources::new(
            self,
            bounds.dest_bounds(),
            texture_page,
            clut,
            sampling_policy,
        );
        let source_independent = resources.source_snapshot.is_none();
        let stats = self.draw_shaded_textured_triangle_with_resources(
            a,
            b,
            c,
            colors,
            bounds,
            &resources,
            options,
            texture_window,
        );
        self.finish_recovery_raster(
            cache_key,
            [stats, TexturedDrawStats::default()],
            source_independent,
        );
        stats
    }

    pub fn draw_textured_quad_triangles_shared(
        &mut self,
        first: [TexturedPoint; 3],
        second: [TexturedPoint; 3],
        texture_page: u16,
        clut: u16,
        options: TextureDrawOptions,
        texture_window: TextureWindow,
    ) -> (TexturedDrawStats, TexturedDrawStats) {
        let first_bounds = self.textured_triangle_raster_bounds(first[0], first[1], first[2]);
        let second_bounds = self.textured_triangle_raster_bounds(second[0], second[1], second[2]);
        let Some(dest_bounds) = shared_triangle_dest_bounds(first_bounds, second_bounds) else {
            return (TexturedDrawStats::default(), TexturedDrawStats::default());
        };
        let points = [
            first[0], first[1], first[2], second[0], second[1], second[2],
        ];
        let cache_key = self.recovery_textured_raster_key(
            3,
            &points,
            &[],
            texture_page,
            clut,
            options,
            texture_window,
        );
        if let Some(key) = cache_key
            && let Some(stats) = self.try_replay_recovery_raster(key)
        {
            return (stats[0], stats[1]);
        }

        let sampling_policy = TextureSamplingPolicy::from_draw_options(options);
        let resources = PreparedTextureDrawResources::new(
            self,
            dest_bounds,
            texture_page,
            clut,
            sampling_policy,
        );
        let source_independent = resources.source_snapshot.is_none();
        let first_stats = first_bounds.map_or_else(TexturedDrawStats::default, |bounds| {
            self.draw_textured_triangle_with_resources(
                first[0],
                first[1],
                first[2],
                bounds,
                &resources,
                options,
                texture_window,
            )
        });
        let second_stats = second_bounds.map_or_else(TexturedDrawStats::default, |bounds| {
            self.draw_textured_triangle_with_resources(
                second[0],
                second[1],
                second[2],
                bounds,
                &resources,
                options,
                texture_window,
            )
        });
        self.finish_recovery_raster(cache_key, [first_stats, second_stats], source_independent);
        (first_stats, second_stats)
    }

    pub fn draw_shaded_textured_quad_triangles_shared(
        &mut self,
        first: [TexturedPoint; 3],
        first_colors: [u32; 3],
        second: [TexturedPoint; 3],
        second_colors: [u32; 3],
        texture_page: u16,
        clut: u16,
        options: TextureDrawOptions,
        texture_window: TextureWindow,
    ) -> (TexturedDrawStats, TexturedDrawStats) {
        let first_bounds = self.textured_triangle_raster_bounds(first[0], first[1], first[2]);
        let second_bounds = self.textured_triangle_raster_bounds(second[0], second[1], second[2]);
        let Some(dest_bounds) = shared_triangle_dest_bounds(first_bounds, second_bounds) else {
            return (TexturedDrawStats::default(), TexturedDrawStats::default());
        };
        let points = [
            first[0], first[1], first[2], second[0], second[1], second[2],
        ];
        let cache_key = self.recovery_textured_raster_key(
            4,
            &points,
            &[
                u64::from(first_colors[0]),
                u64::from(first_colors[1]),
                u64::from(first_colors[2]),
                u64::from(second_colors[0]),
                u64::from(second_colors[1]),
                u64::from(second_colors[2]),
            ],
            texture_page,
            clut,
            options,
            texture_window,
        );
        if let Some(key) = cache_key
            && let Some(stats) = self.try_replay_recovery_raster(key)
        {
            return (stats[0], stats[1]);
        }

        let sampling_policy = TextureSamplingPolicy::from_draw_options(options);
        let resources = PreparedTextureDrawResources::new(
            self,
            dest_bounds,
            texture_page,
            clut,
            sampling_policy,
        );
        let source_independent = resources.source_snapshot.is_none();
        let first_stats = first_bounds.map_or_else(TexturedDrawStats::default, |bounds| {
            self.draw_shaded_textured_triangle_with_resources(
                first[0],
                first[1],
                first[2],
                first_colors,
                bounds,
                &resources,
                options,
                texture_window,
            )
        });
        let second_stats = second_bounds.map_or_else(TexturedDrawStats::default, |bounds| {
            self.draw_shaded_textured_triangle_with_resources(
                second[0],
                second[1],
                second[2],
                second_colors,
                bounds,
                &resources,
                options,
                texture_window,
            )
        });
        self.finish_recovery_raster(cache_key, [first_stats, second_stats], source_independent);
        (first_stats, second_stats)
    }

    pub fn draw_textured_rect(
        &mut self,
        dest: Point,
        size: (i32, i32),
        texture_page: u16,
        clut: u16,
        uv: TextureCoordinate,
        options: TextureDrawOptions,
        texture_window: TextureWindow,
    ) -> TexturedDrawStats {
        let mut stats = TexturedDrawStats::default();
        let (width, height) = size;
        if width <= 0 || height <= 0 {
            return stats;
        }

        let dest_bounds = (
            dest.x,
            dest.y,
            dest.x.saturating_add(width).saturating_sub(1),
            dest.y.saturating_add(height).saturating_sub(1),
        );
        let textured_point = TexturedPoint {
            point: dest,
            u: uv.u,
            v: uv.v,
        };
        let cache_key = self.recovery_textured_raster_key(
            5,
            &[textured_point],
            &[width as u32 as u64, height as u32 as u64],
            texture_page,
            clut,
            options,
            texture_window,
        );
        if let Some(key) = cache_key
            && let Some(stats) = self.try_replay_recovery_raster(key)
        {
            return stats[0];
        }
        let sampling_policy = TextureSamplingPolicy::from_draw_options(options);
        let source_snapshot = self.textured_draw_requires_snapshot(
            dest_bounds,
            texture_page,
            clut,
            sampling_policy.allow_texture_descriptor_alias,
        );
        let source_raw = source_snapshot.as_deref();
        let sampler = PreparedTextureSampler::new(
            source_raw.unwrap_or(&self.raw_pixels),
            texture_page,
            clut,
            sampling_policy,
            self.recovery_raster_context
                .is_some()
                .then_some(&self.recovery_palette_history),
        );
        for row in 0..height {
            for col in 0..width {
                stats.sampled_pixels = stats.sampled_pixels.saturating_add(1);
                let u = if options.texture_flip_x {
                    uv.u.wrapping_sub(col as u8)
                } else {
                    uv.u.wrapping_add(col as u8)
                };
                let v = if options.texture_flip_y {
                    uv.v.wrapping_sub(row as u8)
                } else {
                    uv.v.wrapping_add(row as u8)
                };
                let (u, v) = texture_window.apply(u, v);
                let sample = sampler.sample(source_raw.unwrap_or(&self.raw_pixels), u, v);
                stats.record_sample(sample);
                if sample.color != 0 {
                    let color = options.apply_color(sample.color);
                    stats.record_color(color);
                    stats.drawn_pixels = stats.drawn_pixels.saturating_add(1);
                    if self.set_textured_pixel(dest.x + col, dest.y + row, color, options) {
                        stats.written_pixels = stats.written_pixels.saturating_add(1);
                    } else {
                        stats.clipped_pixels = stats.clipped_pixels.saturating_add(1);
                    }
                } else {
                    stats.transparent_pixels = stats.transparent_pixels.saturating_add(1);
                }
            }
        }
        self.finish_recovery_raster(
            cache_key,
            [stats, TexturedDrawStats::default()],
            source_snapshot.is_none(),
        );
        stats
    }

    fn textured_triangle_raster_bounds(
        &self,
        a: TexturedPoint,
        b: TexturedPoint,
        c: TexturedPoint,
    ) -> Option<TexturedTriangleRasterBounds> {
        if triangle_exceeds_gpu_size_limit(a.point, b.point, c.point) {
            return None;
        }
        let (clip_left, clip_top, clip_right, clip_bottom) = self.clip_bounds();
        if clip_left >= clip_right || clip_top >= clip_bottom {
            return None;
        }

        let min_x = a
            .point
            .x
            .min(b.point.x)
            .min(c.point.x)
            .clamp(clip_left, clip_right - 1);
        let max_x = a
            .point
            .x
            .max(b.point.x)
            .max(c.point.x)
            .clamp(clip_left, clip_right - 1);
        let min_y = a
            .point
            .y
            .min(b.point.y)
            .min(c.point.y)
            .clamp(clip_top, clip_bottom - 1);
        let max_y = a
            .point
            .y
            .max(b.point.y)
            .max(c.point.y)
            .clamp(clip_top, clip_bottom - 1);
        let area = edge(a.point, b.point, c.point);
        (area != 0).then_some(TexturedTriangleRasterBounds {
            min_x,
            min_y,
            max_x,
            max_y,
            area,
        })
    }

    fn draw_textured_triangle_with_resources(
        &mut self,
        a: TexturedPoint,
        b: TexturedPoint,
        c: TexturedPoint,
        bounds: TexturedTriangleRasterBounds,
        resources: &PreparedTextureDrawResources,
        options: TextureDrawOptions,
        texture_window: TextureWindow,
    ) -> TexturedDrawStats {
        let mut stats = TexturedDrawStats::default();
        let denom = bounds.area.unsigned_abs() as i64;
        for y in bounds.min_y..=bounds.max_y {
            for x in bounds.min_x..=bounds.max_x {
                let point = Point { x, y };
                let Some((w0, w1, w2)) =
                    psx_triangle_weights(a.point, b.point, c.point, point, bounds.area)
                else {
                    continue;
                };

                stats.sampled_pixels = stats.sampled_pixels.saturating_add(1);
                let u = ((a.u as i64 * w0 + b.u as i64 * w1 + c.u as i64 * w2) / denom) as u8;
                let v = ((a.v as i64 * w0 + b.v as i64 * w1 + c.v as i64 * w2) / denom) as u8;
                let (u, v) = texture_window.apply(u, v);
                let sample = resources.sample(self, u, v);
                stats.record_sample(sample);
                if sample.color != 0 {
                    let color = options.apply_color(sample.color);
                    stats.record_color(color);
                    stats.drawn_pixels = stats.drawn_pixels.saturating_add(1);
                    if self.set_textured_pixel(x, y, color, options) {
                        stats.written_pixels = stats.written_pixels.saturating_add(1);
                    } else {
                        stats.clipped_pixels = stats.clipped_pixels.saturating_add(1);
                    }
                } else {
                    stats.transparent_pixels = stats.transparent_pixels.saturating_add(1);
                }
            }
        }
        stats
    }

    fn draw_shaded_textured_triangle_with_resources(
        &mut self,
        a: TexturedPoint,
        b: TexturedPoint,
        c: TexturedPoint,
        colors: [u32; 3],
        bounds: TexturedTriangleRasterBounds,
        resources: &PreparedTextureDrawResources,
        options: TextureDrawOptions,
        texture_window: TextureWindow,
    ) -> TexturedDrawStats {
        let mut stats = TexturedDrawStats::default();
        let denom = bounds.area.unsigned_abs() as i64;
        for y in bounds.min_y..=bounds.max_y {
            for x in bounds.min_x..=bounds.max_x {
                let point = Point { x, y };
                let Some((w0, w1, w2)) =
                    psx_triangle_weights(a.point, b.point, c.point, point, bounds.area)
                else {
                    continue;
                };

                stats.sampled_pixels = stats.sampled_pixels.saturating_add(1);
                let u = ((a.u as i64 * w0 + b.u as i64 * w1 + c.u as i64 * w2) / denom) as u8;
                let v = ((a.v as i64 * w0 + b.v as i64 * w1 + c.v as i64 * w2) / denom) as u8;
                let (u, v) = texture_window.apply(u, v);
                let sample = resources.sample(self, u, v);
                stats.record_sample(sample);
                if sample.color != 0 {
                    let primitive_color = interpolate_psx_rgb(colors, w0, w1, w2, denom);
                    let color = options.apply_color_with_primitive(sample.color, primitive_color);
                    stats.record_color(color);
                    stats.drawn_pixels = stats.drawn_pixels.saturating_add(1);
                    if self.set_textured_pixel(x, y, color, options) {
                        stats.written_pixels = stats.written_pixels.saturating_add(1);
                    } else {
                        stats.clipped_pixels = stats.clipped_pixels.saturating_add(1);
                    }
                } else {
                    stats.transparent_pixels = stats.transparent_pixels.saturating_add(1);
                }
            }
        }
        stats
    }

    fn in_clip(&self, x: i32, y: i32) -> bool {
        self.clip.is_none_or(|clip| {
            x >= clip.left && x <= clip.right && y >= clip.top && y <= clip.bottom
        })
    }

    fn clip_bounds(&self) -> (i32, i32, i32, i32) {
        self.clip
            .map_or((0, 0, VRAM_WIDTH as i32, VRAM_HEIGHT as i32), |clip| {
                (
                    clip.left.clamp(0, VRAM_WIDTH as i32),
                    clip.top.clamp(0, VRAM_HEIGHT as i32),
                    clip.right.saturating_add(1).clamp(0, VRAM_WIDTH as i32),
                    clip.bottom.saturating_add(1).clamp(0, VRAM_HEIGHT as i32),
                )
            })
    }

    fn sample_texture_sample_from(
        &self,
        raw_pixels: &[u16],
        texture_page: u16,
        clut: u16,
        u: u8,
        v: u8,
        sampling_policy: TextureSamplingPolicy,
    ) -> TextureSample {
        let (texture_page, clut) = sampling_policy.resolve_descriptor(texture_page, clut);
        let (page_x, page_y) = texture_page_origin_for_sample(texture_page, clut, u, v);
        self.sample_texture_sample_from_origin(
            raw_pixels,
            texture_page,
            clut,
            u,
            v,
            page_x,
            page_y,
            texture_page_raw_width(texture_page),
            sampling_policy.allow_palette_fallback,
        )
    }

    fn sample_texture_sample_from_origin(
        &self,
        raw_pixels: &[u16],
        texture_page: u16,
        clut: u16,
        u: u8,
        v: u8,
        origin_x: i32,
        origin_y: i32,
        raw_width: usize,
        allow_palette_fallback: bool,
    ) -> TextureSample {
        let mode = texture_page_color_mode(texture_page);
        let raw_width = raw_width.max(1) as i32;
        let u = u as i32;
        let v = v as i32;
        let raw_x = |offset: i32| origin_x + offset.rem_euclid(raw_width);

        match mode {
            0 => {
                let packed = raw_pixel_from(raw_pixels, raw_x(u / 4), origin_y + v);
                let index = ((packed >> ((u & 3) * 4)) & 0x0f) as i32;
                indexed_palette_sample_from(
                    raw_pixels,
                    texture_page,
                    clut,
                    index,
                    16,
                    allow_palette_fallback,
                )
            }
            1 => {
                let packed = raw_pixel_from(raw_pixels, raw_x(u / 2), origin_y + v);
                let use_high_byte = (u & 1 == 0) == native_swap_8bpp_texture_bytes();
                let index = if use_high_byte {
                    packed >> 8
                } else {
                    packed & 0x00ff
                } as i32;
                indexed_palette_sample_from(
                    raw_pixels,
                    texture_page,
                    clut,
                    index,
                    256,
                    allow_palette_fallback,
                )
            }
            _ => {
                let color = raw_pixel_from(raw_pixels, raw_x(u), origin_y + v);
                TextureSample {
                    color,
                    texture_nonzero: color != 0,
                    zero_texel: color == 0,
                    clut_nonzero: color != 0,
                    ..TextureSample::default()
                }
            }
        }
    }

    fn textured_draw_requires_snapshot(
        &self,
        dest_bounds: (i32, i32, i32, i32),
        texture_page: u16,
        clut: u16,
        allow_texture_descriptor_alias: bool,
    ) -> Option<Vec<u16>> {
        let (texture_page, clut) = if allow_texture_descriptor_alias {
            br2_texture_descriptor_alias(texture_page, clut)
        } else {
            (texture_page, clut)
        };
        let texture_bounds = texture_page_raw_bounds_for_clut(texture_page, clut);
        let palette_bounds = texture_palette_raw_bounds(texture_page, clut);
        if bounds_overlap(dest_bounds, texture_bounds)
            || palette_bounds.is_some_and(|bounds| bounds_overlap(dest_bounds, bounds))
        {
            return Some(self.raw_pixels.clone());
        }
        None
    }

    pub fn decoded_texture_png(&self, texture_page: u16, clut: u16) -> Vec<u8> {
        self.decoded_texture_png_with_sampling_policy(
            texture_page,
            clut,
            TextureSamplingPolicy::diagnostics_default(),
        )
    }

    fn decoded_texture_png_with_sampling_policy(
        &self,
        texture_page: u16,
        clut: u16,
        sampling_policy: TextureSamplingPolicy,
    ) -> Vec<u8> {
        let (width, height) = texture_page_dimensions(texture_page);
        let mut pixels = Vec::with_capacity(width.saturating_mul(height));
        for y in 0..height {
            for x in 0..width {
                let color = self
                    .sample_texture_sample_from(
                        &self.raw_pixels,
                        texture_page,
                        clut,
                        x as u8,
                        y as u8,
                        sampling_policy,
                    )
                    .color;
                pixels.push(rgb555_to_rgb888(color));
            }
        }
        png_from_rgb888_pixels(width, height, &pixels)
    }

    fn decoded_texture_png_from_origin(
        &self,
        texture_page: u16,
        clut: u16,
        origin_x: i32,
        origin_y: i32,
        raw_width: usize,
        allow_palette_fallback: bool,
    ) -> Vec<u8> {
        let (width, height) = texture_page_dimensions(texture_page);
        let mut pixels = Vec::with_capacity(width.saturating_mul(height));
        for y in 0..height {
            for x in 0..width {
                let color = self
                    .sample_texture_sample_from_origin(
                        &self.raw_pixels,
                        texture_page,
                        clut,
                        x as u8,
                        y as u8,
                        origin_x,
                        origin_y,
                        raw_width,
                        allow_palette_fallback,
                    )
                    .color;
                pixels.push(rgb555_to_rgb888(color));
            }
        }
        png_from_rgb888_pixels(width, height, &pixels)
    }

    pub fn raw_texture_page_png(&self, texture_page: u16) -> Vec<u8> {
        let (page_x, page_y) = texture_page_origin(texture_page);
        let width = texture_page_raw_width(texture_page);
        let height = 256usize;
        let mut pixels = Vec::with_capacity(width.saturating_mul(height));
        for y in 0..height {
            for x in 0..width {
                let color = raw_pixel_from(&self.raw_pixels, page_x + x as i32, page_y + y as i32);
                pixels.push(rgb555_to_rgb888(color));
            }
        }
        png_from_rgb888_pixels(width, height, &pixels)
    }

    fn raw_texture_page_png_from_origin(
        &self,
        origin_x: i32,
        origin_y: i32,
        raw_width: usize,
    ) -> Vec<u8> {
        let width = raw_width.max(1);
        let height = 256usize;
        let mut pixels = Vec::with_capacity(width.saturating_mul(height));
        for y in 0..height {
            for x in 0..width {
                let color =
                    raw_pixel_from(&self.raw_pixels, origin_x + x as i32, origin_y + y as i32);
                pixels.push(rgb555_to_rgb888(color));
            }
        }
        png_from_rgb888_pixels(width, height, &pixels)
    }

    pub fn texture_candidate_images(
        &self,
        texture_page: u16,
        clut: u16,
    ) -> Vec<TextureCandidateImage> {
        self.texture_candidate_images_with_sampling_policy(
            texture_page,
            clut,
            TextureSamplingPolicy::diagnostics_default(),
        )
    }

    fn texture_candidate_images_with_sampling_policy(
        &self,
        texture_page: u16,
        clut: u16,
        sampling_policy: TextureSamplingPolicy,
    ) -> Vec<TextureCandidateImage> {
        let (texture_page, clut) = sampling_policy.resolve_descriptor(texture_page, clut);
        let mut raw_widths = vec![texture_page_raw_width(texture_page)];
        if texture_page_color_mode(texture_page) == 1
            && texture_page_uses_zn_extended_origin(texture_page)
        {
            push_unique_usize(&mut raw_widths, 64);
            push_unique_usize(&mut raw_widths, 128);
        }

        let mut images = Vec::new();
        for candidate in texture_origin_candidates_for_clut(texture_page, clut) {
            for raw_width in raw_widths.iter().copied() {
                let label = format!("{}-raw{}", candidate.label, raw_width);
                images.push(TextureCandidateImage {
                    label,
                    x: candidate.x,
                    y: candidate.y,
                    raw_width,
                    raw_height: 256,
                    decoded_png: self.decoded_texture_png_from_origin(
                        texture_page,
                        clut,
                        candidate.x,
                        candidate.y,
                        raw_width,
                        sampling_policy.allow_palette_fallback,
                    ),
                    raw_png: self.raw_texture_page_png_from_origin(
                        candidate.x,
                        candidate.y,
                        raw_width,
                    ),
                });
            }
        }
        images
    }

    pub fn texture_palette_png(&self, texture_page: u16, clut: u16) -> Vec<u8> {
        let palette_entries = texture_palette_entries(texture_page);
        if palette_entries == 0 {
            return png_from_rgb888_pixels(1, 1, &[0]);
        }

        let clut_x = ((clut & 0x3f) as i32) * 16;
        let clut_y = clut_y(clut) as i32;
        self.palette_region_png(clut_x, clut_y, palette_entries, false)
    }

    pub fn resolved_texture_palette_png(&self, texture_page: u16, clut: u16) -> Vec<u8> {
        let (texture_page, clut) =
            TextureSamplingPolicy::diagnostics_default().resolve_descriptor(texture_page, clut);
        let palette_entries = texture_palette_entries(texture_page);
        if palette_entries == 0 {
            return png_from_rgb888_pixels(1, 1, &[0]);
        }

        let requested_nonzero_entries =
            palette_row_nonzero_entries(&self.raw_pixels, clut, palette_entries);
        let requested_stats = palette_row_stats(&self.raw_pixels, clut, palette_entries);
        if palette_entries == 16
            && let Some(candidate) =
                fallback_br2_4bpp_palette_candidate(&self.raw_pixels, texture_page, clut)
            && should_use_br2_4bpp_palette_sample(texture_page, clut, true, requested_stats)
        {
            return self.palette_region_png(candidate.x, candidate.y, palette_entries, false);
        }
        if palette_entries == 16
            && let Some(candidate) =
                fallback_zn_4bpp_palette_candidate(&self.raw_pixels, texture_page, clut)
            && should_use_zn_4bpp_palette_sample(
                texture_page,
                clut,
                true,
                requested_stats.nonzero_entries,
                requested_stats.unique_entries,
                candidate.nonzero_entries,
                candidate.unique_entries,
            )
        {
            return self.palette_region_png(candidate.x, candidate.y, palette_entries, false);
        }

        if requested_nonzero_entries == 0
            && palette_entries == 256
            && let Some(candidate) =
                fallback_tiled_256_palette_candidate(&self.raw_pixels, texture_page, clut)
        {
            return self.palette_region_png(candidate.x, candidate.y, palette_entries, true);
        }

        self.texture_palette_png(texture_page, clut)
    }

    pub fn texture_diagnostics_json(&self, texture_page: u16, clut: u16) -> String {
        let sampling_policy = TextureSamplingPolicy::diagnostics_default();
        let original_texture_page = texture_page;
        let original_clut = clut;
        let (texture_page, clut) = sampling_policy.resolve_descriptor(texture_page, clut);
        let descriptor_alias_applied =
            texture_page != original_texture_page || clut != original_clut;
        let mode = texture_page_color_mode(texture_page);
        let (page_x, page_y) = texture_page_origin_for_clut(texture_page, clut);
        let (decoded_width, decoded_height) = texture_page_dimensions(texture_page);
        let raw_width = texture_page_raw_width(texture_page);
        let entries = texture_palette_entries(texture_page);
        let clut_x = ((clut & 0x3f) as i32) * 16;
        let clut_y = clut_y(clut) as i32;

        let texture_candidates = texture_origin_candidates_for_clut(texture_page, clut);

        let texture_candidates_json = texture_candidates
            .iter()
            .map(|candidate| self.texture_origin_diagnostics_json(texture_page, *candidate))
            .collect::<Vec<_>>()
            .join(",");

        let mut palette_candidates = Vec::new();
        push_unique_origin(&mut palette_candidates, "requested", clut_x, clut_y);
        push_unique_origin(
            &mut palette_candidates,
            "requested_y16_base",
            clut_x,
            clut_y & !0x0f,
        );
        push_unique_origin(
            &mut palette_candidates,
            "requested_y32_base",
            clut_x,
            clut_y & !0x1f,
        );
        push_unique_origin(
            &mut palette_candidates,
            "requested_prev_row",
            clut_x,
            clut_y.saturating_sub(16),
        );
        push_unique_origin(
            &mut palette_candidates,
            "requested_next_row",
            clut_x,
            clut_y.saturating_add(16),
        );
        push_unique_origin(&mut palette_candidates, "left_alias", clut_x - 16, clut_y);
        push_unique_origin(&mut palette_candidates, "right_alias", clut_x + 16, clut_y);
        if entries == 16 && br2_low_bank_gameplay_character_texture_descriptor(texture_page, clut) {
            push_unique_origin(
                &mut palette_candidates,
                "br2_shared_fighter_bank",
                32,
                clut_y,
            );
        }

        if entries == 256 {
            for x in [384, 400, 416, 432, 448, 464, 480, 496] {
                push_unique_origin(
                    &mut palette_candidates,
                    "tiled_256_y32_base",
                    x,
                    clut_y & !0x1f,
                );
                push_unique_origin(
                    &mut palette_candidates,
                    "tiled_256_y16_base",
                    x,
                    clut_y & !0x0f,
                );
            }
            if let Some(candidate) =
                fallback_tiled_256_palette_candidate(&self.raw_pixels, texture_page, clut)
            {
                push_unique_origin(
                    &mut palette_candidates,
                    "resolved_tiled_256",
                    candidate.x,
                    candidate.y,
                );
            }
        }

        let palette_candidates_json = palette_candidates
            .iter()
            .map(|candidate| self.palette_origin_diagnostics_json(*candidate, entries))
            .collect::<Vec<_>>()
            .join(",");

        format!(
            "{{\"texture_page\":{},\"texture_page_hex\":\"0x{:04x}\",\"clut\":{},\"clut_hex\":\"0x{:04x}\",\"descriptor_alias_enabled\":{},\"descriptor_alias_applied\":{},\"sampled_texture_page\":{},\"sampled_texture_page_hex\":\"0x{:04x}\",\"sampled_clut\":{},\"sampled_clut_hex\":\"0x{:04x}\",\"mode\":{},\"decoded_width\":{},\"decoded_height\":{},\"raw_width\":{},\"raw_height\":256,\"resolved_origin\":{{\"x\":{},\"y\":{}}},\"clut_origin\":{{\"x\":{},\"y\":{}}},\"palette_entries\":{},\"texture_candidates\":[{}],\"palette_candidates\":[{}]}}",
            original_texture_page,
            original_texture_page,
            original_clut,
            original_clut,
            sampling_policy.allow_texture_descriptor_alias,
            descriptor_alias_applied,
            texture_page,
            texture_page,
            clut,
            clut,
            mode,
            decoded_width,
            decoded_height,
            raw_width,
            page_x,
            page_y,
            clut_x,
            clut_y,
            entries,
            texture_candidates_json,
            palette_candidates_json
        )
    }

    fn texture_origin_diagnostics_json(
        &self,
        texture_page: u16,
        candidate: OriginCandidate,
    ) -> String {
        let mode = texture_page_color_mode(texture_page);
        let raw_width = texture_page_raw_width(texture_page);
        let raw_height = 256;
        let mut nonzero_words = 0usize;
        let mut zero_words = 0usize;
        let mut outside_words = 0usize;
        let mut first_nonzero = None;
        let mut last_nonzero = None;
        let mut checksum = 2_166_136_261u32;
        let mut index_hist = [0usize; 256];
        let mut index_nonzero_samples = 0usize;
        let mut low_byte_nonzero_samples = 0usize;
        let mut high_byte_nonzero_samples = 0usize;

        for y in 0..raw_height {
            for x in 0..raw_width {
                let raw_x = candidate.x + x as i32;
                let raw_y = candidate.y + y;
                if raw_x < 0
                    || raw_y < 0
                    || raw_x >= VRAM_WIDTH as i32
                    || raw_y >= VRAM_HEIGHT as i32
                {
                    outside_words = outside_words.saturating_add(1);
                    continue;
                }

                let word = self.raw_pixel(raw_x, raw_y);
                checksum ^= u32::from(word);
                checksum = checksum.wrapping_mul(16_777_619);
                if word == 0 {
                    zero_words = zero_words.saturating_add(1);
                } else {
                    nonzero_words = nonzero_words.saturating_add(1);
                    first_nonzero.get_or_insert((raw_x, raw_y, word));
                    last_nonzero = Some((raw_x, raw_y, word));
                }

                match mode {
                    0 => {
                        for shift in [0, 4, 8, 12] {
                            let index = ((word >> shift) & 0x0f) as usize;
                            index_hist[index] = index_hist[index].saturating_add(1);
                            if index != 0 {
                                index_nonzero_samples = index_nonzero_samples.saturating_add(1);
                            }
                        }
                    }
                    1 => {
                        let low = (word & 0x00ff) as usize;
                        let high = (word >> 8) as usize;
                        index_hist[low] = index_hist[low].saturating_add(1);
                        index_hist[high] = index_hist[high].saturating_add(1);
                        if low != 0 {
                            index_nonzero_samples = index_nonzero_samples.saturating_add(1);
                            low_byte_nonzero_samples = low_byte_nonzero_samples.saturating_add(1);
                        }
                        if high != 0 {
                            index_nonzero_samples = index_nonzero_samples.saturating_add(1);
                            high_byte_nonzero_samples = high_byte_nonzero_samples.saturating_add(1);
                        }
                    }
                    _ => {}
                }
            }
        }

        let index_hist_json = match mode {
            0 => format_index_hist_json(&index_hist[..16], false),
            1 => format_index_hist_json(&index_hist, true),
            _ => "[]".to_string(),
        };
        let unique_index_count = index_hist.iter().filter(|count| **count != 0).count();

        format!(
            "{{\"label\":\"{}\",\"x\":{},\"y\":{},\"raw_width\":{},\"raw_height\":{},\"nonzero_words\":{},\"zero_words\":{},\"outside_words\":{},\"first_nonzero\":{},\"last_nonzero\":{},\"checksum\":{},\"checksum_hex\":\"0x{:08x}\",\"unique_index_count\":{},\"index_nonzero_samples\":{},\"low_byte_nonzero_samples\":{},\"high_byte_nonzero_samples\":{},\"index_hist\":{}}}",
            candidate.label,
            candidate.x,
            candidate.y,
            raw_width,
            raw_height,
            nonzero_words,
            zero_words,
            outside_words,
            optional_raw_sample_json(first_nonzero),
            optional_raw_sample_json(last_nonzero),
            checksum,
            checksum,
            unique_index_count,
            index_nonzero_samples,
            low_byte_nonzero_samples,
            high_byte_nonzero_samples,
            index_hist_json
        )
    }

    fn palette_origin_diagnostics_json(
        &self,
        candidate: OriginCandidate,
        entries: usize,
    ) -> String {
        let scan_entries = entries.clamp(0, 256);
        let mut nonzero_entries = 0usize;
        let mut zero_entries = 0usize;
        let mut outside_entries = 0usize;
        let mut unique_colors = Vec::<u16>::new();
        let mut checksum = 2_166_136_261u32;
        let tiled = scan_entries == 256 && candidate.label.contains("tiled");

        for index in 0..scan_entries {
            let (x, y) = if tiled {
                (
                    candidate.x + (index & 0x0f) as i32,
                    candidate.y + (index / 16) as i32,
                )
            } else {
                (candidate.x + index as i32, candidate.y)
            };

            if x < 0 || y < 0 || x >= VRAM_WIDTH as i32 || y >= VRAM_HEIGHT as i32 {
                outside_entries = outside_entries.saturating_add(1);
                continue;
            }

            let color = self.raw_pixel(x, y);
            checksum ^= u32::from(color);
            checksum = checksum.wrapping_mul(16_777_619);
            if color == 0 {
                zero_entries = zero_entries.saturating_add(1);
            } else {
                nonzero_entries = nonzero_entries.saturating_add(1);
            }
            if unique_colors.len() < 32 && !unique_colors.contains(&color) {
                unique_colors.push(color);
            }
        }

        let unique_json = unique_colors
            .iter()
            .map(|color| format!("\"0x{color:04x}\""))
            .collect::<Vec<_>>()
            .join(",");

        format!(
            "{{\"label\":\"{}\",\"x\":{},\"y\":{},\"entries\":{},\"tiled\":{},\"nonzero_entries\":{},\"zero_entries\":{},\"outside_entries\":{},\"unique_color_count_capped\":{},\"unique_colors\":[{}],\"checksum\":{},\"checksum_hex\":\"0x{:08x}\"}}",
            candidate.label,
            candidate.x,
            candidate.y,
            scan_entries,
            tiled,
            nonzero_entries,
            zero_entries,
            outside_entries,
            unique_colors.len(),
            unique_json,
            checksum,
            checksum
        )
    }

    fn palette_region_png(
        &self,
        origin_x: i32,
        origin_y: i32,
        palette_entries: usize,
        tiled_256: bool,
    ) -> Vec<u8> {
        let columns = palette_entries.min(16);
        let rows = palette_entries.div_ceil(columns);
        let cell_size = 8usize;
        let width = columns * cell_size;
        let height = rows * cell_size;
        let mut pixels = vec![0; width.saturating_mul(height)];
        for index in 0..palette_entries {
            let color = if tiled_256 && palette_entries == 256 {
                let col = (index & 0x0f) as i32;
                let row = (index / 16) as i32;
                rgb555_to_rgb888(self.raw_pixel(origin_x + col, origin_y + row))
            } else {
                rgb555_to_rgb888(self.raw_pixel(origin_x + index as i32, origin_y))
            };
            let cell_x = (index % columns) * cell_size;
            let cell_y = (index / columns) * cell_size;
            for y in 0..cell_size {
                let row_start = (cell_y + y) * width;
                for x in 0..cell_size {
                    pixels[row_start + cell_x + x] = color;
                }
            }
        }
        png_from_rgb888_pixels(width, height, &pixels)
    }

    fn set_textured_pixel(
        &mut self,
        x: i32,
        y: i32,
        color: u16,
        options: TextureDrawOptions,
    ) -> bool {
        if !self.in_clip(x, y) {
            return false;
        }
        if x < 0 || y < 0 || x >= VRAM_WIDTH as i32 || y >= VRAM_HEIGHT as i32 {
            return false;
        }

        let index = y as usize * VRAM_WIDTH + x as usize;
        if options.check_mask_bit && self.raw_pixels[index] & 0x8000 != 0 {
            return false;
        }
        let semi_transparent = options.semi_transparent && color & 0x8000 != 0;
        let color = if semi_transparent {
            blend_rgb555(
                color,
                self.raw_pixels[index],
                options.semi_transparency_mode,
            )
        } else {
            color
        };
        self.write_raw_pixel_index(
            index,
            color,
            PixelWriteOptions {
                set_mask_bit: options.set_mask_bit,
                check_mask_bit: options.check_mask_bit,
                semi_transparent: false,
                semi_transparency_mode: 0,
            },
        )
    }

    pub fn png_base64(
        &self,
        start_x: usize,
        start_y: usize,
        width: usize,
        height: usize,
    ) -> String {
        base64_encode(&self.png(start_x, start_y, width, height))
    }

    pub fn png(&self, start_x: usize, start_y: usize, width: usize, height: usize) -> Vec<u8> {
        png_rgb(
            width.max(1),
            height.max(1),
            &self.rgb_rows(start_x, start_y, width.max(1), height.max(1)),
        )
    }

    pub fn psx_display_png(
        &self,
        start_x: usize,
        start_y: usize,
        width: usize,
        height: usize,
    ) -> Vec<u8> {
        self.psx_display_png_with_depth(start_x, start_y, width, height, false)
    }

    pub fn psx_display_png_with_depth(
        &self,
        start_x: usize,
        start_y: usize,
        width: usize,
        height: usize,
        rgb24: bool,
    ) -> Vec<u8> {
        png_rgb(
            width.max(1),
            height.max(1),
            &self.psx_display_rgb_rows_with_depth(
                start_x,
                start_y,
                width.max(1),
                height.max(1),
                rgb24,
            ),
        )
    }

    pub fn psx_display_png_base64(
        &self,
        start_x: usize,
        start_y: usize,
        width: usize,
        height: usize,
    ) -> String {
        base64_encode(&self.psx_display_png(start_x, start_y, width, height))
    }

    pub fn psx_display_png_base64_with_depth(
        &self,
        start_x: usize,
        start_y: usize,
        width: usize,
        height: usize,
        rgb24: bool,
    ) -> String {
        base64_encode(&self.psx_display_png_with_depth(start_x, start_y, width, height, rgb24))
    }

    pub fn rgb_window(
        &self,
        start_x: usize,
        start_y: usize,
        width: usize,
        height: usize,
    ) -> Vec<u32> {
        let width = width.max(1);
        let height = height.max(1);
        let mut pixels = Vec::with_capacity(width * height);
        for y in 0..height {
            let source_y = start_y + y;
            for x in 0..width {
                let source_x = start_x + x;
                let rgb = if source_x < VRAM_WIDTH && source_y < VRAM_HEIGHT {
                    self.pixels[source_y * VRAM_WIDTH + source_x]
                } else {
                    0
                };
                pixels.push(rgb);
            }
        }
        pixels
    }

    pub fn psx_display_rgb_window(
        &self,
        start_x: usize,
        start_y: usize,
        width: usize,
        height: usize,
    ) -> Vec<u32> {
        self.psx_display_rgb_window_with_depth(start_x, start_y, width, height, false)
    }

    pub fn psx_display_rgb_window_with_depth(
        &self,
        start_x: usize,
        start_y: usize,
        width: usize,
        height: usize,
        rgb24: bool,
    ) -> Vec<u32> {
        let width = width.max(1);
        let height = height.max(1);
        let mut pixels = Vec::with_capacity(width * height);
        for y in 0..height {
            for x in 0..width {
                pixels.push(self.psx_display_pixel(start_x, start_y, x, y, rgb24));
            }
        }
        pixels
    }

    pub fn psx_display_pixel(
        &self,
        start_x: usize,
        start_y: usize,
        x: usize,
        y: usize,
        rgb24: bool,
    ) -> u32 {
        let source_y = (start_y + y) % PSX_VRAM_HEIGHT;
        if !rgb24 {
            let source_x = (start_x + x) % VRAM_WIDTH;
            return self.pixels[source_y * VRAM_WIDTH + source_x];
        }

        let byte_offset = x.saturating_mul(3);
        let r = self.psx_display_byte(start_x, source_y, byte_offset);
        let g = self.psx_display_byte(start_x, source_y, byte_offset + 1);
        let b = self.psx_display_byte(start_x, source_y, byte_offset + 2);
        (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b)
    }

    fn psx_display_byte(&self, start_x: usize, source_y: usize, byte_offset: usize) -> u8 {
        let word_x = (start_x + byte_offset / 2) % VRAM_WIDTH;
        let raw = self.raw_pixels[source_y * VRAM_WIDTH + word_x];
        if byte_offset & 1 == 0 {
            (raw & 0x00ff) as u8
        } else {
            (raw >> 8) as u8
        }
    }

    pub fn display_stats(
        &self,
        start_x: usize,
        start_y: usize,
        width: usize,
        height: usize,
    ) -> FrameBufferStats {
        let pixel_count = (width as u64).saturating_mul(height as u64);
        let mut nonzero_pixels = 0_u64;
        let mut bright_pixels = 0_u64;
        let mut luma_sum = 0_u64;
        let mut max_luma = 0_u8;
        let mut detail_edges = 0_u64;
        let mut checksum = 0x811c_9dc5_u32;
        let mut previous_row_luma = vec![0_u8; width];
        for y in 0..height {
            let source_y = start_y + y;
            let mut previous_luma = 0_u8;
            for x in 0..width {
                let source_x = start_x + x;
                let rgb = if source_x < VRAM_WIDTH && source_y < VRAM_HEIGHT {
                    self.pixels[source_y * VRAM_WIDTH + source_x]
                } else {
                    0
                };
                if rgb != 0 {
                    nonzero_pixels += 1;
                }
                let luma = rgb_luma(rgb);
                if luma >= 32 {
                    bright_pixels += 1;
                }
                luma_sum = luma_sum.saturating_add(luma as u64);
                max_luma = max_luma.max(luma);
                if x > 0 && luma.abs_diff(previous_luma) >= 16 {
                    detail_edges += 1;
                }
                if y > 0
                    && previous_row_luma
                        .get(x)
                        .is_some_and(|previous| luma.abs_diff(*previous) >= 16)
                {
                    detail_edges += 1;
                }
                previous_luma = luma;
                if let Some(previous_row_luma) = previous_row_luma.get_mut(x) {
                    *previous_row_luma = luma;
                }
                checksum ^= rgb;
                checksum = checksum.wrapping_mul(16_777_619);
            }
        }
        FrameBufferStats {
            pixel_count,
            nonzero_pixels,
            bright_pixels,
            luma_sum,
            max_luma,
            detail_edges,
            checksum,
        }
    }

    pub fn psx_display_stats(
        &self,
        start_x: usize,
        start_y: usize,
        width: usize,
        height: usize,
    ) -> FrameBufferStats {
        self.psx_display_stats_with_depth(start_x, start_y, width, height, false)
    }

    pub fn psx_display_stats_with_depth(
        &self,
        start_x: usize,
        start_y: usize,
        width: usize,
        height: usize,
        rgb24: bool,
    ) -> FrameBufferStats {
        let width = width.max(1);
        let height = height.max(1);
        let pixel_count = (width as u64).saturating_mul(height as u64);
        let mut nonzero_pixels = 0_u64;
        let mut bright_pixels = 0_u64;
        let mut luma_sum = 0_u64;
        let mut max_luma = 0_u8;
        let mut detail_edges = 0_u64;
        let mut checksum = 0x811c_9dc5_u32;
        let mut previous_row_luma = vec![0_u8; width];
        for y in 0..height {
            let mut previous_luma = 0_u8;
            for x in 0..width {
                let rgb = self.psx_display_pixel(start_x, start_y, x, y, rgb24);
                if rgb != 0 {
                    nonzero_pixels += 1;
                }
                let luma = rgb_luma(rgb);
                if luma >= 32 {
                    bright_pixels += 1;
                }
                luma_sum = luma_sum.saturating_add(luma as u64);
                max_luma = max_luma.max(luma);
                if x > 0 && luma.abs_diff(previous_luma) >= 16 {
                    detail_edges += 1;
                }
                if y > 0
                    && previous_row_luma
                        .get(x)
                        .is_some_and(|previous| luma.abs_diff(*previous) >= 16)
                {
                    detail_edges += 1;
                }
                previous_luma = luma;
                if let Some(previous_row_luma) = previous_row_luma.get_mut(x) {
                    *previous_row_luma = luma;
                }
                checksum ^= rgb;
                checksum = checksum.wrapping_mul(16_777_619);
            }
        }
        FrameBufferStats {
            pixel_count,
            nonzero_pixels,
            bright_pixels,
            luma_sum,
            max_luma,
            detail_edges,
            checksum,
        }
    }

    pub fn stats(&self) -> FrameBufferStats {
        self.display_stats(0, 0, VRAM_WIDTH, VRAM_HEIGHT)
    }

    pub fn densest_window(
        &self,
        width: usize,
        height: usize,
        step: usize,
    ) -> Option<FrameBufferWindow> {
        if width == 0 || height == 0 || width > VRAM_WIDTH || height > VRAM_HEIGHT {
            return None;
        }

        let step = step.max(1);
        let integral = self.nonzero_integral_image();
        let max_x = VRAM_WIDTH - width;
        let max_y = VRAM_HEIGHT - height;
        let mut best: Option<(usize, usize, u64)> = None;

        for y in stepped_positions(max_y, step) {
            for x in stepped_positions(max_x, step) {
                let nonzero_pixels = integral_rect(&integral, x, y, width, height);
                if best.is_none_or(|(_, _, best_count)| nonzero_pixels > best_count) {
                    best = Some((x, y, nonzero_pixels));
                }
            }
        }

        let (x, y, nonzero_pixels) = best?;
        (nonzero_pixels > 0).then(|| FrameBufferWindow {
            x,
            y,
            stats: self.display_stats(x, y, width, height),
        })
    }

    pub fn brightest_window(
        &self,
        width: usize,
        height: usize,
        step: usize,
    ) -> Option<FrameBufferWindow> {
        if width == 0 || height == 0 || width > VRAM_WIDTH || height > VRAM_HEIGHT {
            return None;
        }

        let step = step.max(1);
        let (bright_integral, luma_integral) = self.brightness_integral_images();
        let max_x = VRAM_WIDTH - width;
        let max_y = VRAM_HEIGHT - height;
        let mut best: Option<(usize, usize, u64, u64)> = None;

        for y in stepped_positions(max_y, step) {
            for x in stepped_positions(max_x, step) {
                let bright_pixels = integral_rect(&bright_integral, x, y, width, height);
                let luma_sum = integral_rect_u64(&luma_integral, x, y, width, height);
                if best.is_none_or(|(_, _, best_bright, best_luma)| {
                    bright_pixels > best_bright
                        || (bright_pixels == best_bright && luma_sum > best_luma)
                }) {
                    best = Some((x, y, bright_pixels, luma_sum));
                }
            }
        }

        let (x, y, bright_pixels, _) = best?;
        (bright_pixels > 0).then(|| FrameBufferWindow {
            x,
            y,
            stats: self.display_stats(x, y, width, height),
        })
    }

    pub fn nonzero_bounds(&self) -> Option<FrameBufferBounds> {
        let mut left = VRAM_WIDTH;
        let mut top = VRAM_HEIGHT;
        let mut right = 0;
        let mut bottom = 0;

        for y in 0..VRAM_HEIGHT {
            for x in 0..VRAM_WIDTH {
                if self.pixels[y * VRAM_WIDTH + x] == 0 {
                    continue;
                }
                left = left.min(x);
                top = top.min(y);
                right = right.max(x);
                bottom = bottom.max(y);
            }
        }

        (left <= right && top <= bottom).then_some(FrameBufferBounds {
            left,
            top,
            right,
            bottom,
        })
    }

    fn nonzero_integral_image(&self) -> Vec<u32> {
        let stride = VRAM_WIDTH + 1;
        let mut integral = vec![0_u32; stride * (VRAM_HEIGHT + 1)];

        for y in 0..VRAM_HEIGHT {
            let mut row_total = 0_u32;
            for x in 0..VRAM_WIDTH {
                if self.pixels[y * VRAM_WIDTH + x] != 0 {
                    row_total += 1;
                }
                let index = (y + 1) * stride + x + 1;
                integral[index] = integral[y * stride + x + 1] + row_total;
            }
        }

        integral
    }

    fn brightness_integral_images(&self) -> (Vec<u32>, Vec<u64>) {
        let stride = VRAM_WIDTH + 1;
        let mut bright_integral = vec![0_u32; stride * (VRAM_HEIGHT + 1)];
        let mut luma_integral = vec![0_u64; stride * (VRAM_HEIGHT + 1)];

        for y in 0..VRAM_HEIGHT {
            let mut row_bright = 0_u32;
            let mut row_luma = 0_u64;
            for x in 0..VRAM_WIDTH {
                let luma = rgb_luma(self.pixels[y * VRAM_WIDTH + x]);
                if luma >= 32 {
                    row_bright += 1;
                }
                row_luma += luma as u64;
                let index = (y + 1) * stride + x + 1;
                bright_integral[index] = bright_integral[y * stride + x + 1] + row_bright;
                luma_integral[index] = luma_integral[y * stride + x + 1] + row_luma;
            }
        }

        (bright_integral, luma_integral)
    }

    fn rgb_rows(&self, start_x: usize, start_y: usize, width: usize, height: usize) -> Vec<u8> {
        let mut rows = Vec::with_capacity(height * (1 + width * 3));
        for y in 0..height {
            rows.push(0);
            let source_y = start_y + y;
            for x in 0..width {
                let source_x = start_x + x;
                let rgb = if source_x < VRAM_WIDTH && source_y < VRAM_HEIGHT {
                    self.pixels[source_y * VRAM_WIDTH + source_x]
                } else {
                    0
                };
                rows.push(((rgb >> 16) & 0xff) as u8);
                rows.push(((rgb >> 8) & 0xff) as u8);
                rows.push((rgb & 0xff) as u8);
            }
        }
        rows
    }

    fn psx_display_rgb_rows_with_depth(
        &self,
        start_x: usize,
        start_y: usize,
        width: usize,
        height: usize,
        rgb24: bool,
    ) -> Vec<u8> {
        let mut rows = Vec::with_capacity(height * (1 + width * 3));
        for y in 0..height {
            rows.push(0);
            for x in 0..width {
                let rgb = self.psx_display_pixel(start_x, start_y, x, y, rgb24);
                rows.push(((rgb >> 16) & 0xff) as u8);
                rows.push(((rgb >> 8) & 0xff) as u8);
                rows.push((rgb & 0xff) as u8);
            }
        }
        rows
    }
}

fn wrap_vram_x(x: i32) -> i32 {
    x.rem_euclid(VRAM_WIDTH as i32)
}

fn wrap_vram_y(y: i32) -> i32 {
    y.rem_euclid(VRAM_HEIGHT as i32)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameBufferStats {
    pub pixel_count: u64,
    pub nonzero_pixels: u64,
    pub bright_pixels: u64,
    pub luma_sum: u64,
    pub max_luma: u8,
    pub detail_edges: u64,
    pub checksum: u32,
}

impl FrameBufferStats {
    pub fn json(self) -> String {
        format!(
            "{{\"pixel_count\":{},\"nonzero_pixels\":{},\"bright_pixels\":{},\"avg_luma\":{},\"max_luma\":{},\"detail_edges\":{},\"checksum\":{}}}",
            self.pixel_count,
            self.nonzero_pixels,
            self.bright_pixels,
            self.luma_sum.checked_div(self.pixel_count).unwrap_or(0),
            self.max_luma,
            self.detail_edges,
            self.checksum
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameBufferBounds {
    pub left: usize,
    pub top: usize,
    pub right: usize,
    pub bottom: usize,
}

impl FrameBufferBounds {
    pub fn json(self) -> String {
        format!(
            "{{\"left\":{},\"top\":{},\"right\":{},\"bottom\":{}}}",
            self.left, self.top, self.right, self.bottom
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameBufferWindow {
    pub x: usize,
    pub y: usize,
    pub stats: FrameBufferStats,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClipRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl ClipRect {
    pub fn new(left: i32, top: i32, right: i32, bottom: i32) -> Option<Self> {
        if left > right || top > bottom {
            return None;
        }

        Some(Self {
            left,
            top,
            right,
            bottom,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TexturedPoint {
    pub point: Point,
    pub u: u8,
    pub v: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TexturedTriangleRasterBounds {
    min_x: i32,
    min_y: i32,
    max_x: i32,
    max_y: i32,
    area: i64,
}

impl TexturedTriangleRasterBounds {
    const fn dest_bounds(self) -> (i32, i32, i32, i32) {
        (self.min_x, self.min_y, self.max_x, self.max_y)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TexturedDrawStats {
    pub sampled_pixels: u64,
    pub drawn_pixels: u64,
    pub written_pixels: u64,
    pub clipped_pixels: u64,
    pub transparent_pixels: u64,
    pub texture_nonzero_samples: u64,
    pub zero_texel_samples: u64,
    pub clut_nonzero_samples: u64,
    pub clut_blank_samples: u64,
    pub palette_fallback_samples: u64,
    pub nonzero_texel_transparent_samples: u64,
    pub first_color: u16,
    pub last_color: u16,
    pub color_hash: u32,
    pub color_changes: u64,
}

impl TexturedDrawStats {
    pub fn json(self) -> String {
        format!(
            "{{\"sampled_pixels\":{},\"drawn_pixels\":{},\"written_pixels\":{},\"clipped_pixels\":{},\"transparent_pixels\":{},\"texture_nonzero_samples\":{},\"zero_texel_samples\":{},\"clut_nonzero_samples\":{},\"clut_blank_samples\":{},\"palette_fallback_samples\":{},\"nonzero_texel_transparent_samples\":{},\"first_color\":{},\"first_color_hex\":\"0x{:04x}\",\"last_color\":{},\"last_color_hex\":\"0x{:04x}\",\"color_hash\":{},\"color_hash_hex\":\"0x{:08x}\",\"color_changes\":{}}}",
            self.sampled_pixels,
            self.drawn_pixels,
            self.written_pixels,
            self.clipped_pixels,
            self.transparent_pixels,
            self.texture_nonzero_samples,
            self.zero_texel_samples,
            self.clut_nonzero_samples,
            self.clut_blank_samples,
            self.palette_fallback_samples,
            self.nonzero_texel_transparent_samples,
            self.first_color,
            self.first_color,
            self.last_color,
            self.last_color,
            self.color_hash,
            self.color_hash,
            self.color_changes
        )
    }

    fn record_sample(&mut self, sample: TextureSample) {
        if sample.texture_nonzero {
            self.texture_nonzero_samples = self.texture_nonzero_samples.saturating_add(1);
        }
        if sample.zero_texel {
            self.zero_texel_samples = self.zero_texel_samples.saturating_add(1);
        }
        if sample.clut_nonzero {
            self.clut_nonzero_samples = self.clut_nonzero_samples.saturating_add(1);
        }
        if sample.clut_blank {
            self.clut_blank_samples = self.clut_blank_samples.saturating_add(1);
        }
        if sample.palette_fallback {
            self.palette_fallback_samples = self.palette_fallback_samples.saturating_add(1);
        }
        if sample.texture_nonzero && sample.color == 0 {
            self.nonzero_texel_transparent_samples =
                self.nonzero_texel_transparent_samples.saturating_add(1);
        }
    }

    fn record_color(&mut self, color: u16) {
        if self.drawn_pixels == 0 {
            self.first_color = color;
        } else if self.last_color != color {
            self.color_changes = self.color_changes.saturating_add(1);
        }
        self.last_color = color;
        self.color_hash ^= u32::from(color);
        self.color_hash = self.color_hash.wrapping_mul(16_777_619);
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct TextureSample {
    color: u16,
    texture_nonzero: bool,
    zero_texel: bool,
    clut_nonzero: bool,
    clut_blank: bool,
    palette_fallback: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TextureSamplingPolicy {
    allow_palette_fallback: bool,
    allow_texture_descriptor_alias: bool,
}

impl TextureSamplingPolicy {
    const fn new(allow_palette_fallback: bool, allow_texture_descriptor_alias: bool) -> Self {
        Self {
            allow_palette_fallback,
            allow_texture_descriptor_alias,
        }
    }

    const fn diagnostics_default() -> Self {
        Self::new(true, true)
    }

    const fn from_draw_options(options: TextureDrawOptions) -> Self {
        Self::new(
            options.allow_palette_fallback,
            options.allow_texture_descriptor_alias,
        )
    }

    fn resolve_descriptor(self, texture_page: u16, clut: u16) -> (u16, u16) {
        if self.allow_texture_descriptor_alias {
            br2_texture_descriptor_alias(texture_page, clut)
        } else {
            (texture_page, clut)
        }
    }
}

struct PreparedTextureDrawResources {
    source_snapshot: Option<Vec<u16>>,
    sampler: PreparedTextureSampler,
}

impl PreparedTextureDrawResources {
    fn new(
        framebuffer: &NativeFrameBuffer,
        dest_bounds: (i32, i32, i32, i32),
        texture_page: u16,
        clut: u16,
        sampling_policy: TextureSamplingPolicy,
    ) -> Self {
        let source_snapshot = framebuffer.textured_draw_requires_snapshot(
            dest_bounds,
            texture_page,
            clut,
            sampling_policy.allow_texture_descriptor_alias,
        );
        let sampler = PreparedTextureSampler::new(
            source_snapshot
                .as_deref()
                .unwrap_or(&framebuffer.raw_pixels),
            texture_page,
            clut,
            sampling_policy,
            framebuffer
                .recovery_raster_context
                .is_some()
                .then_some(&framebuffer.recovery_palette_history),
        );
        Self {
            source_snapshot,
            sampler,
        }
    }

    fn sample(&self, framebuffer: &NativeFrameBuffer, u: u8, v: u8) -> TextureSample {
        self.sampler.sample(
            self.source_snapshot
                .as_deref()
                .unwrap_or(&framebuffer.raw_pixels),
            u,
            v,
        )
    }

    #[cfg(test)]
    fn snapshot_used(&self) -> bool {
        self.source_snapshot.is_some()
    }
}

struct PreparedTextureSampler {
    mode: u16,
    origin_x: i32,
    origin_y: i32,
    raw_width: i32,
    swap_8bpp_bytes: bool,
    indexed_samples: [TextureSample; 256],
}

impl PreparedTextureSampler {
    fn new(
        raw_pixels: &[u16],
        texture_page: u16,
        clut: u16,
        sampling_policy: TextureSamplingPolicy,
        recovery_palette_history: Option<&Arc<Mutex<RecoveryPaletteHistory>>>,
    ) -> Self {
        let (texture_page, clut) = sampling_policy.resolve_descriptor(texture_page, clut);
        let mode = texture_page_color_mode(texture_page);
        let (origin_x, origin_y) = texture_page_origin_for_clut(texture_page, clut);
        let entries = texture_palette_entries(texture_page);
        let indexed_samples = prepared_indexed_palette_samples_with_history(
            raw_pixels,
            texture_page,
            clut,
            entries,
            sampling_policy.allow_palette_fallback,
            recovery_palette_history,
        );

        Self {
            mode,
            origin_x,
            origin_y,
            raw_width: texture_page_raw_width(texture_page).max(1) as i32,
            swap_8bpp_bytes: native_swap_8bpp_texture_bytes(),
            indexed_samples,
        }
    }

    fn sample(&self, raw_pixels: &[u16], u: u8, v: u8) -> TextureSample {
        let u = i32::from(u);
        let v = i32::from(v);
        let raw_x = |offset: i32| self.origin_x + offset.rem_euclid(self.raw_width);

        match self.mode {
            0 => {
                let packed = raw_pixel_from(raw_pixels, raw_x(u / 4), self.origin_y + v);
                let index = ((packed >> ((u & 3) * 4)) & 0x0f) as usize;
                self.indexed_samples[index]
            }
            1 => {
                let packed = raw_pixel_from(raw_pixels, raw_x(u / 2), self.origin_y + v);
                let use_high_byte = (u & 1 == 0) == self.swap_8bpp_bytes;
                let index = if use_high_byte {
                    packed >> 8
                } else {
                    packed & 0x00ff
                } as usize;
                self.indexed_samples[index]
            }
            _ => {
                let color = raw_pixel_from(raw_pixels, raw_x(u), self.origin_y + v);
                TextureSample {
                    color,
                    texture_nonzero: color != 0,
                    zero_texel: color == 0,
                    clut_nonzero: color != 0,
                    ..TextureSample::default()
                }
            }
        }
    }
}

fn prepared_indexed_palette_samples_with_history(
    raw_pixels: &[u16],
    texture_page: u16,
    clut: u16,
    entries: usize,
    allow_palette_fallback: bool,
    recovery_palette_history: Option<&Arc<Mutex<RecoveryPaletteHistory>>>,
) -> [TextureSample; 256] {
    let mut samples = prepared_indexed_palette_samples_from(
        raw_pixels,
        texture_page,
        clut,
        entries,
        allow_palette_fallback,
    );
    let Some(history) = recovery_palette_history else {
        return samples;
    };
    if entries != 16
        || !allow_palette_fallback
        || !br2_low_bank_gameplay_character_texture_descriptor(texture_page, clut)
    {
        return samples;
    }

    let key = RecoveryPaletteHistoryKey {
        texture_page: texture_page_without_dither(texture_page),
        clut,
        texture_signature: recovery_texture_page_signature(raw_pixels, texture_page, clut),
    };
    let stats = palette_colors_stats(samples[..16].iter().map(|sample| sample.color));
    let trustworthy = stats.nonzero_entries >= 12
        && stats.unique_entries >= 3
        && stats.average_luma() >= 6
        && stats.max_luma >= 6
        && !stats.is_implausibly_dark_texture_row()
        && !stats.is_low_bank_red_polluted();
    let mut history = history.lock().expect("recovery palette history lock");
    if trustworthy {
        let mut colors = [0_u16; 16];
        for (color, sample) in colors.iter_mut().zip(samples.iter()) {
            *color = sample.color;
        }
        if !history.entries.contains_key(&key) {
            history.order.push_back(key);
        }
        history.entries.insert(key, colors);
        while history.entries.len() > RECOVERY_PALETTE_HISTORY_ENTRY_LIMIT {
            let Some(oldest) = history.order.pop_front() else {
                break;
            };
            history.entries.remove(&oldest);
        }
        return samples;
    }

    let Some(colors) = history.entries.get(&key).copied() else {
        return samples;
    };
    drop(history);
    for (index, sample) in samples.iter_mut().enumerate().take(16) {
        sample.color = colors[index];
        sample.clut_nonzero = colors[index] != 0;
        sample.clut_blank = false;
        sample.palette_fallback = true;
    }
    if br2_character_model_palette_index_zero_transparent(texture_page, clut)
        || native_palette_index_zero_transparent(texture_page, samples[0].color)
    {
        samples[0].color = 0;
        samples[0].clut_nonzero = false;
    }
    samples
}

fn recovery_texture_page_signature(raw_pixels: &[u16], texture_page: u16, clut: u16) -> u64 {
    let (origin_x, origin_y) = texture_page_origin_for_clut(texture_page, clut);
    let raw_width = texture_page_raw_width(texture_page).max(1);
    let x_stride = (raw_width / 8).max(1);
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;

    for y in (0..256).step_by(16) {
        for x in (0..raw_width).step_by(x_stride) {
            let color = raw_pixel_from(raw_pixels, origin_x + x as i32, origin_y + y);
            hash ^= u64::from(color) | ((x as u64) << 16) | ((y as u64) << 32);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    hash
}

fn prepared_indexed_palette_samples_from(
    raw_pixels: &[u16],
    texture_page: u16,
    clut: u16,
    entries: usize,
    allow_palette_fallback: bool,
) -> [TextureSample; 256] {
    let mut samples = [TextureSample::default(); 256];
    if entries == 256 {
        let requested_stats = palette_row_stats(raw_pixels, clut, entries);
        let linear_candidate =
            fallback_linear_256_palette_candidate(raw_pixels, texture_page, clut);
        let tiled_candidate = fallback_tiled_256_palette_candidate(raw_pixels, texture_page, clut);
        let force_y16_base = allow_palette_fallback && native_force_y16_base_256_palette();

        for (index, sample) in samples.iter_mut().enumerate() {
            let requested_color = palette_raw_pixel_from(raw_pixels, clut, index as i32);
            *sample = TextureSample {
                color: requested_color,
                texture_nonzero: index != 0,
                zero_texel: index == 0,
                clut_nonzero: requested_color != 0,
                ..TextureSample::default()
            };

            if index == 0 {
                if br2_character_model_palette_index_zero_transparent(texture_page, clut)
                    || native_palette_index_zero_transparent(texture_page, sample.color)
                {
                    sample.color = 0;
                    sample.clut_nonzero = false;
                }
                continue;
            }

            sample.clut_blank = requested_stats.nonzero_entries == 0;
            if force_y16_base {
                let color = palette_y16_base_raw_pixel_from(raw_pixels, clut, index as i32);
                sample.color = color;
                sample.palette_fallback = true;
                sample.clut_nonzero = color != 0;
                continue;
            }

            if let Some(candidate) = linear_candidate
                && should_use_linear_256_palette_sample(
                    texture_page,
                    clut,
                    allow_palette_fallback,
                    requested_stats.nonzero_entries,
                    candidate.nonzero_entries,
                    requested_color,
                )
            {
                sample.color = raw_pixel_from(raw_pixels, candidate.x + index as i32, candidate.y);
                sample.palette_fallback = true;
                sample.clut_nonzero = sample.color != 0;
                continue;
            }

            if let Some(candidate) = tiled_candidate
                && should_use_tiled_256_palette_sample(
                    texture_page,
                    clut,
                    allow_palette_fallback,
                    requested_stats.nonzero_entries,
                    candidate.nonzero_entries,
                    requested_color,
                )
            {
                sample.color = raw_pixel_from(
                    raw_pixels,
                    candidate.x + (index as i32 & 0x0f),
                    candidate.y + index as i32 / 16,
                );
                sample.palette_fallback = true;
                sample.clut_nonzero = sample.color != 0;
                continue;
            }

            if requested_color != 0 || !sample.clut_blank || !allow_palette_fallback {
                continue;
            }

            if let Some(candidate) = tiled_candidate {
                let fallback = raw_pixel_from(
                    raw_pixels,
                    candidate.x + (index as i32 & 0x0f),
                    candidate.y + index as i32 / 16,
                );
                if fallback != 0 {
                    sample.color = fallback;
                    sample.palette_fallback = true;
                }
            }
        }
        return samples;
    }
    if entries != 16 {
        return samples;
    }

    let requested_stats = palette_row_stats(raw_pixels, clut, entries);
    let use_br2_fallback = fallback_br2_4bpp_palette_candidate(raw_pixels, texture_page, clut)
        .filter(|_| {
            should_use_br2_4bpp_palette_sample(
                texture_page,
                clut,
                allow_palette_fallback,
                requested_stats,
            )
        });
    let use_zn_fallback = fallback_zn_4bpp_palette_candidate(raw_pixels, texture_page, clut)
        .filter(|candidate| {
            should_use_zn_4bpp_palette_sample(
                texture_page,
                clut,
                allow_palette_fallback,
                requested_stats.nonzero_entries,
                requested_stats.unique_entries,
                candidate.nonzero_entries,
                candidate.unique_entries,
            )
        });
    let missing_body_material = allow_palette_fallback
        && br2_gameplay_body_missing_palette_descriptor(texture_page, clut)
        && requested_stats.nonzero_entries == 0
        && should_use_br2_4bpp_palette_sample(
            texture_page,
            clut,
            allow_palette_fallback,
            requested_stats,
        );

    for (index, sample) in samples.iter_mut().enumerate().take(entries) {
        let requested_color = palette_raw_pixel_from(raw_pixels, clut, index as i32);
        *sample = TextureSample {
            color: requested_color,
            texture_nonzero: index != 0,
            zero_texel: index == 0,
            clut_nonzero: requested_color != 0,
            clut_blank: index != 0 && requested_stats.nonzero_entries == 0,
            ..TextureSample::default()
        };

        if let Some(candidate) = use_br2_fallback {
            sample.color = raw_pixel_from(raw_pixels, candidate.x + index as i32, candidate.y);
            sample.palette_fallback = true;
            sample.clut_nonzero = sample.color != 0;
        } else if index != 0 && missing_body_material {
            sample.color = 0x7fff;
            sample.palette_fallback = true;
            sample.clut_nonzero = true;
        } else if index != 0
            && let Some(candidate) = use_zn_fallback
        {
            sample.color = raw_pixel_from(raw_pixels, candidate.x + index as i32, candidate.y);
            sample.palette_fallback = true;
            sample.clut_nonzero = sample.color != 0;
        } else if index != 0
            && requested_color == 0
            && requested_stats.nonzero_entries == 0
            && allow_palette_fallback
            && let Some(fallback) = fallback_palette_raw_pixel_from(
                raw_pixels,
                texture_page,
                clut,
                index as i32,
                entries,
            )
        {
            sample.color = fallback;
            sample.palette_fallback = true;
        }

        if index == 0
            && (br2_character_model_palette_index_zero_transparent(texture_page, clut)
                || native_palette_index_zero_transparent(texture_page, sample.color))
        {
            sample.color = 0;
            sample.clut_nonzero = false;
        }
    }

    samples
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextureCoordinate {
    pub u: u8,
    pub v: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextureDrawOptions {
    pub primitive_color: u32,
    pub raw_texture: bool,
    pub semi_transparent: bool,
    pub semi_transparency_mode: u8,
    pub set_mask_bit: bool,
    pub check_mask_bit: bool,
    pub texture_flip_x: bool,
    pub texture_flip_y: bool,
    pub allow_palette_fallback: bool,
    pub allow_texture_descriptor_alias: bool,
}

impl TextureDrawOptions {
    pub const fn opaque_raw() -> Self {
        Self {
            primitive_color: 0x0080_8080,
            raw_texture: true,
            semi_transparent: false,
            semi_transparency_mode: 0,
            set_mask_bit: false,
            check_mask_bit: false,
            texture_flip_x: false,
            texture_flip_y: false,
            allow_palette_fallback: true,
            allow_texture_descriptor_alias: true,
        }
    }

    fn apply_color(self, color: u16) -> u16 {
        self.apply_color_with_primitive(color, self.primitive_color)
    }

    fn apply_color_with_primitive(self, color: u16, primitive_color: u32) -> u16 {
        if self.raw_texture {
            return color;
        }
        modulate_rgb555(color, primitive_color)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TextureWindow {
    mask_x: u8,
    mask_y: u8,
    offset_x: u8,
    offset_y: u8,
}

impl TextureWindow {
    pub fn from_gp0_e2(value: u32) -> Self {
        Self {
            mask_x: ((value & 0x1f) as u8) << 3,
            mask_y: (((value >> 5) & 0x1f) as u8) << 3,
            offset_x: (((value >> 10) & 0x1f) as u8) << 3,
            offset_y: (((value >> 15) & 0x1f) as u8) << 3,
        }
    }

    pub fn json(self) -> String {
        format!(
            "{{\"mask_x\":{},\"mask_y\":{},\"offset_x\":{},\"offset_y\":{}}}",
            self.mask_x, self.mask_y, self.offset_x, self.offset_y
        )
    }

    pub fn gp0_value(self) -> u32 {
        u32::from(self.mask_x >> 3)
            | (u32::from(self.mask_y >> 3) << 5)
            | (u32::from(self.offset_x >> 3) << 10)
            | (u32::from(self.offset_y >> 3) << 15)
    }

    fn apply(self, u: u8, v: u8) -> (u8, u8) {
        (
            (u & !self.mask_x) | (self.offset_x & self.mask_x),
            (v & !self.mask_y) | (self.offset_y & self.mask_y),
        )
    }
}

fn edge(a: Point, b: Point, c: Point) -> i64 {
    ((c.x - a.x) as i64 * (b.y - a.y) as i64) - ((c.y - a.y) as i64 * (b.x - a.x) as i64)
}

fn recovery_raster_mix(hash: u64, value: u64) -> u64 {
    hash ^ value
        .wrapping_add(0x9e37_79b9_7f4a_7c15)
        .wrapping_add(hash << 6)
        .wrapping_add(hash >> 2)
}

fn psx_top_left_edge(a: Point, b: Point, area: i64) -> bool {
    let (dx, dy) = if area > 0 {
        (b.x - a.x, b.y - a.y)
    } else {
        (a.x - b.x, a.y - b.y)
    };
    dy > 0 || (dy == 0 && dx < 0)
}

fn psx_inside_edge(value: i64, a: Point, b: Point, area: i64) -> bool {
    let signed = if area > 0 { value } else { -value };
    signed > 0 || (signed == 0 && psx_top_left_edge(a, b, area))
}

fn psx_triangle_weights(
    a: Point,
    b: Point,
    c: Point,
    point: Point,
    area: i64,
) -> Option<(i64, i64, i64)> {
    let mut w0 = edge(b, c, point);
    let mut w1 = edge(c, a, point);
    let mut w2 = edge(a, b, point);
    if !psx_inside_edge(w0, b, c, area)
        || !psx_inside_edge(w1, c, a, area)
        || !psx_inside_edge(w2, a, b, area)
    {
        return None;
    }
    if area < 0 {
        w0 = -w0;
        w1 = -w1;
        w2 = -w2;
    }
    Some((w0, w1, w2))
}

fn triangle_exceeds_gpu_size_limit(a: Point, b: Point, c: Point) -> bool {
    let min_x = a.x.min(b.x).min(c.x);
    let max_x = a.x.max(b.x).max(c.x);
    let min_y = a.y.min(b.y).min(c.y);
    let max_y = a.y.max(b.y).max(c.y);
    let width = max_x.saturating_sub(min_x).saturating_add(1);
    let height = max_y.saturating_sub(min_y).saturating_add(1);
    width > GPU_MAX_PRIMITIVE_WIDTH || height > GPU_MAX_PRIMITIVE_HEIGHT
}

fn shared_triangle_dest_bounds(
    first: Option<TexturedTriangleRasterBounds>,
    second: Option<TexturedTriangleRasterBounds>,
) -> Option<(i32, i32, i32, i32)> {
    match (first, second) {
        (Some(first), Some(second)) => Some((
            first.min_x.min(second.min_x),
            first.min_y.min(second.min_y),
            first.max_x.max(second.max_x),
            first.max_y.max(second.max_y),
        )),
        (Some(bounds), None) | (None, Some(bounds)) => Some(bounds.dest_bounds()),
        (None, None) => None,
    }
}

fn integral_rect(integral: &[u32], x: usize, y: usize, width: usize, height: usize) -> u64 {
    let stride = VRAM_WIDTH + 1;
    let right = x + width;
    let bottom = y + height;
    let value = integral[bottom * stride + right] + integral[y * stride + x]
        - integral[y * stride + right]
        - integral[bottom * stride + x];
    value as u64
}

fn integral_rect_u64(integral: &[u64], x: usize, y: usize, width: usize, height: usize) -> u64 {
    let stride = VRAM_WIDTH + 1;
    let right = x + width;
    let bottom = y + height;
    integral[bottom * stride + right] + integral[y * stride + x]
        - integral[y * stride + right]
        - integral[bottom * stride + x]
}

fn stepped_positions(max: usize, step: usize) -> impl Iterator<Item = usize> {
    (0..=max)
        .step_by(step)
        .chain(std::iter::once(max))
        .scan(None, |previous, value| {
            if previous.is_some_and(|previous| previous == value) {
                return Some(None);
            }
            *previous = Some(value);
            Some(Some(value))
        })
        .flatten()
}

pub fn png_from_rgb888_pixels(width: usize, height: usize, pixels: &[u32]) -> Vec<u8> {
    let width = width.max(1);
    let height = height.max(1);
    let mut rows = Vec::with_capacity(height.saturating_mul(width.saturating_mul(3) + 1));
    for y in 0..height {
        rows.push(0);
        let row = y.saturating_mul(width);
        for x in 0..width {
            let color = pixels.get(row + x).copied().unwrap_or_default();
            rows.push(((color >> 16) & 0xff) as u8);
            rows.push(((color >> 8) & 0xff) as u8);
            rows.push((color & 0xff) as u8);
        }
    }
    png_rgb(width, height, &rows)
}

fn png_rgb(width: usize, height: usize, filtered_rgb_rows: &[u8]) -> Vec<u8> {
    let mut png = Vec::new();
    png.extend_from_slice(&[137, 80, 78, 71, 13, 10, 26, 10]);

    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&(width as u32).to_be_bytes());
    ihdr.extend_from_slice(&(height as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    write_png_chunk(&mut png, b"IHDR", &ihdr);
    write_png_chunk(&mut png, b"IDAT", &zlib_uncompressed(filtered_rgb_rows));
    write_png_chunk(&mut png, b"IEND", &[]);
    png
}

fn write_png_chunk(output: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    output.extend_from_slice(&(data.len() as u32).to_be_bytes());
    output.extend_from_slice(kind);
    output.extend_from_slice(data);
    let mut crc_input = Vec::with_capacity(kind.len() + data.len());
    crc_input.extend_from_slice(kind);
    crc_input.extend_from_slice(data);
    output.extend_from_slice(&crc32(&crc_input).to_be_bytes());
}

fn zlib_uncompressed(data: &[u8]) -> Vec<u8> {
    let mut output = vec![0x78, 0x01];
    let mut remaining = data;
    while !remaining.is_empty() {
        let chunk_len = remaining.len().min(u16::MAX as usize);
        let final_block = chunk_len == remaining.len();
        output.push(if final_block { 0x01 } else { 0x00 });
        output.extend_from_slice(&(chunk_len as u16).to_le_bytes());
        output.extend_from_slice(&(!(chunk_len as u16)).to_le_bytes());
        output.extend_from_slice(&remaining[..chunk_len]);
        remaining = &remaining[chunk_len..];
    }
    if data.is_empty() {
        output.extend_from_slice(&[0x01, 0x00, 0x00, 0xff, 0xff]);
    }
    output.extend_from_slice(&adler32(data).to_be_bytes());
    output
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffff;
    for byte in data {
        crc ^= *byte as u32;
        for _ in 0..8 {
            let mask = 0_u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

fn adler32(data: &[u8]) -> u32 {
    const MOD: u32 = 65_521;
    let mut a = 1_u32;
    let mut b = 0_u32;
    for byte in data {
        a = (a + *byte as u32) % MOD;
        b = (b + a) % MOD;
    }
    (b << 16) | a
}

fn rgb555_to_rgb888(value: u16) -> u32 {
    let r = (value & 0x1f) as u32;
    let g = ((value >> 5) & 0x1f) as u32;
    let b = ((value >> 10) & 0x1f) as u32;
    ((r << 3) | (r >> 2)) << 16 | ((g << 3) | (g >> 2)) << 8 | ((b << 3) | (b >> 2))
}

fn rgb888_to_rgb555(value: u32) -> u16 {
    let r = ((value >> 19) & 0x1f) as u16;
    let g = ((value >> 11) & 0x1f) as u16;
    let b = ((value >> 3) & 0x1f) as u16;
    r | (g << 5) | (b << 10)
}

fn modulate_rgb555(color: u16, primitive_color: u32) -> u16 {
    let r = modulate_channel(color & 0x1f, primitive_color & 0xff);
    let g = modulate_channel((color >> 5) & 0x1f, (primitive_color >> 8) & 0xff);
    let b = modulate_channel((color >> 10) & 0x1f, (primitive_color >> 16) & 0xff);
    (color & 0x8000) | r | (g << 5) | (b << 10)
}

fn interpolate_psx_rgb(colors: [u32; 3], w0: i64, w1: i64, w2: i64, denom: i64) -> u32 {
    let denom = denom.max(1);
    let channel = |shift: u32| -> u32 {
        (((i64::from((colors[0] >> shift) & 0xff) * w0)
            + (i64::from((colors[1] >> shift) & 0xff) * w1)
            + (i64::from((colors[2] >> shift) & 0xff) * w2))
            / denom)
            .clamp(0, 0xff) as u32
    };
    channel(0) | (channel(8) << 8) | (channel(16) << 16)
}

fn psx_rgb_to_rgb888(value: u32) -> u32 {
    let r = value & 0xff;
    let g = (value >> 8) & 0xff;
    let b = (value >> 16) & 0xff;
    (r << 16) | (g << 8) | b
}

fn clut_y(clut: u16) -> u16 {
    (clut >> 6) & 0x03ff
}

pub fn texture_page_color_mode_for_diagnostics(texture_page: u16) -> u16 {
    texture_page_color_mode(texture_page)
}

pub fn texture_page_origin_for_diagnostics(texture_page: u16) -> (i32, i32) {
    texture_page_origin(texture_page)
}

pub fn texture_page_origin_for_clut_for_diagnostics(texture_page: u16, clut: u16) -> (i32, i32) {
    texture_page_origin_for_clut(texture_page, clut)
}

pub fn clut_origin_for_diagnostics(clut: u16) -> (i32, i32) {
    (((clut & 0x3f) as i32) * 16, clut_y(clut) as i32)
}

fn raw_pixel_from(raw_pixels: &[u16], x: i32, y: i32) -> u16 {
    if x < 0 || y < 0 || x >= VRAM_WIDTH as i32 || y >= VRAM_HEIGHT as i32 {
        return 0;
    }

    raw_pixels
        .get(y as usize * VRAM_WIDTH + x as usize)
        .copied()
        .unwrap_or_default()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OriginCandidate {
    label: &'static str,
    x: i32,
    y: i32,
}

fn texture_origin_candidates(texture_page: u16) -> Vec<OriginCandidate> {
    let (page_x, page_y) = texture_page_origin(texture_page);
    let standard_x = ((texture_page & 0x0f) as i32) * 64;
    let standard_y = texture_page_y(texture_page) as i32;
    let extended_x = (((texture_page >> 10) & 0x0f) as i32) * 64;
    let extended_low3_x = (((texture_page >> 10) & 0x07) as i32) * 64;
    let extended_y = (((texture_page >> 9) & 0x01) as i32) * 256;

    let mut candidates = Vec::new();
    push_unique_origin(&mut candidates, "resolved", page_x, page_y);
    push_unique_origin(&mut candidates, "resolved_minus_64", page_x - 64, page_y);
    push_unique_origin(&mut candidates, "resolved_plus_64", page_x + 64, page_y);
    push_unique_origin(&mut candidates, "psx_standard", standard_x, standard_y);
    push_unique_origin(&mut candidates, "standard_y0", standard_x, 0);
    push_unique_origin(&mut candidates, "standard_y256", standard_x, 256);
    push_unique_origin(&mut candidates, "zn_extended_bits", extended_x, extended_y);
    push_unique_origin(
        &mut candidates,
        "zn_extended_low3_bits",
        extended_low3_x,
        extended_y,
    );
    push_unique_origin(&mut candidates, "resolved_y0", page_x, 0);
    push_unique_origin(&mut candidates, "resolved_y256", page_x, 256);
    candidates
}

fn texture_origin_candidates_for_clut(texture_page: u16, clut: u16) -> Vec<OriginCandidate> {
    let (page_x, page_y) = texture_page_origin_for_clut(texture_page, clut);
    let mut candidates = Vec::new();
    push_unique_origin(&mut candidates, "resolved", page_x, page_y);
    push_unique_origin(&mut candidates, "resolved_minus_64", page_x - 64, page_y);
    push_unique_origin(&mut candidates, "resolved_plus_64", page_x + 64, page_y);
    for candidate in texture_origin_candidates(texture_page) {
        push_unique_origin(&mut candidates, candidate.label, candidate.x, candidate.y);
    }
    candidates
}

fn push_unique_origin(candidates: &mut Vec<OriginCandidate>, label: &'static str, x: i32, y: i32) {
    if candidates
        .iter()
        .any(|candidate| candidate.x == x && candidate.y == y)
    {
        return;
    }
    candidates.push(OriginCandidate { label, x, y });
}

fn optional_raw_sample_json(sample: Option<(i32, i32, u16)>) -> String {
    sample.map_or_else(
        || "null".to_string(),
        |(x, y, value)| {
            format!(
                "{{\"x\":{},\"y\":{},\"value\":{},\"value_hex\":\"0x{:04x}\"}}",
                x, y, value, value
            )
        },
    )
}

fn format_index_hist_json(hist: &[usize], sparse: bool) -> String {
    format!(
        "[{}]",
        hist.iter()
            .enumerate()
            .filter(|(_, count)| !sparse || **count != 0)
            .map(|(index, count)| format!("{{\"index\":{},\"count\":{}}}", index, count))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn palette_raw_pixel_from(raw_pixels: &[u16], clut: u16, index: i32) -> u16 {
    let clut_x = ((clut & 0x3f) as i32) * 16;
    raw_pixel_from(raw_pixels, clut_x + index, clut_y(clut) as i32)
}

fn indexed_palette_sample_from(
    raw_pixels: &[u16],
    texture_page: u16,
    clut: u16,
    index: i32,
    entries: usize,
    allow_palette_fallback: bool,
) -> TextureSample {
    let color = palette_raw_pixel_from(raw_pixels, clut, index);
    let mut sample = TextureSample {
        color,
        texture_nonzero: index != 0,
        zero_texel: index == 0,
        clut_nonzero: color != 0,
        ..TextureSample::default()
    };
    let requested_stats = palette_row_stats(raw_pixels, clut, entries);
    if index == 0 {
        if allow_palette_fallback
            && entries == 16
            && let Some(fallback) =
                fallback_br2_4bpp_palette_sample(raw_pixels, texture_page, clut, index)
            && should_use_br2_4bpp_palette_sample(
                texture_page,
                clut,
                allow_palette_fallback,
                requested_stats,
            )
        {
            sample.color = fallback.color;
            sample.palette_fallback = true;
            sample.clut_nonzero = fallback.color != 0;
        }
        if br2_character_model_palette_index_zero_transparent(texture_page, clut)
            || native_palette_index_zero_transparent(texture_page, sample.color)
        {
            sample.color = 0;
            sample.clut_nonzero = false;
        }
        return sample;
    }

    sample.clut_blank = requested_stats.nonzero_entries == 0;
    if allow_palette_fallback
        && entries == 16
        && let Some(fallback) =
            fallback_br2_4bpp_palette_sample(raw_pixels, texture_page, clut, index)
        && should_use_br2_4bpp_palette_sample(
            texture_page,
            clut,
            allow_palette_fallback,
            requested_stats,
        )
    {
        sample.color = fallback.color;
        sample.palette_fallback = true;
        sample.clut_nonzero = fallback.color != 0;
        return sample;
    }
    if allow_palette_fallback
        && entries == 16
        && br2_gameplay_body_missing_palette_descriptor(texture_page, clut)
        && sample.clut_blank
        && should_use_br2_4bpp_palette_sample(
            texture_page,
            clut,
            allow_palette_fallback,
            requested_stats,
        )
    {
        // If the shared fighter bank is unavailable, preserve the material
        // shading without sampling the adjacent stage-atlas data.
        sample.color = 0x7fff;
        sample.palette_fallback = true;
        sample.clut_nonzero = true;
        return sample;
    }
    if entries == 16
        && let Some(fallback) =
            fallback_zn_4bpp_palette_sample(raw_pixels, texture_page, clut, index)
        && should_use_zn_4bpp_palette_sample(
            texture_page,
            clut,
            allow_palette_fallback,
            requested_stats.nonzero_entries,
            requested_stats.unique_entries,
            fallback.nonzero_entries,
            fallback.unique_entries,
        )
    {
        sample.color = fallback.color;
        sample.palette_fallback = true;
        sample.clut_nonzero = fallback.color != 0;
        return sample;
    }

    if allow_palette_fallback && entries == 256 && native_force_y16_base_256_palette() {
        let color = palette_y16_base_raw_pixel_from(raw_pixels, clut, index);
        sample.color = color;
        sample.palette_fallback = true;
        sample.clut_nonzero = color != 0;
        return sample;
    }

    if entries == 256
        && let Some(fallback) =
            fallback_linear_256_palette_sample(raw_pixels, texture_page, clut, index)
        && should_use_linear_256_palette_sample(
            texture_page,
            clut,
            allow_palette_fallback,
            requested_stats.nonzero_entries,
            fallback.nonzero_entries,
            color,
        )
    {
        sample.color = fallback.color;
        sample.palette_fallback = true;
        sample.clut_nonzero = fallback.color != 0;
        return sample;
    }

    if entries == 256
        && let Some(fallback) =
            fallback_tiled_256_palette_sample(raw_pixels, texture_page, clut, index)
        && should_use_tiled_256_palette_sample(
            texture_page,
            clut,
            allow_palette_fallback,
            requested_stats.nonzero_entries,
            fallback.nonzero_entries,
            color,
        )
    {
        sample.color = fallback.color;
        sample.palette_fallback = true;
        sample.clut_nonzero = fallback.color != 0;
        return sample;
    }

    if color != 0 {
        return sample;
    }

    if !sample.clut_blank {
        return sample;
    }

    if !allow_palette_fallback {
        return sample;
    }

    if let Some(fallback) =
        fallback_palette_raw_pixel_from(raw_pixels, texture_page, clut, index, entries)
    {
        sample.color = fallback;
        sample.palette_fallback = true;
    }
    sample
}

fn palette_row_nonzero_entries(raw_pixels: &[u16], clut: u16, entries: usize) -> usize {
    palette_row_stats(raw_pixels, clut, entries).nonzero_entries
}

fn palette_row_stats(raw_pixels: &[u16], clut: u16, entries: usize) -> PaletteRegionStats {
    let clut_x = ((clut & 0x3f) as i32) * 16;
    let clut_y = clut_y(clut) as i32;
    palette_region_stats(raw_pixels, clut_x, clut_y, entries).unwrap_or_default()
}

fn palette_y16_base_raw_pixel_from(raw_pixels: &[u16], clut: u16, index: i32) -> u16 {
    let clut_x = ((clut & 0x3f) as i32) * 16;
    let clut_y = (clut_y(clut) as i32) & !0x0f;
    raw_pixel_from(raw_pixels, clut_x + index, clut_y)
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct PaletteRegionStats {
    nonzero_entries: usize,
    unique_entries: usize,
    luma_sum: u32,
    red_sum: u32,
    green_sum: u32,
    blue_sum: u32,
    red_only_entries: usize,
    entries: usize,
    max_luma: u8,
}

impl PaletteRegionStats {
    fn average_luma(self) -> u8 {
        self.luma_sum
            .checked_div(self.entries as u32)
            .unwrap_or(0)
            .min(31) as u8
    }

    fn average_red(self) -> u8 {
        self.average_nonzero_channel(self.red_sum)
    }

    fn average_green(self) -> u8 {
        self.average_nonzero_channel(self.green_sum)
    }

    fn average_blue(self) -> u8 {
        self.average_nonzero_channel(self.blue_sum)
    }

    fn average_nonzero_channel(self, sum: u32) -> u8 {
        sum.checked_div(self.nonzero_entries as u32)
            .unwrap_or(0)
            .min(31) as u8
    }

    fn is_low_bank_red_polluted(self) -> bool {
        self.nonzero_entries >= 12
            && self.unique_entries >= 8
            && self.average_luma() <= 10
            && self.max_luma <= 14
            && self.red_only_entries >= 6
            && self.average_red() >= 8
            && self.average_green() <= 4
            && self.average_blue() <= 4
            && self.red_sum
                >= self
                    .green_sum
                    .saturating_add(self.blue_sum)
                    .saturating_mul(2)
    }

    fn is_implausibly_dark_texture_row(self) -> bool {
        self.nonzero_entries == self.entries
            && (self.unique_entries.saturating_add(4) >= self.entries || self.red_only_entries >= 5)
            && self.average_luma() <= 8
            && self.max_luma <= 8
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LinearPaletteSample {
    color: u16,
    nonzero_entries: usize,
    unique_entries: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LinearPaletteCandidate {
    x: i32,
    y: i32,
    nonzero_entries: usize,
    unique_entries: usize,
}

fn palette_region_stats(
    raw_pixels: &[u16],
    x: i32,
    y: i32,
    entries: usize,
) -> Option<PaletteRegionStats> {
    if x < 0 || y < 0 || x + entries as i32 > VRAM_WIDTH as i32 || y >= VRAM_HEIGHT as i32 {
        return None;
    }

    let mut unique = [0_u16; 256];
    let mut unique_entries = 0usize;
    let mut nonzero_entries = 0usize;
    let mut luma_sum = 0_u32;
    let mut red_sum = 0_u32;
    let mut green_sum = 0_u32;
    let mut blue_sum = 0_u32;
    let mut red_only_entries = 0usize;
    let mut max_luma = 0_u8;
    for offset in 0..entries {
        let color = raw_pixel_from(raw_pixels, x + offset as i32, y);
        if color != 0 {
            nonzero_entries = nonzero_entries.saturating_add(1);
        }
        let red = color & 0x001f;
        let green = (color >> 5) & 0x001f;
        let blue = (color >> 10) & 0x001f;
        red_sum = red_sum.saturating_add(u32::from(red));
        green_sum = green_sum.saturating_add(u32::from(green));
        blue_sum = blue_sum.saturating_add(u32::from(blue));
        if color != 0 && red >= 8 && green <= 1 && blue == 0 {
            red_only_entries = red_only_entries.saturating_add(1);
        }
        let luma = rgb555_luma(color);
        luma_sum = luma_sum.saturating_add(u32::from(luma));
        max_luma = max_luma.max(luma);
        if !unique[..unique_entries].contains(&color) {
            unique[unique_entries] = color;
            unique_entries = unique_entries.saturating_add(1);
        }
    }

    Some(PaletteRegionStats {
        nonzero_entries,
        unique_entries,
        luma_sum,
        red_sum,
        green_sum,
        blue_sum,
        red_only_entries,
        entries,
        max_luma,
    })
}

fn palette_colors_stats(colors: impl IntoIterator<Item = u16>) -> PaletteRegionStats {
    let colors = colors.into_iter().collect::<Vec<_>>();
    let mut unique = [0_u16; 256];
    let mut unique_entries = 0usize;
    let mut nonzero_entries = 0usize;
    let mut luma_sum = 0_u32;
    let mut red_sum = 0_u32;
    let mut green_sum = 0_u32;
    let mut blue_sum = 0_u32;
    let mut red_only_entries = 0usize;
    let mut max_luma = 0_u8;
    for color in colors.iter().copied() {
        if color != 0 {
            nonzero_entries = nonzero_entries.saturating_add(1);
        }
        let red = color & 0x001f;
        let green = (color >> 5) & 0x001f;
        let blue = (color >> 10) & 0x001f;
        red_sum = red_sum.saturating_add(u32::from(red));
        green_sum = green_sum.saturating_add(u32::from(green));
        blue_sum = blue_sum.saturating_add(u32::from(blue));
        if color != 0 && red >= 8 && green <= 1 && blue == 0 {
            red_only_entries = red_only_entries.saturating_add(1);
        }
        let luma = rgb555_luma(color);
        luma_sum = luma_sum.saturating_add(u32::from(luma));
        max_luma = max_luma.max(luma);
        if !unique[..unique_entries].contains(&color) {
            unique[unique_entries] = color;
            unique_entries = unique_entries.saturating_add(1);
        }
    }

    PaletteRegionStats {
        nonzero_entries,
        unique_entries,
        luma_sum,
        red_sum,
        green_sum,
        blue_sum,
        red_only_entries,
        entries: colors.len(),
        max_luma,
    }
}

fn fallback_br2_4bpp_palette_sample(
    raw_pixels: &[u16],
    texture_page: u16,
    clut: u16,
    index: i32,
) -> Option<LinearPaletteSample> {
    let candidate = fallback_br2_4bpp_palette_candidate(raw_pixels, texture_page, clut)?;

    Some(LinearPaletteSample {
        color: raw_pixel_from(raw_pixels, candidate.x + index, candidate.y),
        nonzero_entries: candidate.nonzero_entries,
        unique_entries: candidate.unique_entries,
    })
}

fn fallback_br2_4bpp_palette_candidate(
    raw_pixels: &[u16],
    texture_page: u16,
    clut: u16,
) -> Option<LinearPaletteCandidate> {
    if let Some(candidate) = br2_character_model_palette_candidate(raw_pixels, texture_page, clut) {
        return Some(candidate);
    }

    if !br2_stage_low_color_palette_needs_base_row(texture_page, clut) {
        return None;
    }

    let clut_x = ((clut & 0x3f) as i32) * 16;
    let candidate_y = (clut_y(clut) as i32) & !0x1f;
    let stats = palette_region_stats(raw_pixels, clut_x, candidate_y, 16)?;
    if stats.nonzero_entries < 12 || stats.unique_entries < 8 {
        return None;
    }

    Some(LinearPaletteCandidate {
        x: clut_x,
        y: candidate_y,
        nonzero_entries: stats.nonzero_entries,
        unique_entries: stats.unique_entries,
    })
}

fn br2_character_model_palette_candidate(
    raw_pixels: &[u16],
    texture_page: u16,
    clut: u16,
) -> Option<LinearPaletteCandidate> {
    if native_disable_br2_character_model_palette_alias() {
        return None;
    }
    br2_character_model_palette_candidate_with_policy(raw_pixels, texture_page, clut, true)
}

fn br2_character_model_palette_candidate_with_policy(
    raw_pixels: &[u16],
    texture_page: u16,
    clut: u16,
    enabled: bool,
) -> Option<LinearPaletteCandidate> {
    br2_character_model_palette_candidate_with_override(
        raw_pixels,
        texture_page,
        clut,
        enabled,
        native_br2_character_palette_x_override(),
    )
}

fn br2_character_model_palette_candidate_with_override(
    raw_pixels: &[u16],
    texture_page: u16,
    clut: u16,
    enabled: bool,
    preferred_x_override: Option<i32>,
) -> Option<LinearPaletteCandidate> {
    if !enabled {
        return None;
    }
    let requested_x = ((clut & 0x3f) as i32) * 16;
    let requested_y = clut_y(clut) as i32;
    if let Some(candidate_x) = preferred_x_override
        && candidate_x != requested_x
        && (br2_low_bank_gameplay_character_texture_descriptor(texture_page, clut)
            || br2_character_model_texture_descriptor(texture_page, clut))
    {
        let stats = palette_region_stats(raw_pixels, candidate_x, requested_y, 16)?;
        if stats.nonzero_entries >= 12 && stats.unique_entries >= 8 {
            return Some(LinearPaletteCandidate {
                x: candidate_x,
                y: requested_y,
                nonzero_entries: stats.nonzero_entries,
                unique_entries: stats.unique_entries,
            });
        }
    }
    let (x, y) = if br2_low_bank_gameplay_character_texture_descriptor(texture_page, clut) {
        let requested_stats = palette_region_stats(raw_pixels, requested_x, requested_y, 16)?;
        if let Some(candidate_x) = preferred_x_override
            && candidate_x != requested_x
        {
            let stats = palette_region_stats(raw_pixels, candidate_x, requested_y, 16)?;
            if stats.nonzero_entries >= 12 && stats.unique_entries >= 8 {
                return Some(LinearPaletteCandidate {
                    x: candidate_x,
                    y: requested_y,
                    nonzero_entries: stats.nonzero_entries,
                    unique_entries: stats.unique_entries,
                });
            }
        }
        let requested_is_rich =
            requested_stats.nonzero_entries >= 12 && requested_stats.unique_entries >= 8;
        let requested_is_implausibly_dark =
            requested_is_rich && requested_stats.is_implausibly_dark_texture_row();
        let requested_is_red_polluted = requested_stats.is_low_bank_red_polluted();
        if requested_is_rich && !requested_is_implausibly_dark && !requested_is_red_polluted {
            return None;
        }
        if requested_is_implausibly_dark || requested_is_red_polluted {
            let preferred_x = preferred_x_override.unwrap_or(32);
            let previous_y = requested_y.saturating_sub(16);
            let base_y = requested_y & !0x0f;
            let yugo_previous_material_row = texture_page_without_dither(texture_page) == 0x0008
                && clut == 0x7a00
                && preferred_x_override.is_none();
            let candidates = [
                (requested_x, previous_y),
                (preferred_x, previous_y),
                (requested_x.saturating_sub(16), previous_y),
                (requested_x.saturating_add(16), previous_y),
                (requested_x, base_y),
                (preferred_x, base_y),
            ];
            if let Some(candidate) =
                candidates
                    .into_iter()
                    .find_map(|(candidate_x, candidate_y)| {
                        if (candidate_x, candidate_y) == (requested_x, requested_y) {
                            return None;
                        }
                        let stats = palette_region_stats(raw_pixels, candidate_x, candidate_y, 16)?;
                        let candidate_luma = stats.average_luma();
                        let captured_yugo_previous = yugo_previous_material_row
                            && (candidate_x, candidate_y) == (requested_x, previous_y);
                        let minimum_unique_entries = if captured_yugo_previous { 2 } else { 3 };
                        let minimum_luma = if captured_yugo_previous { 4 } else { 6 };
                        (stats.nonzero_entries >= 12
                            && stats.unique_entries >= minimum_unique_entries
                            && !stats.is_low_bank_red_polluted()
                            && candidate_luma >= minimum_luma
                            && candidate_luma >= requested_stats.average_luma().saturating_add(2))
                        .then_some(LinearPaletteCandidate {
                            x: candidate_x,
                            y: candidate_y,
                            nonzero_entries: stats.nonzero_entries,
                            unique_entries: stats.unique_entries,
                        })
                    })
            {
                return Some(candidate);
            }
        }
        // Keep the runtime-selected bank first so real VRAM captures can
        // validate character palette placement without recompiling.
        let preferred_x = preferred_x_override.unwrap_or(32);
        let candidate_xs = if requested_x == 0 {
            [requested_x + 16, preferred_x, requested_x]
        } else {
            [
                preferred_x,
                requested_x.saturating_sub(16),
                requested_x.saturating_add(16),
            ]
        };
        let candidate = candidate_xs.into_iter().find_map(|candidate_x| {
            if candidate_x == requested_x {
                return None;
            }
            let stats = palette_region_stats(raw_pixels, candidate_x, requested_y, 16)?;
            (stats.nonzero_entries >= 12
                && stats.unique_entries >= 8
                && !stats.is_low_bank_red_polluted())
            .then_some(LinearPaletteCandidate {
                x: candidate_x,
                y: requested_y,
                nonzero_entries: stats.nonzero_entries,
                unique_entries: stats.unique_entries,
            })
        })?;
        return Some(candidate);
    } else if !br2_character_model_texture_descriptor(texture_page, clut) {
        return None;
    } else {
        // Match captures for the y=486/y=490 model descriptors point at an
        // interleaved texture-data row while the coherent 16-color material
        // palette is uploaded at the start of the same 16-row CLUT block or in
        // the adjacent 16-word row. A populated requested row is not
        // authoritative for these descriptors: the live 0x7a9a capture has 15
        // unrelated colors at x=416 and the actual grayscale material ramp at
        // x=432.
        if matches!(requested_y, 486 | 490) {
            let base_y = requested_y & !0x0f;
            let base_stats = palette_region_stats(raw_pixels, requested_x, base_y, 16)?;
            if base_stats.nonzero_entries >= 12 && base_stats.unique_entries >= 8 {
                return Some(LinearPaletteCandidate {
                    x: requested_x,
                    y: base_y,
                    nonzero_entries: base_stats.nonzero_entries,
                    unique_entries: base_stats.unique_entries,
                });
            }
        }
        // Captured BR2 model draws reference the adjacent 16-word CLUT upload,
        // not a later row. Most model rows place it to the right; row 487 is
        // the paired upload immediately to the left.
        let alias_x = if requested_y == 487 {
            requested_x.saturating_sub(16)
        } else {
            requested_x.saturating_add(16)
        };
        (alias_x, requested_y)
    };
    let stats = palette_region_stats(raw_pixels, x, y, 16)?;
    // Reject texture/clear rows that only happen to be populated. Captured BR2
    // frames place near-uniform white and dark-red rows at some of these
    // offsets; treating them as CLUTs turns the recovered character mesh into
    // large single-color polygons.
    if stats.nonzero_entries < 12 || stats.unique_entries < 8 {
        return None;
    }

    Some(LinearPaletteCandidate {
        x,
        y,
        nonzero_entries: stats.nonzero_entries,
        unique_entries: stats.unique_entries,
    })
}

fn br2_character_model_texture_descriptor(texture_page: u16, clut: u16) -> bool {
    texture_page_without_dither(texture_page) == 0x0039
        && matches!(clut_y(clut), 486 | 487 | 489 | 490)
}

fn br2_low_bank_gameplay_character_texture_descriptor(texture_page: u16, clut: u16) -> bool {
    let texture_page = texture_page_without_dither(texture_page);
    texture_page_color_mode(texture_page) == 0
        && (0x0008..=0x000e).contains(&texture_page)
        && (480..512).contains(&clut_y(clut))
        && ((clut & 0x3f) as usize + 2) * 16 <= VRAM_WIDTH
}

fn br2_gameplay_body_missing_palette_descriptor(texture_page: u16, clut: u16) -> bool {
    let descriptor = texture_page_without_dither(texture_page);
    matches!(descriptor, 0x000c | 0x000d)
        && ((clut & 0x3f) as i32) * 16 == 64
        && (480..=486).contains(&clut_y(clut))
}

fn texture_page_without_dither(texture_page: u16) -> u16 {
    texture_page & !0x0200
}

fn texture_page_polygon_descriptor(texture_page: u16) -> u16 {
    texture_page & !0x0600
}

fn br2_texture_descriptor_alias(texture_page: u16, clut: u16) -> (u16, u16) {
    if native_disable_br2_texture_descriptor_alias() {
        return (texture_page, clut);
    }
    // BR2's recovered gameplay packet stream retains the stage descriptor
    // used before the final upload relocation. The live stage atlas is stored
    // at page 0x001c with CLUT 0x7850; sampling 0x0039/0x7859 reads the stale
    // title/object atlas and produces the neon-corrupted background.
    if texture_page_polygon_descriptor(texture_page) == 0x0039 && clut == 0x7859 {
        (0x001c, 0x7850)
    } else if texture_page_polygon_descriptor(texture_page) == 0x001c
        && !(texture_page == 0x021c && clut == 0x79d4)
        && (0x11..=0x17).contains(&(clut & 0x3f))
        && (480..=490).contains(&clut_y(clut))
    {
        // Captured stage packets retain material-specific CLUT columns at
        // x=272..368, but the final per-lighting palettes are uploaded down
        // the x=256 column. The stale columns decode the same coherent atlas
        // with the neon corruption visible across the upper playfield.
        (texture_page, (clut & !0x003f) | 0x0010)
    } else {
        (texture_page, clut)
    }
}

fn br2_stage_low_color_palette_needs_base_row(texture_page: u16, clut: u16) -> bool {
    texture_page_color_mode(texture_page) == 0 && texture_page == 0x001a && clut == 0x7ede
}

fn should_use_br2_4bpp_palette_sample(
    texture_page: u16,
    clut: u16,
    allow_palette_fallback: bool,
    requested_stats: PaletteRegionStats,
) -> bool {
    if !allow_palette_fallback || texture_page_color_mode(texture_page) != 0 {
        return false;
    }

    if native_br2_character_palette_x_override().is_some()
        && br2_low_bank_gameplay_character_texture_descriptor(texture_page, clut)
    {
        return true;
    }
    // BR2 model descriptors address texture data where a conventional PSX CLUT
    // would live. A populated requested row is therefore not evidence that it
    // is a palette; captured model uploads use the validated alias candidate.
    br2_character_model_texture_descriptor(texture_page, clut)
        || (br2_low_bank_gameplay_character_texture_descriptor(texture_page, clut)
            && requested_stats.nonzero_entries >= 12
            && requested_stats.unique_entries >= 8
            && (requested_stats.is_implausibly_dark_texture_row()
                || requested_stats.is_low_bank_red_polluted()))
        || requested_stats.nonzero_entries < 12
        || requested_stats.unique_entries < 8
}

fn rgb555_luma(color: u16) -> u8 {
    let red = u32::from(color & 0x001f);
    let green = u32::from((color >> 5) & 0x001f);
    let blue = u32::from((color >> 10) & 0x001f);
    ((red * 3 + green * 6 + blue) / 10) as u8
}

fn fallback_zn_4bpp_palette_sample(
    raw_pixels: &[u16],
    texture_page: u16,
    clut: u16,
    index: i32,
) -> Option<LinearPaletteSample> {
    let candidate = fallback_zn_4bpp_palette_candidate(raw_pixels, texture_page, clut)?;

    Some(LinearPaletteSample {
        color: raw_pixel_from(raw_pixels, candidate.x + index, candidate.y),
        nonzero_entries: candidate.nonzero_entries,
        unique_entries: candidate.unique_entries,
    })
}

fn fallback_zn_4bpp_palette_candidate(
    raw_pixels: &[u16],
    texture_page: u16,
    clut: u16,
) -> Option<LinearPaletteCandidate> {
    let br2_gameplay_palette = texture_page == 0x002e && clut == 0x7b9e;
    if texture_page_color_mode(texture_page) != 0
        || (!br2_gameplay_palette && texture_page & 0x0200 == 0 && texture_page & 0x2000 == 0)
    {
        return None;
    }

    let clut_x = ((clut & 0x3f) as i32) * 16;
    let clut_y = clut_y(clut) as i32;
    if clut_y < 480 {
        return None;
    }

    for y in [
        clut_y & !0x0f,
        clut_y & !0x1f,
        clut_y.saturating_sub(16),
        clut_y.saturating_add(16),
    ] {
        if y == clut_y {
            continue;
        }
        let Some(stats) = palette_region_stats(raw_pixels, clut_x, y, 16) else {
            continue;
        };
        if stats.nonzero_entries < 12 || stats.unique_entries < 8 {
            continue;
        }
        return Some(LinearPaletteCandidate {
            x: clut_x,
            y,
            nonzero_entries: stats.nonzero_entries,
            unique_entries: stats.unique_entries,
        });
    }

    None
}

fn should_use_zn_4bpp_palette_sample(
    texture_page: u16,
    clut: u16,
    allow_palette_fallback: bool,
    requested_nonzero_entries: usize,
    requested_unique_entries: usize,
    fallback_nonzero_entries: usize,
    fallback_unique_entries: usize,
) -> bool {
    if !allow_palette_fallback || texture_page_color_mode(texture_page) != 0 {
        return false;
    }

    let clut_y = clut_y(clut) as i32;
    let br2_gameplay_palette = texture_page == 0x002e && clut == 0x7b9e;
    if clut_y < 480
        || (!br2_gameplay_palette && texture_page & 0x0200 == 0 && texture_page & 0x2000 == 0)
    {
        return false;
    }

    // ZN title/character pages sometimes point 4bpp sprites at a sparse row
    // inside a 16-row CLUT upload block. Do not override valid 15/16-color
    // rows; only recover visibly sparse rows like 0x7c80 used by BR2 portraits.
    (requested_nonzero_entries <= 8 || requested_unique_entries <= 6)
        && fallback_nonzero_entries >= 12
        && fallback_unique_entries >= 8
}

fn fallback_palette_raw_pixel_from(
    raw_pixels: &[u16],
    texture_page: u16,
    clut: u16,
    index: i32,
    entries: usize,
) -> Option<u16> {
    if entries == 256 {
        return fallback_tiled_256_palette_raw_pixel_from(raw_pixels, texture_page, clut, index);
    }

    let clut_x = ((clut & 0x3f) as i32) * 16;
    let clut_y = clut_y(clut) as i32;
    let offsets = fallback_palette_alias_offsets(entries);

    offsets.iter().find_map(|(dx, dy)| {
        let x = clut_x + dx;
        let y = clut_y + dy;
        if x < 0 || y < 0 || x + entries as i32 > VRAM_WIDTH as i32 || y >= VRAM_HEIGHT as i32 {
            return None;
        }
        let nonzero_entries = (0..entries as i32)
            .filter(|offset| raw_pixel_from(raw_pixels, x + offset, y) != 0)
            .count();
        if nonzero_entries < fallback_palette_min_nonzero_entries(entries) {
            return None;
        }
        let color = raw_pixel_from(raw_pixels, x + index, y);
        (color != 0).then_some(color)
    })
}

fn fallback_tiled_256_palette_raw_pixel_from(
    raw_pixels: &[u16],
    texture_page: u16,
    clut: u16,
    index: i32,
) -> Option<u16> {
    fallback_tiled_256_palette_sample(raw_pixels, texture_page, clut, index).and_then(|sample| {
        if sample.color != 0 {
            Some(sample.color)
        } else {
            None
        }
    })
}

fn fallback_linear_256_palette_sample(
    raw_pixels: &[u16],
    texture_page: u16,
    clut: u16,
    index: i32,
) -> Option<LinearPaletteSample> {
    let candidate = fallback_linear_256_palette_candidate(raw_pixels, texture_page, clut)?;
    Some(LinearPaletteSample {
        color: raw_pixel_from(raw_pixels, candidate.x + index, candidate.y),
        nonzero_entries: candidate.nonzero_entries,
        unique_entries: candidate.unique_entries,
    })
}

fn fallback_linear_256_palette_candidate(
    raw_pixels: &[u16],
    texture_page: u16,
    clut: u16,
) -> Option<LinearPaletteCandidate> {
    if let Some(candidate) =
        br2_character_select_256_palette_candidate(raw_pixels, texture_page, clut)
    {
        return Some(candidate);
    }

    let clut_x = ((clut & 0x3f) as i32) * 16;
    let clut_y = clut_y(clut) as i32;
    let candidates = [
        clut_y & !0x1f,
        clut_y & !0x0f,
        clut_y.saturating_sub(16),
        clut_y.saturating_add(16),
    ];

    let mut best = None;
    for y in candidates {
        if y == clut_y {
            continue;
        }
        let Some(stats) = palette_region_stats(raw_pixels, clut_x, y, 256) else {
            continue;
        };
        if stats.nonzero_entries < 128 || stats.unique_entries < 32 {
            continue;
        }
        let candidate = LinearPaletteCandidate {
            x: clut_x,
            y,
            nonzero_entries: stats.nonzero_entries,
            unique_entries: stats.unique_entries,
        };
        best = prefer_linear_256_palette_candidate(best, candidate);
    }

    best
}

fn br2_character_select_256_palette_candidate(
    raw_pixels: &[u16],
    texture_page: u16,
    clut: u16,
) -> Option<LinearPaletteCandidate> {
    let descriptor = texture_page_without_dither(texture_page);
    if !matches!(
        (descriptor, clut),
        (0x0088, 0x7d40) | (0x0088, 0x7d80) | (0x008a, 0x7d80)
    ) {
        return None;
    }

    let clut_x = ((clut & 0x3f) as i32) * 16;
    let clut_y = clut_y(clut) as i32;
    let y = clut_y.saturating_sub(16);
    if !(480..496).contains(&y) {
        return None;
    }

    let stats = palette_region_stats(raw_pixels, clut_x, y, 256)?;
    if stats.nonzero_entries < 128 || stats.unique_entries < 32 {
        return None;
    }

    Some(LinearPaletteCandidate {
        x: clut_x,
        y,
        nonzero_entries: stats.nonzero_entries,
        unique_entries: stats.unique_entries,
    })
}

fn prefer_linear_256_palette_candidate(
    best: Option<LinearPaletteCandidate>,
    candidate: LinearPaletteCandidate,
) -> Option<LinearPaletteCandidate> {
    let Some(best) = best else {
        return Some(candidate);
    };

    if candidate.nonzero_entries > best.nonzero_entries {
        return Some(candidate);
    }
    if candidate.nonzero_entries == best.nonzero_entries && candidate.y < best.y {
        return Some(candidate);
    }
    Some(best)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TiledPaletteSample {
    color: u16,
    nonzero_entries: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TiledPaletteCandidate {
    x: i32,
    y: i32,
    nonzero_entries: usize,
}

const TILED_256_PALETTE_SWITCH_NONZERO_MARGIN: usize = 96;

fn fallback_tiled_256_palette_sample(
    raw_pixels: &[u16],
    texture_page: u16,
    clut: u16,
    index: i32,
) -> Option<TiledPaletteSample> {
    let row = index / 16;
    let col = index & 0x0f;
    let candidate = fallback_tiled_256_palette_candidate(raw_pixels, texture_page, clut)?;

    Some(TiledPaletteSample {
        color: raw_pixel_from(raw_pixels, candidate.x + col, candidate.y + row),
        nonzero_entries: candidate.nonzero_entries,
    })
}

fn fallback_tiled_256_palette_candidate(
    raw_pixels: &[u16],
    texture_page: u16,
    clut: u16,
) -> Option<TiledPaletteCandidate> {
    let clut_x = ((clut & 0x3f) as i32) * 16;
    let clut_y = clut_y(clut) as i32;
    let override_x = native_clut_tile_x_override();
    let preferred_x = override_x.unwrap_or_else(|| preferred_tiled_256_palette_x(texture_page));
    let preferred_actual_x = if (384..512).contains(&clut_x) {
        preferred_x
    } else {
        clut_x + preferred_x
    };
    let lock_preferred_x =
        override_x.is_none() && locks_preferred_tiled_256_palette_x(texture_page);
    let x_candidates = tiled_256_palette_x_candidates(clut_x, preferred_x, override_x);
    let y_candidates = [
        clut_y & !0x1f,
        clut_y & !0x0f,
        clut_y.saturating_sub(16) & !0x0f,
    ];

    let mut best = None;
    let mut best_preferred_x = None;
    for y in y_candidates {
        for x in x_candidates.iter().copied() {
            if x < 0 || y < 0 || x + 16 > VRAM_WIDTH as i32 || y + 16 > VRAM_HEIGHT as i32 {
                continue;
            }
            let nonzero_entries = (0..16)
                .flat_map(|tile_y| (0..16).map(move |tile_x| (tile_x, tile_y)))
                .filter(|(tile_x, tile_y)| raw_pixel_from(raw_pixels, x + tile_x, y + tile_y) != 0)
                .count();
            if nonzero_entries < 64 {
                continue;
            }

            let candidate = TiledPaletteCandidate {
                x,
                y,
                nonzero_entries,
            };
            best = prefer_tiled_256_palette_candidate(best, candidate);
            if x == preferred_actual_x {
                best_preferred_x = if lock_preferred_x {
                    prefer_locked_tiled_256_palette_candidate(best_preferred_x, candidate)
                } else {
                    prefer_tiled_256_palette_candidate(best_preferred_x, candidate)
                };
            }
        }
    }
    if lock_preferred_x && best_preferred_x.is_some() {
        return best_preferred_x;
    }
    best
}

fn tiled_256_palette_x_candidates(
    clut_x: i32,
    preferred_x: i32,
    override_x: Option<i32>,
) -> Vec<i32> {
    if let Some(override_x) = override_x {
        return vec![clut_x + override_x];
    }

    let mut candidates = Vec::with_capacity(12);

    if (384..512).contains(&clut_x) {
        // Some ZN commands already point CLUT x into the tiled palette bank.
        // Treat the bank coordinates as absolute; adding 384 again selects
        // unrelated texture art around x=800 and produces striped playfields.
        for x in [
            clut_x,
            preferred_x,
            clut_x - 16,
            clut_x + 16,
            384,
            400,
            416,
            432,
            448,
            464,
            480,
            496,
        ] {
            push_unique_i32(&mut candidates, x);
        }
    } else {
        for dx in [preferred_x, 0, 384, 400, 416, 432, 448, 464, 480, 496] {
            push_unique_i32(&mut candidates, clut_x + dx);
        }
    }

    candidates
}

fn push_unique_i32(values: &mut Vec<i32>, value: i32) {
    if !values.contains(&value) {
        values.push(value);
    }
}

fn push_unique_usize(values: &mut Vec<usize>, value: usize) {
    if !values.contains(&value) {
        values.push(value);
    }
}

fn prefer_locked_tiled_256_palette_candidate(
    best: Option<TiledPaletteCandidate>,
    candidate: TiledPaletteCandidate,
) -> Option<TiledPaletteCandidate> {
    let Some(best) = best else {
        return Some(candidate);
    };

    // ZN title/intro pages upload 256-color palettes as explicit 16x16 tiles.
    // When the requested CLUT row lands inside a 32-row block, the block base is
    // the stable tile origin; the next 16-row band often contains unrelated art
    // with more nonzero pixels and used to win by density alone.
    if candidate.y < best.y {
        return Some(candidate);
    }

    if candidate.y == best.y
        && candidate.nonzero_entries
            >= best
                .nonzero_entries
                .saturating_add(TILED_256_PALETTE_SWITCH_NONZERO_MARGIN)
    {
        return Some(candidate);
    }

    Some(best)
}

fn prefer_tiled_256_palette_candidate(
    best: Option<TiledPaletteCandidate>,
    candidate: TiledPaletteCandidate,
) -> Option<TiledPaletteCandidate> {
    let Some(best) = best else {
        return Some(candidate);
    };

    if candidate.nonzero_entries
        >= best
            .nonzero_entries
            .saturating_add(TILED_256_PALETTE_SWITCH_NONZERO_MARGIN)
    {
        Some(candidate)
    } else {
        Some(best)
    }
}

fn should_prefer_tiled_256_palette(
    requested_nonzero_entries: usize,
    tiled_nonzero_entries: usize,
    requested_color: u16,
) -> bool {
    if tiled_nonzero_entries < 64 {
        return false;
    }
    if requested_nonzero_entries == 0 {
        return true;
    }

    let sparse_requested = requested_nonzero_entries <= 32;
    let much_richer_tile =
        tiled_nonzero_entries >= requested_nonzero_entries.saturating_mul(2).max(64);

    (requested_color == 0 || sparse_requested) && much_richer_tile
}

fn should_use_linear_256_palette_sample(
    texture_page: u16,
    clut: u16,
    allow_palette_fallback: bool,
    requested_nonzero_entries: usize,
    linear_nonzero_entries: usize,
    requested_color: u16,
) -> bool {
    if texture_page_color_mode(texture_page) != 1 || linear_nonzero_entries < 128 {
        return false;
    }

    let clut_y = clut_y(clut);
    if !(480..512).contains(&clut_y) {
        return false;
    }

    allow_palette_fallback
        && should_prefer_tiled_256_palette(
            requested_nonzero_entries,
            linear_nonzero_entries,
            requested_color,
        )
}

fn should_use_tiled_256_palette_sample(
    texture_page: u16,
    clut: u16,
    allow_palette_fallback: bool,
    requested_nonzero_entries: usize,
    tiled_nonzero_entries: usize,
    requested_color: u16,
) -> bool {
    if native_force_tiled_256_palette() {
        return true;
    }

    if should_force_zn_tiled_256_palette(
        texture_page,
        clut,
        requested_nonzero_entries,
        tiled_nonzero_entries,
        requested_color,
    ) {
        return true;
    }

    if !allow_palette_fallback {
        return false;
    }

    if should_prefer_tiled_256_palette(
        requested_nonzero_entries,
        tiled_nonzero_entries,
        requested_color,
    ) {
        return true;
    }

    false
}

fn should_force_zn_tiled_256_palette(
    texture_page: u16,
    clut: u16,
    requested_nonzero_entries: usize,
    tiled_nonzero_entries: usize,
    requested_color: u16,
) -> bool {
    let texture_mode = (texture_page >> 7) & 0x03;
    if texture_mode != 1 {
        return false;
    }

    if texture_page & 0x0200 == 0 && texture_page & 0x2000 == 0 {
        return false;
    }

    let clut_y = clut_y(clut);
    if !(480..512).contains(&clut_y) {
        return false;
    }

    if texture_page & 0x2000 != 0
        && texture_page_uses_zn_extended_origin(texture_page)
        && requested_color == 0
        && requested_nonzero_entries <= 32
    {
        return tiled_nonzero_entries >= 128;
    }

    false
}

fn preferred_tiled_256_palette_x(texture_page: u16) -> i32 {
    let texture_mode = (texture_page >> 7) & 0x03;
    if texture_mode == 1 && texture_page & 0x0200 != 0 && texture_page & 0x0010 != 0 {
        let low_bank_page = ((texture_page & 0x000e) as i32 - 8).max(0) / 2;
        return 384 + low_bank_page * 16;
    }
    384
}

fn locks_preferred_tiled_256_palette_x(texture_page: u16) -> bool {
    let texture_mode = (texture_page >> 7) & 0x03;
    texture_mode == 1 && texture_page & 0x0200 != 0 && texture_page & 0x0010 != 0
}

fn native_clut_tile_x_override() -> Option<i32> {
    static OVERRIDE: OnceLock<Option<i32>> = OnceLock::new();
    *OVERRIDE.get_or_init(|| {
        std::env::var("BLOODYROAR2_NATIVE_CLUT_TILE_X")
            .ok()
            .and_then(|value| value.parse::<i32>().ok())
    })
}

fn native_force_tiled_256_palette() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("BLOODYROAR2_NATIVE_FORCE_TILED_256_PALETTE")
            .is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
    })
}

fn native_force_y16_base_256_palette() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("BLOODYROAR2_NATIVE_FORCE_Y16_BASE_256_PALETTE")
            .is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
    })
}

fn fallback_palette_alias_offsets(entries: usize) -> &'static [(i32, i32)] {
    match entries {
        16 => &[
            (0, -16),
            (16, 0),
            (-16, 0),
            (0, 16),
            (16, -16),
            (-16, -16),
            (16, 16),
            (-16, 16),
        ],
        256 => &[],
        _ => &[],
    }
}

fn fallback_palette_min_nonzero_entries(entries: usize) -> usize {
    match entries {
        16 => entries.saturating_sub(1),
        256 => 16,
        _ => usize::MAX,
    }
}

fn texture_page_raw_bounds_for_clut(texture_page: u16, clut: u16) -> (i32, i32, i32, i32) {
    let (page_x, page_y) = texture_page_origin_for_clut(texture_page, clut);
    let raw_width = texture_page_raw_width(texture_page) as i32;
    (page_x, page_y, page_x + raw_width - 1, page_y + 256 - 1)
}

fn texture_page_raw_width(texture_page: u16) -> usize {
    match texture_page_color_mode(texture_page) {
        0 => 64,
        1 => 128,
        _ => 256,
    }
}

fn texture_palette_raw_bounds(texture_page: u16, clut: u16) -> Option<(i32, i32, i32, i32)> {
    let entries = texture_palette_entries(texture_page);
    if entries == 0 {
        return None;
    }
    let left = ((clut & 0x3f) as i32) * 16;
    let top = clut_y(clut) as i32;
    Some((left, top, left + entries as i32 - 1, top))
}

fn recovery_palette_fallback_dependency_bounds(
    texture_page: u16,
    clut: u16,
) -> (i32, i32, i32, i32) {
    recovery_palette_fallback_dependency_bounds_with_override(
        texture_page,
        clut,
        native_br2_character_palette_x_override(),
    )
}

fn recovery_palette_fallback_dependency_bounds_with_override(
    texture_page: u16,
    clut: u16,
    character_palette_x_override: Option<i32>,
) -> (i32, i32, i32, i32) {
    let clut_x = ((clut & 0x3f) as i32) * 16;
    let clut_y = clut_y(clut) as i32;
    let entries = texture_palette_entries(texture_page);

    if entries == 16 {
        // Every 4bpp fallback path reads only the requested CLUT column, its
        // immediate neighbors, and nearby 16/32-row aliases. Low-bank BR2
        // fighters may additionally use the shared x=32 material bank.
        let character_descriptor =
            br2_low_bank_gameplay_character_texture_descriptor(texture_page, clut)
                || br2_character_model_texture_descriptor(texture_page, clut);
        let preferred_x = if character_descriptor {
            character_palette_x_override.unwrap_or_else(|| {
                if br2_low_bank_gameplay_character_texture_descriptor(texture_page, clut) {
                    32
                } else {
                    clut_x
                }
            })
        } else {
            clut_x
        };
        let left = clut_x.saturating_sub(16).min(preferred_x);
        let right = clut_x
            .saturating_add(31)
            .max(preferred_x.saturating_add(15));
        let top = clut_y.saturating_sub(16).min(clut_y & !0x1f);
        let bottom = clut_y.saturating_add(16).max(clut_y & !0x0f);
        return (left, top, right, bottom);
    }

    // 8bpp recovery probes several tiled palette banks across the CLUT upload
    // row. Preserve that wider dependency until those candidates are indexed.
    let band_top = (clut_y & !0x1f).saturating_sub(16);
    let band_bottom = (clut_y | 0x1f).saturating_add(16);
    (0, band_top, VRAM_WIDTH as i32 - 1, band_bottom)
}

fn texture_page_origin(texture_page: u16) -> (i32, i32) {
    if texture_page_uses_zn_low_bank_y0_alias(texture_page) {
        return (
            low_bank_y0_alias_page_x(texture_page, native_low_bank_y0_alias_preserves_odd_x()),
            0,
        );
    }

    if texture_page_uses_zn_4bpp_title_y0_alias(texture_page) {
        return (((texture_page & 0x0f) as i32) * 64, 0);
    }

    if native_force_4bpp_y0_alias() {
        let texture_mode = (texture_page >> 7) & 0x03;
        if texture_mode == 0 && texture_page & 0x0010 != 0 && texture_page & 0x0800 == 0 {
            return (((texture_page & 0x0f) as i32) * 64, 0);
        }
    }

    (
        ((texture_page & 0x0f) as i32) * 64,
        texture_page_y(texture_page) as i32,
    )
}

fn texture_page_origin_for_clut(texture_page: u16, clut: u16) -> (i32, i32) {
    let (page_x, page_y) = texture_page_origin(texture_page);
    if let Some(origin) = br2_gameplay_4bpp_texture_origin(texture_page, clut, page_x) {
        return origin;
    }
    if let Some(origin) = br2_character_select_8bpp_texture_origin(texture_page, clut, page_x) {
        return origin;
    }
    if let Some(alias_x) = br2_stage_y256_alias_x(texture_page, clut, page_x) {
        return (alias_x, PSX_VRAM_HEIGHT as i32 / 2);
    }
    (page_x, page_y)
}

fn texture_page_origin_for_sample(texture_page: u16, clut: u16, _u: u8, _v: u8) -> (i32, i32) {
    texture_page_origin_for_clut(texture_page, clut)
}

fn br2_gameplay_4bpp_texture_origin(
    texture_page: u16,
    clut: u16,
    page_x: i32,
) -> Option<(i32, i32)> {
    let descriptor = texture_page_without_dither(texture_page);
    let clut_x = ((clut & 0x3f) as i32) * 16;
    let clut_y = clut_y(clut);
    if descriptor == 0x000d && clut == 0x7c1d {
        // The left character-select portrait is uploaded to the paired high
        // VRAM bank, while recovered packets retain the low-bank descriptor.
        return Some((page_x, PSX_VRAM_HEIGHT as i32 / 2));
    }
    if descriptor == 0x003f && matches!(clut, 0x7818 | 0x7958) {
        // These live match descriptors reuse the title page bits after the
        // stage atlas has moved to the paired high VRAM bank.
        return Some((page_x, PSX_VRAM_HEIGHT as i32 / 2));
    }
    if descriptor == 0x0039 && clut == 0x785a {
        // Recovered Beast effect quads retain the title-page descriptor. The
        // y=0 atlas is opaque character art, while the live effect upload and
        // its transparent texels are in the paired high VRAM bank.
        return Some((page_x, PSX_VRAM_HEIGHT as i32 / 2));
    }
    if matches!(descriptor, 0x000c | 0x000d) && clut_x == 64 && (480..=486).contains(&clut_y) {
        // Gameplay body packets retain the low-bank page bits, while the
        // character atlas is uploaded to the paired y=256 bank. HUD packets
        // use page 0x000e and different CLUT columns, so keep those on y=0.
        return Some((page_x, PSX_VRAM_HEIGHT as i32 / 2));
    }
    if descriptor == 0x000b
        && clut == 0x7903
        && let Some(x) = native_br2_fighter_texture_x_override()
    {
        return Some((x, 0));
    }
    // BR2 reuses page 0x0039 for both stage and character material. Captured
    // model descriptors point at the high-bank upload; sampling the title
    // page at y=0 paints the character mesh with the full-screen title atlas.
    br2_character_model_texture_descriptor(texture_page, clut)
        .then_some((page_x, PSX_VRAM_HEIGHT as i32 / 2))
}

fn br2_character_select_8bpp_texture_origin(
    texture_page: u16,
    clut: u16,
    page_x: i32,
) -> Option<(i32, i32)> {
    if native_disable_br2_stage_y256_alias() {
        return None;
    }

    let descriptor = texture_page_without_dither(texture_page);
    let clut_x = ((clut & 0x3f) as i32) * 16;
    let clut_y = clut_y(clut);
    match (descriptor, clut) {
        // The right-side character-select portrait is uploaded as a 64-word
        // 8bpp strip in the high VRAM bank, while the GP0 draw references the
        // nominal low-bank texture page.
        (0x008a, 0x7d80) => Some((page_x, PSX_VRAM_HEIGHT as i32 / 2)),
        _ if descriptor == 0x009d && clut_x == 0 && (480..=490).contains(&clut_y) => {
            // Character-select cells retain the ZN low-bank 0x029d descriptor.
            // Its y=0 alias contains the font atlas, while the live portraits
            // and stage icons are uploaded at the paired y=256 origin.
            Some((page_x, PSX_VRAM_HEIGHT as i32 / 2))
        }
        _ => None,
    }
}

fn br2_stage_y256_alias_x(texture_page: u16, clut: u16, page_x: i32) -> Option<i32> {
    if native_disable_br2_stage_y256_alias() {
        return None;
    }

    let clut_x = ((clut & 0x3f) as i32) * 16;
    let clut_y = clut_y(clut);
    if texture_page_color_mode(texture_page) != 0 {
        return None;
    }

    match (texture_page, clut_x, clut_y) {
        // 0x000b/0x781b is used during the early stage transition and the
        // normal y=0 page contains the valid blue stage texture.  Keep the old
        // y=256 mirror available only for targeted diagnostics because it
        // exposes the broken texture fragments seen in native-play snapshots.
        (0x000b, 432, 480)
            if std::env::var_os("BLOODYROAR2_NATIVE_ENABLE_BR2_STAGE_000B_Y256_ALIAS")
                .is_some() =>
        {
            Some(page_x)
        }
        // Match-stage foreground/background strips are emitted with 0x000c
        // plus high CLUT rows 0x7b1f/0x7b5f. The nominal y=0 page is still the
        // title/HUD atlas; the live tree/stage art is uploaded in the y=256
        // mirror at the same raw page X.
        (0x000c, 496, 492 | 493) => Some(page_x),
        (0x000c, 384 | 400, 500) => Some(page_x - 64),
        // Gameplay character quads use the same low-bank mirror pattern for
        // 0x002e while the y=0 page contains HUD/title atlas data.
        (0x002e, 480, 494) => Some(page_x),
        // BR2 emits raw textured gameplay/effect quads on 0x002f after
        // entering play; the y=0 bank contains warning/title/HUD glyphs, while
        // the live character/effect atlas is the adjacent high-bank strip.
        // Captured gameplay UVs hit v=0..32, which are empty at page_x but
        // populated at page_x - 64.
        (0x002f, 496, 483 | 484) => Some(page_x - 64),
        _ => None,
    }
}

fn low_bank_y0_alias_page_x(texture_page: u16, preserve_odd_x: bool) -> i32 {
    let page_bits = if preserve_odd_x {
        texture_page & 0x0f
    } else {
        texture_page & 0x0e
    };
    page_bits as i32 * 64
}

fn texture_page_uses_zn_extended_origin(texture_page: u16) -> bool {
    texture_page & 0x2000 != 0 && texture_page & 0x0e00 != 0
}

fn texture_page_uses_zn_low_bank_y0_alias(texture_page: u16) -> bool {
    if native_disable_low_bank_y0_alias() {
        return false;
    }

    let texture_mode = (texture_page >> 7) & 0x03;
    texture_mode == 1
        && texture_page & 0x0200 != 0
        && texture_page & 0x0010 != 0
        && texture_page & 0x0800 == 0
}

fn texture_page_uses_zn_4bpp_title_y0_alias(texture_page: u16) -> bool {
    if native_disable_4bpp_title_y0_alias() {
        return false;
    }

    let texture_mode = (texture_page >> 7) & 0x03;
    texture_mode == 0 && matches!(texture_page, 0x0039 | 0x003f)
}

fn native_disable_low_bank_y0_alias() -> bool {
    static DISABLED: OnceLock<bool> = OnceLock::new();
    *DISABLED.get_or_init(|| {
        std::env::var("BLOODYROAR2_NATIVE_DISABLE_LOW_BANK_Y0_ALIAS")
            .is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
    })
}

fn native_disable_4bpp_title_y0_alias() -> bool {
    static DISABLED: OnceLock<bool> = OnceLock::new();
    *DISABLED.get_or_init(|| {
        std::env::var("BLOODYROAR2_NATIVE_DISABLE_4BPP_TITLE_Y0_ALIAS")
            .is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
    })
}

fn native_force_4bpp_y0_alias() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("BLOODYROAR2_NATIVE_FORCE_4BPP_Y0_ALIAS")
            .is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
    })
}

fn native_disable_br2_stage_y256_alias() -> bool {
    static DISABLED: OnceLock<bool> = OnceLock::new();
    *DISABLED.get_or_init(|| {
        std::env::var("BLOODYROAR2_NATIVE_DISABLE_BR2_STAGE_Y256_ALIAS")
            .is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
    })
}

fn native_disable_br2_character_model_palette_alias() -> bool {
    static DISABLED: OnceLock<bool> = OnceLock::new();
    *DISABLED.get_or_init(|| {
        std::env::var("BLOODYROAR2_NATIVE_DISABLE_BR2_CHARACTER_MODEL_PALETTE_ALIAS")
            .is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
    })
}

fn native_disable_recovery_raster_cache() -> bool {
    static DISABLED: OnceLock<bool> = OnceLock::new();
    *DISABLED.get_or_init(|| {
        std::env::var("BLOODYROAR2_NATIVE_DISABLE_RECOVERY_RASTER_CACHE")
            .is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
    })
}

fn native_br2_character_palette_x_override() -> Option<i32> {
    static OVERRIDE: OnceLock<Option<i32>> = OnceLock::new();
    *OVERRIDE.get_or_init(|| {
        std::env::var("BLOODYROAR2_NATIVE_BR2_CHARACTER_PALETTE_X")
            .ok()
            .and_then(|value| value.trim().parse::<i32>().ok())
            .filter(|x| *x >= 0 && *x + 16 <= VRAM_WIDTH as i32 && x.rem_euclid(16) == 0)
    })
}

fn native_br2_fighter_texture_x_override() -> Option<i32> {
    static OVERRIDE: OnceLock<Option<i32>> = OnceLock::new();
    *OVERRIDE.get_or_init(|| {
        std::env::var("BLOODYROAR2_NATIVE_BR2_FIGHTER_TEXTURE_X")
            .ok()
            .and_then(|value| value.trim().parse::<i32>().ok())
            .filter(|x| *x >= 0 && *x + 64 <= VRAM_WIDTH as i32 && x.rem_euclid(64) == 0)
    })
}

fn native_disable_br2_texture_descriptor_alias() -> bool {
    static DISABLED: OnceLock<bool> = OnceLock::new();
    *DISABLED.get_or_init(|| {
        std::env::var("BLOODYROAR2_NATIVE_DISABLE_BR2_TEXTURE_DESCRIPTOR_ALIAS")
            .is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
    })
}

fn native_low_bank_y0_alias_preserves_odd_x() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("BLOODYROAR2_NATIVE_LOW_BANK_Y0_ALIAS_PRESERVE_ODD_X").map_or(true, |value| {
            value != "0" && !value.eq_ignore_ascii_case("false")
        })
    })
}

fn bounds_overlap(a: (i32, i32, i32, i32), b: (i32, i32, i32, i32)) -> bool {
    let (a_left, a_top, a_right, a_bottom) = a;
    let (b_left, b_top, b_right, b_bottom) = b;
    a_left <= a_right
        && a_top <= a_bottom
        && b_left <= b_right
        && b_top <= b_bottom
        && a_left <= b_right
        && b_left <= a_right
        && a_top <= b_bottom
        && b_top <= a_bottom
}

fn texture_page_y(texture_page: u16) -> u16 {
    ((texture_page & 0x0010) << 4) | ((texture_page & 0x0800) >> 2)
}

fn texture_page_dimensions(_texture_page: u16) -> (usize, usize) {
    (256, 256)
}

fn texture_palette_entries(texture_page: u16) -> usize {
    match texture_page_color_mode(texture_page) {
        0 => 16,
        1 => 256,
        _ => 0,
    }
}

fn texture_page_color_mode(texture_page: u16) -> u16 {
    let mode = (texture_page >> 7) & 0x03;
    if native_force_texture_raw15() && mode == 1 {
        return 2;
    }
    mode
}

fn native_force_texture_raw15() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("BLOODYROAR2_NATIVE_FORCE_TEXTURE_RAW15")
            .is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
    })
}

fn native_swap_8bpp_texture_bytes() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("BLOODYROAR2_NATIVE_SWAP_8BPP_TEXTURE_BYTES")
            .is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
    })
}

fn native_palette_index_zero_transparent(texture_page: u16, color: u16) -> bool {
    if native_disable_palette_index_zero_transparent() {
        return false;
    }
    if native_force_palette_index_zero_transparent() {
        return true;
    }

    let texture_mode = (texture_page >> 7) & 0x03;
    color == 0 && texture_mode <= 1 && (texture_page & 0x0200 != 0 || texture_page & 0x2000 != 0)
}

fn br2_character_model_palette_index_zero_transparent(texture_page: u16, clut: u16) -> bool {
    br2_character_model_texture_descriptor(texture_page, clut)
}

fn native_force_palette_index_zero_transparent() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("BLOODYROAR2_NATIVE_FORCE_PALETTE_INDEX_ZERO_TRANSPARENT")
            .is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
    })
}

fn native_disable_palette_index_zero_transparent() -> bool {
    static DISABLED: OnceLock<bool> = OnceLock::new();
    *DISABLED.get_or_init(|| {
        std::env::var("BLOODYROAR2_NATIVE_DISABLE_PALETTE_INDEX_ZERO_TRANSPARENT")
            .is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
    })
}

fn modulate_channel(value_5bit: u16, primitive_8bit: u32) -> u16 {
    ((u32::from(value_5bit) * primitive_8bit) >> 7).min(0x1f) as u16
}

fn blend_rgb555(source: u16, destination: u16, mode: u8) -> u16 {
    let r = blend_channel(source & 0x1f, destination & 0x1f, mode);
    let g = blend_channel((source >> 5) & 0x1f, (destination >> 5) & 0x1f, mode);
    let b = blend_channel((source >> 10) & 0x1f, (destination >> 10) & 0x1f, mode);
    (source & 0x8000) | r | (g << 5) | (b << 10)
}

fn blend_channel(source: u16, destination: u16, mode: u8) -> u16 {
    let source = i32::from(source);
    let destination = i32::from(destination);
    let value = match mode & 0x03 {
        0 => destination / 2 + source / 2,
        1 => destination + source,
        2 => destination - source,
        _ => destination + source / 4,
    };
    value.clamp(0, 0x1f) as u16
}

fn rgb_luma(value: u32) -> u8 {
    let r = (value >> 16) & 0xff;
    let g = (value >> 8) & 0xff;
    let b = value & 0xff;
    ((77 * r + 150 * g + 29 * b) >> 8) as u8
}

pub fn bytes_base64(data: &[u8]) -> String {
    base64_encode(data)
}

fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        output.push(TABLE[(b0 >> 2) as usize] as char);
        output.push(TABLE[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            output.push(TABLE[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            output.push('=');
        }
        if chunk.len() > 2 {
            output.push(TABLE[(b2 & 0x3f) as usize] as char);
        } else {
            output.push('=');
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{
        NativeFrameBuffer, PSX_VRAM_HEIGHT, PixelWriteOptions, Point, PreparedTextureDrawResources,
        PreparedTextureSampler, RecoveryRasterCacheKey, TextureCoordinate, TextureDrawOptions,
        TextureSamplingPolicy, TextureWindow, TexturedDrawStats, TexturedPoint, VRAM_HEIGHT,
        VRAM_WIDTH, br2_character_model_palette_candidate_with_override,
        br2_character_model_palette_candidate_with_policy, br2_texture_descriptor_alias,
        fallback_tiled_256_palette_candidate, indexed_palette_sample_from,
        low_bank_y0_alias_page_x, palette_colors_stats, palette_region_stats,
        preferred_tiled_256_palette_x, prepared_indexed_palette_samples_from,
        prepared_indexed_palette_samples_with_history,
        recovery_palette_fallback_dependency_bounds_with_override, texture_page_origin,
        texture_page_origin_for_clut, texture_page_origin_for_sample,
        texture_page_raw_bounds_for_clut, tiled_256_palette_x_candidates,
    };

    const TEST_4BPP_TEXTURE_PAGE: u16 = 0x0000;
    const TEST_4BPP_CLUT: u16 = 300 << 6;
    const TEST_RAW15_TEXTURE_PAGE: u16 = 0x0100;

    fn textured_point(x: i32, y: i32, u: u8, v: u8) -> TexturedPoint {
        TexturedPoint {
            point: Point { x, y },
            u,
            v,
        }
    }

    fn write_4bpp_texel(framebuffer: &mut NativeFrameBuffer, u: i32, v: i32, index: u16) {
        let x = u / 4;
        let shift = ((u & 3) * 4) as u16;
        let mask = 0x000f_u16 << shift;
        let packed = (framebuffer.raw_pixel(x, v) & !mask) | ((index & 0x000f) << shift);
        framebuffer.set_raw_pixel(x, v, packed);
    }

    fn test_4bpp_framebuffer() -> NativeFrameBuffer {
        let mut framebuffer = NativeFrameBuffer::default();
        for index in 0..16 {
            framebuffer.set_raw_pixel(index, 300, 0x0421_u16.saturating_mul(index as u16));
        }
        for v in 0..32 {
            for u in 0..32 {
                write_4bpp_texel(&mut framebuffer, u, v, ((u + v) % 15 + 1) as u16);
            }
        }
        framebuffer
    }

    fn test_textured_quad_triangles() -> ([TexturedPoint; 3], [TexturedPoint; 3]) {
        (
            [
                textured_point(120, 120, 0, 0),
                textured_point(128, 120, 16, 0),
                textured_point(120, 128, 0, 16),
            ],
            [
                textured_point(128, 120, 16, 0),
                textured_point(120, 128, 0, 16),
                textured_point(128, 128, 16, 16),
            ],
        )
    }

    fn draw_textured_quad_as_separate_triangles(
        framebuffer: &mut NativeFrameBuffer,
        first: [TexturedPoint; 3],
        second: [TexturedPoint; 3],
        texture_page: u16,
        clut: u16,
        options: TextureDrawOptions,
        texture_window: TextureWindow,
    ) -> (TexturedDrawStats, TexturedDrawStats) {
        (
            framebuffer.draw_textured_triangle(
                first[0],
                first[1],
                first[2],
                texture_page,
                clut,
                options,
                texture_window,
            ),
            framebuffer.draw_textured_triangle(
                second[0],
                second[1],
                second[2],
                texture_page,
                clut,
                options,
                texture_window,
            ),
        )
    }

    fn draw_shaded_textured_quad_as_separate_triangles(
        framebuffer: &mut NativeFrameBuffer,
        first: [TexturedPoint; 3],
        first_colors: [u32; 3],
        second: [TexturedPoint; 3],
        second_colors: [u32; 3],
        texture_page: u16,
        clut: u16,
        options: TextureDrawOptions,
        texture_window: TextureWindow,
    ) -> (TexturedDrawStats, TexturedDrawStats) {
        (
            framebuffer.draw_shaded_textured_triangle(
                first[0],
                first[1],
                first[2],
                first_colors,
                texture_page,
                clut,
                options,
                texture_window,
            ),
            framebuffer.draw_shaded_textured_triangle(
                second[0],
                second[1],
                second[2],
                second_colors,
                texture_page,
                clut,
                options,
                texture_window,
            ),
        )
    }

    #[test]
    fn prepared_texture_sampler_matches_scalar_sampling() {
        let mut framebuffer = NativeFrameBuffer::default();
        for y in 0..64 {
            for x in 0..256 {
                let value = ((x * 17 + y * 31) as u16) & 0x7fff;
                framebuffer.set_raw_pixel(x, y, value);
            }
        }

        for (texture_page, clut) in [(0x0000, 0x0041), (0x0080, 0x0082), (0x0100, 0)] {
            let sampling_policy = TextureSamplingPolicy::new(true, true);
            let sampler = PreparedTextureSampler::new(
                &framebuffer.raw_pixels,
                texture_page,
                clut,
                sampling_policy,
                None,
            );
            for (u, v) in [(0, 0), (1, 2), (31, 17), (127, 63), (255, 255)] {
                assert_eq!(
                    sampler.sample(&framebuffer.raw_pixels, u, v),
                    framebuffer.sample_texture_sample_from(
                        &framebuffer.raw_pixels,
                        texture_page,
                        clut,
                        u,
                        v,
                        sampling_policy,
                    )
                );
            }
        }
    }

    #[test]
    fn prepared_4bpp_palette_matches_scalar_fallback_resolution() {
        let mut framebuffer = NativeFrameBuffer::default();
        for y in 468..=500 {
            for x in 0..96 {
                let color = (((x * 7 + y * 3) as u16) & 0x7fff).max(1);
                framebuffer.set_raw_pixel(x, y, color);
            }
        }

        for (texture_page, clut, allow_fallback) in [
            (0x020b, 0x7903, true),
            (0x020c, 0x7904, true),
            (0x020b, 0x7903, false),
            (0x0000, 0x7903, true),
        ] {
            let prepared = prepared_indexed_palette_samples_from(
                &framebuffer.raw_pixels,
                texture_page,
                clut,
                16,
                allow_fallback,
            );
            for (index, actual) in prepared.iter().enumerate().take(16) {
                let expected = indexed_palette_sample_from(
                    &framebuffer.raw_pixels,
                    texture_page,
                    clut,
                    index as i32,
                    16,
                    allow_fallback,
                );
                assert_eq!(
                    *actual, expected,
                    "texture_page=0x{texture_page:04x} clut=0x{clut:04x} index={index}"
                );
            }
        }
    }

    #[test]
    fn prepared_8bpp_palette_matches_scalar_fallback_resolution() {
        let mut framebuffer = NativeFrameBuffer::default();
        for y in 464..512 {
            for x in 0..768 {
                let value = if (x + y) % 11 == 0 {
                    0
                } else {
                    (((x * 17 + y * 31) as u16) & 0x7fff).max(1)
                };
                framebuffer.set_raw_pixel(x, y, value);
            }
        }

        for (texture_page, clut, allow_fallback) in [
            (0x0088, 0x7d40, true),
            (0x008a, 0x7d80, true),
            (0x0299, 0x7c80, true),
            (0x0088, 0x7d40, false),
        ] {
            let prepared = prepared_indexed_palette_samples_from(
                &framebuffer.raw_pixels,
                texture_page,
                clut,
                256,
                allow_fallback,
            );
            for (index, actual) in prepared.iter().enumerate() {
                let expected = indexed_palette_sample_from(
                    &framebuffer.raw_pixels,
                    texture_page,
                    clut,
                    index as i32,
                    256,
                    allow_fallback,
                );
                assert_eq!(
                    *actual, expected,
                    "texture_page=0x{texture_page:04x} clut=0x{clut:04x} index={index}"
                );
            }
        }
    }

    #[test]
    fn framebuffer_textured_quad_shared_sampler_matches_separate_triangles() {
        let base = test_4bpp_framebuffer();
        let (first, second) = test_textured_quad_triangles();
        let options = TextureDrawOptions::opaque_raw();
        let texture_window = TextureWindow::default();

        let mut separate = base.clone();
        let expected = draw_textured_quad_as_separate_triangles(
            &mut separate,
            first,
            second,
            TEST_4BPP_TEXTURE_PAGE,
            TEST_4BPP_CLUT,
            options,
            texture_window,
        );

        let mut shared = base;
        let actual = shared.draw_textured_quad_triangles_shared(
            first,
            second,
            TEST_4BPP_TEXTURE_PAGE,
            TEST_4BPP_CLUT,
            options,
            texture_window,
        );

        assert_eq!(actual, expected);
        assert_eq!(shared.raw_pixels, separate.raw_pixels);
    }

    #[test]
    fn framebuffer_shaded_textured_quad_shared_sampler_matches_separate_triangles() {
        let base = test_4bpp_framebuffer();
        let (first, second) = test_textured_quad_triangles();
        let first_colors = [0x0080_8080, 0x00c0_8080, 0x0080_c080];
        let second_colors = [0x00c0_8080, 0x0080_c080, 0x00c0_c080];
        let mut options = TextureDrawOptions::opaque_raw();
        options.raw_texture = false;
        let texture_window = TextureWindow::default();

        let mut separate = base.clone();
        let expected = draw_shaded_textured_quad_as_separate_triangles(
            &mut separate,
            first,
            first_colors,
            second,
            second_colors,
            TEST_4BPP_TEXTURE_PAGE,
            TEST_4BPP_CLUT,
            options,
            texture_window,
        );

        let mut shared = base;
        let actual = shared.draw_shaded_textured_quad_triangles_shared(
            first,
            first_colors,
            second,
            second_colors,
            TEST_4BPP_TEXTURE_PAGE,
            TEST_4BPP_CLUT,
            options,
            texture_window,
        );

        assert_eq!(actual, expected);
        assert_eq!(shared.raw_pixels, separate.raw_pixels);
    }

    #[test]
    fn framebuffer_textured_quad_shared_snapshot_matches_separate_triangles() {
        let mut base = NativeFrameBuffer::default();
        for y in 40..=48 {
            for x in 40..=48 {
                base.set_raw_pixel(x, y, 0x001f + ((x + y) as u16 & 0x03e0));
            }
        }
        let first = [
            textured_point(8, 8, 40, 40),
            textured_point(16, 8, 48, 40),
            textured_point(8, 16, 40, 48),
        ];
        let second = [
            textured_point(16, 8, 48, 40),
            textured_point(8, 16, 40, 48),
            textured_point(16, 16, 48, 48),
        ];
        let options = TextureDrawOptions::opaque_raw();
        let texture_window = TextureWindow::default();
        let union_bounds = (8, 8, 16, 16);
        let resources = PreparedTextureDrawResources::new(
            &base,
            union_bounds,
            TEST_RAW15_TEXTURE_PAGE,
            0,
            TextureSamplingPolicy::from_draw_options(options),
        );
        assert!(resources.snapshot_used());

        let mut separate = base.clone();
        let expected = draw_textured_quad_as_separate_triangles(
            &mut separate,
            first,
            second,
            TEST_RAW15_TEXTURE_PAGE,
            0,
            options,
            texture_window,
        );

        let mut shared = base;
        let actual = shared.draw_textured_quad_triangles_shared(
            first,
            second,
            TEST_RAW15_TEXTURE_PAGE,
            0,
            options,
            texture_window,
        );

        assert_eq!(actual, expected);
        assert_eq!(shared.raw_pixels, separate.raw_pixels);
    }

    #[test]
    fn prepared_texture_sampler_matches_scalar_sampling_with_descriptor_alias_policy() {
        let mut framebuffer = NativeFrameBuffer::default();
        let stale_texture_page = 0x0039;
        let stale_clut = 0x7859;
        let (live_texture_page, live_clut) =
            br2_texture_descriptor_alias(stale_texture_page, stale_clut);
        let (stale_x, stale_y) = texture_page_origin_for_clut(stale_texture_page, stale_clut);
        let (live_x, live_y) = texture_page_origin_for_clut(live_texture_page, live_clut);
        let stale_clut_x = ((stale_clut & 0x3f) as i32) * 16;
        let stale_clut_y = ((stale_clut >> 6) & 0x03ff) as i32;
        let live_clut_x = ((live_clut & 0x3f) as i32) * 16;
        let live_clut_y = ((live_clut >> 6) & 0x03ff) as i32;

        framebuffer.set_raw_pixel(stale_x, stale_y, 0x0001);
        framebuffer.set_raw_pixel(stale_clut_x + 1, stale_clut_y, 0x03e0);
        framebuffer.set_raw_pixel(live_x, live_y, 0x0001);
        framebuffer.set_raw_pixel(live_clut_x + 1, live_clut_y, 0x001f);

        for (allow_alias, expected_color) in [(true, 0x001f), (false, 0x03e0)] {
            let sampling_policy = TextureSamplingPolicy::new(false, allow_alias);
            let sampler = PreparedTextureSampler::new(
                &framebuffer.raw_pixels,
                stale_texture_page,
                stale_clut,
                sampling_policy,
                None,
            );
            let prepared = sampler.sample(&framebuffer.raw_pixels, 0, 0);
            let scalar = framebuffer.sample_texture_sample_from(
                &framebuffer.raw_pixels,
                stale_texture_page,
                stale_clut,
                0,
                0,
                sampling_policy,
            );

            assert_eq!(prepared, scalar, "allow_alias={allow_alias}");
            assert_eq!(prepared.color, expected_color, "allow_alias={allow_alias}");
            assert!(!prepared.palette_fallback);
        }
    }

    #[test]
    fn framebuffer_exports_png_base64() {
        let mut framebuffer = NativeFrameBuffer::default();
        framebuffer.fill_rect(0, 0, 8, 8, 0x00ff_0000);

        let png = framebuffer.png_base64(0, 0, 8, 8);

        assert!(png.starts_with("iVBORw0KGgo"));
    }

    #[test]
    fn framebuffer_draws_clipped_triangle() {
        let mut framebuffer = NativeFrameBuffer::default();
        framebuffer.draw_triangle(
            Point { x: -10, y: -10 },
            Point { x: 20, y: 4 },
            Point { x: 4, y: 20 },
            0x0000_ff00,
        );

        let png = framebuffer.png_base64(0, 0, 16, 16);

        assert!(png.starts_with("iVBORw0KGgo"));
    }

    #[test]
    fn framebuffer_psx_quad_split_is_half_open() {
        let mut framebuffer = NativeFrameBuffer::default();
        framebuffer.draw_triangle(
            Point { x: 0, y: 0 },
            Point { x: 4, y: 0 },
            Point { x: 0, y: 4 },
            0x00ff_ffff,
        );
        framebuffer.draw_triangle(
            Point { x: 4, y: 0 },
            Point { x: 0, y: 4 },
            Point { x: 4, y: 4 },
            0x00ff_ffff,
        );

        let covered = (0..=4)
            .flat_map(|y| (0..=4).map(move |x| (x, y)))
            .filter(|(x, y)| framebuffer.raw_pixel(*x, *y) != 0)
            .count();
        assert_eq!(covered, 16);
        for offset in 0..=4 {
            assert_eq!(framebuffer.raw_pixel(4, offset), 0);
            assert_eq!(framebuffer.raw_pixel(offset, 4), 0);
        }
    }

    #[test]
    fn framebuffer_psx_quad_split_does_not_double_blend_shared_diagonal() {
        let mut framebuffer = NativeFrameBuffer::default();
        for y in 0..4 {
            for x in 0..4 {
                framebuffer.set_raw_pixel(x, y, 0x7fff);
            }
        }
        let options = PixelWriteOptions {
            semi_transparent: true,
            semi_transparency_mode: 0,
            ..PixelWriteOptions::default()
        };
        framebuffer.draw_triangle_with_options(
            Point { x: 0, y: 0 },
            Point { x: 4, y: 0 },
            Point { x: 0, y: 4 },
            0x00ff_0000,
            options,
        );
        framebuffer.draw_triangle_with_options(
            Point { x: 4, y: 0 },
            Point { x: 0, y: 4 },
            Point { x: 4, y: 4 },
            0x00ff_0000,
            options,
        );

        assert_eq!(framebuffer.raw_pixel(2, 2), 0x3dfe);
    }

    #[test]
    fn framebuffer_psx_triangle_edge_rule_is_winding_invariant() {
        let a = Point { x: 1, y: 1 };
        let b = Point { x: 5, y: 2 };
        let c = Point { x: 2, y: 6 };
        let mut clockwise = NativeFrameBuffer::default();
        clockwise.draw_triangle(a, b, c, 0x00ff_ffff);
        let mut counter_clockwise = NativeFrameBuffer::default();
        counter_clockwise.draw_triangle(a, c, b, 0x00ff_ffff);

        for y in 0..8 {
            for x in 0..8 {
                assert_eq!(
                    clockwise.raw_pixel(x, y),
                    counter_clockwise.raw_pixel(x, y),
                    "winding mismatch at ({x}, {y})"
                );
            }
        }
    }

    #[test]
    fn framebuffer_culls_triangles_above_gpu_primitive_limits() {
        let mut framebuffer = NativeFrameBuffer::default();
        framebuffer.draw_triangle(
            Point { x: -1024, y: 0 },
            Point { x: 0, y: 0 },
            Point { x: 0, y: 1 },
            0x00ff_ffff,
        );

        assert_eq!(framebuffer.pixel(0, 0), 0);
        assert_eq!(framebuffer.pixel(0, 1), 0);
    }

    #[test]
    fn framebuffer_accepts_exact_gpu_primitive_limits() {
        assert!(!super::triangle_exceeds_gpu_size_limit(
            Point { x: -1023, y: 0 },
            Point { x: 0, y: 0 },
            Point { x: 0, y: 511 },
        ));
        assert!(super::triangle_exceeds_gpu_size_limit(
            Point { x: -1024, y: 0 },
            Point { x: 0, y: 0 },
            Point { x: 0, y: 1 },
        ));
        assert!(super::triangle_exceeds_gpu_size_limit(
            Point { x: 0, y: -512 },
            Point { x: 1, y: 0 },
            Point { x: 0, y: 0 },
        ));
    }

    #[test]
    fn framebuffer_writes_rgb555_images_and_copies_rects() {
        let mut framebuffer = NativeFrameBuffer::default();

        framebuffer.write_rgb555_image(4, 4, 2, 1, &[0x03e0_001f]);
        framebuffer.copy_rect(4, 4, 8, 8, 2, 1);

        assert_eq!(framebuffer.pixel(8, 8), 0x00ff_0000);
        assert_eq!(framebuffer.pixel(9, 8), 0x0000_ff00);
        assert_eq!(framebuffer.raw_pixel(8, 8), 0x001f);
        assert_eq!(framebuffer.raw_pixel(9, 8), 0x03e0);
    }

    #[test]
    fn framebuffer_region_snapshot_restores_display_without_touching_texture_vram() {
        let mut framebuffer = NativeFrameBuffer::default();
        framebuffer.set_raw_pixel(8, 8, 0x001f);
        framebuffer.set_raw_pixel(700, 8, 0x03e0);
        let snapshot = framebuffer.snapshot_psx_display_region(0, 0, 320, 240);

        assert_eq!(snapshot.rows.len(), 240);
        assert_eq!(snapshot.raw_pixels.len(), 320 * 240);
        framebuffer.set_raw_pixel(8, 8, 0x7c00);
        framebuffer.set_raw_pixel(700, 8, 0x7fff);
        framebuffer.restore_region_snapshot(&snapshot);

        assert_eq!(framebuffer.raw_pixel(8, 8), 0x001f);
        assert_eq!(
            framebuffer.raw_pixel(700, 8),
            0x7fff,
            "restoring a display field must preserve texture pages outside it"
        );
    }

    #[test]
    fn framebuffer_region_snapshot_wraps_at_psx_vram_height() {
        let mut framebuffer = NativeFrameBuffer::default();
        framebuffer.set_raw_pixel(0, PSX_VRAM_HEIGHT as i32 - 1, 0x001f);
        framebuffer.set_raw_pixel(0, 0, 0x03e0);
        let snapshot = framebuffer.snapshot_psx_display_region(0, PSX_VRAM_HEIGHT - 1, 1, 2);

        framebuffer.set_raw_pixel(0, PSX_VRAM_HEIGHT as i32 - 1, 0x7c00);
        framebuffer.set_raw_pixel(0, 0, 0x7c00);
        framebuffer.restore_region_snapshot(&snapshot);

        assert_eq!(framebuffer.raw_pixel(0, PSX_VRAM_HEIGHT as i32 - 1), 0x001f);
        assert_eq!(framebuffer.raw_pixel(0, 0), 0x03e0);
    }

    #[test]
    fn framebuffer_region_snapshot_wraps_x_and_y_as_row_segments() {
        let mut framebuffer = NativeFrameBuffer::default();
        let right = VRAM_WIDTH as i32 - 1;
        let bottom = PSX_VRAM_HEIGHT as i32 - 1;
        framebuffer.set_raw_pixel(right, bottom, 0x001f);
        framebuffer.set_raw_pixel(0, bottom, 0x03e0);
        framebuffer.set_raw_pixel(right, 0, 0x7c00);
        framebuffer.set_raw_pixel(0, 0, 0x7fff);
        framebuffer.set_raw_pixel(1, 0, 0x4210);

        let snapshot =
            framebuffer.snapshot_psx_display_region(VRAM_WIDTH - 1, PSX_VRAM_HEIGHT - 1, 2, 2);

        assert_eq!(snapshot.rows.len(), 4);
        assert_eq!(snapshot.raw_pixels, vec![0x001f, 0x03e0, 0x7c00, 0x7fff]);
        framebuffer.set_raw_pixel(right, bottom, 0x0001);
        framebuffer.set_raw_pixel(0, bottom, 0x0002);
        framebuffer.set_raw_pixel(right, 0, 0x0003);
        framebuffer.set_raw_pixel(0, 0, 0x0004);
        framebuffer.restore_region_snapshot(&snapshot);

        assert_eq!(framebuffer.raw_pixel(right, bottom), 0x001f);
        assert_eq!(framebuffer.raw_pixel(0, bottom), 0x03e0);
        assert_eq!(framebuffer.raw_pixel(right, 0), 0x7c00);
        assert_eq!(framebuffer.raw_pixel(0, 0), 0x7fff);
        assert_eq!(
            framebuffer.raw_pixel(1, 0),
            0x4210,
            "restoring a wrapped display snapshot must not touch pixels outside the snapshot"
        );
    }

    #[test]
    fn framebuffer_preserves_extended_2mb_vram_rows() {
        let mut framebuffer = NativeFrameBuffer::default();
        let extended_y = 768;

        framebuffer.write_rgb555_image(4, extended_y, 2, 1, &[0x03e0_001f]);
        framebuffer.copy_rect(4, extended_y, 8, extended_y + 8, 2, 1);

        assert_eq!(framebuffer.pixel(8, extended_y + 8), 0x00ff_0000);
        assert_eq!(framebuffer.pixel(9, extended_y + 8), 0x0000_ff00);
        assert_eq!(
            framebuffer.pixel(8, extended_y + 8 - PSX_VRAM_HEIGHT as i32),
            0
        );
    }

    #[test]
    fn framebuffer_psx_display_reads_wrap_at_512_vram_rows() {
        let mut framebuffer = NativeFrameBuffer::default();

        framebuffer.set_raw_pixel(0, (PSX_VRAM_HEIGHT - 1) as i32, 0x001f);
        framebuffer.set_raw_pixel(0, 0, 0x03e0);
        framebuffer.set_raw_pixel(0, PSX_VRAM_HEIGHT as i32, 0x7c00);

        let pixels = framebuffer.psx_display_rgb_window(0, PSX_VRAM_HEIGHT - 1, 1, 2);
        let stats = framebuffer.psx_display_stats(0, PSX_VRAM_HEIGHT - 1, 1, 2);

        assert_eq!(pixels, vec![0x00ff_0000, 0x0000_ff00]);
        assert_eq!(stats.nonzero_pixels, 2);
        assert_ne!(pixels[1], framebuffer.pixel(0, PSX_VRAM_HEIGHT as i32));
    }

    #[test]
    fn framebuffer_texture_sampling_uses_zn_extended_clut_rows() {
        let mut framebuffer = NativeFrameBuffer::default();

        framebuffer.set_raw_pixel(0, 0, 0x0001);
        framebuffer.set_raw_pixel(1, 1, 0x001f);
        framebuffer.set_raw_pixel(1, PSX_VRAM_HEIGHT as i32 + 1, 0x03e0);
        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            0,
            0x8040,
            TextureCoordinate { u: 0, v: 0 },
            TextureDrawOptions::opaque_raw(),
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), 0x0000_ff00);
    }

    #[test]
    fn framebuffer_texture_sampling_uses_zn_extended_texture_page_y() {
        let mut framebuffer = NativeFrameBuffer::default();

        framebuffer.set_raw_pixel(0, 0, 0x03e0);
        framebuffer.set_raw_pixel(0, PSX_VRAM_HEIGHT as i32, 0x001f);
        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            0x0900,
            0,
            TextureCoordinate { u: 0, v: 0 },
            TextureDrawOptions::opaque_raw(),
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), 0x00ff_0000);
    }

    #[test]
    fn framebuffer_texture_sampling_uses_zn_low_bank_y0_alias_for_title_pages() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x0299;
        let (page_x, page_y) = texture_page_origin(texture_page);

        assert_eq!((page_x, page_y), (576, 0));

        framebuffer.set_raw_pixel(page_x, 256, 0x0002);
        framebuffer.set_raw_pixel(page_x, page_y, 0x0001);
        framebuffer.set_raw_pixel(1, 0, 0x001f);
        framebuffer.set_raw_pixel(2, 0, 0x03e0);
        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            0,
            TextureCoordinate { u: 0, v: 0 },
            TextureDrawOptions::opaque_raw(),
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), 0x00ff_0000);
    }

    #[test]
    fn framebuffer_texture_sampling_uses_zn_4bpp_title_y0_alias_for_runtime_title_pages() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x003f;
        let (page_x, page_y) = texture_page_origin(texture_page);

        assert_eq!((page_x, page_y), (960, 0));

        framebuffer.set_raw_pixel(page_x, 0, 0x0001);
        framebuffer.set_raw_pixel(page_x, 256, 0x0002);
        framebuffer.set_raw_pixel(1, 0, 0x001f);
        framebuffer.set_raw_pixel(2, 0, 0x03e0);
        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            0,
            TextureCoordinate { u: 0, v: 0 },
            TextureDrawOptions::opaque_raw(),
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), 0x00ff_0000);
    }

    #[test]
    fn framebuffer_texture_sampling_uses_br2_live_003f_stage_bank_without_breaking_title_alias() {
        let texture_page = 0x003f;
        assert_eq!(texture_page_origin(texture_page), (960, 0));
        assert_eq!(
            texture_page_origin_for_clut(texture_page, 0x7818),
            (960, 256)
        );
        assert_eq!(
            texture_page_origin_for_clut(texture_page, 0x7958),
            (960, 256)
        );
        assert_eq!(texture_page_origin_for_clut(texture_page, 0), (960, 0));

        let mut framebuffer = NativeFrameBuffer::default();
        framebuffer.set_raw_pixel(960, 0, 0x0001);
        framebuffer.set_raw_pixel(960, 256, 0x0002);
        let clut_x = (0x7818 & 0x3f) * 16;
        let clut_y = (0x7818 >> 6) & 0x03ff;
        framebuffer.set_raw_pixel(clut_x + 1, clut_y, 0x001f);
        framebuffer.set_raw_pixel(clut_x + 2, clut_y, 0x03e0);
        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            0x7818,
            TextureCoordinate { u: 0, v: 0 },
            TextureDrawOptions::opaque_raw(),
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), 0x0000_ff00);
    }

    #[test]
    fn recovery_raster_cache_promotes_repeated_opaque_draw_and_restores_pixels() {
        let mut framebuffer = test_4bpp_framebuffer();
        let dest = Point { x: 128, y: 128 };
        let options = TextureDrawOptions::opaque_raw();

        framebuffer.begin_recovery_raster_command(0x1111);
        let expected_stats = framebuffer.draw_textured_rect(
            dest,
            (8, 8),
            TEST_4BPP_TEXTURE_PAGE,
            TEST_4BPP_CLUT,
            TextureCoordinate { u: 0, v: 0 },
            options,
            TextureWindow::default(),
        );
        framebuffer.end_recovery_raster_command();
        let expected = framebuffer.raw_pixel(dest.x, dest.y);
        assert_ne!(expected, 0);
        assert_eq!(framebuffer.recovery_raster_cache_stats(), (0, 1, 0, 0));

        framebuffer.begin_recovery_raster_command(0x1111);
        let promoted_stats = framebuffer.draw_textured_rect(
            dest,
            (8, 8),
            TEST_4BPP_TEXTURE_PAGE,
            TEST_4BPP_CLUT,
            TextureCoordinate { u: 0, v: 0 },
            options,
            TextureWindow::default(),
        );
        framebuffer.end_recovery_raster_command();
        assert_eq!(promoted_stats, expected_stats);
        let (_, misses, entries, writes) = framebuffer.recovery_raster_cache_stats();
        assert_eq!(misses, 2);
        assert_eq!(entries, 1);
        assert!(writes > 0);

        framebuffer.set_raw_pixel(dest.x, dest.y, 0);
        framebuffer.begin_recovery_raster_command(0x1111);
        let replayed_stats = framebuffer.draw_textured_rect(
            dest,
            (8, 8),
            TEST_4BPP_TEXTURE_PAGE,
            TEST_4BPP_CLUT,
            TextureCoordinate { u: 0, v: 0 },
            options,
            TextureWindow::default(),
        );
        framebuffer.end_recovery_raster_command();

        assert_eq!(replayed_stats, expected_stats);
        assert_eq!(framebuffer.raw_pixel(dest.x, dest.y), expected);
        assert_eq!(framebuffer.recovery_raster_cache_stats().0, 1);
    }

    #[test]
    fn recovery_raster_cache_separates_context_and_rejects_unsafe_draws() {
        let mut framebuffer = test_4bpp_framebuffer();
        let dest = Point { x: 128, y: 128 };

        for fingerprint in [0x1111, 0x2222] {
            framebuffer.begin_recovery_raster_command(fingerprint);
            framebuffer.draw_textured_rect(
                dest,
                (8, 8),
                TEST_4BPP_TEXTURE_PAGE,
                TEST_4BPP_CLUT,
                TextureCoordinate { u: 0, v: 0 },
                TextureDrawOptions::opaque_raw(),
                TextureWindow::default(),
            );
            framebuffer.end_recovery_raster_command();
        }
        assert_eq!(framebuffer.recovery_raster_cache_stats(), (0, 2, 0, 0));

        let mut semi_transparent = TextureDrawOptions::opaque_raw();
        semi_transparent.semi_transparent = true;
        for _ in 0..3 {
            framebuffer.begin_recovery_raster_command(0x3333);
            framebuffer.draw_textured_rect(
                dest,
                (8, 8),
                TEST_4BPP_TEXTURE_PAGE,
                TEST_4BPP_CLUT,
                TextureCoordinate { u: 0, v: 0 },
                semi_transparent,
                TextureWindow::default(),
            );
            framebuffer.end_recovery_raster_command();
        }
        assert_eq!(framebuffer.recovery_raster_cache_stats(), (0, 2, 0, 0));

        for _ in 0..3 {
            framebuffer.begin_recovery_raster_command(0x4444);
            framebuffer.draw_textured_rect(
                Point { x: 0, y: 0 },
                (8, 8),
                TEST_4BPP_TEXTURE_PAGE,
                TEST_4BPP_CLUT,
                TextureCoordinate { u: 0, v: 0 },
                TextureDrawOptions::opaque_raw(),
                TextureWindow::default(),
            );
            framebuffer.end_recovery_raster_command();
        }
        let (hits, misses, entries, writes) = framebuffer.recovery_raster_cache_stats();
        assert_eq!(hits, 0);
        assert_eq!(misses, 5);
        assert_eq!(entries, 0);
        assert_eq!(writes, 0);
    }

    #[test]
    fn recovery_raster_cache_ignores_unrelated_vram_writes() {
        let mut framebuffer = test_4bpp_framebuffer();
        let dest = Point { x: 128, y: 128 };
        let draw = |framebuffer: &mut NativeFrameBuffer| {
            framebuffer.begin_recovery_raster_command(0x5555);
            framebuffer.draw_textured_rect(
                dest,
                (8, 8),
                TEST_4BPP_TEXTURE_PAGE,
                TEST_4BPP_CLUT,
                TextureCoordinate { u: 0, v: 0 },
                TextureDrawOptions::opaque_raw(),
                TextureWindow::default(),
            );
            framebuffer.end_recovery_raster_command();
        };

        draw(&mut framebuffer);
        draw(&mut framebuffer);
        assert_eq!(framebuffer.recovery_raster_cache_stats().0, 0);

        framebuffer.set_raw_pixel(900, 700, 0x7fff);
        framebuffer.fill_rect_unclipped(dest.x, dest.y, 8, 8, 0);
        draw(&mut framebuffer);

        assert_eq!(framebuffer.recovery_raster_cache_stats().0, 1);
        assert_ne!(framebuffer.raw_pixel(dest.x, dest.y), 0);
    }

    #[test]
    fn recovery_raster_cache_invalidates_changed_texture_content() {
        let mut framebuffer = test_4bpp_framebuffer();
        let dest = Point { x: 128, y: 128 };
        let draw = |framebuffer: &mut NativeFrameBuffer| {
            framebuffer.begin_recovery_raster_command(0x6666);
            framebuffer.draw_textured_rect(
                dest,
                (8, 8),
                TEST_4BPP_TEXTURE_PAGE,
                TEST_4BPP_CLUT,
                TextureCoordinate { u: 0, v: 0 },
                TextureDrawOptions::opaque_raw(),
                TextureWindow::default(),
            );
            framebuffer.end_recovery_raster_command();
        };

        draw(&mut framebuffer);
        draw(&mut framebuffer);
        let original = framebuffer.raw_pixel(dest.x, dest.y);
        assert_eq!(framebuffer.recovery_raster_cache_stats(), (0, 2, 1, 64));

        write_4bpp_texel(&mut framebuffer, 0, 0, 15);
        framebuffer.fill_rect_unclipped(dest.x, dest.y, 8, 8, 0);
        draw(&mut framebuffer);

        let (hits, misses, entries, _) = framebuffer.recovery_raster_cache_stats();
        assert_eq!(hits, 0);
        assert_eq!(misses, 3);
        assert_eq!(entries, 1);
        assert_ne!(framebuffer.raw_pixel(dest.x, dest.y), original);
    }

    #[test]
    fn recovery_raster_cache_separates_textured_rect_sizes() {
        let mut framebuffer = test_4bpp_framebuffer();
        let dest = Point { x: 128, y: 128 };
        let draw = |framebuffer: &mut NativeFrameBuffer, size| {
            framebuffer.begin_recovery_raster_command(0x7777);
            framebuffer.draw_textured_rect(
                dest,
                size,
                TEST_4BPP_TEXTURE_PAGE,
                TEST_4BPP_CLUT,
                TextureCoordinate { u: 0, v: 0 },
                TextureDrawOptions::opaque_raw(),
                TextureWindow::default(),
            );
            framebuffer.end_recovery_raster_command();
        };

        draw(&mut framebuffer, (8, 8));
        draw(&mut framebuffer, (8, 8));
        framebuffer.fill_rect_unclipped(dest.x, dest.y, 16, 16, 0);
        draw(&mut framebuffer, (16, 16));

        let (hits, misses, entries, _) = framebuffer.recovery_raster_cache_stats();
        assert_eq!(hits, 0);
        assert_eq!(misses, 3);
        assert_eq!(entries, 1);
        assert_ne!(framebuffer.raw_pixel(dest.x + 15, dest.y + 15), 0);
    }

    #[test]
    fn recovery_raster_cache_separates_shaded_vertex_colors() {
        let mut framebuffer = test_4bpp_framebuffer();
        let points = [
            textured_point(128, 128, 0, 0),
            textured_point(144, 128, 16, 0),
            textured_point(128, 144, 0, 16),
        ];
        let mut options = TextureDrawOptions::opaque_raw();
        options.raw_texture = false;
        let draw = |framebuffer: &mut NativeFrameBuffer, colors| {
            framebuffer.begin_recovery_raster_command(0x8888);
            framebuffer.draw_shaded_textured_triangle(
                points[0],
                points[1],
                points[2],
                colors,
                TEST_4BPP_TEXTURE_PAGE,
                TEST_4BPP_CLUT,
                options,
                TextureWindow::default(),
            );
            framebuffer.end_recovery_raster_command();
        };

        draw(&mut framebuffer, [0x0000_00ff; 3]);
        draw(&mut framebuffer, [0x0000_00ff; 3]);
        let blue = framebuffer.raw_pixel(130, 130);
        framebuffer.fill_rect_unclipped(128, 128, 17, 17, 0);
        draw(&mut framebuffer, [0x00ff_0000; 3]);
        let red = framebuffer.raw_pixel(130, 130);

        let (hits, misses, entries, _) = framebuffer.recovery_raster_cache_stats();
        assert_eq!(hits, 0);
        assert_eq!(misses, 3);
        assert_eq!(entries, 1);
        assert_ne!(blue, red);
    }

    #[test]
    fn recovery_palette_dependencies_include_high_bank_character_override() {
        let texture_page = 0x0039;
        let clut = 0x799a;
        let requested_x = ((clut & 0x3f) as i32) * 16;
        let bounds =
            recovery_palette_fallback_dependency_bounds_with_override(texture_page, clut, Some(32));

        assert!(bounds.0 <= 32);
        assert!(bounds.2 >= requested_x + 15);
        assert!(bounds.1 <= 486);
        assert!(bounds.3 >= 486);
    }

    #[test]
    fn recovery_raster_cache_replaces_existing_key_without_write_leak() {
        let mut framebuffer = NativeFrameBuffer::default();
        let key = RecoveryRasterCacheKey {
            command: 0x1234,
            raster: 0x5678,
        };
        let stats = [TexturedDrawStats::default(); 2];

        framebuffer.recovery_raster_capture = Some(vec![(1, 0x001f)]);
        framebuffer.finish_recovery_raster(Some(key), stats, true);
        assert_eq!(framebuffer.recovery_raster_cache_stats(), (0, 0, 1, 1));

        framebuffer.recovery_raster_capture = Some(vec![(1, 0x03e0), (2, 0x7c00), (3, 0x7fff)]);
        framebuffer.finish_recovery_raster(Some(key), stats, true);

        assert_eq!(framebuffer.recovery_raster_cache_stats(), (0, 0, 1, 3));
        let cache = framebuffer
            .recovery_raster_cache
            .lock()
            .expect("recovery raster cache lock");
        assert_eq!(
            cache.order.iter().filter(|queued| **queued == key).count(),
            1
        );
    }

    #[test]
    fn framebuffer_texture_sampling_uses_br2_stage_y0_for_reused_title_page() {
        for clut in [0x7859, 0x7959] {
            let texture_page = 0x0039;
            let (page_x, page_y) = texture_page_origin(texture_page);

            assert_eq!((page_x, page_y), (576, 0));
            assert_eq!(
                texture_page_origin_for_clut(texture_page, clut),
                (page_x, page_y),
                "stage CLUT 0x{clut:04x} must not sample the y=256 effects atlas"
            );
        }
    }

    #[test]
    fn framebuffer_texture_sampling_uses_br2_model_y256_for_reused_title_page() {
        for clut in [0x799a, 0x79d9, 0x7a5a, 0x7a9a] {
            let texture_page = 0x0039;
            assert_eq!(
                texture_page_origin_for_clut(texture_page, clut),
                (576, 256),
                "model CLUT 0x{clut:04x} must sample the high-bank model upload"
            );
        }
    }

    #[test]
    fn framebuffer_texture_sampling_uses_br2_beast_effect_y256_alias() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x0039;
        let clut = 0x785a;
        let page_x = 576;
        let alias_y = PSX_VRAM_HEIGHT as i32 / 2;
        let clut_x = ((clut & 0x3f) as i32) * 16;
        let clut_y = ((clut >> 6) & 0x03ff) as i32;

        assert_eq!(texture_page_origin(texture_page), (page_x, 0));
        assert_eq!(
            texture_page_origin_for_clut(texture_page, clut),
            (page_x, alias_y)
        );
        assert_eq!(
            texture_page_origin_for_clut(0x0039, 0x7859),
            (page_x, 0),
            "the adjacent stage descriptor must remain on the low VRAM bank"
        );

        framebuffer.set_raw_pixel(page_x + 40, 224, 0x1111);
        framebuffer.set_raw_pixel(page_x + 40, alias_y + 224, 0x0001);
        framebuffer.set_raw_pixel(clut_x, clut_y, 0);
        framebuffer.set_raw_pixel(clut_x + 1, clut_y, 0x03e0);

        let sample = framebuffer.sample_texture_sample_from(
            &framebuffer.raw_pixels,
            texture_page,
            clut,
            160,
            224,
            TextureSamplingPolicy::new(false, true),
        );
        assert_eq!(sample.color, 0x03e0);
        assert!(sample.texture_nonzero);
    }

    #[test]
    fn framebuffer_texture_sampling_uses_br2_model_y256_for_dynamic_clut_x_variants() {
        for clut in [0x79dd, 0x7a9d] {
            assert_eq!(
                texture_page_origin_for_clut(0x0039, clut),
                (576, 256),
                "dynamic model CLUT 0x{clut:04x} must retain the model texture bank"
            );
        }
    }

    #[test]
    fn framebuffer_texture_sampling_keeps_br2_low_bank_fighter_atlas_on_y0() {
        for (texture_page, clut, expected_x) in [
            (0x0208, 0x7900, 512),
            (0x0208, 0x7ac0, 512),
            (0x0209, 0x7940, 576),
            (0x0209, 0x7980, 576),
        ] {
            assert_eq!(
                texture_page_origin_for_clut(texture_page, clut),
                (expected_x, 0),
                "gameplay fighter TPage 0x{texture_page:04x} CLUT 0x{clut:04x} must not sample the y=256 UI atlas"
            );
        }
    }

    #[test]
    fn framebuffer_texture_sampling_uses_br2_character_model_palette_upload_rows() {
        for (clut, alias_dx, expected_color) in [
            (0x799a, 16, 0x7fff),
            (0x79d9, -16, 0x4210),
            (0x7a5a, 16, 0x03e0),
            (0x7a9a, 16, 0x001f),
        ] {
            let mut framebuffer = NativeFrameBuffer::default();
            let texture_page = 0x0039;
            let (page_x, page_y) = texture_page_origin_for_clut(texture_page, clut);
            let clut_x = ((clut & 0x3f) as i32) * 16;
            let requested_y = ((clut >> 6) & 0x03ff) as i32;
            let alias_x = clut_x + alias_dx;

            framebuffer.set_raw_pixel(page_x, page_y, 0x0001);
            for index in 1..16 {
                framebuffer.set_raw_pixel(clut_x + index, requested_y, 0x03e0);
                framebuffer.set_raw_pixel(
                    alias_x + index,
                    requested_y,
                    if index == 1 {
                        expected_color
                    } else {
                        0x0400 + index as u16
                    },
                );
            }
            framebuffer.draw_textured_rect(
                Point { x: 8, y: 8 },
                (1, 1),
                texture_page,
                clut,
                TextureCoordinate { u: 0, v: 0 },
                TextureDrawOptions::opaque_raw(),
                TextureWindow::default(),
            );

            assert_eq!(
                framebuffer.pixel(8, 8),
                super::rgb555_to_rgb888(expected_color)
            );
        }
    }

    #[test]
    fn framebuffer_texture_sampling_prefers_br2_model_y16_base_over_adjacent_material_rows() {
        for clut in [0x799a, 0x7a9a] {
            let mut framebuffer = NativeFrameBuffer::default();
            let texture_page = 0x0039;
            let (page_x, page_y) = texture_page_origin_for_clut(texture_page, clut);
            let requested_x = ((clut & 0x3f) as i32) * 16;
            let requested_y = ((clut >> 6) & 0x03ff) as i32;
            let base_y = requested_y & !0x0f;

            framebuffer.set_raw_pixel(page_x, page_y, 0x0001);
            for index in 0..16 {
                framebuffer.set_raw_pixel(
                    requested_x + index,
                    base_y,
                    if index == 0 {
                        0
                    } else if index == 1 {
                        0x5295
                    } else {
                        0x0400 + index as u16
                    },
                );
                framebuffer.set_raw_pixel(
                    requested_x + 16 + index,
                    requested_y,
                    if index == 0 { 0 } else { 0x001f },
                );
            }

            framebuffer.draw_textured_rect(
                Point { x: 8, y: 8 },
                (1, 1),
                texture_page,
                clut,
                TextureCoordinate { u: 0, v: 0 },
                TextureDrawOptions::opaque_raw(),
                TextureWindow::default(),
            );

            assert_eq!(
                framebuffer.pixel(8, 8),
                super::rgb555_to_rgb888(0x5295),
                "CLUT 0x{clut:04x} must use the coherent y16 block-base palette"
            );
        }
    }

    #[test]
    fn framebuffer_model_palette_alias_accepts_dynamic_clut_x_variants() {
        for (clut, alias_dx) in [(0x79dd_u16, -16), (0x7a9d, 16)] {
            let mut raw_pixels = vec![0u16; VRAM_WIDTH * VRAM_HEIGHT];
            let requested_x = ((clut & 0x3f) as i32) * 16;
            let requested_y = ((clut >> 6) & 0x03ff) as i32;
            let alias_x = requested_x + alias_dx;
            for index in 1..16 {
                let offset = requested_y as usize * VRAM_WIDTH + alias_x as usize + index;
                raw_pixels[offset] = 0x0400 + index as u16;
            }

            let candidate =
                br2_character_model_palette_candidate_with_policy(&raw_pixels, 0x0039, clut, true)
                    .expect("dynamic model CLUT variants must use the uploaded palette row");
            assert_eq!((candidate.x, candidate.y), (alias_x, requested_y));
        }
    }

    #[test]
    fn framebuffer_texture_sampling_prefers_captured_br2_model_palette_upload() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x0039;
        let clut = 0x799a;
        let (page_x, page_y) = texture_page_origin_for_clut(texture_page, clut);
        let clut_x = ((clut & 0x3f) as i32) * 16;
        let requested_y = ((clut >> 6) & 0x03ff) as i32;
        let alias_x = clut_x + 16;

        framebuffer.set_raw_pixel(page_x, page_y, 0x0001);
        for index in 1..16 {
            framebuffer.set_raw_pixel(alias_x + index, requested_y, 0x0800 + index as u16);
        }
        for index in 1..7 {
            framebuffer.set_raw_pixel(clut_x + index, requested_y, 0x0400 + index as u16);
        }
        framebuffer.set_raw_pixel(clut_x + 1, requested_y, 0x03e0);
        framebuffer.set_raw_pixel(alias_x + 1, requested_y, 0x001f);

        let requested_png = framebuffer.texture_palette_png(texture_page, clut);
        let resolved_png = framebuffer.resolved_texture_palette_png(texture_page, clut);
        let sample = super::indexed_palette_sample_from(
            &framebuffer.raw_pixels,
            texture_page,
            clut,
            1,
            16,
            true,
        );
        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            clut,
            TextureCoordinate { u: 0, v: 0 },
            TextureDrawOptions::opaque_raw(),
            TextureWindow::default(),
        );

        assert_ne!(requested_png, resolved_png);
        assert!(sample.palette_fallback);
        assert_eq!(sample.color, 0x001f);
        assert_eq!(framebuffer.pixel(8, 8), super::rgb555_to_rgb888(0x001f));
    }

    #[test]
    fn framebuffer_texture_sampling_keeps_coherent_sparse_br2_model_palette_rows() {
        for clut in [0x799a, 0x7a5a, 0x7a9a] {
            let mut framebuffer = NativeFrameBuffer::default();
            let texture_page = 0x0039;
            let (page_x, page_y) = texture_page_origin_for_clut(texture_page, clut);
            let requested_x = ((clut & 0x3f) as i32) * 16;
            let requested_y = ((clut >> 6) & 0x03ff) as i32;
            let alias_x = requested_x + 16;

            framebuffer.set_raw_pixel(page_x, page_y, 0x0001);
            for index in 1..8 {
                framebuffer.set_raw_pixel(requested_x + index, requested_y, 0x0400 + index as u16);
            }
            for index in 1..16 {
                framebuffer.set_raw_pixel(alias_x + index, requested_y, 0x001f);
            }

            let sample = super::indexed_palette_sample_from(
                &framebuffer.raw_pixels,
                texture_page,
                clut,
                1,
                16,
                true,
            );

            assert!(!sample.palette_fallback, "CLUT 0x{clut:04x}");
            assert_eq!(sample.color, 0x0401, "CLUT 0x{clut:04x}");
        }
    }

    #[test]
    fn framebuffer_texture_sampling_keeps_br2_model_palette_index_zero_transparent() {
        for texture_page in [0x0039, 0x0239] {
            for clut in [0x799a, 0x79d9, 0x7a5a, 0x7a9a] {
                let mut framebuffer = NativeFrameBuffer::default();
                let (page_x, page_y) = texture_page_origin_for_clut(texture_page, clut);
                let requested_x = ((clut & 0x3f) as i32) * 16;
                let requested_y = ((clut >> 6) & 0x03ff) as i32;
                let uploaded_x = if requested_y == 487 {
                    requested_x - 16
                } else {
                    requested_x + 16
                };

                framebuffer.set_raw_pixel(page_x, page_y, 0x0000);
                framebuffer.set_raw_pixel(requested_x, requested_y, 0x03e0);
                framebuffer.set_raw_pixel(uploaded_x, requested_y, 0x7fff);
                framebuffer.draw_textured_rect(
                    Point { x: 8, y: 8 },
                    (1, 1),
                    texture_page,
                    clut,
                    TextureCoordinate { u: 0, v: 0 },
                    TextureDrawOptions::opaque_raw(),
                    TextureWindow::default(),
                );

                assert_eq!(
                    framebuffer.pixel(8, 8),
                    0,
                    "TPage 0x{texture_page:04x} model CLUT 0x{clut:04x} index zero must stay transparent"
                );
            }
        }
    }

    #[test]
    fn framebuffer_texture_sampling_uses_br2_7a5a_uploaded_palette() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x0039;
        let clut = 0x7a5a;
        let (page_x, page_y) = texture_page_origin_for_clut(texture_page, clut);
        let requested_x = ((clut & 0x3f) as i32) * 16;
        let requested_y = ((clut >> 6) & 0x03ff) as i32;
        let uploaded_x = requested_x + 16;

        framebuffer.set_raw_pixel(page_x, page_y, 0x0001);
        for index in 1..16 {
            framebuffer.set_raw_pixel(requested_x + index, requested_y, 0x1135);
            framebuffer.set_raw_pixel(
                uploaded_x + index,
                requested_y,
                if index == 1 {
                    0x03e0
                } else {
                    0x0400 + index as u16
                },
            );
        }
        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            clut,
            TextureCoordinate { u: 0, v: 0 },
            TextureDrawOptions::opaque_raw(),
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), super::rgb555_to_rgb888(0x03e0));
    }

    #[test]
    fn framebuffer_texture_sampling_keeps_br2_7a5a_model_index_zero_transparent() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x0039;
        let clut = 0x7a5a;
        let (page_x, page_y) = texture_page_origin_for_clut(texture_page, clut);
        let requested_x = ((clut & 0x3f) as i32) * 16;
        let requested_y = ((clut >> 6) & 0x03ff) as i32;
        let uploaded_x = requested_x + 16;

        framebuffer.set_raw_pixel(page_x, page_y, 0x0000);
        framebuffer.set_raw_pixel(requested_x, requested_y, 0x1135);
        framebuffer.set_raw_pixel(uploaded_x, requested_y, 0x0000);
        for index in 1..16 {
            framebuffer.set_raw_pixel(requested_x + index, requested_y, 0x1135);
            framebuffer.set_raw_pixel(
                uploaded_x + index,
                requested_y,
                if index & 1 == 0 { 0x03e0 } else { 0x001f },
            );
        }
        framebuffer.set_raw_pixel(8, 8, 0x7c00);

        let sample = framebuffer.sample_texture_sample_from(
            &framebuffer.raw_pixels,
            texture_page,
            clut,
            0,
            0,
            TextureSamplingPolicy::new(false, true),
        );
        assert_eq!(sample.color, 0);
        assert!(sample.zero_texel);
        assert!(!sample.palette_fallback);

        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            clut,
            TextureCoordinate { u: 0, v: 0 },
            TextureDrawOptions::opaque_raw(),
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.raw_pixel(8, 8), 0x7c00);
    }

    #[test]
    fn framebuffer_texture_sampling_recovers_br2_low_bank_gameplay_palette_to_the_right() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x0009;
        let clut = 0x7901;
        let (page_x, page_y) = texture_page_origin_for_clut(texture_page, clut);
        let requested_x = ((clut & 0x3f) as i32) * 16;
        let requested_y = ((clut >> 6) & 0x03ff) as i32;
        let uploaded_x = requested_x + 16;

        framebuffer.set_raw_pixel(page_x, page_y, 0x0001);
        for index in 0..16 {
            framebuffer.set_raw_pixel(
                uploaded_x + index,
                requested_y,
                if index == 0 {
                    0
                } else if index == 1 {
                    0x03e0
                } else {
                    0x0400 + index as u16
                },
            );
        }

        let sample = framebuffer.sample_texture_sample_from(
            &framebuffer.raw_pixels,
            texture_page,
            clut,
            0,
            0,
            TextureSamplingPolicy::new(true, true),
        );

        assert_eq!(sample.color, 0x03e0);
        assert!(sample.palette_fallback);
        assert!(sample.clut_blank);
    }

    #[test]
    fn framebuffer_texture_sampling_recovers_br2_yugo_palette_from_shared_fighter_bank() {
        for clut in [0x7803, 0x7903] {
            let mut framebuffer = NativeFrameBuffer::default();
            let texture_page = 0x020b;
            let (page_x, page_y) = texture_page_origin_for_clut(texture_page, clut);
            let requested_y = ((clut >> 6) & 0x03ff) as i32;
            let uploaded_x = 32;

            framebuffer.set_raw_pixel(page_x, page_y, 0x0001);
            for index in 0..16 {
                framebuffer.set_raw_pixel(
                    uploaded_x + index,
                    requested_y,
                    if index == 0 {
                        0
                    } else if index == 1 {
                        0x4210
                    } else {
                        0x0400 + index as u16
                    },
                );
            }

            let sample = framebuffer.sample_texture_sample_from(
                &framebuffer.raw_pixels,
                texture_page,
                clut,
                0,
                0,
                TextureSamplingPolicy::new(true, true),
            );

            assert_eq!(sample.color, 0x4210);
            assert!(sample.palette_fallback);
            assert!(sample.clut_blank);
        }
    }

    #[test]
    fn framebuffer_captured_br2_fighter_descriptor_keeps_hardware_texture_page() {
        assert_eq!(
            texture_page_origin_for_clut(0x020b, 0x7903),
            (704, 0),
            "the dither bit must not relocate the hardware texture page"
        );
        assert_eq!(
            texture_page_origin_for_clut(0x020b, 0x7803),
            (704, 0),
            "other low-bank fighter descriptors must keep their native page"
        );
        assert_eq!(
            texture_page_origin_for_clut(0x000b, 0x781b),
            (704, 0),
            "stage page 0x000b must not inherit the fighter-only relocation"
        );
    }

    #[test]
    fn framebuffer_texture_sampling_uses_shared_fighter_bank_for_missing_br2_body_palette() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x000c;
        let clut = 0x7844;
        let (page_x, page_y) = texture_page_origin_for_clut(texture_page, clut);
        let requested_y = ((clut >> 6) & 0x03ff) as i32;

        framebuffer.set_raw_pixel(page_x, page_y, 0x0001);
        for index in 0..16 {
            framebuffer.set_raw_pixel(
                32 + index,
                requested_y,
                if index == 0 {
                    0
                } else if index == 1 {
                    0x4210
                } else {
                    0x0400 + index as u16
                },
            );
        }

        let sample = framebuffer.sample_texture_sample_from(
            &framebuffer.raw_pixels,
            texture_page,
            clut,
            0,
            0,
            TextureSamplingPolicy::new(true, true),
        );

        assert_eq!(sample.color, 0x4210);
        assert!(sample.palette_fallback);
        assert!(sample.clut_blank);
    }

    #[test]
    fn framebuffer_low_bank_fighter_palette_prefers_rich_same_row_over_previous_row() {
        let mut raw_pixels = vec![0u16; VRAM_WIDTH * VRAM_HEIGHT];
        let texture_page = 0x020b;
        let clut = 0x7903;
        let requested_x = ((clut & 0x3f) as i32) * 16;
        let requested_y = ((clut >> 6) & 0x03ff) as i32;

        for index in 0..16 {
            raw_pixels[(requested_y - 16) as usize * VRAM_WIDTH + requested_x as usize + index] =
                if index & 1 == 0 { 0x1084 } else { 0x18c6 };
            raw_pixels[requested_y as usize * VRAM_WIDTH + 32 + index] =
                if index == 0 { 0 } else { 0x0400 + index as u16 };
        }

        let candidate = br2_character_model_palette_candidate_with_policy(
            &raw_pixels,
            texture_page,
            clut,
            true,
        )
        .expect("shared same-row fighter palette must be recovered");
        assert_eq!((candidate.x, candidate.y), (32, requested_y));
        assert!(candidate.unique_entries >= 8);
    }

    #[test]
    fn framebuffer_low_bank_fighter_palette_prefers_adjacent_x16_from_x0_descriptor() {
        let mut raw_pixels = vec![0u16; VRAM_WIDTH * VRAM_HEIGHT];
        let texture_page = 0x0208;
        let clut = 0x7900;
        let requested_y = ((clut >> 6) & 0x03ff) as i32;

        for index in 0..16 {
            raw_pixels[requested_y as usize * VRAM_WIDTH + 16 + index] =
                if index == 0 { 0 } else { 0x0800 + index as u16 };
            raw_pixels[requested_y as usize * VRAM_WIDTH + 32 + index] =
                if index == 0 { 0 } else { 0x1000 + index as u16 };
        }

        let candidate = br2_character_model_palette_candidate_with_policy(
            &raw_pixels,
            texture_page,
            clut,
            true,
        )
        .expect("x=0 fighter descriptor must recover the adjacent x=16 upload");
        assert_eq!((candidate.x, candidate.y), (16, requested_y));
    }

    #[test]
    fn framebuffer_texture_sampling_keeps_populated_br2_low_bank_gameplay_palette() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x0009;
        let clut = 0x7901;
        let (page_x, page_y) = texture_page_origin_for_clut(texture_page, clut);
        let requested_x = ((clut & 0x3f) as i32) * 16;
        let requested_y = ((clut >> 6) & 0x03ff) as i32;
        let uploaded_x = requested_x + 16;

        framebuffer.set_raw_pixel(page_x, page_y, 0x0001);
        for index in 0..16 {
            framebuffer.set_raw_pixel(
                requested_x + index,
                requested_y,
                if index == 0 {
                    0
                } else if index == 1 {
                    0x001f
                } else {
                    0x0800 + index as u16
                },
            );
            framebuffer.set_raw_pixel(
                uploaded_x + index,
                requested_y,
                if index == 0 {
                    0
                } else if index == 1 {
                    0x03e0
                } else {
                    0x0400 + index as u16
                },
            );
        }

        let sample = framebuffer.sample_texture_sample_from(
            &framebuffer.raw_pixels,
            texture_page,
            clut,
            0,
            0,
            TextureSamplingPolicy::new(true, true),
        );

        assert_eq!(sample.color, 0x001f);
        assert!(!sample.palette_fallback);
        assert!(!sample.clut_blank);
    }

    #[test]
    fn framebuffer_texture_sampling_recovers_captured_implausibly_dark_low_bank_fighter_palette() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x000a;
        let clut = 0x7a41;
        let (page_x, page_y) = texture_page_origin_for_clut(texture_page, clut);
        let requested_x = ((clut & 0x3f) as i32) * 16;
        let requested_y = ((clut >> 6) & 0x03ff) as i32;
        let previous_y = requested_y - 16;
        let captured_dark = [
            0x0044, 0x0843, 0x0008, 0x0443, 0x0009, 0x0863, 0x0864, 0x0464, 0x0027, 0x000a, 0x0029,
            0x000c, 0x0c63, 0x0466, 0x0044, 0x0843,
        ];
        let uploaded_material = [0x1084, 0x31b0, 0x214d, 0x296e];

        framebuffer.set_raw_pixel(page_x, page_y, 0x0001);
        for index in 0..16 {
            framebuffer.set_raw_pixel(
                requested_x + index,
                requested_y,
                captured_dark[index as usize],
            );
            framebuffer.set_raw_pixel(
                requested_x + index,
                previous_y,
                uploaded_material[index as usize % uploaded_material.len()],
            );
            framebuffer.set_raw_pixel(
                requested_x + 16 + index,
                requested_y,
                if index == 0 { 0 } else { 0x3000 + index as u16 },
            );
        }

        let candidate = br2_character_model_palette_candidate_with_policy(
            &framebuffer.raw_pixels,
            texture_page,
            clut,
            true,
        )
        .expect("the captured near-black fighter row should use its uploaded material palette");
        assert_eq!((candidate.x, candidate.y), (requested_x, previous_y));

        let sample = framebuffer.sample_texture_sample_from(
            &framebuffer.raw_pixels,
            texture_page,
            clut,
            0,
            0,
            TextureSamplingPolicy::new(true, true),
        );
        assert_eq!(sample.color, uploaded_material[1]);
        assert!(sample.palette_fallback);
        assert!(!sample.clut_blank);
    }

    #[test]
    fn br2_captured_0x7840_fighter_palette_recovers_previous_material_row() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x0208;
        let clut = 0x7840;
        let requested_x = ((clut & 0x3f) as i32) * 16;
        let requested_y = ((clut >> 6) & 0x03ff) as i32;
        let previous_y = requested_y - 16;
        let captured_dark = [
            0x0421, 0x0421, 0x0001, 0x0421, 0x0002, 0x0003, 0x0421, 0x0421, 0x0402, 0x0004, 0x0821,
            0x0005, 0x0422, 0x0006, 0x0423, 0x0404,
        ];
        let uploaded_material = [0x214c, 0x214d, 0x296e];

        for index in 0..16 {
            framebuffer.set_raw_pixel(
                requested_x + index,
                requested_y,
                captured_dark[index as usize],
            );
            framebuffer.set_raw_pixel(
                requested_x + index,
                previous_y,
                uploaded_material[index as usize % uploaded_material.len()],
            );
        }

        let requested_stats =
            palette_region_stats(&framebuffer.raw_pixels, requested_x, requested_y, 16)
                .expect("captured palette row must be addressable");
        assert_eq!(requested_stats.unique_entries, 12);
        assert!(requested_stats.is_implausibly_dark_texture_row());

        let candidate = br2_character_model_palette_candidate_with_policy(
            &framebuffer.raw_pixels,
            texture_page,
            clut,
            true,
        )
        .expect("the captured 0x7840 texture row must use its previous material palette");
        assert_eq!((candidate.x, candidate.y), (requested_x, previous_y));
    }

    #[test]
    fn br2_low_bank_dark_repeating_palette_is_not_treated_as_texture_data() {
        let stats = palette_colors_stats([
            0x0421, 0x0842, 0x0c63, 0x1084, 0x14a5, 0x18c6, 0x1ce7, 0x2108, 0x0421, 0x0842, 0x0c63,
            0x1084, 0x14a5, 0x18c6, 0x1ce7, 0x2108,
        ]);

        assert_eq!(stats.nonzero_entries, 16);
        assert_eq!(stats.unique_entries, 8);
        assert!(!stats.is_implausibly_dark_texture_row());
    }

    #[test]
    fn framebuffer_texture_sampling_resolves_br2_relocated_stage_descriptor() {
        for stale_texture_page in [0x0039, 0x0239] {
            let mut framebuffer = NativeFrameBuffer::default();
            let stale_clut = 0x7859;
            let live_texture_page = 0x001c;
            let live_clut = 0x7850;
            let (stale_x, stale_y) = texture_page_origin(stale_texture_page);
            let (live_x, live_y) = texture_page_origin(live_texture_page);
            let stale_clut_x = ((stale_clut & 0x3f) as i32) * 16;
            let stale_clut_y = ((stale_clut >> 6) & 0x03ff) as i32;
            let live_clut_x = (live_clut & 0x3f) * 16;
            let live_clut_y = (live_clut >> 6) & 0x03ff;

            framebuffer.set_raw_pixel(stale_x, stale_y, 0x0001);
            framebuffer.set_raw_pixel(stale_clut_x + 1, stale_clut_y, 0x03e0);
            framebuffer.set_raw_pixel(live_x, live_y, 0x0001);
            framebuffer.set_raw_pixel(live_clut_x + 1, live_clut_y, 0x4210);

            let sample = framebuffer.sample_texture_sample_from(
                &framebuffer.raw_pixels,
                stale_texture_page,
                stale_clut,
                0,
                0,
                TextureSamplingPolicy::new(false, true),
            );

            assert_eq!(
                sample.color, 0x4210,
                "texture page {stale_texture_page:#06x}"
            );
            assert!(!sample.palette_fallback);
        }
    }

    #[test]
    fn framebuffer_texture_sampling_keeps_other_br2_page_0039_descriptors() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x0039;
        let clut = 0x785b;
        let (page_x, page_y) = texture_page_origin(texture_page);
        let clut_x = ((clut & 0x3f) as i32) * 16;
        let clut_y = ((clut >> 6) & 0x03ff) as i32;

        framebuffer.set_raw_pixel(page_x, page_y, 0x0001);
        framebuffer.set_raw_pixel(clut_x + 1, clut_y, 0x03e0);

        let sample = framebuffer.sample_texture_sample_from(
            &framebuffer.raw_pixels,
            texture_page,
            clut,
            0,
            0,
            TextureSamplingPolicy::new(true, true),
        );

        assert_eq!(sample.color, 0x03e0);
    }

    #[test]
    fn framebuffer_texture_sampling_respects_disabled_br2_model_palette_fallback() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x0039;
        let clut = 0x799a;
        let (page_x, page_y) = texture_page_origin_for_clut(texture_page, clut);
        let clut_x = ((clut & 0x3f) as i32) * 16;
        let requested_y = ((clut >> 6) & 0x03ff) as i32;
        let alias_x = clut_x + 16;

        framebuffer.set_raw_pixel(page_x, page_y, 0x0001);
        for index in 1..16 {
            framebuffer.set_raw_pixel(clut_x + index, requested_y, 0x03e0);
            framebuffer.set_raw_pixel(alias_x + index, requested_y, 0x001f);
        }
        let mut options = TextureDrawOptions::opaque_raw();
        options.allow_palette_fallback = false;
        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            clut,
            TextureCoordinate { u: 0, v: 0 },
            options,
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), super::rgb555_to_rgb888(0x03e0));
    }

    #[test]
    fn framebuffer_texture_sampling_does_not_use_sparse_br2_model_palette_alias() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x0039;
        let clut = 0x799a;
        let (page_x, page_y) = texture_page_origin_for_clut(texture_page, clut);
        let clut_x = ((clut & 0x3f) as i32) * 16;
        let requested_y = ((clut >> 6) & 0x03ff) as i32;
        let alias_x = clut_x + 16;

        framebuffer.set_raw_pixel(page_x, page_y, 0x0001);
        framebuffer.set_raw_pixel(clut_x + 1, requested_y, 0x03e0);
        for index in 1..12 {
            framebuffer.set_raw_pixel(alias_x + index, requested_y, 0x001f);
        }
        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            clut,
            TextureCoordinate { u: 0, v: 0 },
            TextureDrawOptions::opaque_raw(),
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), 0x0000_ff00);
    }

    #[test]
    fn framebuffer_texture_sampling_keeps_br2_title_clut_on_y0() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x0039;
        let clut = 0x795a;
        let (page_x, page_y) = texture_page_origin(texture_page);
        let clut_x = ((clut & 0x3f) as i32) * 16;
        let clut_y = ((clut >> 6) & 0x03ff) as i32;

        assert_eq!((page_x, page_y), (576, 0));
        assert_eq!(
            texture_page_origin_for_clut(texture_page, clut),
            (page_x, page_y)
        );

        framebuffer.set_raw_pixel(page_x, page_y, 0x0001);
        framebuffer.set_raw_pixel(page_x, PSX_VRAM_HEIGHT as i32 / 2, 0x0002);
        framebuffer.set_raw_pixel(clut_x + 1, clut_y, 0x001f);
        framebuffer.set_raw_pixel(clut_x + 2, clut_y, 0x03e0);
        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            clut,
            TextureCoordinate { u: 0, v: 0 },
            TextureDrawOptions::opaque_raw(),
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), 0x00ff_0000);
    }

    #[test]
    fn framebuffer_texture_sampling_keeps_standard_4bpp_y_page_outside_title_alias() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x001f;
        let (page_x, page_y) = texture_page_origin(texture_page);

        assert_eq!((page_x, page_y), (960, 256));

        framebuffer.set_raw_pixel(page_x, 0, 0x0001);
        framebuffer.set_raw_pixel(page_x, page_y, 0x0002);
        framebuffer.set_raw_pixel(1, 0, 0x001f);
        framebuffer.set_raw_pixel(2, 0, 0x03e0);
        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            0,
            TextureCoordinate { u: 0, v: 0 },
            TextureDrawOptions::opaque_raw(),
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), 0x0000_ff00);
    }

    #[test]
    fn framebuffer_low_bank_y0_alias_preserves_runtime_title_odd_page_x_by_default() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x0299;
        let (page_x, page_y) = texture_page_origin(texture_page);

        assert_eq!((page_x, page_y), (576, 0));

        framebuffer.set_raw_pixel(512, 0, 0x0002);
        framebuffer.set_raw_pixel(page_x, page_y, 0x0001);
        framebuffer.set_raw_pixel(1, 0, 0x001f);
        framebuffer.set_raw_pixel(2, 0, 0x03e0);
        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            0,
            TextureCoordinate { u: 0, v: 0 },
            TextureDrawOptions::opaque_raw(),
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), 0x00ff_0000);
    }

    #[test]
    fn framebuffer_texture_sampling_keeps_standard_low_page_y_without_zn_alias() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x0099;
        let (page_x, page_y) = texture_page_origin(texture_page);

        assert_eq!((page_x, page_y), (576, 256));

        framebuffer.set_raw_pixel(page_x, 0, 0x0001);
        framebuffer.set_raw_pixel(page_x, page_y, 0x0002);
        framebuffer.set_raw_pixel(1, 0, 0x001f);
        framebuffer.set_raw_pixel(2, 0, 0x03e0);
        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            0,
            TextureCoordinate { u: 0, v: 0 },
            TextureDrawOptions::opaque_raw(),
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), 0x0000_ff00);
    }

    #[test]
    fn framebuffer_texture_sampling_uses_br2_stage_y256_alias_for_playfield_page() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x000c;
        let clut = 0x7d18;
        let (page_x, page_y) = texture_page_origin(texture_page);
        let clut_x = ((clut & 0x3f) as i32) * 16;
        let clut_y = ((clut >> 6) & 0x03ff) as i32;

        assert_eq!((page_x, page_y), (768, 0));
        assert_eq!((clut_x, clut_y), (384, 500));
        assert_eq!(
            texture_page_origin_for_clut(texture_page, clut),
            (704, PSX_VRAM_HEIGHT as i32 / 2)
        );
        assert_eq!(
            texture_page_origin_for_sample(texture_page, clut, 0, 0),
            (704, PSX_VRAM_HEIGHT as i32 / 2)
        );
        assert_eq!(
            texture_page_origin_for_sample(texture_page, clut, 64, 0),
            (704, PSX_VRAM_HEIGHT as i32 / 2)
        );
        assert_eq!(
            texture_page_raw_bounds_for_clut(texture_page, clut),
            (704, PSX_VRAM_HEIGHT as i32 / 2, 767, 511)
        );

        framebuffer.set_raw_pixel(page_x, PSX_VRAM_HEIGHT as i32 / 2, 0x0001);
        framebuffer.set_raw_pixel(page_x - 64, PSX_VRAM_HEIGHT as i32 / 2, 0x0002);
        framebuffer.set_raw_pixel(clut_x + 1, clut_y, 0x001f);
        framebuffer.set_raw_pixel(clut_x + 2, clut_y, 0x03e0);
        framebuffer.set_raw_pixel(clut_x + 3, clut_y, 0x7c00);
        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            clut,
            TextureCoordinate { u: 0, v: 0 },
            TextureDrawOptions::opaque_raw(),
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), 0x0000_ff00);
    }

    #[test]
    fn framebuffer_texture_sampling_uses_br2_stage_y256_alias_for_select_name_page() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x000c;
        let clut = 0x7d19;
        let (page_x, page_y) = texture_page_origin(texture_page);
        let clut_x = ((clut & 0x3f) as i32) * 16;
        let clut_y = ((clut >> 6) & 0x03ff) as i32;

        assert_eq!((page_x, page_y), (768, 0));
        assert_eq!((clut_x, clut_y), (400, 500));
        assert_eq!(
            texture_page_origin_for_clut(texture_page, clut),
            (704, PSX_VRAM_HEIGHT as i32 / 2)
        );
        assert_eq!(
            texture_page_origin_for_sample(texture_page, clut, 0, 0),
            (704, PSX_VRAM_HEIGHT as i32 / 2)
        );
        assert_eq!(
            texture_page_origin_for_sample(texture_page, clut, 64, 0),
            (704, PSX_VRAM_HEIGHT as i32 / 2)
        );

        framebuffer.set_raw_pixel(page_x, 0, 0x0001);
        framebuffer.set_raw_pixel(page_x - 64, PSX_VRAM_HEIGHT as i32 / 2, 0x0002);
        framebuffer.set_raw_pixel(page_x - 48, PSX_VRAM_HEIGHT as i32 / 2, 0x0003);
        framebuffer.set_raw_pixel(clut_x + 1, clut_y, 0x001f);
        framebuffer.set_raw_pixel(clut_x + 2, clut_y, 0x03e0);
        framebuffer.set_raw_pixel(clut_x + 3, clut_y, 0x7c00);
        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            clut,
            TextureCoordinate { u: 0, v: 0 },
            TextureDrawOptions::opaque_raw(),
            TextureWindow::default(),
        );
        framebuffer.draw_textured_rect(
            Point { x: 9, y: 8 },
            (1, 1),
            texture_page,
            clut,
            TextureCoordinate { u: 64, v: 0 },
            TextureDrawOptions::opaque_raw(),
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), 0x0000_ff00);
        assert_eq!(framebuffer.pixel(9, 8), 0x0000_00ff);
    }

    #[test]
    fn framebuffer_texture_sampling_uses_br2_stage_y256_alias_for_match_tree_page() {
        for (case, clut) in [0x7b1f, 0x7b5f].into_iter().enumerate() {
            let mut framebuffer = NativeFrameBuffer::default();
            let texture_page = 0x000c;
            let (page_x, page_y) = texture_page_origin(texture_page);
            let clut_x = ((clut & 0x3f) as i32) * 16;
            let clut_y = ((clut >> 6) & 0x03ff) as i32;

            assert_eq!((page_x, page_y), (768, 0));
            assert_eq!(clut_x, 496);
            assert!(matches!(clut_y, 492 | 493));
            assert_eq!(
                texture_page_origin_for_clut(texture_page, clut),
                (page_x, PSX_VRAM_HEIGHT as i32 / 2)
            );
            assert_eq!(
                texture_page_origin_for_sample(texture_page, clut, 0, 0),
                (page_x, PSX_VRAM_HEIGHT as i32 / 2)
            );

            framebuffer.set_raw_pixel(page_x, 0, 0x0001);
            framebuffer.set_raw_pixel(page_x, PSX_VRAM_HEIGHT as i32 / 2, 0x0002);
            framebuffer.set_raw_pixel(clut_x + 1, clut_y, 0x001f);
            framebuffer.set_raw_pixel(clut_x + 2, clut_y, 0x03e0);
            framebuffer.draw_textured_rect(
                Point {
                    x: 8 + i32::try_from(case).unwrap() * 2,
                    y: 8,
                },
                (1, 1),
                texture_page,
                clut,
                TextureCoordinate { u: 0, v: 0 },
                TextureDrawOptions::opaque_raw(),
                TextureWindow::default(),
            );

            assert_eq!(
                framebuffer.pixel(8 + i32::try_from(case).unwrap() * 2, 8),
                0x0000_ff00
            );
        }
    }

    #[test]
    fn framebuffer_texture_sampling_uses_br2_gameplay_body_y256_alias() {
        for (case, (texture_page, clut)) in
            [(0x000c, 0x7804), (0x000d, 0x7984)].into_iter().enumerate()
        {
            let mut framebuffer = NativeFrameBuffer::default();
            let (page_x, page_y) = texture_page_origin(texture_page);
            let clut_x = ((clut & 0x3f) as i32) * 16;
            let clut_y = ((clut >> 6) & 0x03ff) as i32;

            assert_eq!(page_y, 0);
            assert_eq!(clut_x, 64);
            assert!((480..=486).contains(&clut_y));
            assert_eq!(
                texture_page_origin_for_clut(texture_page, clut),
                (page_x, PSX_VRAM_HEIGHT as i32 / 2)
            );

            framebuffer.set_raw_pixel(page_x, 0, 0x0001);
            framebuffer.set_raw_pixel(page_x, PSX_VRAM_HEIGHT as i32 / 2, 0x0002);
            framebuffer.set_raw_pixel(clut_x + 1, clut_y, 0x001f);
            framebuffer.set_raw_pixel(clut_x + 2, clut_y, 0x03e0);
            framebuffer.draw_textured_rect(
                Point {
                    x: 8 + i32::try_from(case).unwrap(),
                    y: 8,
                },
                (1, 1),
                texture_page,
                clut,
                TextureCoordinate { u: 0, v: 0 },
                TextureDrawOptions::opaque_raw(),
                TextureWindow::default(),
            );

            assert_eq!(
                framebuffer.pixel(8 + i32::try_from(case).unwrap(), 8),
                0x0000_ff00
            );
        }
    }

    #[test]
    fn framebuffer_texture_descriptor_aliases_br2_stage_material_cluts() {
        assert_eq!(
            br2_texture_descriptor_alias(0x001c, 0x7a54),
            (0x001c, 0x7a50)
        );
        assert_eq!(
            br2_texture_descriptor_alias(0x001c, 0x7814),
            (0x001c, 0x7810)
        );
        assert_eq!(
            br2_texture_descriptor_alias(0x001c, 0x7911),
            (0x001c, 0x7910)
        );
        assert_eq!(
            br2_texture_descriptor_alias(0x001c, 0x7997),
            (0x001c, 0x7990)
        );
        assert_eq!(
            br2_texture_descriptor_alias(0x001c, 0x7850),
            (0x001c, 0x7850)
        );
        assert_eq!(
            br2_texture_descriptor_alias(0x000c, 0x7a54),
            (0x000c, 0x7a54)
        );
        assert_eq!(
            br2_texture_descriptor_alias(0x001c, 0x7b54),
            (0x001c, 0x7b54)
        );
        assert_eq!(
            br2_texture_descriptor_alias(0x001c, 0x7998),
            (0x001c, 0x7998)
        );
    }

    #[test]
    fn framebuffer_texture_descriptor_aliases_live_stage_material_cluts() {
        for clut in [0x7954, 0x7994, 0x79d4, 0x7a14] {
            assert_eq!(
                br2_texture_descriptor_alias(0x001c, clut),
                (0x001c, (clut & !0x003f) | 0x0010),
                "live stage material CLUTs must use the populated x=256 palette alias"
            );
        }
    }

    #[test]
    fn framebuffer_texture_descriptor_keeps_fighter_material_clut_page() {
        assert_eq!(
            br2_texture_descriptor_alias(0x021c, 0x79d4),
            (0x021c, 0x79d4),
            "fighter page 0x0200 must not inherit the stage x=256 palette alias"
        );
    }

    #[test]
    fn framebuffer_texture_descriptor_aliases_live_stage_packet_state() {
        assert_eq!(
            br2_texture_descriptor_alias(0x061c, 0x7954),
            (0x061c, 0x7950),
            "stage packet state bits must not suppress the live palette alias"
        );
    }

    #[test]
    fn framebuffer_texture_sampling_uses_live_stage_palette_alias() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x001c;
        let clut = 0x7954;
        let (page_x, page_y) = texture_page_origin(texture_page);
        let requested_x = ((clut & 0x3f) as i32) * 16;
        let requested_y = ((clut >> 6) & 0x03ff) as i32;
        let stage_alias_x = 256;

        assert_eq!((page_x, page_y), (768, 256));
        assert_eq!((requested_x, requested_y), (320, 485));

        framebuffer.set_raw_pixel(page_x, page_y, 0x0001);
        framebuffer.set_raw_pixel(stage_alias_x + 1, requested_y, 0x001f);
        framebuffer.set_raw_pixel(requested_x + 1, requested_y, 0x03e0);
        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            clut,
            TextureCoordinate { u: 0, v: 0 },
            TextureDrawOptions::opaque_raw(),
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), super::rgb555_to_rgb888(0x001f));
    }

    #[test]
    fn framebuffer_texture_descriptor_keeps_adjacent_stage_material_aliases() {
        for clut in [0x7914, 0x7a54] {
            let expected_clut = (clut & !0x003f) | 0x0010;
            assert_eq!(
                br2_texture_descriptor_alias(0x001c, clut),
                (0x001c, expected_clut),
                "non-model stage CLUT 0x{clut:04x} must retain the stage palette relocation"
            );
        }
    }

    #[test]
    fn framebuffer_texture_sampling_keeps_br2_gameplay_hud_page_on_y0() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x000e;
        let clut = 0x795e;
        let (page_x, page_y) = texture_page_origin(texture_page);
        let clut_x = ((clut & 0x3f) as i32) * 16;
        let clut_y = ((clut >> 6) & 0x03ff) as i32;

        assert_eq!(page_y, 0);
        assert_eq!(
            texture_page_origin_for_clut(texture_page, clut),
            (page_x, 0)
        );

        framebuffer.set_raw_pixel(page_x, 0, 0x0001);
        framebuffer.set_raw_pixel(page_x, PSX_VRAM_HEIGHT as i32 / 2, 0x0002);
        framebuffer.set_raw_pixel(clut_x + 1, clut_y, 0x001f);
        framebuffer.set_raw_pixel(clut_x + 2, clut_y, 0x03e0);
        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            clut,
            TextureCoordinate { u: 0, v: 0 },
            TextureDrawOptions::opaque_raw(),
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), 0x00ff_0000);
    }

    #[test]
    fn framebuffer_texture_sampling_keeps_br2_stage_000b_on_y0_by_default() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x000b;
        let clut = 0x781b;
        let (page_x, page_y) = texture_page_origin(texture_page);
        let clut_x = ((clut & 0x3f) as i32) * 16;
        let clut_y = ((clut >> 6) & 0x03ff) as i32;

        assert_eq!((page_x, page_y), (704, 0));
        assert_eq!((clut_x, clut_y), (432, 480));
        assert_eq!(texture_page_origin_for_clut(texture_page, clut), (704, 0));

        framebuffer.set_raw_pixel(page_x, 0, 0x0001);
        framebuffer.set_raw_pixel(page_x, PSX_VRAM_HEIGHT as i32 / 2, 0x0002);
        framebuffer.set_raw_pixel(clut_x + 1, clut_y, 0x001f);
        framebuffer.set_raw_pixel(clut_x + 2, clut_y, 0x03e0);
        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            clut,
            TextureCoordinate { u: 0, v: 0 },
            TextureDrawOptions::opaque_raw(),
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), 0x00ff_0000);
    }

    #[test]
    fn framebuffer_texture_sampling_keeps_low_playfield_page_y0_without_br2_clut_alias() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x000b;
        let clut = 0x781a;
        let (page_x, page_y) = texture_page_origin(texture_page);
        let clut_x = ((clut & 0x3f) as i32) * 16;
        let clut_y = ((clut >> 6) & 0x03ff) as i32;

        assert_eq!((page_x, page_y), (704, 0));
        assert_eq!(texture_page_origin_for_clut(texture_page, clut), (704, 0));

        framebuffer.set_raw_pixel(page_x, 0, 0x0001);
        framebuffer.set_raw_pixel(page_x, PSX_VRAM_HEIGHT as i32 / 2, 0x0002);
        framebuffer.set_raw_pixel(clut_x + 1, clut_y, 0x001f);
        framebuffer.set_raw_pixel(clut_x + 2, clut_y, 0x03e0);
        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            clut,
            TextureCoordinate { u: 0, v: 0 },
            TextureDrawOptions::opaque_raw(),
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), 0x00ff_0000);
    }

    #[test]
    fn framebuffer_texture_sampling_uses_br2_character_y256_alias_for_gameplay_clut() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x002e;
        let clut = 0x7b9e;
        let (page_x, page_y) = texture_page_origin(texture_page);
        let clut_x = ((clut & 0x3f) as i32) * 16;
        let clut_y = ((clut >> 6) & 0x03ff) as i32;

        assert_eq!((page_x, page_y), (896, 0));
        assert_eq!((clut_x, clut_y), (480, 494));
        assert_eq!(
            texture_page_origin_for_clut(texture_page, clut),
            (896, PSX_VRAM_HEIGHT as i32 / 2)
        );

        framebuffer.set_raw_pixel(page_x + 48, 0, 0x0001);
        framebuffer.set_raw_pixel(page_x + 48, PSX_VRAM_HEIGHT as i32 / 2, 0x0002);
        framebuffer.set_raw_pixel(clut_x + 1, clut_y, 0x001f);
        framebuffer.set_raw_pixel(clut_x + 2, clut_y, 0x03e0);
        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            clut,
            TextureCoordinate { u: 192, v: 0 },
            TextureDrawOptions::opaque_raw(),
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), 0x0000_ff00);
    }

    #[test]
    fn framebuffer_texture_sampling_uses_br2_effect_y256_alias_for_gameplay_clut() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x002f;
        let clut = 0x78df;
        let (page_x, page_y) = texture_page_origin(texture_page);
        let clut_x = ((clut & 0x3f) as i32) * 16;
        let clut_y = ((clut >> 6) & 0x03ff) as i32;

        assert_eq!((page_x, page_y), (960, 0));
        assert_eq!((clut_x, clut_y), (496, 483));
        assert_eq!(
            texture_page_origin_for_clut(texture_page, clut),
            (896, PSX_VRAM_HEIGHT as i32 / 2)
        );

        framebuffer.set_raw_pixel(page_x + 4, 0, 0x0001);
        framebuffer.set_raw_pixel(page_x + 4, PSX_VRAM_HEIGHT as i32 / 2, 0x0002);
        framebuffer.set_raw_pixel(page_x - 60, PSX_VRAM_HEIGHT as i32 / 2, 0x0003);
        framebuffer.set_raw_pixel(clut_x + 1, clut_y, 0x7c00);
        framebuffer.set_raw_pixel(clut_x + 2, clut_y, 0x03e0);
        framebuffer.set_raw_pixel(clut_x + 3, clut_y, 0x001f);
        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            clut,
            TextureCoordinate { u: 16, v: 0 },
            TextureDrawOptions::opaque_raw(),
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), 0x00ff_0000);
    }

    #[test]
    fn framebuffer_texture_sampling_uses_br2_effect_y256_alias_for_adjacent_gameplay_clut() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x002f;
        let clut = 0x791f;
        let (page_x, page_y) = texture_page_origin(texture_page);
        let clut_x = ((clut & 0x3f) as i32) * 16;
        let clut_y = ((clut >> 6) & 0x03ff) as i32;

        assert_eq!((page_x, page_y), (960, 0));
        assert_eq!((clut_x, clut_y), (496, 484));
        assert_eq!(
            texture_page_origin_for_clut(texture_page, clut),
            (896, PSX_VRAM_HEIGHT as i32 / 2)
        );

        framebuffer.set_raw_pixel(page_x + 18, 0, 0x0001);
        framebuffer.set_raw_pixel(page_x + 18, PSX_VRAM_HEIGHT as i32 / 2, 0x0002);
        framebuffer.set_raw_pixel(page_x - 46, PSX_VRAM_HEIGHT as i32 / 2, 0x0003);
        framebuffer.set_raw_pixel(clut_x + 1, clut_y, 0x7c00);
        framebuffer.set_raw_pixel(clut_x + 2, clut_y, 0x03e0);
        framebuffer.set_raw_pixel(clut_x + 3, clut_y, 0x001f);
        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            clut,
            TextureCoordinate { u: 72, v: 0 },
            TextureDrawOptions::opaque_raw(),
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), 0x00ff_0000);
    }

    #[test]
    fn framebuffer_texture_sampling_keeps_br2_stage_high_u_on_y256_alias() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x000c;
        let clut = 0x7d18;
        let (page_x, _page_y) = texture_page_origin(texture_page);
        let clut_x = ((clut & 0x3f) as i32) * 16;
        let clut_y = ((clut >> 6) & 0x03ff) as i32;

        assert_eq!(
            texture_page_origin_for_clut(texture_page, clut),
            (704, PSX_VRAM_HEIGHT as i32 / 2)
        );
        assert_eq!(
            texture_page_origin_for_sample(texture_page, clut, 64, 0),
            (704, PSX_VRAM_HEIGHT as i32 / 2)
        );

        framebuffer.set_raw_pixel(page_x - 48, PSX_VRAM_HEIGHT as i32 / 2, 0x0001);
        framebuffer.set_raw_pixel(page_x - 48, 0, 0x0002);
        framebuffer.set_raw_pixel(clut_x + 1, clut_y, 0x001f);
        framebuffer.set_raw_pixel(clut_x + 2, clut_y, 0x03e0);
        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            clut,
            TextureCoordinate { u: 64, v: 0 },
            TextureDrawOptions::opaque_raw(),
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), 0x00ff_0000);
    }

    #[test]
    fn framebuffer_texture_sampling_recovers_br2_stage_low_color_polygon_clut_base_row() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x001a;
        let clut = 0x7ede;
        let (page_x, page_y) = texture_page_origin(texture_page);
        let clut_x = ((clut & 0x3f) as i32) * 16;
        let clut_y = ((clut >> 6) & 0x03ff) as i32;
        let base_y = clut_y & !0x1f;
        let options = TextureDrawOptions::opaque_raw();

        assert_eq!((clut_x, clut_y, base_y), (480, 507, 480));

        framebuffer.set_raw_pixel(page_x, page_y, 0x0002);
        framebuffer.set_raw_pixel(clut_x + 2, clut_y, 0x001f);
        for index in 1..16 {
            framebuffer.set_raw_pixel(clut_x + index, base_y, 0x0400 + index as u16);
        }
        framebuffer.set_raw_pixel(clut_x + 2, base_y, 0x03e0);
        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            clut,
            TextureCoordinate { u: 0, v: 0 },
            options,
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), 0x0000_ff00);
    }

    #[test]
    fn framebuffer_texture_sampling_keeps_non_br2_low_color_polygon_clut_requested_row() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x001a;
        let clut = 0x7e5e;
        let (page_x, page_y) = texture_page_origin(texture_page);
        let clut_x = ((clut & 0x3f) as i32) * 16;
        let clut_y = ((clut >> 6) & 0x03ff) as i32;
        let base_y = clut_y & !0x1f;
        let options = TextureDrawOptions::opaque_raw();

        assert_eq!((clut_x, clut_y, base_y), (480, 505, 480));

        framebuffer.set_raw_pixel(page_x, page_y, 0x0002);
        framebuffer.set_raw_pixel(clut_x + 2, clut_y, 0x001f);
        for index in 1..16 {
            framebuffer.set_raw_pixel(clut_x + index, base_y, 0x0400 + index as u16);
        }
        framebuffer.set_raw_pixel(clut_x + 2, base_y, 0x03e0);
        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            clut,
            TextureCoordinate { u: 0, v: 0 },
            options,
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), 0x00ff_0000);
    }

    #[test]
    fn framebuffer_textured_draw_snapshots_br2_stage_alias_self_overlap() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x000c;
        let clut = 0x7d18;
        let (page_x, _page_y) = texture_page_origin(texture_page);
        let alias_y = PSX_VRAM_HEIGHT as i32 / 2;
        let alias_x = page_x - 64;
        let clut_x = ((clut & 0x3f) as i32) * 16;
        let clut_y = ((clut >> 6) & 0x03ff) as i32;

        assert_eq!(
            texture_page_raw_bounds_for_clut(texture_page, clut),
            (alias_x, alias_y, page_x - 1, 511)
        );

        framebuffer.set_raw_pixel(alias_x, alias_y, 0x2221);
        framebuffer.set_raw_pixel(clut_x + 1, clut_y, 0x001f);
        framebuffer.set_raw_pixel(clut_x + 2, clut_y, 0x03e0);
        framebuffer.draw_textured_rect(
            Point {
                x: alias_x,
                y: alias_y,
            },
            (4, 1),
            texture_page,
            clut,
            TextureCoordinate { u: 0, v: 0 },
            TextureDrawOptions::opaque_raw(),
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(alias_x, alias_y), 0x00ff_0000);
        assert_eq!(framebuffer.pixel(alias_x + 1, alias_y), 0x0000_ff00);
        assert_eq!(framebuffer.pixel(alias_x + 2, alias_y), 0x0000_ff00);
        assert_eq!(framebuffer.pixel(alias_x + 3, alias_y), 0x0000_ff00);
    }

    #[test]
    fn framebuffer_texture_sampling_keeps_adjacent_4bpp_page_y0() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x000e;
        let clut = 0x7c1f;
        let (page_x, page_y) = texture_page_origin(texture_page);
        let clut_x = ((clut & 0x3f) as i32) * 16;
        let clut_y = ((clut >> 6) & 0x03ff) as i32;

        assert_eq!((page_x, page_y), (896, 0));

        framebuffer.set_raw_pixel(page_x, page_y, 0x0001);
        framebuffer.set_raw_pixel(page_x, PSX_VRAM_HEIGHT as i32 / 2, 0x0002);
        framebuffer.set_raw_pixel(clut_x + 1, clut_y, 0x001f);
        framebuffer.set_raw_pixel(clut_x + 2, clut_y, 0x03e0);
        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            clut,
            TextureCoordinate { u: 0, v: 0 },
            TextureDrawOptions::opaque_raw(),
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), 0x00ff_0000);
    }

    #[test]
    fn framebuffer_low_bank_y0_alias_can_preserve_odd_page_x_for_diagnostics() {
        let texture_page = 0x0299;

        assert_eq!(low_bank_y0_alias_page_x(texture_page, false), 512);
        assert_eq!(low_bank_y0_alias_page_x(texture_page, true), 576);
    }

    #[test]
    fn framebuffer_texture_sampling_uses_zn_type2_texture_page_origin() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x2e80;
        let (page_x, page_y) = texture_page_origin(texture_page);

        assert_eq!((page_x, page_y), (0, 512));

        framebuffer.set_raw_pixel(page_x + 64, page_y, 0x0201);
        framebuffer.set_raw_pixel(1, 0, 0x001f);
        framebuffer.set_raw_pixel(2, 0, 0x03e0);
        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (2, 1),
            texture_page,
            0,
            TextureCoordinate { u: 128, v: 0 },
            TextureDrawOptions::opaque_raw(),
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), 0x00ff_0000);
        assert_eq!(framebuffer.pixel(9, 8), 0x0000_ff00);
    }

    #[test]
    fn framebuffer_zn_extended_8bpp_candidate_widths_split_adjacent_uploads() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x2e80;
        let clut = 0;
        let (page_x, page_y) = texture_page_origin(texture_page);

        assert_eq!((page_x, page_y), (0, 512));

        framebuffer.set_raw_pixel(page_x, page_y, 0x0201);
        framebuffer.set_raw_pixel(page_x + 64, page_y, 0x0403);
        framebuffer.set_raw_pixel(1, 0, 0x001f);
        framebuffer.set_raw_pixel(2, 0, 0x03e0);
        framebuffer.set_raw_pixel(3, 0, 0x7c00);
        framebuffer.set_raw_pixel(4, 0, 0x03ff);

        let native_width = framebuffer
            .sample_texture_sample_from_origin(
                &framebuffer.raw_pixels,
                texture_page,
                clut,
                128,
                0,
                page_x,
                page_y,
                128,
                true,
            )
            .color;
        let wrapped_width = framebuffer
            .sample_texture_sample_from_origin(
                &framebuffer.raw_pixels,
                texture_page,
                clut,
                128,
                0,
                page_x,
                page_y,
                64,
                true,
            )
            .color;
        let candidate_images = framebuffer.texture_candidate_images(texture_page, clut);

        assert_eq!(native_width, 0x7c00);
        assert_eq!(wrapped_width, 0x001f);
        assert!(
            candidate_images
                .iter()
                .any(|candidate| candidate.label == "resolved-raw64")
        );
        assert!(
            candidate_images
                .iter()
                .any(|candidate| candidate.label == "resolved_plus_64-raw128")
        );
    }

    #[test]
    fn framebuffer_texture_sampling_uses_zn_tiled_clut_for_textured_polygons() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x2e80;
        let clut = 0x795a;
        let (page_x, page_y) = texture_page_origin(texture_page);
        let clut_x = ((clut & 0x3f) as i32) * 16;
        let clut_y = ((clut >> 6) & 0x03ff) as i32;
        let tiled_y = clut_y & !0x1f;
        let index = 0x2a;

        assert_eq!((page_x, page_y), (0, 512));
        assert_eq!((clut_x, tiled_y), (416, 480));

        framebuffer.set_raw_pixel(page_x, page_y, index as u16);
        framebuffer.set_raw_pixel(clut_x + index, clut_y, 0x001f);
        for palette_index in 1..180 {
            let col = palette_index & 0x0f;
            let row = palette_index / 16;
            framebuffer.set_raw_pixel(clut_x + col, tiled_y + row, 0x03e0);
        }
        framebuffer.set_raw_pixel(clut_x + (index & 0x0f), tiled_y + (index / 16), 0x03e0);

        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            clut,
            TextureCoordinate { u: 0, v: 0 },
            TextureDrawOptions::opaque_raw(),
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), 0x0000_ff00);
    }

    #[test]
    fn framebuffer_texture_sampling_honors_disabled_zn_tiled_clut_fallback() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x2e80;
        let clut = 0x795a;
        let (page_x, page_y) = texture_page_origin(texture_page);
        let clut_x = ((clut & 0x3f) as i32) * 16;
        let clut_y = ((clut >> 6) & 0x03ff) as i32;
        let tiled_y = clut_y & !0x1f;
        let index = 0x2a;
        let mut options = TextureDrawOptions::opaque_raw();
        options.allow_palette_fallback = false;

        assert_eq!((page_x, page_y), (0, 512));
        assert_eq!((clut_x, clut_y, tiled_y), (416, 485, 480));

        framebuffer.set_raw_pixel(page_x, page_y, index as u16);
        framebuffer.set_raw_pixel(clut_x + index, clut_y, 0x001f);
        for palette_index in 1..245 {
            let col = palette_index & 0x0f;
            let row = palette_index / 16;
            framebuffer.set_raw_pixel(clut_x + col, tiled_y + row, 0x03e0);
        }
        framebuffer.set_raw_pixel(clut_x + (index & 0x0f), tiled_y + (index / 16), 0x03e0);

        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            clut,
            TextureCoordinate { u: 0, v: 0 },
            options,
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), 0x00ff_0000);
    }

    #[test]
    fn framebuffer_texture_sampling_forces_blank_high_zn_256_clut_tile_for_polygons() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x2e80;
        let clut = 0x795a;
        let (page_x, page_y) = texture_page_origin(texture_page);
        let clut_x = ((clut & 0x3f) as i32) * 16;
        let clut_y = ((clut >> 6) & 0x03ff) as i32;
        let tiled_y = clut_y & !0x1f;
        let index = 0x2a;
        let mut options = TextureDrawOptions::opaque_raw();
        options.allow_palette_fallback = false;

        assert_eq!((page_x, page_y), (0, 512));
        assert_eq!((clut_x, clut_y, tiled_y), (416, 485, 480));

        framebuffer.set_raw_pixel(page_x, page_y, index as u16);
        for palette_index in 1..160 {
            let col = palette_index & 0x0f;
            let row = palette_index / 16;
            framebuffer.set_raw_pixel(clut_x + col, tiled_y + row, 0x03e0);
        }
        framebuffer.set_raw_pixel(clut_x + (index & 0x0f), tiled_y + (index / 16), 0x03e0);

        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            clut,
            TextureCoordinate { u: 0, v: 0 },
            options,
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), 0x0000_ff00);
    }

    #[test]
    fn framebuffer_texture_sampling_rejects_far_zn_high_clut_tile_alias() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x2e80;
        let clut = 0x795a;
        let (page_x, page_y) = texture_page_origin(texture_page);
        let clut_x = ((clut & 0x3f) as i32) * 16;
        let clut_y = ((clut >> 6) & 0x03ff) as i32;
        let tiled_y = clut_y & !0x1f;
        let index = 0x2a;

        assert_eq!((page_x, page_y), (0, 512));
        assert_eq!((clut_x, clut_y, tiled_y), (416, 485, 480));
        assert!(
            !tiled_256_palette_x_candidates(
                clut_x,
                preferred_tiled_256_palette_x(texture_page),
                None,
            )
            .contains(&800)
        );

        framebuffer.set_raw_pixel(page_x, page_y, index as u16);

        for palette_index in 1..196 {
            let col = palette_index & 0x0f;
            let row = palette_index / 16;
            framebuffer.set_raw_pixel(clut_x + col, tiled_y + row, 0x03e0);
        }
        for palette_index in 1..246 {
            let col = palette_index & 0x0f;
            let row = palette_index / 16;
            framebuffer.set_raw_pixel(clut_x + 16 + col, tiled_y + row, 0x001f);
        }
        for palette_index in 1..256 {
            let col = palette_index & 0x0f;
            let row = palette_index / 16;
            framebuffer.set_raw_pixel(800 + col, tiled_y + row, 0x7c00);
        }
        framebuffer.set_raw_pixel(clut_x + (index & 0x0f), tiled_y + (index / 16), 0x03e0);
        framebuffer.set_raw_pixel(clut_x + 16 + (index & 0x0f), tiled_y + (index / 16), 0x001f);
        framebuffer.set_raw_pixel(800 + (index & 0x0f), tiled_y + (index / 16), 0x7c00);

        let candidate =
            fallback_tiled_256_palette_candidate(&framebuffer.raw_pixels, texture_page, clut)
                .expect("expected nearby fallback palette candidate");

        assert_ne!((candidate.x, candidate.y), (800, tiled_y));
        assert!(candidate.x < 512);

        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            clut,
            TextureCoordinate { u: 0, v: 0 },
            TextureDrawOptions::opaque_raw(),
            TextureWindow::default(),
        );

        assert_ne!(framebuffer.pixel(8, 8), 0x0000_00ff);
    }

    #[test]
    fn framebuffer_texture_sampling_recovers_blank_zn_clut_alias() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x000d;
        let clut = 0x7c17;
        let page_x = ((texture_page & 0x0f) as i32) * 64;
        let clut_x = ((clut & 0x3f) as i32) * 16;
        let clut_y = ((clut >> 6) & 0x03ff) as i32;

        framebuffer.set_raw_pixel(page_x, 0, 0x0001);
        framebuffer.set_raw_pixel(clut_x + 1, clut_y - 16, 0x001f);
        for index in 2..16 {
            framebuffer.set_raw_pixel(clut_x + index, clut_y - 16, 0x03e0);
        }
        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            clut,
            TextureCoordinate { u: 0, v: 0 },
            TextureDrawOptions::opaque_raw(),
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), 0x00ff_0000);
    }

    #[test]
    fn framebuffer_texture_sampling_can_disable_blank_clut_alias_fallback() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x000d;
        let clut = 0x7c17;
        let page_x = ((texture_page & 0x0f) as i32) * 64;
        let clut_x = ((clut & 0x3f) as i32) * 16;
        let clut_y = ((clut >> 6) & 0x03ff) as i32;
        let mut options = TextureDrawOptions::opaque_raw();
        options.allow_palette_fallback = false;

        framebuffer.set_raw_pixel(page_x, 0, 0x0001);
        framebuffer.set_raw_pixel(clut_x + 1, clut_y - 16, 0x001f);
        for index in 2..16 {
            framebuffer.set_raw_pixel(clut_x + index, clut_y - 16, 0x03e0);
        }
        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            clut,
            TextureCoordinate { u: 0, v: 0 },
            options,
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), 0);
    }

    #[test]
    fn framebuffer_texture_sampling_prefers_requested_clut_when_present() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x000d;
        let clut = 0x7c17;
        let page_x = ((texture_page & 0x0f) as i32) * 64;
        let clut_x = ((clut & 0x3f) as i32) * 16;
        let clut_y = ((clut >> 6) & 0x03ff) as i32;

        framebuffer.set_raw_pixel(page_x, 0, 0x0001);
        framebuffer.set_raw_pixel(clut_x + 1, clut_y, 0x03e0);
        framebuffer.set_raw_pixel(clut_x + 1, clut_y - 16, 0x001f);
        for index in 2..16 {
            framebuffer.set_raw_pixel(clut_x + index, clut_y - 16, 0x001f);
        }
        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            clut,
            TextureCoordinate { u: 0, v: 0 },
            TextureDrawOptions::opaque_raw(),
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), 0x0000_ff00);
    }

    #[test]
    fn framebuffer_texture_sampling_recovers_sparse_zn_4bpp_clut_base_row() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x020a;
        let clut = 0x7c80;
        let (page_x, page_y) = texture_page_origin(texture_page);
        let clut_x = ((clut & 0x3f) as i32) * 16;
        let clut_y = ((clut >> 6) & 0x03ff) as i32;
        let base_y = clut_y & !0x0f;

        assert_eq!((page_x, page_y), (640, 0));
        assert_eq!((clut_x, clut_y, base_y), (0, 498, 496));

        framebuffer.set_raw_pixel(page_x, page_y, 0x0001);
        for index in 1..=7 {
            framebuffer.set_raw_pixel(clut_x + index, clut_y, 0x1000);
        }
        for index in 1..=14 {
            framebuffer.set_raw_pixel(clut_x + index, base_y, 0x0400 + index as u16);
        }
        framebuffer.set_raw_pixel(clut_x + 1, clut_y, 0x001f);
        framebuffer.set_raw_pixel(clut_x + 1, base_y, 0x03e0);

        let requested_png = framebuffer.texture_palette_png(texture_page, clut);
        let resolved_png = framebuffer.resolved_texture_palette_png(texture_page, clut);
        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            clut,
            TextureCoordinate { u: 0, v: 0 },
            TextureDrawOptions::opaque_raw(),
            TextureWindow::default(),
        );

        assert_ne!(requested_png, resolved_png);
        assert_eq!(framebuffer.pixel(8, 8), 0x0000_ff00);
    }

    #[test]
    fn framebuffer_texture_sampling_draws_nonzero_zn_4bpp_clut_zero_color() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x020a;
        let clut = 0x7c80;
        let (page_x, page_y) = texture_page_origin(texture_page);
        let clut_x = ((clut & 0x3f) as i32) * 16;
        let clut_y = ((clut >> 6) & 0x03ff) as i32;

        framebuffer.set_raw_pixel(page_x, page_y, 0x0000);
        framebuffer.set_raw_pixel(clut_x, clut_y, 0x001f);
        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            clut,
            TextureCoordinate { u: 0, v: 0 },
            TextureDrawOptions::opaque_raw(),
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), 0x00ff_0000);
    }

    #[test]
    fn framebuffer_texture_sampling_keeps_dense_zn_4bpp_requested_clut() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x020a;
        let clut = 0x7b1f;
        let (page_x, page_y) = texture_page_origin(texture_page);
        let clut_x = ((clut & 0x3f) as i32) * 16;
        let clut_y = ((clut >> 6) & 0x03ff) as i32;
        let base_y = clut_y & !0x0f;

        assert_eq!((page_x, page_y), (640, 0));
        assert_eq!((clut_x, clut_y, base_y), (496, 492, 480));

        framebuffer.set_raw_pixel(page_x, page_y, 0x0001);
        for index in 1..=15 {
            framebuffer.set_raw_pixel(clut_x + index, clut_y, 0x0400 + index as u16);
            framebuffer.set_raw_pixel(clut_x + index, base_y, 0x0800 + index as u16);
        }
        framebuffer.set_raw_pixel(clut_x + 1, clut_y, 0x001f);
        framebuffer.set_raw_pixel(clut_x + 1, base_y, 0x03e0);

        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            clut,
            TextureCoordinate { u: 0, v: 0 },
            TextureDrawOptions::opaque_raw(),
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), 0x00ff_0000);
    }

    #[test]
    fn framebuffer_texture_sampling_recovers_blank_zn_256_clut_alias() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x0299;
        let clut = 0x7c80;
        let (page_x, page_y) = texture_page_origin(texture_page);
        let clut_y = ((clut >> 6) & 0x03ff) as i32;
        let alias_x = 384;
        let alias_y = clut_y & !0x0f;

        assert_eq!((page_x, page_y), (576, 0));

        framebuffer.set_raw_pixel(page_x, page_y, 0x002a);
        for index in 1..256 {
            framebuffer.set_raw_pixel(alias_x + (index & 0x0f), alias_y + (index / 16), 0x03e0);
        }
        framebuffer.set_raw_pixel(alias_x + 0x0a, alias_y + 0x02, 0x001f);
        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            clut,
            TextureCoordinate { u: 0, v: 0 },
            TextureDrawOptions::opaque_raw(),
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), 0x00ff_0000);
    }

    #[test]
    fn framebuffer_resolved_palette_png_uses_blank_zn_256_clut_alias() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x0299;
        let clut = 0x7c80;
        let clut_y = ((clut >> 6) & 0x03ff) as i32;
        let alias_x = 384;
        let alias_y = clut_y & !0x0f;

        for index in 1..256 {
            framebuffer.set_raw_pixel(alias_x + (index & 0x0f), alias_y + (index / 16), 0x03e0);
        }

        let candidate =
            fallback_tiled_256_palette_candidate(&framebuffer.raw_pixels, texture_page, clut)
                .expect("expected fallback palette candidate");
        let requested_png = framebuffer.texture_palette_png(texture_page, clut);
        let resolved_png = framebuffer.resolved_texture_palette_png(texture_page, clut);

        assert_eq!((candidate.x, candidate.y), (alias_x, alias_y));
        assert_ne!(requested_png, resolved_png);
    }

    #[test]
    fn br2_character_model_palette_alias_can_be_disabled() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x0039;
        let clut = 0x7a5a;
        let requested_x = ((clut & 0x3f) as i32) * 16;
        let requested_y = ((clut >> 6) & 0x03ff) as i32;
        let alias_x = requested_x + 16;

        for index in 0..16 {
            framebuffer.set_raw_pixel(alias_x + index, requested_y, 0x0400 + index as u16);
        }

        assert!(
            br2_character_model_palette_candidate_with_policy(
                &framebuffer.raw_pixels,
                texture_page,
                clut,
                true,
            )
            .is_some()
        );
        assert_eq!(
            br2_character_model_palette_candidate_with_policy(
                &framebuffer.raw_pixels,
                texture_page,
                clut,
                false,
            ),
            None
        );
    }

    #[test]
    fn br2_character_model_palette_alias_rejects_low_diversity_rows() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x0039;
        let clut = 0x7a9a;
        let requested_x = ((clut & 0x3f) as i32) * 16;
        let requested_y = ((clut >> 6) & 0x03ff) as i32;
        let alias_x = requested_x + 16;

        for index in 1..16 {
            framebuffer.set_raw_pixel(requested_x + index, requested_y, 0x0400 + index as u16);
            framebuffer.set_raw_pixel(alias_x + index, requested_y, 0x001f);
        }

        assert_eq!(
            br2_character_model_palette_candidate_with_policy(
                &framebuffer.raw_pixels,
                texture_page,
                clut,
                true,
            ),
            None
        );

        let sample = super::indexed_palette_sample_from(
            &framebuffer.raw_pixels,
            texture_page,
            clut,
            1,
            16,
            true,
        );
        assert!(!sample.palette_fallback);
        assert_eq!(sample.color, 0x0401);
    }

    #[test]
    fn br2_character_model_palette_uses_rich_adjacent_upload_over_texture_data_row() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x0039;
        let clut = 0x7a9a;
        let requested_x = ((clut & 0x3f) as i32) * 16;
        let requested_y = ((clut >> 6) & 0x03ff) as i32;
        let uploaded_x = requested_x + 16;
        let texture_data = [
            0x1d55, 0x1e15, 0x307f, 0x07bc, 0x291c, 0x2db7, 0x35c9, 0x72b0, 0x0e71, 0x30bb, 0x21dc,
            0x21dc, 0x21bc, 0x0aa0, 0x2193, 0x00fd,
        ];

        for (index, color) in texture_data.into_iter().enumerate() {
            framebuffer.set_raw_pixel(requested_x + index as i32, requested_y, color);
            framebuffer.set_raw_pixel(
                uploaded_x + index as i32,
                requested_y,
                if index == 0 {
                    0
                } else {
                    0x0400 + index as u16 * 0x21
                },
            );
        }

        let candidate = br2_character_model_palette_candidate_with_policy(
            &framebuffer.raw_pixels,
            texture_page,
            clut,
            true,
        )
        .expect("the adjacent uploaded material palette must be selected");
        assert_eq!((candidate.x, candidate.y), (uploaded_x, requested_y));

        framebuffer.set_raw_pixel(576, 256, 0x0001);
        let sample = framebuffer.sample_texture_sample_from(
            &framebuffer.raw_pixels,
            texture_page,
            clut,
            0,
            0,
            TextureSamplingPolicy::new(true, true),
        );
        assert!(sample.palette_fallback);
        assert_eq!(sample.color, 0x0421);
    }

    #[test]
    fn br2_captured_low_bank_fighter_palette_rejects_implausibly_dark_requested_clut() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x020b;
        let clut = 0x7903;
        let requested_x = ((clut & 0x3f) as i32) * 16;
        let requested_y = ((clut >> 6) & 0x03ff) as i32;
        let candidate_x = requested_x + 16;

        for (index, color) in [
            0x10c9, 0x1ce7, 0x0018, 0x10cc, 0x0019, 0x1108, 0x001a, 0x4d01, 0x001b, 0x2108, 0x10cd,
            0x001c, 0x1129, 0x150b, 0x001d, 0x10ee,
        ]
        .into_iter()
        .enumerate()
        {
            framebuffer.set_raw_pixel(requested_x + index as i32, requested_y, color);
        }
        for (index, color) in [
            0x2529, 0x001e, 0x112a, 0x001f, 0x0cd3, 0x112d, 0x150f, 0x294a, 0x154b, 0x045f, 0x2d6b,
            0x1931, 0x194f, 0x4588, 0x158c, 0x65a2,
        ]
        .into_iter()
        .enumerate()
        {
            framebuffer.set_raw_pixel(candidate_x + index as i32, requested_y, color);
        }

        let candidate = br2_character_model_palette_candidate_with_policy(
            &framebuffer.raw_pixels,
            texture_page,
            clut,
            true,
        )
        .expect("the captured near-black fighter CLUT must use its uploaded material row");
        assert_eq!((candidate.x, candidate.y), (candidate_x, requested_y));

        framebuffer.set_raw_pixel(704, 0, 0x0001);
        let sample = framebuffer.sample_texture_sample_from(
            &framebuffer.raw_pixels,
            texture_page,
            clut,
            0,
            0,
            TextureSamplingPolicy::new(true, true),
        );
        assert_eq!(sample.color, 0x001e);
        assert!(sample.palette_fallback);
    }

    #[test]
    fn br2_captured_low_bank_fighter_palette_recovers_requested_index_zero() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x020b;
        let clut = 0x7903;
        let requested_x = ((clut & 0x3f) as i32) * 16;
        let requested_y = ((clut >> 6) & 0x03ff) as i32;
        let candidate_x = requested_x + 16;

        for (index, color) in [
            0x10c9, 0x1ce7, 0x0018, 0x10cc, 0x0019, 0x1108, 0x001a, 0x4d01, 0x001b, 0x2108, 0x10cd,
            0x001c, 0x1129, 0x150b, 0x001d, 0x10ee,
        ]
        .into_iter()
        .enumerate()
        {
            framebuffer.set_raw_pixel(requested_x + index as i32, requested_y, color);
        }
        for (index, color) in [
            0x2529, 0x001e, 0x112a, 0x001f, 0x0cd3, 0x112d, 0x150f, 0x294a, 0x154b, 0x045f, 0x2d6b,
            0x1931, 0x194f, 0x4588, 0x158c, 0x65a2,
        ]
        .into_iter()
        .enumerate()
        {
            framebuffer.set_raw_pixel(candidate_x + index as i32, requested_y, color);
        }

        let sample = super::indexed_palette_sample_from(
            &framebuffer.raw_pixels,
            texture_page,
            clut,
            0,
            16,
            true,
        );
        assert_eq!(sample.color, 0x2529);
        assert!(sample.zero_texel);
        assert!(sample.palette_fallback);
        assert!(sample.clut_nonzero);

        framebuffer.set_raw_pixel(704, 0, 0x0000);
        framebuffer.set_raw_pixel(8, 8, 0x7c00);
        let stats = framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            clut,
            TextureCoordinate { u: 0, v: 0 },
            TextureDrawOptions::opaque_raw(),
            TextureWindow::default(),
        );

        assert_eq!(stats.zero_texel_samples, 1);
        assert_eq!(stats.transparent_pixels, 0);
        assert_eq!(stats.palette_fallback_samples, 1);
        assert_ne!(framebuffer.raw_pixel(8, 8), 0x7c00);
    }

    #[test]
    fn br2_captured_low_bank_fighter_palette_recovers_0x7883_clut() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x020b;
        let clut = 0x7883;
        let requested_x = ((clut & 0x3f) as i32) * 16;
        let requested_y = ((clut >> 6) & 0x03ff) as i32;

        for (index, color) in [
            0x10c9, 0x1ce7, 0x0018, 0x10cc, 0x0019, 0x1108, 0x001a, 0x4d01, 0x001b, 0x2108, 0x10cd,
            0x001c, 0x1129, 0x150b, 0x001d, 0x001d,
        ]
        .into_iter()
        .enumerate()
        {
            framebuffer.set_raw_pixel(requested_x + index as i32, requested_y, color);
            framebuffer.set_raw_pixel(32 + index as i32, requested_y, 0x3000 + index as u16 * 0x21);
        }

        let candidate = br2_character_model_palette_candidate_with_policy(
            &framebuffer.raw_pixels,
            texture_page,
            clut,
            true,
        )
        .expect("the captured near-black 0x7883 CLUT must use the uploaded material row");
        assert_eq!((candidate.x, candidate.y), (32, requested_y));

        framebuffer.set_raw_pixel(704, 0, 0x0001);
        let sample = framebuffer.sample_texture_sample_from(
            &framebuffer.raw_pixels,
            texture_page,
            clut,
            0,
            0,
            TextureSamplingPolicy::new(true, true),
        );
        assert_eq!(sample.color, 0x3021);
        assert!(sample.palette_fallback);
    }

    #[test]
    fn br2_low_bank_fighter_palette_does_not_replace_coherent_requested_bank() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x020b;
        let clut = 0x7903;
        let requested_x = ((clut & 0x3f) as i32) * 16;
        let requested_y = ((clut >> 6) & 0x03ff) as i32;

        for index in 0..16 {
            framebuffer.set_raw_pixel(
                requested_x + index,
                requested_y,
                0x3000 + index as u16 * 0x21,
            );
            framebuffer.set_raw_pixel(64 + index, requested_y, 0x7c00 + index as u16);
        }

        let candidate = br2_character_model_palette_candidate_with_policy(
            &framebuffer.raw_pixels,
            texture_page,
            clut,
            true,
        );
        assert_eq!(candidate, None);
    }

    #[test]
    fn br2_captured_yugo_palette_recovers_shared_material_bank() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x0208;
        let clut = 0x7a00;
        let requested_x = ((clut & 0x3f) as i32) * 16;
        let requested_y = ((clut >> 6) & 0x03ff) as i32;

        for (index, color) in [
            0x0421, 0x0421, 0x0001, 0x0002, 0x0800, 0x0003, 0x0421, 0x0420, 0x0004, 0x0005, 0x0022,
            0x0006, 0x1420, 0x0442, 0x0006, 0x0c40,
        ]
        .into_iter()
        .enumerate()
        {
            framebuffer.set_raw_pixel(requested_x + index as i32, requested_y, color);
            framebuffer.set_raw_pixel(
                requested_x + index as i32,
                requested_y - 16,
                if index.is_multiple_of(2) {
                    0x18c6
                } else {
                    0x1084
                },
            );
        }
        for (index, color) in [
            0x0008, 0x1841, 0x0044, 0x0843, 0x0009, 0x000a, 0x000b, 0x1081, 0x2461, 0x0863, 0x000c,
            0x0429, 0x0066, 0x1c64, 0x000d, 0x2882,
        ]
        .into_iter()
        .enumerate()
        {
            framebuffer.set_raw_pixel(16 + index as i32, requested_y, color);
        }
        for (index, color) in [
            0x000e, 0x0c85, 0x1ca2, 0x14c0, 0x000f, 0x0088, 0x0011, 0x3082, 0x0012, 0x10a6, 0x0089,
            0x34a2, 0x1900, 0x0013, 0x20e3, 0x0014,
        ]
        .into_iter()
        .enumerate()
        {
            framebuffer.set_raw_pixel(32 + index as i32, requested_y, color);
        }

        let candidate = br2_character_model_palette_candidate_with_policy(
            &framebuffer.raw_pixels,
            texture_page,
            clut,
            true,
        )
        .expect("the captured Yugo texture row must resolve to its previous material row");
        assert_eq!((candidate.x, candidate.y), (requested_x, requested_y - 16));

        let scalar =
            indexed_palette_sample_from(&framebuffer.raw_pixels, texture_page, clut, 2, 16, true);
        assert_eq!(scalar.color, 0x18c6);
        assert!(scalar.palette_fallback);

        let prepared = prepared_indexed_palette_samples_from(
            &framebuffer.raw_pixels,
            texture_page,
            clut,
            16,
            true,
        );
        assert_eq!(prepared[2].color, 0x18c6);
        assert!(prepared[2].palette_fallback);
    }

    #[test]
    fn br2_recovery_palette_history_survives_material_row_overwrite() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x0208;
        let clut = 0x7a00;
        let requested_x = ((clut & 0x3f) as i32) * 16;
        let requested_y = ((clut >> 6) & 0x03ff) as i32;
        let previous_y = requested_y - 16;
        let material = [0x18e9, 0x18c6, 0x1ce6];

        for (index, color) in [
            0x0421, 0x0421, 0x0001, 0x0002, 0x0800, 0x0003, 0x0421, 0x0420, 0x0004, 0x0005, 0x0022,
            0x0006, 0x1420, 0x0442, 0x0006, 0x0c40,
        ]
        .into_iter()
        .enumerate()
        {
            framebuffer.set_raw_pixel(requested_x + index as i32, requested_y, color);
            framebuffer.set_raw_pixel(
                requested_x + index as i32,
                previous_y,
                material[index % material.len()],
            );
        }

        let initial = prepared_indexed_palette_samples_with_history(
            &framebuffer.raw_pixels,
            texture_page,
            clut,
            16,
            true,
            Some(&framebuffer.recovery_palette_history),
        );
        assert_eq!(initial[2].color, material[2]);

        for index in 0..16 {
            framebuffer.set_raw_pixel(requested_x + index, previous_y, 0);
            framebuffer.set_raw_pixel(16 + index, requested_y, 1 + index as u16);
            framebuffer.set_raw_pixel(32 + index, requested_y, 1 + index as u16);
        }

        let recovered = prepared_indexed_palette_samples_with_history(
            &framebuffer.raw_pixels,
            texture_page,
            clut,
            16,
            true,
            Some(&framebuffer.recovery_palette_history),
        );
        assert_eq!(recovered[2].color, material[2]);
        assert!(recovered[2].palette_fallback);
    }

    #[test]
    fn br2_recovery_palette_history_does_not_cross_texture_generations() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x0208;
        let clut = 0x7a00;
        let requested_x = ((clut & 0x3f) as i32) * 16;
        let requested_y = ((clut >> 6) & 0x03ff) as i32;
        let previous_y = requested_y - 16;
        let material = [0x18e9, 0x18c6, 0x1ce6];

        for (index, color) in [
            0x0421, 0x0421, 0x0001, 0x0002, 0x0800, 0x0003, 0x0421, 0x0420, 0x0004, 0x0005, 0x0022,
            0x0006, 0x1420, 0x0442, 0x0006, 0x0c40,
        ]
        .into_iter()
        .enumerate()
        {
            framebuffer.set_raw_pixel(requested_x + index as i32, requested_y, color);
            framebuffer.set_raw_pixel(
                requested_x + index as i32,
                previous_y,
                material[index % material.len()],
            );
        }

        let initial = prepared_indexed_palette_samples_with_history(
            &framebuffer.raw_pixels,
            texture_page,
            clut,
            16,
            true,
            Some(&framebuffer.recovery_palette_history),
        );
        assert_eq!(initial[2].color, material[2]);

        for index in 0..16 {
            framebuffer.set_raw_pixel(requested_x + index, previous_y, 0);
            framebuffer.set_raw_pixel(16 + index, requested_y, 1 + index as u16);
            framebuffer.set_raw_pixel(32 + index, requested_y, 1 + index as u16);
        }
        let (page_x, page_y) = texture_page_origin_for_clut(texture_page, clut);
        framebuffer.set_raw_pixel(page_x, page_y, 0x1234);

        let recovered = prepared_indexed_palette_samples_with_history(
            &framebuffer.raw_pixels,
            texture_page,
            clut,
            16,
            true,
            Some(&framebuffer.recovery_palette_history),
        );
        assert_ne!(
            recovered[2].color, material[2],
            "a new texture generation must not reuse the prior scene palette"
        );
    }

    #[test]
    fn br2_low_bank_fighter_palette_recovers_implausibly_dark_requested_bank() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x020b;
        let clut = 0x7903;
        let requested_x = ((clut & 0x3f) as i32) * 16;
        let requested_y = ((clut >> 6) & 0x03ff) as i32;
        let previous_y = requested_y - 16;

        for index in 0..16 {
            framebuffer.set_raw_pixel(requested_x + index, requested_y, 1 + index as u16);
            framebuffer.set_raw_pixel(
                requested_x + index,
                previous_y,
                0x1084 + index as u16 * 0x21,
            );
        }

        let candidate = br2_character_model_palette_candidate_with_policy(
            &framebuffer.raw_pixels,
            texture_page,
            clut,
            true,
        )
        .expect("an implausibly dark fighter CLUT should resolve to its uploaded material row");
        assert_eq!((candidate.x, candidate.y), (requested_x, previous_y));
    }

    #[test]
    fn br2_low_bank_fighter_palette_keeps_dense_red_material_bank() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x020b;
        let clut = 0x7903;
        let requested_x = ((clut & 0x3f) as i32) * 16;
        let requested_y = ((clut >> 6) & 0x03ff) as i32;
        let balanced_x = requested_x + 16;

        for index in 0..16u16 {
            let requested = if index == 0 {
                0
            } else {
                let red = 20 + index % 8;
                let green = 3 + index % 3;
                let blue = 2 + index % 2;
                red | (green << 5) | (blue << 10)
            };
            let balanced = if index == 0 {
                0
            } else {
                let red = 12 + index % 4;
                let green = 10 + (index * 3) % 6;
                let blue = 9 + (index * 5) % 7;
                red | (green << 5) | (blue << 10)
            };
            framebuffer.set_raw_pixel(requested_x + i32::from(index), requested_y, requested);
            framebuffer.set_raw_pixel(balanced_x + i32::from(index), requested_y, balanced);
        }

        let candidate = br2_character_model_palette_candidate_with_policy(
            &framebuffer.raw_pixels,
            texture_page,
            clut,
            true,
        );
        assert_eq!(
            candidate, None,
            "a dense red fighter material CLUT must remain authoritative"
        );
    }

    #[test]
    fn br2_low_bank_fighter_palette_replaces_pure_red_polluted_requested_bank() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x020b;
        let clut = 0x7903;
        let requested_x = ((clut & 0x3f) as i32) * 16;
        let requested_y = ((clut >> 6) & 0x03ff) as i32;
        let balanced_x = requested_x + 16;

        for index in 0..16u16 {
            let polluted = if index == 0 {
                0
            } else {
                (8 + index) | (u16::from(index.is_multiple_of(5)) << 5)
            };
            let balanced = if index == 0 {
                0
            } else {
                let red = 10 + index % 5;
                let green = 9 + (index * 3) % 7;
                let blue = 8 + (index * 5) % 8;
                red | (green << 5) | (blue << 10)
            };
            framebuffer.set_raw_pixel(requested_x + i32::from(index), requested_y, polluted);
            framebuffer.set_raw_pixel(balanced_x + i32::from(index), requested_y, balanced);
        }

        let candidate = br2_character_model_palette_candidate_with_policy(
            &framebuffer.raw_pixels,
            texture_page,
            clut,
            true,
        )
        .expect("a pure-red polluted fighter CLUT must use the balanced adjacent upload");
        assert_eq!((candidate.x, candidate.y), (balanced_x, requested_y));
    }

    #[test]
    fn br2_low_bank_fighter_palette_skips_pure_red_polluted_fallback_candidate() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x020b;
        let clut = 0x7903;
        let requested_x = ((clut & 0x3f) as i32) * 16;
        let requested_y = ((clut >> 6) & 0x03ff) as i32;
        let polluted_x = requested_x - 16;
        let balanced_x = requested_x + 16;

        for index in 0..16u16 {
            framebuffer.set_raw_pixel(requested_x + i32::from(index), requested_y, 1 + index);
            framebuffer.set_raw_pixel(
                polluted_x + i32::from(index),
                requested_y,
                if index == 0 { 0 } else { 8 + index },
            );
            framebuffer.set_raw_pixel(
                balanced_x + i32::from(index),
                requested_y,
                if index == 0 {
                    0
                } else {
                    let red = 11 + index % 4;
                    let green = 10 + (index * 3) % 6;
                    let blue = 9 + (index * 5) % 7;
                    red | (green << 5) | (blue << 10)
                },
            );
        }

        let candidate = br2_character_model_palette_candidate_with_policy(
            &framebuffer.raw_pixels,
            texture_page,
            clut,
            true,
        )
        .expect("the balanced candidate after a polluted row must remain selectable");
        assert_eq!((candidate.x, candidate.y), (balanced_x, requested_y));
    }

    #[test]
    fn br2_low_bank_fighter_palette_does_not_replace_red_pollution_with_more_red_pollution() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x020b;
        let clut = 0x7903;
        let requested_x = ((clut & 0x3f) as i32) * 16;
        let requested_y = ((clut >> 6) & 0x03ff) as i32;
        let polluted_x = requested_x - 16;

        for index in 0..16u16 {
            framebuffer.set_raw_pixel(
                requested_x + i32::from(index),
                requested_y,
                if index == 0 { 0 } else { 8 + index },
            );
            framebuffer.set_raw_pixel(
                polluted_x + i32::from(index),
                requested_y,
                if index == 0 { 0 } else { 9 + index },
            );
        }

        let candidate = br2_character_model_palette_candidate_with_policy(
            &framebuffer.raw_pixels,
            texture_page,
            clut,
            true,
        );
        assert_eq!(
            candidate, None,
            "a second pure-red row is not a safe recovery palette"
        );
    }

    #[test]
    fn br2_captured_low_bank_fighter_palette_honors_explicit_clut_x_override() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x020b;
        let clut = 0x7903;
        let requested_x = ((clut & 0x3f) as i32) * 16;
        let requested_y = ((clut >> 6) & 0x03ff) as i32;
        let override_x = 64;

        for index in 0..16 {
            framebuffer.set_raw_pixel(requested_x + index, requested_y, 0x1000 + index as u16);
            framebuffer.set_raw_pixel(override_x + index, requested_y, 0x3000 + index as u16);
        }

        let candidate = br2_character_model_palette_candidate_with_override(
            &framebuffer.raw_pixels,
            texture_page,
            clut,
            true,
            Some(override_x),
        )
        .expect("explicit fighter palette override should select a populated CLUT");
        assert_eq!((candidate.x, candidate.y), (override_x, requested_y));
    }

    #[test]
    fn br2_captured_high_bank_model_palette_honors_explicit_clut_x_override() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x0039;
        let clut = 0x79d9;
        let requested_x = ((clut & 0x3f) as i32) * 16;
        let requested_y = ((clut >> 6) & 0x03ff) as i32;
        let override_x = 320;

        for index in 0..16 {
            framebuffer.set_raw_pixel(requested_x + index, requested_y, 0x1000 + index as u16);
            framebuffer.set_raw_pixel(override_x + index, requested_y, 0x3000 + index as u16);
        }

        let candidate = br2_character_model_palette_candidate_with_override(
            &framebuffer.raw_pixels,
            texture_page,
            clut,
            true,
            Some(override_x),
        )
        .expect("explicit high-bank model palette override should select a populated CLUT");
        assert_eq!((candidate.x, candidate.y), (override_x, requested_y));
    }

    #[test]
    fn framebuffer_texture_sampling_prefers_zn_256_clut_32row_block_base() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x0299;
        let clut = 0x7c80;
        let (page_x, page_y) = texture_page_origin(texture_page);
        let clut_y = ((clut >> 6) & 0x03ff) as i32;
        let alias_x = 384;
        let alias_y_32 = clut_y & !0x1f;
        let alias_y_16 = clut_y & !0x0f;

        assert_eq!((page_x, page_y), (576, 0));
        assert_ne!(alias_y_32, alias_y_16);

        framebuffer.set_raw_pixel(page_x, page_y, 0x002a);
        for index in 1..256 {
            let col = index & 0x0f;
            let row = index / 16;
            framebuffer.set_raw_pixel(alias_x + col, alias_y_32 + row, 0x03e0);
            framebuffer.set_raw_pixel(alias_x + col, alias_y_16 + row, 0x001f);
        }
        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            clut,
            TextureCoordinate { u: 0, v: 0 },
            TextureDrawOptions::opaque_raw(),
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), 0x0000_ff00);
    }

    #[test]
    fn framebuffer_texture_sampling_prefers_tiled_256_clut_x_for_zn_page_pair() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x029b;
        let clut = 0x7cc0;
        let (page_x, page_y) = texture_page_origin(texture_page);
        let clut_y = ((clut >> 6) & 0x03ff) as i32;
        let alias_y = clut_y & !0x1f;

        assert_eq!((page_x, page_y), (704, 0));

        framebuffer.set_raw_pixel(page_x, page_y, 0x002a);
        for index in 1..256 {
            let col = index & 0x0f;
            let row = index / 16;
            framebuffer.set_raw_pixel(384 + col, alias_y + row, 0x03e0);
            framebuffer.set_raw_pixel(400 + col, alias_y + row, 0x001f);
        }
        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            clut,
            TextureCoordinate { u: 0, v: 0 },
            TextureDrawOptions::opaque_raw(),
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), 0x00ff_0000);
    }

    #[test]
    fn framebuffer_texture_sampling_keeps_preferred_zn_256_clut_when_neighbor_is_only_modestly_denser()
     {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x0299;
        let clut = 0x7c80;
        let (page_x, page_y) = texture_page_origin(texture_page);
        let clut_y = ((clut >> 6) & 0x03ff) as i32;
        let preferred_x = preferred_tiled_256_palette_x(texture_page);
        let neighbor_x = preferred_x + 48;
        let alias_y = clut_y & !0x1f;

        assert_eq!((page_x, page_y), (576, 0));
        assert_eq!(preferred_x, 384);

        framebuffer.set_raw_pixel(page_x, page_y, 0x002a);
        for index in 1..196 {
            let col = index & 0x0f;
            let row = index / 16;
            framebuffer.set_raw_pixel(preferred_x + col, alias_y + row, 0x001f);
        }
        for index in 1..246 {
            let col = index & 0x0f;
            let row = index / 16;
            framebuffer.set_raw_pixel(neighbor_x + col, alias_y + row, 0x03e0);
        }

        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            clut,
            TextureCoordinate { u: 0, v: 0 },
            TextureDrawOptions::opaque_raw(),
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), 0x00ff_0000);
    }

    #[test]
    fn framebuffer_texture_sampling_locks_preferred_zn_256_clut_x_over_dense_neighbor() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x0299;
        let clut = 0x7c80;
        let (page_x, page_y) = texture_page_origin(texture_page);
        let clut_y = ((clut >> 6) & 0x03ff) as i32;
        let preferred_x = preferred_tiled_256_palette_x(texture_page);
        let neighbor_x = preferred_x + 48;
        let alias_y = clut_y & !0x1f;

        assert_eq!((page_x, page_y), (576, 0));
        assert_eq!(preferred_x, 384);

        framebuffer.set_raw_pixel(page_x, page_y, 0x002a);
        for index in 1..96 {
            let col = index & 0x0f;
            let row = index / 16;
            framebuffer.set_raw_pixel(preferred_x + col, alias_y + row, 0x001f);
        }
        for index in 1..256 {
            let col = index & 0x0f;
            let row = index / 16;
            framebuffer.set_raw_pixel(neighbor_x + col, alias_y + row, 0x03e0);
        }

        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            clut,
            TextureCoordinate { u: 0, v: 0 },
            TextureDrawOptions::opaque_raw(),
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), 0x00ff_0000);
    }

    #[test]
    fn framebuffer_texture_sampling_keeps_zero_from_valid_zn_256_clut_32row_tile() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x0299;
        let clut = 0x7c80;
        let (page_x, page_y) = texture_page_origin(texture_page);
        let clut_y = ((clut >> 6) & 0x03ff) as i32;
        let alias_x = 384;
        let alias_y_32 = clut_y & !0x1f;
        let alias_y_16 = clut_y & !0x0f;

        framebuffer.set_raw_pixel(page_x, page_y, 0x002a);
        for index in 1..256 {
            let col = index & 0x0f;
            let row = index / 16;
            let color = if index == 0x2a { 0 } else { 0x03e0 };
            framebuffer.set_raw_pixel(alias_x + col, alias_y_32 + row, color);
            framebuffer.set_raw_pixel(alias_x + col, alias_y_16 + row, 0x001f);
        }
        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            clut,
            TextureCoordinate { u: 0, v: 0 },
            TextureDrawOptions::opaque_raw(),
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), 0);
    }

    #[test]
    fn framebuffer_texture_sampling_keeps_valid_zn_256_clut_32row_tile_over_dense_neighbor() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x0299;
        let clut = 0x7c80;
        let (page_x, page_y) = texture_page_origin(texture_page);
        let clut_y = ((clut >> 6) & 0x03ff) as i32;
        let alias_x = 384;
        let sparse_alias_y = clut_y & !0x1f;
        let dense_alias_y = clut_y & !0x0f;

        assert_eq!((page_x, page_y), (576, 0));
        assert_ne!(sparse_alias_y, dense_alias_y);

        framebuffer.set_raw_pixel(page_x, page_y, 0x002a);
        for index in 1..80 {
            let col = index & 0x0f;
            let row = index / 16;
            framebuffer.set_raw_pixel(alias_x + col, sparse_alias_y + row, 0x03e0);
        }
        framebuffer.set_raw_pixel(alias_x + 0x0a, sparse_alias_y + 0x02, 0x03e0);
        for index in 1..256 {
            let col = index & 0x0f;
            let row = index / 16;
            framebuffer.set_raw_pixel(alias_x + col, dense_alias_y + row, 0x001f);
        }
        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            clut,
            TextureCoordinate { u: 0, v: 0 },
            TextureDrawOptions::opaque_raw(),
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), 0x0000_ff00);
    }

    #[test]
    fn framebuffer_texture_sampling_rejects_zn_256_linear_row_alias() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x0299;
        let clut = 0x7c80;
        let (page_x, page_y) = texture_page_origin(texture_page);
        let clut_y = ((clut >> 6) & 0x03ff) as i32;
        let alias_x = 384;

        framebuffer.set_raw_pixel(page_x, page_y, 0x002a);
        for index in 1..128 {
            framebuffer.set_raw_pixel(alias_x + index, clut_y, 0x03e0);
        }
        framebuffer.set_raw_pixel(alias_x + 0x2a, clut_y, 0x001f);
        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            clut,
            TextureCoordinate { u: 0, v: 0 },
            TextureDrawOptions::opaque_raw(),
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), 0);
    }

    #[test]
    fn framebuffer_texture_sampling_can_disable_blank_zn_256_clut_alias_fallback() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x0299;
        let clut = 0x7c80;
        let (page_x, page_y) = texture_page_origin(texture_page);
        let clut_y = ((clut >> 6) & 0x03ff) as i32;
        let alias_x = 384;
        let mut options = TextureDrawOptions::opaque_raw();
        options.allow_palette_fallback = false;

        framebuffer.set_raw_pixel(page_x, page_y, 0x002a);
        for index in 1..128 {
            framebuffer.set_raw_pixel(alias_x + index, clut_y, 0x03e0);
        }
        framebuffer.set_raw_pixel(alias_x + 0x2a, clut_y, 0x001f);
        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            clut,
            TextureCoordinate { u: 0, v: 0 },
            options,
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), 0);
    }

    #[test]
    fn framebuffer_texture_sampling_keeps_br2_low_bank_character_on_standard_origin_with_palette_fallback()
     {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x0088;
        let clut = 0x7d40;
        let (page_x, page_y) = texture_page_origin(texture_page);
        let clut_x = ((clut & 0x3f) as i32) * 16;
        let clut_y = ((clut >> 6) & 0x03ff) as i32;
        let palette_y = clut_y.saturating_sub(16);
        let options = TextureDrawOptions::opaque_raw();

        assert_eq!((page_x, page_y), (512, 0));
        assert_eq!((clut_x, clut_y, palette_y), (0, 501, 485));
        assert_eq!(
            texture_page_origin_for_clut(texture_page, clut),
            (page_x, page_y)
        );

        framebuffer.set_raw_pixel(page_x, page_y, 0x002a);
        for index in 1..256 {
            framebuffer.set_raw_pixel(clut_x + index, palette_y, 0x0400 + index as u16);
        }
        framebuffer.set_raw_pixel(clut_x + 0x2a, palette_y, 0x03e0);
        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            clut,
            TextureCoordinate { u: 0, v: 0 },
            options,
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), 0x0000_ff00);
    }

    #[test]
    fn framebuffer_texture_sampling_keeps_br2_large_character_portrait_on_y0() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x0088;
        let clut = 0x7d40;
        let page_x = 512;
        let page_y = 0;
        let clut_x = ((clut & 0x3f) as i32) * 16;
        let palette_y = ((clut >> 6) & 0x03ff) as i32 - 16;
        let index = 0x2a;

        assert_eq!(texture_page_origin(texture_page), (page_x, page_y));
        assert_eq!(
            texture_page_origin_for_clut(texture_page, clut),
            (page_x, page_y)
        );
        assert_eq!(
            texture_page_origin_for_clut(texture_page | 0x0200, clut),
            (page_x, page_y)
        );

        framebuffer.set_raw_pixel(page_x, page_y, index as u16 | 0x1100);
        framebuffer.set_raw_pixel(832, PSX_VRAM_HEIGHT as i32 / 2, 0x0011);
        for palette_index in 1..256 {
            framebuffer.set_raw_pixel(
                clut_x + palette_index,
                palette_y,
                0x0400 + palette_index as u16,
            );
        }
        framebuffer.set_raw_pixel(clut_x + index, palette_y, 0x03e0);

        let sample = framebuffer.sample_texture_sample_from(
            &framebuffer.raw_pixels,
            texture_page | 0x0200,
            clut,
            0,
            0,
            TextureSamplingPolicy::new(true, true),
        );
        assert_eq!(sample.color, 0x03e0);
        assert!(sample.texture_nonzero);
    }

    #[test]
    fn framebuffer_texture_sampling_uses_br2_left_select_portrait_y256_alias() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x020d;
        let clut = 0x7c1d;
        let page_x = 832;
        let alias_y = PSX_VRAM_HEIGHT as i32 / 2;
        let clut_x = ((clut & 0x3f) as i32) * 16;
        let clut_y = ((clut >> 6) & 0x03ff) as i32;

        assert_eq!(texture_page_origin(texture_page), (page_x, 0));
        assert_eq!(
            texture_page_origin_for_clut(texture_page, clut),
            (page_x, alias_y)
        );
        assert_eq!(
            texture_page_origin_for_clut(texture_page & !0x0200, clut),
            (page_x, alias_y)
        );
        assert_eq!(
            texture_page_origin_for_clut(0x020c, 0x7d18),
            (768, 0),
            "normal character-select background strips must keep their low-bank origin"
        );

        framebuffer.set_raw_pixel(page_x, 0, 0x1111);
        framebuffer.set_raw_pixel(page_x, alias_y, 0x2222);
        framebuffer.set_raw_pixel(clut_x + 1, clut_y, 0x001f);
        framebuffer.set_raw_pixel(clut_x + 2, clut_y, 0x03e0);

        let sample = framebuffer.sample_texture_sample_from(
            &framebuffer.raw_pixels,
            texture_page,
            clut,
            0,
            0,
            TextureSamplingPolicy::new(true, true),
        );
        assert_eq!(sample.color, 0x03e0);
        assert!(sample.texture_nonzero);
    }

    #[test]
    fn framebuffer_texture_sampling_uses_br2_right_portrait_y256_texture_alias() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x008a;
        let clut = 0x7d80;
        let (page_x, page_y) = texture_page_origin(texture_page);
        let alias_y = PSX_VRAM_HEIGHT as i32 / 2;
        let clut_x = ((clut & 0x3f) as i32) * 16;
        let clut_y = ((clut >> 6) & 0x03ff) as i32;
        let base_palette_y = clut_y & !0x1f;
        let palette_y = clut_y - 16;
        let index = 0x2a;
        let options = TextureDrawOptions::opaque_raw();

        assert_eq!((page_x, page_y), (640, 0));
        assert_eq!(
            texture_page_origin_for_clut(texture_page, clut),
            (640, alias_y)
        );
        assert_eq!(
            texture_page_origin_for_clut(texture_page | 0x0200, clut),
            (640, alias_y)
        );
        assert_eq!(
            (clut_x, clut_y, base_palette_y, palette_y),
            (0, 502, 480, 486)
        );

        framebuffer.set_raw_pixel(page_x, page_y, 0x0011);
        framebuffer.set_raw_pixel(page_x, alias_y, index as u16 | 0x1100);
        for palette_index in 1..256 {
            framebuffer.set_raw_pixel(clut_x + palette_index, base_palette_y, 0x7c00);
            framebuffer.set_raw_pixel(
                clut_x + palette_index,
                palette_y,
                0x0400 + palette_index as u16,
            );
        }
        framebuffer.set_raw_pixel(clut_x + index, palette_y, 0x03e0);
        assert_eq!(
            texture_page_origin_for_sample(texture_page, clut, 0, 0),
            (640, alias_y)
        );
        assert_eq!(
            framebuffer.raw_pixel(page_x, alias_y),
            index as u16 | 0x1100
        );
        assert_eq!(framebuffer.raw_pixel(clut_x + index, palette_y), 0x03e0);
        assert_eq!(super::texture_page_color_mode(texture_page), 1);
        let fallback = super::fallback_linear_256_palette_sample(
            &framebuffer.raw_pixels,
            texture_page,
            clut,
            index,
        )
        .expect("BR2 right portrait palette row should resolve to the 256-color block base");
        assert_eq!(fallback.color, 0x03e0);
        assert!(options.allow_palette_fallback);
        assert_eq!(
            super::indexed_palette_sample_from(
                &framebuffer.raw_pixels,
                texture_page,
                clut,
                index,
                256,
                options.allow_palette_fallback,
            )
            .color,
            0x03e0
        );
        let sample = framebuffer.sample_texture_sample_from(
            &framebuffer.raw_pixels,
            texture_page,
            clut,
            0,
            0,
            TextureSamplingPolicy::from_draw_options(options),
        );
        assert_eq!(sample.color, 0x03e0);
        assert!(sample.texture_nonzero);
        assert!(sample.palette_fallback);
        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            clut,
            TextureCoordinate { u: 0, v: 0 },
            options,
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), 0x0000_ff00);
    }

    #[test]
    fn framebuffer_texture_sampling_uses_br2_character_select_cell_y256_alias() {
        for clut in [
            0x7800, 0x7840, 0x7880, 0x78c0, 0x7900, 0x7940, 0x7980, 0x79c0, 0x7a00, 0x7a40, 0x7a80,
        ] {
            assert_eq!(
                texture_page_origin_for_clut(0x029d, clut),
                (832, 256),
                "character-select CLUT 0x{clut:04x} must sample the portrait bank"
            );
        }
        assert_eq!(
            texture_page_origin_for_clut(0x029d, 0x7ac0),
            (832, 0),
            "unobserved 0x029d CLUT rows must keep the standard low-bank alias"
        );
        assert_eq!(
            texture_page_origin_for_clut(0x029d, 0x7801),
            (832, 0),
            "other CLUT columns must not inherit the BR2 character-select alias"
        );
    }

    #[test]
    fn framebuffer_texture_sampling_blocks_br2_low_bank_character_linear_256_clut_alias_when_disabled()
     {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x0088;
        let clut = 0x7d40;
        let (page_x, page_y) = texture_page_origin(texture_page);
        let clut_x = ((clut & 0x3f) as i32) * 16;
        let alias_y = (((clut >> 6) & 0x03ff) as i32) & !0x1f;
        let mut options = TextureDrawOptions::opaque_raw();
        options.allow_palette_fallback = false;

        framebuffer.set_raw_pixel(page_x, page_y, 0x002a);
        for index in 1..256 {
            framebuffer.set_raw_pixel(clut_x + index, alias_y, 0x0400 + index as u16);
        }
        framebuffer.set_raw_pixel(clut_x + 0x2a, alias_y, 0x03e0);
        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            clut,
            TextureCoordinate { u: 0, v: 0 },
            options,
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), 0);
    }

    #[test]
    fn framebuffer_texture_sampling_keeps_non_br2_linear_256_clut_alias_blocked() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x0299;
        let clut = 0x7d40;
        let (page_x, page_y) = texture_page_origin(texture_page);
        let clut_x = ((clut & 0x3f) as i32) * 16;
        let alias_y = (((clut >> 6) & 0x03ff) as i32) & !0x1f;
        let mut options = TextureDrawOptions::opaque_raw();
        options.allow_palette_fallback = false;

        framebuffer.set_raw_pixel(page_x, page_y, 0x002a);
        for index in 1..256 {
            framebuffer.set_raw_pixel(clut_x + index, alias_y, 0x03e0);
        }
        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            clut,
            TextureCoordinate { u: 0, v: 0 },
            options,
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), 0);
    }

    #[test]
    fn framebuffer_texture_sampling_prefers_requested_zn_256_clut_when_present() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x0299;
        let clut = 0x7c80;
        let (page_x, page_y) = texture_page_origin(texture_page);
        let clut_x = ((clut & 0x3f) as i32) * 16;
        let clut_y = ((clut >> 6) & 0x03ff) as i32;
        let alias_x = 384;

        framebuffer.set_raw_pixel(page_x, page_y, 0x002a);
        framebuffer.set_raw_pixel(clut_x + 0x2a, clut_y, 0x03e0);
        for index in 1..128 {
            framebuffer.set_raw_pixel(alias_x + index, clut_y, 0x001f);
        }
        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            clut,
            TextureCoordinate { u: 0, v: 0 },
            TextureDrawOptions::opaque_raw(),
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), 0x0000_ff00);
    }

    #[test]
    fn framebuffer_texture_sampling_overrides_sparse_requested_zn_256_clut() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x0299;
        let clut = 0x7c80;
        let (page_x, page_y) = texture_page_origin(texture_page);
        let clut_x = ((clut & 0x3f) as i32) * 16;
        let clut_y = ((clut >> 6) & 0x03ff) as i32;
        let alias_x = 384;
        let alias_y = clut_y & !0x1f;

        framebuffer.set_raw_pixel(page_x, page_y, 0x002a);
        framebuffer.set_raw_pixel(clut_x + 0x2a, clut_y, 0x03e0);
        for index in 1..256 {
            let col = index & 0x0f;
            let row = index / 16;
            framebuffer.set_raw_pixel(alias_x + col, alias_y + row, 0x001f);
        }
        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            clut,
            TextureCoordinate { u: 0, v: 0 },
            TextureDrawOptions::opaque_raw(),
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), 0x00ff_0000);
    }

    #[test]
    fn framebuffer_texture_sampling_keeps_dense_requested_zn_256_clut() {
        let mut framebuffer = NativeFrameBuffer::default();
        let texture_page = 0x0299;
        let clut = 0x7c80;
        let (page_x, page_y) = texture_page_origin(texture_page);
        let clut_x = ((clut & 0x3f) as i32) * 16;
        let clut_y = ((clut >> 6) & 0x03ff) as i32;
        let alias_x = 384;
        let alias_y = clut_y & !0x1f;

        framebuffer.set_raw_pixel(page_x, page_y, 0x002a);
        for index in 1..256 {
            framebuffer.set_raw_pixel(clut_x + index, clut_y, 0x03e0);
            let col = index & 0x0f;
            let row = index / 16;
            framebuffer.set_raw_pixel(alias_x + col, alias_y + row, 0x001f);
        }
        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (1, 1),
            texture_page,
            clut,
            TextureCoordinate { u: 0, v: 0 },
            TextureDrawOptions::opaque_raw(),
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), 0x0000_ff00);
    }

    #[test]
    fn framebuffer_textured_rect_honors_zn_sprite_flip_bits() {
        let mut framebuffer = NativeFrameBuffer::default();
        let mut options = TextureDrawOptions::opaque_raw();
        options.texture_flip_x = true;
        options.texture_flip_y = true;

        framebuffer.set_raw_pixel(2, 2, 0x001f);
        framebuffer.set_raw_pixel(1, 2, 0x03e0);
        framebuffer.set_raw_pixel(2, 1, 0x7c00);
        framebuffer.draw_textured_rect(
            Point { x: 8, y: 8 },
            (2, 2),
            0x0100,
            0,
            TextureCoordinate { u: 2, v: 2 },
            options,
            TextureWindow::default(),
        );

        assert_eq!(framebuffer.pixel(8, 8), 0x00ff_0000);
        assert_eq!(framebuffer.pixel(9, 8), 0x0000_ff00);
        assert_eq!(framebuffer.pixel(8, 9), 0x0000_00ff);
    }

    #[test]
    fn framebuffer_vram_copies_wrap_edges() {
        let mut framebuffer = NativeFrameBuffer::default();

        framebuffer.set_raw_pixel((VRAM_WIDTH - 1) as i32, (VRAM_HEIGHT - 1) as i32, 0x001f);
        framebuffer.set_raw_pixel(0, 0, 0x03e0);
        framebuffer.copy_rect(
            (VRAM_WIDTH - 1) as i32,
            (VRAM_HEIGHT - 1) as i32,
            2,
            3,
            2,
            2,
        );

        assert_eq!(framebuffer.pixel(2, 3), 0x00ff_0000);
        assert_eq!(framebuffer.pixel(3, 4), 0x0000_ff00);
    }

    #[test]
    fn framebuffer_finds_densest_nonzero_window() {
        let mut framebuffer = NativeFrameBuffer::default();

        framebuffer.fill_rect(512, 24, 32, 32, 0x00ff_ffff);

        let window = framebuffer
            .densest_window(320, 240, 8)
            .expect("densest window");

        assert_eq!(window.x, 224);
        assert_eq!(window.y, 0);
        assert_eq!(window.stats.nonzero_pixels, 1024);
    }

    #[test]
    fn framebuffer_finds_brightest_window_over_dark_nonzero_pixels() {
        let mut framebuffer = NativeFrameBuffer::default();

        framebuffer.fill_rect_unclipped(0, 0, 320, 240, 0x0000_0008);
        framebuffer.fill_rect_unclipped(496, 256, 320, 240, 0x0000_ff00);

        let window = framebuffer
            .brightest_window(320, 240, 8)
            .expect("brightest window");

        assert_eq!(window.x, 496);
        assert_eq!(window.y, 256);
        assert_eq!(window.stats.bright_pixels, 320 * 240);
    }
}
