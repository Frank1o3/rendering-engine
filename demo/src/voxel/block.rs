// src/voxel/block.rs

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[repr(transparent)]
pub struct BlockId(pub u8);

impl BlockId {
    pub const AIR: BlockId = BlockId(0);
    pub const STONE: BlockId = BlockId(1);
    pub const DIRT: BlockId = BlockId(2);
    pub const GRASS: BlockId = BlockId(3);

    #[inline]
    pub fn is_solid(self) -> bool {
        self != Self::AIR
    }

    /// Flat placeholder color per block type — Vertex has no UV channel yet
    /// (see engine/src/mesh.rs), so texturing is a separate engine change
    /// (Vertex format + atlas) for a later phase. This is enough to verify
    /// the meshing pipeline visually in the meantime.
    pub fn color(self) -> [u8; 4] {
        match self {
            Self::STONE => [120, 120, 125, 255],
            Self::DIRT => [110, 80, 55, 255],
            Self::GRASS => [95, 160, 70, 255],
            _ => [255, 0, 255, 255], // magenta = "forgot to add a color"
        }
    }
}
