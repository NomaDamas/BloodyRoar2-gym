#![allow(clippy::too_many_arguments)]

pub const VRAM_WIDTH: usize = 1024;
pub const VRAM_HEIGHT: usize = 1024;
pub const PSX_VRAM_HEIGHT: usize = 512;
pub const DEFAULT_DISPLAY_WIDTH: usize = 320;
pub const DEFAULT_DISPLAY_HEIGHT: usize = 240;

#[derive(Clone, Debug)]
pub struct NativeFrameBuffer {
    pixels: Vec<u32>,
    raw_pixels: Vec<u16>,
    clip: Option<ClipRect>,
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
        self.raw_pixels[index] = color;
        self.pixels[index] = rgb555_to_rgb888(color);
        true
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
            self.set_raw_pixel_unclipped_with_options(x + col, y + row, raw as u16, options);
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
                let w0 = edge(b, c, point);
                let w1 = edge(c, a, point);
                let w2 = edge(a, b, point);
                let inside = if area > 0 {
                    w0 >= 0 && w1 >= 0 && w2 >= 0
                } else {
                    w0 <= 0 && w1 <= 0 && w2 <= 0
                };
                if inside {
                    self.set_pixel_with_options(x, y, rgb, options);
                }
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
        let mut stats = TexturedDrawStats::default();
        let (clip_left, clip_top, clip_right, clip_bottom) = self.clip_bounds();
        if clip_left >= clip_right || clip_top >= clip_bottom {
            return stats;
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
        if area == 0 {
            return stats;
        }

        let source = self.clone();
        let denom = area.unsigned_abs() as i64;
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let point = Point { x, y };
                let mut w0 = edge(b.point, c.point, point);
                let mut w1 = edge(c.point, a.point, point);
                let mut w2 = edge(a.point, b.point, point);
                let inside = if area > 0 {
                    w0 >= 0 && w1 >= 0 && w2 >= 0
                } else {
                    w0 <= 0 && w1 <= 0 && w2 <= 0
                };
                if !inside {
                    continue;
                }

                stats.sampled_pixels = stats.sampled_pixels.saturating_add(1);
                if area < 0 {
                    w0 = -w0;
                    w1 = -w1;
                    w2 = -w2;
                }
                let u = ((a.u as i64 * w0 + b.u as i64 * w1 + c.u as i64 * w2) / denom) as u8;
                let v = ((a.v as i64 * w0 + b.v as i64 * w1 + c.v as i64 * w2) / denom) as u8;
                let (u, v) = texture_window.apply(u, v);
                if let Some(color) = source.sample_texture(texture_page, clut, u, v) {
                    let color = options.apply_color(color);
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

        let source = self.clone();
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
                if let Some(color) = source.sample_texture(texture_page, clut, u, v) {
                    let color = options.apply_color(color);
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

    fn sample_texture(&self, texture_page: u16, clut: u16, u: u8, v: u8) -> Option<u16> {
        let color = self.sample_texture_raw(texture_page, clut, u, v);
        (color != 0).then_some(color)
    }

    fn sample_texture_raw(&self, texture_page: u16, clut: u16, u: u8, v: u8) -> u16 {
        let page_x = ((texture_page & 0x0f) as i32) * 64;
        let page_y = texture_page_y(texture_page) as i32;
        let mode = (texture_page >> 7) & 0x03;
        let u = u as i32;
        let v = v as i32;

        match mode {
            0 => {
                let packed = self.raw_pixel(page_x + u / 4, page_y + v);
                let index = ((packed >> ((u & 3) * 4)) & 0x0f) as i32;
                self.raw_pixel(((clut & 0x3f) as i32) * 16 + index, clut_y(clut) as i32)
            }
            1 => {
                let packed = self.raw_pixel(page_x + u / 2, page_y + v);
                let index = if u & 1 == 0 {
                    packed & 0x00ff
                } else {
                    packed >> 8
                } as i32;
                self.raw_pixel(((clut & 0x3f) as i32) * 16 + index, clut_y(clut) as i32)
            }
            _ => self.raw_pixel(page_x + u, page_y + v),
        }
    }

    pub fn decoded_texture_png(&self, texture_page: u16, clut: u16) -> Vec<u8> {
        let (width, height) = texture_page_dimensions(texture_page);
        let mut pixels = Vec::with_capacity(width.saturating_mul(height));
        for y in 0..height {
            for x in 0..width {
                let color = self.sample_texture_raw(texture_page, clut, x as u8, y as u8);
                pixels.push(rgb555_to_rgb888(color));
            }
        }
        png_from_rgb888_pixels(width, height, &pixels)
    }

    pub fn texture_palette_png(&self, texture_page: u16, clut: u16) -> Vec<u8> {
        let palette_entries = texture_palette_entries(texture_page);
        if palette_entries == 0 {
            return png_from_rgb888_pixels(1, 1, &[0]);
        }

        let columns = palette_entries.min(16);
        let rows = palette_entries.div_ceil(columns);
        let cell_size = 8usize;
        let width = columns * cell_size;
        let height = rows * cell_size;
        let clut_x = ((clut & 0x3f) as i32) * 16;
        let clut_y = clut_y(clut) as i32;
        let mut pixels = vec![0; width.saturating_mul(height)];
        for index in 0..palette_entries {
            let color = rgb555_to_rgb888(self.raw_pixel(clut_x + index as i32, clut_y));
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
        png_rgb(
            width.max(1),
            height.max(1),
            &self.psx_display_rgb_rows(start_x, start_y, width.max(1), height.max(1)),
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
        let width = width.max(1);
        let height = height.max(1);
        let mut pixels = Vec::with_capacity(width * height);
        for y in 0..height {
            let source_y = (start_y + y) % PSX_VRAM_HEIGHT;
            for x in 0..width {
                let source_x = (start_x + x) % VRAM_WIDTH;
                pixels.push(self.pixels[source_y * VRAM_WIDTH + source_x]);
            }
        }
        pixels
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
            let source_y = (start_y + y) % PSX_VRAM_HEIGHT;
            let mut previous_luma = 0_u8;
            for x in 0..width {
                let source_x = (start_x + x) % VRAM_WIDTH;
                let rgb = self.pixels[source_y * VRAM_WIDTH + source_x];
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

    fn psx_display_rgb_rows(
        &self,
        start_x: usize,
        start_y: usize,
        width: usize,
        height: usize,
    ) -> Vec<u8> {
        let mut rows = Vec::with_capacity(height * (1 + width * 3));
        for y in 0..height {
            rows.push(0);
            let source_y = (start_y + y) % PSX_VRAM_HEIGHT;
            for x in 0..width {
                let source_x = (start_x + x) % VRAM_WIDTH;
                let rgb = self.pixels[source_y * VRAM_WIDTH + source_x];
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TexturedDrawStats {
    pub sampled_pixels: u64,
    pub drawn_pixels: u64,
    pub written_pixels: u64,
    pub clipped_pixels: u64,
    pub transparent_pixels: u64,
    pub first_color: u16,
    pub last_color: u16,
    pub color_hash: u32,
    pub color_changes: u64,
}

impl TexturedDrawStats {
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
        }
    }

    fn apply_color(self, color: u16) -> u16 {
        if self.raw_texture {
            return color;
        }
        modulate_rgb555(color, self.primitive_color)
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

fn clut_y(clut: u16) -> u16 {
    (clut >> 6) & 0x03ff
}

fn texture_page_y(texture_page: u16) -> u16 {
    ((texture_page & 0x0010) << 4) | ((texture_page & 0x0800) >> 2)
}

fn texture_page_dimensions(_texture_page: u16) -> (usize, usize) {
    (256, 256)
}

fn texture_palette_entries(texture_page: u16) -> usize {
    match (texture_page >> 7) & 0x03 {
        0 => 16,
        1 => 256,
        _ => 0,
    }
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
        NativeFrameBuffer, PSX_VRAM_HEIGHT, Point, TextureCoordinate, TextureDrawOptions,
        TextureWindow, VRAM_HEIGHT, VRAM_WIDTH,
    };

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
    fn framebuffer_writes_rgb555_images_and_copies_rects() {
        let mut framebuffer = NativeFrameBuffer::default();

        framebuffer.write_rgb555_image(4, 4, 2, 1, &[0x03e0_001f]);
        framebuffer.copy_rect(4, 4, 8, 8, 2, 1);

        assert_eq!(framebuffer.pixel(8, 8), 0x00ff_0000);
        assert_eq!(framebuffer.pixel(9, 8), 0x0000_ff00);
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
