extern crate iron_oxide;

use std::ffi::CString;

use iron_oxide::bvh::BVHTree;
use iron_oxide::raylib::rgui::RaylibDrawGui;
use iron_oxide::Builder;
use iron_oxide::Color;
use iron_oxide::Join;
use iron_oxide::RaylibDraw;
use iron_oxide::WorldExt;

const INITIAL_VELOCITY: f32 = 400f32;

struct EntCount(usize);
struct MousePos(iron_oxide::Vector2);
struct SettingsState {
    debug_bvh: bool,
    debug_aabb: bool,
    show_velocity: bool,
    radius: f32,
}

/// update loop
// 12000 particles 100fps
fn main() {
    let builder = iron_oxide::build();

    let mut data = iron_oxide::init(builder);
    let timer = data.0.get_time();
    data.2.insert(EntCount(0));
    data.2.insert(SettingsState {
        debug_bvh: false,
        debug_aabb: false,
        show_velocity: false,
        radius: 15.0,
    });
    data.2.insert(MousePos(data.0.get_mouse_position()));
    data.2.insert(timer);

    let mut rng = iron_oxide::rand::thread_rng();

    let mut speed = 1.0;

    while !data.0.window_should_close() {
        let l_m_down = data
            .0
            .is_mouse_button_down(iron_oxide::MouseButton::MOUSE_LEFT_BUTTON);

        {
            data.2.write_resource::<MousePos>().0 = data.0.get_mouse_position();
        }

        {
            let speed_delta = data.0.get_mouse_wheel_move() / 10.0;
            speed += speed_delta;
            let mut delta = data.2.write_resource::<iron_oxide::utils::Delta>();
            *delta = iron_oxide::utils::Delta(data.0.get_frame_time() * speed);
        }

        if l_m_down {
            let pos = data.2.read_resource::<MousePos>().0;
            {
                let bvh_option = &mut *data.2.write_resource::<Option<BVHTree>>();
                if let Some(bvh) = bvh_option {
                    let ents = bvh.query_point(&pos, &[true; iron_oxide::collider::LAYERS]);
                    if ents.len() > 0 {
                        let id = ents[0];
                        let mut entities: iron_oxide::Entities = data.2.system_data();

                        iron_oxide::utils::delete_ent(id, &mut entities, bvh);
                    }
                }
            }
        }
        if data.0.is_key_pressed(iron_oxide::KeyboardKey::KEY_R) {
            speed = 1.0;

            data.2.write_resource::<EntCount>().0 = 0;
            *data.2.write_resource::<Option<iron_oxide::bvh::BVHTree>>() = None;

            data.2.delete_all();
            data.2.maintain();
        }
        if data.0.is_key_down(iron_oxide::KeyboardKey::KEY_LEFT_SHIFT) {
            if data.0.get_time() - timer > 0.01 {
                gen_enity(&mut data.2, &mut rng);
            }
        }
        iron_oxide::update(&mut data, draw);
    }
}

fn draw(world: &mut iron_oxide::World, d: &mut iron_oxide::prelude::RaylibDrawHandle) {
    {
        let mut state = world.write_resource::<SettingsState>();
        state.debug_bvh = d.gui_check_box(
            iron_oxide::Rectangle::new(0.0, 60.0, 20.0, 20.0),
            Some(&CString::new("debug BVH").unwrap()),
            state.debug_bvh,
        );
        state.debug_aabb = d.gui_check_box(
            iron_oxide::Rectangle::new(0.0, 80.0, 20.0, 20.0),
            Some(&CString::new("debug bounding boxes").unwrap()),
            state.debug_aabb,
        );
        state.show_velocity = d.gui_check_box(
            iron_oxide::Rectangle::new(0.0, 100.0, 20.0, 20.0),
            Some(&CString::new("debug velocity").unwrap()),
            state.show_velocity,
        );
        state.radius = d.gui_slider(
            iron_oxide::Rectangle::new(0.0, 120.0, 100.0, 20.0),
            Some(&CString::new("1px radius").unwrap()),
            Some(&CString::new("30px radius").unwrap()),
            state.radius,
            1.0,
            30.0,
        );
    }

    {
        let mut system_data: iron_oxide::RenderingData = world.system_data();
        for data in (
            &mut system_data.0,
            &system_data.1,
            (&system_data.2).maybe(),
            (&system_data.3).maybe(),
        )
            .join()
        {
            let (r, pos, phys, col) = data;
            if let Some(p) = phys {
                match r {
                    iron_oxide::renderer::Renderer::CircleRenderer { radius: _, color } => {
                        *color = iron_oxide::Color::color_from_hsv(
                            p.velocity.angle_to(iron_oxide::Vector2::one()) * 2f32
                                / std::f32::consts::PI.to_radians(),
                            1.0,
                            1.0,
                        );
                    }
                    iron_oxide::renderer::Renderer::RectangeRenderer { size: _, color } => {
                        *color = Color::color_from_hsv(
                            p.velocity.angle_to(iron_oxide::Vector2::one()) * 2f32
                                / std::f32::consts::PI.to_radians(),
                            1.0,
                            1.0,
                        );
                    }
                    _ => (),
                }
            }
            if world.read_resource::<SettingsState>().debug_aabb {
                if let Some(c) = col {
                    let bb = c.get_bounding_box(&pos.0);
                    d.draw_rectangle_lines(
                        bb.lx as i32,
                        bb.ly as i32,
                        (bb.rx - bb.lx) as i32,
                        (bb.ry - bb.ly) as i32,
                        iron_oxide::Color::new(0, 255, 0, 100),
                    )
                }
            }
            if world.read_resource::<SettingsState>().show_velocity {
                if let Some(p) = phys {
                    if let Some(c) = col {
                        let bb = c.get_bounding_box(&pos.0);
                        let start =
                            pos.0 - iron_oxide::Vector2::new(bb.lx - bb.rx, bb.ly - bb.ry) / 2.0;
                        d.draw_line_ex(
                            start,
                            start + p.velocity / 10.0,
                            5.0,
                            iron_oxide::Color::new(255, 0, 0, 255),
                        );
                    }
                }
            }
        }

        if world.read_resource::<SettingsState>().debug_bvh {
            let mut cost = 0;
            let bvh_read: iron_oxide::Read<Option<iron_oxide::bvh::BVHTree>> = world.system_data();
            if let Some(bvh_root) = &*bvh_read {
                let p = world.read_resource::<MousePos>().0;
                for node in bvh_root
                    .debug_query_point(&p, &[true; iron_oxide::collider::LAYERS])
                    .1
                {
                    let rect;
                    match node.0 {
                        iron_oxide::bvh::Node::Branch(bb, _) => rect = bb,
                        // iron_oxide::bvh::Node::Branch(bb, _, _) => rect = bb,
                        iron_oxide::bvh::Node::Fruit(bb, _, _) => rect = bb,
                    }
                    cost += 1;
                    let mut color =
                        iron_oxide::Color::color_from_hsv(node.1 as f32 * 10.0, 1.0, 1.0);
                    color.a = 100;
                    d.draw_rectangle(
                        rect.lx as i32,
                        rect.ly as i32,
                        (rect.rx - rect.lx) as i32,
                        (rect.ry - rect.ly) as i32,
                        color,
                    );
                }
            }
            d.draw_text(
                format!("{:?} collision checks", cost).as_str(),
                0,
                40,
                20,
                iron_oxide::Color::RED,
            );
        }
    }
    d.draw_text(
        format!("{:?} circles", { world.read_resource::<EntCount>().0 }).as_str(),
        0,
        20,
        20,
        iron_oxide::Color::BLACK,
    );
}

fn gen_enity(world: &mut iron_oxide::World, rng: &mut impl iron_oxide::rand::Rng) {
    let x_size;
    let y_size;
    {
        let size = world.read_resource::<[i32; 2]>();
        x_size = size[0];
        y_size = size[1];
    }
    let radius = world.write_resource::<SettingsState>().radius;
    let position = iron_oxide::Vector2::new(
        rng.gen::<f32>() * x_size as f32,
        rng.gen::<f32>() * y_size as f32,
    );
    let mut particle_physics = iron_oxide::physics::Physics::new(radius * radius);
    let mut rand_vec = iron_oxide::Vector2::new(0f32, 0f32);
    while rand_vec.length_sqr() == 0f32 {
        rand_vec =
            iron_oxide::Vector2::new(1.0 - 2.0 * rng.gen::<f32>(), 1.0 - 2.0 * rng.gen::<f32>());
    }
    rand_vec.normalize();
    rand_vec.scale(INITIAL_VELOCITY);
    particle_physics.velocity = rand_vec;
    {
        world.write_resource::<EntCount>().0 += 1;
    }

    let mut layers = [false; iron_oxide::collider::LAYERS];
    layers[0] = true;
    let mut mask = [false; iron_oxide::collider::LAYERS];
    mask[0] = true;

    let collider = iron_oxide::collider::Collider {
        shape: iron_oxide::collider::Shape::CircleCollider { radius: radius },
        physics_collider: true,
        collision_layers: layers,
        collision_mask: mask,
    };

    let e = world
        .create_entity()
        .with(iron_oxide::utils::Position(position))
        .with(particle_physics)
        .with(collider.clone())
        .with(iron_oxide::renderer::Renderer::CircleRenderer {
            radius,
            color: Color::new(0, 0, 0, 255),
        })
        .with(iron_oxide::utils::Collisions(Vec::new()))
        .build();

    iron_oxide::utils::register_ent(
        (
            &collider,
            position,
            collider.get_bounding_box(&position),
            e.id(),
        ),
        world,
    );
}
