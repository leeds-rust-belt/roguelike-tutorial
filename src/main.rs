use std::cmp;
use tcod::colors::*;
use tcod::console::*;
use rand::Rng;

const SCREEN_WIDTH: i32 = 80;
const SCREEN_HEIGHT: i32 = 50;
const MAP_WIDTH: i32 = 80;
const MAP_HEIGHT: i32 = 45;
const ROOM_MAX_SIZE: i32 = 10;
const ROOM_MIN_SIZE: i32 = 6;
const MAX_ROOMS: i32 = 30;

// colour defs
const DARK_WALL: Color = Color { r: 0, g: 0, b: 100 };
const DARK_GROUND: Color = Color { r: 50, g: 50, b: 150 };

const LIMIT_FPS: i32 = 20;

struct Tcod {
    root: Root,
    con: Offscreen,
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
    colour: Color
}

impl Object {
    // Create a new in-game object
    pub fn new(x: i32, y: i32, chr: char, colour: Color) -> Self {
        Object { x, y, chr, colour }
    }

    // Move this object by the given delta
    pub fn move_by(&mut self, dx: i32, dy: i32, game: &Game) {
        if !game.map[(self.x + dx) as usize][(self.y + dy) as usize].blocked {
            self.x += dx;
            self.y += dy;
        }
    }

    // draw the Object (this includes setting the colour appropriately etc)
    // Note - the `dyn` keyword dentoes that we're working on a trait rather than a concrete type
    pub fn draw(&self, con: &mut dyn Console) {
        con.set_default_foreground(self.colour);
        con.put_char(self.x, self.y, self.chr, BackgroundFlag::None);
    }
}

// tile definitions
#[derive(Debug, Clone, Copy)]
struct Tile {
    blocked: bool,
    block_sight: bool,
}

impl Tile {
    pub fn empty() -> Self {
        Tile { blocked: false, block_sight: false }
    }

    pub fn wall() -> Self {
        Tile { blocked: true, block_sight: true }
    }
}

fn handle_keys(tcod: &mut Tcod, game: &Game, player: &mut Object) -> bool {
    use tcod::input::Key;
    use tcod::input::KeyCode::*;

    let key = tcod.root.wait_for_keypress(true);
    match key {
        Key { code: Enter, alt: true, .. } => {
            let fullscreen = tcod.root.is_fullscreen();
            tcod.root.set_fullscreen(!fullscreen);
        },
        Key { code: Up, .. } => player.move_by(0, -1, game),
        Key { code: Down, .. } => player.move_by(0, 1, game),
        Key { code: Left, .. } => player.move_by(-1, 0, game),
        Key { code: Right, .. } =>player.move_by(1, 0, game),
        Key { code: Escape, .. } => return true,
        _ => {}
    }
    false
}

// map creation functions
fn make_map(player: &mut Object) -> Map {
    let mut map = vec![vec![Tile::wall(); MAP_HEIGHT as usize]; MAP_WIDTH as usize];
    
    // test
    // let room1 = Rect::new(20, 15, 10, 15);
    // let room2 = Rect::new(50, 15, 10, 15);
    // create_room(room1, &mut map);
    // create_room(room2, &mut map);
    // create_h_tunnel(30, 50, 23, &mut map);

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
            let (sx, sy) = new_room.centre();

            if rooms.is_empty() {
                // first room - let's put @ here! :)
                player.x = sx;
                player.y = sy;
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

// render functions
fn render_all(tcod: &mut Tcod, game: &Game, objects: &[Object]) {
    for obj in objects {
        obj.draw(&mut tcod.con);
    }

    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let wall = game.map[x as usize][y as usize].block_sight;
            if wall {
                tcod.con.set_char_background(x, y, DARK_WALL, BackgroundFlag::Set);
            } else {
                tcod.con.set_char_background(x, y, DARK_GROUND, BackgroundFlag::Set);
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
    let mut tcod = Tcod{ root, con };

    // limit FPS (doesn't really matter for a key inpyt roguelike)
    tcod::system::set_fps(LIMIT_FPS);

    // Game objects
    let player = Object::new(0, 0, '@', WHITE);
    let npc = Object::new(MAP_WIDTH / 2 - 5, MAP_HEIGHT / 2, '@', YELLOW);
    let mut objects =  [player, npc];
    let game = Game { map: make_map(&mut objects[0]) };

    // It's a game; it needs a game loop
    while !tcod.root.window_closed() {

        tcod.con.clear();
        render_all(&mut tcod, &game, &objects);
        tcod.root.flush();

        let player = &mut objects[0];
        let exit = handle_keys(&mut tcod, &game, player);
        if exit { break; }
    }
}
