//! Cube-map maths shared by the 3D preview and the cubemap pipeline. The maths lives in `texcore::cube` (Notes DS1,
//! HX5) and is re-exported here; this module keeps the cube jobs and the pipeline stages.
pub mod checks;
pub mod filter;
pub mod layout;
pub mod marker;
pub mod plan;
pub(crate) mod run;

pub use texcore::cube::{cube_dir, dir_at, dir_to_face, latlong, rgbe, sample, world, Cube, FACES};

use crate::config::CubeLayout;
use crate::policy::SourceStamp;
use std::path::PathBuf;

/// How a cube job is built (`plan::plan_cubes`): one cross/strip/cube DDS file, or six face files in +X..-Z order.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct CubeJob {
    pub layout: CubeLayout,
    pub sources: Vec<CubeSource>,
}

/// One source file of a cube job, with its stamp when planned; `face` is its face index for a six-file cube.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct CubeSource {
    pub path: PathBuf,
    pub stamp: Option<SourceStamp>,
    pub face: Option<usize>,
}
