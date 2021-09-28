use crate::utils::Position;
use crate::ReadStorage;
use crate::System;
use raylib::prelude::*;
use specs::WriteExpect;

use specs::{Component, VecStorage};

/// currently only handles rendering primitives, but raylib supports sprites.
#[derive(Debug, Clone, Component)]
#[storage(VecStorage)]
pub enum Renderer {
    CircleRenderer { radius: f32, color: Color },
    RectangeRenderer { size: Vector2, color: Color },
}

impl Renderer {
    pub fn render(&self, d: &mut RaylibDrawHandle, position: &Position) {
        match self {
            Renderer::CircleRenderer { radius, color } => {
                d.draw_circle_v(position.0 + (Vector2::one() * (*radius)), *radius, color);
            }
            Renderer::RectangeRenderer { size, color } => {
                d.draw_rectangle_v(position.0, size, color);
            }
        }
    }
}