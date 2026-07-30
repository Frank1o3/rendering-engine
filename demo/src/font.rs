use glam::{Quat, Vec3};

use rendering_engine::render::frame_data::RenderCommand;
use rendering_engine::render::renderer::MeshId;
use rendering_engine::resources::material::MaterialId;

const FONT: [[u8; 5]; 10] = [
    [0b111, 0b101, 0b101, 0b101, 0b111], // 0
    [0b010, 0b010, 0b010, 0b010, 0b010], // 1
    [0b111, 0b001, 0b111, 0b100, 0b111], // 2
    [0b111, 0b001, 0b111, 0b001, 0b111], // 3
    [0b101, 0b101, 0b111, 0b001, 0b001], // 4
    [0b111, 0b100, 0b111, 0b001, 0b111], // 5
    [0b111, 0b100, 0b111, 0b101, 0b111], // 6
    [0b111, 0b001, 0b010, 0b010, 0b010], // 7
    [0b111, 0b101, 0b111, 0b101, 0b111], // 8
    [0b111, 0b101, 0b111, 0b001, 0b111], // 9
];

fn char_glyph(ch: char) -> Option<[u8; 5]> {
    match ch.to_ascii_uppercase() {
        'A' => Some([0b111, 0b101, 0b111, 0b101, 0b101]),
        'C' => Some([0b111, 0b100, 0b100, 0b100, 0b111]),
        'E' => Some([0b111, 0b100, 0b111, 0b100, 0b111]),
        'F' => Some([0b111, 0b100, 0b111, 0b100, 0b100]),
        'I' => Some([0b111, 0b010, 0b010, 0b010, 0b111]),
        'O' => Some([0b111, 0b101, 0b101, 0b101, 0b111]),
        'P' => Some([0b111, 0b101, 0b111, 0b100, 0b100]),
        'R' => Some([0b111, 0b101, 0b111, 0b101, 0b101]),
        'S' => Some([0b111, 0b100, 0b111, 0b001, 0b111]),
        _ => None,
    }
}

pub fn emit_ui_text(
    commands: &mut Vec<RenderCommand>,
    mesh_id: MeshId,
    material_id: MaterialId,
    mut x: f32,
    y: f32,
    text: &str,
    pixel_size: f32,
) {
    for ch in text.chars() {
        match ch {
            ' ' => {
                x += 4.0 * pixel_size;
                continue;
            }

            ':' => {
                for &row in &[1u32, 3] {
                    commands.push(RenderCommand {
                        mesh_id,
                        material_id,
                        position: Vec3::new(x + pixel_size, y + row as f32 * pixel_size, 0.0),
                        rotation: Quat::IDENTITY,
                        scale: pixel_size * 0.9,
                    });
                }

                x += 3.0 * pixel_size;
                continue;
            }

            _ => {}
        }

        let glyph = if ch.is_ascii_digit() {
            Some(FONT[(ch as u8 - b'0') as usize])
        } else {
            char_glyph(ch)
        };

        if let Some(rows) = glyph {
            for (row_idx, &row) in rows.iter().enumerate() {
                for col in 0..3u32 {
                    if (row & (1 << (2 - col))) != 0 {
                        commands.push(RenderCommand {
                            mesh_id,
                            material_id,
                            position: Vec3::new(
                                x + col as f32 * pixel_size,
                                y + row_idx as f32 * pixel_size,
                                0.0,
                            ),
                            rotation: Quat::IDENTITY,
                            scale: pixel_size * 0.9,
                        });
                    }
                }
            }
        }

        x += 4.0 * pixel_size;
    }
}
