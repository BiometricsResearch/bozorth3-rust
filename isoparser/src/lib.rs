use std::convert::TryInto;
use std::path::Path;

#[derive(Debug)]
pub struct Record {
    pub capture_equipment: u16,
    pub x_image_size: u16,
    pub y_image_size: u16,
    pub x_resolution: u16,
    pub y_resolution: u16,
    pub views: Vec<View>,
}

#[derive(Debug)]
pub struct View {
    pub finger_position: u8,
    pub impr_type: u8,
    pub finger_quality: u8,
    pub minutiae: Vec<Minutia>,
}

#[derive(Debug)]
pub struct Minutia {
    pub ty: MinutiaType,
    pub x: u16,
    pub y: u16,
    pub angle: f32,
    pub quality: u8,
}

#[derive(Debug)]
pub enum ParseError {
    InvalidFormat,
    InvalidLength,
    Io(std::io::Error),
}

#[derive(Debug, Copy, Clone)]
#[repr(u8)]
pub enum MinutiaType {
    Other = 0b00,
    RidgeEnding = 0b01,
    RidgeBifurcation = 0b10,
}

pub fn load_iso(path: impl AsRef<Path>) -> Result<Record, ParseError> {
    let file = std::fs::read(path).map_err(ParseError::Io)?;

    let format_id: [u8; 4] = file[0..4]
        .try_into()
        .map_err(|_| ParseError::InvalidFormat)?;
    if &format_id != b"FMR\0" {
        return Err(ParseError::InvalidFormat);
    }

    let length = u32::from_be_bytes(file[8..12].try_into().unwrap());
    if length != file.len() as u32 {
        return Err(ParseError::InvalidLength);
    }

    let capture_equipment = u16::from_be_bytes(file[12..14].try_into().unwrap());
    let x_image_size = u16::from_be_bytes(file[14..16].try_into().unwrap());
    let y_image_size = u16::from_be_bytes(file[16..18].try_into().unwrap());
    let x_resolution = u16::from_be_bytes(file[18..20].try_into().unwrap());
    let y_resolution = u16::from_be_bytes(file[20..22].try_into().unwrap());
    let n_finger_views = file[22];
    let _reserved_byte = file[23];

    let mut record = Record {
        capture_equipment,
        x_image_size,
        y_image_size,
        x_resolution,
        y_resolution,
        views: Vec::new(),
    };

    let views = &file[24..];
    for _ in 0..n_finger_views as usize {
        let finger_position = views[0];
        let impr_type = views[1];
        let finger_quality = views[2];
        let minutiae = views[3];

        let mut view = View {
            finger_position,
            impr_type,
            finger_quality,
            minutiae: Vec::new(),
        };

        let mut views = &views[4..];
        for _ in 0..minutiae as usize {
            let raw_x = u16::from_be_bytes(views[0..2].try_into().unwrap());
            let raw_y = u16::from_be_bytes(views[2..4].try_into().unwrap());
            const MASK: u16 = 0b11000000_00000000;
            let ty = (raw_x & MASK) >> (MASK.trailing_zeros() as u16);
            let x = raw_x & !MASK;
            let y = raw_y & !MASK;

            let angle = views[4];
            let quality = views[5];
            view.minutiae.push(Minutia {
                ty: match ty {
                    0b00 => MinutiaType::Other,
                    0b01 => MinutiaType::RidgeEnding,
                    0b10 => MinutiaType::RidgeBifurcation,
                    _ => return Err(ParseError::InvalidFormat),
                },
                x: x as u16,
                y: y as u16,
                angle: angle as f32 * 1.40625f32,
                quality,
            });
            views = &views[6..];
        }
        record.views.push(view);
    }
    Ok(record)
}
