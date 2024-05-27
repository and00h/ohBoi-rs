use std::cmp::{max, max_by, min};
use imgui::{StyleColor, StyleVar, Ui};

pub fn hex_view(ui: &Ui, bytes_per_row: usize, data: &[u8], tab_threshold: Option<usize>, selected: &mut usize) {
    let spacing = ui.clone_style().item_spacing;
    let offset_width = max_by(ui.calc_text_size("0".repeat(4))[0], ui.calc_text_size("Offset")[0], |v1,v2| v1.partial_cmp(v2).unwrap()) + spacing[0];
    let hex_width = (ui.calc_text_size("00")[0] + spacing[0]) * (bytes_per_row - 1) as f32;
    let ascii_width = ui.calc_text_size(" ".repeat(bytes_per_row))[0] + spacing[0] * (bytes_per_row - 1) as f32;
    let char_width = ui.calc_text_size("0")[0] + spacing[0];

    let bank_size = tab_threshold.unwrap_or(data.len());
    let banks = (data.len() + bank_size - 1) / bank_size;
    if banks > 1 {
        ui.combo("##hex_view_combo", selected, &(0..banks).collect::<Vec<usize>>(), |i| std::borrow::Cow::from(format!("Bank {}", i)));
    }

    ui.columns(3, "hex_view", true);
    ui.set_column_width(0, offset_width);
    ui.set_column_width(1, hex_width);
    ui.set_column_width(2, ascii_width);

    ui.text("Offset");
    ui.next_column();
    ui.text("Hex");
    ui.next_column();
    ui.text("ASCII");
    ui.next_column();
    for (i, chunk) in data[(bank_size * *selected)..min(bank_size * (*selected + 1), data.len())].chunks(bytes_per_row).enumerate() {
        ui.text(format!("{:04X}", bank_size * *selected + i * bytes_per_row));
        ui.next_column();
        for byte in chunk.iter() {
            ui.text(format!("{:02X}", byte));
            ui.same_line();
        }
        ui.next_column();
        let mut ascii_str = String::new();
        chunk.iter().for_each(|byte| match *byte {
            0x20..=0x7E => ascii_str.push(*byte as char),
            _ => ascii_str.push('.')
        });
        ui.text(ascii_str);
        ui.next_column();
    }
}

#[cfg(feature = "debug_ui")]
pub(super) struct HexView {
    // ...
    toggle: bool,
    current_bank: usize
    // ...
}

#[cfg(feature = "debug_ui")]
impl HexView {
    // ...
    pub fn new() -> Self {
        HexView {
            toggle: true,
            current_bank: 0
        }
    }

    pub fn show(&mut self, ui: &mut Ui, data: &[u8], position: [f32; 2]) {
        if !self.toggle {
            return;
        }
        let spacing = unsafe { ui.style().item_spacing[0] };
        let mut window_width = ui.calc_text_size("Offset")[0] + spacing +
            ui.calc_text_size("00")[0] * 16.0 + spacing * 15.0 +
            ui.calc_text_size(" ".repeat(16))[0] + spacing;
        ui.window("Hex View")
            .position(position, imgui::Condition::FirstUseEver)
            .size([window_width, 400.0], imgui::Condition::FirstUseEver)
            .build(|| {
                hex_view(ui, 16, data, Some(0x4000), &mut self.current_bank);
            });
    }

    pub fn toggle(&mut self) {
        self.toggle = !self.toggle;
    }
}