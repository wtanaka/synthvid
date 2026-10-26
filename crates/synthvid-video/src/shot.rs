//! The shot that a video specification records.
//!
//! A [`FlatShot`] is a `Scene` together with the [`Camera`] that records it. A [`Shot`] is
//! either a flat shot or a [`DeepShot`] seen through a [`Rig`](crate::rig::Rig). Each shot
//! holds only its own parts, so a flat shot cannot carry a rig and a deep shot cannot carry a
//! camera.

use crate::camera::Camera;
use crate::recorder::Recorder;
use crate::rig::DeepShot;
use synthvid_scene::Scene;

/// A scene recorded by a flat camera: the scene to render, and the camera that records it.
///
/// The parts are private, so a flat shot is built only through [`FlatShot::new`].
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct FlatShot {
    /// The scene to render: its scale, background, and objects.
    scene: Scene,
    /// The camera that records the scene: its view, optics, and recorder.
    camera: Camera,
}

impl FlatShot {
    /// Creates a flat shot from a scene and the camera that records it. The camera is copied
    /// into the shot.
    #[must_use]
    pub fn new(scene: Scene, camera: &Camera) -> Self {
        Self::from_camera(scene, camera.clone())
    }

    /// Creates a flat shot from a scene and the camera that records it, taking ownership of
    /// the camera.
    pub(crate) const fn from_camera(scene: Scene, camera: Camera) -> Self {
        Self { scene, camera }
    }

    /// Returns the scene to render.
    #[must_use]
    pub const fn scene(&self) -> &Scene {
        &self.scene
    }

    /// Returns the camera that records the scene.
    #[must_use]
    pub const fn camera(&self) -> &Camera {
        &self.camera
    }
}

/// The shot a video specification records: a flat shot, or a deep shot.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum Shot {
    /// A scene recorded by a flat camera. Boxed, so the variants are of similar size.
    Flat(Box<FlatShot>),
    /// A stage of depth layers seen through a three-dimensional rig. Boxed, so the variants
    /// are of similar size.
    Deep(Box<DeepShot>),
}

impl Shot {
    /// Returns what records the shot's light: the camera's recorder for a flat shot, and the
    /// rig's recorder for a deep shot.
    #[must_use]
    pub fn recorder(&self) -> Recorder {
        match self {
            Self::Flat(flat) => flat.camera().recorder(),
            Self::Deep(deep) => deep.rig().recorder(),
        }
    }

    /// Returns the flat shot, or `None` for a deep shot.
    #[must_use]
    pub fn flat(&self) -> Option<&FlatShot> {
        match self {
            Self::Flat(flat) => Some(flat.as_ref()),
            Self::Deep(_) => None,
        }
    }
}

#[cfg(test)]
mod tests;
