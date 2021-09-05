#[macro_use]
extern crate lazy_static;
use rand::Rng;
use raylib::prelude::*;
use rayon::prelude::*;
use specs::DispatcherBuilder;
use specs::{
    Builder, Entities, Join, ParJoin, Read, ReadStorage, System, World, WorldExt, Write,
    WriteStorage,
};
use std::collections::HashSet;

mod bvh;
mod collider;
mod physics;
mod renderer;
mod utils;
// mod tests;

const RADIUS: f32 = 10.0f32;
const COLLISION_FRICTION: f32 = 0.998f32;
// const COLLISION_FRICTION: f32 = 1f32;
// const FRICTION: f32 = 0.998f32;
const FRICTION: f32 = 1f32;
const WINDOW_SIZE: [i32; 2] = [1400, 1000];
const SCREEN_BOUNDS: [f32; 4] = [0f32, 0f32, WINDOW_SIZE[0] as f32, WINDOW_SIZE[1] as f32];
const INITIAL_VELOCITY: f32 = 400f32;
// const GRAVITY: f32 = 1f32;
const GRAVITY: f32 = 0f32;
// const MIN_BHV_UPDATE_TIME: f32 = 1f32;
const MIN_BHV_UPDATE_TIME: f32 = 0.1f32;
lazy_static! {
    static ref HS1: HashSet<i8> = vec![0].into_iter().collect();
    static ref HS2: HashSet<i8> = vec![1].into_iter().collect();
}

type RenderingData<'a> = (
    ReadStorage<'a, renderer::Renderer>,
    ReadStorage<'a, utils::Position>,
    ReadStorage<'a, physics::Physics>,
    WriteStorage<'a, collider::Collider>,
);

type BvhData<'a> = (
    Entities<'a>,
    ReadStorage<'a, utils::Position>,
    ReadStorage<'a, collider::Collider>,
);

#[derive(Default)]
struct Delta(f32);

struct UpdatePhysics;

impl<'a> System<'a> for UpdatePhysics {
    type SystemData = (
        Write<'a, Option<bvh::BVHTree>>,
        Entities<'a>,
        ReadStorage<'a, collider::Collider>,
        Read<'a, Delta>,
        WriteStorage<'a, utils::Position>,
        WriteStorage<'a, physics::Physics>,
    );

    fn run(&mut self, (mut bvh_tree, ents, col, delta, mut pos, mut phys): Self::SystemData) {
        (&mut phys).par_join().for_each(|phys| {
            phys.velocity.y += GRAVITY;
            phys.velocity *= FRICTION;
        });

        // make this parrelel
        if let Some(ref mut bvh) = *bvh_tree {
            for (pos, phys, col_m, ent) in (&mut pos, &mut phys, (&col).maybe(), &ents).join() {
                let old_pos = pos.0.clone();
                phys.update(&mut pos.0, delta.0);
                if let Some(col) = col_m {
                    bvh.update(
                        (col.get_bounding_box(&old_pos), ent.id() as u32),
                        (col.get_bounding_box(&pos.0), ent.id() as u32),
                    );
                }
            }
        }
    }
}

struct CollideBounds;

impl<'a> System<'a> for CollideBounds {
    type SystemData = (
        WriteStorage<'a, utils::Position>,
        ReadStorage<'a, collider::Collider>,
        WriteStorage<'a, physics::Physics>,
    );

    fn run(&mut self, (mut pos, col, mut phys): Self::SystemData) {
        (&mut pos, &col, &mut phys)
            .par_join()
            .for_each(|(pos, col, phys)| {
                let overlap_vec = col.get_collision_bounds(&pos.0, SCREEN_BOUNDS);
                if let Some(unwraped) = overlap_vec {
                    phys.collide_bound(&mut pos.0, unwraped);
                }
            });
    }
}

struct CollideEnities;

impl<'a> System<'a> for CollideEnities {
    type SystemData = (
        Read<'a, Option<bvh::BVHTree>>,
        WriteStorage<'a, utils::Position>,
        ReadStorage<'a, collider::Collider>,
        WriteStorage<'a, physics::Physics>,
    );

    fn run(&mut self, mut data: Self::SystemData) {
        let bvh_tree = data.0;
        let mut entity_data: Vec<(
            &mut utils::Position,
            &collider::Collider,
            &mut physics::Physics,
        )> = (&mut data.1, &data.2, &mut data.3).join().collect();

        // costly
        let old_positions: Vec<Vector2> =
            (&entity_data).into_iter().map(|t| t.0 .0.clone()).collect();

        if let Some(ref bvh) = *bvh_tree {
            // 1323 50fps
            // 5193 50fps
            for i in 1..entity_data.len() + 1 {
                let hs = &*HS1;
                // let hs = &HS1;

                let (l, r) = entity_data.split_at_mut(i);
                let p = &mut l[l.len() - 1];
                let old_pos = &old_positions[i - 1];
                let collisions = bvh.query_rect(p.1.get_bounding_box(&old_pos), Some(hs));

                for p2_index in &collisions {
                    // make sure collisions are not handled twice
                    if p2_index >= &&(i as u32) {
                        // println!("{:?}", p2_index);
                        let p2m = &mut r[(**p2_index) as usize - i];
                        let p2_pos = &old_positions[(**p2_index) as usize];
                        let overlap_vec = p.1.get_collision(&old_pos, &p2_pos, &p2m.1);
                        if let Some(unwraped) = overlap_vec {
                            p.2.resolve_collision(&mut p.0 .0, &mut p2m.0 .0, &mut p2m.2, unwraped);
                        }
                    }
                }
            }
        }
    }
}

/// update loop
fn main() {
    let (mut rl, thread) = raylib::init()
        .size(WINDOW_SIZE[0], WINDOW_SIZE[1])
        .title("Hello, World")
        .build();

    let mut time_since_bvh_update = 0f32;
    let bvh_tree: Option<bvh::BVHTree> = None;

    let mut world = World::new();
    world.register::<utils::Position>();
    world.register::<physics::Physics>();
    world.register::<collider::Collider>();
    world.register::<renderer::Renderer>();
    world.insert(Delta(0.00));
    world.insert(bvh_tree);
    let mut dispatcher = DispatcherBuilder::new()
        .with(UpdatePhysics, "update_physics", &[])
        .with(CollideBounds, "collide_bounds", &["update_physics"])
        .with(CollideEnities, "collide_entities", &["update_physics"])
        // .with(HelloWorld, "hello_updated", &["update_pos"])
        .build();

    let mut timer = rl.get_time();
    let mut rng = rand::thread_rng();

    let mut entity_count = 0;

    while !rl.window_should_close() {
        dispatcher.dispatch(&mut world);
        world.maintain();

        let mouse_pos = rl.get_mouse_position();

        {
            let mut delta = world.write_resource::<Delta>();
            *delta = Delta(rl.get_frame_time());
            time_since_bvh_update += delta.0;
        }

        if rl.is_key_pressed(KeyboardKey::KEY_R) {
            entity_count = 0;
            world.delete_all();
        }

        if rl.is_key_pressed(KeyboardKey::KEY_SPACE) {
            timer = rl.get_time();
        }

        if rl.get_fps() > 50 {
            // if rl.is_key_down(KeyboardKey::KEY_SPACE) {
            if rl.get_time() - timer > 0.01 {
                let position = Vector2::new(rng.gen::<f32>() * WINDOW_SIZE[0] as f32, 0f32);
                let radius = 5f32 + RADIUS * ((rng.gen::<u8>() % 32) as f32) / 128f32;
                let mut particle_physics = physics::Physics::new(radius);
                let mut rand_vec = Vector2::new(0f32, 0f32);
                while rand_vec.length_sqr() == 0f32 {
                    rand_vec = Vector2::new(rng.gen::<f32>(), rng.gen::<f32>());
                }
                rand_vec.normalize();
                rand_vec.scale(INITIAL_VELOCITY);
                particle_physics.velocity = rand_vec;
                entity_count += 1;
                world
                    .create_entity()
                    .with(utils::Position(position))
                    .with(particle_physics)
                    .with(collider::Collider::CircleCollider { radius })
                    .with(renderer::Renderer::CircleRenderer {
                        radius,
                        color: Color::new(0, 0, 0, 255),
                    })
                    .build();
                time_since_bvh_update = 1f32 + MIN_BHV_UPDATE_TIME;
                timer = rl.get_time();
            }
        }

        {
            let mut system_data: (WriteStorage<physics::Physics>, ReadStorage<utils::Position>) =
                world.system_data();
            for (phys, pos) in (&mut system_data.0, &system_data.1).join() {
                if rl.is_mouse_button_down(MouseButton::MOUSE_LEFT_BUTTON) {
                    phys.velocity += (mouse_pos - pos.0).normalized() * 10f32;
                }
            }
        }

        let l_m_down = rl.is_mouse_button_down(MouseButton::MOUSE_RIGHT_BUTTON);

        let mut d = rl.begin_drawing(&thread);
        d.clear_background(Color::WHITE);

        {
            let system_data: BvhData = world.system_data();
            let mut bvh_data: Write<Option<bvh::BVHTree>> = world.system_data();
            if time_since_bvh_update > MIN_BHV_UPDATE_TIME {
                *bvh_data = Some(create_bvh(system_data));
                // println!("{:?}", time_since_bvh_update);
                time_since_bvh_update = 0f32;
            }
        }

        {
            let system_data: RenderingData = world.system_data();
            for data in (
                &system_data.0,
                &system_data.1,
                (&system_data.2).maybe(),
                (&system_data.3).maybe(),
            )
                .join()
            {
                let (r, pos, phys, col) = data;
                r.render(&mut d, pos);
                if l_m_down {
                    if let Some(c) = col {
                        let bb = c.get_bounding_box(&pos.0);
                        let bb_size = bb[1] - bb[0];
                        d.draw_rectangle_lines(
                            bb[0].x as i32,
                            bb[0].y as i32,
                            bb_size.x as i32,
                            bb_size.y as i32,
                            Color::new(0, 255, 0, 100),
                        )
                    }
                }
                // d.draw_circle_v(p.position, 10f32, Color::new(255, 0, 255, 0));
            }
        }

        d.draw_fps(0, 0);
        d.draw_text(
            format!("{:?}", entity_count).as_str(),
            0,
            20,
            20,
            if time_since_bvh_update < f32::EPSILON {
                Color::RED
            } else {
                Color::BLACK
            },
        );
    }
}

fn create_bvh(entities: BvhData) -> bvh::BVHTree {
    let mut data = Vec::new();

    for entity in (&entities.0, &entities.1, &entities.2).join() {
        let (ent, pos, col) = entity;
        let id = ent.id();
        // let mut hs = HashSet::new();
        // hs.insert(0);
        // hs.insert(0);
        data.push((col, pos.0, col.get_bounding_box(&pos.0), id, HS1.clone()));
    }

    bvh::BVHTree::new(data)
}
