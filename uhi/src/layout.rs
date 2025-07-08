use std::mem;

use crate::{Rect, Vec2};

// heavilly inspired by ratatui - <https://ratatui.rs/concepts/layout/>; nearly infinitely simpler.

pub enum Constraint {
    Length(f32),
    /// accepts value in range of 0.0..1.0.
    ///
    /// the value gets multiplied by the area's dimension.
    Percentage(f32),
    /// accepts [None] or [Some] non-negative weight.
    ///
    /// fill excess available space, proportionally matching other [Constraint::Fill] elements
    /// while satisfying all other constraints.
    Fill(f32),
}

pub enum Direction {
    Horizontal,
    Vertical,
}

pub struct Stack<const N: usize> {
    constraints: [Constraint; N],
    direction: Direction,
}

impl<const N: usize> Stack<N> {
    pub fn new(direction: Direction, constraints: [Constraint; N]) -> Self {
        assert!(N > 0);
        Self {
            direction,
            constraints,
        }
    }

    pub fn split(self, area: Rect) -> [Rect; N] {
        let (main_dimension, mut main_offset, cross_dimension) = match self.direction {
            Direction::Horizontal => (area.width(), area.min.x, area.height()),
            Direction::Vertical => (area.height(), area.min.y, area.width()),
        };

        let mut remaining_space = main_dimension;
        let mut resolved_sizes = [0.0_f32; N];
        let mut total_fill_weight: f32 = 0.0;

        // pass:
        // resolve length and percentage constraints, compute total fill weight.
        for (constraint, res) in self.constraints.iter().zip(resolved_sizes.iter_mut()) {
            match *constraint {
                Constraint::Length(length) => {
                    remaining_space -= length;
                    *res = length;
                }
                Constraint::Percentage(percentage) => {
                    assert!((0.0..1.0).contains(&percentage), "invalid percentage");
                    let length = main_dimension * percentage;
                    remaining_space -= length;
                    *res = length;
                }
                Constraint::Fill(fill_weight) => {
                    total_fill_weight += fill_weight;
                }
            };
        }

        // maybe pass:
        // resolve fill constraints with remaining space.
        if total_fill_weight > 0.0 {
            for (constraint, res) in self.constraints.iter().zip(resolved_sizes.iter_mut()) {
                if let (Constraint::Fill(fill_weight), res @ 0.0) = (constraint, res) {
                    *res = remaining_space * (fill_weight / total_fill_weight);
                };
            }
        }

        // pass:
        // position rects sequentially along the main dimension.
        let mut ret = mem::MaybeUninit::<[Rect; N]>::uninit();
        for (i, size) in resolved_sizes.into_iter().enumerate() {
            let rect = match self.direction {
                Direction::Horizontal => Rect::new(
                    Vec2::new(main_offset, area.min.y),
                    Vec2::new(main_offset + size, area.min.y + cross_dimension),
                ),
                Direction::Vertical => Rect::new(
                    Vec2::new(area.min.x, main_offset),
                    Vec2::new(area.min.x + cross_dimension, main_offset + size),
                ),
            };
            unsafe { ret.as_mut_ptr().cast::<Rect>().add(i).write(rect) };
            main_offset += size;
        }
        unsafe { ret.assume_init() }
    }
}

pub fn hstack<const N: usize>(constraints: [Constraint; N]) -> Stack<N> {
    Stack::new(Direction::Horizontal, constraints)
}

pub fn vstack<const N: usize>(constraints: [Constraint; N]) -> Stack<N> {
    Stack::new(Direction::Vertical, constraints)
}
