use crate::io::{BinaryStream, SeekFrom, Stream};
use crate::texture::{Bitmap, decode_dx_image, decode_tpl_image, DXGI_Encoding, TPLEncoding};
use crate::system::{Platform, SystemInfo};
use image::{ImageBuffer, RgbaImage};

use std::error::Error;
use std::path::Path;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum BitmapError {
    #[error("Unsupported texture encoding of {version}")]
    UnsupportedEncoding {
        version: u32,
    },
    #[error("Unsupported bitmap bpp of {bpp}")]
    UnsupportedBitmapBpp {
        bpp: u8
    },
}

impl Bitmap {
    pub fn from_stream(stream: &mut dyn Stream, info: &SystemInfo) -> Result<Bitmap, Box<dyn Error>> {
        let mut bitmap = Bitmap::new();
        let mut reader = BinaryStream::from_stream_with_endian(stream, info.endian);

        let _byte_1 = reader.read_uint8()?; // TODO: Verify always 1

        bitmap.bpp = reader.read_uint8()?;
        bitmap.encoding = reader.read_uint32()?;
        bitmap.mip_maps = reader.read_uint8()?;

        bitmap.width = reader.read_uint16()?;
        bitmap.height = reader.read_uint16()?;
        bitmap.bpl = reader.read_uint16()?;

        reader.seek(SeekFrom::Current(19))?; // Skip empty bytes

        // TODO: Calculate expected data size and verify against actual
        let current_pos = reader.pos();
        let stream_len = reader.len()?;
        let rem_bytes = stream_len - current_pos as usize;

        bitmap.raw_data = reader.read_bytes(rem_bytes)?;

        Ok(bitmap)
    }

    pub fn unpack_rgba(&self, info: &SystemInfo) -> Result<Vec<u8>, Box<dyn Error>> {
        if info.platform == Platform::PS2 && self.encoding == 3 {
            // Decode PS2 bitmap
            let mut rgba = vec![0u8; self.calc_rgba_size()];
            decode_from_bitmap(self, info, &mut rgba[..])?;
            return Ok(rgba);
        } else if info.platform == Platform::PS3 || info.platform == Platform::X360 {
            // Decode next gen texture
            let dx_enc = match self.encoding {
                 8 => DXGI_Encoding::DXGI_FORMAT_BC1_UNORM,
                // TODO: Implement these encodings
                24 => DXGI_Encoding::DXGI_FORMAT_BC3_UNORM,
                /*32 => DXGI_Encoding::DXGI_FORMAT_BC5_UNORM,*/
                _ => {
                    return Err(Box::new(BitmapError::UnsupportedEncoding {
                        version: self.encoding,
                    }));
                }
            };

            let mut rgba = vec![0u8; self.calc_rgba_size()];

            let mut mips = self.mip_maps;
            let mut width = self.width;
            let mut height = self.height;

            let mut start_dxt = 0usize;
            let mut start_rgba = 0usize;

            // Hacky way to decode w/ mip maps
            // TODO: Clean up code
            loop {
                let dxt_size = ((width as usize) * (height as usize) * (self.bpp as usize)) / 8;
                let dxt_img = &self.raw_data.as_slice()[start_dxt..(start_dxt + dxt_size)];

                let rgba_size = (width as usize) * (height as usize) * 4;
                let rgba_img = &mut rgba.as_mut_slice()[start_rgba..(start_rgba + rgba_size)];

                decode_dx_image(dxt_img, rgba_img, self.width as u32, dx_enc);

                if mips == 0 {
                    break;
                }

                start_dxt += dxt_size;
                start_rgba += rgba_size;

                mips -= 1;
                width >>= 1;
                height >>= 1;
            }

            return Ok(rgba);
        } else if info.platform == Platform::Wii {
            // Decode wii texture
            let tpl_enc = match self.encoding {
                 72 => TPLEncoding::CMP,
                328 => TPLEncoding::CMP_ALPHA,
                _ => {
                    return Err(Box::new(BitmapError::UnsupportedEncoding {
                        version: self.encoding,
                    }));
                }
            };

            let mut rgba = vec![0u8; self.calc_rgba_size()];

            let mut mips = self.mip_maps;
            let mut width = self.width;
            let mut height = self.height;

            let mut start_tpl = 0usize;
            let mut start_rgba = 0usize;

            // Hacky way to decode w/ mip maps
            // TODO: Clean up code
            loop {
                let tpl_size = ((width as usize) * (height as usize) * (self.bpp as usize)) / 8;
                let tpl_img = &self.raw_data.as_slice()[start_tpl..(start_tpl + tpl_size)];

                let rgba_size = (width as usize) * (height as usize) * 4;
                let rgba_img = &mut rgba.as_mut_slice()[start_rgba..(start_rgba + rgba_size)];

                decode_tpl_image(tpl_img, rgba_img, self.width as u32, tpl_enc);
                //decode_dx_image(tpl_img, rgba_img, self.width as u32, DXGI_Encoding::DXGI_FORMAT_BC1_UNORM);

                if mips == 0 {
                    break;
                }

                start_tpl += tpl_size;
                start_rgba += rgba_size;

                mips -= 1;
                width >>= 1;
                height >>= 1;
            }

            return Ok(rgba);
        }

        Err(Box::new(BitmapError::UnsupportedEncoding {
            version: self.encoding,
        }))
    }

    fn calc_rgba_size(&self) -> usize {
        let Bitmap { width: w, height: h, mip_maps: mips, ..} = self;
        calc_rgba_size(*w, *h, *mips)
    }
}

fn calc_rgba_size(mut w: u16, mut h: u16, mut mips: u8) -> usize {
    let mut size = 0;

    loop {
        size += (w as usize) * (h as usize) * 4;

        if mips == 0 {
            break;
        }

        w >>= 1;
        h >>= 1;
        mips -= 1;
    }

    size
}


fn decode_from_bitmap(bitmap: &Bitmap, _info: &SystemInfo, rgba: &mut [u8]) -> Result<(), Box<dyn Error>> {
    let Bitmap { bpp, raw_data: data, .. } = bitmap;

    if *bpp == 4 || *bpp == 8 {
        let mut palette = data[..(1 << (*bpp + 2))].to_owned(); // Takes 1024 bytes for 8bpp and 64 bytes for 4bpp
        let encoded = &data[palette.len()..];
        update_alpha_channels(&mut palette, false);

        let mut i = 0; // Image index
        let mut e = 0; // Encoded index

        if *bpp == 4 {
            // Each byte encodes two colors as palette indices
            let mut p1;
            let mut p2;

            while i < rgba.len() {
                // Palette indices
                p1 = ((encoded[e] & 0x0F) as usize) << 2;
                p2 = ((encoded[e] & 0xF0) as usize) >> 2;

                // Copy colors from palette into rgba array
                rgba[i..(i + 4)].clone_from_slice(&palette[p1..(p1 + 4)]);
                rgba[(i + 4)..(i + 8)].clone_from_slice(&palette[p2..(p2 + 4)]);

                // Increment index
                e += 1;
                i += 8; // 2 pixels
            }
        } else { // 8 bpp
            // Each byte encodes single color as palette index
            let mut p1;
            let mut enc;

            while i < rgba.len() {
                enc = encoded[e];

                // Palette index
                // Swaps bits 3 and 4 with eachother
                // Ex: 0110 1011 -> 0111 0011
                p1 = (((enc & 0b1110_0111)
                    | ((enc & 0b0000_1000) << 1)
                    | ((enc & 0b0001_0000) >> 1)) as usize) << 2;

                // Copy color from palette into rgba array
                rgba[i..(i + 4)].clone_from_slice(&palette[p1..(p1 + 4)]);

                // Increment index
                e += 1;
                i += 4; // 1 pixel
            }
        }

    } else {
        return Err(Box::new(BitmapError::UnsupportedBitmapBpp { bpp: bitmap.bpp}));
    }

    Ok(())
}

fn update_alpha_channels(data: &mut [u8], reduce: bool) {
    if reduce {
        // 8-bit -> 7-bit alpha
        for alpha in data.iter_mut().skip(3).step_by(4) {
            *alpha = match *alpha {
                0xFF => 0x80,
                _ => *alpha >> 1
            }
        }
    } else {
        // 7-bit -> 8-bit alpha
        for alpha in data.iter_mut().skip(3).step_by(4) {
            *alpha = match *alpha {
                0x80 ..= 0xFF => 0xFF, // It should max out at 0x80 but just in case
                _ => (*alpha & 0x7F) << 1
            }
        }
    }
}

pub fn write_rgba_to_file(width: u32, height: u32, rgba: &[u8], path: &Path) -> Result<(), Box<dyn Error>> {
    let mut image: RgbaImage = ImageBuffer::new(width, height);
    let mut rgba_idx;
    let mut rgba_pix: [u8; 4] = Default::default();

    for (i, p) in image.pixels_mut().enumerate() {
        rgba_idx = i << 2;
        rgba_pix.clone_from_slice(&rgba[rgba_idx..(rgba_idx + 4)]);

        *p = image::Rgba(rgba_pix);
    }

    image.save(path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use rstest::*;
    use super::*;

    #[rstest]
    #[case(64, 64, 0, 16384)]
    #[case(64, 64, 2, 21504)]
    #[case(256, 256, 4, 349184)]
    #[case(4096, 4096, 0, 67108864)]
    fn test_calc_rgba_size(#[case] w: u16, #[case] h: u16, #[case] mips: u8, #[case] expected: usize) {
        assert_eq!(expected, calc_rgba_size(w, h, mips));
    }
}