//! The scene of a catalog entry: a flat scene of a background, an object and its motion, or one
//! of the deep shots.
//!
//! A deep shot has its own stage, so it takes none of the flat axes. Holding the two kinds of
//! scene in one enum keeps a deep entry from carrying a background, shape or motion, and a flat
//! entry from carrying a depth effect.

use super::cover_axes::{BackgroundAxis, MotionAxis, ShapeAxis};

/// A flat scene: the background, the shape of the object and the object's motion.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct FlatScene {
    /// Background type.
    background: BackgroundAxis,
    /// Shape type.
    shape: ShapeAxis,
    /// Motion type.
    motion: MotionAxis,
}

impl FlatScene {
    /// A flat scene with the given background, object shape and object motion.
    #[must_use]
    pub const fn new(background: BackgroundAxis, shape: ShapeAxis, motion: MotionAxis) -> Self {
        Self {
            background,
            shape,
            motion,
        }
    }

    /// Background type.
    #[must_use]
    pub const fn background(self) -> BackgroundAxis {
        self.background
    }

    /// Shape type.
    #[must_use]
    pub const fn shape(self) -> ShapeAxis {
        self.shape
    }

    /// Motion type.
    #[must_use]
    pub const fn motion(self) -> MotionAxis {
        self.motion
    }
}

/// The depth effect of a deep shot. Every deep shot is one of these four; a flat scene is not
/// a deep shot.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum DeepAxis {
    /// Flat layers at several depths, seen by a moving perspective camera: near layers
    /// move faster across the frame than far ones.
    Parallax,
    /// Flat layers at different depths, seen by a perspective camera rolling about its axis.
    Roll,
    /// Point highlights at several depths, seen through a disc-shaped iris focused at one
    /// of them.
    DiscBokeh,
    /// The same highlights seen through a six-bladed iris, so each blur is a hexagon.
    HexBokeh,
}

/// The scene of a catalog entry.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum EntryScene {
    /// An ordinary flat scene, recorded by a flat camera.
    Flat(FlatScene),
    /// A deep shot, seen through a perspective camera.
    Deep(DeepAxis),
}

impl EntryScene {
    /// The depth effect of this scene: `None` for a flat scene, and the effect of the shot for a
    /// deep one.
    #[must_use]
    pub const fn depth(self) -> Option<DeepAxis> {
        match self {
            Self::Flat(_) => None,
            Self::Deep(deep) => Some(deep),
        }
    }

    /// The flat scene, or `None` for a deep shot, which has no flat axes.
    #[must_use]
    pub const fn flat(self) -> Option<FlatScene> {
        match self {
            Self::Flat(flat) => Some(flat),
            Self::Deep(_) => None,
        }
    }
}
