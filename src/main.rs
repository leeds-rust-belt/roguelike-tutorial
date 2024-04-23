use std::cmp;
use tcod::colors;
use tcod::colors::*;
use tcod::console::*;
use rand::Rng;
use tcod::map::{FovAlgorithm, Map as FovMap};

const SCREEN_WIDTH: i32 = 80;
const SCREEN_HEIGHT: i32 = 50;

const MAP_WIDTH: i32 = 80;
const MAP_HEIGHT: i32 = 45;
const ROOM_MAX_SIZE: i32 = 10;
const ROOM_MIN_SIZE: i32 = 6;
const MAX_ROOMS: i32 = 30;
const MAX_ROOM_MONSTERS: i32 = 3;

const FOV_ALGO: FovAlgorithm = FovAlgorithm::Basic;
const FOV_LIGHT_WALLS: bool = true;
const TORCH_RADIUS: i32 = 10;

const PLAYER: usize = 0;

#[derive(Debug, Clone, Copy, PartialEq)]
enum PlayerAction {
    TookTurn,
    DidntTakeTurn,
    Exit,
}

#[derive(Clone, Debug, PartialEq)]
enum AI {
    Basic,
}

// colour defs
const DARK_WALL: Color = Color { r: 0, g: 0, b: 100 };
const LIGHT_WALL: Color = Color { r: 130, g: 110, b: 50};
const DARK_GROUND: Color = Color { r: 50, g: 50, b: 150 };
const LIGHT_GROUND: Color = Color { r: 200, g: 180, b: 50 };

const LIMIT_FPS: i32 = 20;

struct Tcod {
    root: Root,
    con: Offscreen,
    fov: FovMap,
}

// type definitions
type Map = Vec<Vec<Tile>>;

// the game map
struct Game {
    map: Map
}

#[derive(Debug, Clone, Copy)]
struct Rect {
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32
}

impl Rect {
    pub fn new(x1: i32, y1: i32, w: i32, h: i32) -> Self {
        Rect {x1, y1, x2: x1 + w, y2: y1 + h }
    }

    pub fn centre(&self) -> (i32, i32) {
        let centre_x = (self.x1 + self.x2) / 2;
        let centre_y = (self.y1 + self.y2) / 2;
        (centre_x, centre_y)
    }

    pub fn intersects_with(&self, other: &Rect) -> bool {
        (self.x1 <= other.x2) && (self.x2 >= other.x1) && (self.y1 <= other.y2) && (self.y2 >= other.y1)
    }
}

// an in-game object (e.g. player, monster, et al)
#[derive(Debug)]
struct Object {
    x: i32,
    y: i32,
    chr: char,
    colour: Color,
    name: String,
    blocks: bool,
    alive: bool,
    fighter: Option<Fighter>,
    ai: Option<AI>,
}

impl Object {
    // Create a new in-game object
    pub fn new(name: &str, x: i32, y: i32, chr: char, colour: Color, blocks: bool) -> Self {
        Object { x, y, chr, colour, name: name.into(), blocks, alive: false, fighter: None, ai: None }
    }

    pub fn pos(&self) -> (i32, i32) {
        (self.x, self.y)
    }

    pub fn set_pos(&mut self, x: i32, y: i32) {
        self.x = x;
        self.y = y;
    }

    pub fn distance_to(&self, other: &Object) -> f32 {
        let dx = other.x - self.x;
        let dy = other.y - self.y;
        ((dx*dx + dy*dy) as f32).sqrt()
    }

    // draw the Object (this includes setting the colour appropriately etc)
    // Note - the `dyn` keyword dentoes that we're working on a trait rather than a concrete type
    pub fn draw(&self, con: &mut dyn Console) {
        con.set_default_foreground(self.colour);
        con.put_char(self.x, self.y, self.chr, BackgroundFlag::None);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Fighter {
    max_hp: i32,
    hp: i32,
    defence: i32,
    power: i32,
}

// tile definitions
#[derive(Debug, Clone, Copy)]
struct Tile {
    blocked: bool,
    explored: bool,
    block_sight: bool,
}

impl Tile {
    pub fn empty() -> Self {
        Tile { blocked: false, explored: false, block_sight: false }
    }

    pub fn wall() -> Self {
        Tile { blocked: true, explored: false, block_sight: true }
    }
}

fn handle_keys(tcod: &mut Tcod, game: &Game, objects: &mut Vec<Object>) -> PlayerAction {
    use tcod::input::Key;
    use tcod::input::KeyCode::*;
    use PlayerAction::*;

    let key = tcod.root.wait_for_keypress(true);
    let player_alive = objects[PLAYER].alive;
    match (key, key.text(), player_alive) {
        (Key { code: Enter, alt: true, .. }, _, _) => {
            let fullscreen = tcod.root.is_fullscreen();
            tcod.root.set_fullscreen(!fullscreen);
            DidntTakeTurn
        },
        (Key { code: Up, .. }, _, true) => {
            player_move_or_attack(0, -1, &game.map, objects);
            TookTurn
        },
        (Key { code: Down, .. }, _, true) => {
            player_move_or_attack(0, 1, &game.map, objects);
            TookTurn
        },
        (Key { code: Left, .. }, _, true) => {
            player_move_or_attack(-1, 0, &game.map, objects);
            TookTurn
        },
        (Key { code: Right, .. }, _, true) => {
            player_move_or_attack(1, 0, &game.map, objects);
            TookTurn
        },
        (Key { code: Escape, .. }, _, _) => Exit,
        _ => DidntTakeTurn
    }
}

// map creation functions
fn make_map(objects: &mut Vec<Object>) -> Map {
    let mut map = vec![vec![Tile::wall(); MAP_HEIGHT as usize]; MAP_WIDTH as usize];
    let mut rooms = vec![];

    for _ in 0..MAX_ROOMS {
        // random room size
        let w = rand::thread_rng().gen_range(ROOM_MIN_SIZE..ROOM_MAX_SIZE + 1);
        let h = rand::thread_rng().gen_range(ROOM_MIN_SIZE..ROOM_MAX_SIZE + 1);

        // random room placememnt withing our map bounds
        let x = rand::thread_rng().gen_range(0..MAP_WIDTH - w);
        let y = rand::thread_rng().gen_range(0..MAP_HEIGHT- h);

        let new_room = Rect::new(x, y, w, h);
        let failed = rooms.iter().any(|r| new_room.intersects_with(r));
        if !failed {
            // no intersections so slap the room down
            create_room(new_room, &mut map);
            place_objects(new_room, &map, objects);
            let (sx, sy) = new_room.centre();

            if rooms.is_empty() {
                // first room - let's put @ here! :)
                objects[PLAYER].set_pos(sx, sy);
            } else {
                // Connect this room to the last one
                let (prev_x, prev_y) = rooms[rooms.len() - 1].centre();

                // this could be improved by setting up a cached generator and using that - might do this later ;) 
                if rand::random() {
                    // Horizontal tunnel then vertical
                    create_h_tunnel(prev_x, sx, prev_y, &mut map);
                    create_v_tunnel(prev_y, sy, sx, &mut map);
                } else {
                    // Vertical then horizontal
                    create_v_tunnel(prev_y, sy, prev_x, &mut map);
                    create_h_tunnel(prev_x, sx, sy, &mut map);
                }
            }

            rooms.push(new_room);
        }
    }

    map
}

fn create_room(room: Rect, map: &mut Map) {
    for x in (room.x1 + 1)..room.x2 {
        for y in (room.y1 + 1)..room.y2 {
            map[x as usize][y as usize] = Tile::empty();
        }
    }
}

fn create_h_tunnel(x1: i32, x2: i32, y: i32, map: &mut Map) {
    for x in cmp::min(x1, x2)..(cmp::max(x1, x2) + 1) {
        map[x as usize][y as usize] = Tile::empty();
    }
}

fn create_v_tunnel(y1: i32, y2: i32, x: i32, map: &mut Map) {
    for y in cmp::min(y1, y2)..(cmp::max(y1, y2) + 1) {
        map[x as usize][y as usize] = Tile::empty();
    }
}

fn place_objects(room: Rect, map: &Map, objects: &mut Vec<Object>) {
    let num_monsters = rand::thread_rng().gen_range(0..MAX_ROOM_MONSTERS + 1);
    for _ in 0..num_monsters {
        let x = rand::thread_rng().gen_range(room.x1+1..room.x2);
        let y = rand::thread_rng().gen_range(room.y1+1..room.y2);

        if !is_blocked(x, y, map, objects) {
            let mut monster = if rand::random::<f32>() < 0.8 {
                let mut orc = Object::new("Orc", x, y, 'o', colors::DESATURATED_GREEN, true);
                orc.fighter = Some(Fighter { max_hp: 10, hp: 10, defence: 0, power: 3 });
                orc.ai = Some(AI::Basic);
                orc
            } else {
                let mut troll = Object::new("Troll", x, y, 'T', colors::DARKER_GREEN, true);
                troll.fighter = Some(Fighter { max_hp: 16, hp: 16, defence: 1, power: 4 });
                troll.ai = Some(AI::Basic);
                troll
            };
    
            monster.alive = true;
            objects.push(monster);
        }
    }
}

fn is_blocked(x: i32, y: i32, map: &Map, objects: &[Object]) -> bool {
    if map[x as usize][y as usize].blocked {
        return true;
    }

    objects.iter().any(|obj| obj.blocks && obj.pos() == (x, y))
}

fn ai_take_turn(id: usize, tcod: &Tcod, game: &Game, objects: &mut [Object]) {
    let (monster_x, monster_y) = objects[id].pos();
    if tcod.fov.is_in_fov(monster_x, monster_y) {
        if objects[id].distance_to(&objects[PLAYER]) >= 2.0 {
            // Let's move closer
            let (px, py) = objects[PLAYER].pos();
            move_towards(id, px, py, &game.map, objects);
        } else if objects[PLAYER].fighter.map_or(false, |m| m.hp > 0) {
            // ATTTACK!!!!!
            let monster = &objects[id];
            println!("The attack of the {} bounces off your shiny armour", monster.name);
        }
    }
}

fn move_towards(id: usize, target_x: i32, target_y: i32, map: &Map, objects: &mut [Object]) {
    // figure direction vector out
    let dx = target_x - objects[id].x;
    let dy = target_y - objects[id].y;
    let distance = ((dx*dx + dy*dy) as f32).sqrt();

    // normalise vector to unit - mmm type conversions
    let dx = (dx as f32 / distance).round() as i32;
    let dy = (dy as f32 / distance).round() as i32;
    move_by(id, dx, dy, map, objects);
}

// Move this object by the given delta
fn move_by(id: usize, dx: i32, dy: i32, map: &Map, objects: &mut [Object]) {
    let (x, y) = objects[id].pos();
    if !is_blocked(x + dx, y + dy, map, objects) {
        objects[id].set_pos(x + dx, y + dy);
    }
}

fn player_move_or_attack(dx: i32, dy: i32, map: &Map, objects: &mut [Object]) {
    let target_pos = (objects[PLAYER].x + dx, objects[PLAYER].y + dy);
    let target_id = objects.iter().position(|obj| obj.pos() == target_pos);

    match target_id {
        Some(target_id) => {
            // Attackable target
            println!("You would attack the {} but your pacifist oath prevents you", objects[target_id].name);
        },
        None => {
            // Player move
            move_by(PLAYER, dx, dy, map, objects);
        }
    }
}

// render functions
fn render_all(tcod: &mut Tcod, game: &mut Game, objects: &[Object], fov_recompute: bool) {
    if fov_recompute {
        let player = &objects[0];
        tcod.fov.compute_fov(player.x, player.y, TORCH_RADIUS, FOV_LIGHT_WALLS, FOV_ALGO);
    }

    for obj in objects {
        if tcod.fov.is_in_fov(obj.x, obj.y) {
            obj.draw(&mut tcod.con);
        }
    }

    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let visible = tcod.fov.is_in_fov(x, y);
            let wall = game.map[x as usize][y as usize].block_sight;
            let colour = match (visible, wall) {
                // outside FOV
                (false, false) => DARK_GROUND,
                (false, true) => DARK_WALL,
                // inside FOV
                (true, false) => LIGHT_GROUND,
                (true, true) => LIGHT_WALL
            };

            // Need to define here as we're borrowing game.map again, but this time as mutable. Previously borrowed as wall.
            let explored = &mut game.map[x as usize][y as usize].explored;
            if visible {
                *explored = true;
            }

            if *explored {
                tcod.con.set_char_background(x, y, colour, BackgroundFlag::Set);
            }
        }
    }

    blit(&tcod.con, (0, 0), (SCREEN_WIDTH, SCREEN_HEIGHT), &mut tcod.root, (0, 0), 1.0, 1.0);
}

fn main() {
    // Initialise and create the root window
    let root = Root::initializer()
        .font("arial10x10.png", FontLayout::Tcod)
        .font_type(FontType::Greyscale)
        .size(SCREEN_WIDTH, SCREEN_HEIGHT)
        .title("Making a window happen")
        .init();

    let con = Offscreen::new(MAP_WIDTH, MAP_HEIGHT);
    let mut tcod = Tcod{ root, con, fov: FovMap::new(MAP_WIDTH, MAP_HEIGHT) };

    // limit FPS (doesn't really matter for a key input roguelike)
    tcod::system::set_fps(LIMIT_FPS);

    // Game objects
    let mut player = Object::new("Player", 0, 0, '@', WHITE, true);
    player.alive = true;
    player.fighter = Some(Fighter {max_hp: 30, hp: 30, defence: 2, power: 5});

    let mut objects = vec![player];
    let mut game = Game { map: make_map(&mut objects) };

    // Set up FOV map
    for x in 0..MAP_WIDTH {
        for y in 0..MAP_HEIGHT {
            tcod.fov.set(x, y, !game.map[x as usize][y as usize].block_sight, !game.map[x as usize][y as usize].blocked);
        }
    }

    let mut prev_pos = (-1, -1);

    // It's a game; it needs a game loop
    while !tcod.root.window_closed() {
        let fov_recompute = prev_pos != (objects[PLAYER].x, objects[PLAYER].y);

        tcod.con.clear();
        render_all(&mut tcod, &mut game, &objects, fov_recompute);
        tcod.root.flush();

        let player = &mut objects[PLAYER];
        prev_pos = player.pos();
        let action = handle_keys(&mut tcod, &game, &mut objects);
        if action == PlayerAction::Exit { break; }

        if objects[PLAYER].alive && action != PlayerAction::DidntTakeTurn {
            for id in 0..objects.len() {
                if objects[id].ai.is_some() {
                    ai_take_turn(id, &tcod, &game, &mut objects);
                }
            }
        }
    }
}
