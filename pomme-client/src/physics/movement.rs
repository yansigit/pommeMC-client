// Fall distance is tracked below for vanilla movement state. Damage remains
// server-authoritative until the client has a local damage event pipeline.

use glam::{DVec3, dvec3};

use super::aabb::Aabb;
use super::collision::{no_collision, resolve_collision};
use crate::app::input::{self, InputState};
use crate::player::{CROUCH_HEIGHT, LocalPlayer, STANDING_HEIGHT};
use crate::world::chunk::ChunkStore;

const GRAVITY: f64 = 0.08;
const JUMP_VELOCITY: f64 = 0.42;
const VERTICAL_DRAG: f32 = 0.98;
const HORIZONTAL_DRAG: f64 = 0.91;
const BLOCK_FRICTION: f64 = 0.6;
const GROUND_ACCEL_FACTOR: f64 = 0.216;
const MOVEMENT_SPEED: f64 = 0.1;
const SPRINT_SPEED_MODIFIER: f64 = 0.3;
const INPUT_DAMPING: f64 = 0.98;
const AIR_ACCELERATION: f64 = 0.02;
const WATER_ACCELERATION: f64 = 0.02;
const WATER_HORIZONTAL_DRAG: f64 = 0.8;
const WATER_HORIZONTAL_DRAG_SPRINT: f64 = 0.9;
const WATER_EFFICIENT_DRAG: f64 = 0.546_000_06_f32 as f64;
const WATER_VERTICAL_DRAG: f64 = 0.8;
const WATER_GRAVITY: f64 = 0.02;
const STEP_HEIGHT: f64 = 0.6;
pub const PLAYER_HALF_WIDTH: f64 = 0.3;
pub const PLAYER_HEIGHT: f64 = 1.8;
const SPRINT_JUMP_BOOST: f64 = 0.2;
const FLYING_VERTICAL_FRICTION: f64 = 0.6;
// Vanilla Player.getFlyingSpeed: sprinting while airborne (not flying).
const SPRINT_AIR_ACCELERATION: f64 = 0.025_999_999_f32 as f64;
const SPRINT_HUNGER_THRESHOLD: u32 = 6;
const JUMP_DELAY_TICKS: u32 = 10;
// Vanilla `Entity.getFluidJumpThreshold`; always 0.4 for the player.
const FLUID_JUMP_THRESHOLD: f64 = 0.4;
// Vanilla `LivingEntity.jumpInLiquid` adds 0.04f.
const LIQUID_JUMP_ACCELERATION: f64 = 0.04_f32 as f64;
const DEFAULT_SPRINT_WINDOW: u32 = 7;
const FLY_TOGGLE_WINDOW: u32 = 7;
const MINOR_COLLISION_ANGLE: f64 = 0.13962634;

pub fn tick(
    player: &mut LocalPlayer,
    input: &InputState,
    chunk_store: &ChunkStore,
    use_speed_multiplier: f64,
    slow_due_to_using_item: bool,
) {
    let start_y = player.position.y;
    let jump_held = input.performing_action(input::Action::Jump);

    // Vanilla `LivingEntity.aiStep`.
    if player.no_jump_delay > 0 {
        player.no_jump_delay -= 1;
    }

    player.update_water_state(chunk_store);
    update_crouch_state(player, input, chunk_store);
    player.tick_eye_height();

    // Vanilla `LocalPlayer.modifyInput`: an in-use item scales the movement
    // input by its `UseEffects` speed multiplier (0.2 for food).
    let (forward, strafe) = movement_input(input, player.crouching, player.sneaking_speed);
    let (forward, strafe) = (
        forward * use_speed_multiplier,
        strafe * use_speed_multiplier,
    );
    let forward_pressed = input.performing_action(input::Action::MoveForward);

    update_sprint_state(
        player,
        input,
        forward,
        forward_pressed,
        slow_due_to_using_item,
    );

    let (sin_y_rot, cos_y_rot) = look_sin_cos(player);

    update_fly_state(player, input, chunk_store, sin_y_rot, cos_y_rot);

    if crate::player::is_spectator(player.game_mode) {
        tick_spectator(player, input);
        update_fall_distance(player, player.position.y - start_y);
        player.tick_air_supply();
        player.was_forward_pressed = forward_pressed;
        player.was_jump_pressed = jump_held;
        return;
    }

    if player.flying {
        let mut input_ya = 0.0f32;
        if input.performing_action(input::Action::Sneak) {
            input_ya -= 1.0;
        }
        if jump_held {
            input_ya += 1.0;
        }
        if input_ya != 0.0 {
            // Vanilla does this math in f32 before widening.
            player.velocity.y += f64::from(input_ya * player.fly_speed * 3.0);
        }
    }

    // Vanilla `LivingEntity.aiStep`: swim upward when submerged past the jump
    // threshold, otherwise a full jump off the ground or the shallow-fluid floor.
    if jump_held {
        let in_water = player.in_water && player.fluid_height > 0.0;
        if in_water && (!player.on_ground || player.fluid_height > FLUID_JUMP_THRESHOLD) {
            player.velocity.y += LIQUID_JUMP_ACCELERATION;
        } else if (player.on_ground || (in_water && player.fluid_height <= FLUID_JUMP_THRESHOLD))
            && player.no_jump_delay == 0
        {
            jump_from_ground(player, chunk_store, sin_y_rot, cos_y_rot);
            player.no_jump_delay = JUMP_DELAY_TICKS;
        }
    } else {
        player.no_jump_delay = 0;
    }

    if player.in_water {
        tick_water(
            player,
            input,
            chunk_store,
            forward,
            strafe,
            PLAYER_HALF_WIDTH,
            player.height(),
        );
    } else {
        tick_land(
            player,
            input,
            chunk_store,
            forward,
            strafe,
            PLAYER_HALF_WIDTH,
            player.height(),
        );
    }

    update_fall_distance(player, player.position.y - start_y);

    player.tick_air_supply();

    // Touching down cancels flight, even in creative.
    if player.on_ground && player.flying && !crate::player::is_spectator(player.game_mode) {
        player.flying = false;
        player.abilities_dirty = true;
    }

    player.was_forward_pressed = forward_pressed;
    player.was_jump_pressed = jump_held;
}

/// Vanilla dead-player `LivingEntity.aiStep`: input is immobile, but travel
/// still applies existing velocity, gravity, collision, and drag until tick-20
/// removal.
pub fn tick_dead(player: &mut LocalPlayer, chunk_store: &ChunkStore) {
    player.no_jump_delay = 0;
    player.sprinting = false;

    // Local players enter death through SetHealth; entity event 3 intentionally
    // skips LivingEntity.die for players, so the current ordinary player pose
    // remains authoritative until Player.updatePlayerPose runs at tick end.
    let half_width = PLAYER_HALF_WIDTH;
    let height = player.height();
    let eye_height = player.target_eye_height();

    let start_y = player.position.y;
    let neutral = InputState::released();
    player.update_water_state_for_dimensions(chunk_store, half_width, height, eye_height);
    player.tick_eye_height();

    if player.in_water {
        tick_water(player, &neutral, chunk_store, 0.0, 0.0, half_width, height);
    } else {
        tick_land(player, &neutral, chunk_store, 0.0, 0.0, half_width, height);
    }

    update_fall_distance(player, player.position.y - start_y);

    // Player.updatePlayerPose runs after LivingEntity.tick in vanilla. With
    // death-screen input released, this becomes standing unless clearance keeps
    // the player in the crouching pose for the following tick.
    update_crouch_state(player, &neutral, chunk_store);

    if player.on_ground && player.flying && !crate::player::is_spectator(player.game_mode) {
        player.flying = false;
        player.abilities_dirty = true;
    }
    player.was_forward_pressed = false;
    player.was_jump_pressed = false;
}

/// Tracks the vanilla `LivingEntity.fallDistance` input without applying local
/// damage. The server remains authoritative for damage and landing effects.
fn update_fall_distance(player: &mut LocalPlayer, vertical_delta: f64) {
    if player.flying || player.in_water {
        player.fall_distance = 0.0;
    } else if player.on_ground {
        // Include the final clipped downward movement in the completed fall,
        // then clear the accumulator for the next fall.
        player.fall_distance = 0.0;
    } else if vertical_delta < 0.0 {
        player.fall_distance += -vertical_delta;
    }
}

fn look_sin_cos(player: &LocalPlayer) -> (f64, f64) {
    (player.look_dir.y_rot_rad() as f64).sin_cos()
}

// Vanilla `LocalPlayer.aiStep`: a fresh jump press arms the toggle window;
// a second one inside it toggles flight.
fn update_fly_state(
    player: &mut LocalPlayer,
    input: &InputState,
    chunk_store: &ChunkStore,
    sin_y_rot: f64,
    cos_y_rot: f64,
) {
    if player.may_fly {
        if crate::player::is_spectator(player.game_mode) {
            // Spectator flight is forced on. `tick` routes spectator travel
            // through `tick_spectator`, which moves directly without collision.
            if !player.flying {
                player.flying = true;
                player.abilities_dirty = true;
            }
        } else if !player.was_jump_pressed && input.performing_action(input::Action::Jump) {
            if player.jump_trigger_time == 0 {
                player.jump_trigger_time = FLY_TOGGLE_WINDOW;
            } else if !player.swimming {
                player.flying = !player.flying;
                if player.flying && player.on_ground {
                    jump_from_ground(player, chunk_store, sin_y_rot, cos_y_rot);
                }
                player.abilities_dirty = true;
                player.jump_trigger_time = 0;
            }
        }
    }
    // Vanilla decrements after the toggle check (unlike sprint_toggle_timer).
    if player.jump_trigger_time > 0 {
        player.jump_trigger_time -= 1;
    }
}

fn jump_from_ground(
    player: &mut LocalPlayer,
    chunk_store: &ChunkStore,
    sin_y_rot: f64,
    cos_y_rot: f64,
) {
    let jump_velocity = JUMP_VELOCITY * block_jump_factor(player, chunk_store);
    player.velocity.y = jump_velocity.max(player.velocity.y);

    if player.sprinting {
        player.velocity.x -= sin_y_rot * SPRINT_JUMP_BOOST;
        player.velocity.z += cos_y_rot * SPRINT_JUMP_BOOST;
    }
}

fn tick_land(
    player: &mut LocalPlayer,
    input: &InputState,
    chunk_store: &ChunkStore,
    forward: f64,
    strafe: f64,
    half_width: f64,
    height: f64,
) {
    // Vanilla `travelInAir` samples on-ground once before the move and reuses
    // it for the end-of-tick drag, so a jump launches with ground friction.
    let on_ground_at_start = player.on_ground;

    let saved_vy = player.velocity.y;

    let speed = if player.sprinting {
        MOVEMENT_SPEED * (1.0 + SPRINT_SPEED_MODIFIER)
    } else {
        MOVEMENT_SPEED
    };

    let block_friction = ground_slipperiness(player, chunk_store);
    let accel = friction_influenced_speed(speed, player, block_friction);
    let (sin_y_rot, cos_y_rot) = look_sin_cos(player);
    let (move_x, move_z) = world_movement(forward, strafe, sin_y_rot, cos_y_rot);
    player.velocity.x += move_x * accel;
    player.velocity.z += move_z * accel;

    apply_collision(
        player,
        input,
        chunk_store,
        forward,
        strafe,
        half_width,
        height,
    );

    player.velocity.y -= GRAVITY;
    player.velocity.y *= VERTICAL_DRAG as f64;

    let h_friction = if on_ground_at_start {
        block_friction * HORIZONTAL_DRAG
    } else {
        HORIZONTAL_DRAG
    };
    player.velocity.x *= h_friction;
    player.velocity.z *= h_friction;

    overwrite_flying_vy(player, saved_vy);
}

/// Vanilla samples `getBlockPosBelowThatAffectsMyMovement`, half a block below
/// the player's feet.
fn ground_slipperiness(player: &LocalPlayer, chunk_store: &ChunkStore) -> f64 {
    crate::world::block::slipperiness(movement_effect_block_state(player, chunk_store))
}

/// Vanilla's `getBlockPosBelowThatAffectsMyMovement` samples half a block below
/// the feet, not merely an epsilon below them. This keeps slabs, carpets, and
/// blocks occupied by the player's feet on the same lookup path as vanilla.
fn movement_effect_block_state(
    player: &LocalPlayer,
    chunk_store: &ChunkStore,
) -> azalea_block::BlockState {
    chunk_store.get_block_state(
        player.position.x.floor() as i32,
        (player.position.y - 0.500_001).floor() as i32,
        player.position.z.floor() as i32,
    )
}

fn current_block_state(player: &LocalPlayer, chunk_store: &ChunkStore) -> azalea_block::BlockState {
    chunk_store.get_block_state(
        player.position.x.floor() as i32,
        player.position.y.floor() as i32,
        player.position.z.floor() as i32,
    )
}

fn block_jump_factor(player: &LocalPlayer, chunk_store: &ChunkStore) -> f64 {
    let current = crate::world::block::block_physics(current_block_state(player, chunk_store));
    let factor = if current.jump_factor != 1.0 {
        current.jump_factor
    } else {
        crate::world::block::block_physics(movement_effect_block_state(player, chunk_store))
            .jump_factor
    };
    f64::from(factor)
}

fn block_speed_factor(player: &LocalPlayer, chunk_store: &ChunkStore) -> f64 {
    let current_state = current_block_state(player, chunk_store);
    let current = crate::world::block::block_physics(current_state);
    let current_id = crate::world::block::block_id(current_state);
    let factor = if current.speed_factor != 1.0 || matches!(current_id, "water" | "bubble_column") {
        current.speed_factor
    } else {
        crate::world::block::block_physics(movement_effect_block_state(player, chunk_store))
            .speed_factor
    };
    f64::from(factor)
}

/// Vanilla spectator travel is a camera move, not `Entity.move`: it ignores
/// block collision, gravity, crouch edge-clamping, and fluid drag. Forward
/// follows the full look vector while strafe stays level, matching
/// `SpectatorInput`'s camera-relative movement.
fn tick_spectator(player: &mut LocalPlayer, input: &InputState) {
    let (forward, strafe) = movement_input(input, false, 1.0);
    let vertical = if input.performing_action(input::Action::Jump) {
        1.0
    } else if input.performing_action(input::Action::Sneak) {
        -1.0
    } else {
        0.0
    };
    spectator_move(player, forward, strafe, vertical);
}

fn spectator_move(player: &mut LocalPlayer, forward: f64, strafe: f64, vertical: f64) {
    let look = player.look_dir.as_vec();
    let yaw = player.look_dir.y_rot_rad() as f64;
    let (sin_yaw, cos_yaw) = yaw.sin_cos();
    let right = glam::dvec3(-cos_yaw, 0.0, -sin_yaw);
    let mut movement = glam::dvec3(
        f64::from(look.x) * forward + right.x * strafe,
        f64::from(look.y) * forward + vertical,
        f64::from(look.z) * forward + right.z * strafe,
    );
    if movement.length_squared() > 1.0 {
        movement = movement.normalize();
    }
    let speed = f64::from(player.fly_speed) * if player.sprinting { 2.5 } else { 1.0 };
    player.position += movement * speed;
    player.velocity = movement.into();
    player.on_ground = false;
    player.horizontal_collision = false;
}

fn tick_water(
    player: &mut LocalPlayer,
    input: &InputState,
    chunk_store: &ChunkStore,
    forward: f64,
    strafe: f64,
    half_width: f64,
    height: f64,
) {
    if input.performing_action(input::Action::Sneak) {
        player.velocity.y -= 0.04;
    }

    let (acceleration, h_drag) = water_motion_parameters(player);

    let (sin_y_rot, cos_y_rot) = look_sin_cos(player);
    let (move_x, move_z) = world_movement(forward, strafe, sin_y_rot, cos_y_rot);
    player.velocity.x += move_x * acceleration;
    player.velocity.z += move_z * acceleration;

    if player.swimming {
        let sin_x = player.look_dir.x_rot_rad().sin() as f64;
        let target_vy = -sin_x;
        let boost = if target_vy < -0.2 { 0.085 } else { 0.06 };
        player.velocity.y += (target_vy - player.velocity.y) * boost;
    }

    let saved_vy = player.velocity.y;

    apply_collision(
        player,
        input,
        chunk_store,
        forward,
        strafe,
        half_width,
        height,
    );

    player.velocity.x *= h_drag;
    player.velocity.z *= h_drag;

    let gravity = if player.velocity.y <= 0.0 && !player.swimming {
        GRAVITY * 0.25
    } else {
        WATER_GRAVITY
    };
    player.velocity.y -= gravity;
    player.velocity.y *= WATER_VERTICAL_DRAG;

    overwrite_flying_vy(player, saved_vy);
}

fn water_motion_parameters(player: &LocalPlayer) -> (f64, f64) {
    let mut efficiency = player.water_movement_efficiency;
    if !player.on_ground {
        efficiency *= 0.5;
    }
    let acceleration =
        WATER_ACCELERATION + (player.movement_speed - WATER_ACCELERATION) * efficiency;
    let base_drag = if player.sprinting {
        WATER_HORIZONTAL_DRAG_SPRINT
    } else {
        WATER_HORIZONTAL_DRAG
    };
    let drag = base_drag + (WATER_EFFICIENT_DRAG - base_drag) * efficiency;
    (acceleration, drag)
}

// Vanilla Player.travel: while flying the travel step runs normally (gravity
// and water physics included) but its vertical result is discarded, replaced
// with the pre-travel vy decayed by 0.6.
fn overwrite_flying_vy(player: &mut LocalPlayer, saved_vy: f64) {
    if player.flying {
        player.velocity.y = saved_vy * FLYING_VERTICAL_FRICTION;
    }
}

fn apply_collision(
    player: &mut LocalPlayer,
    input: &InputState,
    chunk_store: &ChunkStore,
    forward: f64,
    strafe: f64,
    half_width: f64,
    height: f64,
) {
    let current_velocity = *player.velocity;
    let aabb = Aabb::from_center(player.position.into(), half_width, height / 2.0);
    let delta = back_off_from_edge(
        chunk_store,
        &aabb,
        *player.velocity,
        input.performing_action(input::Action::Sneak),
        player.on_ground,
        player.fall_distance,
        player.flying,
    );
    let step_height = if player.on_ground { STEP_HEIGHT } else { 0.0 };
    let (resolved, on_ground) = resolve_collision(chunk_store, aabb, delta.into(), step_height);

    // Collisions compare against the edge-clamped delta so the clamp itself
    // never zeroes velocity, letting the player keep creeping along the edge.
    let collided_x = (resolved.x - delta.x).abs() > 1.0e-5;
    let collided_y = (resolved.y - delta.y).abs() > 1.0e-5;
    let collided_z = (resolved.z - delta.z).abs() > 1.0e-5;
    let horizontal_collision = collided_x || collided_z;

    player.position += resolved;
    player.on_ground = on_ground;
    player.horizontal_collision = horizontal_collision;

    if collided_x {
        player.velocity.x = 0.0;
    }
    if collided_z {
        player.velocity.z = 0.0;
    }
    // Vanilla computes restitution in Entity.move before LivingEntity applies
    // gravity and drag. Players have zero entity restitution, so the generated
    // block value is authoritative for downward impacts.
    if collided_y {
        let effect_state = movement_effect_block_state(player, chunk_store);
        let block_bounce =
            f64::from(crate::world::block::block_physics(effect_state).bounce_restitution);
        let suppresses_bounce = input.performing_action(input::Action::Sneak)
            || crate::world::block::block_id(effect_state) == "honey_block";
        if delta.y < 0.0
            && -current_velocity.y >= GRAVITY
            && !suppresses_bounce
            && block_bounce > 0.0
        {
            let portion_with_movement = resolved.y / current_velocity.y;
            let gravity_compensation = portion_with_movement * GRAVITY;
            let effective_drag = 1.0 + portion_with_movement * (f64::from(VERTICAL_DRAG) - 1.0);
            player.velocity.y =
                (gravity_compensation - current_velocity.y) * effective_drag * block_bounce;
        } else {
            // Ground/ceiling contact without restitution zeroes vertical speed;
            // the caller reapplies gravity so grounded ticks continue probing.
            player.velocity.y = 0.0;
        }
    }

    let speed_factor = block_speed_factor(player, chunk_store);
    player.velocity.x *= speed_factor;
    player.velocity.z *= speed_factor;

    let (sin_y_rot, cos_y_rot) = look_sin_cos(player);
    if player.sprinting
        && horizontal_collision
        && forward > 0.0
        && !is_minor_horizontal_collision(forward, strafe, sin_y_rot, cos_y_rot, resolved)
    {
        player.sprinting = false;
    }
}

fn update_sprint_state(
    player: &mut LocalPlayer,
    input: &InputState,
    forward: f64,
    forward_pressed: bool,
    slow_due_to_using_item: bool,
) {
    if player.sprint_toggle_timer > 0 {
        player.sprint_toggle_timer -= 1;
    }
    if input.performing_action(input::Action::Sneak) || slow_due_to_using_item {
        player.sprint_toggle_timer = 0;
    }

    // Crouching blocks starting a sprint but doesn't stop one in progress.
    // Vanilla `canStartSprinting` also denies it while slowed by an item use,
    // and the slowed input impulse (< 0.8) stops a sprint in progress.
    let can_sprint = forward > 0.0
        && player.food > SPRINT_HUNGER_THRESHOLD
        && !player.crouching
        && !slow_due_to_using_item;

    if input.performing_action(input::Action::Sprint) && can_sprint {
        player.sprinting = true;
    }

    if !player.was_forward_pressed && forward_pressed && can_sprint {
        if player.sprint_toggle_timer > 0 {
            player.sprinting = true;
        }
        player.sprint_toggle_timer = DEFAULT_SPRINT_WINDOW;
    }

    if player.sprinting
        && (forward <= 0.0 || player.food <= SPRINT_HUNGER_THRESHOLD || slow_due_to_using_item)
    {
        player.sprinting = false;
    }
}

// Forces the crouch pose under ceilings too low to stand in; riding and
// sleeping aren't simulated.
fn update_crouch_state(player: &mut LocalPlayer, input: &InputState, chunk_store: &ChunkStore) {
    player.crouching = !crate::player::is_spectator(player.game_mode)
        && !player.flying
        && !player.swimming
        && can_fit_with_height(chunk_store, player.position.into(), CROUCH_HEIGHT)
        && (input.performing_action(input::Action::Sneak)
            || !can_fit_with_height(chunk_store, player.position.into(), STANDING_HEIGHT));
}

fn can_fit_with_height(chunk_store: &ChunkStore, pos: DVec3, height: f64) -> bool {
    no_collision(
        chunk_store,
        &Aabb::from_center(pos, PLAYER_HALF_WIDTH, height / 2.0).deflate(1.0e-7),
    )
}

// While holding shift on the ground, clamp the horizontal move so the player
// can't fall further than the step height.
fn back_off_from_edge(
    chunk_store: &ChunkStore,
    bb: &Aabb,
    delta: DVec3,
    shift_down: bool,
    on_ground: bool,
    fall_distance: f64,
    flying: bool,
) -> DVec3 {
    if !shift_down || flying || delta.y > 0.0 {
        return delta;
    }
    if !is_above_ground(chunk_store, bb, on_ground, fall_distance, STEP_HEIGHT) {
        return delta;
    }

    let mut dx = delta.x;
    let mut dz = delta.z;
    let step_x = dx.signum() * 0.05;
    let step_z = dz.signum() * 0.05;

    while dx != 0.0 && can_fall_at_least(chunk_store, bb, dx, 0.0, STEP_HEIGHT) {
        if dx.abs() <= 0.05 {
            dx = 0.0;
            break;
        }
        dx -= step_x;
    }
    while dz != 0.0 && can_fall_at_least(chunk_store, bb, 0.0, dz, STEP_HEIGHT) {
        if dz.abs() <= 0.05 {
            dz = 0.0;
            break;
        }
        dz -= step_z;
    }
    while dx != 0.0 && dz != 0.0 && can_fall_at_least(chunk_store, bb, dx, dz, STEP_HEIGHT) {
        dx = if dx.abs() <= 0.05 { 0.0 } else { dx - step_x };
        if dz.abs() <= 0.05 {
            dz = 0.0;
            continue;
        }
        dz -= step_z;
    }

    dvec3(dx, delta.y, dz)
}

/// Vanilla `LocalPlayer.isAboveGround`: crouch edge protection only applies
/// while grounded, before the accumulated fall reaches the step height, and
/// while a supporting shape remains within the unused portion of that height.
fn is_above_ground(
    chunk_store: &ChunkStore,
    bb: &Aabb,
    on_ground: bool,
    fall_distance: f64,
    step_height: f64,
) -> bool {
    on_ground
        && fall_distance < step_height
        && !can_fall_at_least(chunk_store, bb, 0.0, 0.0, step_height - fall_distance)
}

fn can_fall_at_least(
    chunk_store: &ChunkStore,
    bb: &Aabb,
    dx: f64,
    dz: f64,
    min_height: f64,
) -> bool {
    no_collision(
        chunk_store,
        &Aabb::new(
            dvec3(
                bb.min.x + 1.0e-7 + dx,
                bb.min.y - min_height - 1.0e-7,
                bb.min.z + 1.0e-7 + dz,
            ),
            dvec3(bb.max.x - 1.0e-7 + dx, bb.min.y, bb.max.z - 1.0e-7 + dz),
        ),
    )
}

fn world_movement(forward: f64, strafe: f64, sin_y_rot: f64, cos_y_rot: f64) -> (f64, f64) {
    (
        forward * -sin_y_rot + strafe * -cos_y_rot,
        forward * cos_y_rot + strafe * -sin_y_rot,
    )
}

fn friction_influenced_speed(speed: f64, player: &LocalPlayer, block_friction: f64) -> f64 {
    if player.on_ground {
        if block_friction > BLOCK_FRICTION {
            speed * (GROUND_ACCEL_FACTOR / block_friction.powi(3))
        } else {
            speed
        }
    } else if player.flying {
        // Vanilla Player.getFlyingSpeed.
        let fly_speed = f64::from(player.fly_speed);
        if player.sprinting {
            fly_speed * 2.0
        } else {
            fly_speed
        }
    } else if player.sprinting {
        SPRINT_AIR_ACCELERATION
    } else {
        AIR_ACCELERATION
    }
}

fn is_minor_horizontal_collision(
    forward: f64,
    strafe: f64,
    sin_y_rot: f64,
    cos_y_rot: f64,
    resolved: DVec3,
) -> bool {
    let (intent_x, intent_z) = world_movement(forward, strafe, sin_y_rot, cos_y_rot);
    let intent_len_sq = intent_x * intent_x + intent_z * intent_z;
    let resolved_len_sq = resolved.x.powi(2) + resolved.z.powi(2);
    if intent_len_sq < 1.0e-5 || resolved_len_sq < 1.0e-5 {
        return false;
    }
    let dot = intent_x * resolved.x + intent_z * resolved.z;
    let angle = (dot / (intent_len_sq * resolved_len_sq).sqrt()).acos();
    angle < MINOR_COLLISION_ANGLE
}

fn movement_input(input: &InputState, crouching: bool, sneaking_speed: f64) -> (f64, f64) {
    let mut forward = 0.0f64;
    let mut strafe = 0.0f64;

    if let Some(analog_input) = input.get_gamepad_left_analog() {
        forward = analog_input.y as f64;
        strafe = analog_input.x as f64;
    } else {
        if input.performing_action(input::Action::MoveForward) {
            forward += 1.0;
        }
        if input.performing_action(input::Action::MoveBackward) {
            forward -= 1.0;
        }
        if input.performing_action(input::Action::MoveLeft) {
            strafe -= 1.0;
        }
        if input.performing_action(input::Action::MoveRight) {
            strafe += 1.0;
        }
    }

    forward *= INPUT_DAMPING;
    strafe *= INPUT_DAMPING;

    if crouching {
        forward *= sneaking_speed;
        strafe *= sneaking_speed;
    }

    square_movement(forward, strafe)
}

// Assumes cardinal keyboard input (-1/0/+1 axes); analog input would need a
// normalize first.
fn square_movement(forward: f64, strafe: f64) -> (f64, f64) {
    let len = (forward * forward + strafe * strafe).sqrt();
    if len < 1.0e-7 {
        return (0.0, 0.0);
    }
    let max_axis = forward.abs().max(strafe.abs());
    let modified = (len * len / max_axis).min(1.0);
    let scale = modified / len;
    (forward * scale, strafe * scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunks_with_block(name: &str, y: i32) -> ChunkStore {
        use azalea_core::position::ChunkPos;
        use azalea_world::Chunk;

        crate::world::block::init("26.2");
        let mut chunks = ChunkStore::new(2);
        let chunk_pos = ChunkPos::new(0, 0);
        chunks.partial_storage.set(
            &chunk_pos,
            Some(Chunk::default()),
            &mut chunks.chunk_storage,
        );
        let state = crate::world::block::first_state_of(name).expect("test block state");
        chunks.set_block_state(0, y, 0, state);
        chunks
    }

    #[test]
    fn dead_player_keeps_zero_input_air_travel() {
        crate::world::block::init("26.2");
        let mut player = LocalPlayer::new();
        player.position = dvec3(0.0, 80.0, 0.0).into();
        player.velocity = crate::entity::components::Velocity::new(0.25, 0.0, -0.1);
        player.sprinting = true;
        player.crouching = true;
        player.eye_height = 1.27;
        player.prev_eye_height = 1.27;
        let chunks = ChunkStore::new(2);

        player.death_time = 1;
        let starting_height = player.height();
        tick_dead(&mut player, &chunks);

        assert_eq!(
            starting_height, CROUCH_HEIGHT,
            "death must begin from the player's existing ordinary pose"
        );

        assert!(
            !player.crouching,
            "the first dead tick must end by selecting the neutral-input pose"
        );
        assert_eq!(
            player.eye_height, 1.27,
            "the first dead tick must still use the pre-tick crouching eye height"
        );
        player.death_time = 2;
        tick_dead(&mut player, &chunks);
        assert!(
            player.eye_height > 1.27,
            "the next dead tick must smooth toward the neutral standing pose"
        );
        assert!(
            player.position.x > 0.0,
            "dead-player momentum must still move the corpse"
        );
        assert!(
            player.position.z < 0.0,
            "dead-player momentum must still move the corpse"
        );
        assert!(
            player.position.y <= 80.0,
            "dead-player travel must continue applying gravity"
        );
        assert!(
            player.velocity.y < 0.0,
            "dead-player travel must retain downward gravity/drag"
        );
        assert!(
            !player.sprinting,
            "immobile dead-player input must stop sprinting"
        );
    }

    #[test]
    fn fall_distance_accumulates_only_while_descending_in_air() {
        let mut player = LocalPlayer::new();
        player.on_ground = false;

        update_fall_distance(&mut player, -1.25);
        update_fall_distance(&mut player, -0.75);
        assert!((player.fall_distance - 2.0).abs() < f64::EPSILON);

        update_fall_distance(&mut player, 0.5);
        assert!((player.fall_distance - 2.0).abs() < f64::EPSILON);

        player.on_ground = true;
        update_fall_distance(&mut player, -0.01);
        assert_eq!(player.fall_distance, 0.0);
    }

    #[test]
    fn fall_distance_resets_in_water_and_flight() {
        let mut player = LocalPlayer::new();
        player.on_ground = false;
        update_fall_distance(&mut player, -3.0);
        assert_eq!(player.fall_distance, 3.0);

        player.in_water = true;
        update_fall_distance(&mut player, -1.0);
        assert_eq!(player.fall_distance, 0.0);

        player.in_water = false;
        player.flying = true;
        update_fall_distance(&mut player, -1.0);
        assert_eq!(player.fall_distance, 0.0);
    }

    #[test]
    fn ground_slipperiness_uses_the_block_under_the_feet() {
        let mut player = LocalPlayer::new();
        player.position = dvec3(0.5, 1.5, 0.5).into();
        let chunks = chunks_with_block("ice", 0);
        let stone = crate::world::block::first_state_of("stone").expect("stone state");
        chunks.set_block_state(0, 1, 0, stone);

        assert_eq!(ground_slipperiness(&player, &chunks), f64::from(0.98_f32));
    }

    #[test]
    fn honey_block_halves_ground_jump_velocity() {
        let chunks = chunks_with_block("honey_block", 0);
        let mut player = LocalPlayer::new();
        player.position = dvec3(0.5, 1.0, 0.5).into();

        jump_from_ground(&mut player, &chunks, 0.0, 1.0);

        assert!((player.velocity.y - 0.21).abs() < 1.0e-12);
    }

    #[test]
    fn block_speed_factor_scales_horizontal_velocity_after_movement() {
        let chunks = chunks_with_block("soul_sand", 0);
        let mut player = LocalPlayer::new();
        player.position = dvec3(0.5, 1.0, 0.5).into();
        player.velocity = crate::entity::components::Velocity::new(0.1, 0.0, 0.0);

        apply_collision(
            &mut player,
            &InputState::released(),
            &chunks,
            0.0,
            0.0,
            PLAYER_HALF_WIDTH,
            PLAYER_HEIGHT,
        );

        assert!((player.velocity.x - f64::from(0.4_f32) * 0.1).abs() < 1.0e-12);
    }

    #[test]
    fn slime_block_bounces_unless_sneaking() {
        let chunks = chunks_with_block("slime_block", 0);
        let mut player = LocalPlayer::new();
        player.position = dvec3(0.5, 1.2, 0.5).into();
        player.velocity = crate::entity::components::Velocity::new(0.0, -0.5, 0.0);

        apply_collision(
            &mut player,
            &InputState::released(),
            &chunks,
            0.0,
            0.0,
            PLAYER_HALF_WIDTH,
            PLAYER_HEIGHT,
        );
        assert!(player.on_ground);
        assert!(player.velocity.y > 0.5);

        player.position = dvec3(0.5, 1.2, 0.5).into();
        player.velocity = crate::entity::components::Velocity::new(0.0, -0.5, 0.0);
        let mut sneaking = InputState::released();
        sneaking.inject_action_press(input::Action::Sneak);
        apply_collision(
            &mut player,
            &sneaking,
            &chunks,
            0.0,
            0.0,
            PLAYER_HALF_WIDTH,
            PLAYER_HEIGHT,
        );
        assert_eq!(player.velocity.y, 0.0);
    }

    #[test]
    fn spectator_travel_ignores_collision_and_moves_along_look_vector() {
        use azalea_core::position::ChunkPos;
        use azalea_world::Chunk;

        crate::world::block::init("26.2");
        let mut player = LocalPlayer::new();
        player.game_mode = crate::player::GAME_MODE_SPECTATOR;
        player.apply_game_mode_abilities(crate::player::GAME_MODE_SPECTATOR);
        player.fly_speed = 1.0;
        player.position = dvec3(0.5, 0.0, 0.5).into();

        let mut chunks = ChunkStore::new(2);
        let chunk_pos = ChunkPos::new(0, 0);
        chunks.partial_storage.set(
            &chunk_pos,
            Some(Chunk::default()),
            &mut chunks.chunk_storage,
        );
        let stone = crate::world::block::first_state_of("stone").expect("stone state");
        chunks.set_block_state(0, 2, 0, stone);

        let mut input = InputState::released();
        input.inject_action_press(input::Action::Jump);
        tick(&mut player, &input, &chunks, 1.0, false);

        assert_eq!(player.position.y, 1.0);
        assert!(player.flying);
        assert!(!player.on_ground);
        assert!(!player.horizontal_collision);
    }

    #[test]
    fn edge_backoff_requires_ground_within_remaining_step_height() {
        use azalea_core::position::ChunkPos;
        use azalea_world::Chunk;

        crate::world::block::init("26.2");
        let mut chunks = ChunkStore::new(2);
        let chunk_pos = ChunkPos::new(0, 0);
        chunks.partial_storage.set(
            &chunk_pos,
            Some(Chunk::default()),
            &mut chunks.chunk_storage,
        );
        let stone = crate::world::block::first_state_of("stone").expect("stone state");
        chunks.set_block_state(0, -1, 0, stone);

        let bb = Aabb::from_center(dvec3(0.5, 0.0, 0.5), PLAYER_HALF_WIDTH, PLAYER_HEIGHT / 2.0);
        assert!(is_above_ground(&chunks, &bb, true, 0.0, STEP_HEIGHT));
        assert!(is_above_ground(&chunks, &bb, true, 0.25, STEP_HEIGHT));
        assert!(!is_above_ground(
            &chunks,
            &bb,
            true,
            STEP_HEIGHT,
            STEP_HEIGHT
        ));
        assert!(!is_above_ground(&chunks, &bb, false, 0.0, STEP_HEIGHT));
    }

    #[test]
    fn water_efficiency_interpolates_acceleration_and_drag() {
        let mut player = LocalPlayer::new();
        player.movement_speed = 0.1;
        player.water_movement_efficiency = 1.0;
        player.on_ground = true;

        let (ground_accel, ground_drag) = water_motion_parameters(&player);
        assert!((ground_accel - 0.1).abs() < f64::EPSILON);
        assert!((ground_drag - WATER_EFFICIENT_DRAG).abs() < f64::EPSILON);

        player.on_ground = false;
        let (air_accel, air_drag) = water_motion_parameters(&player);
        assert!((air_accel - 0.06).abs() < 1.0e-12);
        assert!((air_drag - (WATER_HORIZONTAL_DRAG + WATER_EFFICIENT_DRAG) * 0.5).abs() < 1.0e-12);
    }
}
