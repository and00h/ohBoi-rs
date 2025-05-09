// Copyright Antonio Porsia 2025. Licensed under the EUPL-1.2 or later.

use std::cmp::{max_by, min};
use imgui::{Condition, Ui};
pub fn hex_view(ui: &Ui, bytes_per_row: usize, data: &[u8], split_threshold: Option<usize>, selected: &mut usize, id: Option<&str>) {
    let spacing = ui.clone_style().item_spacing;
    let offset_width = max_by(ui.calc_text_size("0".repeat(6))[0], ui.calc_text_size("Offset")[0], |v1,v2| v1.partial_cmp(v2).unwrap()) + spacing[0];
    let hex_width = (ui.calc_text_size("00")[0] + spacing[0]) * bytes_per_row as f32;
    let ascii_width = ui.calc_text_size(" ".repeat(bytes_per_row))[0] + spacing[0] * (bytes_per_row - 1) as f32;

    let bank_size = split_threshold.unwrap_or(data.len());
    let banks = data.len().div_ceil(bank_size);
    if banks > 1 {
        ui.combo(format!("{}{}", id.unwrap_or("_"), "##hex_view_combo"), selected, &(0..banks).collect::<Vec<usize>>(), |i| std::borrow::Cow::from(format!("Bank {}", i)));
    }

    ui.columns(3, format!("{}{}", id.unwrap_or("_"), "##hex_view_columns"), true);
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
        ui.text(format!("{:06X}", bank_size * *selected + i * bytes_per_row));
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

pub struct HexView {
    // ...
    toggle: bool,
    current_bank: usize,
    title: String
    // ...
}

impl HexView {
    // ...
    pub fn new(title: String) -> Self {
        HexView {
            toggle: true,
            current_bank: 0,
            title
        }
    }

    pub fn show(&mut self, ui: &mut Ui, data: &[u8], position: [f32; 2], split_threshold: Option<usize>) {
        if !self.toggle {
            return;
        }
        let width = calc_hex_view_width(ui, 16);
        ui.window(self.title.as_str())
            .position(position, Condition::FirstUseEver)
            .size([width, 300.0], Condition::FirstUseEver)
            .build(|| {
                hex_view(ui, 16, data, split_threshold, &mut self.current_bank, None);
            });
    }

    pub fn toggle(&mut self) {
        self.toggle = !self.toggle;
    }
}

pub fn calc_hex_view_width(ui: &mut Ui, bytes_per_row: usize) -> f32 {
    let spacing = ui.clone_style().item_spacing[0];
    let offset_width = max_by(ui.calc_text_size("0".repeat(6))[0], ui.calc_text_size("Offset")[0], |v1,v2| v1.partial_cmp(v2).unwrap()) + spacing;
    let hex_width = (ui.calc_text_size("00")[0] + spacing) * bytes_per_row as f32;
    let ascii_width = ui.calc_text_size(" ".repeat(bytes_per_row))[0] + spacing;

    offset_width + hex_width + ascii_width + ui.clone_style().columns_min_spacing * 2.0 + ui.clone_style().frame_padding[0] * 2.0
}