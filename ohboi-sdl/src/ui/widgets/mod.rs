// Copyright Antonio Porsia 2025. Licensed under the EUPL-1.2 or later.

pub mod hexview;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use imgui::{Condition, StyleColor, Ui};
use ohboi_core::cpu::Register16;
use ohboi_core::GameBoy;
use crate::logging::ImguiLogString;

pub fn log_window(ui: &Ui, title: &str, log: Arc<Mutex<VecDeque<ImguiLogString>>>) {
    ui.window(title)
        .size([400.0, 300.0], Condition::FirstUseEver)
        .build(|| {
            if ui.button("Clear") {
                log.lock().unwrap().clear();
            }
            ui.child_window("log_child")
                .border(true)
                .always_vertical_scrollbar(true)
                .build(|| {
                    for line in log.lock().unwrap().iter() {
                        let _text_color = match line.level {
                            log::Level::Error => ui.push_style_color(StyleColor::Text, [1.0, 0.0, 0.0, 1.0]),
                            log::Level::Warn => ui.push_style_color(StyleColor::Text, [1.0, 1.0, 0.0, 1.0]),
                            log::Level::Info => ui.push_style_color(StyleColor::Text, [0.0, 1.0, 1.0, 1.0]),
                            log::Level::Debug => ui.push_style_color(StyleColor::Text, [0.0, 1.0, 0.0, 1.0]),
                            log::Level::Trace => ui.push_style_color(StyleColor::Text, [1.0, 0.0, 1.0, 1.0]),
                        };
                        ui.text(line.text.as_str());
                        ui.set_scroll_y(ui.scroll_max_y());
                    }
                });
        });
}

#[cfg(feature = "debug_ui")]
pub(super) struct DisassemblyView {
    // ...
    toggle: bool,
    title: String
    // ...
}

#[cfg(feature = "debug_ui")]

impl DisassemblyView {
    // ...
    pub fn new(title: String) -> Self {
        DisassemblyView {
            toggle: true,
            title
        }
    }

    pub fn show(&mut self, ui: &mut Ui, gb: &mut GameBoy, position: [f32; 2]) {
        if !self.toggle {
            return;
        }
        let instructions = gb.get_current_instruction_window();
        let cur_pc = gb.get_current_instr_pc();
        ui.window(self.title.as_str())
            .position(position, Condition::FirstUseEver)
            .size([500.0, 400.0], Condition::FirstUseEver)
            .build(|| {
                if gb.is_running() {
                    if ui.button("Stop") {
                        gb.debug_stop();
                    }
                } else if ui.button("Continue") {
                    gb.debug_continue();
                }
                ui.same_line();
                if ui.button("Step") {
                    gb.debug_step();
                }
                ui.columns(2, "##disassembly_view_columns_ao", false);
                ui.set_column_width(0, 300.0);
                ui.set_column_width(1, 100.0);
                ui.text("Disassembly");
                ui.next_column();
                ui.text("Registers");
                ui.next_column();
                ui.child_window("disass")
                    .border(false)
                    .build(|| {
                        ui.columns(2, "##disassembly_view_columns", true);
                        ui.set_column_width(0, 100.0);
                        ui.set_column_width(1, 200.0);
                        ui.text("Address");
                        ui.next_column();
                        ui.text("Instruction");
                        ui.next_column();
                        for (addr, instruction) in instructions.iter() {
                            let color = if *addr == cur_pc {
                                ui.push_style_color(StyleColor::Text, [1.0, 0.0, 0.0, 1.0])
                            } else {
                                ui.push_style_color(StyleColor::Text, [1.0, 1.0, 1.0, 1.0])
                            };
                            ui.text(format!("{:04X}", addr));
                            ui.next_column();
                            ui.text(instruction);
                            ui.next_column();
                            color.pop();
                        }
                    });
                ui.next_column();
                let registers = gb.get_cpu_registers();
                ui.text(format!("AF: {:04X}", registers.get_reg16(Register16::AF)));
                ui.text(format!("BC: {:04X}", registers.get_reg16(Register16::BC)));
                ui.text(format!("DE: {:04X}", registers.get_reg16(Register16::DE)));
                ui.text(format!("HL: {:04X}", registers.get_reg16(Register16::HL)));
            });
    }

    pub fn toggle(&mut self) {
        self.toggle = !self.toggle;
    }
}