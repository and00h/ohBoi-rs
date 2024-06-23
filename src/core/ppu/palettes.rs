const DMG_PALETTE: [[u8; 4]; 4] = [
    [0xFF, 0xFF, 0xFF, 0xFF],
    [0xCC, 0xCC, 0xCC, 0xFF],
    [0x77, 0x77, 0x77, 0xFF],
    [0x00, 0x00, 0x00, 0xFF]
];

pub struct DmgPalette {
    pub(super) value: u8,
    colors: [[u8; 4]; 4]
}

impl DmgPalette {
    pub fn new(value: u8) -> Self {
        let mut res = Self {
            value: 0,
            colors: [[0; 4]; 4]
        };

        res.update_palette(value);
        res
    }

    pub fn update_palette(&mut self, mut new_value: u8) {
        self.value = new_value;
        self.colors
            .iter_mut()
            .for_each(|color| {
                let new_color = &DMG_PALETTE[(new_value & 3) as usize];
                color.copy_from_slice(new_color);
                new_value >>= 2;
            })
    }

    pub fn colors(&self) -> &[[u8; 4]; 4] {
        &self.colors
    }

    pub fn value(&self) -> u8 {
        self.value
    }
}
