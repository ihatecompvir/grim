use super::*;

#[allow(non_camel_case_types)]
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum DXGI_Encoding {
    DXGI_FORMAT_BC1_UNORM =  8, // DXT1
    DXGI_FORMAT_BC3_UNORM = 24, // DXT5
    DXGI_FORMAT_BC5_UNORM = 32, // ATI2
}

impl Default for DXGI_Encoding {
    fn default() -> DXGI_Encoding {
        DXGI_Encoding::DXGI_FORMAT_BC3_UNORM
    }
}

impl From<u32> for DXGI_Encoding {
    fn from(num: u32) -> DXGI_Encoding {
        match num {
             8 => DXGI_Encoding::DXGI_FORMAT_BC1_UNORM,
            24 => DXGI_Encoding::DXGI_FORMAT_BC3_UNORM,
            32 => DXGI_Encoding::DXGI_FORMAT_BC5_UNORM,
            // Default
            _ => DXGI_Encoding::DXGI_FORMAT_BC3_UNORM,
        }
    }
}

pub fn decode_dx_image(dx_img: &[u8], rgba: &mut [u8], width: u32, encoding: DXGI_Encoding) {
    match &encoding {
        DXGI_Encoding::DXGI_FORMAT_BC1_UNORM => decode_dxt1_image(dx_img, rgba, width),
        DXGI_Encoding::DXGI_FORMAT_BC3_UNORM => decode_dxt5_image(dx_img, rgba, width),
        DXGI_Encoding::DXGI_FORMAT_BC5_UNORM => todo!("Implement BC5 texture decoding"),
    };
}

fn decode_dxt1_image(dx_img: &[u8], rgba: &mut [u8], width: u32) {
    let bpp = get_dx_bpp(&DXGI_Encoding::DXGI_FORMAT_BC1_UNORM) as u32;

    // Get block counts
    let block_x = width >> 2;
    let block_y = calculate_texture_height(dx_img.len(), width, bpp) >> 2;
    let block_size = ((16 * bpp) / 8) as usize;

    let mut packed_0;
    let mut packed_1;

    let mut color_0 = [0u8; 4];
    let mut color_1 = [0u8; 4];
    let mut color_2 = [0u8; 4];
    let mut color_3 = [0u8; 4];

    let mut indices = [0u8; 16];

    let mut i = 0usize; // Block index
    let mut x;
    let mut y;

    for by in 0..block_y {
        for bx in 0..block_x {
            x = bx << 2;
            y = by << 2;

            // Read packed bytes
            packed_0 = read_as_u16(&dx_img[i..(i + 2)]);
            packed_1 = read_as_u16(&dx_img[(i + 2)..(i + 4)]);

            // Unpack colors to rgba
            unpack_rgb565(packed_0, &mut color_0);
            unpack_rgb565(packed_1, &mut color_1);

            // Interpolate other colors
            if packed_0 > packed_1 {
                // 4 colors
                mix_colors_66_33(&color_0, &color_1, &mut color_2);
                mix_colors_66_33(&color_1, &color_0, &mut color_3);
            } else {
                // 3 colors + transparent
                mix_colors_50_50(&color_0, &color_1, &mut color_2);
                zero_out(&mut color_3);
            }

            // Unpack color indicies
            unpack_indices(&dx_img[(i + 4)..(i + 8)], &mut indices);

            // Copy colors to pixel data
            let colors = [&color_0, &color_1, &color_2, &color_3];
            copy_unpacked_pixels(rgba, &colors, &indices, x, y, width);

            i += block_size;
        }
    }
}

fn decode_dxt5_image(dx_img: &[u8], rgba: &mut [u8], width: u32) {
    let bpp = get_dx_bpp(&DXGI_Encoding::DXGI_FORMAT_BC3_UNORM) as u32;

    // Get block counts
    let block_x = width >> 2;
    let block_y = ((dx_img.len() * 8) as u32 / (width * bpp)) >> 2;
    let block_size = ((16 * bpp) / 8) as usize;

    let mut packed_0;
    let mut packed_1;

    let mut color_0 = [0u8; 4];
    let mut color_1 = [0u8; 4];
    let mut color_2 = [0u8; 4];
    let mut color_3 = [0u8; 4];

    let mut indices = [0u8; 16];

    let mut i = 0usize; // Block index
    let mut x;
    let mut y;

    for by in 0..block_y {
        for bx in 0..block_x {
            x = bx << 2;
            y = by << 2;

            // Skip alphas for now
            i += block_size >> 1;

            // Read packed bytes
            packed_0 = read_as_u16(&dx_img[i..(i + 2)]);
            packed_1 = read_as_u16(&dx_img[(i + 2)..(i + 4)]);

            // Unpack colors to rgba
            unpack_rgb565(packed_0, &mut color_0);
            unpack_rgb565(packed_1, &mut color_1);

            // Interpolate other colors
            if packed_0 > packed_1 {
                // 4 colors
                mix_colors_66_33(&color_0, &color_1, &mut color_2);
                mix_colors_66_33(&color_1, &color_0, &mut color_3);
            } else {
                // 3 colors + transparent
                mix_colors_50_50(&color_0, &color_1, &mut color_2);
                zero_out(&mut color_3);
            }

            // Unpack color indicies
            unpack_indices(&dx_img[(i + 4)..(i + 8)], &mut indices);

            // Copy colors to pixel data
            let colors = [&color_0, &color_1, &color_2, &color_3];
            copy_unpacked_pixels(rgba, &colors, &indices, x, y, width);

            i += block_size >> 1;
        }
    }
}

fn get_dx_bpp(encoding: &DXGI_Encoding) -> u8 {
    match encoding {
        DXGI_Encoding::DXGI_FORMAT_BC1_UNORM => 4,
        DXGI_Encoding::DXGI_FORMAT_BC3_UNORM => 8,
        DXGI_Encoding::DXGI_FORMAT_BC5_UNORM => 8,
    }
}