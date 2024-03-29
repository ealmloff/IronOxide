use crate::bvh::BVHTree;
use crate::collider;
use crate::Entities;
use crate::World;
use crate::Write;
use raylib::core::math::Vector2;
use specs::{Component, VecStorage};

#[derive(Debug, Clone, Component)]
#[storage(VecStorage)]
pub struct Position(pub Vector2);

#[derive(Debug, Component)]
#[storage(VecStorage)]
pub struct Frozen;

#[derive(Debug, Component)]
#[storage(VecStorage)]
pub struct Collisions(pub Vec<u32>);

#[derive(Default)]
pub struct Delta(pub f32);

pub fn to_arr(v: Vector2) -> [f32; 2] {
    [v.x, v.y]
}

pub fn from_tuple(t: [f32; 2]) -> Vector2 {
    Vector2::new(t[0], t[1])
}

pub fn register_ent(
    tuple_data: (&collider::Collider, Vector2, collider::AABB, u32),
    world: &mut World,
) {
    let mut bvh_write: Write<Option<BVHTree>> = world.system_data();
    if let Some(ref mut bvh) = *bvh_write {
        bvh.insert(&tuple_data);
    }
}

pub fn delete_ent(ent: u32, ents: &mut Entities, bvh: &mut BVHTree) {
    bvh.delete(ent);
    ents.delete(ents.entity(ent)).unwrap();
}
